//! The child setup where no step can run between the fork and `exec`: a
//! request that asks for one fails, so no process runs without the setup it
//! was meant to have.

use std::ffi::CStr;
use std::process::Command;

use crate::{Error, Spawn};

/// The command for `request`.
pub(crate) fn command(request: &mut Spawn) -> Result<Command, Error> {
    let options = &request.options;
    if options.cgroup_procs.is_some() {
        return Err(Error::Unsupported("joining a cgroup"));
    }
    if !options.pids_limits.is_empty() {
        return Err(Error::Unsupported("checking process counts"));
    }
    if options.open_file_limit.is_some() {
        return Err(Error::Unsupported("setting an open-file limit"));
    }
    if options.process_limit.is_some() {
        return Err(Error::Unsupported("setting a process limit"));
    }
    // `argv[0]`, the process group and the terminal are Unix's; elsewhere
    // the child runs as the platform starts it.
    Ok(request.base_command())
}

/// Spawns `command` on tokio's process driver.
#[cfg(windows)]
pub(crate) fn spawn(
    command: Command,
    kill_on_drop: bool,
) -> std::io::Result<tokio::process::Child> {
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(kill_on_drop);
    command.spawn()
}

/// Spawns `command`; there is no kill on drop here.
#[cfg(not(windows))]
pub(crate) fn spawn(mut command: Command, _kill_on_drop: bool) -> std::io::Result<Child> {
    Ok(Child {
        inner: command.spawn()?,
    })
}

/// Refuses: this platform cannot replace a process.
pub(crate) fn exec(_command: Command) -> Error {
    Error::Unsupported("replacing the process")
}

/// Whether nothing exists at `path`. No command joins a cgroup here, so a
/// spawn never fails for a missing `cgroup.procs` file.
pub(crate) const fn is_missing(_path: &CStr) -> bool {
    false
}

/// A started child, where tokio has no process driver.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct Child {
    inner: std::process::Child,
}

#[cfg(not(windows))]
impl Child {
    /// The child's process id, which this platform does not report.
    pub const fn id(&self) -> Option<u32> {
        None
    }

    /// Waits for the child to exit.
    ///
    /// # Errors
    ///
    /// The I/O error from waiting on the child.
    pub async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.inner.wait()
    }

    /// Waits for the child to exit and collects its output.
    ///
    /// # Errors
    ///
    /// The I/O error from waiting on the child or collecting its output.
    pub async fn wait_with_output(self) -> std::io::Result<std::process::Output> {
        self.inner.wait_with_output()
    }
}
