//! Integration tests for `RuntimeOptions::max_kept_descriptors`, the cap on
//! the file descriptors above standard error that `exec` and `coproc` leave a
//! shell keeping across commands.

#![cfg(unix)]
#![cfg(test)]
#![allow(clippy::panic_in_result_fn, clippy::expect_used)]

use anyhow::Result;
use shrub_builtins::{BuiltinSet, ShellBuilderExt as _};
use shrub_core::extensions::DefaultShellExtensions;
use shrub_core::options::{DEFAULT_MAX_KEPT_DESCRIPTORS, RuntimeOptions};
use shrub_core::{CreateOptions, Shell, ShellFd};

async fn new_shell(max: usize) -> Result<Shell> {
    Ok(Shell::builder()
        .default_builtins(BuiltinSet::BashMode)
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .kill_external_commands_on_drop(true)
        .max_kept_descriptors(max)
        .build()
        .await?)
}

/// Runs `script`, and returns whether it succeeded; an error the shell
/// returns instead of a status counts as a failure.
async fn succeeds(shell: &mut Shell, script: &str) -> bool {
    let params = shell.default_exec_params();
    shell
        .run_string(script, &shrub_core::SourceInfo::default(), &params)
        .await
        .is_ok_and(|result| result.is_success())
}

/// The descriptors above standard error the shell keeps, in order.
fn kept(shell: &Shell) -> Vec<ShellFd> {
    let mut fds: Vec<_> = shell
        .open_files()
        .iter_fds()
        .map(|(fd, _)| fd)
        .filter(|fd| *fd > 2)
        .collect();
    fds.sort_unstable();
    fds
}

#[tokio::test]
async fn a_built_shell_has_the_default_cap() -> Result<()> {
    let shell = Shell::builder().do_not_inherit_env(true).build().await?;
    assert_eq!(
        shell.options().max_kept_descriptors,
        Some(DEFAULT_MAX_KEPT_DESCRIPTORS)
    );
    Ok(())
}

#[test]
fn default_create_options_have_the_default_cap() {
    let options =
        RuntimeOptions::defaults_from(&CreateOptions::<DefaultShellExtensions>::default());
    assert_eq!(
        options.max_kept_descriptors,
        Some(DEFAULT_MAX_KEPT_DESCRIPTORS)
    );
}

#[tokio::test]
async fn exec_keeps_descriptors_up_to_the_cap() -> Result<()> {
    let mut shell = new_shell(2).await?;
    assert!(succeeds(&mut shell, "exec 3>/dev/null 4>/dev/null").await);
    assert_eq!(kept(&shell), [3, 4]);

    assert!(!succeeds(&mut shell, "exec 5>/dev/null").await);
    assert_eq!(kept(&shell), [3, 4]);

    // Replacing or swapping a kept descriptor does not grow the set.
    assert!(succeeds(&mut shell, "exec 4>/dev/null").await);
    assert!(succeeds(&mut shell, "exec 3>&- 5>/dev/null").await);
    assert_eq!(kept(&shell), [4, 5]);
    Ok(())
}

#[tokio::test]
async fn a_set_above_the_cap_can_still_shrink() -> Result<()> {
    let mut shell = new_shell(3).await?;
    assert!(succeeds(&mut shell, "exec 3>/dev/null 4>/dev/null 5>/dev/null").await);
    shell.options_mut().max_kept_descriptors = Some(1);

    assert!(succeeds(&mut shell, "exec 3>&-").await);
    assert_eq!(kept(&shell), [4, 5]);
    assert!(!succeeds(&mut shell, "exec 3>/dev/null").await);
    assert_eq!(kept(&shell), [4, 5]);
    Ok(())
}

#[tokio::test]
async fn no_cap_keeps_any_number() -> Result<()> {
    let mut shell = new_shell(0).await?;
    shell.options_mut().max_kept_descriptors = None;
    assert!(succeeds(&mut shell, "exec 3>/dev/null 4>/dev/null 5>/dev/null").await);
    assert_eq!(kept(&shell), [3, 4, 5]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn coproc_fails_when_its_descriptors_would_pass_the_cap() -> Result<()> {
    let mut shell = new_shell(1).await?;
    assert!(!succeeds(&mut shell, "coproc true").await);
    assert_eq!(kept(&shell), Vec::<ShellFd>::new());
    Ok(())
}
