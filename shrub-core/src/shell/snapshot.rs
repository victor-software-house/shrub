//! Taking a snapshot of a shell's state, and applying one to another shell.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::env::EnvironmentLookup;
use crate::namedoptions::{self, ShellOptionKind};
use crate::openfiles::OpenFiles;
use crate::snapshot::{
    NotKept, NotKeptKind, RestoreFailureReason, RestoreReport, SavedFunction, SavedValue,
    SavedVariable, ShellSnapshot, SnapshotOptions, StateKind,
};
use crate::traps::TrapSignal;
use crate::variables::{ShellValue, ShellVariable, ShellVariableUpdateTransform};
use crate::{SourceInfo, error, extensions, wellknownvars};

/// `shopt` options that record how the shell was started, which bash refuses
/// to change; a snapshot neither copies nor restores them.
const STARTUP_SHOPT_OPTIONS: &[&str] = &["login_shell", "restricted_shell"];

/// Variables a snapshot never copies besides
/// [`wellknownvars::PROCESS_VARIABLES`]: the working directory is restored on
/// its own, `OLDPWD` is not carried, and `_` belongs to the last command.
const UNCOPIED_VARIABLES: &[&str] = &["PWD", "OLDPWD", "_"];

impl<SE: extensions::ShellExtensions> crate::Shell<SE> {
    /// Copies the state a command can change, for
    /// [`Shell::restore_snapshot`](Self::restore_snapshot) to apply to a fresh
    /// shell, in this process or another. See [`crate::snapshot`] for what a
    /// snapshot holds and what it cannot carry.
    ///
    /// # Arguments
    ///
    /// * `options` - The baseline values and names to leave out.
    ///
    /// # Errors
    ///
    /// Returns [`error::ErrorKind::NotBetweenCommands`] if a function or
    /// command scope is open.
    pub fn snapshot(&self, options: &SnapshotOptions) -> Result<ShellSnapshot, error::Error> {
        self.check_between_commands()?;

        let mut snapshot = ShellSnapshot {
            working_dir: self.working_dir.clone(),
            directory_stack: self.directory_stack.clone(),
            last_exit_status: self.last_exit_status,
            positional_parameters: self.current_shell_args().to_vec(),
            ..ShellSnapshot::default()
        };

        for (name, var) in self.env.iter_using_policy(EnvironmentLookup::OnlyInGlobal) {
            if is_uncopied(name, options) || !var.is_enumerable() {
                continue;
            }
            if let Some(saved) = saved_variable(var)
                && !holds_baseline(&saved, options.baseline.get(name))
            {
                snapshot.variables.insert(name.clone(), saved);
            }
        }

        for name in options.baseline.keys() {
            if !is_uncopied(name, options)
                && self
                    .env
                    .get_using_policy(name, EnvironmentLookup::OnlyInGlobal)
                    .is_none()
            {
                snapshot.unset_variables.insert(name.clone());
            }
        }

        for (name, registration) in self.funcs.iter() {
            let body = std::format!("() {}", registration.definition().body);
            if self.parses_back(&body) {
                snapshot.functions.insert(
                    name.clone(),
                    SavedFunction {
                        body,
                        exported: registration.is_exported(),
                    },
                );
            } else {
                snapshot.not_kept.push(NotKept {
                    kind: NotKeptKind::Function,
                    name: name.clone(),
                });
            }
        }

        snapshot.aliases = self
            .aliases
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();

        snapshot.traps = self
            .traps
            .iter_handlers()
            .map(|(signal, handler)| (signal.as_str().to_owned(), handler.command.clone()))
            .collect();

        snapshot.set_options = self.option_values(ShellOptionKind::SetO);
        snapshot.shopt_options = self.option_values(ShellOptionKind::Shopt);

        snapshot.disabled_builtins = self
            .builtins
            .iter()
            .filter(|(_, registration)| registration.disabled)
            .map(|(name, _)| name.clone())
            .collect();

        for job in &self.jobs.jobs {
            snapshot.not_kept.push(NotKept {
                kind: NotKeptKind::Job,
                name: std::format!("%{} {}", job.id, job.command_name()),
            });
        }

        let mut descriptors: Vec<_> = self
            .open_files
            .iter_fds()
            .map(|(fd, _)| fd)
            .filter(|fd| *fd > OpenFiles::STDERR_FD)
            .collect();
        descriptors.sort_unstable();
        for fd in descriptors {
            snapshot.not_kept.push(NotKept {
                kind: NotKeptKind::OpenFile,
                name: fd.to_string(),
            });
        }

        Ok(snapshot)
    }

