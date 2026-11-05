---
shrub: patch
---

The experimental listing and read-window builtins follow their programs more closely:

- `ls` with several operands lists each in the order given under its own header, and passes `--` before a path it hands to the `ls` program; GNU's `-I`, `-T` and `-w` keep their values.
- `tree` marks a directory it cannot read where it stands and goes on, does not descend into a lister's entry whose name is not one path component, and walks on a thread of its own through `ShellTasks::spawn_blocking`, now public in `shrub-core`.
- `cat` with a windowed operand reads every operand from one stream, so `-n` and `-s` span files; `-n` is not taken as taking a value; `head` and `tail` honour `-q` and `-v`, and pass `--` before a path.
