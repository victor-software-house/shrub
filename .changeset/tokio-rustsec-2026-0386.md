---
shrub: patch
---

The lockfile moves tokio from 1.53.2 to 1.53.3, the fixed release for [RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386), an unsound `AsyncFd` constructor.
shrub does not use `AsyncFd`; the update clears the advisory for builds that use the lockfile, such as `cargo install --locked`.
