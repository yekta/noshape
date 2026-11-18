# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-18 21:25 UTC)

### Landed 11-18
- The 41-commit stack (asm-partorigin) as df22f92c: translator part mapping by body id / volume, std content sizes,
  thread lengths + remembered picks, BOM scorer (a live row short by a live-ERROR pattern's instances = live defect),
  Insert › Other documents sort menus (prefs::InsertMemory, insert_documents.json), region arc join (7bc9d8).
- a033c082 A in an edited mate turns about the connector's Y; fb5e117d no-drop cursor dropped at a fixed drag's release.

### In flight
- Land queue (vibe2): 009997ba (asm-feel) BOM Description / Part number from a custom feature's setProperty
  (Context::feature_property; bjr OPG006; e2e a_custom_features_part_properties_fill_the_bom).
- Queued next from asm-feel once 009997ba lands: d656fd93d mate row double-click test (debug_selector "mate-row
  <name>", feel test on mated_blocks: the pattern fixture's mate rows don't draw in tests, cause unknown), 3fc2cbe6c
  entering a face in the mate dialog shows the centroid, dots on the next move (live2 place-1118 21:05).
- Nested flexible subassemblies, step 1, landing from asm-cursor (branch asm-nested, a28facae4): Instance::nested_deep
  (paths below the instance's direct children, each placement relative to the subassembly instance holding it);
  eval prepare_subassembly takes an occurrence's stored places for the content's flexible subs (self.deep, passed
  down level by level; hashed into the subs_done key); assembly_transform at any depth; translator places every
  occurrence deeper than 2 in Assembly 1 at live's. bjr-config 127 -> 137/137 occurrences (vibe2 ~/tmp/asm/bjr-nested.log,
  ops-nested). e2e a_flexible_subassembly_instances_part_moves_in_its_parent_only (three-level part).

### Step 2 of nested flexible (lead: go, land in steps), design
Today a subassembly holding a flexible one stays rigid (eval.rs prepare_subassembly `sub_prep.flexible.is_empty()`);
step 1 only gives each occurrence its own start. Step 2: the nested children join the top solve (drags and top-level
mates reach them, as live).
- Keys become paths below the top-level instance: nested_body_of HashMap<Vec<InstanceId>, (usize, Frame)>,
  nested_bodies Vec<(usize, Vec<InstanceId>, Frame)>, Solved.nested HashMap<Vec<InstanceId>, Placement> (relative to
  the top instance), SolveOptions nested_start / nested_drags by path, LeafInstance.flex Option<(Vec<InstanceId>,
  DMat4)>. Look-ups `get(&(s, c))` -> `get(&[s, c][..])` (Vec keys borrow as slices). ~45 sites in eval.rs, tests.rs,
  assembly-ui (panels, behaviors, view), ops (assembly.rs fold_solved, assembly/features.rs, positions.rs).
- flex_subs entries per flexible level: a flexible S holding flexible T adds T's children as bodies keyed [S, T, c]
  (start = S-frame of T's place * c's place in T), T's mates resolved with entity paths [S, T, ...] (the `entity`
  closure at the "flexible subassembly mates" block keys nested_body_of by the path), T fixed children ride T's body.
- fold_solved writes len-2 paths to Instance.nested and longer ones to nested_deep (holder-relative: divide by the
  holder's solved place).
- UI: dragging a nested child (assembly_behaviors nested drag) by path; is_flexible_child by path.
- Check: bjr-config stays 137/137 (now solved, not just started), xdev-subs 266/266 (its three-level linkage), tvc-mount
  (flexible subs + top mates), infento-subs; assembly crate tests + e2e nested tests.

### Others' items (sent 11-18)
- robot-relations BOM 36/42: 0c3466 Import 1 makes 6 sheet parts in live, ours sews one 171987 mm³ solid (dx, via lead).
- suspension: 805e29 Sketch 2 unsolvable -> skasm (18a8ce79-…); 995636 Fillet 3 + 805e29 Fillet 3 / 4 -> blend
  (de66e481-…). Rerun suspension when they land. Cap <2> / Cup <1>: empty partIds in live's data.
- pattern-std BOM 13/17: Linear pattern 5 is ERROR in live (mate-messages.json) -> the landed scorer rule covers it;
  confirm in the night run.
- welding-config 9c7704 Part 5 = live Part 5 + 6 -> core (lead). scissor-lift Top <3>: fed22d fillets/holes (model side).
- psui: Coarse level for big assemblies (assembly_open refine pass + eval best_occ); psui edits eval.rs and
  assembly_open.rs, I agreed (keep best_tessellation Fine and the mesh sharing test at assembly_open :445).

### Open asks
- live2: REQUESTS 17:02 Select other + triad ball reach (pending); 21:05 reach done (±49–50 px, first entry centroid).
- Decision pending with the lead: Insert from a release (▲) for local documents (live shows it on company docs only).

### Next, in order
1. Nested flexible step 2 (above), in steps with tests; pair bjr / xdev / tvc / infento after each.
2. live2's assembly placement rows as they arrive (feel first).
3. Night run tag c (xdev-subs, isaac-ball, infento-subs, scissor-lift) 02:00 UTC on vibe2 (night.sh) with a binary of
   main after the lands; then the light 20 again on main for the new baseline.
4. pinslot: 42/42 occurrences, 16/16 mates on main (washer config in the corpus; CENTERLINE = diameter already).

## Scores (last measured, stack / main 11-18)
- bjr-config 137/137 occ (step 1), 31/31 mates, BOM 22/23 (OPG006 Description fix queued). pinslot 42/42, 16/16.
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
- Worktrees: asm (stack, landed), asm-feel (feel rows), asm-cursor (branch asm-nested), asm-docs (docs-only).
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
