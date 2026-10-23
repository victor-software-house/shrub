//! File descriptor utilities.
//!
//! # Why this module is unsafe
//!
//! 1. Safe code cannot do it. The shell takes over descriptors it finds by
//!    number, in `/proc/self/fd` or `/dev/fd` or from its caller, which may
//!    have closed since. The safe way to duplicate one,
//!    `BorrowedFd::try_clone_to_owned`, needs a `BorrowedFd`, and
//!    `BorrowedFd::borrow_raw` requires the descriptor to stay open, which
//!    these callers cannot promise. nix's `fcntl` also takes a borrowed
//!    descriptor.
//! 2. Speed is not the reason.
//! 3. The unsafe operations are the `fcntl` call that duplicates the number and
//!    `OwnedFd::from_raw_fd` on the duplicate, in `open_file_by_fd`.

use std::os::fd::RawFd;

use crate::{ShellFd, error, openfiles};

/// The lowest number a duplicate may take, as for
/// `BorrowedFd::try_clone_to_owned`: below it sit standard input, output and
/// error.
const FIRST_DUPLICATE_FD: RawFd = 3;

cfg_if::cfg_if! {
    if #[cfg(any(target_os = "linux", target_os = "android"))] {
        const FD_DIR_PATH: &str = "/proc/self/fd";
    } else if #[cfg(any(
            target_os = "freebsd",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "openbsd"
        ))] {
        const FD_DIR_PATH: &str = "/dev/fd";
    } else {
        /// Returns an iterator over all open file descriptors for the shell.
        ///
        /// # Errors
        ///
        /// This fallback implementation always succeeds with an empty iterator.
        pub fn iter_fds()
        -> Result<impl Iterator<Item = (ShellFd, openfiles::OpenFile)>, error::Error> {
            Ok(std::iter::empty())
        }
    }
}

/// Makes a best-effort attempt to iterate over all open file descriptors
/// for the current process.
///
/// If the platform does not support enumerating file descriptors, an empty
/// iterator is returned. This function will skip any file descriptors that
/// cannot be opened.
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub fn try_iter_open_fds() -> impl Iterator<Item = (ShellFd, openfiles::OpenFile)> {
    std::fs::read_dir(FD_DIR_PATH)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let fd: RawFd = entry.file_name().to_str()?.parse().ok()?;

            // The descriptor may have closed, or been reused, since it was
            // listed; one that cannot be duplicated is skipped, as this is a
            // best-effort listing.
            let file = open_file_by_fd(fd).ok()?;

            Some((fd, file))
        })
}

/// Attempts to retrieve an `OpenFile` representation for the given already-open
/// file descriptor.
///
/// If the file descriptor cannot be opened, `None` is returned. Note that there
/// is no guarantee that the returned file matches the original file descriptor,
/// as the fd may have been closed and potentially re-used in the meantime.
///
/// # Arguments
///
/// * `fd` - The file descriptor to open.
pub fn try_get_file_for_open_fd(fd: RawFd) -> Option<openfiles::OpenFile> {
    open_file_by_fd(fd).ok()
}

/// Duplicates the descriptor numbered `fd`, whatever it now refers to, into a
/// new file that this process owns.
///
/// It takes a number, not a borrowed descriptor, because its callers cannot
/// promise that `fd` stays open: they found it in a listing, or were given
/// it. A closed `fd` fails with `EBADF`.
fn open_file_by_fd(fd: RawFd) -> Result<openfiles::OpenFile, error::Error> {
    use std::os::fd::FromRawFd as _;

    // SAFETY: `fcntl` with `F_DUPFD_CLOEXEC` takes the descriptor number and
    // the lowest new number by value, so it reads no memory of this process,
    // and it neither closes nor changes `fd`, which another owner may hold.
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, FIRST_DUPLICATE_FD) };
    if duplicate == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: `from_raw_fd` requires an open descriptor that nothing else
    // owns. `duplicate` was just returned by `fcntl`, so it is open, and no
    // other code has seen its number.
    let owned_fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(duplicate) };
    Ok(std::fs::File::from(owned_fd).into())
}
