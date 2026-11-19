# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 14:10 UTC)

### In flight: Derived mirror of an assembly (live2 audit-1119 m2–m14; behaviours "asm (11-19)")
- Piece 1 (model + eval), branch asm-dmirror 28ddc6e06: `InstanceSource::DerivedMirror { assembly, derived,
  placement, mirrored_studios }`, a rigid node of the source's instances mirrored in its XY (Transform: same part,
  placed by its Placement type; Derived: the "-Mirrored" studio's part, "Block-Mirrored <1>"); BOM, drawing sources.
  Full ops e2e passed (253).
- Piece 2 (ops, uncommitted): `Document::{mirrored_assembly, ensure_mirrored_assembly, mirrored_tab,
  ensure_mirrored_tab}`; Mirror strategy Derived on a subassembly makes "<A>-Mirrored" (fixed "Derived mirror" node)
  and the copy is "<A>-Mirrored <1>"; eval's reflection_of treats it like a mirrored studio. e2e
  `a_derived_mirror_of_a_subassembly_makes_its_mirrored_assembly`.
- Branch asm-mass (stacked on queued 673142ab; land after it): mass overrides (below), 3a ecca84b2e (strategy row of a
  subassembly: assembly icon, "<A>-Mirrored" reference; UI test of Update on a subassembly), 3b a194eedac
  (`assembly_derived_mirror`: the node's source and per-part strategy, Derived makes the "-Mirrored" studio; e2e).
- 3c 53a0e2897: the node's row icon (derived-mirror), menu as live m10, Edit…'s dialog (Assembly to mirror, Mirror
  strategy… sharing the Mirror's table through `StrategyHost`, Update, ✓ "Edit : Derived mirror"); UI test.
- 3d: the Mirror strategy's child rows under a subassembly row (chevron, Transform / Derived, placement; ✓ writes the
  "-Mirrored" assembly's node strategy, "Edit : Derived mirror"); UI test. Derived mirror is complete; left only
  live's exact node icon and symmetry detection on child rows (they show Symmetric).
- Note: rb.sh clippy can report clean on stale synced mtimes: `git diff --name-only origin/main -- crates | xargs
  touch` before the final clippy (11-19 a complex-type warning hid that way).

### In flight: mass-property overrides (live2 12:42–12:45, behaviours "asm (11-19)", live/asm/mass-override-1119 c1–c5)
- Branch asm-mass a96eaf1b2: PartProperties `com_override` (m) / `inertia_override` (kg·m², column by column
  as live: Lxy at index 3); `measure::with_overrides` (CoM: parallel axis to it; Mass: inertia × m / computed, centre
  kept; Inertia replaces); Part Studio mass with one part measured; assemblies per part (`part_overrides`,
  `assembly_mass_properties_with`); the assembly's Mass override now scales the inertia in assembly_mass_properties
  (the dialog already did). part_set_properties `center_of_mass_override` / `inertia_override`; `null` now clears the
  overrides (Args::get dropped nulls: mass_override null never cleared). e2e center_of_mass_and_inertia_overrides_….
- Left: Part Studio panel's CoM / Inertia ☑ Override rows (psui's part-studio-ui: tell psui once landed); several
  parts measured with CoM / inertia overrides (not captured).

### In flight: parity_assemblies differences (lead 11-19 15:00: rank by documents, one land each)
- Suite on vibe2: `~/agents/asm-keep/suite1.sh <bin> <tag> <dirs>` (slot, one model at a time) -> suite/<tag>.log;
  bin ops-main1119 = main 17c26dc1 + translator steps. Light 20 running as tag l1119; heavy 4 (xdev, isaac,
  infento, scissor) next.
- isaac (10:16 trace): of 31 mate differences, 14 have an end off live's connector, the rest are collateral (their
  ends exact, the solve fails around them). 8 of the 14 are on Part 1 <12> (a2e4a2, core's Chamfer 2); the others are
  Part 1 <22> (Ball 2 / 8: 1.37 mm, 90°), Part 1 <1> (Fastened 43: 90°), Part 1 <84> (Fastened 83), Part 2 <5>
  (Fastened 84: 1°), Part 1 <88> (Ball 9: 6 / 32.8 mm).

### Others' items from today
- core 12:4x: 90d254008 moved c2-surf-intro-* Derived 1's body +0.1 in x: correct (live's source index 4 has the
  Title text on the x=0 face, −25.4..2.54). Our Derived 1 imports Instructions (JHD) instead of live's Car (JWD):
  the source's car isn't built. Loft 1's profile (INTERSECT of Projected curve 2's own extrude1 / extrude2 bodies,
  a wire), Lofts 2-4 and Fillet 1 map now (6bca92a2 queued + local Loft 3 commit): the source translates fully.
  Parked (lead 11-19 15:00, routed to core): live's Loft 4 (surface ADD) closes Car into a 5-face SOLID, ours stays
  "Surface 1" (4 faces), so Derived 1 still takes Instructions (JWD resolves to it with no Car). Repro: vibex8
  NOSHAPE_PARITY_ONLY=c2-surf-intro-offset (7 s).

### In flight: HEAVY-ASM-SOFTWARE-FRAME, LOD side (branch asm-lod, 97b431e3e, not landed)
- asm-xdev-subs' coarsest level: 462 k triangles / 273 k edge segments, 358 k / 219 k of them one 10 482-face part
  (x5). `MeshBuffers::proxy` (vertex clustering, 200 cells along the diagonal, bodies > 4000 triangles, kept when it
  halves them; memoised, warmed after a big scene's evaluation on a software adapter) replaces the coarsest level on a
  software rasteriser (`AsmSceneOptions::software`, `asm_scene::display_levels`): 133 k / 110 k. GPU unchanged.
- psui's renderer side (3da0173e) on vibe2 Xvfb (hv.sh): frames 1.1-1.3 s -> 0.11-0.28 s, wheel notch 3.6 -> 0.72 s
  (live 3.4 s). My A/B: vibe2 ~/tmp/asm/hv/hv.sh (Xvfb :393) noshape-base (= psui's 3da0173e build) vs the release
  build of 97b431e3e; parse with /root/psui-scratch/hvparse.py.

### Landed 10:12 as 8f27d450 (3 commits)
- Sketch plane off live's frame -> our face on that frame (bjr 5d8bc1 Sketch 40 + Extrude 21; bjr now 3 differences).
- qSketchRegion(…, true) leaves out a disc a line joins to the outline when inside that region (isaac a04711
  Extrude 1), not a circle at the outline's corner (a04711 Extrude 4). Model suite 127/127 models as base.
- First-picked connector at 20 % + its face selected (live2 recheck (2)); live2 asked to retake on green.
- In flight: isaac with both (vibe2 one.sh ops-region2 -> ~/agents/asm-keep/isaac-region.out).

### isaac-ball connector cases (traced run 07:26, 98/129 mates)
- Live fails them too (matched): Revolute 10, Fastened 6, Revolute 15, Fastened 16, Ball 16 (7 untranslated connectors).
- GearLab "#desc" studios 1e81ca / 90f78b (3 connectors; Spiral Bevel BOM rows): FS decision with the lead.
- Our 7 "cannot resolve" mates are on parts whose geometry differs ("volume fits []"):
  - a04711 (Revolute 6, 11, Ball 13): Extrude 1 region (mine, landed in 8f27d450); next Extrude 4 REMOVE non-manifold
    (Part 2 at 2x live's volume): its six regions match live's one to one -> bool.
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
