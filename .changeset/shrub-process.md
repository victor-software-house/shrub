---
shrub: minor
---

A new crate, `shrub-process`, launches processes for shrub and for programs that embed it: one spawn request, `Spawn`, started by `spawn` or `exec`, with the child setup that runs between the fork and exec (the cgroup join, the open-file and process limits, the process counts, the descriptor map, the process group and the terminal) behind safe functions.
`shrub-core` now runs every external command through it, so a program that embeds shrub launches its own processes exactly as the shell does, without the interpreter and without unsafe code of its own.

Breaking changes in `shrub-core`, each caller updated here:

- `commands::compose_std_command`, which returned a `std::process::Command`, is now `commands::compose_spawn`, which returns a `shrub_process::Spawn`. Its callers, the external command path in `shrub-core` and the `exec` builtin in `shrub-builtins`, use it.
- `SpawnOptions`, `PidsLimit`, `SpawnedProcess` and `SpawnGuard` are defined in `shrub-process`; `shrub_core::options` re-exports them under the same names.
- The `sys::commands` extension traits `CommandFdInjectionExt`, `CommandFgControlExt`, `CommandCgroupExt`, `CommandOpenFileLimitExt`, `CommandProcessLimitExt` and `CommandPidsLimitsExt` are gone; their steps are `shrub-process`'s. `commands::process_error` maps a `shrub_process::Error` to a shell error.
- A setup step the platform cannot take now fails the spawn, in `shrub_process::spawn` or `exec`, rather than the composition.
