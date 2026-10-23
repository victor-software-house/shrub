//! A copy of a shell's state that a fresh shell, in another process or another
//! build of shrub, can take on.
//!
//! [`Shell::snapshot`](crate::Shell::snapshot) copies the state a command can
//! change: the working directory, variables, functions, aliases, traps,
//! options, disabled builtins, the directory stack, the positional parameters
//! and `$?`. [`Shell::restore_snapshot`](crate::Shell::restore_snapshot)
//! applies a copy to another shell through the same API a builtin uses, so
//! nothing in a snapshot is run as a command. A function is kept as its
//! source text, the one item with no other form that every build reads the
//! same way.
//!
//! The types are plain data with no serialization of their own, so the
//! embedder chooses how a snapshot is stored. A snapshot holds whatever values
//! the shell held, which can include secrets a command put there; an embedder
//! that stores one keeps that in mind, and [`SnapshotOptions::exclude`] keeps
//! named variables out of it.
//!
//! What a snapshot cannot carry is listed in [`ShellSnapshot::not_kept`]:
//! background jobs, open file descriptors above 2, and functions whose source
//! does not parse back to the same definition, such as one holding a
//! here-document, whose body shrub prints right after its operator. A fresh
//! shell rebuilds the command hash table, completion specifications and
//! history itself, and `OLDPWD`, `RANDOM`'s seed and `SECONDS`' offset are not
//! carried.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::variables::{ShellValueUnsetType, ShellVariableUpdateTransform};

/// What [`Shell::snapshot`](crate::Shell::snapshot) leaves out.
#[derive(Clone, Debug, Default)]
pub struct SnapshotOptions {
    /// Exported values the embedder gives every new shell itself. A variable
    /// that still holds exactly its baseline value, exported and with no
    /// other attribute, is left out, and a baseline name the shell no longer
    /// has goes into [`ShellSnapshot::unset_variables`].
    pub baseline: BTreeMap<String, String>,
    /// Variable names never copied, whatever their value.
    pub exclude: BTreeSet<String>,
}

/// A shell's state between two commands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellSnapshot {
    /// The working directory.
    pub working_dir: PathBuf,
    /// The directory stack of `pushd` and `popd`, without the working
    /// directory.
    pub directory_stack: Vec<PathBuf>,
    /// `$?`.
    pub last_exit_status: u8,
    /// `$1` and on.
    pub positional_parameters: Vec<String>,
    /// Global variables, by name.
    pub variables: BTreeMap<String, SavedVariable>,
    /// Baseline names the shell no longer has.
    pub unset_variables: BTreeSet<String>,
    /// Functions, by name.
    pub functions: BTreeMap<String, SavedFunction>,
    /// Aliases: name to value.
    pub aliases: BTreeMap<String, String>,
    /// Trap handlers: signal name, as `trap -p` prints it, to command text.
    pub traps: BTreeMap<String, String>,
    /// `set -o` options, by name.
    pub set_options: BTreeMap<String, bool>,
    /// `shopt` options, by name.
    pub shopt_options: BTreeMap<String, bool>,
    /// Builtins turned off with `enable -n`.
    pub disabled_builtins: BTreeSet<String>,
    /// State the snapshot could not copy.
    pub not_kept: Vec<NotKept>,
}

/// A variable and its attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedVariable {
    /// The value.
    pub value: SavedValue,
    /// `declare -x`.
    pub exported: bool,
    /// `declare -r`.
    pub readonly: bool,
    /// `declare -i`.
    pub integer: bool,
    /// `declare -n`; the value is the name it refers to.
    pub nameref: bool,
    /// `declare -t`.
    pub trace: bool,
    /// `declare -l`, `-u` or `-c`; the value is already transformed.
    pub transform: ShellVariableUpdateTransform,
}

/// A variable's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedValue {
    /// Declared with no value, such as `declare -a name` or `export name`.
    Unset(ShellValueUnsetType),
    /// A string.
    String(String),
    /// An indexed array.
    IndexedArray(BTreeMap<u64, String>),
    /// An associative array.
    AssociativeArray(BTreeMap<String, String>),
}

/// A function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedFunction {
    /// The definition after its name, starting with `()`, as an exported
    /// function's `BASH_FUNC_` value holds it.
    pub body: String,
    /// `declare -fx`.
    pub exported: bool,
}

/// An item a snapshot could not copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotKept {
    /// What kind of item it is.
    pub kind: NotKeptKind,
    /// Which one: a job spec and command name such as `%1 sleep`, a
    /// descriptor number, or a function name. Never a value.
    pub name: String,
}

/// The kinds of item a snapshot cannot copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotKeptKind {
    /// A background or stopped job: a process cannot move to another shell.
    Job,
    /// A file descriptor above 2, opened with `exec`.
    OpenFile,
    /// A function whose source does not parse back to the same definition.
    Function,
}

/// What [`Shell::restore_snapshot`](crate::Shell::restore_snapshot) could
/// not apply. Every other item of the snapshot was applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestoreReport {
    /// The items that failed, in the order they were applied.
    pub failed: Vec<RestoreFailure>,
}

impl RestoreReport {
    /// Returns whether every item was applied.
    pub const fn is_complete(&self) -> bool {
        self.failed.is_empty()
    }

    /// Records that the item `name` of `kind` failed to apply.
    pub(crate) fn fail(&mut self, kind: StateKind, name: &str, reason: RestoreFailureReason) {
        self.failed.push(RestoreFailure {
            kind,
            name: name.to_owned(),
            reason,
        });
    }
}

/// One item that could not be applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreFailure {
    /// What kind of item it is.
    pub kind: StateKind,
    /// Which one: a path, variable, function, alias, signal, option or
    /// builtin name. Never a value.
    pub name: String,
    /// Why it failed.
    pub reason: RestoreFailureReason,
}

/// The kinds of state in a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateKind {
    /// The working directory.
    WorkingDir,
    /// A variable to set.
    Variable,
    /// A variable to unset.
    UnsetVariable,
    /// A function.
    Function,
    /// A trap handler.
    Trap,
    /// A `set -o` option.
    SetOption,
    /// A `shopt` option.
    ShoptOption,
    /// A builtin to turn off.
    DisabledBuiltin,
}

/// Why an item could not be applied. It names no value, so it is safe to
/// show wherever the item's name is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreFailureReason {
    /// The directory is gone or is not a directory.
    NotADirectory,
    /// The shell already holds a readonly variable of that name with another
    /// value, or a readonly variable to unset, or a readonly `PWD` or
    /// `OLDPWD` that changing the working directory must set.
    Readonly,
    /// The function's source does not parse in this shell.
    DoesNotParse,
    /// This shell knows no signal of that name.
    UnknownSignal,
    /// This shell knows no option of that name.
    UnknownOption,
    /// This shell has no builtin of that name.
    UnknownBuiltin,
}
