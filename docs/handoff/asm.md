# Handoff: asm (assemblies), current state

Area: crates/assembly, crates/assembly-ui, the assembly ops in crates/ops and their e2e tests, the asm-* ui-parity
scenarios, the assembly parity suite (`parity_assemblies`) and the parity translator (crates/ops/src/parity.rs) for
the assembly corpora's studios. History: docs/handoff/archive/asm.md. Rulings: docs/DECISIONS.md.

## State (2026-11-15 23:59 UTC)

### In flight
- Worktree /root/projects/noshape-wt/asm (detached on 93346d03 = landed fd98e8a6), on top (none landed):
  - 60b13cba (**decision with the lead**): `regions_by_sum` takes the closest group of live's sketch regions.
  - 70ae7e9b: in-context bodies from live's `<element>.context.jsonl` (a planar one-face sheet as its face, wires and
    curved sheets as 3D edges, splines through live's 10 points; an in-context sketch on live's planar face). 1b88ff 6
    -> 12/12 features, a2e4a2 7 -> 9/64.
  - 154e6c65: an assembly's Save view / Delete view is no undo step (psui's FYI).
  - 08d6feee: configured refs.jsonl lines (`configuration`): the studio's build configuration's; instances by our
    part's body id. config-assembly 21 -> 25/25 occurrences, 14 -> 18/18 mates.
  - 07f3351a + 0c676223 + fd230597: BOM scoring against live's bom.json (rows matched by parent and fields; "Part N"
    renamed to live's names; standard content gets our catalogue's material; our "Sub <eid>" rows named as live's in
    the comparison only: naming the tabs broke keyboard-pattern, s38).
  - cde9262e: a mate live leaves unresolved (mate-messages.json message or red connectors) that ours solves counts as
    live, listed as "live defect" (DECISIONS 11-15).
  - 40205609: an in-context POINT body (context.jsonl) imported as a point body (keyboard 2bb133's Plane 1).
  - 668e761a: mate status = mate-messages.json's liveStatusNow / status over the older featureStates.
  - f4159c6d: a Derived picking its source's sketch brings it in; regions live names as the Derived's found in it; an
    empty `foreignId` takes blobData's file. keyboard-pattern (s47, with live's new refs) 20 -> 21/25, BOM 13 -> 15/16.
  - 913fad5f: a sketch instance (live's type "Feature") inserted as ours of that sketch (T1; keyboard's three need
    studio a66ca5, requested from live).
- Runs on vibe6 (queue g49/run.sh, each through ~/bin/slot, status in g49/status): s41 full suite (to 668e761a)
  running (full/s41b.log; 9 of 24 at 23:24, equal to base29 but config-assembly 25/25, 18/18); then the model gate
  base29 vs s49 (g49/models-*.log); then the full suite with s49 (full/s49.log) for the SCORE row. Then land.
- Tried and reverted (s48): keeping a sketch moved by a configuration variable's dimension (20e932's
  #WidthAcrossFlats, stored at the workspace's configuration). config-assembly fell 25 -> 23/25, the Revolves still
  fail. Not the cause; 20e932's Revolves stay open.
- Asked live (REQUESTS.md 23:25 to 23:5x): (1) every studio's part metadata (studios/<element>.metadata.json) for the
  BOM's part number, description and material (clamp-pattern 2/14, gearbox-std 2/8, hand-drill-pinslot 0/20 rows);
  then the translator sets them with part_set_properties / part_set_material. (2) refs for non-sketch features of
  keyboard 8c3f79 (Plane 1's importTag edge) and df4a26 (Extrude 1's endBoundEntityVertex, 49 of ours). (3)
  config-assembly d2ecd2's Feature Studios (setData, custDesc not built: #SocketSize). (4) keyboard studio a66ca5.
