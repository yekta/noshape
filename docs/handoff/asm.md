# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 04:19 UTC)

### Landed 11-18 / 11-19
- Nested flexible step 1 0fd15c9a2, step 2 a1f3a7933 (paths from the top-level instance as keys).
- d9ed9fc8a translator sliver guard; d12e3025 a live part nothing of ours fits takes its feature's own body
  (asm-suspension Knuckle <1>: 25/28, 8/8, 21/21).
- f4de8cf5 live2 shortcuts-1118 keys stack; 7599cd64 qa session-asm-1119 (a new mate's type switch takes that
  type's next name, Esc on an insertion offers Restore, ui_script markers for Insert rows / search, limit fields,
  exploded Done).
- c75f1894 config-assembly BOM: PartStudio::standard_content on the translator's copies of std studios; the BOM's
  feature-property fallback skips them (12/12 on vibex8).

### Queued / in flight
- fd0740e4 (asm-feel, branch asm-err1119): live2 errors-1119 rows (error flyout bold title / width / grey field
  values, list rows full width so the flyout opens at the list's edge, fixed ⚠ on the fixed and root icons with
  live's bubble, crossed limits disable ✓ / Enter + field tooltip, inactive mate italic only, "(Suppressed)" tooltip,
  Insert leaves out empty studios + "No Part Studios match current search."), Select other's reach (edge 8 px, the
  face behind 11 px; live selother-ball-1119; app-core Viewport::pick_at_pointer_within). 86/86 assembly-ui.
- asm worktree, branch asm-live1119 (not queued): translator keeps a sketch solve that lands on live's probed
  sketch vertices (bjr's std screw 5b44a0 / nut bfc58c: #Length / #HexSize were taken back -> Revolve 2 missed,
  Fillet 1 no edges, Washer Face non-manifold). Test passes; bjr run with it queued on vibex8 (ops-probe ->
  asm-keep/bjr-probe.out). Land if the std studios' differences go and nothing else moves. Note for the lead: the
  rule also applies to Part Studio corpora with probe sketches.json (only toward live's vertices).

### Runs
- Night run tag c on vibex8 (asm-night-vibex8 unit, ssh-held): xdev-subs 244 -> 266/266 occ, BOM 58 -> 74/74 on the
  stack; isaac-ball, infento-subs, scissor-lift running. pair/{mainbase,stack}-c.log, done-c.
- Light 20 pair m19 on vibe2 (slow, load 25): 9 / 8 of 20 done; corexy 21 -> 22 mates; config-assembly 12 -> 10 BOM
  (fixed by c75f1894). Traced isaac-ball queued after it (isaac-m19.out); the night run's isaac comes first.

### Open (mine)
- isaac-ball connectors (CAP_EDGE / SWEPT_FACE stale queries fall back to live's connector point; INTERSECT and
  edge centres): confirm and rank from the night run's / traced output, fix translator ones with T1 tests.
- Ctrl+V's see-through: which instance is translucent in live (REQUESTS 11-19 03:29 to live2); ours draws the copy
  orange and opaque.
- asm-suspension Cap <2> / Cup <1>: live can't resolve them either (live 11-19: no Cap / Cup in 995636 at that
  microversion; red rows "Failed to resolve instance.", mates Cup distance / Fastened 1 red). Already scored as live
  defects; the rebuild leaves them out instead of inserting them unresolved (cosmetic).
- bjr by-configuration data (live 11-19): studios/<eid>.sketches-by-config.json, feature states + volumes per instance
  configuration (5b44a0: 36.578 / 75.091 / 148.443 mm³; bfc58c 50.064 mm³): use to check our instances' volumes.

### Others' items
- suspension Body / Joint / Lower Control Arm: core's B-spline / r40 offsets (Fillet 3 / 4 of 805e29, 995636 Loft 1).
- robot-relations BOM 36/42 (dx: live's own re-import makes one faulty solid). pattern-std BOM 13/17 (Linear pattern
  5 ERROR in live: scorer rule). welding-config Part 5 (core). scissor-lift Top <3> (model side).
- psui: Coarse level for big assemblies (eval best_occ, assembly_open refine): agreed, theirs.

### Open asks
- Decision pending with the lead: Insert from a release (▲) for local documents (live shows it on company docs only).

### Next, in order
1. Report fd0740e4 when it lands; bjr probe result -> land asm-live1119 or fix.
2. isaac-ball connectors from the night run (vibex8 pair-c) / traced run.
3. m19 short report; night run c results (infento-subs occurrences, isaac mates).
4. live2's answer on Ctrl+V; then the paste look.

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
- Worktrees: asm (parity binaries; branch asm-live1119), asm-feel (asm-err1119, queued), asm-cursor (landed), asm-docs
  (docs-only).
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
