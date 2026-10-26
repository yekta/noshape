#!/bin/bash
# T1 core (docs/TESTS.md): every non-ignored test of the workspace, the app crate included. ≤ 10 min on an idle
# 16-core machine once built. Gates main (the lead's watcher) and releases (CI on Linux, macOS, Windows).
# Usage: tools/tiers/t1.sh [extra cargo args, e.g. --locked or -p <crate>]
set -o pipefail
cd "$(dirname "$0")/../.."
export OCCT_ROOT=${OCCT_ROOT:-/root/occt} CARGO_INCREMENTAL=0
exec cargo test --workspace "$@"
