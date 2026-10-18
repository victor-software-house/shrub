use shrub_core::{ExecutionResult, builtins, error};

/// No-op command. Same with :.
pub(crate) struct TrueCommand {}

impl builtins::SimpleCommand for TrueCommand {
    fn get_content(
        _name: &str,
        content_type: builtins::ContentType,
        _options: &builtins::ContentOptions,
    ) -> Result<String, shrub_core::Error> {
        match content_type {
            builtins::ContentType::DetailedHelp => Ok("Returns a successful exit status.".into()),
            builtins::ContentType::ShortUsage => Ok("true".into()),
            builtins::ContentType::ShortDescription => Ok("true - success".into()),
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
