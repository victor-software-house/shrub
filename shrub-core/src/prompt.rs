use std::path::Path;

use jiff::Zoned;
use shrub_parser::prompt;
use shrub_parser::prompt::{PromptDateFormat, PromptPiece, PromptTimeFormat};

use crate::shell::Shell;
use crate::sys::{self, users};
use crate::{ExecutionParameters, error, expansion, extensions, time_format};

const VERSION_MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");
const VERSION_MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");
const VERSION_PATCH: &str = env!("CARGO_PKG_VERSION_PATCH");

pub(crate) async fn expand_prompt(
    shell: &mut Shell<impl extensions::ShellExtensions>,
    params: &ExecutionParameters,
    spec: &str,
) -> Result<String, error::Error> {
    // Parse the prompt spec into its pieces.
    let prompt_pieces = parse_prompt(spec)?;

    // Now, render each piece.
    let mut formatted_prompt = String::new();
    for piece in prompt_pieces {
        // Pieces that semantically represent a literal char (e.g. user wrote
        // `\$` meaning a literal `$`, not input for pass-2 expansion). We
        // prepend a `\` so pass 2 consumes it and leaves the char alone.
        let semantically_literal = matches!(piece, PromptPiece::DollarOrPound);

        let formatted_piece = format_prompt_piece(shell, piece)?;

        // Only useful when pass 2 actually consumes a `\` for that leading
        // byte; otherwise the `\` would leak through.
        if shell.options().expand_prompt_strings
            && semantically_literal
            && formatted_piece.starts_with(expansion::DOUBLE_QUOTED_ESCAPE_CHARS)
        {
            formatted_prompt.push('\\');
        }

        formatted_prompt.push_str(&formatted_piece);
    }

    if shell.options().expand_prompt_strings {
        // Now expand any remaining escape sequences, but without
        // tilde-expansion. Use double-quote escape rules so that
        // backslashes emitted in the previous step survive intact
        // unless they precede a character that would also be escapable
        // inside a double-quoted string.
        let options = expansion::ExpanderOptions {
            tilde_expand: false,
            brace_expand: false,
            unquoted_backslash_handling: expansion::UnquotedBackslashHandling::DoubleQuoted,
            ..Default::default()
        };
        formatted_prompt =
            expansion::basic_expand_word_with_options(shell, params, &formatted_prompt, &options)
                .await?;
    }

    Ok(formatted_prompt)
}

#[cached::macros::cached(max_size = 64, key = "String", convert = r#"{ spec.to_owned() }"#)]
fn parse_prompt(spec: &str) -> Result<Vec<PromptPiece>, shrub_parser::WordParseError> {
    prompt::parse(spec)
}

fn format_prompt_piece(
    shell: &Shell<impl extensions::ShellExtensions>,
    piece: PromptPiece,
) -> Result<String, error::Error> {
    let formatted = match piece {
        PromptPiece::EscapedSequence(s) => s,
        PromptPiece::Literal(l) => l,
        PromptPiece::AsciiCharacter(c) => {
            char::from_u32(c).map_or_else(String::new, |c| c.to_string())
        }
        PromptPiece::Backslash => "\\".to_owned(),
        PromptPiece::BellCharacter => "\x07".to_owned(),
        PromptPiece::CarriageReturn => "\r".to_owned(),
        PromptPiece::CurrentCommandNumber => {
            return error::unimp("prompt: current command number");
        }
        PromptPiece::CurrentHistoryNumber => {
            return error::unimp("prompt: current history number");
        }
        PromptPiece::CurrentUser => users::get_current_username()?,
        PromptPiece::CurrentWorkingDirectory {
            tilde_replaced,
            basename,
        } => format_current_working_directory(shell, tilde_replaced, basename),
        PromptPiece::Date(format) => format_date(&Zoned::now(), &format)?,
        PromptPiece::DollarOrPound => {
            if users::is_root() {
                "#".to_owned()
            } else {
                "$".to_owned()
            }
        }
        // NOTE: We mimic bash and convert \[ into \001, a.k.a.
        // RL_PROMPT_START_IGNORE. It will need to get removed before it's
        // actually displayed. While present it also has the important
        // (compatible) side effect of ensuring the text on either side of it is
        // not concatenated together, potentially resulting in incompatible
        // variable expansions. Also, we *only* do this if the shell is
        // interactive.
        PromptPiece::EndNonPrintingSequence => {
            if shell.options().interactive {
                "\x02".to_owned()
            } else {
                String::new()
            }
        }
        PromptPiece::EscapeCharacter => "\x1b".to_owned(),
        PromptPiece::Hostname {
            only_up_to_first_dot,
        } => {
            let hn = sys::network::get_hostname()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if only_up_to_first_dot && let Some((first, _)) = hn.split_once('.') {
                return Ok(first.to_owned());
            }
            hn
        }
        PromptPiece::Newline => "\n".to_owned(),
        PromptPiece::NumberOfManagedJobs => shell.jobs().jobs.len().to_string(),
        PromptPiece::ShellBaseName => {
            if let Some(shell_name) = shell.current_shell_name() {
                Path::new(shell_name.as_ref())
                    .file_name()
                    .map_or_default(|name| name.to_string_lossy().to_string())
            } else {
                String::new()
            }
        }
        PromptPiece::ShellRelease => {
            std::format!("{VERSION_MAJOR}.{VERSION_MINOR}.{VERSION_PATCH}")
        }
        PromptPiece::ShellVersion => {
            std::format!("{VERSION_MAJOR}.{VERSION_MINOR}")
        }
        // NOTE: See above note for EndNonPrintingSequence
        PromptPiece::StartNonPrintingSequence => {
            if shell.options().interactive {
                "\x01".to_owned()
            } else {
                String::new()
            }
        }
        PromptPiece::TerminalDeviceBaseName => sys::terminal::try_get_terminal_device_path()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_default(),
        PromptPiece::Time(time_fmt) => format_time(&Zoned::now(), &time_fmt)?,
    };

    Ok(formatted)
}

