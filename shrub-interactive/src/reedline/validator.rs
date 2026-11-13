use crate::{completeness, refs};

pub(crate) struct ReedlineValidator<SE: shrub_core::ShellExtensions> {
    pub shell: refs::ShellRef<SE>,
}

impl<SE: shrub_core::ShellExtensions> reedline::Validator for ReedlineValidator<SE> {
    fn validate(&self, line: &str) -> reedline::ValidationResult {
        if completeness::needs_more_input(&self.shell, line) {
            reedline::ValidationResult::Incomplete
        } else {
            reedline::ValidationResult::Complete
        }
    }
}
