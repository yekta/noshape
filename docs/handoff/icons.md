# icons: copied-icon redraw and no-copied-assets guard (2026-10-19)

## State: done, waiting for the lead to integrate

- Redrawn from scratch on the 24 viewBox / `scale(1.2)` 20-unit grid (own geometry, mask-safe):
  `close-x`, `configuration-visibiliity`, `function`, `part-mesh`, `performance-info`, `performance-warning`,
  `create-partstudio-incontext`, `flip-direction-opposite`, `inserted-version-update`, `realign-mate-dialog`,
  `reorder-items-button`, `update-all-button`, plus `add-button` and `connectivity-analysis-{g0,g1,g2,g3,no-connection}`,
  which the guard found as byte-identical copies.
- `calculate-button` and `mate-connector-qlv-item` had already been redrawn by another agent and pass the guard.
  `hole-tolerance-precision.svg` was deleted by another agent; `part_studio/sketch.rs` now uses `tolerance.svg`.
- Guard: `crates/app/tests/no_onshape_assets.rs` with `onshape_path_hashes.txt` (hashes only). It walks the
  workspace (skipping `target*`, hidden dirs and `node_modules`), normalises every SVG `d=` and checks the whole path,
  its 40-char prefix, each subpath and its prefix, and all of a file's paths joined. It also hashes asset files
  (svg/png/gif/jpg/fonts/css/js/env maps) whole. The SHA-256 implementation is inline, so there are no new
  dependencies.
- Generator (outside the repo): `/root/icon-guard/gen.py`, `make.sh` (regenerates the list from
  /root/onshape-research), `gen.py --check @/root/icon-guard/sources.txt -- FILES` names the Onshape symbol
  that matched. Comparison sheet: /tmp/icons/redraw-compare.png.

## Gotchas

- Trivial paths are deliberately not hashed (only half-unit grid numbers and 8 numbers or fewer: rects,
  triangles, quads), so independently drawn primitives don't trip the test. Whole-file copies of such icons are
  still caught by the file hash.
- If the Onshape reference material grows (new sprite version, new captures), rerun `/root/icon-guard/make.sh`.
