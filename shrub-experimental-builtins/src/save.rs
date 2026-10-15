use shrub_core::{ExecutionResult, builtins};
use clap::Parser;
use std::io::Write;

/// (*EXPERIMENTAL*) Serializes the current shell state to JSON and writes it to stdout.
/// Beware that the serialized state may include sensitive information, such as any
/// secrets stored in shell variables or referenced in command history.
#[derive(Parser)]
pub(crate) struct SaveCommand {}

impl builtins::Command for SaveCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let serialized_str = serde_json::to_string(&context.shell).map_err(|e| {
            shrub_core::Error::from(shrub_core::ErrorKind::InternalError(e.to_string()))
        })?;

        writeln!(context.stdout(), "{serialized_str}")?;

        Ok(ExecutionResult::success())
    }
}
