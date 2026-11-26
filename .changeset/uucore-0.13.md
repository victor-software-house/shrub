---
shrub: patch
---

The coreutils builtins and `shrub-builtins` move to uutils 0.13.0.
uucore 0.13.0 depends on procfs without its default features, so `chrono`, which RUSTSEC-2026-0543 marks unmaintained, is no longer a dependency on Linux.
