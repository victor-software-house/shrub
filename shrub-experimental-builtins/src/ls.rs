use std::io::Write;

use clap::Parser;
use shrub_core::listing::ListedEntry;
use shrub_core::{ExecutionResult, builtins};

use crate::program::{run_program, split_args};

/// (*EXPERIMENTAL*) Lists a directory. A directory the embedder lists itself
/// shows one line per entry, as the embedder writes it; any other path is
/// listed by the `ls` program found in `PATH`, with the same options. With
/// several operands, each is listed in the order given under its own header.
#[derive(Parser)]
#[command(disable_help_flag = true)]
pub(crate) struct LsCommand {
    /// Options and paths, as `ls` takes them.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

/// The options of GNU `ls` that take the next word as their value. BSD `ls`
/// has none, and reads `-I` and `-w` as flags, so on BSD a path right after
/// one of them is taken as its value.
const WITH_VALUE: &[&str] = &["-I", "-T", "-w"];

impl builtins::Command for LsCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        mut context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let (options, paths) = split_args(&self.args, WITH_VALUE);
        let lister = context.shell.options().directory_lister.clone();
        let operands: Vec<&str> = if paths.is_empty() {
            vec!["."]
        } else {
            paths.clone()
        };
        let listed: Vec<Option<Vec<ListedEntry>>> = operands
            .iter()
            .map(|operand| {
                lister
                    .as_ref()
                    .and_then(|lister| lister.list(&context.shell.absolute_path(operand)))
            })
            .collect();
        if listed.iter().all(Option::is_none) {
            return run_program(&mut context, "ls", &self.args, None).await;
        }
        let headed = operands.len() > 1;
        let mut failed = None;
        for (index, (operand, entries)) in operands.iter().zip(listed).enumerate() {
            if headed {
                if index > 0 {
                    writeln!(context.stdout())?;
                }
                writeln!(context.stdout(), "{operand}:")?;
            }
            if let Some(entries) = entries {
                for entry in entries {
                    writeln!(context.stdout(), "{}", entry.line)?;
                }
                continue;
            }
            // `ls` given one operand prints no header, and `--` keeps a path
            // that starts with `-` a path.
            let args: Vec<String> = options
                .iter()
                .copied()
                .chain(["--", operand])
                .map(str::to_owned)
                .collect();
            let result = run_program(&mut context, "ls", &args, None).await?;
            if failed.is_none() && !result.is_success() {
                failed = Some(result);
            }
        }
        Ok(failed.unwrap_or_else(ExecutionResult::success))
    }
}
