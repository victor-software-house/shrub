//! The interactive front end of shrub: reads command lines, edits them, and
//! drives a [`shrub_core::Shell`] with them.
//!
//! `shrub-core` interprets commands but has no user interface. This crate
//! sits above it and supplies the prompt loop, line editing, completion, and
//! highlighting, so an embedder or the `shrub` binary can offer an interactive
//! session. It reads shell state through `shrub-core`'s public API and does
//! not reimplement shell semantics.
//!
//! ## Quick start
//!
//! Wrap a shell in a [`ShellRef`], pick an [`InputBackend`], and run an
//! [`InteractiveShell`] until the user exits or input ends:
//!
//! ```no_run
//! # // The example needs the `minimal` feature, which is off by default.
//! # #[cfg(feature = "minimal")]
//! # async fn run() -> Result<(), shrub_interactive::ShellError> {
//! use std::sync::Arc;
//!
//! use shrub_core::Shell;
//! use shrub_interactive::{InteractiveOptions, InteractiveShell, MinimalInputBackend, ShellRef};
//! use tokio::sync::Mutex;
//!
//! let shell = Shell::builder().interactive(true).build().await?;
//! let shell: ShellRef = Arc::new(Mutex::new(shell));
//!
//! let mut input = MinimalInputBackend;
//! InteractiveShell::new(&shell, &mut input, &InteractiveOptions::default())?
//!     .run_interactively()
//!     .await?;
//! # Ok(())
//! # }
//! # fn main() {}
//! ```
//!
//! ## Input backends
//!
//! An [`InputBackend`] turns terminal input into [`ReadResult`] values. The
//! crate provides three:
//!
//! - `MinimalInputBackend` reads whole lines from standard input and draws only
//!   the prompt. It works everywhere, including when standard input is not a
//!   terminal.
//! - `VtInputBackend` edits the line with completion and history, using nothing
//!   from the terminal but bytes and escape sequences. A host that supplies its
//!   own `Terminal` can use it on any target.
//! - `ReedlineInputBackend` offers the richest editing, built on the [reedline](https://docs.rs/reedline)
//!   library. It exists on Unix and Windows.
//!
//! Implement [`InputBackend`] to add another.
//!
//! ## Options
//!
//! [`UIOptions`] holds the presentation choices, such as color and syntax
//! highlighting. [`InteractiveOptions`] holds what the prompt loop itself
//! needs and is built from a [`UIOptions`] with `From`.
//!
//! ## Hooks
//!
//! [`init_zsh_style_hooks`] prepares a shell for zsh-style `precmd` and
//! `preexec` hooks, which the prompt loop then runs. Call it before the shell
//! loads its profile and rc files, and set
//! [`InteractiveOptions::zsh_style_hooks`] so the loop dispatches them.
//!
//! ## Cargo features
//!
//! | Feature | Effect |
//! |---|---|
//! | `minimal` | Compiles `MinimalInputBackend`, which also reads input when standard input is not a terminal. |
//! | `vt` | Compiles the `vt` module and `VtInputBackend`; also enables `completion` and `minimal`. |
//! | `basic` | Enables `vt`. |
//! | `reedline` | Compiles `ReedlineInputBackend` on Unix and Windows; also enables `completion` and `highlighting`. Elsewhere it enables nothing. |
//! | `completion` | Compiles completion support. |
//! | `highlighting` | Compiles the `highlighting` module. |
//!
//! No feature is on by default.

#![cfg_attr(docsrs, feature(doc_cfg))]

mod error;
pub use error::ShellError;

mod interactive_shell;
pub use interactive_shell::{InteractiveExecutionResult, InteractiveOptions, InteractiveShell};

mod zsh_hooks;
pub use zsh_hooks::init as init_zsh_style_hooks;

mod input_backend;
pub use input_backend::{InputBackend, InteractivePrompt, ReadResult};

mod options;
pub use options::UIOptions;

mod refs;
pub use refs::ShellRef;

mod completeness;
mod term_detection;
mod term_integration;
mod trace_categories;

#[cfg(feature = "highlighting")]
pub mod highlighting;

#[cfg(feature = "completion")]
mod completion;

// Reedline-based shell
#[cfg(reedline_backend)]
mod reedline;
#[cfg(reedline_backend)]
pub use reedline::ReedlineInputBackend;

// Minimal shell
#[cfg(feature = "minimal")]
mod minimal;
#[cfg(feature = "minimal")]
pub use minimal::MinimalInputBackend;

// Line editing over plain bytes
#[cfg(feature = "vt")]
pub mod vt;
#[cfg(feature = "vt")]
pub use vt::VtInputBackend;
