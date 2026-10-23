//! Command execution utilities.

use std::ffi::{CStr, CString};
pub use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::sync::Arc;

use command_fds::{CommandFdExt, FdMapping};

use crate::{ShellFd, error, openfiles};

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
    /// - An error if a file's descriptor cannot be duplicated.
    /// - [`crate::error::ErrorKind::ChildCreationFailure`] if the descriptor
    ///   mappings cannot be applied to the command, for example because two
    ///   files map to the same child descriptor.
    fn inject_fds(
        &mut self,
        open_files: impl Iterator<Item = (ShellFd, openfiles::OpenFile)>,
    ) -> Result<(), error::Error>;
}

impl CommandFdInjectionExt for std::process::Command {
    fn inject_fds(
        &mut self,
        open_files: impl Iterator<Item = (ShellFd, openfiles::OpenFile)>,
    ) -> Result<(), error::Error> {
        let fd_mappings: Vec<FdMapping> = open_files
            .map(|(child_fd, open_file)| -> Result<FdMapping, error::Error> {
                let parent_fd = open_file.try_clone_to_owned()?;
                Ok(FdMapping {
                    child_fd,
                    parent_fd,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.fd_mappings(fd_mappings)
            .map_err(|_e| error::ErrorKind::ChildCreationFailure)?;

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
        // SAFETY:
        // This arranges for a provided function to run in the context of
        // the forked process before it exec's the target command. In general,
        // rust can't guarantee safety of code running in such a context.
        unsafe {
            self.pre_exec(pre_exec_take_foreground);
        }
    }

    fn lead_session(&mut self) {
        // SAFETY:
        // This arranges for a provided function to run in the context of
        // the forked process before it exec's the target command. In general,
        // rust can't guarantee safety of code running in such a context.
        unsafe {
            self.pre_exec(pre_exec_lead_session);
        }
    }
}

fn pre_exec_take_foreground() -> Result<(), std::io::Error> {
    use crate::sys;

    sys::terminal::move_self_to_foreground()?;
    Ok(())
}

fn pre_exec_lead_session() -> Result<(), std::io::Error> {
    if let Err(e) = nix::unistd::setsid() {
        return Err(std::io::Error::other(format!(
            "failed to become session leader: {e}"
        )));
    }

    #[cfg(not(target_os = "macos"))]
    let control = libc::TIOCSCTTY;
    #[cfg(target_os = "macos")]
    let control: u64 = libc::TIOCSCTTY.into();

    // SAFETY:
    // This is calling a libc function to set the controlling terminal.
    let result = unsafe { libc::ioctl(0, control, 0) };
    if result != 0 {
        return Err(std::io::Error::other("failed to set controlling terminal"));
    }

    Ok(())
}

/// Extension trait for moving a command into a cgroup before it runs.
pub trait CommandCgroupExt {
    /// Arranges for the command to write `0` to `cgroup_procs`, a cgroup v2's
    /// `cgroup.procs` file, after it is forked and before it runs its program,
    /// which moves it into that cgroup.
    ///
    /// # Errors
    ///
    /// None here: a child that cannot open or write the file fails its spawn.
    fn join_cgroup(&mut self, cgroup_procs: Arc<CString>) -> Result<(), error::Error>;
}

impl CommandCgroupExt for std::process::Command {
    fn join_cgroup(&mut self, cgroup_procs: Arc<CString>) -> Result<(), error::Error> {
        // SAFETY:
        // This arranges for a provided function to run in the context of
        // the forked process before it exec's the target command. The
        // function allocates nothing and only calls open, write and close,
        // which are async-signal-safe.
        unsafe {
            self.pre_exec(move || pre_exec_join_cgroup(&cgroup_procs));
        }
        Ok(())
    }
}

fn pre_exec_join_cgroup(cgroup_procs: &CStr) -> Result<(), std::io::Error> {
    use nix::fcntl::{OFlag, open};
    use nix::sys::stat::Mode;

    let file = open(
        cgroup_procs,
        OFlag::O_WRONLY | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?;
    nix::unistd::write(&file, b"0")?;
    Ok(())
}

/// Extension trait for setting a command's open-file limit before it runs.
pub trait CommandOpenFileLimitExt {
    /// Arranges for the command to set its soft `RLIMIT_NOFILE` to `limit`,
    /// or to its hard limit if that is lower, after it is forked and before
    /// it runs its program.
    ///
    /// # Errors
    ///
    /// None here: a child that cannot set the limit fails its spawn.
    fn set_open_file_limit(&mut self, limit: u64) -> Result<(), error::Error>;
}

impl CommandOpenFileLimitExt for std::process::Command {
    fn set_open_file_limit(&mut self, limit: u64) -> Result<(), error::Error> {
        // SAFETY:
        // This arranges for a provided function to run in the context of
        // the forked process before it exec's the target command. The
        // function allocates nothing and only calls getrlimit and
        // setrlimit, which are async-signal-safe.
        unsafe {
            self.pre_exec(move || pre_exec_set_open_file_limit(limit));
        }
        Ok(())
    }
}

/// The highest soft open-file limit macOS always accepts: `OPEN_MAX` in
/// `<sys/syslimits.h>`. Its `setrlimit(2)` page says to use
/// `min(OPEN_MAX, rlim_max)` where a higher soft limit fails with `EINVAL`.
#[cfg(target_os = "macos")]
const MACOS_OPEN_MAX: libc::rlim_t = 10240;

fn pre_exec_set_open_file_limit(limit: u64) -> Result<(), std::io::Error> {
    use nix::sys::resource::{Resource, getrlimit, setrlimit};

    let (_, hard) = getrlimit(Resource::RLIMIT_NOFILE)?;
    // `rlim_t` is `u64` on Linux and macOS but `i64` on FreeBSD; a limit
    // past its range sets the hard limit, as any limit above it does.
    #[allow(
        clippy::unnecessary_fallible_conversions,
        reason = "rlim_t is not u64 on every Unix"
    )]
    let soft = libc::rlim_t::try_from(limit).unwrap_or(hard).min(hard);
    let result = setrlimit(Resource::RLIMIT_NOFILE, soft, hard);
    #[cfg(target_os = "macos")]
    let result = match result {
        Err(nix::errno::Errno::EINVAL) => {
            setrlimit(Resource::RLIMIT_NOFILE, soft.min(MACOS_OPEN_MAX), hard)
        }
        other => other,
    };
    Ok(result?)
}

/// Whether nothing exists at `path`, such as a `cgroup.procs` file whose
/// cgroup was removed.
pub(crate) fn is_missing(path: &CStr) -> bool {
    use std::os::unix::ffi::OsStrExt as _;

    !std::path::Path::new(std::ffi::OsStr::from_bytes(path.to_bytes())).exists()
}
