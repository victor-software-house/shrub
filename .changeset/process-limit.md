---
shrub: minor
---

`SpawnOptions::process_limit` sets each external command's soft and hard `RLIMIT_NPROC` after its fork and before its program runs, or keeps its hard limit when that is lower, so the kernel refuses a fork by the command or its descendants once the user id holds that many processes; root and a process with `CAP_SYS_RESOURCE` or `CAP_SYS_ADMIN` are not bound.
Only Linux sets it; on another platform, setting it fails every external command.
The `exec` builtin refuses to replace the shell while it is set, since a failed exec would leave the shell under a hard limit it cannot raise.
Breaking: `SpawnOptions` gains a public field, so a struct expression that names every field must add `process_limit`; one that ends in `..SpawnOptions::default()` is unchanged.
