---
shrub: minor
---

`RuntimeOptions` gains `max_kept_descriptors`, the most file descriptors above standard error a shell keeps across commands, and `CreateOptions::max_kept_descriptors` sets it; left unset, as in `CreateOptions::default()`, it is `options::DEFAULT_MAX_KEPT_DESCRIPTORS` (64).
An `exec` redirection or a `coproc` that would keep more fails, and the shell keeps the descriptors it had, so a script that opens descriptors in a loop cannot exhaust the process that embeds the shell.
A change that does not grow the set is allowed even above the cap, so a script can still close descriptors; `None` keeps any number, as before.
The cap is a shell option, so pipeline stages and subshells, which run on copies of the shell, carry it.
Breaking: `Shell::replace_open_files` now returns a `Result`, and `ErrorKind` gains `TooManyKeptDescriptors`, which a `match` over every variant must handle.
