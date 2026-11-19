# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 00:30 UTC)

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
  cache key "assembly2"). xdev-subs unchanged on it (266/266, 7/7, 74/74).
- d9ed9fc8a translator: a live part never maps to a sliver (< 1 % of live's volume band); by_volume fallback (nearest
  untaken within 10 %). tvc-mount 9 -> 12/12 mates, 43/43, 26/26. The drop was my df22f92c mapping, not the model
  (11:35 vs 23:05 main: same 13 parts, Inner Ring 11501.5 / 11503.0 mm³, live 11788.8: an old 4 % at Extrude 15 /
  Fillet 4; skasm told).

### In flight: ONE stack (asm-cursor, branch asm-keys), queue only after the lead's daemon fix 58d20676 lands
Land-daemon bug (lead 00:2x): a change stacked on a batch already holding the same commits failed on an empty
cherry-pick ("rebase conflict … (in )"); 5a34b488 / f1c69ac9 / efff9aa4 / bf59d5c6 failed so. The stack (clippy clean,
assembly-ui lib 80/80 on vibex8):
- keys (live2 shortcuts-1118): Shift+R the shared high quality bar (os_ui::high_quality_bar), W prompt
  (ZOOM_WINDOW_PROMPT, Esc / box end it), N view_normal, Shift+P hides the origin (ring + dot; triad only while
  picking), Ctrl+C no toast, Ctrl+V place_at_pointer (unselected), Shift+S no banner (render_mode_banner removed),
  Isolate clears the selection; tests the_assembly_keys_act_as_live, show_mates_mode_shows_the_hovered_instances_mates.
- feel: mate row double-click test, face entry shows the centroid then dots, error flyout / faded red / lost ! badge;
  error-wording rows; this handoff.
- asm-feel and asm-docs are now plain main (their commits are in this stack).

### Runs
- xDev drag (HEAVY-ASM-SOFTWARE-FRAME): solve 0.3 ms / step, scene move 0.4 ms, release re-eval 76 ms (headless probe on
  qa's save): the 13.3 s is the frame render (psui's Coarse level), not assembly.
- infento-subs (traced, ops-map) timed out at 3000 s under load; the night run measures it.
- suspension with skasm 7a8ca3b2: unchanged, 24/28 occurrences, 8/8 mates, 20/21 BOM both ways (vibe2 sus-{map,sus}.out).
  Knuckle <1>: studio 805e29 makes Part 1 + Part 2 where live has Knuckle (model side: its Fillet 3 / 4, core); Cup <1> /
  Cap <2>: live defects (empty partId).
- Night run tag c 02:00 vibe2 (night.sh armed): ops-stack = ops-map (main + step 2 + sliver), ops-mainbase = 11:35 main.

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
1. Queue the stack once the lead says 58d20676 landed; suspension pair result.
2. live2's assembly placement rows as they arrive (feel first); recheck asks: H with a pointer move first, Ctrl+V's
   see-through look.
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
