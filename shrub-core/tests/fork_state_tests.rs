//! Tests for `Shell::fork_state`: a copy of the shell's state at the same
//! level, with its own job table, that a caller may keep in place of the
//! original or drop.
//!
//! `shrub-core` alone has no builtins, so the background command is an
//! external program and the working directory is changed through the API.

#![cfg(unix)]
#![cfg(test)]
#![allow(clippy::panic_in_result_fn)]

use anyhow::Result;

/// A background command that is still running when the test looks at the job
/// tables; `kill_external_commands_on_drop` stops it when its shell is dropped.
const BACKGROUND: &str = "/bin/sleep 30 &";

async fn new_shell() -> Result<shrub_core::Shell> {
    Ok(shrub_core::Shell::builder()
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .kill_external_commands_on_drop(true)
        .build()
        .await?)
}

async fn run(shell: &mut shrub_core::Shell, script: &str) -> Result<shrub_core::ExecutionResult> {
    let params = shell.default_exec_params();
    Ok(shell
        .run_string(script, &shrub_core::SourceInfo::default(), &params)
        .await?)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_copy_is_not_a_subshell() -> Result<()> {
    let shell = new_shell().await?;

    let copy = shell.fork_state();

    assert_eq!(copy.depth(), shell.depth());
    assert!(!copy.is_subshell());
    let subshell = shell.clone();
    assert_eq!(subshell.depth(), shell.depth() + 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn bash_subshell_is_unchanged_in_the_copy() -> Result<()> {
    let shell = shrub_core::Shell::builder()
        .do_not_inherit_env(true)
        .build()
        .await?;

    let copy = shell.fork_state();
    let subshell = shell.clone();

    assert_eq!(copy.env_str("BASH_SUBSHELL").as_deref(), Some("0"));
    assert_eq!(subshell.env_str("BASH_SUBSHELL").as_deref(), Some("1"));
    assert_eq!(shell.env_str("BASH_SUBSHELL").as_deref(), Some("0"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn the_copy_starts_with_an_empty_job_table() -> Result<()> {
    let mut shell = new_shell().await?;
    run(&mut shell, BACKGROUND).await?;
    assert_eq!(shell.jobs().jobs.len(), 1);

    let mut copy = shell.fork_state();
    assert!(copy.jobs().jobs.is_empty());

    run(&mut copy, BACKGROUND).await?;
    assert_eq!(copy.jobs().jobs.len(), 1);
    assert_eq!(shell.jobs().jobs.len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn changes_in_a_dropped_copy_do_not_reach_the_shell() -> Result<()> {
    let mut shell = new_shell().await?;
    run(&mut shell, "FOO=before").await?;
    let dir = tempfile::tempdir()?;

    let mut copy = shell.fork_state();
    run(&mut copy, "FOO=after; f() { /bin/true; }").await?;
    copy.set_working_dir(dir.path())?;
    drop(copy);

    assert_eq!(shell.env_str("FOO").as_deref(), Some("before"));
    assert!(shell.funcs().get("f").is_none());
    assert_ne!(shell.working_dir(), dir.path());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_kept_copy_carries_its_changes() -> Result<()> {
    let mut shell = new_shell().await?;
    run(&mut shell, "FOO=before").await?;
    let dir = tempfile::tempdir()?;

    let mut copy = shell.fork_state();
    run(&mut copy, "FOO=after; f() { /bin/true; }").await?;
    copy.set_working_dir(dir.path())?;
    shell = copy;

    assert_eq!(shell.env_str("FOO").as_deref(), Some("after"));
    assert!(shell.funcs().get("f").is_some());
    assert_eq!(shell.working_dir(), dir.path());
    assert!(!shell.is_subshell());

    Ok(())
}
