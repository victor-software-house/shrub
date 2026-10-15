//! Shrub-only test harness.
//!
//! This test harness runs YAML-based test cases with inline expectations
//! or insta snapshots, without comparing against an oracle shell.

#![cfg(any(unix, windows))]

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use shrub_test_harness::{RunnerConfig, TestMode, TestOptions, TestRunner};

async fn run_shrub_tests(mut options: TestOptions) -> Result<bool> {
    // Resolve path to the shell-under-test.
    if options.shrub_path.is_empty() {
        options.shrub_path = assert_cmd::cargo::cargo_bin!("shrub")
            .to_string_lossy()
            .to_string();
    }
    if !Path::new(&options.shrub_path).exists() {
        return Err(anyhow::anyhow!(
            "shrub binary not found: {}",
            options.shrub_path
        ));
    }

    // Resolve test cases directory (in cases/shrub/).
    let test_cases_dir = options.test_cases_path.as_deref().map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cases/shrub"),
        |p| p.to_owned(),
    );

    let test_shell = options.create_test_shell_config()?;

    let config = RunnerConfig::new(PathBuf::from(&options.shrub_path), test_cases_dir)
        .with_mode(TestMode::Expectation)
        .with_platform_tags(options.platform_tags());

    let config = RunnerConfig {
        test_shell,
        ..config
    };

    let runner = TestRunner::new(config, options);
    runner.run().await
}

fn main() -> Result<()> {
    let unparsed_args: Vec<_> = std::env::args().collect();
    let options = TestOptions::parse_from(unparsed_args);

    let success = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(32)
        .build()?
        .block_on(run_shrub_tests(options))?;

    if !success {
        std::process::exit(1);
    }

    Ok(())
}
