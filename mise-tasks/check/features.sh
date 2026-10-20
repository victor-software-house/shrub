#!/usr/bin/env bash
#MISE description="Type-check every crate, alone and with all features"
#MISE env={CARGO_TARGET_DIR = "target/features"}
# verify depends on check:features. A starter: the template writes this file
# once and never rewrites it, so the repository keeps its own feature sets
# here.
set -euo pipefail
cargo check --locked --workspace --all-targets
cargo check --locked --workspace --all-targets --all-features
