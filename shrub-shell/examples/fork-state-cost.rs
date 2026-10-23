//! Measures what `Shell::fork_state` costs on a shell with realistic state,
//! next to `Shell::clone` and one run of the external `true` program through
//! the same shell. The program is found on the shell's `PATH` and run by its
//! path, so the `true` builtin does not run in its place, and the report names
//! the path.
//!
//! The shell holds the process environment as exported variables, plus
//! `VARIABLES` variables, `FUNCTIONS` functions and a directory stack
//! `DIRECTORIES` deep. Each measured operation runs `WARM_UP` times, then
//! `RUNS` timed times, in this process; the report gives the median and the
//! 25th and 75th percentiles in microseconds.
//!
//! Run it with `cargo run --release --package shrub-shell --example
//! fork-state-cost`.

use std::fmt::Write as _;
use std::hint::black_box;
use std::time::{Duration, Instant};

use anyhow::Result;
use shrub_builtins::ShellBuilderExt;

/// Variables the shell defines beyond the process environment.
const VARIABLES: usize = 200;
/// Functions the shell defines.
const FUNCTIONS: usize = 50;
/// Directories pushed onto the directory stack.
const DIRECTORIES: usize = 5;
/// Untimed runs of each operation before the timed ones.
const WARM_UP: usize = 100;
/// Timed runs of each operation.
const RUNS: usize = 1000;
/// The external program each baseline run starts: the cheapest command that
/// forks and execs.
const BASELINE: &str = "true";

fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(measure())
}

async fn measure() -> Result<()> {
    let dir = assert_fs::TempDir::new()?;
    let mut shell = shrub_core::Shell::builder()
        .default_builtins(shrub_builtins::BuiltinSet::BashMode)
        .working_dir(dir.path().to_path_buf())
        .no_editing(true)
        .profile(shrub_core::ProfileLoadBehavior::Skip)
        .rc(shrub_core::RcLoadBehavior::Skip)
        .kill_external_commands_on_drop(true)
        .do_not_inherit_env(true)
        .build()
        .await?;

    for (name, value) in std::env::vars() {
        let mut variable = shrub_core::ShellVariable::new(value);
        variable.export();
        shell.env_mut().set_global(name, variable)?;
    }
    let mut script = String::new();
    for i in 0..VARIABLES {
        writeln!(
            script,
            "var_{i}='value {i} of a realistic length, say forty'"
        )?;
    }
    for i in 0..FUNCTIONS {
        writeln!(
            script,
            "func_{i}() {{ local a=\"$1\"; if [[ -n $a ]]; then echo \"$a\"; fi; return 0; }}"
        )?;
    }
    for i in 0..DIRECTORIES {
        let sub = dir.path().join(format!("d{i}"));
        std::fs::create_dir(&sub)?;
        writeln!(script, "pushd '{}' >/dev/null", sub.display())?;
    }
    run(&mut shell, &script).await?;
    anyhow::ensure!(
        shell.directory_stack().len() == DIRECTORIES,
        "the directory stack holds {} entries, not {DIRECTORIES}",
        shell.directory_stack().len()
    );

    let fork_state = time(|| drop(black_box(shell.fork_state())));
    let clone = time(|| drop(black_box(shell.clone())));
    let true_path = shell
        .find_first_executable_in_path(BASELINE)
        .ok_or_else(|| anyhow::anyhow!("no `{BASELINE}` program on PATH"))?;
    let true_command = true_path.display().to_string();
    anyhow::ensure!(
        !true_command.contains(char::is_whitespace),
        "`{true_command}` would need quoting"
    );
    let mut true_runs = Vec::with_capacity(RUNS);
    for i in 0..WARM_UP + RUNS {
        let start = Instant::now();
        run(&mut shell, &true_command).await?;
        if i >= WARM_UP {
            true_runs.push(start.elapsed());
        }
    }

    println!(
        "state: {} variables, {} functions, {} directories; {RUNS} runs after {WARM_UP} warm-up",
        shell.env().iter().count(),
        shell.funcs().iter().count(),
        shell.directory_stack().len()
    );
    let fork_state = report("fork_state", fork_state);
    report("clone", clone);
    let true_median = report(&true_command, true_runs);
    println!(
        "fork_state median / {true_command} median: {:.2}%",
        100.0 * fork_state.as_secs_f64() / true_median.as_secs_f64()
    );
    Ok(())
}

async fn run(shell: &mut shrub_core::Shell, script: &str) -> Result<()> {
    let params = shell.default_exec_params();
    let result = shell
        .run_string(script, &shrub_core::SourceInfo::default(), &params)
        .await?;
    anyhow::ensure!(
        result.is_success(),
        "`{script}` exited with {}",
        u8::from(result.exit_code)
    );
    Ok(())
}

fn time(mut operation: impl FnMut()) -> Vec<Duration> {
    for _ in 0..WARM_UP {
        operation();
    }
    (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            operation();
            start.elapsed()
        })
        .collect()
}

/// Prints the median and quartiles of `runs`, and returns the median.
fn report(name: &str, mut runs: Vec<Duration>) -> Duration {
    runs.sort_unstable();
    let quartile = |q: usize| runs[runs.len() * q / 4];
    let micros = |duration: Duration| duration.as_secs_f64() * 1e6;
    let median = quartile(2);
    println!(
        "{name}: median {:.1} µs, p25 {:.1} µs, p75 {:.1} µs",
        micros(median),
        micros(quartile(1)),
        micros(quartile(3))
    );
    median
}
