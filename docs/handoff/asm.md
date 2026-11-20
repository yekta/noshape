# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-20 05:00 UTC)

### In flight
- Queued 04:55: 5bfaa556 (feel A1b/A2, ASM-INSTANCE-PROPS-FIELD, Ball start turn (fuzz 76/80, R3), mate preview
  colour PICKED, tickets). NEVER run land.sh while it is queued. 8a6d26c4 landed 04:49 as c6c38a39.
- Uncommitted (land after 5bfaa556):
  - ASM-FLEX-SUB-RIGID (P1, live flexsub-1120): eval.rs `prepare_subassembly` keeps a sub flexible with linear /
    circular patterns and Tangent mates (mirror, part pattern, replicate still rigid); the flex-sub loop adds its
    Tangent mates (`tangent_pair((sub, prefix), ..)`) and its patterns' followers (`pattern_steps`, shared with the
    top level's Linear / Circular). Test tests.rs `a_flexible_subassembly_keeps_its_patterns_and_tangent_mates`.
  - SNAP-INSERT-FACE (P1, reopened by qa): live 1704's crops show the centroid is the only dot in reach at A and none
    at B (no snap, placed without a mate), the same nearest-within-reach rule as ours; T1 test
    `a_snap_insert_onto_a_plain_face_takes_its_centroid_and_turns_about_x` (assembly_ui_tests.rs: centroid target,
    180° about X, T (-50, 20, 10), Fastened 1).
  - ASM-NEW-MATE-PREVIEW-GROUP: dropped a stale "[ ]" duplicate line in TICKETS (verified [x] row stays).
  - ops tests/parity.rs: live's composite parts (metadata partType "composite"; live's lines never list them) join
    the live parts list with their mass.json band and centroid (infento 1dc32b's 15 unplaced instances); needs an
    infento run.
  - parity.rs nested Derived sources as Arc (`fetched_derived_sources`) + big_assembly_perf.rs
    `suite_assembly_open_profile`: commit with the scissor rerun.
- Running: isaac open profile on vibex8 (waiting for the slot since 04:41; output ~/agents/asm-keep/isaac-profile.out)
  -> top 5 studios / features with ms to the lead.
- FEEL-ASM-DRAG-START: live drag-delay-1120 withdrew A1b (2 px / 50 ms, whole delta; the 0.6 s was software GL + round trip): 15 px reverted to 2 px (uncommitted, test renamed _past_2_px_); qa told to re-walk DEAD-ZONE after 5bfaa556.
- Next: (1) profile report; (2) scissor traced rerun with the Arc fix (4ce5c7 / 5aae43 / fed22d / 57725f should
  build); (3) infento composites (below).

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
