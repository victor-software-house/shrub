---
shrub: minor
---

`SpawnOptions::before_spawn` is called before each external command is spawned, and the shell holds the `SpawnGuard` it returns until `on_spawn` has returned, or until the spawn fails.
An embedder that reaps processes it did not spawn, such as a child subreaper, holds a lock in the guard, so it never reaps a child that exited before `on_spawn` told it the child is the shell's.
Breaking: `SpawnOptions` gains a public field, so a struct expression that names every field must add `before_spawn`; one that ends in `..SpawnOptions::default()` is unchanged.
