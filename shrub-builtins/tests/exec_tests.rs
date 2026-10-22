//! Integration tests for the `exec` builtin under `SpawnOptions`.

#![cfg(unix)]
#![cfg(test)]
#![allow(clippy::panic_in_result_fn, clippy::expect_used)]

use std::ffi::CString;
use std::sync::Arc;

use anyhow::Result;
use shrub_builtins::{BuiltinSet, ShellBuilderExt as _};
use shrub_core::options::SpawnOptions;

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
