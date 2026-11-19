# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 08:53 UTC)

### Landed 11-19
- c75f1894 config-assembly BOM (standard content's feature properties), 12/12. 7599cd64 qa session-asm-1119.
- 24792169 live2 errors-1119 / asm-dialogs-1119 rows, Select other reach.
- 058942cc isaac c3fdd4 "Part 2" by name (BOM 58 -> 60/62, mates 96 -> 98/129); mate dialog's hovered face outlined once.
- cd1acebd an assembly's Part Studio gets the Feature Studios its custom features' namespaces name (bjr 5d8bc1 Sketch 29).
- 4c5c1d10 written BLEND_EDGE resolver (bjr 5b44a0 Fillet 1) + live2 recheck-1119 rows (1) Insert sort / filters,
  (3) QUERY face drops the selection tint, (4) new group's row red, (5)/(7) mate rows span the list + bold flyout
  title, (6) Measure panel at top 279 / right 13. live2 rechecked on green bfe09bcd5: all six hold (0f4633705).

### In flight (branch asm-fs, /root/projects/noshape-wt/asm)
- 43f8fb59c sketch plane off live's frame -> our face on that frame: bjr 5d8bc1 Sketch 40 + Extrude 21 fixed (bjr now
  3 differences: 2 custDesc, Washer Face).
- a93d0a438 qSketchRegion(…, true) leaves out a disc a line joins to the outline when inside that region (isaac
  a04711 Extrude 1 took the slot's hole: 1838 vs 1790 mm³), not a circle at the outline's corner (a04711 Extrude 4).
- Model suite with both (vibe2 m-one.sh ops-region2 region -> ~/agents/asm-keep/m-region-ops-region2.out) vs base
  m-plane-ops-blend.out (08:58, 127 models): compare with ~/tmp/asm-cmp.py, land both if none worse.

### isaac-ball connector cases (traced run 07:26, 98/129 mates)
- Live fails them too (matched): Revolute 10, Fastened 6, Revolute 15, Fastened 16, Ball 16 (7 untranslated connectors).
- GearLab "#desc" studios 1e81ca / 90f78b (3 connectors; Spiral Bevel BOM rows): FS decision with the lead.
- Our 7 "cannot resolve" mates are on parts whose geometry differs ("volume fits []"):
  - a04711 (Revolute 6, 11, Ball 13): Extrude 1 region (mine, a93d0a438); next Extrude 4 REMOVE non-manifold
    (Part 2 at 2x live's volume), as before the fix.
  - a2e4a2 (Ball 7, Fastened 33): Chamfer 2 TWO_OFFSETS 5.5/6 on a concave 7 mm line edge (our edge = live's JpF):
    live +40 mm³, ours -568 mm³ -> core. Its Sketch 20 plane: the 43f8fb59c case (check isaac-plane.out).
  - e54410 (Ball 14, Revolute 17, Ball 13): Chamfer 1 TWO_OFFSETS 2/5 on a plane-cylinder arc (our edge = live's JTF)
    fails; Extrude 22 leaves RMBD 15/37/22 vs live 14/32/18 (same volume); Extrude 26 leaves 2 slivers (0.077 /
    0.022 mm³), so "Part 3" maps to a sliver -> core / bool.
  - Repros: vibe2 ~/tmp/asm/e544 and ~/tmp/asm/stu/<model> (NOSHAPE_PARITY_MODELS=…, ops-plane, ~11 s), saved docs
    ~/tmp/asm/save2 (a2e4a2), save3 (e54410), save/wf.noshape (bjr Washer Face, tab "Studio bfc58c"); delete after.
- Fastened / Revolute "overdefines" / "not consistent" (21 mates) next: ends off live's on the same studios.

### bjr
- 137/137 occ, 31/31 mates, 23/23 BOM. Left: 5b44a0 / b509fd custDesc (live's getRow matches the first value only;
  ours strict `tolerantEquals(a) && tolerantEquals(b)`: FS -> lead to route, dx), bfc58c Washer Face (non-manifold): the REMOVE of the hex - circle
  region on the nut's own face fails even with one copy of the region (ours had it 6 times) -> bool.

### Open (mine)
- live2 recheck (2) done: the first-picked connector at 20 % (disc, rim, axes) and its face selected yellow; ask live2 to
  retake it on a green build with it. Ctrl+V see-through
  (REQUESTS 03:29 to live2, unanswered).
- P0 HEAVY-ASM-SOFTWARE-FRAME: split agreed with psui (psui: crates/render + viewport scheduling; asm: LOD meshes and
  load). asm-xdev-subs Assembly 1: 208 occ, finest 2,116,100 tris, level 0 462,235, level 1 679,934. A decimated level
  below Coarse only if psui reports level 0 still too slow.
- Night run c on vibex8 (infento-subs / scissor-lift pair) results.

### Others' items
- psui: coplanar depth tie. suspension Body / Joint / LCA: core. robot-relations BOM 36/42 (dx); pattern-std BOM 13/17
  (scorer rule); welding Part 5 (core); scissor-lift Top <3>.

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
