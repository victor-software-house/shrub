use std::io::Write;

use shrub_core::openfiles::{OpenFile, OpenFiles};
use shrub_core::{ExecutionResult, commands};

/// Runs the program `name` found in `PATH`, never a builtin or function of
/// that name, with `args`, and with `stdin` as its standard input when one is
/// given, and returns its result; a missing program is reported as the shell
/// reports a command not found.
pub(crate) async fn run_program(
    context: &mut shrub_core::ExecutionContext<'_, impl shrub_core::ShellExtensions>,
    name: &str,
    args: &[String],
    stdin: Option<OpenFile>,
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
    let mut params = context.params.clone();
    if let Some(stdin) = stdin {
        params.set_fd(OpenFiles::STDIN_FD, stdin);
    }
    let mut command = commands::SimpleCommand::new(
        commands::ShellForCommand::ParentShell(&mut *context.shell),
        params,
        command_name,
        words,
    );
    command.use_functions = false;
    command.argv0 = Some(name.to_owned());
    let spawned = command.execute().await?;
    Ok(spawned.wait().await?.into())
}

/// `args` split into options, which start with `-`, and paths; an option in
/// `with_value`, such as `head`'s `-n`, takes the next word with it, and a
/// `--` ends the options, every word after it a path.
pub(crate) fn split_args<'a>(
    args: &'a [String],
    with_value: &[&str],
) -> (Vec<&'a str>, Vec<&'a str>) {
    let mut options = Vec::new();
    let mut paths = Vec::new();
    let mut words = args.iter().map(String::as_str);
    while let Some(word) = words.next() {
        if word == "--" {
            break;
        }
        if word.len() > 1 && word.starts_with('-') {
            options.push(word);
            if with_value.contains(&word) {
                options.extend(words.next());
            }
        } else {
            paths.push(word);
        }
    }
    paths.extend(words);
    (options, paths)
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
        let (options, paths) = split_args(&args, &[]);
        assert_eq!(options, ["-la"]);
        assert_eq!(paths, ["turns", "-", "-odd"]);
    }

    #[test]
    fn an_option_with_a_value_keeps_its_value() {
        let args = words(&["-n", "5", "out", "-c5", "more"]);
        let (options, paths) = split_args(&args, &["-n", "-c"]);
        assert_eq!(options, ["-n", "5", "-c5"]);
        assert_eq!(paths, ["out", "more"]);
    }
}
