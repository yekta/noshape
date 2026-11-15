# Core modelling (agent "core"): current state

Updated 11-15 22:40 UTC. History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## In flight
- **core-oc, branch core-shellcheck** (2 commits, not landed): 220142955 regions' volume check via
  `rough_volume_by_faces` (rings Shell 2 65 -> 9 ms a check); 3315dcd6d by_regions takes parts with convex pockets
  (block of supporting planes - pockets, each grown by t through by_regions itself, added as tubes; concave edges sharp
  as live's S4; pocket with R2 floor fillets 1.4 s / 41 faces in slabs -> ~75 ms / 29 faces, exact walls; T1
  features::a_pocketed_block_shells_with_sharp_concave_corners_as_live). Gates: g++-13 + shell tests ok; hats/dock +
  47 tickets ALL-SAME; **bool's gate** (lead: Shell is bool's area) running on vibe5 (bool reports); suite pair: vibe7
  run-hbase.log done, run-hshell.log detached waiting in vibe7's slot. Land from core-oc after both.
- **core-bis, branch core-pole** (ee2b83779): a sphere's / revolved face's pole is a vertex where the face has a boundary
  (knife Revolve 1 dish: HANDLE 19 vertices as live; knife 46 -> 51/52). T1 seam::a_spherical_dish_keeps_its_pole_vertex;
  g++-13 ok. Suite pair: binaries building on vibe5 (`.gob.sh` pbase from core-cache at bd7c25f88 / pole), then
  `bash ~/tmp/core/pairrun.sh pbase ~/agents/core-cache/src pole ~/agents/core-bis/src` detached on vibe5; every
  document whose vertex count changes: check against live.
