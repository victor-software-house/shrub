//! Create for shrub, an executable bash-compatible shell.

#![allow(dead_code)]

pub mod args;
mod shrubctl;
pub mod bundled;
pub mod config;
pub mod entry;
mod error_formatter;
pub mod events;
mod productinfo;
