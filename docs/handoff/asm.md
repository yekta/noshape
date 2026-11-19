# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 01:58 UTC)

### Landed 11-18 / 11-19
- Nested flexible step 1 0fd15c9a2, step 2 a1f3a7933 (paths from the top-level instance as keys; xdev-subs unchanged).
- d9ed9fc8a translator sliver guard (tvc-mount 12/12 mates; the drop was my df22f92c mapping, not the model).
- f4de8cf5 stack: live2 shortcuts-1118 keys (high quality bar shared as os_ui::high_quality_bar, W prompt, N, Shift+P
  origin ring + dot, Ctrl+C / Ctrl+V, Shift+S, Isolate clears the selection), mate row double-click, face entry
  centroid, error flyout / faded red / lost ! badge, error-wording rows.
- d12e3025 translator: a live part nothing of ours fits takes its feature's own body, a sliver too (asm-suspension's
  Knuckle <1>, uninserted since d9ed9fc8a: 805e29's Fillet 3 / 4 fail; suspension 25/28, 8/8, 21/21 on vibex8).

### Runs
- Night run tag c 02:00 on vibex8 (lead: vibex8 via slot), held by the local unit asm-night-vibex8 (ssh): ops-stack =
  main at d12e3025, ops-mainbase = 11:35 main; pair/{mainbase,stack}-c.log, done-c. vibe2's copy stopped.
- Light 20 pair tag m19 on vibe2 (same binaries): pair/{mainbase,stack}-m19.log, done-m19. First 6: corexy 21 -> 22/22
  mates; config-assembly BOM 12 -> 10/12 (below); the rest equal.
- Traced isaac-ball queued after m19 on vibe2 (isaac-m19.out).
- xDev drag: solve 0.3 ms / step, scene 0.4 ms, release re-eval 76 ms (headless): the 13.3 s is the frame (psui).

### Open (mine)
- config-assembly BOM: a8dedfc1's feature-property fallback fills the std nuts' Description from their library Feature
  Studio's setProperty (live's BOM: empty for standard content; bjr OPG006's custom feature: shown). Only the translator's
  copies of std studios hit it (BomSource::Part); the app's std content is BomSource::Standard. Needs a mark on translated
  std parts (or inserting them as standard content), not done.
- isaac-ball mates (11-18 02:59 data, before the live-point fallback): CAP_EDGE / SWEPT_FACE connectors name sketch
  curves the studio no longer has (a2e4a2 Sketch 5 has no 9MZ2WnAZbZgZ): a stale query live resolves by history; ours
  should fall back to live's connector point. "overdefines" Ball 3-6 etc. are on loops with "not consistent" Fastened
  whose connectors are 2-26 mm off live's (model). Confirm and rank with the traced run.
- bjr std studios 5b44a0 Revolve 2 / Fillet 1, bfc58c chamfers / Washer Face: their sketches were fetched at the default
  configuration (live 22:35) while bjr uses other sizes: likely the corpus copy, not the model. Occurrences 137/137.

### Others' items (sent 11-18)
- bjr std studios (configs added by live 22:35): 5b44a0 Fillet 1 "Select edges or faces to fillet.", bfc58c Washer Face
  non-manifold: model side, not yet routed.
- robot-relations BOM 36/42: 0c3466 Import 1 makes 6 sheet parts in live, ours sews one 171987 mm³ solid (dx, via lead).
- suspension: 805e29 Sketch 2 fixed (skasm 7a8ca3b2 on main); Fillet 3 / 4 are core's B-spline edges;
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
1. Report m19 to the lead when done-m19 appears (short); then the traced isaac-ball run: confirm and rank the
   connector cases (live-point fallback, INTERSECT, edge centres), fix the translator ones with T1 tests.
2. Night run tag c results (vibex8): infento-subs' occurrences and isaac-ball's mates first.
3. live2's assembly rows as they arrive (feel first); REQUESTS 11-19 01:22 (H with a hover, Ctrl+V see-through).
4. config-assembly's std nut Description (above).

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
- Worktrees: asm (main + parity binaries), asm-feel (main), asm-cursor (branch asm-keys, the stack), asm-docs (docs-only).
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
