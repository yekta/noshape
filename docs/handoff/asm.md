# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 21:01 UTC)

### Landed today (6686624b as 702c9878: Display states Esc, driver-mate named position, paste; d671313d; 335f8e43; 9f52415c; free drag c9a6822d queued)
- LOD (91478e2c), Derived mirror of an assembly complete (e87a2b3e, 17c26dc1; left: live's exact node icon, symmetry
  detection on child rows), mass overrides (17c26dc1; left: psui's Part Studio panel CoM / Inertia ☑ Override rows,
  several parts measured with overrides, not captured), surf-intro translator steps (8c6ec107, eb3b06a8).
- rb.sh clippy can report clean on stale synced mtimes: `git diff --name-only origin/main -- crates | xargs touch`
  before the final clippy.

### P1 done: assembly-ui SIGSEGV at exit (main CI 44554083714): RenderWorker joins its thread on drop (d671313d);
not reproduced off CI (15 runs of the unfixed binary on vibe2, all exit 0); TICKETS ASM-UI-SIGSEGV-AT-EXIT [x].

### In flight: interaction feel (lead 20:4x: feel is P0; drags, mate hover and select first)
- Queued c9a6822d: a free (6-DOF) instance dragged by its face moves in the plane of the screen without turning
  (`run_drag_solve`: targets from the drag-start placement; the point drag had turned it 1–2° on 20 px); test
  `a_free_instances_drag_keeps_the_default_cursor_and_follows_the_pointer`; feel rows: the free drag tested, Ctrl+V
  `=` (fixed 702c9878).
- Open assembly rows in docs/parity/interaction-feel.md, next in this order: line 42 Mate connector reach `≈` (live:
  22–23 px across, 14–15 down on a face's centre; on a vertex 22 across, 16–21 down and wins over the vertex
  throughout: a GPU feel test in assembly_feel_tests.rs); 1424 assembly section arrow drag (value box at the arrow,
  0.1 mm steps, tan hatched caps: test missing); 1418 explode drag's 0 mm step (test missing); 1568 Y Hide `≈` (live
  leaves the hover outline until the pointer moves); 1421–1423 / 1471 named positions and display states (tests
  missing); 1454–1457 assembly pattern previews (tests missing).
- Find them: `awk -F'|' 'NF>4 { st=$(NF-1); t=$(NF-2); if (st ~ /≠|≈/ || t ~ /missing/) print NR": "$2 }'
  docs/parity/interaction-feel.md | grep -iE "asm|assembl|mate|instance|explode|named|display|section|drag"`.

### parity_assemblies (lead 11-19: rank by documents, one land each)
- Light 20 (main + the BOM fixes): 18 fully as live; robot-relations BOM 36/42 (dx), suspension 25/28 occ (core:
  805e29 Fillet 3/4, 995636 Body two bodies; trace vibex7 ~/agents/asm-keep/susp.out). Landed today: pattern-std BOM
  17/17 (335f8e43), welding BOM 29/29 (9f52415c; row order left: core's multi-region extrude body order).
- Heavy 4 running on vibex7 (`suite1.sh ops-bomfix h1119 asm-xdev-subs asm-isaac-ball asm-infento-subs
  asm-scissor-lift`, suite/h1119.log): xdev-subs 266/266 occ, 7/7 mates, 74/74 BOM (21:01).
- isaac (98/129): closed for asm (lead 20:4x). Off ends: 7 on Part 1 <12> (a2e4a2 Chamfer 2, core, sent 19:5x) and
  Fastened 43 (TICKETS HOLE-THROUGH-ROUND-POLE [bool]: a Ø6 hole coaxial through an R4 corner round's ball pole;
  repro vibex8 ~/agents/asm-keep/t-mini-false.sh with the current CLI ~/agents/asm/target/debug/noshape-cli).
- Others': robot-relations BOM 36/42 (dx); scissor-lift Top <3>; psui coplanar depth tie.

### isaac-ball connector cases (traced run 07:26, 98/129 mates)
- Live fails them too (matched): Revolute 10, Fastened 6, Revolute 15, Fastened 16, Ball 16 (7 untranslated connectors).
- GearLab "#desc" studios 1e81ca / 90f78b (3 connectors; Spiral Bevel BOM rows): FS decision with the lead.
- Our 7 "cannot resolve" mates are on parts whose geometry differs ("volume fits []"):
  - a04711 (Revolute 6, 11, Ball 13): Extrude 4 REMOVE non-manifold (Part 2 at 2x live's volume) -> bool.
  - a2e4a2 (Ball 7, Fastened 33): Chamfer 2 TWO_OFFSETS 5.5/6 on a concave 7 mm line edge -> core.
  - e54410 (Ball 14, Revolute 17, Ball 13): Chamfer 1 TWO_OFFSETS 2/5 on a plane-cylinder arc fails; Extrude 26
    leaves 2 slivers, so "Part 3" maps to a sliver -> core / bool.

### Parked
- surf-intro (lead 11-19 15:00, routed to core): live's Loft 4 (surface ADD) closes Car into a 5-face SOLID, ours
  stays "Surface 1", so Derived 1 takes Instructions. Repro vibex8 NOSHAPE_PARITY_ONLY=c2-surf-intro-offset (7 s).
- bjr: 5b44a0 / b509fd custDesc (FS getRow first value only: dx); bfc58c Washer Face non-manifold REMOVE -> bool.
- HEAVY-ASM: a decimated level below Coarse only if psui reports level 0 still too slow.
- Scratch to delete when done: vibe2 ~/tmp/asm/{save,save2,save3,stu*,e544}, bins ops-mainbase / ops-main1119;
  vibex8 bin ops-along; vibex7 bin ops-main1119.

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
- Live data: REQUESTS.md (times from `date -u`); live2 d7f4a0c2-c793-48d7-b749-13dbbe1dd3a1, live
  e8549719-ef9a-42de-960c-e24b29f142a6.
- Agent ids: psui 7e3f2635-05e1-47c0-8594-9dbc0e9a79d7, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, core
  d30002ca-d013-4435-b7d9-7da9cb564cec, blend de66e481-56e2-4005-a5d3-9fa897ca5fde, qa 40e853dc-757f-4498-878f-00b41fd8af75, bool 754a9804-feb1-4153-b32a-acf9b2a6d94d
  (stopped, never message: core e8af89fe, blend 0672fa20, psui 4973e295, qa 132756b3, live2 79ae5cdb); lead 818534eb-f6b1-4a00-bb48-ebf0f7cdbd8a.

## Assembly model suite (what it does)
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over ~/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES).
  `rebuild_assembly`: Part Studios by the translator, each level (subassemblies first, each its own "Sub <eid>" tab)
  inserted, fixed and placed as live's occurrences, mates between the same connectors; top-level instances held at
  live's places, compared, freed (our mates mustn't move them); mate status, BOM rows (parent, depth, name, qty, part
  number, description, material) as live's.
