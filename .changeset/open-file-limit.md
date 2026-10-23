---
shrub: minor
---

`SpawnOptions` gains `open_file_limit`, the soft `RLIMIT_NOFILE` each external process gets, set after the fork and before its program runs, so an embedder that raised its own soft limit can give its children the one it started with.
A value above the hard limit sets the hard limit; `None`, the default, leaves the inherited limit, and on a platform without resource limits setting it fails every external command.
