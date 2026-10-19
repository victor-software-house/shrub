//! The standard POSIX and bash builtins of shrub, registered on a
//! [`shrub_core::Shell`].
//!
//! A builtin is a command the shell runs in its own process: `cd`, `export`,
//! `read`, `trap`, and the rest of the set bash and POSIX `sh` define.
//! `shrub-core` provides the interpreter and the registration API; this crate
//! supplies the implementations and the ready-made sets to register.
//!
//! ## Quick start
//!
//! Add the standard builtins to a shell builder with
//! [`ShellBuilderExt::default_builtins`], then build the shell and run a
//! command:
//!
//! ```
//! use shrub_builtins::{BuiltinSet, ShellBuilderExt as _};
//! use shrub_core::{ProfileLoadBehavior, RcLoadBehavior, Shell, SourceInfo};
//!
//! # use shrub_core::Error;
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), Error> {
//! let mut shell = Shell::builder()
//!     .default_builtins(BuiltinSet::BashMode)
//!     .profile(ProfileLoadBehavior::Skip)
//!     .rc(RcLoadBehavior::Skip)
//!     .build()
//!     .await?;
//!
//! let params = shell.default_exec_params();
//! let result = shell
//!     .run_string("true", &SourceInfo::from("quick start"), &params)
//!     .await?;
//! assert!(result.is_success());
//! # Ok(())
//! # }
//! ```
//!
//! ## Builtin sets
//!
//! [`BuiltinSet`] names the two sets [`default_builtins`] can return:
//!
//! - [`BuiltinSet::ShMode`] holds the smaller set suited to POSIX `sh`
//!   compatibility.
//! - [`BuiltinSet::BashMode`] adds what bash has beyond POSIX, such as
//!   `declare`, `mapfile`, `shopt`, `pushd`, the completion builtins, and
//!   `bind`. It also registers `disown` and `logout` as placeholders that
//!   report the exit code for unimplemented functionality.
//!
//! ## Customizing the set
//!
//! [`default_builtins`] returns the registrations as a map from command name
//! to [`shrub_core::builtins::Registration`], so a caller can add, replace, or
//! remove entries before handing the map to
//! [`ShellBuilder::builtins`](shrub_core::ShellBuilder::builtins):
//!
//! ```
//! use shrub_builtins::{BuiltinSet, default_builtins};
//! use shrub_core::extensions::DefaultShellExtensions;
//!
//! let mut builtins = default_builtins::<DefaultShellExtensions>(BuiltinSet::BashMode);
//! builtins.remove("ulimit");
//! assert!(!builtins.contains_key("ulimit"));
//! ```
//!
//! ## Platform support
//!
//! A builtin that depends on Unix process control is compiled only on Unix:
//! `exec`, `kill`, `suspend`, `ulimit`, and `umask`. `printf` needs Unix or
//! Windows. On any other target, such as wasm, the other builtins are still
//! available.
//!
//! ## Cargo features
//!
//! Each builtin has a `builtin.<name>` feature that compiles it in. All of
//! them are on by default; disable default features to pick a subset.
//!
//! | Feature | Effect |
//! |---|---|
//! | `default` | Enables every `builtin.<name>` feature. |
//! | `builtin.<name>` | Compiles in the builtin `<name>`, such as `builtin.cd` or `builtin.trap`. |
//! | `builtin.exec` | Compiles in `exec` and also enables `builtin.command`. |
//!
//! `builtin.declare` covers `declare`, `local`, `readonly`, and `typeset`.
//! `builtin.complete`, `builtin.compgen`, and `builtin.compopt` share one
//! implementation module. `builtin.dot` covers `.` and `source`, and
//! `builtin.test` covers `test` and `[`.

#![cfg_attr(docsrs, feature(doc_cfg))]
// `shrub_core::builtins::Command::execute` is async by contract: the trait declares a desugared
// `-> impl Future<...> + Send` so that `shrub_core::builtins::exec_builtin` can box and dispatch
// every builtin uniformly. Most builtins in this crate do purely synchronous work, so their
// `execute` bodies contain no `.await` -- that is the trait contract being honored, not a defect.
#![allow(
    clippy::unused_async_trait_impl,
    reason = "builtins implement a trait whose `execute` is async by contract"
)]

