//! Defines runtime options for the shell.

use std::ffi::CString;
use std::sync::Arc;

use itertools::Itertools;

use crate::{CreateOptions, extensions, namedoptions};

/// The default for [`CreateOptions::max_kept_descriptors`].
///
/// It is enough for the descriptors a script keeps with `exec` and `coproc`,
/// while a script that opens them in a loop fails long before the process
/// runs out of them.
pub const DEFAULT_MAX_KEPT_DESCRIPTORS: usize = 64;

/// What the shell does with each external process it spawns, for an embedder
/// that confines or tracks the processes its commands start. The default does
/// nothing.
///
/// The options are part of the shell's [`RuntimeOptions`], so they reach every
/// external command the shell runs, pipelines, background jobs and subshells
/// included, since those copy the shell they run in.
#[derive(Clone, Default)]
pub struct SpawnOptions {
    /// A file each child writes `0` to after it is forked and before it runs
    /// its program. For a cgroup v2's `cgroup.procs` file, the write moves
    /// the child into that cgroup, so it cannot fork a process outside it;
    /// only Linux has cgroups. When the child cannot open the file or write
    /// to it, the command fails to execute and its program never runs.
    pub cgroup_procs: Option<Arc<CString>>,
    /// Called with each child once it has started, before the shell waits
    /// for it. It runs on the shell's task, so it should return quickly.
    pub on_spawn: Option<Arc<dyn Fn(SpawnedProcess) + Send + Sync>>,
}

impl std::fmt::Debug for SpawnOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnOptions")
            .field("cgroup_procs", &self.cgroup_procs)
            .field("on_spawn", &self.on_spawn.as_ref().map(|_| "Fn"))
            .finish()
    }
}

/// An external process the shell has started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnedProcess {
    /// The child's process ID.
    pub pid: i32,
    /// The process group the child was placed in, if the shell knows it.
    pub process_group: Option<i32>,
}

/// Runtime changeable options for a shell instance.
#[derive(Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[expect(clippy::module_name_repetitions)]
pub struct RuntimeOptions {
    //
    // Single-character options.
    /// -a
    pub export_variables_on_modification: bool,
    /// -b
    pub notify_job_termination_immediately: bool,
    /// -e
    pub exit_on_nonzero_command_exit: bool,
    /// -f
    pub disable_filename_globbing: bool,
    /// -h
    pub remember_command_locations: bool,
    /// -k
    pub place_all_assignment_args_in_command_env: bool,
    /// -m
    pub enable_job_control: bool,
    /// -n
    pub do_not_execute_commands: bool,
    /// -p
    pub real_effective_uid_mismatch: bool,
    /// -t
    pub exit_after_one_command: bool,
    /// -u
    pub treat_unset_variables_as_error: bool,
    /// -v
    pub print_shell_input_lines: bool,
    /// -x
    pub print_commands_and_arguments: bool,
    /// -B
    pub perform_brace_expansion: bool,
    /// -C
    pub disallow_overwriting_regular_files_via_output_redirection: bool,
    /// -E
    pub shell_functions_inherit_err_trap: bool,
    /// -H
    pub enable_bang_style_history_substitution: bool,
    /// -P
    pub do_not_resolve_symlinks_when_changing_dir: bool,
    /// -T
    pub shell_functions_inherit_debug_and_return_traps: bool,

    //
    // Options set through -o.
    /// 'emacs'
    pub emacs_mode: bool,
    /// 'history'
    pub enable_command_history: bool,
    /// 'ignoreeof'
    pub ignore_eof: bool,
    /// 'pipefail'
    pub return_last_failure_from_pipeline: bool,
    /// 'posix'
    pub posix_mode: bool,
    /// 'vi'
    pub vi_mode: bool,

