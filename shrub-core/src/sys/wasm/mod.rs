pub mod async_pipe;
pub use crate::sys::stubs::commands;
pub(crate) mod env;
pub use crate::sys::stubs::fd;
pub(crate) mod fs;
pub use crate::sys::stubs::input;
pub(crate) use crate::sys::stubs::network;
pub(crate) mod pipes;
pub use crate::sys::stubs::{poll, process, resource, signal};
pub(crate) mod tasks;
pub use crate::sys::stubs::terminal;
pub(crate) use crate::sys::stubs::users;

/// Platform-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {}