fn format_current_working_directory(
    shell: &Shell<impl extensions::ShellExtensions>,
    tilde_replaced: bool,
    basename: bool,
) -> String {
    let mut working_dir_str = shell.working_dir().to_string_lossy().to_string();

    if tilde_replaced {
        working_dir_str = shell.tilde_shorten(working_dir_str);
    }

    if basename && let Some(filename) = Path::new(&working_dir_str).file_name() {
        working_dir_str = filename.to_string_lossy().to_string();
    }

    // ast-grep-ignore: platform-cfg-outside-sys
    if cfg!(windows) {
        working_dir_str = working_dir_str.replace('\\', "/");
    }

    working_dir_str
}

fn format_time(datetime: &Zoned, format: &PromptTimeFormat) -> Result<String, error::Error> {
    let format = match format {
        PromptTimeFormat::TwelveHourAM => "%I:%M %p",
        PromptTimeFormat::TwelveHourHHMMSS => "%I:%M:%S",
        PromptTimeFormat::TwentyFourHourHHMM => "%H:%M",
        PromptTimeFormat::TwentyFourHourHHMMSS => "%H:%M:%S",
    };

    time_format::format_zoned(datetime, format)
}

fn format_date(datetime: &Zoned, format: &PromptDateFormat) -> Result<String, error::Error> {
    let format = match format {
        PromptDateFormat::WeekdayMonthDate => "%a %b %d",
        PromptDateFormat::Custom(fmt) => fmt.as_str(),
    };

    time_format::format_zoned(datetime, format)
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use jiff::tz::TimeZone;
    use shrub_parser::prompt::{PromptDateFormat, PromptTimeFormat};

    use super::*;

    fn utc(rfc3339: &str) -> Result<Zoned, error::Error> {
        Ok(rfc3339.parse::<Timestamp>()?.to_zoned(TimeZone::UTC))
    }

    #[test]
    fn test_format_time() -> Result<(), error::Error> {
        // Create a well-known test date/time.
        let dt = utc("2024-12-25T13:34:56.789Z")?;

        assert_eq!(
            format_time(&dt, &PromptTimeFormat::TwelveHourAM)?,
            "01:34 PM"
        );
        assert_eq!(
            format_time(&dt, &PromptTimeFormat::TwentyFourHourHHMMSS)?,
            "13:34:56"
        );
        assert_eq!(
            format_time(&dt, &PromptTimeFormat::TwelveHourHHMMSS)?,
            "01:34:56"
        );
        Ok(())
    }

    #[test]
    fn test_format_date() -> Result<(), error::Error> {
        // Create a well-known test date/time.
        let dt = utc("2024-12-25T12:34:56.789Z")?;

        assert_eq!(
            format_date(&dt, &PromptDateFormat::WeekdayMonthDate)?,
            "Wed Dec 25"
        );
        assert_eq!(
            format_date(
                &dt,
                &PromptDateFormat::Custom(String::from("%Y-%m-%d %H:%M:%S"))
            )?,
            "2024-12-25 12:34:56"
        );
        Ok(())
    }
}
