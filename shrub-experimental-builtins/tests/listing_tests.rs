//! Integration tests for the `ls` and `tree` builtins with a
//! `DirectoryLister`.

#![cfg(unix)]
#![cfg(test)]
#![cfg(all(feature = "builtin.ls", feature = "builtin.tree"))]
#![allow(clippy::panic_in_result_fn)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use shrub_core::listing::{DirectoryLister, ListedEntry};
use shrub_experimental_builtins::ShellBuilderExt as _;

/// A lister that answers only for one directory, with one file in it.
struct Fake {
    dir: PathBuf,
}

impl DirectoryLister for Fake {
    fn list(&self, path: &Path) -> Option<Vec<ListedEntry>> {
        (path == self.dir).then(|| {
            vec![ListedEntry {
                name: "412.md".to_owned(),
                is_dir: false,
                line: "412.md  user  \"fix the login\"".to_owned(),
            }]
        })
    }
}

/// Runs `command` in a shell whose lister answers for `listed`, and returns
/// the lines it wrote to standard output.
async fn output(listed: &Path, command: &str) -> Result<Vec<String>> {
    let mut shell = shrub_core::Shell::builder()
        .experimental_builtins()
        .do_not_inherit_env(true)
        .skip_well_known_vars(true)
        .build()
        .await?;
    shell.options_mut().directory_lister = Some(Arc::new(Fake {
        dir: listed.to_owned(),
    }));
    let out = tempfile::NamedTempFile::new()?;
    let script = format!("{command} > '{}'", out.path().display());
    let params = shell.default_exec_params();
    let result = shell
        .run_string(&script, &shrub_core::SourceInfo::default(), &params)
        .await?;
    assert!(result.is_success(), "{command} failed");
    let text = std::fs::read_to_string(out.path())?;
    Ok(text.lines().map(str::to_owned).collect())
}

#[tokio::test(flavor = "multi_thread")]
async fn ls_prints_the_listers_lines_for_a_directory_it_answers_for() -> Result<()> {
    let listed = tempfile::tempdir()?;
    let command = format!("ls '{}'", listed.path().display());
    let lines = output(listed.path(), &command).await?;
    assert_eq!(lines, ["412.md  user  \"fix the login\""]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn ls_hands_any_other_directory_to_the_ls_program() -> Result<()> {
    let listed = tempfile::tempdir()?;
    let other = tempfile::tempdir()?;
    std::fs::write(other.path().join("plain.txt"), "")?;
    let command = format!("PATH=/bin:/usr/bin; ls '{}'", other.path().display());
    let lines = output(listed.path(), &command).await?;
    assert_eq!(lines, ["plain.txt"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn tree_prints_the_listers_lines_and_a_count() -> Result<()> {
    let listed = tempfile::tempdir()?;
    let command = format!("tree '{}'", listed.path().display());
    let lines = output(listed.path(), &command).await?;
    let root = listed.path().display().to_string();
    assert_eq!(
        lines,
        [
            root.as_str(),
            "└── 412.md  user  \"fix the login\"",
            "",
            "0 directories, 1 file",
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn ls_lists_mixed_operands_in_order_each_under_its_header() -> Result<()> {
    let listed = tempfile::tempdir()?;
    let other = tempfile::tempdir()?;
    std::fs::write(other.path().join("plain.txt"), "")?;
    let command = format!(
        "PATH=/bin:/usr/bin; ls '{}' '{}'",
        other.path().display(),
        listed.path().display()
    );
    let lines = output(listed.path(), &command).await?;
    let first = format!("{}:", other.path().display());
    let second = format!("{}:", listed.path().display());
    assert_eq!(
        lines,
        [
            first.as_str(),
            "plain.txt",
            "",
            second.as_str(),
            "412.md  user  \"fix the login\""
        ]
    );
    Ok(())
}
