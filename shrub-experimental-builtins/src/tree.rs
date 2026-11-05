use std::io::{self, Write};
use std::path::Path;

use clap::Parser;
use shrub_core::listing::{DirectoryLister, ListedEntry};
use shrub_core::{ExecutionResult, builtins};

/// (*EXPERIMENTAL*) Prints a directory tree. A directory the embedder lists
/// itself shows each entry's line, as the embedder writes it, and its
/// subdirectories are asked of the embedder in turn; any other directory is
/// read from the file system, names only, without hidden entries unless
/// `-a` is given.
#[derive(Parser)]
pub(crate) struct TreeCommand {
    /// Show hidden entries of file-system directories.
    #[arg(short = 'a')]
    all: bool,

    /// Descend at most this many levels.
    #[arg(short = 'L', value_name = "LEVEL")]
    level: Option<usize>,

    /// The directories to print; the working directory when none is given.
    paths: Vec<String>,
}

/// How many directories and files a tree printed, for its last line.
#[derive(Default)]
struct Counts {
    dirs: u64,
    files: u64,
}

impl builtins::Command for TreeCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let lister = context.shell.options().directory_lister.clone();
        let operands = if self.paths.is_empty() {
            vec![".".to_owned()]
        } else {
            self.paths.clone()
        };
        let mut counts = Counts::default();
        let mut failed = false;
        let mut out = context.stdout();
        for operand in &operands {
            let root = context.shell.absolute_path(operand);
            writeln!(out, "{operand}")?;
            let walk = Walk {
                lister: lister.as_deref(),
                all: self.all,
                level: self.level,
            };
            if let Err(error) = walk.print(&mut out, &root, "", 1, &mut counts) {
                writeln!(context.stderr(), "tree: {operand}: {error}")?;
                failed = true;
            }
        }
        writeln!(out)?;
        writeln!(
            out,
            "{} {}, {} {}",
            counts.dirs,
            if counts.dirs == 1 {
                "directory"
            } else {
                "directories"
            },
            counts.files,
            if counts.files == 1 { "file" } else { "files" },
        )?;
        Ok(if failed {
            ExecutionResult::general_error()
        } else {
            ExecutionResult::success()
        })
    }
}

/// One tree print's settings.
struct Walk<'a> {
    lister: Option<&'a dyn DirectoryLister>,
    all: bool,
    level: Option<usize>,
}

impl Walk<'_> {
    /// Prints the entries of the directory at `dir`, each after `prefix` and
    /// its branch, and descends into its subdirectories while `depth` is
    /// within the level.
    fn print(
        &self,
        out: &mut impl Write,
        dir: &Path,
        prefix: &str,
        depth: usize,
        counts: &mut Counts,
    ) -> io::Result<()> {
        if self.level.is_some_and(|level| depth > level) {
            return Ok(());
        }
        let entries = self.entries(dir)?;
        let last = entries.len().saturating_sub(1);
        for (index, entry) in entries.iter().enumerate() {
            let (branch, indent) = if index == last {
                ("└── ", "    ")
            } else {
                ("├── ", "│   ")
            };
            writeln!(out, "{prefix}{branch}{}", entry.line)?;
            if entry.is_dir {
                counts.dirs += 1;
                let child = format!("{prefix}{indent}");
                self.print(out, &dir.join(&entry.name), &child, depth + 1, counts)?;
            } else {
                counts.files += 1;
            }
        }
        Ok(())
    }

    /// The entries of `dir`: the lister's when it answers for `dir`,
    /// otherwise the file system's, sorted by name.
    fn entries(&self, dir: &Path) -> io::Result<Vec<ListedEntry>> {
        if let Some(entries) = self.lister.and_then(|lister| lister.list(dir)) {
            return Ok(entries);
        }
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !self.all && name.starts_with('.') {
                continue;
            }
            let is_dir = entry.file_type()?.is_dir();
            entries.push(ListedEntry {
                line: name.clone(),
                name,
                is_dir,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    /// A lister that answers for `/v` and `/v/d`.
    struct Fake;

    impl DirectoryLister for Fake {
        fn list(&self, path: &Path) -> Option<Vec<ListedEntry>> {
            let entry = |name: &str, is_dir: bool, line: &str| ListedEntry {
                name: name.to_owned(),
                is_dir,
                line: line.to_owned(),
            };
            match path.to_str()? {
                "/v" => Some(vec![
                    entry("d", true, "d/  a directory"),
                    entry("f.md", false, "f.md  a file"),
                ]),
                "/v/d" => Some(vec![entry("g.md", false, "g.md  inside")]),
                _ => None,
            }
        }
    }

    /// The lines `walk` prints for `root`, and the directories and files it
    /// counted.
    fn printed(walk: &Walk<'_>, root: &Path) -> (Vec<String>, u64, u64) {
        let mut out = Vec::new();
        let mut counts = Counts::default();
        walk.print(&mut out, root, "", 1, &mut counts).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines = text.lines().map(str::to_owned).collect();
        (lines, counts.dirs, counts.files)
    }

    #[test]
    fn a_listed_directory_prints_its_lines_and_descends_through_the_lister() {
        let walk = Walk {
            lister: Some(&Fake),
            all: false,
            level: None,
        };
        let (lines, dirs, files) = printed(&walk, Path::new("/v"));
        assert_eq!(
            lines,
            [
                "├── d/  a directory",
                "│   └── g.md  inside",
                "└── f.md  a file"
            ]
        );
        assert_eq!((dirs, files), (1, 2));
    }

    #[test]
    fn the_level_stops_the_descent() {
        let walk = Walk {
            lister: Some(&Fake),
            all: false,
            level: Some(1),
        };
        let (lines, _, _) = printed(&walk, Path::new("/v"));
        assert_eq!(lines, ["├── d/  a directory", "└── f.md  a file"]);
    }

    #[test]
    fn a_file_system_directory_prints_names_sorted_without_hidden_entries() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("b")).unwrap();
        fs::write(temp.path().join("b/c"), "").unwrap();
        fs::write(temp.path().join("a"), "").unwrap();
        fs::write(temp.path().join(".hidden"), "").unwrap();
        let walk = Walk {
            lister: None,
            all: false,
            level: None,
        };
        let root: PathBuf = temp.path().to_owned();
        let (lines, dirs, files) = printed(&walk, &root);
        assert_eq!(lines, ["├── a", "└── b", "    └── c"]);
        assert_eq!((dirs, files), (1, 2));
    }
}
