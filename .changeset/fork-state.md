---
shrub: minor
---

`Shell::fork_state` returns a copy of the shell's state at the same level, not as a subshell: the copy keeps the shell's depth, so `$BASH_SUBSHELL` is unchanged and `exec` replaces the process instead of running as `command`.
Like `Clone::clone`, the copy starts with an empty job table, so a job it starts with `&` belongs to the copy alone.
An embedder can run a command in the copy and then keep it in place of the shell or drop it.
The `fork-state-cost` example of `shrub-shell` times `fork_state` and `clone` on a shell with 200 variables, 50 functions and a directory stack, next to one run of the external `true` program, found on `PATH`.
