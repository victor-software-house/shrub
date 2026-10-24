//! Integration tests for the `exec` builtin under `SpawnOptions`.

#![cfg(unix)]
#![cfg(test)]
#![allow(clippy::panic_in_result_fn, clippy::expect_used)]

use std::ffi::CString;
use std::sync::Arc;

use anyhow::Result;
use shrub_builtins::{BuiltinSet, ShellBuilderExt as _};
use shrub_core::options::{PidsLimit, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn exec_with_a_missing_join_file_fails_without_reading_as_not_found() -> Result<()> {
    let mut shell = shrub_core::Shell::builder()
        .default_builtins(BuiltinSet::BashMode)
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .build()
        .await?;
    shell.options_mut().spawn = SpawnOptions {
        cgroup_procs: Some(Arc::new(CString::new(
            "/nonexistent-shrub-cgroup/cgroup.procs",
        )?)),
        ..SpawnOptions::default()
    };

    // Were the join to succeed, this test process would become the program
    // and exit 3, which fails the test run.
    let params = shell.default_exec_params();
    let result = shell
        .run_string(
            "exec /bin/sh -c 'exit 3'",
            &shrub_core::SourceInfo::default(),
            &params,
        )
        .await;

    if let Ok(result) = result {
        assert!(!result.is_success(), "exec succeeded without its join file");
        assert_ne!(
            u8::from(result.exit_code),
            127,
            "reported as command not found"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn exec_with_process_limits_is_refused_and_the_shell_lives_on() -> Result<()> {
    let mut shell = shrub_core::Shell::builder()
        .default_builtins(BuiltinSet::BashMode)
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .build()
        .await?;
    // A count far under its limit, which a forked child would pass.
    let count = std::env::temp_dir().join(format!("shrub-exec-count-{}", std::process::id()));
    std::fs::write(&count, "1\n")?;
    let current = Arc::new(std::fs::File::open(&count)?);
    std::fs::remove_file(&count)?;
    shell.options_mut().spawn = SpawnOptions {
        pids_limits: vec![PidsLimit {
            current,
            max: u64::MAX,
        }]
        .into(),
        ..SpawnOptions::default()
    };

    // exec would replace this test process: were it not refused, the test
    // run would end here with status 3.
    let params = shell.default_exec_params();
    let result = shell
        .run_string(
            "exec /bin/sh -c 'exit 3'",
            &shrub_core::SourceInfo::default(),
            &params,
        )
        .await?;

    assert_eq!(u8::from(result.exit_code), 126);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn exec_with_a_process_limit_is_refused_and_the_shell_lives_on() -> Result<()> {
    let mut shell = shrub_core::Shell::builder()
        .default_builtins(BuiltinSet::BashMode)
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .build()
        .await?;
    shell.options_mut().spawn = SpawnOptions {
        process_limit: Some(u64::MAX),
        ..SpawnOptions::default()
    };

    // exec would replace this test process: were it not refused, the test
    // run would end here with status 3.
    let params = shell.default_exec_params();
    let result = shell
        .run_string(
            "exec /bin/sh -c 'exit 3'",
            &shrub_core::SourceInfo::default(),
            &params,
        )
        .await?;

    assert_eq!(u8::from(result.exit_code), 126);

    Ok(())
}
