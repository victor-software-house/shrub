---
shrub: patch
---

On macOS, an external command with no descriptor beyond standard input, output and error no longer registers an empty descriptor mapping, so when no other step needs to run between the fork and exec, the standard library starts it with `posix_spawn` instead of fork and exec.
