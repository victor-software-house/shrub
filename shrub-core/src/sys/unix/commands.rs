//! Command execution utilities.
//!
//! # Why this module is unsafe
//!
//! 1. Safe code cannot do it. Each step here runs in a command's child after
//!    the fork and before `exec`: taking the terminal's foreground, leading a
//!    session and taking its controlling terminal, joining a cgroup, setting
//!    the open-file and process limits and checking process counts. The
//!    standard library's only hook there is `CommandExt::pre_exec`, which is
//!    unsafe; its safe `process_group` sets a process group only, and its
//!    `setsid` is unstable. `TIOCSCTTY` has no safe wrapper, and nix's
//!    `tcsetpgrp` needs a borrowed descriptor, which for descriptor 0 means the
//!    unsafe `BorrowedFd::borrow_raw` or `std::io::stdin()`, whose first call
//!    allocates.
//! 2. Speed is not the reason.
//! 3. The unsafe operations are the `pre_exec` registrations in the `Command`
//!    extension traits, and the `tcsetpgrp` and `ioctl` calls in the child.
//!
//! Each function a child runs before `exec` allocates nothing, error paths
//! included: its errors come from an errno. `std::process::Command` runs it
//! after the fork, with the command's descriptors in place.

use std::ffi::{CStr, CString};
pub use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::sync::Arc;

use command_fds::{CommandFdExt, FdMapping};

use crate::options::PidsLimit;
use crate::{ShellFd, error, openfiles};

/// The bytes a child reads from a `pids.current` file: more than the longest
/// count, a `u64` of 20 digits and a newline, so a full buffer means the file
/// holds something else.
const PIDS_CURRENT_BUFFER: usize = 32;

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

        // `fd_mappings` registers a `pre_exec` closure even for an empty
        // list, and any closure makes the standard library fork and exec
        // instead of calling `posix_spawn`.
        if fd_mappings.is_empty() {
            return Ok(());
        }

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
        // SAFETY: `pre_exec` requires a closure that is safe to run in a
        // forked child of a process that may have other threads: no
        // allocation, no lock, no `std::env`. `pre_exec_take_foreground`
        // calls only `getpgrp` and `tcsetpgrp`, both async-signal-safe
        // (signal-safety(7)), and allocates nothing.
        unsafe {
            self.pre_exec(|| {
                pre_exec_take_foreground();
                Ok(())
            });
        }
    }

    fn lead_session(&mut self) {
        // SAFETY: `pre_exec` requires a closure that is safe to run in a
        // forked child of a process that may have other threads: no
        // allocation, no lock, no `std::env`. `pre_exec_lead_session` calls
        // only `setsid`, which POSIX lists as async-signal-safe
        // (signal-safety(7)), and `ioctl`, which it does not list. Each
        // target libc makes `ioctl` one system call with no lock or
        // allocation:
        // - glibc: https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/unix/sysv/linux/ioctl.c;hb=9e77a668c94f892f2ce0ad9ef7f6e88525852af3
        // - musl: https://git.musl-libc.org/cgit/musl/tree/src/misc/ioctl.c?id=7813e9119eb5fe681b6750b8a033a0490bca0941
        // - macOS: https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.1.9/libsyscall/wrappers/ioctl.c
        // The function allocates nothing, and its errors come from the
        // errno.
        unsafe {
            self.pre_exec(pre_exec_lead_session);
        }
    }
}

/// Moves the child's process group to the foreground of the terminal on its
/// standard input. It uses descriptor 0 directly, because the first call to
/// `std::io::stdin()` allocates its buffer.
fn pre_exec_take_foreground() {
    let pgrp = nix::unistd::getpgrp();
    // SAFETY: `tcsetpgrp` takes a descriptor number and a process group by
    // value and reads no memory of this process. When descriptor 0 is closed
    // or is not this session's terminal, it fails with `EBADF` or `ENOTTY`,
    // which is ignored, as `move_self_to_foreground` ignores it.
    let _ = unsafe { libc::tcsetpgrp(0, pgrp.as_raw()) };
}