    //
    // Options set through shopt.
    /// `array_expand_once`
    pub array_expand_once: bool,
    /// `assoc_expand_once`
    pub assoc_expand_once: bool,
    /// 'autocd'
    pub auto_cd: bool,
    /// `bash_source_full_path`
    pub bash_source_full_path: bool,
    /// `cdable_vars`
    pub cdable_vars: bool,
    /// 'cdspell'
    pub cd_autocorrect_spelling: bool,
    /// 'checkhash'
    pub check_hashtable_before_command_exec: bool,
    /// 'checkjobs'
    pub check_jobs_before_exit: bool,
    /// 'checkwinsize'
    pub check_window_size_after_external_commands: bool,
    /// 'cmdhist'
    pub save_multiline_cmds_in_history: bool,
    /// 'compat31'
    pub compat31: bool,
    /// 'compat32'
    pub compat32: bool,
    /// 'compat40'
    pub compat40: bool,
    /// 'compat41'
    pub compat41: bool,
    /// 'compat42'
    pub compat42: bool,
    /// 'compat43'
    pub compat43: bool,
    /// 'compat44'
    pub compat44: bool,
    /// `complete_fullquote`
    pub quote_all_metachars_in_completion: bool,
    /// 'direxpand'
    pub expand_dir_names_on_completion: bool,
    /// 'dirspell'
    pub autocorrect_dir_spelling_on_completion: bool,
    /// 'dotglob'
    pub glob_matches_dotfiles: bool,
    /// 'execfail'
    pub exit_on_exec_fail: bool,
    /// `expand_aliases`
    pub expand_aliases: bool,
    /// 'extdebug'
    pub enable_debugger: bool,
    /// 'extglob'
    pub extended_globbing: bool,
    /// 'extquote'
    pub extquote: bool,
    /// 'failglob'
    pub fail_expansion_on_globs_without_match: bool,
    /// `force_fignore`
    pub force_fignore: bool,
    /// 'globasciiranges'
    pub glob_ranges_use_c_locale: bool,
    /// 'globskipdots'
    pub glob_skip_dots: bool,
    /// 'globstar'
    pub enable_star_star_glob: bool,
    /// `gnu_errfmt`
    pub errors_in_gnu_format: bool,
    /// 'histappend'
    pub append_to_history_file: bool,
    /// 'histreedit'
    pub allow_reedit_failed_history_subst: bool,
    /// 'histverify'
    pub allow_modifying_history_substitution: bool,
    /// 'hostcomplete'
    pub enable_hostname_completion: bool,
    /// 'huponexit'
    pub send_sighup_to_all_jobs_on_exit: bool,
    /// `inherit_errexit`
    pub command_subst_inherits_errexit: bool,
    /// `interactive_comments`
    pub interactive_comments: bool,
    /// 'lastpipe'
    pub run_last_pipeline_cmd_in_current_shell: bool,
    /// 'lithist'
    pub embed_newlines_in_multiline_cmds_in_history: bool,
    /// `localvar_inherit`
    pub local_vars_inherit_value_and_attrs: bool,
    /// `localvar_unset`
    pub localvar_unset: bool,
    /// `login_shell`
    pub login_shell: bool,
    /// 'mailwarn'
    pub mail_warn: bool,
    /// `no_empty_cmd_completion`
    pub no_empty_cmd_completion: bool,
    /// 'nocaseglob'
    pub case_insensitive_pathname_expansion: bool,
    /// 'nocasematch'
    pub case_insensitive_conditionals: bool,
    /// `noexpand_translation`
    pub no_expand_translation: bool,
    /// 'nullglob'
    pub expand_non_matching_patterns_to_null: bool,
    /// `patsub_replacement`
    pub patsub_replacement: bool,
    /// 'progcomp'
    pub programmable_completion: bool,
    /// `progcomp_alias`
    pub programmable_completion_alias: bool,
    /// 'promptvars'
    pub expand_prompt_strings: bool,
    /// `restricted_shell`
    pub restricted_shell: bool,
    /// `shift_verbose`
    pub shift_verbose: bool,
    /// `sourcepath`
    pub source_builtin_searches_path: bool,
    /// `varredir_close`
    pub var_redir_close: bool,
    /// `xpg_echo`
    pub echo_builtin_expands_escape_sequences: bool,

