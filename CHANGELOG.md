# Changelog

## shrub 0.6.1

- On macOS, an external command with no descriptor beyond standard input, output and error no longer registers an empty descriptor mapping, so when no other step needs to run between the fork and exec, the standard library starts it with `posix_spawn` instead of fork and exec ([#71](https://github.com/victor-software-house/shrub/pull/71)).

## shrub 0.6.0

- `SpawnOptions::process_limit` sets each external command's soft and hard `RLIMIT_NPROC` after its fork and before its program runs, or keeps its hard limit when that is lower, so the kernel refuses a fork by the command or its descendants once the user id holds that many processes; root and a process with `CAP_SYS_RESOURCE` or `CAP_SYS_ADMIN` are not bound.
Only Linux sets it; on another platform, setting it fails every external command.
The `exec` builtin refuses to replace the shell while it is set, since a failed exec would leave the shell under a hard limit it cannot raise.
Breaking: `SpawnOptions` gains a public field, so a struct expression that names every field must add `process_limit`; one that ends in `..SpawnOptions::default()` is unchanged ([#61](https://github.com/victor-software-house/shrub/pull/61)).

## shrub 0.5.0

- `SpawnOptions::before_spawn` is called before each external command is spawned, and the shell holds the `SpawnGuard` it returns until `on_spawn` has returned, or until the spawn fails.
An embedder that reaps processes it did not spawn, such as a child subreaper, holds a lock in the guard, so it never reaps a child that exited before `on_spawn` told it the child is the shell's.
Breaking: `SpawnOptions` gains a public field, so a struct expression that names every field must add `before_spawn`; one that ends in `..SpawnOptions::default()` is unchanged ([#59](https://github.com/victor-software-house/shrub/pull/59)).

## shrub 0.4.0

- `Shell::fork_state` returns a copy of the shell's state at the same level, not as a subshell: the copy keeps the shell's depth, so `$BASH_SUBSHELL` is unchanged and `exec` replaces the process instead of running as `command`.
Like `Clone::clone`, the copy starts with an empty job table, so a job it starts with `&` belongs to the copy alone.
An embedder can run a command in the copy and then keep it in place of the shell or drop it.
The `fork-state-cost` example of `shrub-shell` times `fork_state` and `clone` on a shell with 200 variables, 50 functions and a directory stack, next to one run of the external `true` program, found on `PATH` ([#49](https://github.com/victor-software-house/shrub/pull/49)).
- Each module with unsafe code now says why safe code cannot do its job, and each `unsafe` block's `SAFETY:` comment names the callee's preconditions and how they are met.
A child that takes the foreground no longer calls `std::io::stdin()` between the fork and `exec`, whose first call allocates; it calls `tcsetpgrp` on descriptor 0, and still ignores its failure.
A child that leads a session builds its `setsid` and `TIOCSCTTY` errors from the errno instead of allocating a message, so the spawn error is the OS error alone.
`sys::fd::try_get_file_for_open_fd` and `try_iter_open_fds` duplicate a descriptor number with `fcntl(F_DUPFD_CLOEXEC)` instead of borrowing a descriptor that may have closed ([#51](https://github.com/victor-software-house/shrub/pull/51)).

## shrub 0.3.1

- The lockfile moves tokio from 1.53.2 to 1.53.3, the fixed release for [RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386), an unsound `AsyncFd` constructor.
shrub does not use `AsyncFd`; the update clears the advisory for builds that use the lockfile, such as `cargo install --locked` ([#54](https://github.com/victor-software-house/shrub/pull/54)).

## shrub 0.3.0

- `RuntimeOptions` gains `max_kept_descriptors`, the most file descriptors above standard error a shell keeps across commands, and `CreateOptions::max_kept_descriptors` sets it; left unset, as in `CreateOptions::default()`, it is `options::DEFAULT_MAX_KEPT_DESCRIPTORS` (64).
An `exec` redirection or a `coproc` that would keep more fails, and the shell keeps the descriptors it had, so a script that opens descriptors in a loop cannot exhaust the process that embeds the shell.
A change that does not grow the set is allowed even above the cap, so a script can still close descriptors; `None` keeps any number, as before.
The cap is a shell option, so pipeline stages and subshells, which run on copies of the shell, carry it.
Breaking: `Shell::replace_open_files` now returns a `Result`, and `ErrorKind` gains `TooManyKeptDescriptors`, which a `match` over every variant must handle ([#40](https://github.com/victor-software-house/shrub/pull/40)).
- `SpawnOptions` gains `open_file_limit`, the soft `RLIMIT_NOFILE` each external process gets, set after the fork and before its program runs, so an embedder that raised its own soft limit can give its children the one it started with.
A value above the hard limit sets the hard limit; `None`, the default, leaves the inherited limit, and on a platform without resource limits setting it fails every external command ([#41](https://github.com/victor-software-house/shrub/pull/41)).
- `SpawnOptions` gains `pids_limits`, a list of `options::PidsLimit` (an open `pids.current` file and the highest count allowed) that each external process checks after it joins `cgroup_procs` and before its program runs.
The kernel does not apply `pids.max` to a process that moves into a cgroup, so a child whose count is above a limit, or cannot be read, fails its spawn without running its program: the command fails to execute, with status 126, and `on_spawn` is not called.
The check reads only descriptors opened before the fork, allocates nothing, and returns an error instead of exiting, and the `exec` builtin refuses to replace the shell while the list is non-empty, with status 126 and before any step runs, because in the shell's own process a count would include every thread of the shell; an empty list, the default, checks nothing, and on a platform without the check a non-empty list fails every external command ([#42](https://github.com/victor-software-house/shrub/pull/42)).

## shrub 0.2.0

- `Shell::snapshot` copies the state a command can change into a `snapshot::ShellSnapshot`, and `Shell::restore_snapshot` applies one to a fresh shell, so an embedder can carry a shell across a process restart or an upgrade.
A snapshot holds the working directory, global variables with their attributes, functions, aliases, traps, `set -o` and `shopt` options, disabled builtins, the directory stack, the positional parameters and `$?`; it names the background jobs, file descriptors above 2 and functions it could not copy.
It is plain data with no serialization of its own, and `SnapshotOptions` leaves out the variables an embedder sets itself and any names it excludes.
Restoring runs nothing as a command: every item is applied through the shell's API, and an item that does not apply is reported by name, never by value.
`ShellVariableUpdateTransform` and `ShellValueUnsetType` now implement `PartialEq` and `Eq`, and `ShellValueUnsetType` implements `Copy`.
Breaking: `ErrorKind` gains `NotBetweenCommands`, which a `match` over every variant must handle ([#36](https://github.com/victor-software-house/shrub/pull/36)).

## shrub 0.1.0

- `RuntimeOptions` gains `spawn: SpawnOptions`, which an embedder uses to see each external process the shell starts (`on_spawn`, called with its pid and process group) and to move each one into a cgroup v2 before its program runs (`cgroup_procs`, a `cgroup.procs` path the child writes `0` to).
A child that cannot write that file fails to execute, so no command runs outside its cgroup; on a platform without cgroups, setting the path fails every external command.
An external command or `exec` whose cgroup file is gone fails with an error that names the file, not as a missing program; `commands::missing_cgroup` makes that check.
The default does neither, so existing callers behave as before ([#33](https://github.com/victor-software-house/shrub/pull/33)).

## shrub 0.0.4

- The shared verify runs verctl's version guard: ctl-core's Rust tasks at 0.6.27 and verctl at 0.5.3, so a manifest version changes only on the Version PR ([#26](https://github.com/victor-software-house/shrub/pull/26)).

## shrub 0.0.3

- A brace sequence with an increment too large for a 64-bit integer, such as `echo {1..3..99999999999999999999}`, no longer panics and stays literal text, as in bash, and a very large increment that does fit no longer overflows or loops forever while the sequence is expanded ([#23](https://github.com/victor-software-house/shrub/pull/23)).
- Every fallible or panicking public function now documents when it errors or panics ([#14](https://github.com/victor-software-house/shrub/pull/14)).

## shrub 0.0.2

- Arithmetic takes `++` or `--` before anything but a variable as two signs, as bash does, so `$((++0))` is 0 instead of a parse error ([#9](https://github.com/victor-software-house/shrub/pull/9)).
- Every library crate has a docs.rs landing page, and docs.rs builds each crate with all features enabled ([#15](https://github.com/victor-software-house/shrub/pull/15)).
- First release of shrub, a hard fork of brush: every crate is published under the shrub name ([#2](https://github.com/victor-software-house/shrub/pull/2)).
- The `shrub` crate now holds the only `shrub` binary and passes through every shrub-shell feature, so install with `cargo install shrub`. shrub-shell is now a library; its command line and entry point moved to `shrub` ([#11](https://github.com/victor-software-house/shrub/pull/11)).

