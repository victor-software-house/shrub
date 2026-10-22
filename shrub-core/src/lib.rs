//! The shell itself: the [`Shell`] type and everything that runs a script, for
//! programs that embed shrub or build a front end on it.
//!
//! `shrub-core` sits between [`shrub_parser`], which turns text into a syntax
//! tree, and the crates that add builtins or a line editor on top. It owns
//! variables, expansion, redirection, jobs, traps, and programmable
//! completion. It has no line editor or prompt drawing of its own.
//!
//! ## Quick start
//!
//! Build a [`Shell`] with [`Shell::builder`], then run text with
//! [`Shell::run_string`]. The shell keeps its variables, functions, and working
//! directory between calls.
//!
//! ```
//! use shrub_core::{Error, Shell, SourceInfo};
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), Error> {
//! let mut shell = Shell::builder().build().await?;
//!
//! let result = shell
//!     .run_string(
//!         "name=world; greet() { echo \"Hello, $1!\"; }; greet \"$name\"",
//!         &SourceInfo::default(),
//!         &shell.default_exec_params(),
//!     )
//!     .await?;
//!
//! assert!(result.is_success());
//! assert_eq!(shell.env_str("name").as_deref(), Some("world"));
//! # Ok::<(), Error>(())
//! # }
//! ```
//!
//! A failed parse or an error the shell cannot continue past comes back as
//! [`Error`]. A command that exits non-zero is not an error: it is an
//! [`ExecutionResult`] whose exit code you inspect.
//!
//! ## Builtins
//!
//! A new shell has no builtins registered. Add them while building, with
//! [`ShellBuilder::builtin`], or later with [`Shell::register_builtin`]. Use
//! [`builtins::builtin`] to register a type that implements
//! [`builtins::Command`], or [`builtins::simple_builtin`] for a
//! [`builtins::SimpleCommand`]. The `shrub-builtins` crate registers the
//! standard set this way.
//!
//! ## Customizing a shell
//!
//! [`Shell`] is generic over [`ShellExtensions`], which swaps in parts of the
//! shell's behavior at compile time; today that is how errors are formatted.
//! Create such a shell with [`Shell::builder_with_extensions`].
//!
//! ## Parsing
//!
//! The syntax tree types are re-exported in [`parser`]. [`Shell::parse_string`]
//! parses with the options the shell currently has set.
//!
//! ## Cargo features
//!
//! | Feature | Effect |
//! |---|---|
//! | `serde` | Implements `Serialize` and `Deserialize` for the syntax tree and other data types, through the same feature of `shrub-parser`. |
//! | `experimental-parser` | Makes the `winnow`-based parser available through [`parser::ParserImpl`], by enabling `shrub-parser`'s `winnow-parser` feature. |

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod arithmetic;
mod braceexpansion;
pub mod builtins;
pub mod callstack;
pub mod commands;
pub mod completion;
pub mod env;
pub mod error;
pub mod escape;
pub mod expansion;
mod extendedtests;
pub mod extensions;
pub mod functions;
pub mod history;
pub mod int_utils;
pub mod interfaces;
mod interp;
mod ioutils;
pub mod jobs;
mod keywords;
pub mod namedoptions;
pub mod openfiles;
pub mod options;
pub mod pathcache;
pub mod pathsearch;
pub mod patterns;
pub mod processes;
mod prompt;
mod regex;
pub mod results;
mod shell;
pub mod sourceinfo;
pub mod sys;
pub mod tasks;
pub mod terminal;
pub mod tests;
pub mod timing;
pub mod trace_categories;
pub mod traps;
pub mod variables;
mod wellknownvars;

/// Re-export parser types used in core definitions.
pub mod parser {
    pub use shrub_parser::{
        BindingParseError, ParseError, ParserImpl, SourcePosition, SourcePositionOffset,
        SourceSpan, TestCommandParseError, WordParseError, ast,
    };
}

pub use commands::{CommandArg, ExecutionContext};
pub use error::{BuiltinError, Error, ErrorKind};
pub use extensions::ShellExtensions;
pub use interp::{ExecutionParameters, ProcessGroupPolicy, SpawnOptions, SpawnedProcess};
pub use parser::{SourcePosition, SourcePositionOffset, SourceSpan};
pub use results::{ExecutionControlFlow, ExecutionExitCode, ExecutionResult, ExecutionSpawnResult};
pub use shell::{
    CreateOptions, ProfileLoadBehavior, RcLoadBehavior, SavedCommandStatus, Shell, ShellBuilder,
    ShellBuilderState, ShellFd, ShellState,
};
pub use sourceinfo::SourceInfo;
pub use variables::{ShellValue, ShellVariable};
