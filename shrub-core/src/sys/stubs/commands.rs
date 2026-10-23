//! Command execution utilities.

use std::ffi::{CString, OsStr};
use std::sync::Arc;

use crate::{ShellFd, error, openfiles};

/// Extension trait for Unix-like command extensions.
pub trait CommandExt {
    /// Sets the zeroth argument (argv[0]) of the command.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument to set as argv[0].
    fn arg0<S>(&mut self, arg: S) -> &mut Self
    where
        S: AsRef<OsStr>;

    /// Sets the process group ID of the command.
    ///
    /// # Arguments
    ///
    /// * `pgroup` - The process group ID to set.
    fn process_group(&mut self, pgroup: i32) -> &mut Self;
}

impl CommandExt for std::process::Command {
    fn arg0<S>(&mut self, _arg: S) -> &mut Self
    where
        S: AsRef<OsStr>,
    {
        // NOTE: no-op.
        self
    }

    fn process_group(&mut self, _pgroup: i32) -> &mut Self {
        // NOTE: no-op.
        self
    }
}

/// Extension trait for Unix-like exit status extensions.
pub trait ExitStatusExt {
    /// Returns the signal that terminated the process, if any.
    fn signal(&self) -> Option<i32>;
}

impl ExitStatusExt for std::process::ExitStatus {
    fn signal(&self) -> Option<i32> {
        None
    }
}

/// Extension trait for injecting file descriptors into commands.
pub trait CommandFdInjectionExt {
    /// Injects the given open files as file descriptors into the command.
    ///
    /// # Arguments
    ///
    /// * `open_files` - A mapping of child file descriptors to open files.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ErrorKind::NotSupportedOnThisPlatform`] if
    /// `open_files` yields any file, because this platform cannot map file
    /// descriptors into children.
    fn inject_fds(
        &mut self,
        open_files: impl Iterator<Item = (ShellFd, openfiles::OpenFile)>,
    ) -> Result<(), error::Error>;
}

impl CommandFdInjectionExt for std::process::Command {
    fn inject_fds(
        &mut self,
        mut open_files: impl Iterator<Item = (ShellFd, openfiles::OpenFile)>,
    ) -> Result<(), error::Error> {
        if open_files.next().is_some() {
            return Err(error::ErrorKind::NotSupportedOnThisPlatform("fd redirections").into());
        }

        Ok(())
    }
}

/// Extension trait for arranging for commands to take the foreground.
pub trait CommandFgControlExt {
    /// Arranges for the command to take the foreground when it is executed.
    fn take_foreground(&mut self);
    /// Arranges for the command to become a session leader when it is executed.
    fn lead_session(&mut self);
}

impl CommandFgControlExt for std::process::Command {
    fn take_foreground(&mut self) {
        // NOTE: This is a no-op.
    }

    fn lead_session(&mut self) {
        // NOTE: This is a no-op.
    }
}

/// Extension trait for moving a command into a cgroup before it runs.
pub trait CommandCgroupExt {
    /// Fails: only Linux has cgroups, so a command cannot join one here.
    ///
    /// # Errors
    ///
    /// Always, so that no command runs outside the cgroup it was meant for.
    fn join_cgroup(&mut self, cgroup_procs: Arc<CString>) -> Result<(), error::Error>;
}

impl CommandCgroupExt for std::process::Command {
    fn join_cgroup(&mut self, _cgroup_procs: Arc<CString>) -> Result<(), error::Error> {
        Err(error::ErrorKind::NotSupportedOnThisPlatform("joining a cgroup").into())
    }
}

/// Extension trait for setting a command's open-file limit before it runs.
pub trait CommandOpenFileLimitExt {
    /// Fails: this platform has no resource limits to set.
    ///
    /// # Errors
    ///
    /// Always, so that no command runs with a limit other than the one it was
    /// meant to have.
    fn set_open_file_limit(&mut self, limit: u64) -> Result<(), error::Error>;
}

impl CommandOpenFileLimitExt for std::process::Command {
    fn set_open_file_limit(&mut self, _limit: u64) -> Result<(), error::Error> {
        Err(error::ErrorKind::NotSupportedOnThisPlatform("setting an open-file limit").into())
    }
}

/// Whether nothing exists at `path`. No command joins a cgroup here, so a
/// spawn never fails for a missing `cgroup.procs` file.
pub(crate) const fn is_missing(_path: &std::ffi::CStr) -> bool {
    false
}
