# Core modelling (agent "core"): current state

Updated 11-16 05:23 UTC. History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## In flight
- **Impeller speed P0 (qa 11-16 04:40): waiting on blend.** Delete face 2 3.9 -> 26.4 s from v0.4.48 to v0.4.53
  (release A/B beside asm's full7: a hint; qa reruns it quiet). Chamfer 26 now builds (Error -> Ok, as live), so OCCT's
  Defeaturing gets a bigger part (trimming faces 13.2 s, FF 6.4 s, EF 6.1 s). Root: Part 1 differs from Fillet 28 on
  (ours 34/89 vs live 35/100; Fillet 29 fails, live 39/111; both hang on Fillet 3 (27), blend's next item), so the face
  "Sketch Stroemungskanal" is imprinted on differs: our sketch's circles are split (4 regions) but Extrude impellerraum
  adds 3 faces (live 20) and Extrude 6's ring is 4 faces (live 21, same 2337 mm³). Move face 1's 4 SWEPT_FACEs of the
  ring's split sides don't exist in ours: it fails ("Could not offset ...", 8 s in the reblend's defeature) and Delete
  face 2 defeatures a part live never has. Turning the fuse's unify off changes nothing there. blend tells me when
  27-29 match; then recheck from Extrude impellerraum (the probe now prints a sketch's regions: NOSHAPE_PROBE_SKETCH).
- **Gate running (core-trace, core-tooluni aa92b1899):** an Add whose tool misses its merge scope (eval.rs "fuse
  merged nothing" branch) unifies the tool (`unified_tool`) as Parasolid's union returns it: asm's welding 9bdeb5
  Extrude 4 7/15/10 -> live 6/12/8, m9b 8/24 -> 24/24. NEW keeps split faces (live's impeller Extrude 6 does). T1
  features_a::an_add_that_misses_its_part_makes_a_part_whose_collinear_sides_are_one_face. Suite pair on vibe5
  (/root/core-scratch/gate-tooluni.sh, log beside it): land if only m9b-like changes.
- Landed 11-16: 41c146a8 (Routing curve fit), 48beead66 (parting-line draft), ee16972d8 (INTERNAL edges hidden; DECISIONS
  e677ec1c0), ce0e6ae8 + 601df0ac (failed Shell: error shapes, FeatureState::error_parts), c2fd0ca7 (corner break
  takes a side whole), 7629ea25 (Offset surface sheets at a corner as one), 09c11d4f (path loft through middle
  profiles: c2-loft-guide-curves-wave-washer Loft 1), f7a8d4d0 (loft to a sketch point, Tangent to profile: a dome in the
  point's plane; dx reruns paths-guides and wave-washer once its Sketch 2 plane fix is on main).
- **Parked: knife Revolve 1's 19th vertex (branch core-pole-v2 in core-bis, not landed).** Our sphere's pole there sits
  where live's vertex is, (114.5, 0, -12.56), inside the dish. "Pole shown where the face has a boundary" regressed
  cf-box-dovetail-joint, prize-wheel-tim, cf-surface-pattern; "boundary not only circles round the pole's axis" still
  regresses prize-wheel-tim's knob (our sphere's poles lie on a horizontal axis, (±4.9, 0, 24.5): our sphere frame,
  not live's) and cf-surface-pattern (259 -> 344 vertices). Needs live's surface type of the knife's dish
  (SPHERE or a spun B-spline: Parasolid may keep a vertex where a spun profile meets the axis) before another try.
- Probe (uncommitted, /root/core-scratch/bodies-probe.patch): ops parity::bodies_after_feature_probe (+ NOSHAPE_PROBE_SKETCH: a
  sketch's regions)
  (NOSHAPE_PROBE_DOC = a copy WITH its -wal / -shm, NOSHAPE_PROBE_AFTER, NOSHAPE_PROBE_OUT, NOSHAPE_PROBE_PART,
  NOSHAPE_PROBE_TAB): bodies as .brep, a part's faces (surface, edge indices), edges and vertices with shown flags.
  Saved documents' part names are Part N, not live's.
- Landed 11-15: 654611f6f / 8e3d52063, 52433a3d2, 348b6e241 (Kerf: offset retried in mm when empty), 9012904e (Shell by
  regions: rough check, convex pockets).

## Waiting on others
- **GearLab 90f78b:** with ae8b7f751 plus asm's unlanded 93346d03d (custom features with a `def` map; std 1777/1793)
  the loft builds; Gear Lab 1 then fails at an opFillet on the 157-face toothed solid ("Failed to fillet
  selections."): blend's. 4.3 s vs live 2.4 s, mostly 2 Booleans (1.7 s, bool's) and 1.2 M edge_point_at calls
  (0.53 s; not looked at). Repro vibe7: `NOSHAPE_FS_TRACE=1 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/m90
  <ops test binary> parity::parity_models --ignored --nocapture`.
- **1dc32b (asm-infento-subs):** Draft 1 = live on dx's corrected input (-1306.543 vs -1306.54 mm³; OCCT 0030 +
  draft work bound landed af81dda05). Left: Fillet 2 / 3 / 12, Chamfer 3 (blend's); Extrude 42 17.6 s (bool's
  Boolean); Mirror 2 2.2 s (unify, bool2's). Hole 4-6 time: f1f212d03 above.
- **c2-impeller-loft:** first difference feature 15 (Boolean Flansch+Gehäuse 19/53/36 vs 19/56/39) is the fuse-then-unify
  merge (eval.rs fuse_targets -> unify_same_domain): bool takes it (11-15 22:40). Move face 1 / Delete face 2 speed only
  after that (they act on a part live never has). OCCT's Defeaturing dumps vibe7 ~/tmp/core/imp/dd.
- **asm-bjr-config 5d8bc1:** Sweep 1 landed 72248180. Kerf Compensation 1: live's corpus line is Manufacture=true
  (3 plain Ø2.2 holes, 3704.237 -> 3745.156 mm³, offset faces 454.835 -> 454.462 mm²); at false live leaves the part
  unchanged; ours offsets at false (dx's translation). Check our offset against the true numbers once dx has that input.
  Shell 1 is dx's. Extrude 55 (3.8 s vs 333 ms) is bool's fuse / unify.
- **e53bba:** bool's slit re-cut; then re-test the band-end patch vibe5 ~/agents/core-keep/e53bba-band-end.patch.
- **c2-hunting-knife feature 24:** live's own lines disagree (16-23 LINE, 24 OTHER, no geometry change); live asked
  in REQUESTS.md (11-15 00:45) to refetch 15-24 in one session.

## Next, in order
0. Impeller Move face 1 / Delete face 2 after blend's Fillet 3 / 28 / 29 (In flight). Left from 11-16's asks: c2-sm-hem-plus Derived 1 6/12/8 vs live 14/36/24 (after Offset
   surface 1, now one sheet); c2-sm-side-cuts-tab's next difference is a Hole (13/18 after the corner break).
1. Knife's dish vertex: asked live in REQUESTS.md (11-16 01:32) for the face's surface type and its edges / vertices;
   then retry core-pole-v2 (Parked above) on what live says.
2. qa's speed list (P0), what is mine: c2-valve-config Extrude 3 `prism_until.split` 131 ms (a logo up to one revolved
   dome face: OCCT's splitter, 26 face-face intersections; OBB and a (u, v) patch of the dome tried 11-15, no gain; a
   projection-built extrude would be a big change for ~100 ms; valve's other time is bool's / bool2's); c2-rings-loft: Circular pattern 1 913 ms vs live
   767: its 51 prism_until calls (302 ms) seem to run in parallel, bool's Boolean (892 ms) is the path; Shell 2
   `regions.tubes` 55-65 ms (Common of the solid with the hole tubes + fuse) and prism_until (~35 ms a call up to a
   1 m² cylinder face). Not mine there: rings Shell 1 (bool's quadrics), Circular patterns and Extrude 5 / housing
   Rotieren 1 (bool's Rust Boolean), Fillet 1 / Chamfer 1 (blend). The suite's per-feature part checks cost seconds of
   area / volume on offset faces (impeller's -3 mm wall 2.8 s); that is harness time, not regeneration.
3. psui's Move boundary: Up to vertex / face land for picked straight edges (81a182b14); other edge kinds and Up to part
   still open.
4. SHELL-SLOW tail after pockets: hang402 Shell 5's cost is the corner patches' slab fans (archive "Next, in order" 2);
   rounded blocks with pockets still decline regions (an outer fillet's sliver isn't a convex pocket); c2-bottle-split
   Shell 1 declines ("a pocket face isn't a plane or cylinder") and runs bool's quadrics (bool's self-interference
   filter halves that).
5. Live specs not implemented: revolved cones framed at their apex, tori / spheres at their centre (`reframe_cylinder`
   covers cylinders only); extruded circle edges on the sketch circle's frame vs our seam at u = 0; extrude side faces'
   x along their bottom edge (check).

## Parked
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