fn pre_exec_lead_session() -> Result<(), std::io::Error> {
    nix::unistd::setsid()?;

    #[cfg(not(target_os = "macos"))]
    let control = libc::TIOCSCTTY;
    #[cfg(target_os = "macos")]
    let control: u64 = libc::TIOCSCTTY.into();

    // SAFETY: `TIOCSCTTY` takes its argument by value, not through a
    // pointer, so the call reads and writes no memory of this process. On
    // a descriptor 0 that is not a terminal it fails with an errno.
    let result = unsafe { libc::ioctl(0, control, 0) };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
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
        // SAFETY: `pre_exec` requires a closure that is safe to run in a
        // forked child of a process that may have other threads: no
        // allocation, no lock, no `std::env`. The closure reads the captured
        // path without cloning its `Arc`, and `pre_exec_join_cgroup` calls
        // only `open`, `write` and `close`, all async-signal-safe
        // (signal-safety(7)); it allocates nothing, and its errors come from
        // the errno.
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
        // SAFETY: `pre_exec` requires a closure that is safe to run in a
        // forked child of a process that may have other threads: no
        // allocation, no lock, no `std::env`. `pre_exec_set_open_file_limit`
        // calls only `getrlimit` and `setrlimit`, which POSIX does not list
        // as async-signal-safe (signal-safety(7)). Each target libc makes
        // each one system call with no lock or allocation:
        // - glibc: `prlimit64`, https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/unix/sysv/linux/getrlimit64.c;hb=9e77a668c94f892f2ce0ad9ef7f6e88525852af3
        //   and https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/unix/sysv/linux/setrlimit64.c;hb=9e77a668c94f892f2ce0ad9ef7f6e88525852af3
        // - musl: `prlimit64`, https://git.musl-libc.org/cgit/musl/tree/src/misc/getrlimit.c?id=7813e9119eb5fe681b6750b8a033a0490bca0941
        //   and https://git.musl-libc.org/cgit/musl/tree/src/misc/setrlimit.c?id=7813e9119eb5fe681b6750b8a033a0490bca0941;
        //   its locking `__synccall` fallback runs only on `ENOSYS`, before
        //   Linux 2.6.36, older than any kernel Rust supports.
        // - macOS: the `__getrlimit` and `__setrlimit` system call stubs, after
        //   setting `_RLIMIT_POSIX_FLAG`, https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.1.9/libsyscall/wrappers/unix03/getrlimit.c
        //   and https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.1.9/libsyscall/wrappers/unix03/setrlimit.c
        // The function allocates nothing, and its errors come from the
        // errno.
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

/// Extension trait for setting a command's process limit before it runs.
pub trait CommandProcessLimitExt {
    /// Arranges for the command to set its soft and hard `RLIMIT_NPROC` to
    /// `limit`, or to its hard limit if that is lower, after it is forked
    /// and before it runs its program. Only Linux sets it.
    ///
    /// # Errors
    ///
    /// [`error::ErrorKind::NotSupportedOnThisPlatform`] on a Unix other than
    /// Linux, where `nix` names no `RLIMIT_NPROC` for every target; on
    /// Linux, none here: a child that cannot set the limit fails its spawn.
    fn set_process_limit(&mut self, limit: u64) -> Result<(), error::Error>;
}

impl CommandProcessLimitExt for std::process::Command {
    #[cfg(target_os = "linux")]
    fn set_process_limit(&mut self, limit: u64) -> Result<(), error::Error> {
        // SAFETY: `pre_exec` requires a closure that is safe to run in a
        // forked child of a process that may have other threads: no
        // allocation, no lock, no `std::env`. `pre_exec_set_process_limit`
        // calls only `getrlimit` and `setrlimit`, which POSIX does not list
        // as async-signal-safe (signal-safety(7)). Each Linux libc makes
        // each one system call with no lock or allocation:
        // - glibc: `prlimit64`, https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/unix/sysv/linux/getrlimit64.c;hb=9e77a668c94f892f2ce0ad9ef7f6e88525852af3
        //   and https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/unix/sysv/linux/setrlimit64.c;hb=9e77a668c94f892f2ce0ad9ef7f6e88525852af3
        // - musl: `prlimit64`, https://git.musl-libc.org/cgit/musl/tree/src/misc/getrlimit.c?id=7813e9119eb5fe681b6750b8a033a0490bca0941
        //   and https://git.musl-libc.org/cgit/musl/tree/src/misc/setrlimit.c?id=7813e9119eb5fe681b6750b8a033a0490bca0941;
        //   its locking `__synccall` fallback runs only on `ENOSYS`, before
        //   Linux 2.6.36, older than any kernel Rust supports.
        // The function allocates nothing, and its errors come from the
        // errno.
        unsafe {
            self.pre_exec(move || pre_exec_set_process_limit(limit));
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn set_process_limit(&mut self, _limit: u64) -> Result<(), error::Error> {
        Err(error::ErrorKind::NotSupportedOnThisPlatform("setting a process limit").into())
    }
}

/// Sets this process's soft and hard `RLIMIT_NPROC` to `limit`, or to its
/// hard limit if that is lower, in a child between the fork and `exec`.
#[cfg(target_os = "linux")]
fn pre_exec_set_process_limit(limit: u64) -> Result<(), std::io::Error> {
    use nix::sys::resource::{Resource, getrlimit, setrlimit};

    let (_, hard) = getrlimit(Resource::RLIMIT_NPROC)?;
    // `rlim_t` is `u64` on 64-bit Linux and `u32` on some 32-bit targets; a
    // limit past its range keeps the hard limit, as any limit above it does.
    #[allow(
        clippy::unnecessary_fallible_conversions,
        reason = "rlim_t is not u64 on every Linux target"
    )]
    let limit = libc::rlim_t::try_from(limit).unwrap_or(hard).min(hard);
    Ok(setrlimit(Resource::RLIMIT_NPROC, limit, limit)?)
}

/// Extension trait for checking process counts before a command runs.
pub trait CommandPidsLimitsExt {
    /// Arranges for the command, after it is forked and before it runs its
    /// program, to read each limit's count and to fail its spawn when a count
    /// is above its limit (`EAGAIN`, the error `fork` returns at `pids.max`)
    /// or cannot be read (`EIO`).
    ///
    /// In the process that registered it, as on a path that runs the
    /// command's steps with no fork, the check fails with `ENOTSUP` without
    /// reading a count: there a count would include every thread of that
    /// process, and the `exec` builtin refuses such a command before it
    /// composes it.
    ///
    /// # Errors
    ///
    /// None here: a child whose check fails fails its spawn.
    fn check_pids_limits(&mut self, limits: Arc<[PidsLimit]>) -> Result<(), error::Error>;
}

impl CommandPidsLimitsExt for std::process::Command {
    fn check_pids_limits(&mut self, limits: Arc<[PidsLimit]>) -> Result<(), error::Error> {
        // The process that registers the check: the function runs in it, not
        // in a child, only on the exec builtin's path.
        let shell_pid = nix::unistd::getpid();
        // SAFETY: `pre_exec` requires a closure that is safe to run in a
        // forked child of a process that may have other threads: no
        // allocation, no lock, no `std::env`. The closure reads the captured
        // limits without cloning their `Arc`, and `pre_exec_check_pids_limits`
        // calls only `getpid`, which POSIX lists as async-signal-safe
        // (signal-safety(7)), and `pread`, which it does not list. Each
        // target libc makes `pread` one system call with no lock or
        // allocation:
        // - glibc: `pread64`, https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/unix/sysv/linux/pread64.c;hb=9e77a668c94f892f2ce0ad9ef7f6e88525852af3
        // - musl: https://git.musl-libc.org/cgit/musl/tree/src/unistd/pread.c?id=7813e9119eb5fe681b6750b8a033a0490bca0941
        // - macOS: a generated stub that makes system call 153, https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.1.9/bsd/kern/syscalls.master
        // The function allocates nothing, and its errors come from an
        // errno. A failure is returned rather than ending the process,
        // because a path with no fork runs it in the shell's own process.
        unsafe {
            self.pre_exec(move || pre_exec_check_pids_limits(&limits, shell_pid));
        }
        Ok(())
    }
}

fn pre_exec_check_pids_limits(
    limits: &[PidsLimit],
    shell_pid: nix::unistd::Pid,
) -> Result<(), std::io::Error> {
    if nix::unistd::getpid() == shell_pid {
        return Err(std::io::Error::from_raw_os_error(libc::ENOTSUP));
    }
    limits.iter().try_for_each(check_pids_limit)
}

/// Fails with `EAGAIN` when `limit`'s count is above its maximum, and with
/// `EIO` when the count cannot be read. Both errors are built from an errno,
/// which allocates nothing.
fn check_pids_limit(limit: &PidsLimit) -> Result<(), std::io::Error> {
    use std::os::unix::fs::FileExt as _;

    let mut buffer = [0u8; PIDS_CURRENT_BUFFER];
    let count = match limit.current.read_at(&mut buffer, 0) {
        Ok(len) if len < buffer.len() => buffer.get(..len).and_then(parse_count),
        _ => None,
    };
    match count {
        Some(count) if count <= limit.max => Ok(()),
        Some(_) => Err(std::io::Error::from_raw_os_error(libc::EAGAIN)),
        None => Err(std::io::Error::from_raw_os_error(libc::EIO)),
    }
}

/// Parses a cgroup count file: ASCII digits and an optional newline, with no
/// allocation, so a forked child can call it.
fn parse_count(bytes: &[u8]) -> Option<u64> {
    let digits = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if digits.is_empty() {
        return None;
    }
    digits.iter().try_fold(0u64, |count, byte| {
        let digit = byte.checked_sub(b'0').filter(|digit| *digit <= 9)?;
        count.checked_mul(10)?.checked_add(u64::from(digit))
    })
}

/// Whether nothing exists at `path`, such as a `cgroup.procs` file whose
/// cgroup was removed.
pub(crate) fn is_missing(path: &CStr) -> bool {
    use std::os::unix::ffi::OsStrExt as _;

    !std::path::Path::new(std::ffi::OsStr::from_bytes(path.to_bytes())).exists()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::parse_count;

    proptest! {
        #[test]
        fn a_count_parses_with_or_without_its_newline(count: u64) {
            prop_assert_eq!(parse_count(count.to_string().as_bytes()), Some(count));
            prop_assert_eq!(parse_count(format!("{count}\n").as_bytes()), Some(count));
        }

        #[test]
        fn anything_parsed_is_the_decimal_it_reads(bytes: Vec<u8>) {
            if let Some(count) = parse_count(&bytes) {
                let digits = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
                prop_assert!(digits.iter().all(u8::is_ascii_digit));
                let text = std::str::from_utf8(digits).map_err(|e| TestCaseError::fail(e.to_string()))?;
                prop_assert_eq!(text.parse::<u64>().ok(), Some(count));
            }
        }

        #[test]
        fn a_count_with_anything_else_does_not_parse(
            count: u64,
            junk in "[^0-9]{1,4}",
        ) {
            prop_assume!(junk != "\n");
            let text = format!("{count}{junk}");
            prop_assert_eq!(parse_count(text.as_bytes()), None);
        }
    }

    #[test]
    fn empty_and_overflowing_counts_do_not_parse() {
        assert_eq!(parse_count(b""), None);
        assert_eq!(parse_count(b"\n"), None);
        assert_eq!(parse_count(b"18446744073709551616"), None);
        assert_eq!(parse_count(b"max\n"), None);
    }
}
