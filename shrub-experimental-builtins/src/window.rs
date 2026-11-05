use std::io::{Seek, SeekFrom, Write};

use clap::Parser;
use shrub_core::{ExecutionResult, builtins};

use crate::program::{run_program, split_args};

/// (*EXPERIMENTAL*) `cat`, `head` or `tail`, by the name it is registered
/// under, through the program of that name found in `PATH`. A file the
/// embedder's read window answers for is opened at the first byte it names
/// and given to the program on its standard input, so the program's own
/// options and output stay; with no such file, every word goes to the program
/// unchanged.
#[derive(Parser)]
#[command(disable_help_flag = true)]
pub(crate) struct WindowCommand {
    /// Options and paths, as the program takes them.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

/// The options of `head` and `tail` that take the next word as their value;
/// `cat` has none.
const WITH_VALUE: &[&str] = &["-n", "-c"];

impl builtins::Command for WindowCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        mut context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let name = context.command_name.clone();
        let (options, paths) = split_args(&self.args, WITH_VALUE);
        let window = context.shell.options().read_window.clone();
        let operands: Vec<(&str, Option<u64>)> = paths
            .iter()
            .map(|path| {
                let start = window
                    .as_ref()
                    .and_then(|window| window.first_byte(&context.shell.absolute_path(path)));
                (*path, start)
            })
            .collect();
        if operands.iter().all(|(_, start)| start.is_none()) {
            return run_program(&mut context, &name, &self.args, None).await;
        }
        let options: Vec<String> = options.into_iter().map(str::to_owned).collect();
        // The program reads one operand at a time, so it prints no header of
        // its own; `head` and `tail` print one per file when given several.
        let headed = name != "cat" && operands.len() > 1;
        let mut failed = None;
        for (index, (path, start)) in operands.into_iter().enumerate() {
            if headed {
                if index > 0 {
                    writeln!(context.stdout())?;
                }
                writeln!(context.stdout(), "==> {path} <==")?;
            }
            let result = if let Some(start) = start {
                match open_at(&context.shell.absolute_path(path), start) {
                    Ok(file) => run_program(&mut context, &name, &options, Some(file)).await?,
                    Err(error) => {
                        writeln!(context.stderr(), "{name}: {path}: {error}")?;
                        ExecutionResult::general_error()
                    }
                }
            } else {
                let mut args = options.clone();
                args.push(path.to_owned());
                run_program(&mut context, &name, &args, None).await?
            };
            if failed.is_none() && !result.is_success() {
                failed = Some(result);
            }
        }
        Ok(failed.unwrap_or_else(ExecutionResult::success))
    }
}

/// The file at `path`, opened for reading at byte `start`.
fn open_at(path: &std::path::Path, start: u64) -> std::io::Result<std::fs::File> {
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    Ok(file)
}
