use std::borrow::Cow;

use clap::Parser;
use shrub_core::{ExecutionResult, Shell, builtins};

/// Unset a variable.
#[derive(Parser)]
pub(crate) struct UnsetCommand {
    #[clap(flatten)]
    name_interpretation: UnsetNameInterpretation,

    /// Names of variables to unset.
    names: Vec<String>,
}

#[derive(Parser)]
#[clap(group = clap::ArgGroup::new("name-interpretation").multiple(false).required(false))]
pub(crate) struct UnsetNameInterpretation {
    /// Treat each name as a shell function.
    #[arg(short = 'f', group = "name-interpretation")]
    shell_functions: bool,

    /// Treat each name as a shell variable.
    #[arg(short = 'v', group = "name-interpretation")]
    shell_variables: bool,

    /// Treat each name as a name reference.
    #[arg(short = 'n', group = "name-interpretation")]
    name_references: bool,
}

impl UnsetNameInterpretation {
    pub const fn unspecified(&self) -> bool {
        !self.shell_functions && !self.shell_variables && !self.name_references
    }
}

impl builtins::Command for UnsetCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<shrub_core::ExecutionResult, Self::Error> {
        //
        // TODO(nameref): implement nameref
        //
        if self.name_interpretation.name_references {
            return shrub_core::error::unimp("unset: name references are not yet implemented");
        }

        let unspecified = self.name_interpretation.unspecified();

        #[expect(clippy::needless_continue)]
        for name in &self.names {
            if unspecified || self.name_interpretation.shell_variables {
                // Try to parse the name as a parameter. If we can't, don't
                // bail; it may not be a valid variable
                // name/parameter but could still be a function name.
                if let Ok(parameter) =
                    shrub_parser::word::parse_parameter(name, &context.shell.parser_options())
                {
                    let result = match parameter {
                        shrub_parser::word::Parameter::Positional(_) => continue,
                        shrub_parser::word::Parameter::Special(_) => continue,
                        shrub_parser::word::Parameter::Named(name) => {
                            context.shell.env_mut().unset(name.as_str())?.is_some()
                        }
                        shrub_parser::word::Parameter::NamedWithIndex { name, index } => {
                            unset_array_index(context.shell, name.as_str(), index.as_str())?
                        }
                        shrub_parser::word::Parameter::NamedWithAllIndices {
                            name: _,
                            concatenate: _,
                        } => continue,
                    };

                    if result {
                        continue;
                    }
                }
            }

            // TODO(unset): Deal with readonly functions
            if unspecified || self.name_interpretation.shell_functions {
                if context.shell.undefine_func(name) {
                    continue;
                }
            }
        }

        Ok(ExecutionResult::success())
    }
}

fn unset_array_index(
    shell: &mut Shell<impl shrub_core::ShellExtensions>,
    name: &str,
    index: &str,
) -> Result<bool, shrub_core::Error> {
    // First check to see if it's an associative array.
    let is_assoc_array = shell
        .env()
        .get(name)
        .is_some_and(|(_, var)| var.value().is_associative_array());

    // Compute which index we should actually use. For indexed arrays, we need
    // to evaluate the index string as an arithmetic expression first.
    let index_to_use: Cow<'_, str> = if is_assoc_array {
        index.into()
    } else {
        // First evaluate the index expression.
        let index_as_expr = shrub_parser::arithmetic::parse(index)?;
        let evaluated_index = shell.eval_arithmetic(&index_as_expr)?;
        evaluated_index.to_string().into()
    };

    // Now we can try to unset, and return the result.
    shell.env_mut().unset_index(name, index_to_use.as_ref())
}
