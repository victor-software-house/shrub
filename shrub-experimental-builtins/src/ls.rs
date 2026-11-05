use std::io::Write;

use clap::Parser;
use shrub_core::listing::ListedEntry;
use shrub_core::{ExecutionResult, builtins, commands};

/// (*EXPERIMENTAL*) Lists a directory. A directory the embedder lists itself
/// shows one line per entry, as the embedder writes it; any other path is
/// listed by the `ls` program found in `PATH`, with the same arguments.
#[derive(Parser)]
#[command(disable_help_flag = true)]
pub(crate) struct LsCommand {
    /// Options and paths, as `ls` takes them.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

impl builtins::Command for LsCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let (options, paths) = split_args(&self.args);
        let lister = context.shell.options().directory_lister.clone();
        let operands: Vec<&str> = if paths.is_empty() {
            vec!["."]
        } else {
            paths.clone()
        };
        let mut listed: Vec<(&str, Vec<ListedEntry>)> = Vec::new();
        let mut others: Vec<&str> = Vec::new();
        for &operand in &operands {
            let entries = lister
                .as_ref()
                .and_then(|lister| lister.list(&context.shell.absolute_path(operand)));
            match entries {
                Some(entries) => listed.push((operand, entries)),
                None => others.push(operand),
            }
        }
        if listed.is_empty() {
            return run_program(context, "ls", &self.args).await;
        }
        let headed = operands.len() > 1;
        for (index, (operand, entries)) in listed.iter().enumerate() {
            if headed {
                if index > 0 {
                    writeln!(context.stdout())?;
                }
                writeln!(context.stdout(), "{operand}:")?;
            }
            for entry in entries {
                writeln!(context.stdout(), "{}", entry.line)?;
            }
        }
        if others.is_empty() {
            return Ok(ExecutionResult::success());
        }
        if headed {
            writeln!(context.stdout())?;
        }
        let rest: Vec<String> = options
            .into_iter()
            .chain(others)
            .map(str::to_owned)
            .collect();
        run_program(context, "ls", &rest).await
    }
}

/// `args` split into options, which start with `-`, and paths; a `--` ends
/// the options, and every word after it is a path.
pub(crate) fn split_args(args: &[String]) -> (Vec<&str>, Vec<&str>) {
    let mut options = Vec::new();
    let mut paths = Vec::new();
    let mut words = args.iter().map(String::as_str);
    for word in words.by_ref() {
        if word == "--" {
            break;
        }
        if word.len() > 1 && word.starts_with('-') {
            options.push(word);
        } else {
            paths.push(word);
        }
    }
    paths.extend(words);
    (options, paths)
}

/// Runs the program `name` found in `PATH`, never a builtin or function of
/// that name, with `args`, and returns its result; a missing program is
/// reported as the shell reports a command not found.
pub(crate) async fn run_program(
    context: shrub_core::ExecutionContext<'_, impl shrub_core::ShellExtensions>,
    name: &str,
    args: &[String],
) -> Result<ExecutionResult, shrub_core::Error> {
    let Some(program) = context.shell.find_first_executable_in_path(name) else {
        writeln!(context.stderr(), "{name}: command not found")?;
        return Ok(ExecutionResult::new(127));
    };
    // A name with a `/` is run as that file, so no builtin or function of
    // the same name answers it.
    let command_name = program.to_string_lossy().into_owned();
    let words = std::iter::once(command_name.clone())
        .chain(args.iter().cloned())
        .map(shrub_core::CommandArg::from);
    let mut command = commands::SimpleCommand::new(
        commands::ShellForCommand::ParentShell(context.shell),
        context.params,
        command_name,
        words,
    );
    command.use_functions = false;
    command.argv0 = Some(name.to_owned());
    let spawned = command.execute().await?;
    Ok(spawned.wait().await?.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn options_and_paths_are_told_apart() {
        let args = words(&["-la", "turns", "-", "--", "-odd"]);
        let (options, paths) = split_args(&args);
        assert_eq!(options, ["-la"]);
        assert_eq!(paths, ["turns", "-", "-odd"]);
    }
}
