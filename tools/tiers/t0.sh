#!/bin/bash
# T0 smoke (docs/TESTS.md): builds the app and the CLI, opens and regenerates the sample documents (ops/tests/smoke.rs)
# and runs the core agent-API flows of ops/tests/e2e.rs. Tests ≤ 3 min once built. Run before every push.
# Usage: tools/tiers/t0.sh [extra cargo args]
set -eo pipefail
cd "$(dirname "$0")/../.."
export OCCT_ROOT=${OCCT_ROOT:-/root/occt}
# The app reads and writes its settings (toolbars, graphics, shortcuts, recent files) in NOSHAPE_CONFIG_DIR, else
# ~/.config/noshape: a fresh dir per run, so parallel runs (the watcher, two land checks on vibe3) never share them (10-29).
[ -n "${NOSHAPE_CONFIG_DIR:-}" ] || { NOSHAPE_CONFIG_DIR=$(mktemp -d -t noshape-config-XXXXXX); export NOSHAPE_CONFIG_DIR; }
E2E=(
  sketch_extrude_fillet_pattern_export_render assembly_with_mates configurations sketch_trim_extend_split
  sketch_fillet_chamfer_offset sketch_mirror_patterns_transform workspace_protection drawing_views_and_dimensions
  analysis_tools import_step_round_trip materials_and_appearances feature_studio_custom_feature variable_studio_table
  part_studio_menu_ops feature_add_makes_the_dialogs_choices feature_folders_dependencies_and_dynamic_suppression
  agent_benchmark_p1_fixes assembly_patterns_bom_interference sketch_dimension_menu import_every_format
)
SMOKE=(demo_document_regenerates demo_document_with_drawing_regenerates live_30_features_opens_and_regenerates)
# Every tests/*.rs file must be in its crate's tests/all.rs or be a [[test]] of its own (tools/tiers/merge-tests.py).
for f in crates/*/tests/*.rs; do
  c=${f%/tests/*}; s=$(basename "$f" .rs); [ "$s" = all ] && continue
  [ -f "$c/tests/all.rs" ] || continue  # crates without all.rs keep cargo's one binary per file
  grep -qE "^(mod $s;|#\[path = \"$s.rs\"\])" "$c/tests/all.rs" 2>/dev/null || grep -q "path = \"tests/$s.rs\"" "$c/Cargo.toml" \
    || { echo "t0: $f is in no test binary: run tools/tiers/merge-tests.py"; exit 1; }
done
cargo build "$@" -p noshape -p noshape-cli
cargo test "$@" -p noshape-ops --test all -- --exact "${SMOKE[@]/#/smoke::}" "${E2E[@]/#/e2e::}"
