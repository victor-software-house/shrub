use shrub_core::extensions::ShellExtensions;

use crate::BuiltinSet;

/// Extension trait that simplifies adding default builtins to a shell builder.
pub trait ShellBuilderExt {
    /// Add default builtins to the shell being built.
    ///
    /// # Arguments
    ///
    /// * `set` - The well-known set of built-ins to add.
    #[must_use]
    fn default_builtins(self, set: BuiltinSet) -> Self;
}

impl<SE: ShellExtensions, S: shrub_core::ShellBuilderState> ShellBuilderExt
    for shrub_core::ShellBuilder<SE, S>
{
    fn default_builtins(self, set: BuiltinSet) -> Self {
        self.builtins(crate::default_builtins(set))
    }
}
