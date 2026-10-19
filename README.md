# shrub

shrub is a Bash-compatible shell written in Rust, built as a set of crates that other programs embed.

shrub is a hard fork of [brush](https://github.com/reubeno/brush) by reuben olinsky, taken from its `main` branch at `0ecc9c5a` on 2026-10-14.
It keeps brush's MIT licence and copyright notice.

## Embed it

Create a shell, run Bash-compatible code in it, and inspect the result:

```rust
let mut shell = shrub_core::Shell::builder().build().await?;

let result = shell
    .run_string(
        r#"greet() { echo "Hello, $1!"; }; greet world"#,
        &shrub_core::SourceInfo::default(),
        &shell.default_exec_params(),
    )
    .await?;

assert!(result.is_success());
```

The [examples guide](docs/reference/examples.md) covers custom builtins, calling shell functions from Rust, and serializing a syntax tree.

## Crates

| Crate | What it provides |
|:--|:--|
| [`shrub-core`](shrub-core) | The shell runtime: expansion, execution, jobs, completion, and the `Shell` API |
| [`shrub-parser`](shrub-parser) | Tokenizer and parser for Bash and POSIX shell syntax, with an optional `serde` syntax tree |
| [`shrub-builtins`](shrub-builtins) | The standard builtins |
| [`shrub-interactive`](shrub-interactive) | Line editing, highlighting, suggestions and completion, built on [reedline](https://github.com/nushell/reedline) |
| [`shrub-shell`](shrub-shell) | The command line of the `shrub` binary |
| [`shrub`](shrub) | The `shrub` binary: `cargo install --locked shrub` |
| [`shrub-coreutils-builtins`](shrub-coreutils-builtins), [`shrub-experimental-builtins`](shrub-experimental-builtins) | Optional builtin sets |

Every crate releases at the same version.

## Tests

More than 2,500 [compatibility cases](shrub/tests/cases) run each script under shrub and Bash and compare stdout, stderr, exit status and file side effects.
[docs/how-to/run-tests.md](docs/how-to/run-tests.md) describes how to run them.

Known gaps: `select`, `wait -n`, `disown`, some traps, and the edge cases listed in the [compatibility reference](docs/reference/compatibility.md).

## Licence

MIT; see [LICENSE](LICENSE).
