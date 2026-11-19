# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History (everything before 11-18 21:25): docs/handoff/archive/asm.md. Rulings:
docs/DECISIONS.md (speed counts only ≥100 ms and ≥2× on a whole user action, 9e5b481d4).

## State (2026-11-19 23:00 UTC)

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

### qa P1s 11-19 (queued 23:0x)
- ASM-DRAG-UNFIXED-GROUP: my eedeede50 free-drag path took any 6-DOF instance, so a mated group with nothing fixed
  didn't move; now `Prepared::moves_alone` (no mate, pattern or group). Test fails without the fix.
- ASM-LIMIT-TYPED-NO-PREVIEW: an edited mate's held values go through `eval::held_in_limits`. Test fails without it.
- Open, next: ASM-NEW-MATE-PREVIEW-GROUP (a new mate's preview moves only the moving part; live the whole group).

### Lead rows 11-19 22:2x (after infento / scissor)
1. isaac on a green with bb667320d: h1119 (built 17:25, has bb667320d) still fails the Chamfer 2 block (Fastened
   7/8/9/57/79/88, Ball 1/7, Fastened 33): only the feature's error line went. Traced run with the current tree on
   vibe2 (`one-save.sh ops-along asm-isaac-ball ~/agents/asm-keep/isaac-main.out`, document kept in
   ~/tmp/asm/save) for core's volume line; core told the first answer was wrong.
2. Speed: `big_assembly_perf::suite_assemblies_speed` (NOSHAPE_PERF_DOCS=<saved suite docs>) in tagged stash
   asm-suite-speed-1119 (c2bc6c6e), not run yet. Live's numbers (behaviours 21425 asm200, 30512 A200): drag
   0.4–1 update/s, insert 4.96 s, add mate 1.94 s, open 11.8 s.
3. Then the suite's next differences (xdev-subs, infento, scissor), P0/P1 first.
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
- Live data: REQUESTS.md (times from `date -u`); live2 d7f4a0c2-c793-48d7-b749-13dbbe1dd3a1, live
  e8549719-ef9a-42de-960c-e24b29f142a6.
- Agent ids (lead 21:4x, only these): lead 818534eb-f6b1-4a00-bb48-ebf0f7cdbd8a, core 08a98761-3bbe-47ce-a91a-32b208f5c6b2,
  blend 459f6c19-c460-4ebf-96ca-bf92455f2e1e, bool abdebc8b-6b39-4794-8390-eb02d6fa37e6, live
  349fe81d-db78-4931-8032-6dc3edc5d558, studios 91339202-41f1-4465-91f4-65ccd26ea1b1, dx 6cfc89bd-1196-43ef-9225-0014546fee1e,
  qa ac50e7f2-4677-4cab-a57a-fd068a0c07bf, psui 7e3f2635-05e1-47c0-8594-9dbc0e9a79d7, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6.

## Assembly model suite (what it does)
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over ~/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES).
  `rebuild_assembly`: Part Studios by the translator, each level (subassemblies first, each its own "Sub <eid>" tab)
  inserted, fixed and placed as live's occurrences, mates between the same connectors; top-level instances held at
  live's places, compared, freed (our mates mustn't move them); mate status, BOM rows (parent, depth, name, qty, part
  number, description, material) as live's.
