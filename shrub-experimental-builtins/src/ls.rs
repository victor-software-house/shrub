use std::io::Write;

use clap::Parser;
use shrub_core::listing::ListedEntry;
use shrub_core::{ExecutionResult, builtins};

use crate::program::{run_program, split_args};

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
        mut context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let (options, paths) = split_args(&self.args, &[]);
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
            return run_program(&mut context, "ls", &self.args, None).await;
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
        run_program(&mut context, "ls", &rest, None).await
    }
}
