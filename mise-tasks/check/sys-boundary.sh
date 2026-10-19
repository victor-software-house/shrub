#!/usr/bin/env bash
#MISE description="Platform cfgs in shrub-core stay under sys"
# shrub-core keeps platform code under its sys module; the rule flags target
# cfgs anywhere else in the crate. `ast-grep test` proves the rule against its
# own cases first.
set -euo pipefail
ast-grep test --skip-snapshot-tests
ast-grep scan
