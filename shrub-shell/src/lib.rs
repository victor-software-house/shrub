//! The command line and entry point of the `shrub` binary, as a library so
//! other programs can embed shrub's CLI.
//!
//! `shrub-shell` is the top of the shrub crate graph. It parses the bash-style
//! command line, loads the configuration file, wires `shrub-core`,
//! `shrub-builtins`, and `shrub-interactive` into one shell, and runs it as a
//! script runner, a `-c` command, or an interactive session. The `shrub`
//! binary calls [`entry::run`] and nothing else.
//!
//! ## Quick start
//!
//! The pieces that sit before the shell starts are public: the parsed
//! arguments, the configuration file, and the interface options derived from
//! both. This reads the default arguments and builds the interface options
//! from them, skipping the configuration file:
//!
//! ```
//! use shrub_shell::args::CommandLineArgs;
//! use shrub_shell::config;
//!
//! let args = CommandLineArgs::default_values();
//! let config = config::load_config(true, None).into_config_or_log()?;
//! let ui = config.to_ui_options(&args);
//! assert!(!ui.disable_color);
//! # Ok::<(), String>(())
//! ```
//!
//! To run the whole shell, call [`entry::run`]. It reads the process's
//! arguments, runs until the shell exits, and ends the process with the
//! shell's exit code, so it never returns.
//!
//! ## Command line
//!
//! [`args::CommandLineArgs`] describes the options `shrub` accepts. They follow
//! bash: `-c` runs a command string, a script path runs a file, and with
//! neither the shell reads commands from standard input and is interactive on
//! a terminal. [`args::InputBackendType`] selects the line editor.
//!
//! ## Configuration file
//!
//! [`config::Config`] is the TOML file read from
//! [`config::default_config_path`] or from a path given on the command line.
//! Unknown fields are ignored, and a file at the default path that fails to
//! load is logged and skipped, while one named explicitly is an error.
//! Command-line options take priority over the file, and the file over built-in
//! defaults.
//!
//! ## Bundled commands
//!
//! The [`bundled`] module lets the binary carry utilities, such as the
//! uutils coreutils, under their usual names. Each runs as a subprocess of
//! `shrub`, so redirections and pipes apply, and a builtin of the same name
//! always wins.
//!
//! ## Tracing
//!
//! [`events::TraceEvent`] names the kinds of event, such as `commands` or
//! `expand`, that `--debug` turns on in the log.
//!
//! ## Cargo features
//!
//! | Feature | Effect |
//! |---|---|
//! | `default` | Enables `basic`, `reedline`, `minimal`, and `vt`. |
//! | `reedline`, `vt`, `basic`, `minimal` | Input backends. Each can be enabled on any target and is offered only where it works. |
//! | `experimental` | Enables every `experimental-*` feature and turns syntax highlighting on by default. |
//! | `experimental-builtins` | Adds the builtins of `shrub-experimental-builtins`. |
//! | `experimental-bundled-coreutils` | Bundles the uutils coreutils through `shrub-coreutils-builtins`. |
//! | `experimental-load` | Adds `--load`, which restores a shell saved with the `save` builtin. |
//! | `experimental-parser` | Adds `--experimental-parser`, selecting an unfinished parser. |
//! | `schema` | Derives `JsonSchema` for [`config::Config`]. |

#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(dead_code)]

pub mod args;
pub mod bundled;
pub mod config;
pub mod entry;
mod error_formatter;
pub mod events;
mod productinfo;
mod shrubctl;
