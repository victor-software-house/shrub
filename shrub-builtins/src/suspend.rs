use std::io::Write;
use std::process;

use clap::Parser;
use nix::sys::signal;
use shrub_core::traps::TrapSignal;
use shrub_core::{ExecutionExitCode, ExecutionResult, builtins, sys};

/// Suspend the shell.
#[derive(Parser)]
pub(crate) struct SuspendCommand {
    /// Force suspend login shells.
    #[arg(short = 'f')]
    force: bool,
}

impl builtins::Command for SuspendCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        if context.shell.options().login_shell && !self.force {
            writeln!(context.stderr(), "login shell cannot be suspended")?;
            return Ok(ExecutionExitCode::InvalidUsage.into());
        }

        #[expect(clippy::cast_possible_wrap)]
        sys::signal::kill_process(process::id() as i32, TrapSignal::Signal(signal::SIGSTOP))?;

        Ok(ExecutionResult::success())
    }
}
