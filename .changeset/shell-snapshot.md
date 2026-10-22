---
shrub: minor
---

`Shell::snapshot` copies the state a command can change into a `snapshot::ShellSnapshot`, and `Shell::restore_snapshot` applies one to a fresh shell, so an embedder can carry a shell across a process restart or an upgrade.
A snapshot holds the working directory, global variables with their attributes, functions, aliases, traps, `set -o` and `shopt` options, disabled builtins, the directory stack, the positional parameters and `$?`; it names the background jobs, file descriptors above 2 and functions it could not copy.
It is plain data with no serialization of its own, and `SnapshotOptions` leaves out the variables an embedder sets itself and any names it excludes.
Restoring runs nothing as a command: every item is applied through the shell's API, and an item that does not apply is reported by name, never by value.
`ShellVariableUpdateTransform` and `ShellValueUnsetType` now implement `PartialEq` and `Eq`, and `ShellValueUnsetType` implements `Copy`.
Breaking: `ErrorKind` gains `NotBetweenCommands`, which a `match` over every variant must handle.
