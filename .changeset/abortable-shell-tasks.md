---
shrub: patch
---

`Shell::tasks` returns a handle to every task a shell and its subshells start: background jobs, coprocesses, pipeline stages, and command and process substitutions. `ShellTasks::abort_and_wait` aborts them and every one started later, and completes once each has ended, so an embedder that stops a command stops what the command started in the background too. A copy from `Shell::fork_state` has tasks of its own.
