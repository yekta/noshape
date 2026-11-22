# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-22 15:11 UTC) — RESUME HERE
- LANDED 22f626c1 as ec33215f (14:40); qa told 15:1x.
- LOCAL, rebased on main (not queued yet): f65ccd02f tyre fix (ASM-OPEN-TYRES-TURNED: `Context::revolve_axes`, implicit.rs
  `revolve_axis()` Z = -drawn; ScopeRule 19 + `PartStudio::legacy_revolve_axes`; least_change skipped when LM moved
  nothing; assembly lib 170 + model 606 green on vibex7), b081fd516 UI (sel-gestures-1122: no click toggle, Ctrl+box
  removes / `Viewport::ctrl_boxes`, crossing box outline `Viewport::box_preview` + `ViewportEvent::BoxTouched` ->
  `ui.box_touched`; mc-infer-1122: Shift locks the face (face rule, centroid off it), cylinder side MidAxis primary,
  MIDPOINT_PICK_RADIUS_PX 41.5; qa FEEL-GEAR-DIALOG-MATES-LOOK (relation mates in the shown-mate loop),
  FEEL-MATE-SECOND-PICK-HOVER (pointer still at `hud`: second pick plain, part outlined), UIS-PATTERN-SECOND-DIRECTION
  (`field_mark`: field:Direction#1)), tickets commit. Clippy + assembly-ui 175 green on the pre-gear tree.
- RUNNING (slots held by core's / qa's parity runs): vibex7 sg.log (ops e2e), sg2.log (clippy app-core/assembly-ui +
  assembly-ui lib on the rebased tree); vibex8 probe8.log (infento probe, legacy vs NOSHAPE_PROBE_NEW_AXES=1, SOLVE ms;
  instrumentation was synced to vibex8 then reverted locally: /root/asm-scratch/probe-instrumentation.patch eval.rs +
  assembly_perf.rs parts, plus a temp env switch in partstudio.rs migration; never commit them).
- UNCOMMITTED: solver.rs test `a_revolute_part_grabbed_on_its_axis_turns_toward_the_pointer` (qa MATE-GEAR-DRAG-TURNS-
  NOTHING reopened: live wf25 s32 presses at Part 1's front-face centre = Revolute 2's axis, lever ~0, and still turns
  Part 2 <2> toward the pointer; hypothesis: our LM sees no gain and stops). Run it (assembly --lib) first; if it fails,
  fix the drag solve for a near-zero lever (turn the lever toward the pull), then the wf25 replay.
- Slots: vibex7 held by core's parity (started ~14:44, timeout 7000 s), vibex8 by qa's parity (~14:47, 5400 s) and two
  bool gates. Mac has an empty ~/noshape-host/agents/asm (cold build). My waiting runs start when those end.
- Then: land (touch -c, diff --stat, cherry), tell core "ScopeRule 19 (legacy_revolve_axes)" and qa the sha.
- Live triad-feel-1122 compared: ours matches (75x26 box focused/selected and kept, 0.1 mm step, ring free beyond 0.4
  step = 2 deg, no geometry snap on arrows). Nothing to change.
- Next: MATE-GEAR-DRAG-TURNS-NOTHING reopened (wf25 step 32: the drag changes no pixel), dx's re-measure once e52b091a is
  on main (infento display key + cached/cold open, ot2.sh vibex8, ~/agents/asm-keep/asm-open-times.sh), then the list below.
- Next after: first frame from StudioDisplay rows; infento parity rerun (10800 s); xdev-subs / pinslot reruns, bjr Sub
  c1e15f Fastened 3/4, pinslot Revolute 5, tvc 41/43, keyboard BOM 15/16.
- Cleanup: asm-base worktree; vibex7 ops-bis / ops-1120 / ops-bt / ops-mc; vibex8 ops-subs / ops-mc / ops-tc / cli-tc,
  weld/w0-p*.noshape, ~/tmp/asm/q, ~/tmp/asm/app-b + app, vibex8 ~/tmp/asm/infento-saved.noshape (copy); vibe2 ops-tc /
  ab1120e-*.out; local branch asm-rb-check; /root/asm-scratch/feel_tests_with_probe.rs.
- Infento main: 157/167 occ, 204/206 mates, 64/64 BOM. Scissor main 07:18: 207/239 occ, 16/34 mates, BOM 62/87.

## State (2026-11-20 18:50 UTC)

### P0 closed: isaac drag 7-10 s/frame (v0.4.62 252 ms)
- `big_assembly_perf::suite_assemblies_speed` on asm-isaac-ball (private copy ~/tmp/asm/speed on vibe2 / vibex7 /
  vibex8; script ~/agents/asm-keep/isaac-speed.sh <binary> <out>, through ~/bin/slot; it now asserts a drag frame
  <= 300 ms). Drag frames don't re-evaluate Part Studios (assembly_perf.rs drag_in: `prepared.solve_drag` only).
- Culprit 33c176a43 (bool, bisect); bool's revert 61295b96f: drag 185 ms/frame (worst 233), bool + lead told.
- isaac "evaluate again" 2.2 s on main vs 62.6 ms at e4bf3ee7a (v0.4.62 2.8 s; not the Ball block): bisecting
  (systemd unit asm-eval-bisect, /root/asm-scratch/eval-bisect.sh, log eval-bisect.log; worktree asm-base, build
  vibex7 ops-bisect, isaac on vibe2; bad > 500 ms; paths = the engine crates). Bad: b0995d8fd 2616, 96aee900a 2781,
  f7b711726 2491 ms; 18:5x measuring 3d459b517, 2 left (3d459b517, 73470cffb, 83423fa52 skasm revert, c1dbf3f03, bad5d2ecc).
- Sparse group solve LANDED 15:25 as 011ef691 (isaac on main 14:45: drag 31 ms/frame, worst 44): `jacobian_rows` + `normal_equations` over each row's
  bodies, `BlockCholesky` (6x6 blocks, minimum degree), dragged LM steps take up to 4 second-order corrections
  (`jt_mul`, same factor), the Ball block (443952080) back with f50 out of KNOWN. isaac on the 07:26 base: 957 ->
  239 ms/frame; T1 the_block_solver_matches_the_dense_one, the_normal_equations_match_the_dense_ones; T2
  a_linkage_drag_step_takes_300_ms_at_most 99 ms; oracle_asm T2 7/7 (fuzz 76/80); assembly 165, assembly-ui 113,
  ops assembl 46 pass (vibex8 14:0x-14:2x).
- 850efe (blend P1): closed as not asm (lead 10:4x): one-instance assembly (vibex8 ~/agents/asm-keep/m85asm) and
  standalone give identical features and body (6729.709 mm³), Fillet 2 + 3 Error on both; Extrude 6 is bool's
  (coaxial cylinders not merged). Saved isaac doc: 850efe 1.4 s (open profile, vibe2 ~/agents/asm-keep/openprof.out).
- qa 10:0x: FEEL-ASM-DRAG-START verified (green b2b0cf9a9). Live answered the triad question (behaviours "Instances,
  triad, dragging", live/asm/triad-click-1120): the triad sits at the click point and stays; a centre press + drag moves
  the triad alone (no value box): ours too (qa told). To check against ours: X arrow 30 px = 1.9 mm with a "1.9 mm" box
  during and after; the ring at the Z tip turns 15° about world Y through the triad's centre with a "15 deg" box.
- Mac 18:1x at f00b10093: assembly-ui lib 113 passed, 0 failed (the 10 Mac-only failures gone). Mac off until 21:35.

### WIP split: LANDED 16:24 as 78b3cc95
- d9a0335d4 P0 FEEL-ASM-MATE-PREVIEW-COLOUR (asm_scene `preview_tops` peach edges 0xf7c171 / hidden 0xdcb971; picked
  boundary PICKED|OUTLINE -> PICKED_OUTLINE #FFC20E); 643e60a35 P2 ASM-INSERT-RELEASED-ITEMS (▲ View released
  items); 9008a18a5 P2 ASM-FACE-TAB-REFERENCE-FRAME + ASM-PATTERN-ZERO-DISTANCE-MARK; 8000528e0 parity translator
  (Derived chains, composites) + `suite_assembly_open_profile`. Tickets marked. asm-mass / asm-wip-0815: superseded.

### Parity translator: Width / Tangent / relations + Tangent flip: LANDED 17:35 as eb4fadd8
- cov3 with ops-wtr: width 1/1, screw 2/2 as live; tangent moved both parts (4.7 / 6.3 mm): a Tangent's flip was taken
  inverted by assembly_mate (model flag = its flip alone, as the dialog) and a new one defaulted from the connectors'
  Z axes; fixed (op, list, app default unflipped = live flipAlignment false), e2e a_tangent_mate_lays_a_pin_on_the_face_unless_flipped.
- asm-cov-{width,tangent,screw} on main+sparse (vibe2 ~/agents/asm-keep/cov3/): 2/2 occurrences each, but width and
  tangent 0/0 mates and screw 1/1 (its Screw relation unseen): the translator skipped BTMWidthMate, BTMGeometryMate
  and BTMMateRelation silently (also the 18 relations in other asm-* models). Draft: `mate_picks` (Width a,b,c,d =
  tab 1, width 1, width 2, tab 2; Tangent's entities as connectors "<id>/first|second"), relations made after every
  part is at live's place (assembly_relation; length for Screw / Rack and pinion), counted with the mates; T1
  width_and_tangent_mates_pick_as_assembly_mate. Runs: cov3.sh with ops-wtr (wtr-test.sh); A/B on the 9 suite models
  with these features (systemd asm-rel-ab, /root/asm-scratch/rel-ab.sh, log rel-ab.log; A ops-wb = b5e676d0d in
  worktree asm-wb, B ops-wtr; vibe2 ~/agents/asm-keep/suite/{rwb,rwtr}.log).

### Next, in order
1. rel-ab A/B (rwb 8/9 at 18:1x: clamp 27/32/14 all, gearbox 23/20/8, infento 162/167 200/203, isaac 118/120 86/129
   60/62, linear-limits, linear-relations, rack-pinion, robot 139 11 36/42): compare rwtr; ops-wtr predates the Tangent
   flip fix (scissor's Tangent: rerun on main). 2. The evaluate-again culprit (bisect) to its owner / fix.
3. Our triad value boxes against live: X arrow 30 px = 1.9 mm with a "1.9 mm" box during and after; the ring at the
   Z tip turns 15° about world Y through the triad's centre with a "15 deg" box.
4. bool: re-check isaac Fastened 43 on main after pole2 b3706f04 (traced isaac run, tell bool if it still differs).
5. infento composites / scissor from the -comp runs (vibex7 ~/agents/asm-keep/{infento,scissor}-comp.out).
6. Feel: all asm rows = (18:4x); 61e3da3ef closed the stale drag-start ≠ row. Left ≈: connector dot reach 47.5 vs
   live ±48 / ±49–50 (captures disagree). studios writes the untested asm rows' tests: coordinate first.

### Landed today (702c9878: Display states Esc, driver-mate named position, paste; d671313d; 335f8e43; 9f52415c; 123ca4b3 free drag)
- LOD (91478e2c), Derived mirror of an assembly complete (e87a2b3e, 17c26dc1; left: live's exact node icon, symmetry
  detection on child rows), mass overrides (17c26dc1; left: psui's Part Studio panel CoM / Inertia ☑ Override rows,
  several parts measured with overrides, not captured), surf-intro translator steps (8c6ec107, eb3b06a8).
- rb.sh clippy can report clean on stale synced mtimes: `git diff --name-only origin/main -- crates | xargs touch`
  locally before the final clippy (the build host's copy has no origin/main).

### P1 done: assembly-ui SIGSEGV at exit (main CI 44554083714): RenderWorker joins its thread on drop (d671313d);
TICKETS ASM-UI-SIGSEGV-AT-EXIT [x].

### Interaction feel (lead 20:4x: feel is P0): my assembly rows are all tested
- Landed 123ca4b3: free drag in the screen plane without turning.
- Landed 615bed1f: 7bc96ec56 mate connector reach row 42
  (22–23 across, 15 down, over the vertex; live's 16–21 down is a Part Studio hold), e2211fcb4 section arrow drag
  (row 1424; the caps stay tan while the dialog is open, live 16303), 88b65b70f explode short drag 0 mm step (1418),
  8bf03e7b3 named positions' menu + Apply (1421–1422), display state choice (1423, 1471; `choose_display_state`
  factored out of the select), Y Hide `=` (live's leftover outline is its redraw lag).
- Not mine (my old list was wrong): 1452–1457 are psui's Part Studio pattern rows; 699 is psui's Dialogs matrix.
- Named position row click (edits the cell, applies nothing): landed 9bbc3f11.

### qa P1s 11-19 (landed 23:21 as d67512a3)
- ASM-DRAG-UNFIXED-GROUP: my eedeede50 free-drag path took any 6-DOF instance, so a mated group with nothing fixed
  didn't move; now `Prepared::moves_alone` (no mate, pattern or group). Test fails without the fix.
- ASM-LIMIT-TYPED-NO-PREVIEW: an edited mate's held values go through `eval::held_in_limits`. Test fails without it.
- ASM-NEW-MATE-PREVIEW-GROUP: preview = live (first pick alone); after ✓ ours moved the other part too, now the moving
  part's group follows it (`Prepared::mated_group`, accept_mate). Test fails without it. Landed 00:50 as 5a8338b6.

### live retakes 11-19 (eef639d7)
- The instance list row menu of an in-document part: "Where used…" after Export… (live asm-menu-add-selection-to-folder
  / unpack-folder): already landed by studios (9edd47f83). Create task… / Add comment: cloud.

### Lead rows 11-19 (after infento / scissor)
1. isaac on the current tree (traced vibe2 ~/agents/asm-keep/isaac-main.out, 7200 s): Chamfer 2 fits live's volume and
   the whole block (Fastened 7/8/9/33/57/79/88, Ball 1/7) matches (h1119's failures were its older binary); core
   told. Now 118/120 occurrences, 126/129 mates, 60/62 BOM rows. Leftovers by owner:
   - mine: 7072ea (and its copies 00f1d4 / d78cbf / de4eff) Extrude 38 REMOVE "do not intersect": its scope is
     body:F48f36a73feb.0, check it is live's part (trace isaac-main.out "Studio 7072ea: feature 71"); Revolute 6's
     end on that studio's part follows. Sketch 39's LENGTH "Value must be positive". 40 instances "freed at live's
     place, our mates move it" 0.05–0.25 mm (<88> 32 mm, <47> 3.9 mm, <85> 3.0 mm): P2, after the studios.
   - bool: a04711 Extrude 4 / 27 and 850efe Extrude 8 "non-manifold body"; a04711's cascade (Sketch 5 plane, Loft 1,
     Extrude 6/13/46) and Revolute 11 / Ball 13 (on a04711's Part 2) follow it.
   - blend: GearLab 1e81ca / 90f78b #desc fillet → Extrude 1-3 → the 2 Spiral Bevel gears (2 occurrences, 2 BOM rows);
     fillets / chamfers in 00f1d4, 884765, a2e4a2 (Chamfer 4), b29603, c3fdd4, de4eff, e4a405, fa73e1.
2. Speed (`big_assembly_perf::suite_assemblies_speed`; landed 68eee738). isaac (vibex7 load 24, release): open 140 s
   cold (prewarm 133 s = regenerating its Part Studios), evaluate again 3.25 s / add a mate 3.21 s (all mate solve),
   drag frame 234 ms, insert 1.26 s. Live (behaviours "asm (11-20): … isaac-ball timings", software GL): open list
   46-58 s (drawn 187-238 s), regenerate quiet 18.7 s, Fastened mate quiet 4.1 s, insert quiet 4.4 s; drag not
   measurable there. So the open is the gap: lead 03:4x made it P1, profile it (`suite_assembly_open_profile`,
   NOSHAPE_PERF_DOCS=vibe2/vibex7 ~/tmp/asm/speed/asm-isaac-ball.noshape): top 5 studios / features with ms to the lead.
3. Infento on the current tree (vibex7 ~/agents/asm-keep/infento-cur.out, 04:00): 147/167 occ, 184/187 mates, 56/64 BOM.
   Not inserted (mine): 15 instances of studio 1dc32b's COMPOSITE parts, which never get a "live part" mapping line (the
   mapping in rebuild_assembly sees no live entry for them; ours has "Composite part 1-17"): pulleys 46T R8DD /
   26T RsDD, Seatboard RBHD, Ducktail board RrJD / extension RLID, 7" Wheel asm R8BD x5); Wheel 7inch x5 on subassembly
   b16934 (DeletedElement in live). Planar 10/11, Revolute 10 cannot resolve; Large wheel tyre <1>/<2> freed move 1.5 m.
   Studios: 74d3d6 Plane 1 INTERSECT, Sketch 15/23; f0eedd Sketch 7; a3f7a1 Extrude 1 region.
4. Older notes on the suite's next differences (xdev-subs 266/266 7/7 74/74 = live; infento timed out at 3600 s in h1119,
   rerun with ≥7200 s; scissor running in h1119 on vibex7). Infento (06:37 base): 5 Wheel 7inch instances whose
   subassembly is DeletedElement in live; missing parts (pulleys R8DD/RsDD, Seatboard <2>, Wheellock rings);
   74d3d6 Plane 1 "three faces have no common vertex of ours" (translator, mine) → Sketch 15 / Extrude 24; the
   fillets / chamfers (1dc32b, f0eedd, a3f7a1, 74d3d6) to blend after a trace.
- Find open rows: `awk -F'|' 'NF>4 { st=$(NF-1); t=$(NF-2); if (st ~ /≠|≈/ || t ~ /missing/) print NR": "$2 }'
  docs/parity/interaction-feel.md | grep -iE "asm|assembl|mate|instance|explode|named|display|section|drag"`.

### parity_assemblies (lead 11-19: rank by documents, one land each)
- Light 20 (main + the BOM fixes): 18 fully as live; robot-relations BOM 36/42 (dx), suspension 25/28 occ (core:
  805e29 Fillet 3/4, 995636 Body two bodies; trace vibex7 ~/agents/asm-keep/susp.out). Landed today: pattern-std BOM
  17/17 (335f8e43), welding BOM 29/29 (9f52415c; row order left: core's multi-region extrude body order).
- Heavy 4 running on vibex7 (`suite1.sh ops-bomfix h1119 asm-xdev-subs asm-isaac-ball asm-infento-subs
  asm-scissor-lift`, suite/h1119.log): xdev-subs 266/266 occ, 7/7 mates, 74/74 BOM (21:01).
- isaac (98/129): closed for asm (lead 20:4x). a2e4a2 Chamfer 2 is fixed by blend's bb667320d (h1119 has no difference
  there; told core 21:5x; a2e4a2's Chamfer 4 "Failed to chamfer selections." is left). Off end:
  Fastened 43 (TICKETS HOLE-THROUGH-ROUND-POLE [bool]: a Ø6 hole coaxial through an R4 corner round's ball pole;
  repro vibex8 ~/agents/asm-keep/t-mini-false.sh with the current CLI ~/agents/asm/target/debug/noshape-cli).
- Others': robot-relations BOM 36/42 (dx); scissor-lift Top <3>; psui coplanar depth tie.

### isaac-ball connector cases (traced run 07:26, 98/129 mates)
- Live fails them too (matched): Revolute 10, Fastened 6, Revolute 15, Fastened 16, Ball 16 (7 untranslated connectors).
- GearLab "#desc" studios 1e81ca / 90f78b (3 connectors; Spiral Bevel BOM rows): FS decision with the lead.
- Our 7 "cannot resolve" mates are on parts whose geometry differs ("volume fits []"):
  - a04711 (Revolute 6, 11, Ball 13): Extrude 4 REMOVE non-manifold (Part 2 at 2x live's volume) -> bool.
  - a2e4a2 (Ball 7, Fastened 33): Chamfer 2 fixed by bb667320d (blend); Chamfer 4 still fails.
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
- Live data: REQUESTS.md (times from `date -u`).
- Agent ids: only /root/lead-scratch/roster.md, read right before every message (lead rule 11-19); never copied here.

## Assembly model suite (what it does)
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over ~/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES).
  `rebuild_assembly`: Part Studios by the translator, each level (subassemblies first, each its own "Sub <eid>" tab)
  inserted, fixed and placed as live's occurrences, mates between the same connectors; top-level instances held at
  live's places, compared, freed (our mates mustn't move them); mate status, BOM rows (parent, depth, name, qty, part
  number, description, material) as live's.
