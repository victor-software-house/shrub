use std::io::Write;

use clap::Parser;
use shrub_core::arithmetic::Evaluatable;
use shrub_core::{ExecutionExitCode, ExecutionResult, builtins};

/// Evaluate arithmetic expressions.
#[derive(Parser)]
pub(crate) struct LetCommand {
    /// Arithmetic expressions to evaluate.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    exprs: Vec<String>,
}

impl builtins::Command for LetCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<shrub_core::ExecutionResult, Self::Error> {
        let mut result = ExecutionExitCode::InvalidUsage.into();

        if self.exprs.is_empty() {
            writeln!(context.stderr(), "missing expression")?;
            return Ok(result);
        }

        for expr in &self.exprs {
            let parsed = shrub_parser::arithmetic::parse(expr.as_str())?;
            let evaluated = parsed.eval(context.shell)?;

            if evaluated == 0 {
                result = ExecutionResult::general_error();
            } else {
                result = ExecutionResult::success();
            }
        }

        Ok(result)
    }
}
