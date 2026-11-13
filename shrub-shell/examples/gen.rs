//! Generates artifacts derived from the `shrub` command-line interface: man
//! pages, markdown help, shell completion scripts, and the config file's JSON
//! schema.
//!
//! This lives here, rather than in `xtask`, so that the build tooling doesn't
//! need to depend on the shell it builds. It's driven by `cargo xtask gen`.

use std::path::PathBuf;
use std::{fs, io};

use anyhow::Result;
use clap::{CommandFactory, Parser};
use shrub_shell::args::CommandLineArgs;
use shrub_shell::config::Config;

/// Generate artifacts derived from the shrub command-line interface.
#[derive(Parser)]
enum GenCommand {
    /// Generate man content into the given directory.
    Man {
        /// Output directory.
        output_dir: PathBuf,
    },
    /// Generate help content in markdown format.
    Markdown {
        /// Output file path.
        output_path: PathBuf,
    },
    /// Generate a completion script, written to standard output.
    Completion {
        /// Shell to generate a completion script for.
        shell: clap_complete::Shell,
    },
    /// Generate the JSON schema for the configuration file.
    Schema {
        /// Output file path.
        output_path: PathBuf,
    },
}

fn main() -> Result<()> {
    match GenCommand::parse() {
        GenCommand::Man { output_dir } => {
            fs::create_dir_all(&output_dir)?;
            clap_mangen::generate_to(CommandLineArgs::command(), &output_dir)?;
        }
        GenCommand::Markdown { output_path } => {
            let options = clap_markdown::MarkdownOptions::new()
                .show_footer(false)
                .show_table_of_contents(true);
            let markdown = clap_markdown::help_markdown_custom::<CommandLineArgs>(&options);
            fs::write(&output_path, markdown)?;
        }
        GenCommand::Completion { shell } => {
            let mut cmd = CommandLineArgs::command();
            clap_complete::generate(shell, &mut cmd, "shrub", &mut io::stdout());
        }
        GenCommand::Schema { output_path } => {
            let schema = schemars::schema_for!(Config);
            let json = serde_json::to_string_pretty(&schema)?;
            fs::write(&output_path, format!("{json}\n"))?;
        }
    }

    Ok(())
}
