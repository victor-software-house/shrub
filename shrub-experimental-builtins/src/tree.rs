use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;
use shrub_core::listing::{DirectoryLister, ListedEntry};
use shrub_core::{ExecutionResult, builtins};

/// (*EXPERIMENTAL*) Prints a directory tree. A directory the embedder lists
/// itself shows each entry's line, as the embedder writes it, and its
/// subdirectories are asked of the embedder in turn; any other directory is
/// read from the file system, names only, without hidden entries unless
/// `-a` is given. A directory that cannot be read is marked where it stands
/// and the walk goes on; the walk runs on a thread of its own, so a large
/// tree does not hold up the shell's other tasks.
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

/// How many directories and files a tree printed, for its last line, and
/// how many directories it could not read.
#[derive(Default)]
struct Counts {
    dirs: u64,
    files: u64,
    errors: u64,
}

impl builtins::Command for TreeCommand {
    type Error = shrub_core::Error;

    async fn execute<SE: shrub_core::ShellExtensions>(
        &self,
        context: shrub_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let operands = if self.paths.is_empty() {
            vec![".".to_owned()]
        } else {
            self.paths.clone()
        };
        let roots: Vec<(String, PathBuf)> = operands
            .into_iter()
            .map(|operand| {
                let root = context.shell.absolute_path(&operand);
                (operand, root)
            })
            .collect();
        let walk = Walk {
            lister: context.shell.options().directory_lister.clone(),
            all: self.all,
            level: self.level,
        };
        let mut out = context.stdout();
        let mut err = context.stderr();
        let task = context
            .shell
            .tasks()
            .spawn_blocking(async move { walk.print_roots(&roots, &mut out, &mut err) })
            .await;
        let failed = task.await??;
        Ok(if failed {
            ExecutionResult::general_error()
        } else {
            ExecutionResult::success()
        })
    }
}

/// One tree print's settings.
struct Walk {
    lister: Option<Arc<dyn DirectoryLister>>,
    all: bool,
    level: Option<usize>,
}

impl Walk {
    /// Prints the tree of each root after its operand, then the counts line,
    /// and reports a root that cannot be read to `err`; returns whether any
    /// directory could not be read.
    fn print_roots(
        &self,
        roots: &[(String, PathBuf)],
        out: &mut impl Write,
        err: &mut impl Write,
    ) -> io::Result<bool> {
        let mut counts = Counts::default();
        for (operand, root) in roots {
            writeln!(out, "{operand}")?;
            match self.entries(root) {
                Ok(entries) => self.print(out, root, &entries, "", 1, &mut counts)?,
                Err(error) => {
                    writeln!(err, "tree: {operand}: {error}")?;
                    counts.errors += 1;
                }
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
        Ok(counts.errors > 0)
    }

    /// Prints `entries`, the directory at `dir`'s, each after `prefix` and
    /// its branch, and descends into its subdirectories while `depth` is
    /// within the level; a subdirectory that cannot be read is marked under
    /// its entry. A lister's entry whose name is not one path component is
    /// shown and not descended into, so no walk leaves the tree or loops.
    fn print(
        &self,
        out: &mut impl Write,
        dir: &Path,
        entries: &[ListedEntry],
        prefix: &str,
        depth: usize,
        counts: &mut Counts,
    ) -> io::Result<()> {
        let last = entries.len().saturating_sub(1);
        for (index, entry) in entries.iter().enumerate() {
            let (branch, indent) = if index == last {
                ("└── ", "    ")
            } else {
                ("├── ", "│   ")
            };
            writeln!(out, "{prefix}{branch}{}", entry.line)?;
            if !entry.is_dir {
                counts.files += 1;
                continue;
            }
            counts.dirs += 1;
            if self.level.is_some_and(|level| depth >= level) || !is_component(&entry.name) {
                continue;
            }
            let child = format!("{prefix}{indent}");
            let path = dir.join(&entry.name);
            match self.entries(&path) {
                Ok(children) => self.print(out, &path, &children, &child, depth + 1, counts)?,
                Err(error) => {
                    writeln!(out, "{child}[error opening dir: {error}]")?;
                    counts.errors += 1;
                }
            }
        }
        Ok(())
    }

    /// The entries of `dir`: the lister's when it answers for `dir`,
    /// otherwise the file system's, sorted by name.
    fn entries(&self, dir: &Path) -> io::Result<Vec<ListedEntry>> {
        if let Some(entries) = self.lister.as_ref().and_then(|lister| lister.list(dir)) {
            return Ok(entries);
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
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

/// Whether `name` is one path component: not empty, not `.` or `..`, and
/// with no separator.
fn is_component(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/')
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    /// A lister that answers for `/v` and `/v/d`; `/v/gone` it lists as a
    /// directory but does not answer for, and no such directory exists.
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
                "/w" => Some(vec![
                    entry("gone", true, "gone/"),
                    entry("z.md", false, "z.md"),
                ]),
                "/x" => Some(vec![entry(".", true, "./"), entry("a/b", true, "a/b/")]),
                _ => None,
            }
        }
    }

    /// A walk with `lister`, hidden entries left out, to `level`.
    fn walk(lister: Option<Arc<dyn DirectoryLister>>, level: Option<usize>) -> Walk {
        Walk {
            lister,
            all: false,
            level,
        }
    }

    /// The lines `walk` prints for `root`, and the directories, files and
    /// unreadable directories it counted.
    fn printed(walk: &Walk, root: &Path) -> (Vec<String>, u64, u64, u64) {
        let mut out = Vec::new();
        let mut counts = Counts::default();
        let entries = walk.entries(root).unwrap();
        walk.print(&mut out, root, &entries, "", 1, &mut counts)
            .unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines = text.lines().map(str::to_owned).collect();
        (lines, counts.dirs, counts.files, counts.errors)
    }

    #[test]
    fn a_listed_directory_prints_its_lines_and_descends_through_the_lister() {
        let (lines, dirs, files, _) = printed(&walk(Some(Arc::new(Fake)), None), Path::new("/v"));
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
        let (lines, ..) = printed(&walk(Some(Arc::new(Fake)), Some(1)), Path::new("/v"));
        assert_eq!(lines, ["├── d/  a directory", "└── f.md  a file"]);
    }

    #[test]
    fn a_file_system_directory_prints_names_sorted_without_hidden_entries() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("b")).unwrap();
        fs::write(temp.path().join("b/c"), "").unwrap();
        fs::write(temp.path().join("a"), "").unwrap();
        fs::write(temp.path().join(".hidden"), "").unwrap();
        let root: PathBuf = temp.path().to_owned();
        let (lines, dirs, files, _) = printed(&walk(None, None), &root);
        assert_eq!(lines, ["├── a", "└── b", "    └── c"]);
        assert_eq!((dirs, files), (1, 2));
    }

    #[test]
    fn a_directory_that_cannot_be_read_is_marked_and_the_walk_goes_on() {
        let (lines, dirs, files, errors) =
            printed(&walk(Some(Arc::new(Fake)), None), Path::new("/w"));
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[0], "├── gone/");
        assert!(
            lines[1].starts_with("│   [error opening dir: "),
            "{lines:?}"
        );
        assert_eq!(lines[2], "└── z.md");
        assert_eq!((dirs, files, errors), (1, 1, 1));
    }

    #[test]
    fn a_listed_name_that_is_not_one_component_is_not_descended_into() {
        let (lines, dirs, _, errors) = printed(&walk(Some(Arc::new(Fake)), None), Path::new("/x"));
        assert_eq!(lines, ["├── ./", "└── a/b/"]);
        assert_eq!((dirs, errors), (2, 0));
    }
}