    //
    // Options set by the shell.
    /// Whether or not the shell is interactive.
    pub interactive: bool,
    /// Whether commands are being read from stdin.
    pub read_commands_from_stdin: bool,
    /// Whether the shell is in command string mode (-c).
    pub command_string_mode: bool,
    /// Whether or not the shell is in maximal `sh` compatibility mode.
    pub sh_mode: bool,
    /// Whether to treat external commands as session leaders.
    pub external_cmd_leads_session: bool,
    /// Whether externally spawned commands are killed when their spawning shell
    /// is dropped.
    pub kill_external_commands_on_drop: bool,
    /// What the shell does with each external process it spawns. Defaults to
    /// nothing; an embedder sets it before each command it runs.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub spawn: SpawnOptions,
    /// Maximum function call depth.
    pub max_function_call_depth: Option<usize>,
    /// The most file descriptors above standard error that the shell keeps
    /// across commands, such as those `exec 3>file` or `coproc` open; `None`
    /// keeps any number. A command that would keep more fails, and the shell
    /// keeps the descriptors it had.
    pub max_kept_descriptors: Option<usize>,
}

impl RuntimeOptions {
    /// Creates a default set of runtime options based on the given creation
    /// options.
    ///
    /// # Arguments
    ///
    /// * `create_options` - The options used to create the shell.
    pub fn defaults_from<SE: extensions::ShellExtensions>(
        create_options: &CreateOptions<SE>,
    ) -> Self {
        // There's a set of options enabled by default for all shells.
        let mut options = Self {
            interactive: create_options.interactive,
            disallow_overwriting_regular_files_via_output_redirection: create_options
                .disallow_overwriting_regular_files_via_output_redirection,
            do_not_execute_commands: create_options.do_not_execute_commands,
            enable_command_history: create_options.interactive,
            enable_job_control: create_options.interactive,
            exit_after_one_command: create_options.exit_after_one_command,
            read_commands_from_stdin: create_options.read_commands_from_stdin,
            command_string_mode: create_options.command_string_mode,
            sh_mode: create_options.sh_mode,
            posix_mode: create_options.posix,
            print_commands_and_arguments: create_options.print_commands_and_arguments,
            print_shell_input_lines: create_options.verbose,
            treat_unset_variables_as_error: create_options.treat_unset_variables_as_error,
            exit_on_nonzero_command_exit: create_options.exit_on_nonzero_command_exit,
            external_cmd_leads_session: create_options.external_cmd_leads_session,
            kill_external_commands_on_drop: create_options.kill_external_commands_on_drop,
            login_shell: create_options.login,
            disable_filename_globbing: create_options.disable_pathname_expansion,
            remember_command_locations: true,
            check_window_size_after_external_commands: true,
            save_multiline_cmds_in_history: true,
            extquote: true,
            force_fignore: true,
            case_insensitive_pathname_expansion:
                crate::sys::fs::default_case_insensitive_path_expansion(),
            enable_hostname_completion: true,
            interactive_comments: true,
            expand_prompt_strings: true,
            source_builtin_searches_path: true,
            perform_brace_expansion: true,
            quote_all_metachars_in_completion: true,
            programmable_completion: true,
            glob_ranges_use_c_locale: true,
            glob_skip_dots: true,
            patsub_replacement: true,
            max_function_call_depth: create_options.max_function_call_depth,
            max_kept_descriptors: Some(
                create_options
                    .max_kept_descriptors
                    .unwrap_or(DEFAULT_MAX_KEPT_DESCRIPTORS),
            ),
            ..Self::default()
        };

        // Additional options are enabled by default for interactive shells.
        if create_options.interactive {
            options.enable_bang_style_history_substitution = true;
            options.emacs_mode = !create_options.no_editing;
            options.expand_aliases = true;
        }

        // Update any options.
        for enabled_option in &create_options.enabled_options {
            if let Some(option) = namedoptions::options(namedoptions::ShellOptionKind::SetO)
                .get(enabled_option.as_str())
            {
                option.set(&mut options, true);
            }
        }
        for disabled_option in &create_options.disabled_options {
            if let Some(option) = namedoptions::options(namedoptions::ShellOptionKind::SetO)
                .get(disabled_option.as_str())
            {
                option.set(&mut options, false);
            }
        }

        // Update any shopt options.
        for enabled_option in &create_options.enabled_shopt_options {
            if let Some(shopt_option) = namedoptions::options(namedoptions::ShellOptionKind::Shopt)
                .get(enabled_option.as_str())
            {
                shopt_option.set(&mut options, true);
            }
        }
        for disabled_option in &create_options.disabled_shopt_options {
            if let Some(shopt_option) = namedoptions::options(namedoptions::ShellOptionKind::Shopt)
                .get(disabled_option.as_str())
            {
                shopt_option.set(&mut options, false);
            }
        }

        options
    }