- keyboard 2bb133 left: Sketch 2's Use (projected) entities from the derived sketch aren't drawn ("clicks didn't
  complete a line"), 20 PROJECTED find no model geometry; Extrude 3, Chamfer 2, Transform 1 after it.
- dx lands a configured-suppression translation (parity.rs: Step.suppressed, `__liveConfiguration`). It touches the
  Translator constructor line and the step loop near mine: whoever lands second rebases.

### Queued to land
- Nothing. Landed today: 790ecad5, 36ebccbc, 163918c9, fd98e8a6, 8ffdb51e / 72805bc5 (this file).

### Decision with the lead
- 60b13cba: isaac-ball 113 -> 116/121 occurrences but as-live mates 97/103 -> 89/121 (old data). The full run base29
  vs s37 shows it with the rest.

### Next, in order
1. Read full/base29 vs full/s41 per assembly (occurrences, mates; BOM rows for s41); land the commits (60b13cba per the
   lead); a SCORE.md "Assemblies" row (the last is 11-08).
2. BOM: standard content's material (insert standard-content instances as our catalogue's, or set live's material);
   then the rows that differ most in the full run.
3. a2e4a2's Extrude 5: 2 extra edges (ours splits a 0.451 mm arc at a vertex of JUm's edges and adds a 13 mm line);
   live's JUm / Sketch 5 / Extrude 5 edges are in studios/a2e4a2….jum-sketch5.json (live 20:0x).
4. isaac-ball 1e81ca / 90f78b (GearLab): core's loft 55e517e0, then the opFillet (blend's). 3c5c58 Fillet 1: blend has a
   dump (vibe7 ~/tmp/blend/m3cd), third in their queue.
5. config-assembly 20e932: Revolve 1 / 2 still "do not intersect" (configured dimensions taken back by the translator).
6. live's mate messages (isaac-ball's 7 ERROR, infento's 23) when they land: compare messages, not only statuses.

### Others' items touching mine
- dx keeps infento (1dc32b, 74d3d6) and bjr-config (5d8bc1). dx: derivedFrom + ORIGINAL_DEPENDENCY (top-level EDGE)
  queries resolve to faces / sketch regions in the translator; dx covered 1dc32b's Chamfer 4 / Fillet 5 with live's
  refs, not a general fix: tell dx before touching that path.
- core: 1dc32b's Chamfer 3 (dx) brings back 14 infento occurrences; Hole 2 (2cc476fe) is core's.
- blend (20:3x): 3c5c58 Fillet 1's input matches live; OCCT fails on the 14 edges together, the by-groups fallback splits
  two rounds; third in blend's queue.
- dx (19:3x): assembly studios take the Feature Studios in studios/<element>.fs their sources import (bjr-config
  5d8bc1); lands near my contextEntities loader lines in crates/ops/tests/parity.rs: whoever lands second rebases.
- psui FYI (11-12): saving a named view is no Undo step in live; the assembly's `self.edit("Save view : …")`
  (assembly_view.rs) still makes one. Not done yet.
- Lead ruling 11-15: don't copy live's unresolved-connector failure (DECISIONS.md).

