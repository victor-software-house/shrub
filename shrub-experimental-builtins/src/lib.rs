//! Builtins for [`shrub_core::Shell`] that are not yet ready to be standard.
//!
//! This crate sits beside `shrub-builtins`: that crate holds the builtins
//! shrub commits to, and this one holds commands whose behavior or output may
//! change or be removed. The `shrub` binary registers them only when it is
//! built with its `experimental-builtins` feature.
//!
//! Today the crate has one builtin, `save`, which writes the shell's state to
//! standard output as JSON.
//!
//! ## Quick start
//!
//! Add the experimental builtins to a shell builder with
//! [`ShellBuilderExt::experimental_builtins`], then run them like any other
//! command:
//!
//! ```no_run
//! use shrub_core::{ProfileLoadBehavior, RcLoadBehavior, Shell, SourceInfo};
//! use shrub_experimental_builtins::ShellBuilderExt as _;
//!
//! # use shrub_core::Error;
//! # async fn run() -> Result<(), Error> {
//! let mut shell = Shell::builder()
//!     .experimental_builtins()
//!     .profile(ProfileLoadBehavior::Skip)
//!     .rc(RcLoadBehavior::Skip)
//!     .build()
//!     .await?;
//!
//! let params = shell.default_exec_params();
//! shell
//!     .run_string("save", &SourceInfo::from("quick start"), &params)
//!     .await?;
//! # Ok(())
//! # }
//! # fn main() {}
//! ```
//!
//! ## Registering without the builder
//!
//! [`experimental_builtins`] returns the registrations as a map from command
//! name to [`shrub_core::builtins::Registration`]. Use it to register the
//! builtins on a shell that already exists, for example one restored from a
//! saved state, since the registrations themselves are not serialized:
//!
//! ```
//! use shrub_core::Shell;
//!
//! let mut shell: Shell = Shell::default();
//! for (name, registration) in shrub_experimental_builtins::experimental_builtins() {
//!     shell.register_builtin(name, registration);
//! }
//! ```
//!
//! ## What `save` writes
//!
//! `save` serializes the shell's state, which may include the values of
//! variables and the command history. Either can contain secrets, so treat
//! its output as sensitive.
//!
//! ## Cargo features
//!
//! | Feature | Effect |
//! |---|---|
//! | `default` | Enables `builtin.save`. |
//! | `builtin.save` | Compiles in `save`, with the serialization support it needs in `shrub-core` and `serde_json`. |

#![cfg_attr(docsrs, feature(doc_cfg))]
// See the equivalent note in `shrub-builtins`: `shrub_core::builtins::Command::execute` is async
// by contract (the trait declares a desugared `-> impl Future<...> + Send` so that the dispatch
// path can box every builtin uniformly), so a builtin that does purely synchronous work still
// implements `execute` as an `async fn` with no `.await`.
#![allow(
    clippy::unused_async_trait_impl,
    reason = "builtins implement a trait whose `execute` is async by contract"
)]

#[cfg(feature = "builtin.save")]
mod save;

#[allow(unused_imports, reason = "not all builtins are used in all configs")]
use shrub_core::builtins::{self, builtin, decl_builtin, raw_arg_builtin, simple_builtin};

/// Returns the set of experimental built-in commands.
pub fn experimental_builtins<SE: shrub_core::extensions::ShellExtensions>()
-> std::collections::HashMap<String, builtins::Registration<SE>> {
    let mut m = std::collections::HashMap::<String, builtins::Registration<SE>>::new();

    #[cfg(feature = "builtin.save")]
    m.insert("save".into(), builtin::<save::SaveCommand, SE>());

    m
}

/// Extension trait that simplifies adding experimental builtins to a shell
/// builder.
pub trait ShellBuilderExt {
    /// Add experimental builtins to the shell being built.
    #[must_use]
    fn experimental_builtins(self) -> Self;
}

impl<SE: shrub_core::extensions::ShellExtensions, S: shrub_core::ShellBuilderState> ShellBuilderExt
    for shrub_core::ShellBuilder<SE, S>
{
    fn experimental_builtins(self) -> Self {
        self.builtins(crate::experimental_builtins())
    }
}
