//! A tokenizer and parsers for POSIX and bash shell syntax, with no
//! dependency on a running shell.
//!
//! `shrub-parser` is the bottom of shrub's crate graph: `shrub-core` builds on
//! it to run scripts, and front ends use it to inspect or highlight input. Use
//! it directly to read shell scripts into a syntax tree without executing them.
//!
//! ## Quick start
//!
//! Wrap the script in a reader, build a [`Parser`], and call
//! [`Parser::parse_program`]. The result is an [`ast::Program`].
//!
//! ```
//! use std::io::Cursor;
//!
//! use shrub_parser::{ParseError, Parser};
//!
//! let mut parser = Parser::builder().build(Cursor::new("echo hello | wc -c\nls -l\n"));
//! let program = parser.parse_program()?;
//!
//! assert_eq!(program.complete_commands.len(), 2);
//! # Ok::<(), ParseError>(())
//! ```
//!
//! Input that is not valid shell syntax returns a [`ParseError`] that says
//! where parsing failed.
//!
//! ## Parser options
//!
//! [`Parser::builder`] sets the options that change what the input means:
//! extended globbing, POSIX mode, `sh` mode, and tilde expansion.
//! [`Parser::new`] takes the same settings as a [`ParserOptions`] value. The
//! defaults match bash.
//!
//! ## Tokenizing
//!
//! [`tokenize_str`] splits a script into [`Token`]s without building a tree.
//! Each token is a word or an operator and records its [`SourceSpan`]. Words
//! keep their quotes; [`unquote_str`] removes them.
//!
//! ```
//! use shrub_parser::{Token, TokenizerError, tokenize_str, unquote_str};
//!
//! let tokens = tokenize_str("echo 'a b' > out")?;
//!
//! assert_eq!(tokens.len(), 4);
//! assert!(matches!(tokens[2], Token::Operator(..)));
//! assert_eq!(unquote_str(tokens[1].to_str()), "a b");
//! # Ok::<(), TokenizerError>(())
//! ```
//!
//! [`tokenize_str_with_options`] takes [`TokenizerOptions`], and
//! [`uncached_tokenize_str`] skips the cache that [`tokenize_str`] keeps of
//! recent inputs.
//!
//! ## Modules
//!
//! - [`ast`]: the syntax tree types.
//! - [`word`]: parses a word into its parts, such as parameter and command
//!   expansions.
//! - [`arithmetic`]: parses arithmetic expressions.
//! - [`pattern`]: parses glob and extglob patterns.
//! - [`test_command`]: parses `[[ ... ]]` and `test` expressions.
//! - [`prompt`]: parses prompt strings such as `PS1`.
//! - [`readline_binding`]: parses readline key binding syntax.
//!
//! ## Cargo features
//!
//! | Feature | Effect |
//! |---|---|
//! | `arbitrary` | Implements `arbitrary::Arbitrary` for the syntax tree and tokens, for fuzzing. |
//! | `debug-tracing` | Turns on the `peg` crate's rule tracing. |
//! | `diagnostics` | Adds [`PrettyError`] and `ParseError::to_pretty_error`, which render a parse error with `miette`. |
//! | `serde` | Implements `Serialize` and `Deserialize` for the syntax tree and tokens. |
//! | `winnow-parser` | Adds the `winnow`-based parser, selected with [`ParserImpl::Winnow`]. |

#![cfg_attr(docsrs, feature(doc_cfg))]
// TODO(unwrap): remove or scope this allow attribute
#![allow(clippy::unwrap_used)]

pub mod arithmetic;
pub mod ast;
pub mod pattern;
pub mod prompt;
pub mod readline_binding;
pub mod test_command;
pub mod word;

mod error;
mod parser;
mod source;
mod tokenizer;

#[cfg(test)]
mod snapshot_tests;

#[cfg(feature = "diagnostics")]
pub use error::miette::PrettyError;
pub use error::{
    BindingParseError, ParseError, ParseErrorLocation, TestCommandParseError, WordParseError,
};
#[cfg(feature = "winnow-parser")]
pub use parser::winnow_str;
pub use parser::{Parser, ParserBuilder, ParserImpl, ParserOptions, SourceInfo, parse_tokens};
pub use source::{SourcePosition, SourcePositionOffset, SourceSpan};
pub use tokenizer::{
    Token, TokenLocation, TokenizerError, TokenizerOptions, tokenize_str,
    tokenize_str_with_options, uncached_tokenize_str, unquote_str,
};
