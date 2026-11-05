//! Directories an embedder lists itself.
//!
//! An embedder can describe some directories better than the file system's
//! names do: a directory whose entries it renders, such as a history it
//! keeps, with one line of text per entry. It sets a [`DirectoryLister`] in
//! the shell's [`RuntimeOptions`](crate::options::RuntimeOptions), and the
//! builtins that list directories (`ls` and `tree` in
//! `shrub-experimental-builtins`) ask it first. A directory the lister does
//! not answer for is listed as it would be without one.
//!
//! The lister is part of the runtime options, so pipelines, background jobs
//! and subshells see it too. It is not serialized: an embedder that restores a
//! shell sets it again.

use std::path::Path;

/// One entry of a directory a [`DirectoryLister`] answers for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedEntry {
    /// The entry's name in its directory.
    pub name: String,
    /// Whether the entry is a directory, which `tree` descends into.
    pub is_dir: bool,
    /// The line a listing shows for the entry: its name, and whatever the
    /// embedder says of it.
    pub line: String,
}

/// Lists the directories an embedder answers for.
pub trait DirectoryLister: Send + Sync {
    /// The entries of the directory at `path`, an absolute path, in the
    /// order a listing shows them, or `None` when the lister does not answer
    /// for `path`.
    fn list(&self, path: &Path) -> Option<Vec<ListedEntry>>;
}