    /// Applies a snapshot that [`Shell::snapshot`](Self::snapshot) took, in
    /// this process or another, to this shell, through the same API the
    /// builtins use: no part of it runs as a command. Functions, aliases,
    /// traps and disabled builtins end up exactly as the snapshot has them;
    /// variables the snapshot does not name keep their values here.
    ///
    /// Each item is applied on its own. One that fails is reported in the
    /// returned [`RestoreReport`], by name and never by value, and the rest
    /// are still applied. The working directory is left unchanged if the
    /// snapshot's is gone.
    ///
    /// # Arguments
    ///
    /// * `snapshot` - The snapshot to apply.
    ///
    /// # Errors
    ///
    /// Returns [`error::ErrorKind::NotBetweenCommands`] if a function or
    /// command scope is open; nothing is applied then.
    pub fn restore_snapshot(
        &mut self,
        snapshot: ShellSnapshot,
    ) -> Result<RestoreReport, error::Error> {
        self.check_between_commands()?;

        let mut report = RestoreReport::default();
        self.restore_working_dir(&snapshot.working_dir, &mut report)?;
        self.directory_stack = snapshot.directory_stack;
        self.restore_variables(&snapshot.unset_variables, snapshot.variables, &mut report)?;
        // Options first: `posix` and `extglob` change how a function parses,
        // and the snapshot checked each function under its own shell's options.
        self.restore_options(ShellOptionKind::SetO, snapshot.set_options, &mut report);
        self.restore_options(ShellOptionKind::Shopt, snapshot.shopt_options, &mut report);
        self.restore_functions(snapshot.functions, &mut report);
        self.aliases = snapshot.aliases.into_iter().collect();
        self.restore_traps(snapshot.traps, &mut report);
        self.restore_disabled_builtins(&snapshot.disabled_builtins, &mut report);
        *self.current_shell_args_mut() = snapshot.positional_parameters;
        self.set_last_exit_status(snapshot.last_exit_status);

        Ok(report)
    }

    /// Moves to `dir`, if it is still a directory, and keeps this shell's
    /// `OLDPWD`, which a snapshot does not carry.
    fn restore_working_dir(
        &mut self,
        dir: &Path,
        report: &mut RestoreReport,
    ) -> Result<(), error::Error> {
        let oldpwd = self
            .env
            .get_using_policy("OLDPWD", EnvironmentLookup::OnlyInGlobal)
            .cloned();
        if let Err(error) = self.set_working_dir(dir) {
            let reason = match error.kind() {
                error::ErrorKind::ReadonlyVariable => RestoreFailureReason::Readonly,
                _ => RestoreFailureReason::NotADirectory,
            };
            report.fail(StateKind::WorkingDir, &dir.to_string_lossy(), reason);
        }
        match oldpwd {
            Some(var) => self.env.set_global("OLDPWD", var)?,
            None => _ = self.env.unset("OLDPWD"),
        }
        Ok(())
    }

    /// Unsets `unset`, then sets `variables`, leaving a readonly variable that
    /// already holds the same value as it is.
    fn restore_variables(
        &mut self,
        unset: &BTreeSet<String>,
        variables: BTreeMap<String, SavedVariable>,
        report: &mut RestoreReport,
    ) -> Result<(), error::Error> {
        for name in unset {
            if self.env.unset(name).is_err() {
                report.fail(
                    StateKind::UnsetVariable,
                    name,
                    RestoreFailureReason::Readonly,
                );
            }
        }

        for (name, saved) in variables {
            if let Some(existing) = self
                .env
                .get_using_policy(&name, EnvironmentLookup::OnlyInGlobal)
                && existing.is_readonly()
            {
                if saved_variable(existing).as_ref() != Some(&saved) {
                    report.fail(StateKind::Variable, &name, RestoreFailureReason::Readonly);
                }
                continue;
            }
            self.env.set_global(name, shell_variable(saved))?;
        }
        Ok(())
    }

    /// Replaces every function with `functions`.
    fn restore_functions(
        &mut self,
        functions: BTreeMap<String, SavedFunction>,
        report: &mut RestoreReport,
    ) {
        self.funcs.clear();
        for (name, saved) in functions {
            if self
                .define_func_from_str(name.as_str(), &saved.body)
                .is_err()
            {
                report.fail(
                    StateKind::Function,
                    &name,
                    RestoreFailureReason::DoesNotParse,
                );
            } else if saved.exported
                && let Some(registration) = self.funcs.get_mut(&name)
            {
                registration.export();
            }
        }
    }

    /// Replaces every trap handler with `traps`.
    fn restore_traps(&mut self, traps: BTreeMap<String, String>, report: &mut RestoreReport) {
        self.traps = crate::traps::TrapHandlerConfig::default();
        for (signal, command) in traps {
            match TrapSignal::try_from(signal.as_str()) {
                Ok(signal) => {
                    self.traps
                        .register_handler(signal, command, SourceInfo::default());
                }
                Err(_) => report.fail(
                    StateKind::Trap,
                    &signal,
                    RestoreFailureReason::UnknownSignal,
                ),
            }
        }
    }

