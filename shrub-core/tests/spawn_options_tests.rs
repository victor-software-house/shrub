//! Integration tests for `SpawnOptions`: the observer that sees each external
//! process the shell starts, and the file each child writes `0` to before its
//! program runs, which an embedder points at a cgroup's `cgroup.procs`.
//!
//! The join file here is a plain file, so the tests run without a cgroup and
//! on every Unix: what they check is that the child writes `0` to it after the
//! fork and before the program, and that a child that cannot write it never
//! runs its program. `shrub-core` alone has no builtins, so every command in
//! these scripts is an external program. The open-file limit tests read the
//! child's limit with `ulimit -n`.

#![cfg(unix)]
#![cfg(test)]
#![allow(clippy::panic_in_result_fn, clippy::expect_used)]

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use nix::sys::resource::{RLIM_INFINITY, Resource};
use shrub_core::options::{SpawnOptions, SpawnedProcess};

/// How long the background process of a script may take to start and write
/// its pid.
const BACKGROUND_WAIT: Duration = Duration::from_secs(10);

/// An open-file limit low enough to sit below any test process's own, and
/// high enough for `/bin/sh` to start.
const OPEN_FILE_LIMIT: u64 = 100;

async fn new_shell(spawn: SpawnOptions) -> Result<shrub_core::Shell> {
    let mut shell = shrub_core::Shell::builder()
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .kill_external_commands_on_drop(true)
        .build()
        .await?;
    shell.options_mut().spawn = spawn;
    Ok(shell)
}

fn join_file(path: &Path) -> Arc<CString> {
    Arc::new(CString::new(path.as_os_str().as_bytes()).expect("a path without a NUL byte"))
}

async fn run(shell: &mut shrub_core::Shell, script: &str) -> Result<shrub_core::ExecutionResult> {
    let params = shell.default_exec_params();
    Ok(shell
        .run_string(script, &shrub_core::SourceInfo::default(), &params)
        .await?)
}

fn pids(path: &Path) -> Result<Vec<i32>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    Ok(text.lines().map(str::parse).collect::<Result<_, _>>()?)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_observer_sees_every_external_process_with_its_pid() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("pids");
    let seen: Arc<Mutex<Vec<SpawnedProcess>>> = Arc::default();
    let mut shell = new_shell(SpawnOptions {
        on_spawn: Some(Arc::new({
            let seen = Arc::clone(&seen);
            move |process| seen.lock().expect("the list's lock").push(process)
        })),
        ..SpawnOptions::default()
    })
    .await?;

    // An assignment, which starts no process, then one plain command, a
    // pipeline of two and a background job: four processes, each printing
    // its own pid.
    let script = format!(
        "unused=1; \
         /bin/sh -c 'echo $$' >> {file}; \
         /bin/sh -c 'echo $$' >> {file} | /bin/sh -c 'echo $$ >> {file}'; \
         /bin/sh -c 'echo $$' >> {file} &",
        file = file.display(),
    );
    let result = run(&mut shell, &script).await?;
    assert!(result.is_success());

    let began = Instant::now();
    while pids(&file)?.len() < 4 && began.elapsed() < BACKGROUND_WAIT {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let mut printed = pids(&file)?;
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
    let mut shell = new_shell(SpawnOptions {
        cgroup_procs: Some(join_file(&procs)),
        ..SpawnOptions::default()
    })
    .await?;

    // The program reads the file, so it sees the child's write only if the
    // write came first.
    let script = format!("/bin/cat {} > {}", procs.display(), seen.display());
    let result = run(&mut shell, &script).await?;

    assert!(result.is_success());
    assert_eq!(std::fs::read_to_string(&seen)?, "0");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_join_file_that_cannot_be_opened_fails_the_command_and_runs_nothing() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let marker = dir.path().join("marker");
    let mut shell = new_shell(SpawnOptions {
        cgroup_procs: Some(join_file(&dir.path().join("missing").join("cgroup.procs"))),
        ..SpawnOptions::default()
    })
    .await?;

    let script = format!("/usr/bin/touch {}", marker.display());
    let result = run(&mut shell, &script).await;

    // A missing join file must not read as a missing program.
    if let Ok(result) = result {
        assert!(
            !result.is_success(),
            "the command succeeded without its join file"
        );
        assert_ne!(
            u8::from(result.exit_code),
            127,
            "reported as command not found"
        );
    }
    assert!(!marker.exists(), "the program ran outside its cgroup");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn without_spawn_options_a_command_runs_as_before() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let marker = dir.path().join("marker");
    let mut shell = new_shell(SpawnOptions::default()).await?;

    let script = format!("/usr/bin/touch {}", marker.display());
    let result = run(&mut shell, &script).await?;

    assert!(result.is_success());
    assert!(marker.exists());

    Ok(())
}

/// Runs `/bin/sh -c 'ulimit -n'` in a shell with `open_file_limit` set to
/// `limit`, and returns what it prints.
async fn child_open_file_limit(limit: u64) -> Result<String> {
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("out");
    let mut shell = new_shell(SpawnOptions {
        open_file_limit: Some(limit),
        ..SpawnOptions::default()
    })
    .await?;

    let script = format!("/bin/sh -c 'ulimit -n' > {}", out.display());
    let result = run(&mut shell, &script).await?;

    assert!(result.is_success());
    Ok(std::fs::read_to_string(&out)?.trim().to_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn each_child_gets_the_open_file_limit() -> Result<()> {
    let (soft, hard) = nix::sys::resource::getrlimit(Resource::RLIMIT_NOFILE)?;
    // Below this process's own limit, so the child's limit has changed.
    let limit = soft.min(hard).min(OPEN_FILE_LIMIT) - 1;

    assert_eq!(child_open_file_limit(limit).await?, limit.to_string());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_file_limit_above_the_hard_limit_sets_the_hard_limit() -> Result<()> {
    let (_, hard) = nix::sys::resource::getrlimit(Resource::RLIMIT_NOFILE)?;
    // An unlimited hard limit has nothing above it.
    if hard == RLIM_INFINITY {
        return Ok(());
    }

    assert_eq!(child_open_file_limit(hard + 1).await?, hard.to_string());

    Ok(())
}
