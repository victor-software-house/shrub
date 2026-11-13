//! Resource utilities

use std::time::Duration;

use nix::sys::resource;
use nix::sys::resource::UsageWho;
use nix::sys::time::TimeVal;

use crate::error;

/// Returns the user and system CPU time used by the current process;
/// expressed as a tuple containing user time and system time, in that order.
///
/// # Errors
///
/// Returns [`crate::error::ErrorKind::PlatformError`] wrapping the OS error if
/// `getrusage` fails.
pub fn get_self_user_and_system_time() -> Result<(Duration, Duration), error::Error> {
    let usage = resource::getrusage(UsageWho::RUSAGE_SELF)?;
    Ok((
        convert_rusage_time(usage.user_time()),
        convert_rusage_time(usage.system_time()),
    ))
}

/// Returns the user and system CPU time used by child processes; expressed
/// as a tuple containing user time and system time, in that order.
///
/// # Errors
///
/// Returns [`crate::error::ErrorKind::PlatformError`] wrapping the OS error if
/// `getrusage` fails.
pub fn get_children_user_and_system_time() -> Result<(Duration, Duration), error::Error> {
    let usage = resource::getrusage(UsageWho::RUSAGE_CHILDREN)?;
    Ok((
        convert_rusage_time(usage.user_time()),
        convert_rusage_time(usage.system_time()),
    ))
}

const fn convert_rusage_time(time: TimeVal) -> Duration {
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    Duration::new(time.tv_sec() as u64, time.tv_usec() as u32 * 1000)
}
