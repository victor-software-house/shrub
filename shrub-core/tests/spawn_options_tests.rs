//! Integration tests for `SpawnOptions`: the observer that sees each external
//! process the shell starts, and the file each child writes `0` to before its
//! program runs, which an embedder points at a cgroup's `cgroup.procs`.
//!
//! The join file here is a plain file, so the tests run without a cgroup and
//! on every Unix: what they check is that the child writes `0` to it after the
//! fork and before the program, and that a child that cannot write it never
//! runs its program.

#![cfg(unix)]
#![cfg(test)]
#![allow(clippy::panic_in_result_fn, clippy::expect_used)]

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use shrub_core::{ExecutionParameters, SpawnOptions, SpawnedProcess};

async fn new_shell() -> Result<shrub_core::Shell> {
    Ok(shrub_core::Shell::builder()
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .kill_external_commands_on_drop(true)
        .build()
        .await?)
}

fn join_file(path: &Path) -> Arc<CString> {
    Arc::new(CString::new(path.as_os_str().as_bytes()).expect("a path without a NUL byte"))
}

async fn run(
    shell: &mut shrub_core::Shell,
    script: &str,
    params: &ExecutionParameters,
) -> Result<shrub_core::ExecutionResult> {
    Ok(shell
        .run_string(script, &shrub_core::SourceInfo::default(), params)
        .await?)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_observer_sees_every_external_process_and_no_builtin() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let pids = dir.path().join("pids");
    let mut shell = new_shell().await?;
    let seen: Arc<Mutex<Vec<SpawnedProcess>>> = Arc::default();
    let mut params = shell.default_exec_params();
    params.spawn = SpawnOptions {
        on_spawn: Some(Arc::new({
            let seen = Arc::clone(&seen);
            move |process| seen.lock().expect("the list's lock").push(process)
        })),
        ..SpawnOptions::default()
    };

    // One plain command, a pipeline of two and a background job: four
    // processes, each printing its own pid. `echo` and `wait` are builtins.
    let script = format!(
        "/bin/sh -c 'echo $$' >> {pids}; \
         /bin/sh -c 'echo $$' >> {pids} | /bin/sh -c 'echo $$ >> {pids}'; \
         /bin/sh -c 'echo $$' >> {pids} & wait; \
         echo builtin > /dev/null",
        pids = pids.display(),
    );
    let result = run(&mut shell, &script, &params).await?;
    assert!(result.is_success());

    let mut printed: Vec<i32> = std::fs::read_to_string(&pids)?
        .lines()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let mut observed: Vec<i32> = seen
        .lock()
        .expect("the list's lock")
        .iter()
        .map(|process| process.pid)
        .collect();
    printed.sort_unstable();
    observed.sort_unstable();
    assert_eq!(printed.len(), 4);
    assert_eq!(observed, printed);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn each_child_writes_zero_to_the_join_file_before_its_program_runs() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let procs = dir.path().join("cgroup.procs");
    let seen = dir.path().join("seen");
    std::fs::write(&procs, "x")?;
    let mut shell = new_shell().await?;
    let mut params = shell.default_exec_params();
    params.spawn = SpawnOptions {
        cgroup_procs: Some(join_file(&procs)),
        ..SpawnOptions::default()
    };

    // The program reads the file, so it sees the child's write only if the
    // write came first.
    let script = format!("/bin/cat {} > {}", procs.display(), seen.display());
    let result = run(&mut shell, &script, &params).await?;

    assert!(result.is_success());
    assert_eq!(std::fs::read_to_string(&seen)?, "0");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_join_file_that_cannot_be_opened_fails_the_command_and_runs_nothing() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let marker = dir.path().join("marker");
    let mut shell = new_shell().await?;
    let mut params = shell.default_exec_params();
    params.spawn = SpawnOptions {
        cgroup_procs: Some(join_file(&dir.path().join("missing").join("cgroup.procs"))),
        ..SpawnOptions::default()
    };

    let script = format!("/usr/bin/touch {}", marker.display());
    let result = run(&mut shell, &script, &params).await;

    assert!(
        result.is_err() || result.is_ok_and(|result| !result.is_success()),
        "the command succeeded without its join file"
    );
    assert!(!marker.exists(), "the program ran outside its cgroup");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn without_spawn_options_a_command_runs_as_before() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let marker = dir.path().join("marker");
    let mut shell = new_shell().await?;
    let params = shell.default_exec_params();

    let script = format!("/usr/bin/touch {}", marker.display());
    let result = run(&mut shell, &script, &params).await?;

    assert!(result.is_success());
    assert!(marker.exists());

    Ok(())
}
