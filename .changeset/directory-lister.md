---
shrub: minor
---

A program that embeds shrub can list some directories itself.
`shrub_core::listing::DirectoryLister` answers for the directories it knows, one `ListedEntry` per entry with the line a listing shows, and the embedder sets it in `RuntimeOptions::directory_lister`.
Two experimental builtins ask it first, each behind its own feature of `shrub-experimental-builtins`, off by default:

- `ls` (`builtin.ls`) prints the lister's lines for a directory it answers for, and runs the `ls` program in `PATH` with the same arguments for any other path.
- `tree` (`builtin.tree`, with `-a` and `-L`) prints a directory tree, descending through the lister where it answers and through the file system elsewhere.
