# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 07:48 UTC)

### Landed 11-19
- c75f1894 config-assembly BOM (standard content's feature properties), 12/12. 7599cd64 qa session-asm-1119.
- 24792169 live2 errors-1119 / asm-dialogs-1119 rows, Select other reach.
- 058942cc (2): isaac c3fdd4 "Part 2" no longer takes our "Part 1" twice (a live part whose only fitting part of ours
  is taken goes by its name): isaac BOM 58 -> 60/62, mates 96 -> 98/129 (with fa73e1's refs: Revolute 19 ok); the
  mate dialog's hovered face outlined once (thin face outline, not the edges' band).

### Queued / in flight
- asm-fs 40d8509c6 queued (land.sh 07:2x): parity suite gives an assembly's Part Studio the Feature Studios its custom
  features name in their namespace (bjr setData / custDesc from other documents): 5d8bc1 Sketch 29 fixed.
- asm-fs working tree (tests running on vibex7, then commit + land): live2 recheck-1119 —
  (1) Insert: Sketches + Flat patterns always, Sort button (InsertMemory.current_desc, A → Z default, kept);
  (3) a QUERY face drops the selection tint (shaders.wgsl mesh_shade; render test);
  (4) the new group's row red bold "!" while its members are mated; (5)/(7) mate rows' wrappers + mates section
  w_full (rows spanned 146 px: flyouts started at x 190); the flyout takes the theme font (bold head);
  (6) Measure panel at top 279 / right 13 of the graphics area (MEASURE_TOP / MEASURE_RIGHT).
- Dropped: the probe-vertex keep rule (branch asm-live1119 2cf5d55e2): no change on the suite with or without studios.

### bjr (after the studios)
- 137/137 occ, 31/31 mates, 23/23 BOM. Left: 5b44a0 Fillet 1 (no edges), bfc58c Washer Face (non-manifold), 5d8bc1
  Sketch 40 / Extrude 21 (main's: told skasm 07:4x), 5b44a0 / b509fd custDesc "Error regenerating" — live's getRow
  matches on the first value only (5b44a0 at default: #Pitch 1 mm yet the M1.6 row, pitch 0.35; b509fd HeadDiameter
  5.54 -> M3 with pitch 0.5): our FS evaluates `tolerantEquals(a) && tolerantEquals(b)` strictly. FeatureScript
  evaluation difference -> lead to route (dx does most crates/featurescript work).

### isaac-ball (60/62 BOM, 98/129 mates)
- 2 BOM rows: Spiral Bevel gears (GearLab "#desc" fails "Failed to fillet selections."): FS owner, decision with lead.
- The 7 CAP_EDGE / SWEPT_FACE / INTERSECT connectors: mates live itself fails. fa73e1 Chamfer 3 / 4 fail in ours (core).

### Open (mine)
- live2 recheck (2): the first-picked connector drawn faded in live (≈; alpha not measured).
- Ctrl+V see-through (REQUESTS 03:29 to live2, unanswered).
- Night run c on vibex8: infento-subs / scissor-lift pair still running (pair/mainbase-asm-infento-subs.out).

### Others' items
- psui: coplanar depth tie (theirs). suspension Body / Joint / LCA: core. robot-relations BOM 36/42 (dx);
  pattern-std BOM 13/17 (scorer rule); welding Part 5 (core); scissor-lift Top <3>.

### Next, in order
1. Commit + land the recheck stack once vibex7's tests pass; ask live2 for a recheck.
2. bjr 5b44a0 Fillet 1: live's edges are Chamfer 1's written `makeQuery(…opChamfer,"BLEND_EDGE",…{blendedFrom:
   [Revolve 1 SWEPT_EDGE, SWEPT_FACE], blendedInto:[SWEPT_FACE]})` (ids JRF / …, consumed: not in live's final state);
   parity.rs ~2919 takes the first nested `sketchEntityQuery(` of any written text (Sketch 1's curves 13 / 15, whose
   swept edges the chamfer consumed -> "Select edges or faces to fillet."). Needs a written BLEND_EDGE resolver (our
   chamfer's edge between its face and the blendedInto face), and that ~2919 path only for a bare sketch-entity query.
   Then bfc58c Washer Face (non-manifold).
3. Night run c results; isaac Chamfer 3 / 4 to core if geometry.

## Scores (last measured, stack / main 11-18)
- bjr-config 137/137 occ (step 2), 31/31 mates, BOM 22/23 before a8dedfc1 (OPG006 Description; rerun). pinslot 42/42, 16/16.
- Light/medium 20 (stack pair 11-18): pattern-std 92/92 mates, BOM 13/17; tvc 12/12 mates, 26/26 BOM; corexy 22/22;
  robotic-arm 31/31, 37/37, 18/18; robot-relations BOM 36/42; suspension 25/28 occ; welding BOM 27/29.

## How to run
- Builds and crate tests: `NOSHAPE_RB_HOST=vibex8 tools/tiers/rb.sh bash -c '…'` (worktree targets warm there;
  rb.sh syncs to vibex8:agents/<worktree>/src; generated docs come back with rsync from there). Grep cargo output for
  `^error|panicked|test result`, dropping `noshape-kernel@` warnings.
- Parity binary: `cargo test -p noshape-ops --test all --no-run` in rb.sh, then relay
  vibex8:agents/<wt>/target/debug/deps/all-<hash> through this machine to vibe2:agents/asm-keep/bin/<name> (vibex8 ->
  vibe2 scp is refused).
- One assembly, traced, via slot: vibe2 `~/agents/asm-keep/one.sh <bin> <asm dir> <out>`; vibex7's one.sh is untraced,
  its one-save.sh is traced and keeps the rebuilt document in ~/tmp/asm/save (700; open it with a noshape-cli batch).
  Suites only on vibe2 / vibex7 (lead 11-18: vibex8 overloaded), one per host.
- Pairs: `~/agents/asm-keep/pairlist.sh <tag> <dirs>` (ops-mainbase vs ops-stack) -> pair/{mainbase,stack}-<tag>.log.
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh` (LAND_BIG_DELETE=1 only for the reviewed standard JSON);
  never from a worktree with commits still queued.
- Worktrees: asm (branch asm-fs; asm-live1119 holds the dropped keep rule), asm-feel / asm-cursor (landed), asm-docs.
- Hosts (lead DECISIONS a67911bef): pick the lowest 1-min load of vibex7 / vibex8 / vibe2 (or the Mac slot) each time.
- Private documents (hat8 / hat15 / dock on vibex7 ~/lead-private): mode-700 copies in ~/tmp/asm only, deleted after.
- Live data: REQUESTS.md (times from `date -u`); live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7, live
  e8549719-ef9a-42de-960c-e24b29f142a6.
- Agent ids: psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, core
  d30002ca-d013-4435-b7d9-7da9cb564cec, blend de66e481-56e2-4005-a5d3-9fa897ca5fde, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, bool 754a9804-feb1-4153-b32a-acf9b2a6d94d
  (old core e8af89fe / blend 0672fa20 stopped: never message them).

## Assembly model suite (what it does)
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over ~/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES).
  `rebuild_assembly`: Part Studios by the translator, each level (subassemblies first, each its own "Sub <eid>" tab)
  inserted, fixed and placed as live's occurrences, mates between the same connectors; top-level instances held at
  live's places, compared, freed (our mates mustn't move them); mate status, BOM rows (parent, depth, name, qty, part
  number, description, material) as live's.
