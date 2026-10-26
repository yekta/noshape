#!/bin/bash
# T0 smoke (docs/TESTS.md): builds the app and the CLI, opens and regenerates the sample documents (ops/tests/smoke.rs)
# and runs the core agent-API flows of ops/tests/e2e.rs. Tests ≤ 3 min once built. Run before every push.
# Usage: tools/tiers/t0.sh [extra cargo args]
set -eo pipefail
cd "$(dirname "$0")/../.."
export OCCT_ROOT=${OCCT_ROOT:-/root/occt} CARGO_INCREMENTAL=0
E2E=(
  sketch_extrude_fillet_pattern_export_render assembly_with_mates configurations sketch_trim_extend_split
  sketch_fillet_chamfer_offset sketch_mirror_patterns_transform workspace_protection drawing_views_and_dimensions
  analysis_tools import_step_round_trip materials_and_appearances feature_studio_custom_feature variable_studio_table
  part_studio_menu_ops feature_add_makes_the_dialogs_choices feature_folders_dependencies_and_dynamic_suppression
  agent_benchmark_p1_fixes assembly_patterns_bom_interference sketch_dimension_menu import_every_format
)
SMOKE=(demo_document_regenerates demo_document_with_drawing_regenerates live_30_features_opens_and_regenerates)
cargo build "$@" -p noshape -p noshape-cli
cargo test "$@" -p noshape-ops --test smoke --test e2e -- --exact "${SMOKE[@]}" "${E2E[@]}"
