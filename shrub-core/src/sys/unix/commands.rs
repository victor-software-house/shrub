//! Command execution utilities.

pub use std::os::unix::process::{CommandExt, ExitStatusExt};

use crate::{ShellFd, error, openfiles};

/// Gives `spawn`'s child each of `open_files` as the descriptor it is keyed
/// by, beside its standard streams.
///
/// # Errors
///
/// An error if a file's descriptor cannot be duplicated.
pub(crate) fn map_fds(
    mut spawn: shrub_process::Spawn,
    open_files: impl Iterator<Item = (ShellFd, openfiles::OpenFile)>,
) -> Result<shrub_process::Spawn, error::Error> {
    for (child_fd, open_file) in open_files {
        spawn = spawn.fd(child_fd, open_file.try_clone_to_owned()?);
    }
    Ok(spawn)
}
