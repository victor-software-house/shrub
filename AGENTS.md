# shrub development guide

shrub is a Bash-compatible shell in Rust, built as crates that other programs embed.
It is a hard fork of brush; the README holds the attribution.

## Architecture

[docs/explanation/architecture.md](docs/explanation/architecture.md) defines the crates, how they depend on each other, and which crate owns which code.
Read it before adding a crate, moving code between crates, or changing the shape of a public API, and keep it current.

```text
shrub-shell → shrub-interactive → shrub-core → shrub-parser
            ↘ shrub-builtins ↗
```

- `shrub-core/src/shell.rs` holds the `Shell` struct; shells are created with `Shell::builder()`.
- `shrub-core/src/lib.rs` holds the public exports.
- `shrub/src/main.rs` is the only binary; it calls the CLI entry point, `shrub_shell::entry::run`.
- Platform-specific code in `shrub-core` lives under its `sys` module; `check:sys-boundary` flags target cfgs elsewhere, and a deliberate exception carries `// ast-grep-ignore: platform-cfg-outside-sys`.

## Tasks

mise pins every tool, and the tasks are the only supported commands.

| Task | What it runs |
|:--|:--|
| `mise run verify` | Toolchain pins, format, clippy, feature checks, tests, doc-tests, cargo-deny, unused crates, repository checks |
| `mise run format` | rustfmt check on the pinned nightly |
| `mise run test` | Unit, integration and compatibility tests with the default features |
| `mise run check:repo` | TOML format, Markdown, spelling, workflow lint, and `check:local` |
| `mise run check:local` | The sys-boundary rule (ast-grep) and the workflow audit (zizmor) |

`mise.toml` composes `verify` from the shared Rust tasks of the public actions repository, included by `git::` URL at the commit the template pins; `check:features` is the template's starter `mise-tasks/check/features.sh`, and `check:local` is `mise-tasks/check/local.sh`, which runs `check:sys-boundary` and `check:workflows`.
`ci.yml` and `template.yml` call the actions repository's shared workflows, reported as `verify / Verify` and `template / Template`; the drift check fails only a pull request that changes `.copier-answers.yml` without reaching the newest tag, and a push to `main` or the daily run keeps a "Template update available" issue open while a newer tag would change a file. Verify's 30-minute limit comes from the `verify_timeout` answer, because a cold build takes 15 to 21 minutes.
`test` overrides the shared one in `mise.toml`, because the compatibility suite and shrub-parser's stub `winnow-parser` feature need the default features.
CI runs `verify` on x86_64 Linux, and `.github/workflows/test.yml` runs `test` on macOS and arm64 Linux.
lefthook runs `format` before a commit, and lints the pushed commits and scans for credentials before a push; the pull request's CI runs `verify`.
`cargo xtask` stays for the test runner and generated docs; see [docs/how-to/run-tests.md](docs/how-to/run-tests.md).

## Tests

- Unit tests live in `src/` under `#[cfg(test)]`; integration tests in `tests/`.
- Compatibility cases are YAML files under `shrub/tests/cases/`, run against Bash.
  A compatibility fix adds a case there.
- Examples in `examples/` must run with `cargo run --package <crate> --example <name>`.
- `schemas/config.schema.json` is generated from `shrub-shell`'s `Config`; a unit test under the `schema` feature fails when it is stale, and `cargo xtask gen schema config --out schemas/config.schema.json` regenerates it.

## Public API

Every crate is published, so everything a crate exports is public API.
A breaking change is allowed but called out in its changeset.
A new field on a public struct is fine when the struct implements `Default` with a sensible value.
When `shrub-core`'s API changes, check the construction sites in `shrub-shell` and `shrub-interactive`.

Exported items carry rustdoc; internal docs are best effort.

## Code conventions

- Errors use `thiserror`; `anyhow` only in tests.
- Debug logging uses `tracing` with the targets in each crate's `trace_categories.rs`, for example `tracing::debug!(target: trace_categories::JOBS, "...")`.
- Avoid clones unless async ownership needs a separate copy.

## Releases

A change to a published crate adds a `.changeset/<slug>.md` fragment naming `shrub`; [docs/how-to/release.md](docs/how-to/release.md) describes the verctl flow.
Commits follow Conventional Commits, checked by `committed`.
