//! Readline edit buffer support for shell instances.

use crate::variables::ShellVariable;
use crate::{error, extensions};

impl<SE: extensions::ShellExtensions> crate::Shell<SE> {
    /// Updates the shell state to reflect the given edit buffer contents.
    ///
    /// # Arguments
    ///
    /// * `contents` - The contents of the edit buffer.
    /// * `cursor` - The cursor position in the edit buffer.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ErrorKind::MissingScopeForNewVariable`] if the
    /// environment has no global scope to hold `READLINE_LINE` and
    /// `READLINE_POINT`.
    pub fn set_edit_buffer(&mut self, contents: String, cursor: usize) -> Result<(), error::Error> {
        self.env
            .set_global("READLINE_LINE", ShellVariable::new(contents))?;

        self.env
            .set_global("READLINE_POINT", ShellVariable::new(cursor.to_string()))?;

        Ok(())
    }

    /// Returns the contents of the shell's edit buffer, if any. The buffer
    /// state is cleared from the shell.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ErrorKind::ReadonlyVariable`] if `READLINE_LINE`
    /// or `READLINE_POINT` is readonly and so cannot be unset.
    pub fn pop_edit_buffer(&mut self) -> Result<Option<(String, usize)>, error::Error> {
        let line = self
            .env
            .unset("READLINE_LINE")?
            .map(|line| line.value().to_cow_str(self).to_string());

        let point = self
            .env
            .unset("READLINE_POINT")?
            .and_then(|point| point.value().to_cow_str(self).parse::<usize>().ok())
            .unwrap_or(0);

        if let Some(line) = line {
            Ok(Some((line, point)))
        } else {
            Ok(None)
        }
    }
}
