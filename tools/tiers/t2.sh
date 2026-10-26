#!/bin/bash
# T2 deep (docs/TESTS.md): the tests marked #[ignore = "T2..."]: oracles vs live, kernel robustness, big models,
# long round trips. Nightly on Linux, weekly on macOS/Windows, on demand. Never blocks main or a release:
# each failure becomes a ticket in docs/TICKETS.md for its owner.
# Usage: tools/tiers/t2.sh [extra cargo args]
set -o pipefail
cd "$(dirname "$0")/../.."
export OCCT_ROOT=${OCCT_ROOT:-/root/occt}
PY=$(command -v python3 || command -v python)
mapfile -t names < <("$PY" tools/tiers/t2-names.py . | tr -d "\r")
[ ${#names[@]} -eq 0 ] && { echo "no T2 tests"; exit 0; }
exec cargo test --workspace --no-fail-fast "$@" -- --ignored --exact "${names[@]}"
