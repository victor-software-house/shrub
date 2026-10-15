use shrub_core::{ExecutionResult, builtins, error};

/// No-op command.
pub(crate) struct ColonCommand {}

impl builtins::SimpleCommand for ColonCommand {
    fn get_content(
        _name: &str,
        content_type: builtins::ContentType,
        _options: &builtins::ContentOptions,
    ) -> Result<String, shrub_core::Error> {
        match content_type {
            builtins::ContentType::DetailedHelp => {
                Ok("Null command; always returns success.".into())
            }
            builtins::ContentType::ShortUsage => Ok(":: :".into()),
            builtins::ContentType::ShortDescription => Ok(": - Null command".into()),
            builtins::ContentType::ManPage => error::unimp("man page not yet implemented"),
        }
    }

    fn execute<SE: shrub_core::ShellExtensions, I: Iterator<Item = S>, S: AsRef<str>>(
        _context: shrub_core::ExecutionContext<'_, SE>,
        _args: I,
    ) -> Result<ExecutionResult, shrub_core::Error> {
        Ok(ExecutionResult::success())
    }
}
