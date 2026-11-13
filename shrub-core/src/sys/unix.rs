pub mod async_pipe;
pub mod commands;
pub(crate) mod env;
pub mod fd;
pub mod fs;
pub mod input;
use nix::errno::Errno;

pub(crate) use crate::sys::os_pipes as pipes;
pub(crate) mod network;
pub mod poll;
use crate::error;
pub use crate::sys::tokio_process as process;
pub mod resource;
pub mod signal;
pub(crate) use crate::sys::tokio_tasks as tasks;
pub mod terminal;
pub(crate) mod users;

/// Platform-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    /// A system error occurred.
    #[error("system error: {0}")]
    ErrnoError(#[from] Errno),
}

impl From<Errno> for error::ErrorKind {
    fn from(err: Errno) -> Self {
        PlatformError::ErrnoError(err).into()
    }
}
