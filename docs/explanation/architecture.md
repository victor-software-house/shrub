# Architecture

This describes how shrub is split into crates, and the principles that decide which crate
a piece of code belongs in. When a change raises the question "where should this go?",
the answer should follow from here. If it doesn't, this document needs updating.

## Crates

| Crate | Published | Role |
|---|---|---|
| `shrub-parser` | yes | Tokenizes and parses shell syntax. Knows nothing about a running shell. |
| `shrub-process` | yes | Process launch: one spawn request, and the child setup between `fork` and `exec` (cgroup join, resource limits, process counts, process group, terminal) behind safe functions. Knows nothing about a shell, so a program can launch processes as shrub does without the interpreter. |
| `shrub-core` | yes | The shell itself: the `Shell` type, expansion, execution, jobs, variables, traps, and programmable completion. Has no line editor or UI of its own. |
| `shrub-builtins` | yes | The standard builtins, registered on a `Shell` through `shrub-core`'s public API. |
| `shrub-experimental-builtins` | yes | Builtins not ready to be standard. |
| `shrub-coreutils-builtins` | yes | Optional builtins wrapping [uutils/coreutils](https://github.com/uutils/coreutils); doesn't depend on the other crates. |
| `shrub-interactive` | yes | The interactive front end: reads input, edits the line (with reedline or a basic line reader), and presents prompts, completions, suggestions, and highlighting. |
| `shrub-shell` | yes | The library behind the `shrub` binary: its command line, and the wiring of the crates above into a shell. |
| `shrub` | yes | The `shrub` binary, a thin wrapper around `shrub-shell`, and its end-to-end tests. |
| `shrub-test-harness` | no | Runs the YAML test cases that compare shrub with bash. |
| `xtask`, `fuzz` | no | Development tooling. |

Their dependencies run one way, from front end to parser. Each crate's direct dependencies
on the others (not counting dev-dependencies) are:

```text
shrub                        → shrub-shell
shrub-shell                  → shrub-interactive, shrub-builtins, shrub-core,
                               shrub-experimental-builtins (optional),
                               shrub-coreutils-builtins (optional)
shrub-interactive            → shrub-core, shrub-parser
shrub-builtins               → shrub-core, shrub-parser, shrub-process (with `exec`)
shrub-experimental-builtins  → shrub-core
shrub-core                   → shrub-parser, shrub-process
shrub-parser                 (none)
shrub-process                (none)
shrub-coreutils-builtins     (none)
```

Anything a published crate exports is public API, including to people who embed shrub
without its front end. Keep that in mind when deciding what to export.

## Structure within `shrub-core`

- Shells are built with `Shell::builder()`, a type-safe builder.
- `Shell` is generic over `ShellExtensions`, which lets embedders swap in parts of its
  behavior (today, how errors are formatted) at compile time.
- Platform-specific code lives under the `sys` module, so the rest of the crate stays
  platform-neutral.
- Every external command, and `exec`, is a `shrub_process::Spawn` composed by
  `commands::compose_spawn`; the code that runs in a child before its program lives in
  `shrub-process`, not here.
- The `snapshot` module holds a copy of a shell's state as plain data:
  `Shell::snapshot` takes one between commands, and `Shell::restore_snapshot` applies
  it to a fresh shell through the public API, so an embedder can carry a shell across
  a process restart or an upgrade and store the copy in its own format.

## Layering principles

These decide where a behavior belongs when more than one crate could plausibly own it.

1. **Meaning vs. presentation.** If getting it wrong changes what the shell does -- what
   command a line runs, or what a script sees -- it generally belongs in `shrub-core`.
   If it only changes what the user sees, or how many keystrokes something takes, it
   belongs to the front end. For example, core composes a prompt from `PS1`, and the
   front end draws it. A front end presents what core produces and chooses among it,
   but doesn't recreate shell semantics on its own.

   Shell semantics that a front end needs are exposed as core APIs rather than
   reimplemented. Helpers that only make sense inside one of core's own operations stay
   private.

2. **Layers own their state.** A feature layered above the interpreter should reach
   `Shell` only through public APIs, and keep its own state. Until `Shell` can hold state
   owned by other crates, such features live in `shrub-core`; don't add new coupling
   between them and the rest of core.