## Scores (last measured)
- Assembly suite, same-base A/B (vibe7 ~/agents/asm-keep/s01): isaac-ball base29 113/121 occurrences, 97/103 mates
  as live; config-assembly 18/18 mates; suspension 6/6 mates; infento-subs 144/167 occurrences, 176/182 mates (61 -> 3
  parts our mates move off live's place); corexy, xdev-subs, clamp-pattern, robotic-arm, bjr-config,
  hand-drill-pinslot, keyboard-pattern unchanged by 36ebccbc.
- Model gate (all models, ops-s28 / s29): 76/87 documents, 881/947 features, custom 103/107 at base24; 80/87 and
  895/947 at s29 (the gain is main's hole fixes via the rebase).
- asm-* ui-parity: 213/214 (only live's crashed asm-cylindrical-limits take).

## How to run
- Builds and crate tests: on the Mac, `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p noshape-ops --test all
  parity::<name>` (one slot; rb.sh waits). No suites, ui-parity or app runs on the Mac.
- Suite binaries are Linux builds made on a vibe host: `NOSHAPE_RB_HOST=vibe6 tools/tiers/rb.sh bash -c
  'CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo test -p noshape-ops --test all --no-run >
  ~/agents/asm-keep/build-sNN.log 2>&1; f=$(ls -t ~/agents/asm/target/debug/deps/all-* | grep -v "\.d$" | head -1);
  cp $f ~/agents/asm-keep/bin/ops-sNN'`; copy between vibe hosts with `zstd -c | ssh … zstd -d`. Never cut cargo's
  output with `head` (SIGPIPE stops the build): log to a file, grep after.
- Heavy runs (assembly / model suites, gates) through `~/bin/slot <cmd>` on vibe6 / vibe7 (one per host, waits for
  10 GB free), never vibe3. Long runs started over ssh with `setsid nohup bash script.sh > /dev/null 2>&1 < /dev/null
  &` (systemd user units get oom_score_adj 100+ and earlyoom kills them first; `EXIT 143` = earlyoom, see
  `journalctl -u earlyoom`).
- One assembly: scripts in vibe7 ~/agents/asm-keep (isaac29.sh, cfg29.sh): `NOSHAPE_PARITY_TRACE=1
  NOSHAPE_PARITY_ASSEMBLIES=<dir with one symlinked corpus> ops-sNN parity::parity_assemblies --ignored --nocapture`.
- One studio as a model: `<dir>/<name>/` with features.json, live.jsonl, sketches.json, mass.json, refs.jsonl
  (copied from the corpus' studios/<element>.*) and a `studios` symlink to the corpus' studios; then
  `NOSHAPE_PARITY_MODELS=<dir> ops-sNN parity::parity_models --ignored --nocapture` (ma2 = a2e4a2, mb1 = 1b88ff,
  m90 = GearLab 90f78b, m3c = 3c5c58).
- Model gate: `NOSHAPE_PARITY_MODELS=$HOME/parity/models ops-sNN parity::parity_models --ignored --nocapture`, base
  vs change, per-model diff. The base must be built from the change's own base: land.sh rebases the worktree, so
  rebuild the base after a land (check out HEAD's files, touch, build, restore).
- Traces: NOSHAPE_PARITY_TRACE=1 (each feature as sent, external refs, region picks); NOSHAPE_FS_TRACE=1 (FS
  operations and why a custom feature failed).
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe5 NOSHAPE_LAND_HOSTS="vibe5 vibe3" tools/tiers/land.sh`; `--status`.
- Live data: ask in /root/onshape-research/live/REQUESTS.md (times from `date -u`) and message live
  (b89af730-e8c6-4234-b83e-00a9e1ba6acc). live's scripts: asm/q-isaac/refs.js (refs at a sketch's rollback,
  USE_SOURCE=1), fsfetch.js (imported Feature Studios).
- ui-parity asm-*: `tools/ui-parity/run.py --app <noshape> --display :105 <names>` on vibe6/vibe7 (≤ 4 apps).

## Assembly model suite (what it does)
- T2 `parity_assemblies` (crates/ops/tests/parity.rs) over /root/parity/models/asm-* (NOSHAPE_PARITY_ASSEMBLIES).
  `rebuild_assembly`: Part Studios by the translator, then each level (subassemblies first, each its own "Sub <eid>"
  tab) inserted, fixed and placed as live's occurrences, mates between the same connectors (root mates reach nested
  parts by path; instance ids are unique per level, so maps key on (level, id)); top-level instances held at live's
  places, compared, freed: our mates mustn't move them; mate status as live's. Not yet: patterns, groups, relations,
  mate limits, configurations, BOM.
- Connectors: implicit (MID_POINT / CENTER / POINT on a face with a second id = the edge / vertex), explicit Mate
  connector features (flipPrimary, secondaryAxisType, transform -> the mate end's flip / reorient / move; not when
  realigned to other entities), live's matedEntities CS paired with picks by occurrence path.
