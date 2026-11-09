//! Integration tests for the `cat`, `head` and `tail` builtins with a
//! `ReadWindow`.

#![cfg(unix)]
#![cfg(test)]
#![cfg(all(
    feature = "builtin.cat",
    feature = "builtin.head",
    feature = "builtin.tail"
))]
#![allow(clippy::panic_in_result_fn)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use shrub_core::window::ReadWindow;
use shrub_experimental_builtins::ShellBuilderExt as _;

/// A window that starts one file at one byte.
struct Fake {
    file: PathBuf,
    start: u64,
}

impl ReadWindow for Fake {
    fn first_byte(&self, path: &Path) -> Option<u64> {
        (path == self.file).then_some(self.start)
    }
}

/// Runs `command` in a shell whose window starts `file` at `start`, with the
/// system programs in `PATH`, and returns the lines it wrote to standard
/// output.
async fn output(file: &Path, start: u64, command: &str) -> Result<Vec<String>> {
    let mut shell = shrub_core::Shell::builder()
        .experimental_builtins()
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .build()
        .await?;
    shell.options_mut().read_window = Some(Arc::new(Fake {
        file: file.to_owned(),
        start,
    }));
    let out = tempfile::NamedTempFile::new()?;
    let script = format!("PATH=/bin:/usr/bin; {command} > '{}'", out.path().display());
    let params = shell.default_exec_params();
    let result = shell
        .run_string(&script, &shrub_core::SourceInfo::default(), &params)
        .await?;
    assert!(result.is_success(), "{command} failed");
    let text = std::fs::read_to_string(out.path())?;
    Ok(text.lines().map(str::to_owned).collect())
}

/// A file in `dir` named `name` holding `text`, and its path as a quoted
/// shell word.
fn file(dir: &Path, name: &str, text: &str) -> Result<(PathBuf, String)> {
    let path = dir.join(name);
    std::fs::write(&path, text)?;
    let word = format!("'{}'", path.display());
    Ok((path, word))
}

#[tokio::test(flavor = "multi_thread")]
async fn cat_starts_a_windowed_file_at_its_first_byte() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (path, word) = file(dir.path(), "out", "dropped\nkept\n")?;
    let lines = output(&path, 8, &format!("cat {word}")).await?;
    assert_eq!(lines, ["kept"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn head_keeps_its_options_on_a_windowed_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (path, word) = file(dir.path(), "out", "gone\none\ntwo\n")?;
    let lines = output(&path, 5, &format!("head -n 1 {word}")).await?;
    assert_eq!(lines, ["one"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn tail_heads_each_file_when_one_of_several_is_windowed() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (path, windowed) = file(dir.path(), "out", "gone\none\ntwo\n")?;
    let (_, plain) = file(dir.path(), "plain", "a\nb\n")?;
    let lines = output(&path, 5, &format!("tail -n 1 {windowed} {plain}")).await?;
    let first = format!("==> {} <==", dir.path().join("out").display());
    let second = format!("==> {} <==", dir.path().join("plain").display());
    assert_eq!(lines, [first.as_str(), "two", "", second.as_str(), "b"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_the_window_does_not_answer_for_reads_whole() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (path, _) = file(dir.path(), "out", "gone\nkept\n")?;
    let (_, plain) = file(dir.path(), "plain", "all\nof it\n")?;
    let lines = output(&path, 5, &format!("cat {plain}")).await?;
    assert_eq!(lines, ["all", "of it"]);
    Ok(())
}
