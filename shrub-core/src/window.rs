//! Files an embedder serves from a later first byte.
//!
//! An embedder may keep a file whose oldest bytes it dropped in place, such as
//! a command's output kept within a budget, where the dropped range reads back
//! as zero bytes. It sets a [`ReadWindow`] in the shell's
//! [`RuntimeOptions`](crate::options::RuntimeOptions), and the builtins that
//! read files (`cat`, `head` and `tail` in `shrub-experimental-builtins`)
//! start such a file at the first byte it names. A file the window does not
//! answer for is read from its start.
//!
//! The window is part of the runtime options, so pipelines, background jobs
//! and subshells see it too. It is not serialized: an embedder that restores a
//! shell sets it again.

use std::path::Path;

/// Names the first byte to read of the files an embedder answers for.
pub trait ReadWindow: Send + Sync {
    /// The first byte to read of the file at `path`, an absolute path, or
    /// `None` when the window does not answer for `path`.
    fn first_byte(&self, path: &Path) -> Option<u64>;
}