#[cfg(feature = "builtin.alias")]
mod alias;
#[cfg(feature = "builtin.bg")]
mod bg;
#[cfg(feature = "builtin.bind")]
mod bind;
#[cfg(feature = "builtin.break")]
mod break_;
#[cfg(feature = "builtin.builtin")]
mod builtin_;
#[cfg(feature = "builtin.caller")]
mod caller;
#[cfg(feature = "builtin.cd")]
mod cd;
#[cfg(feature = "builtin.colon")]
mod colon;
#[cfg(feature = "builtin.command")]
mod command;
#[cfg(any(
    feature = "builtin.complete",
    feature = "builtin.compgen",
    feature = "builtin.compopt"
))]
mod complete;
#[cfg(feature = "builtin.continue")]
mod continue_;
#[cfg(feature = "builtin.declare")]
mod declare;
#[cfg(feature = "builtin.dirs")]
mod dirs;
#[cfg(feature = "builtin.dot")]
mod dot;
#[cfg(feature = "builtin.echo")]
mod echo;
#[cfg(feature = "builtin.enable")]
mod enable;
#[cfg(feature = "builtin.eval")]
mod eval;
#[cfg(all(feature = "builtin.exec", unix))]
mod exec;
#[cfg(feature = "builtin.exit")]
mod exit;
#[cfg(feature = "builtin.export")]
mod export;
#[cfg(feature = "builtin.false")]
mod false_;
#[cfg(feature = "builtin.fc")]
mod fc;
#[cfg(feature = "builtin.fg")]
mod fg;
#[cfg(feature = "builtin.getopts")]
mod getopts;
#[cfg(feature = "builtin.hash")]
mod hash;
#[cfg(feature = "builtin.help")]
mod help;
#[cfg(feature = "builtin.history")]
mod history;
#[cfg(feature = "builtin.jobs")]
mod jobs;
#[cfg(all(feature = "builtin.kill", unix))]
mod kill;
#[cfg(feature = "builtin.let")]
mod let_;
#[cfg(feature = "builtin.mapfile")]
mod mapfile;
#[cfg(feature = "builtin.popd")]
mod popd;
#[cfg(all(feature = "builtin.printf", any(unix, windows)))]
mod printf;
#[cfg(feature = "builtin.pushd")]
mod pushd;
#[cfg(feature = "builtin.pwd")]
mod pwd;
#[cfg(feature = "builtin.read")]
mod read;
#[cfg(feature = "builtin.return")]
mod return_;
#[cfg(feature = "builtin.set")]
mod set;
#[cfg(feature = "builtin.shift")]
mod shift;
#[cfg(feature = "builtin.shopt")]
mod shopt;
#[cfg(all(feature = "builtin.suspend", unix))]
mod suspend;
#[cfg(feature = "builtin.test")]
mod test;
#[cfg(feature = "builtin.times")]
mod times;
#[cfg(feature = "builtin.trap")]
mod trap;
#[cfg(feature = "builtin.true")]
mod true_;
#[cfg(feature = "builtin.type")]
mod type_;
#[cfg(all(feature = "builtin.ulimit", unix))]
mod ulimit;
#[cfg(all(feature = "builtin.umask", unix))]
mod umask;
#[cfg(feature = "builtin.unalias")]
mod unalias;
#[cfg(feature = "builtin.unset")]
mod unset;
#[cfg(feature = "builtin.wait")]
mod wait;

mod builder;
mod factory;
#[cfg(any(feature = "builtin.command", feature = "builtin.type"))]
mod lookup;
mod unimp;

pub use builder::ShellBuilderExt;
pub use factory::{BuiltinSet, default_builtins};

/// Writes an alias definition in the reusable form printed by `alias` and
/// `command -v`.
#[cfg(any(feature = "builtin.alias", feature = "builtin.command"))]
fn write_alias_definition(
    mut writer: impl std::io::Write,
    name: &str,
    value: &str,
) -> std::io::Result<()> {
    writeln!(
        writer,
        "alias {name}={}",
        shrub_core::escape::single_quote(value)
    )
}

/// Macro to define a struct that represents a shell built-in flag argument that
/// can be enabled or disabled by specifying an option with a leading '+' or '-'
/// character.
///
/// # Arguments
///
/// - `$struct_name` - The identifier to be used for the struct to define.
/// - `$flag_char` - The character to use as the flag.
/// - `$desc` - The string description of the flag.
#[macro_export]
macro_rules! minus_or_plus_flag_arg {
    ($struct_name:ident, $flag_char:literal, $desc:literal) => {
        #[derive(clap::Parser)]
        pub(crate) struct $struct_name {
            #[arg(short = $flag_char, name = concat!(stringify!($struct_name), "_enable"), action = clap::ArgAction::SetTrue, help = $desc)]
            _enable: bool,
            #[arg(long = concat!("+", $flag_char), name = concat!(stringify!($struct_name), "_disable"), action = clap::ArgAction::SetTrue, hide = true)]
            _disable: bool,
        }

        impl From<$struct_name> for Option<bool> {
            fn from(value: $struct_name) -> Self {
                value.to_bool()
            }
        }

        impl $struct_name {
            #[allow(dead_code, reason = "may not be used in all macro instantiations")]
            pub const fn is_some(&self) -> bool {
                self._enable || self._disable
            }

            pub const fn to_bool(&self) -> Option<bool> {
                match (self._enable, self._disable) {
                    (true, false) => Some(true),
                    (false, true) => Some(false),
                    _ => None,
                }
            }
        }
    };
}
