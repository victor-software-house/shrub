//! Process management utilities

pub(crate) type ProcessId = i32;

pub(crate) use std::process::{ExitStatus, Output};

pub use shrub_process::Child;