    /// Sets each option of `kind` in `values`.
    fn restore_options(
        &mut self,
        kind: ShellOptionKind,
        values: BTreeMap<String, bool>,
        report: &mut RestoreReport,
    ) {
        let state_kind = match kind {
            ShellOptionKind::Shopt => StateKind::ShoptOption,
            ShellOptionKind::Set | ShellOptionKind::SetO => StateKind::SetOption,
        };
        let definitions = namedoptions::options(kind);
        for (name, value) in values {
            if STARTUP_SHOPT_OPTIONS.contains(&name.as_str()) {
                continue;
            }
            match definitions.get(&name) {
                Some(definition) => definition.set(&mut self.options, value),
                None => report.fail(state_kind, &name, RestoreFailureReason::UnknownOption),
            }
        }
    }

    /// Turns off exactly the builtins in `names`.
    fn restore_disabled_builtins(&mut self, names: &BTreeSet<String>, report: &mut RestoreReport) {
        for registration in self.builtins.values_mut() {
            registration.disabled = false;
        }
        for name in names {
            match self.builtins.get_mut(name) {
                Some(registration) => registration.disabled = true,
                None => report.fail(
                    StateKind::DisabledBuiltin,
                    name,
                    RestoreFailureReason::UnknownBuiltin,
                ),
            }
        }
    }

    /// Fails unless only the global scope is open, as it is between commands.
    fn check_between_commands(&self) -> Result<(), error::Error> {
        if self.env.scope_depth() == 1 {
            Ok(())
        } else {
            Err(error::ErrorKind::NotBetweenCommands.into())
        }
    }

    /// Returns whether `body` parses as a function body that prints back as
    /// `body`, so that a shell restoring it defines the same function.
    fn parses_back(&self, body: &str) -> bool {
        let mut parser = super::parsing::create_parser(body.as_bytes(), &self.parser_options());
        parser
            .parse_function_parens_and_body()
            .is_ok_and(|parsed| std::format!("() {parsed}") == body)
    }

    /// Returns the value of every option of `kind` a command may change.
    fn option_values(&self, kind: ShellOptionKind) -> BTreeMap<String, bool> {
        namedoptions::options(kind)
            .iter()
            .filter(|option| !STARTUP_SHOPT_OPTIONS.contains(&option.name))
            .map(|option| (option.name.to_owned(), option.definition.get(&self.options)))
            .collect()
    }
}

/// Returns whether a snapshot leaves the variable `name` out by its name.
fn is_uncopied(name: &str, options: &SnapshotOptions) -> bool {
    UNCOPIED_VARIABLES.contains(&name)
        || wellknownvars::PROCESS_VARIABLES.contains(&name)
        || options.exclude.contains(name)
}

/// Returns whether `saved` is exactly what the embedder's baseline value
/// gives a new shell: that value, exported, with no other attribute.
fn holds_baseline(saved: &SavedVariable, baseline: Option<&String>) -> bool {
    baseline.is_some_and(|baseline| {
        saved.value == SavedValue::String(baseline.clone())
            && saved.exported
            && !saved.readonly
            && !saved.integer
            && !saved.nameref
            && !saved.trace
            && saved.transform == ShellVariableUpdateTransform::None
    })
}

/// Returns the copy of a variable, or `None` for a dynamic one, whose value
/// the shell computes.
fn saved_variable(var: &ShellVariable) -> Option<SavedVariable> {
    let value = match var.value() {
        ShellValue::Unset(unset_type) => SavedValue::Unset(*unset_type),
        ShellValue::String(value) => SavedValue::String(value.clone()),
        ShellValue::IndexedArray(values) => SavedValue::IndexedArray(values.clone()),
        ShellValue::AssociativeArray(values) => SavedValue::AssociativeArray(values.clone()),
        ShellValue::Dynamic { .. } => return None,
    };
    Some(SavedVariable {
        value,
        exported: var.is_exported(),
        readonly: var.is_readonly(),
        integer: var.is_treated_as_integer(),
        nameref: var.is_treated_as_nameref(),
        trace: var.is_trace_enabled(),
        transform: var.get_update_transform(),
    })
}

/// Returns the variable a copy describes.
fn shell_variable(saved: SavedVariable) -> ShellVariable {
    let mut var = ShellVariable::new(match saved.value {
        SavedValue::Unset(unset_type) => ShellValue::Unset(unset_type),
        SavedValue::String(value) => ShellValue::String(value),
        SavedValue::IndexedArray(values) => ShellValue::IndexedArray(values),
        SavedValue::AssociativeArray(values) => ShellValue::AssociativeArray(values),
    });
    if saved.exported {
        var.export();
    }
    if saved.readonly {
        var.set_readonly();
    }
    if saved.integer {
        var.treat_as_integer();
    }
    if saved.nameref {
        var.treat_as_nameref();
    }
    if saved.trace {
        var.enable_trace();
    }
    var.set_update_transform(saved.transform);
    var
}
