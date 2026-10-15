# How to run tests

## Using xtask (Recommended)

The project provides `cargo xtask` commands for running tests:

```bash
# Run unit tests (fast tests excluding integration binaries)
cargo xtask test unit

# Run integration tests (all workspace tests including compat tests)
cargo xtask test integration

# Run tests with code coverage
cargo xtask test integration --coverage --coverage-output codecov.xml
```

## Repository gate

`mise run verify` runs every check CI runs, the tests included.

## Manual Approach (Alternate)

To run all workspace tests:

```bash
cargo test --workspace
```

To run just bash compatibility tests:

```bash
cargo test --test shrub-compat-tests
```

To run a specific compatibility test case

```bash
cargo test --test shrub-compat-tests -- '<name of test case>'
```