- **core-trace, branch core-kerf** (uncommitted): op_offset_faces retries in millimetres when OCCT's offset in metres
  comes back empty / invalid (dx's 5d8bc1 Kerf Compensation 1: 3729.04 mm³, 11 faces); crash fixture kerf-offset + T1
  crash_inputs::a_plates_sides_and_countersinks_offset_by_a_kerf; replay_one replays offsetfaces dumps. Gates running.
- **Queued:** 55e517e0 (Hole rays + curved-face loft), cb0b6aad (no-change note, psui).
- Probe kept: /root/core-scratch/bodies-probe.patch (ops parity::bodies_after_feature_probe: a saved document's bodies
  after a feature as .brep, a part's OCCT edges / vertices with shown flags).

## Waiting on others
- **GearLab 90f78b:** with ae8b7f751 plus asm's unlanded 93346d03d (custom features with a `def` map; std 1777/1793)
  the loft builds; Gear Lab 1 then fails at an opFillet on the 157-face toothed solid ("Failed to fillet
  selections."): blend's. 4.3 s vs live 2.4 s, mostly 2 Booleans (1.7 s, bool's) and 1.2 M edge_point_at calls
  (0.53 s; not looked at). Repro vibe7: `NOSHAPE_FS_TRACE=1 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/m90
  <ops test binary> parity::parity_models --ignored --nocapture`.
- **1dc32b (asm-infento-subs):** Draft 1 = live on dx's corrected input (-1306.543 vs -1306.54 mm³; OCCT 0030 +
  draft work bound landed af81dda05). Left: Fillet 2 / 3 / 12, Chamfer 3 (blend's); Extrude 42 17.6 s (bool's
  Boolean); Mirror 2 2.2 s (unify, bool2's). Hole 4-6 time: f1f212d03 above.
- **c2-impeller-loft speed (Move face 1 3.5-5.5 s, Delete face 2 11-16 s; live 26 / 12 ms):** they act on a part live
  never has (ours a 37-face Part 1; live moves Part 2's faces and deletes 2 of 65): the document diverges from
  feature 15 (Boolean Flansch+Gehäuse 19/53/36 vs 19/56/39) with untranslated sketches (Bohrung Flansch, Bohrung
  Ansaugflansch) and a missing fillet edge: dx's translation (told 11-15 19:15). OCCT's Defeaturing there: face 26
  alone 13 s ("unable to remove", then our region fallback), face 32 alone 3.8 s. Dumps vibe7 ~/tmp/core/imp/dd.
- **asm-bjr-config 5d8bc1:** Sweep 1 landed 72248180 (mitred prisms when OCCT can't sweep round the hexagon); dx
  retranslates. Shell 1 is dx's. Extrude 55 (3.8 s vs 333 ms) is bool's fuse / unify.
- **e53bba:** bool's slit re-cut; then re-test the band-end patch vibe5 ~/agents/core-keep/e53bba-band-end.patch.
- **c2-hunting-knife feature 24:** live's own lines disagree (16-23 LINE, 24 OTHER, no geometry change); live asked
  in REQUESTS.md (11-15 00:45) to refetch 15-24 in one session.

## Next, in order
1. Land core-shellcheck (above) after its pair; check 55e517e0 landed.
2. qa's speed list (P0), what is mine: c2-valve-config Extrude 3 `prism_until.split` 131 ms (a logo up to one revolved
   dome face: OCCT's splitter, 26 face-face intersections; OBB and a (u, v) patch of the dome tried 11-15, no gain; a
   projection-built extrude would be a big change for ~100 ms; valve's other time is bool's / bool2's); c2-rings-loft Shell 2
   `regions.tubes` 55-65 ms (Common of the solid with the hole tubes + fuse) and prism_until (~35 ms a call up to a
   1 m² cylinder face). Not mine there: rings Shell 1 (bool's quadrics), Circular patterns and Extrude 5 / housing
   Rotieren 1 (bool's Rust Boolean), Fillet 1 / Chamfer 1 (blend). The suite's per-feature part checks cost seconds of
   area / volume on offset faces (impeller's -3 mm wall 2.8 s); that is harness time, not regeneration.
3. asm's a2e4a2 Extrude 5 (32/74/42 vs live 32/72/40): the hole circle's 0.451 mm arc beyond Sketch 5's r 4.455
   circle crosses its frame's angle 0; the extruded side is split at the cylinder seam there (line + vertex) and the
   shown topology doesn't hide them. Repro vibe7 ~/agents/asm-keep/ma2 (save/a2e4a2.noshape); live in the corpus
   studios/a2e4a2e9fdb090f64ffab7aa.jum-sketch5.json.
4. psui's Move boundary: Up to vertex / face land for picked straight edges (81a182b14); other edge kinds and Up to part
   still open.
5. SHELL-SLOW tail after pockets: hang402 Shell 5's cost is the corner patches' slab fans (archive "Next, in order" 2);
   rounded blocks with pockets still decline regions (an outer fillet's sliver isn't a convex pocket).
6. Live specs not implemented: revolved cones framed at their apex, tori / spheres at their centre (`reframe_cylinder`
   covers cylinders only); extruded circle edges on the sketch circle's frame vs our seam at u = 0; extrude side faces'
   x along their bottom edge (check).

## Parked
- Knife Revolve 1's 19th vertex (Parasolid keeps a closed B-spline edge's start vertex; ours joins across our seam).
- Knife lofted handle: +17 mm³ smooth bulge between the guides vs live (v direction or live's section construction).
- E1 Govde (dock) 539/304 -> 538/303 accepted by the lead (bf26f8759's 0.5 mm seam line).

## How to run
- Builds / tests: `NOSHAPE_RB_HOST=<host> tools/tiers/rb.sh bash <script>` from the worktree (env prefixes don't
  reach the host: put them in the script). Mac: builds and crate tests only, one worktree of mine at a time.
- Heavy runs on a vibe host (suites, gates, pairs) through `~/bin/slot <cmd>` there (one per host).
- Suite pair: `core-base/.gof.sh <tag>` (copy into the worktree; `branch` as tag also builds the kernel with g++-13
  and profiles Lighten 1) builds the ops tests in release and runs parity_models into ~/tmp/core/g/run-<tag>.log.
  Base = `git merge-base HEAD origin/main` checked out detached in core-cache, same host, same session. Compare the
  per-document "features match" lines (SAME-PER-DOC) and the "speed:" lines.
- One document: `NOSHAPE_PARITY_ONLY=<doc>[,<doc>] NOSHAPE_PARITY_MODELS=$HOME/parity/models <ops-bin>
  parity::parity_models --ignored --nocapture` (prints each feature's time vs live); `NOSHAPE_PARITY_SAVE=<dir>` keeps
  the translated document; `NOSHAPE_KERNEL_PROFILE=1` prints each feature's kernel profile (includes the harness's
  own part checks; for regeneration alone profile the CLI's `feature list --doc` on a saved copy).
- Dumps: `NOSHAPE_DUMP_OPS=<dir>` (fillet, draft, shell, prism_until, defeature, project_along, unify ...); replay one
  with `NOSHAPE_REPLAY=<dir>/<name> [NOSHAPE_REPLAY_REPEAT=n] <kernel test bin> crash_inputs::replay_one --ignored
  --nocapture`; probes in kernel tests: features::defeature_dump_probe (NOSHAPE_PROBE_DEFEATURE), unify_apart_probe.
- Traces: NOSHAPE_FS_TRACE=1 (FS ops and their errors), NOSHAPE_PATTERN_TRACE=1, NOSHAPE_WORK_TRACE (draft work
  steps), NOSHAPE_SHELL_DEBUG=1 (shell path chosen), NOSHAPE_DEFEATURE_DEBUG=1, NOSHAPE_GUARD_TRACE=1.
- Private user documents (hats, dock, qa's tickets): only mode-700 copies in ~/tmp/core, deleted after;
  /root/qa-scratch/bin/userdocs.py compares two CLIs on them.
- C++: g++-13 build of noshape-kernel on vibe5 before landing (`CC='ccache gcc-13' CXX='ccache g++-13' cargo build
  -p noshape-kernel`); direct-initialise OCCT handles; never name a variable `near` / `far` (empty macros on some
  platforms); never build a gp_Dir from a vector that can be zero (OCCT 8: std::terminate); signal-handler state needs
  `std::atomic_signal_fence` after it is set.
- OCCT patches (patches/occt, newest 0030): rebuilding a library on a vibe host needs `-march=x86-64` (check objdump
  for ymm / vfmadd); installing it on hosts is the lead's (x86) / mine on the Mac (~/noshape-host OCCT, backups beside).
  OCCT loops must poll `noshape_should_break()` with a deterministic bound, never a clock.
