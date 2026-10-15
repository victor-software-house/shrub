//! Example demonstrating AST serialization and deserialization with the `serde`
//! feature.
//!
//! Run with: `cargo run --package shrub-parser --example serde --features
//! serde`

use std::io::BufReader;

use shrub_parser::{Parser, ParserOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse a simple shell command
    let input = "echo 'Hello, World!' && ls -la";
    let reader = BufReader::new(input.as_bytes());
    let options = ParserOptions::default();

    let mut parser = Parser::new(reader, &options);
    let program = parser.parse_program()?;

    // Serialize the AST to JSON
    let json = serde_json::to_string_pretty(&program)?;
    println!("Parsed AST:");
    println!("{json}");

    // Demonstrate round-trip: deserialize the JSON back to AST
    println!("\nRound-trip deserialization:");
    let deserialized: shrub_parser::ast::Program = serde_json::from_str(&json)?;
    println!(
        "Successfully deserialized AST with {} command(s)",
        deserialized.complete_commands.len()
    );

    Ok(())
}
