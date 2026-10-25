//! Spawns real processes through `shrub-process` and checks what each child
//! got: its arguments and environment, its descriptors, its process group,
//! and the callbacks around the spawn.

#![cfg(unix)]
#![cfg(test)]
#![allow(clippy::panic_in_result_fn)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read as _;
use std::os::fd::OwnedFd;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use shrub_process::{Error, Group, Spawn, SpawnOptions, SpawnedProcess, spawn};

fn sh(script: &str) -> Spawn {
    Spawn::new("/bin/sh").args(["-c", script])
}

async fn stdout_of(request: Spawn) -> anyhow::Result<String> {
    let started = spawn(request.stdout(Stdio::piped()))?;
    let output = started.child.wait_with_output().await?;
    anyhow::ensure!(
        output.status.success(),
        "the child failed: {:?}",
        output.status
    );
    Ok(String::from_utf8(output.stdout)?)
}

#[tokio::test]
async fn the_child_gets_exactly_the_environment_and_arguments_given() -> anyhow::Result<()> {
    let env = BTreeMap::from([
        (OsString::from("ONLY"), OsString::from("this")),
        (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
    ]);

    let seen = stdout_of(
        sh(r#"printf '%s|%s|%s' "$ONLY" "${HOME-unset}" "$1""#)
            .args(["x", "argument"])
            .env(env),
    )
    .await?;

    assert_eq!(seen, "this|unset|argument");
    Ok(())
}

#[tokio::test]
async fn a_mapped_descriptor_reaches_the_child_at_its_number() -> anyhow::Result<()> {
    let (mut reader, writer) = std::io::pipe()?;

    let started = spawn(sh("printf mapped >&7").fd(7, OwnedFd::from(writer)))?;
    let status = started.child.wait_with_output().await?.status;
    let mut read = String::new();
    reader.read_to_string(&mut read)?;

    assert!(status.success());
    assert_eq!(read, "mapped");
    Ok(())
}

#[tokio::test]
async fn a_new_group_is_the_child_s_own() -> anyhow::Result<()> {
    let started = spawn(
        sh("ps -o pgid= -p $$")
            .stdout(Stdio::piped())
            .group(Group::New),
    )?;
    let pid = started.pid;
    let process_group = started.process_group;
    let output = started.child.wait_with_output().await?;

    let reported: i32 = String::from_utf8(output.stdout)?.trim().parse()?;
    assert_eq!(process_group, pid);
    assert_eq!(Some(reported), pid);
    Ok(())
}

/// Records its drop in the events it shares.
struct Guard(Arc<Mutex<Vec<String>>>);

impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut events) = self.0.lock() {
            events.push("guard dropped".to_owned());
        }
    }
}

#[tokio::test]
async fn the_guard_is_held_until_on_spawn_has_seen_the_child() -> anyhow::Result<()> {
    let events: Arc<Mutex<Vec<String>>> = Arc::default();
    let (before, after) = (Arc::clone(&events), Arc::clone(&events));
    let options = SpawnOptions {
        before_spawn: Some(Arc::new(move || {
            if let Ok(mut events) = before.lock() {
                events.push("before".to_owned());
            }
            Box::new(Guard(Arc::clone(&before)))
        })),
        on_spawn: Some(Arc::new(move |process: SpawnedProcess| {
            if let Ok(mut events) = after.lock() {
                events.push(format!(
                    "spawned {}",
                    process.process_group == Some(process.pid)
                ));
            }
        })),
        ..SpawnOptions::default()
    };

    let started = spawn(sh("exit 0").group(Group::New).options(options))?;
    started.child.wait_with_output().await?;

    let events = events
        .lock()
        .map_err(|_poisoned| anyhow::anyhow!("poisoned"))?
        .clone();
    assert_eq!(events, ["before", "spawned true", "guard dropped"]);
    Ok(())
}

#[tokio::test]
async fn two_descriptors_on_one_number_fail_the_spawn() -> anyhow::Result<()> {
    let (_first_reader, first) = std::io::pipe()?;
    let (_second_reader, second) = std::io::pipe()?;

    let result = spawn(
        sh("exit 0")
            .fd(5, OwnedFd::from(first))
            .fd(5, OwnedFd::from(second)),
    );

    assert!(matches!(result, Err(Error::FdMapping)), "{result:?}");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn a_process_limit_is_refused_before_anything_is_called() -> anyhow::Result<()> {
    let called = Arc::new(Mutex::new(false));
    let seen = Arc::clone(&called);
    let options = SpawnOptions {
        process_limit: Some(10),
        before_spawn: Some(Arc::new(move || {
            if let Ok(mut called) = seen.lock() {
                *called = true;
            }
            Box::new(())
        })),
        ..SpawnOptions::default()
    };

    let result = spawn(sh("exit 0").options(options));

    assert!(matches!(result, Err(Error::Unsupported(_))), "{result:?}");
    assert!(
        !*called
            .lock()
            .map_err(|_poisoned| anyhow::anyhow!("poisoned"))?
    );
    Ok(())
}
