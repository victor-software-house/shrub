use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use shrub_core::openfiles::OpenFile;
use shrub_core::{ExecutionResult, builtins};

use crate::program::{run_program, split_args};

/// (*EXPERIMENTAL*) `cat`, `head` or `tail`, by the name it is registered
/// under, through the program of that name found in `PATH`. A file the
/// embedder's read window answers for is read from the first byte it names;
/// with no such file, every word goes to the program unchanged.
///
/// `cat` reads every operand, windowed or not, from one stream on its
/// standard input, so options that span files, such as `-n`, behave as with
/// the files named. `head` and `tail` read one operand at a time, each a
/// windowed file on standard input or a path, under the header the program
/// would print.
#[derive(Parser)]
#[command(disable_help_flag = true)]
pub(crate) struct WindowCommand {
    /// Options and paths, as the program takes them.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

/// The options of `head` and `tail` that take the next word as their value.
const WITH_VALUE: &[&str] = &["-n", "-c"];

impl builtins::Command for WindowCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        mut context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let name = context.command_name.clone();
        let with_value = if name == "cat" { &[][..] } else { WITH_VALUE };
        let (options, paths) = split_args(&self.args, with_value);
        let window = context.shell.options().read_window.clone();
        let operands: Vec<Operand> = paths
            .iter()
            .map(|path| {
                let absolute = context.shell.absolute_path(path);
                let start = window
                    .as_ref()
                    .and_then(|window| window.first_byte(&absolute));
                Operand {
                    word: (*path).to_owned(),
                    path: absolute,
                    start,
                }
            })
            .collect();
        if operands.iter().all(|operand| operand.start.is_none()) {
            return run_program(&mut context, &name, &self.args, None).await;
        }
        let options: Vec<String> = options.into_iter().map(str::to_owned).collect();
        if name == "cat" {
            return cat(&mut context, &options, operands).await;
        }
        // The builtin prints the headers itself, so the program, which reads
        // one operand at a time, is not asked for any; BSD `head` has no
        // `-q` or `-v` at all.
        let (headers, options) = take_header_flags(options);
        let headed = headers.unwrap_or(operands.len() > 1);
        let mut failed = None;
        for (index, operand) in operands.into_iter().enumerate() {
            if headed {
                if index > 0 {
                    writeln!(context.stdout())?;
                }
                writeln!(context.stdout(), "==> {} <==", operand.word)?;
            }
            let result = if let Some(start) = operand.start {
                match open_at(&operand.path, start) {
                    Ok(file) => {
                        let stdin = OpenFile::from(file);
                        run_program(&mut context, &name, &options, Some(stdin)).await?
                    }
                    Err(error) => {
                        writeln!(context.stderr(), "{name}: {}: {error}", operand.word)?;
                        ExecutionResult::general_error()
                    }
                }
            } else {
                // `--` keeps a path that starts with `-` a path.
                let mut args = options.clone();
                args.push("--".to_owned());
                args.push(operand.word);
                run_program(&mut context, &name, &args, None).await?
            };
            if failed.is_none() && !result.is_success() {
                failed = Some(result);
            }
        }
        Ok(failed.unwrap_or_else(ExecutionResult::success))
    }
}

/// `options` without the header options of `head` and `tail`, and what the
/// last of them asked for: no headers (`-q`, `--quiet`, `--silent`), headers
/// always (`-v`, `--verbose`), or `None` when none was given, as GNU `head`
/// and `tail` let the last win. A group of short options, such as `-qn3`, is
/// read letter by letter up to `n` or `c`, whose value is the rest of the
/// group.
fn take_header_flags(options: Vec<String>) -> (Option<bool>, Vec<String>) {
    let mut headers = None;
    let mut kept = Vec::new();
    for option in options {
        match option.as_str() {
            "--quiet" | "--silent" => headers = Some(false),
            "--verbose" => headers = Some(true),
            word if word.starts_with("--") || !word.starts_with('-') => kept.push(option),
            word => {
                let mut rest = String::from("-");
                let mut letters = word.chars().skip(1);
                while let Some(letter) = letters.next() {
                    match letter {
                        'q' => headers = Some(false),
                        'v' => headers = Some(true),
                        'n' | 'c' => {
                            rest.push(letter);
                            rest.extend(letters.by_ref());
                        }
                        other => rest.push(other),
                    }
                }
                if rest != "-" {
                    kept.push(rest);
                }
            }
        }
    }
    (headers, kept)
}

/// One path operand: the word as given, its absolute path, and the first
/// byte to read when the window answers for it.
struct Operand {
    word: String,
    path: PathBuf,
    start: Option<u64>,
}

/// Runs `cat` once with `options`, its standard input a pipe that a thread
/// fills with every operand in turn, each windowed one from its first byte;
/// an operand that cannot be read is reported after the program ends.
async fn cat(
    context: &mut shrub_core::ExecutionContext<'_, impl shrub_core::ShellExtensions>,
    options: &[String],
    operands: Vec<Operand>,
) -> Result<ExecutionResult, shrub_core::Error> {
    let (reader, mut writer) = io::pipe()?;
    // `-` names the builtin's own standard input, as it does for `cat`.
    let mut stdin = context.stdin();
    let feeder = std::thread::spawn(move || {
        let mut errors = Vec::new();
        for operand in operands {
            let copied = if operand.word == "-" && operand.start.is_none() {
                io::copy(&mut stdin, &mut writer)
            } else {
                open_at(&operand.path, operand.start.unwrap_or(0))
                    .and_then(|mut file| io::copy(&mut file, &mut writer))
            };
            match copied {
                Ok(_) => {}
                // The program ended first and closed the pipe: nothing more
                // will be read.
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => break,
                Err(error) => errors.push(format!("cat: {}: {error}", operand.word)),
            }
        }
        errors
    });
    let result = run_program(context, "cat", options, Some(OpenFile::from(reader))).await?;
    // The program has read the pipe to its end or closed it, so the thread
    // has finished or ends at its next write.
    let errors = feeder
        .join()
        .unwrap_or_else(|_| vec!["cat: the reader of the operands stopped".to_owned()]);
    for error in &errors {
        writeln!(context.stderr(), "{error}")?;
    }
    if result.is_success() && !errors.is_empty() {
        return Ok(ExecutionResult::general_error());
    }
    Ok(result)
}

/// The file at `path`, opened for reading at byte `start`.
fn open_at(path: &Path, start: u64) -> io::Result<std::fs::File> {
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(options: &[&str]) -> Vec<String> {
        options.iter().map(|option| (*option).to_owned()).collect()
    }

    #[test]
    fn header_options_are_taken_out_of_groups_and_long_forms() {
        assert_eq!(
            take_header_flags(words(&["-qn3"])),
            (Some(false), words(&["-n3"]))
        );
        assert_eq!(
            take_header_flags(words(&["-v", "-n", "5"])),
            (Some(true), words(&["-n", "5"]))
        );
        assert_eq!(
            take_header_flags(words(&["--silent", "-c10", "-q"])),
            (Some(false), words(&["-c10"]))
        );
        assert_eq!(
            take_header_flags(words(&["-n", "-5"])),
            (None, words(&["-n", "-5"]))
        );
    }

    #[test]
    fn the_last_header_option_wins() {
        assert_eq!(take_header_flags(words(&["-q", "-v"])).0, Some(true));
        assert_eq!(take_header_flags(words(&["-v", "--quiet"])).0, Some(false));
        assert_eq!(take_header_flags(words(&["-vq"])).0, Some(false));
    }
}
