#!/bin/bash
# T1 core (docs/TESTS.md): every non-ignored test of the workspace, the app crate included. ≤ 10 min on an idle
# 16-core machine once built. Gates main (the lead's watcher) and releases (CI on Linux, macOS, Windows).
# Usage: tools/tiers/t1.sh [extra cargo args, e.g. --locked or -p <crate>]
set -o pipefail
cd "$(dirname "$0")/../.."
export OCCT_ROOT=${OCCT_ROOT:-/root/occt}
# The app reads and writes its settings (toolbars, graphics, shortcuts, recent files) in NOSHAPE_CONFIG_DIR, else
# ~/.config/noshape: a fresh dir per run, so parallel runs (the watcher, two land checks on vibe3) never share them (10-29).
[ -n "${NOSHAPE_CONFIG_DIR:-}" ] || { NOSHAPE_CONFIG_DIR=$(mktemp -d -t noshape-config-XXXXXX); export NOSHAPE_CONFIG_DIR; }
exec cargo test --workspace "$@"
