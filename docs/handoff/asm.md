# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-18 23:20 UTC)

### Landed 11-18
- The 41-commit stack (asm-partorigin) as df22f92c: translator part mapping by body id / volume, std content sizes,
  thread lengths + remembered picks, BOM scorer (a live row short by a live-ERROR pattern's instances = live defect),
  Insert › Other documents sort menus (prefs::InsertMemory, insert_documents.json), region arc join (7bc9d8).
- a033c082 A in an edited mate turns about the connector's Y; fb5e117d no-drop cursor dropped at a fixed drag's release.
- a8dedfc1 BOM Description / Part number from a custom feature's setProperty (Context::feature_property).
- Nested flexible subassemblies: step 1 0fd15c9a2 (Instance::nested_deep, each occurrence its own places, translator
  places occurrences deeper than 2 at live's); step 2 a1f3a7933 (nested parts join the top solve: keys are paths from
  the top-level instance in Solved.nested / nested_drags / nested_body_of / LeafInstance.flex / Owner::Nested; fold_solved
  writes len-2 paths to nested, deeper to nested_deep holder-relative; UI drag takes the deepest flexible child; open
  cache key "assembly2"). e2e a_mate_moves_a_part_of_a_flexible_subassembly_inside_a_flexible_one.

### In flight (land queue vibe2)
- ac155f8d (asm worktree, branch asm-map) translator: a live part never maps to a sliver (< 1 % of live's volume band);
  fallback by_volume (nearest untaken, within 10 %). tvc-mount 9 -> 12/12 mates, 43/43, 26/26. Test
  a_live_part_is_never_ours_by_a_sliver.
- 33d8db32 (asm-feel): mate row double-click test (debug_selector "mate-row <name>"), face entry shows the centroid and
  dots on the next move (square stays 47.5 px), error mates' flyout (Fastened lists Offset / Simulation connection),
  hidden error row rgb(196,139,148), lost instance red ! badge. After it lands: docs/parity/error-wording.md rows
  (tooltip/flyout, hidden colour, badge) docs-only.

### Others' items (sent 11-18)
- tvc-mount Inner Ring (studio 7bc9d8) 4 % short + a 0.014 mm³ sliver Part 3 since this morning: model side, likely
  skasm cce30a1ad (fewer PROJECTED leftovers in Sketches 21/29/40/41) -> skasm told.
- bjr std studios (configs added by live 22:35): 5b44a0 Fillet 1 "Select edges or faces to fillet.", bfc58c Washer Face
  non-manifold: model side, not yet routed.
- robot-relations BOM 36/42: 0c3466 Import 1 makes 6 sheet parts in live, ours sews one 171987 mm³ solid (dx, via lead).
- suspension: 805e29 Sketch 2 fixed in skasm's tree (lands after its A/B); Fillet 3 / 4 are core's B-spline edges;
  995636 Fillet 3 from core's lopsided Loft 1 (blend -> core). Rerun suspension when they land. Cap <2> / Cup <1>:
  empty partIds in live's data.
- pattern-std BOM 13/17: Linear pattern 5 is ERROR in live -> the scorer rule covers it; confirm in the night run.
- welding-config 9c7704 Part 5 = live Part 5 + 6 -> core (lead). scissor-lift Top <3>: fed22d fillets/holes (model side).
- psui: Coarse level for big assemblies (assembly_open refine pass + eval best_occ); psui edits eval.rs and
  assembly_open.rs, I agreed (keep best_tessellation Fine and the mesh sharing test at assembly_open :445).

### Open asks
- live2: REQUESTS 17:02 Select other + triad ball reach (pending).
- Decision pending with the lead: Insert from a release (▲) for local documents (live shows it on company docs only).

### Next, in order
1. Pair xdev-subs / infento-subs on main with step 2 (three-level linkage; must stay 266/266 occurrences).
2. live2's assembly placement rows as they arrive (feel first).
3. Night run tag c (xdev-subs, isaac-ball, infento-subs, scissor-lift) 02:00 UTC on vibe2 (night.sh) with a binary of
   main after the lands; then the light 20 again on main for the new baseline; then the worst assembly-side gap.

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
- One assembly, traced, via slot: vibe2 `~/agents/asm-keep/one.sh <bin> <asm dir> <out>` (vibex7's one.sh: untraced).
  Suites only on vibe2 / vibex7 (lead 11-18: vibex8 overloaded), one per host.
- Pairs: `~/agents/asm-keep/pairlist.sh <tag> <dirs>` (ops-mainbase vs ops-stack) -> pair/{mainbase,stack}-<tag>.log.
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh` (LAND_BIG_DELETE=1 only for the reviewed standard JSON);
  never from a worktree with commits still queued.
- Worktrees: asm (branch asm-map), asm-feel (feel rows), asm-cursor (branch asm-nested, landed), asm-docs (docs-only).
- Private documents (hat8 / hat15 / dock on vibex7 ~/lead-private): mode-700 copies in ~/tmp/asm only, deleted after.
- Live data: REQUESTS.md (times from `date -u`); live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7, live
  e8549719-ef9a-42de-960c-e24b29f142a6.
- Agent ids: psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, core
  d30002ca-d013-4435-b7d9-7da9cb564cec, blend de66e481-56e2-4005-a5d3-9fa897ca5fde, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b
  (old core e8af89fe / blend 0672fa20 stopped: never message them).

## Assembly model suite (what it does)
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over ~/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES).
  `rebuild_assembly`: Part Studios by the translator, each level (subassemblies first, each its own "Sub <eid>" tab)
  inserted, fixed and placed as live's occurrences, mates between the same connectors; top-level instances held at
  live's places, compared, freed (our mates mustn't move them); mate status, BOM rows (parent, depth, name, qty, part
  number, description, material) as live's.
