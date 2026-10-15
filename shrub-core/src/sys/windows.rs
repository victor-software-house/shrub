pub use crate::sys::stubs::{async_pipe, commands};
pub(crate) mod env;
pub use crate::sys::stubs::fd;
pub(crate) mod fs;
pub(crate) use crate::sys::os_pipes as pipes;
pub use crate::sys::stubs::input;
pub(crate) mod network;
pub use crate::sys::stubs::{poll, resource};

/// Signal processing utilities
pub mod signal {
    pub use tokio::signal::ctrl_c as await_ctrl_c;

    pub(crate) use crate::sys::stubs::signal::*;
}

pub use crate::sys::stubs::terminal;
pub use crate::sys::tokio_process as process;
pub(crate) use crate::sys::tokio_tasks as tasks;
pub(crate) mod users;

/// Platform-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {}