    /// Returns a string representing the current `set`-style option flags set
    /// in the shell.
    pub fn option_flags(&self) -> String {
        let mut cs = vec![];

        for o in namedoptions::options(namedoptions::ShellOptionKind::Set).iter() {
            if o.definition.get(self)
                && let Some(c) = o.name.chars().next()
            {
                cs.push(c);
            }
        }

        // Sort the flags in a way that matches what bash does.
        cs.sort_by_key(|flag| option_flag_sort_key(*flag));

        cs.into_iter().collect()
    }

    /// Returns a colon-separated list of sorted 'set -o' options enabled.
    pub fn seto_optstr(&self) -> String {
        let mut cs = vec![];

        for option in namedoptions::options(namedoptions::ShellOptionKind::SetO).iter() {
            if option.definition.get(self) {
                cs.push(option.name);
            }
        }

        cs.sort_unstable();
        cs.into_iter().join(":")
    }

    /// Returns a colon-separated list of sorted 'shopt' options enabled.
    pub fn shopt_optstr(&self) -> String {
        let mut cs = vec![];

        for option in namedoptions::options(namedoptions::ShellOptionKind::Shopt).iter() {
            if option.definition.get(self) {
                cs.push(option.name);
            }
        }

        cs.sort_unstable();
        cs.into_iter().join(":")
    }
}

/// Sort option flag character in a way that mirrors bash behavior.
///
/// # Arguments
///
/// * `ch` - The option flag character.
const fn option_flag_sort_key(ch: char) -> (u8, char) {
    // NOTE: bash appears to sort in 3 groups. We mimic them:
    //    1) Lowercase letters excluding 'c' and 's' (sorted)
    //    2) Uppercase letters (sorted)
    //    3) All other characters (sorted)
    let group = if ch.is_ascii_lowercase() && !matches!(ch, 'c' | 's') {
        0
    } else if ch.is_ascii_uppercase() {
        1
    } else {
        2
    };

    (group, ch)
}

#[cfg(test)]
mod tests {
    use super::option_flag_sort_key;

    #[test]
    fn lowercase_excluding_c_and_s_sort_first() {
        let mut flags = vec!['b', 'A', 'Z', 's', 'c', 'a'];
        flags.sort_by_key(|flag| option_flag_sort_key(*flag));

        assert_eq!(flags, vec!['a', 'b', 'A', 'Z', 'c', 's']);
    }

    #[test]
    fn uppercase_sorted_before_miscellaneous() {
        let mut flags = vec!['P', 'B', '1', 'T'];
        flags.sort_by_key(|flag| option_flag_sort_key(*flag));

        assert_eq!(flags, vec!['B', 'P', 'T', '1']);
    }

    #[test]
    fn miscellaneous_characters_respect_ascii_order() {
        let mut flags = vec!['s', 'c', '%', ':'];
        flags.sort_by_key(|flag| option_flag_sort_key(*flag));

        assert_eq!(flags, vec!['%', ':', 'c', 's']);
    }
}
