# Core modelling (agent "core"): current state

Updated 11-17 06:12 UTC. History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## In flight
- **11-17 06:1x:** core-next QUEUED f2baf491 (p5 clean, hats/dock SAME 14 studios). Lead: 6a59c4094 (point loft: wrong area
  reported Ok) NOT to land; split -> **core-sing2** (pushed: 484875fbb reapply cylinder, df608b315 bounded per-feature
  defeature, 67f32b275 Thicken guard + own T1). Lands after p6 (old core-sing; singularities expected = base) + its
  hats/dock A/B (building in core-trace). **core-pipe2** (on core-sing2: 3e106dca0 N-profile guided loft only without
  end/guide conditions, 829ac26ef intersection n1 x n2, f6aa8f16f parity pairs by type): p7 relaunched 06:1x vs core-sing2.
  Audit (lead): 3 suite guided lofts set conditions (pipe Loft, singularities Loft with bad singularity + Loft 3), all fail
  today, none silent. **P1 next after impeller P0: apply loft guide + end conditions** (live areas: singularities Problem
  loft 3425.04 mm2, Surface 3 3480.95; both run to a point profile, also not built yet).
- **11-17 04:5x f197 P0 (qa/lead): DONE, not core's.** rust-zero-solid-f197 Mirror 3 slivers bisected (debug CLIs vibe5,
  repro `bash ~/tmp/core/f197/rep.sh <cli>`): a38f06293 (all core commits) good, blend's ba7a8d755 bad; its checked_blend
  necks_cut hunk alone. blend reverted (2ff07685c) and re-lands with f197 as T1.
- **core-pipe2 detail:** pipe's Loft then meets "All guides should intersect": Loft profile 3 (half ellipse, PIERCE to
  both guides) on Plane 3 (Curve point on guide JHt at its end KHcD) sits 2.69 mm off the guide; live's stored
  ellipse (center (0, -14.97) mm) can't hold the plane origin: REQUESTS 05:01 (+ mutualtrim Plane 1). manualblend
  Sweep 2 5321.6 -> 8223.1 mm2 (live 8224.9; rest likely OCCT's approximate section). mutualtrim Sweep 1 = Sketch 2
  spline solve (told skasm 05:2x). wave-washer Loft 1: solid loft, 4 profiles, one closed guide (periodic): not built.
- **Impeller P0 (core-sing2):** bisect 0.4.52 -> 0.4.53 = Chamfer Stevenrohr Halterung building (as live); Delete face 2
  deletes stand-in faces. Move face 1 6.6 s -> 28 ms; Delete face 2 replay 16.6 -> 8.1 s (300K bound per feature,
  spent ones kept). OCCT 0034 dropped (/root/core-scratch/occt-0034-extension-cap.diff). Remeasure once blend's
  Fillet 3 / Stevenrohr zum Halter build; measure on the Mac. p6res/branch.all "(spent)" lines = removals past 300K.
- fan-blade Boolean 2: 56/139/84 with core-next (live 56/143/88): +2 edges +2 vertices per hole need live's x_t
  (REQUESTS 03:20, live-base.x_t). Probe vibe5 ~/tmp/core/probefan (src /root/core-scratch/brepq).
- Worktrees: core-trace (core-sing2 / core-pipe2; probe /root/core-scratch/probe-wip.patch), core-oc (core-next,
  queued), core-bis: docs lands.
- Impeller Extrude 4 model side: closed, nothing to fix. The 12 mm² sliver alone never meets the offset wall: even its
  surface over all of its own bounds only grazes the prism at the sketch plane (replayed dump vibe5
  ~/agents/core-keep/e4dump/prism_until-10, experiment patch /root/core-scratch/uptowiden-exp.patch). Live's Extrude 4
  is the whole region, which we match (impeller 27/48 on main after blend's d7f9264c9; first difference Fillet 3).
- rocker-enclosure-sm Extrude 2 (next): Sketch 4's four Use circles (PROJECTED, live's external edges at x 45-60 mm,
  not on our part) come out as two: Extrude 2 names one region twice and cuts 2 holes of 4. Sketch translation:
  written into docs/handoff/skasm.md (landed 11-16).
- c2-surf-workshop-britatap Touch screen split (4/22): our projected split curve (BRepProj section, a degree 7
  B-spline) ends 0.116 mm from the face's corner along the rim, where the sketch edge ends exactly; live ends it on
  the corner (2/6/5 vs ours 2/7/6). Kernel projection precision at a tangent end; parked.
- c2-surf-workshop-cpcurves Main pipe (thin sweep, 7/24): (1) Sketch 4's circle kept 18.8 mm off the path (translator
  take-back: in skasm's handoff). (2) Mine: live merges the faces swept along the path's three collinear lines into
  one per wall (6 faces; ours 10: a face per path edge). **Committed b173dc5bb (core-oc, NOT in the vibe7 pair):
  join_collinear_lines joins a sweep path's lines along one line into one edge (first piece's source)**: Main pipe's
  faces = live (only its box differs, the take-back). T1 features_a::a_sweep_along_collinear_lines_has_one_side_face.
- c2-surf-intro-examples Fill 1 (5/13, parked): 4 G0 boundary edges (2 fit splines, 2 sketch splines; they match
  live's lengths). fair_g1_fill returns nothing for them, so OCCT's plate (MakeFilling) balloons: 61911 mm², box to
  x 198 mm; live 23827.4. A Coons patch (GeomFill_BSplineCurves, CoonsStyle) gives 23669.5 (-0.66 %: outside the
  band), box inside the boundary's. fair_g1_fill declines because the edges share no vertices: ConnectEdgesToWires
  rebuilds them and the IsSame lookup (at < 0) fails. Matching by midpoint (/root/core-scratch/fair-fill-midpoint.patch)
  makes the fair fill run: 22754.7 (-4.5 %). Live's box bulges past the boundary (y -19..157.6 vs 0..152.4), so
  neither is Parasolid's G0 fill; needs a live capture of a plain 4-edge G0 fill (area, a mid-surface point) first.
- c2-surf-workshop-split Boundary surface 1 (6/12, parked): 2x2 profiles, defaults; ours 3803.55 mm², live 3762.61
  (+1.1 %). It goes through k::fill_surface (op_fill) but not its plate: a Coons branch for 4 G0 edges placed first
  changed nothing (edge count != 4 or Coons declined; not chased). Experimental coons_fill saved in
  /root/core-scratch/features-coons.cpp (full file, not a patch).
- c2-surf-workshop-loft Loft 2: 3-profile surface loft 7e-5 off live's area (interpolation detail); skipped.
- c2-hunting-knife Revolve 1 (46/52, parked): HANDLE 10/27/18, live 10/27/19. Our ring between the handle's B-spline
  face and the dome's sphere is two equal 41 mm edges (vertices at (119.65, 0, 2.99): the B-spline's seam end, and
  (104.36, 0, -27.20): the sphere seam's end), both hidden by the seam-join rule. Live's ring RvBB (81.385 mm) has its
  arc-length midpoint exactly on the first, so live keeps one vertex, at the sphere seam's end. Rule unknown (a
  Parasolid sphere has no seam); a live capture of a dome revolved onto a swept handle would settle it.
- c2-fan-blade-sweep Boolean 2 (48/53, parked; lead: core's): Subtract with Offset 0.2 mm on 6 picked tool faces
  (11 per tool after the pattern copies: the sweep's free-form thread flanks among them). basic.rs offsets only analytic
  faces (else the tool as is), so ours removes 54.7 mm³ less (56/139/84 vs live 56/143/88). OCCT's offset_faces on them,
  limit lifted: fails every time after 33-62 s (4 tools). Needs a free-form face offset (Geom_OffsetSurface faces
  trimmed against their neighbours' offsets) in the kernel: a sizeable item, not started.
- **Lofts (mine, from dx's list):**
  c2-loft-guide-curves-wave-washer Loft 1: Sketch 4's rectangles are 63.5 mm off live's (an external COINCIDENT to
  live vertex JHk/JH4 unresolved: in skasm's handoff 11-16), which explains the "83 mm off the ring" below; redo the
  closed-path WIP once skasm fixes that. 4 profiles along a closed path; live closes it (4/4/0, 2080.8 mm³). WIP
  /root/core-scratch/loft-closed-path-wip.patch closes it round the path but comes out −11279 mm³: two profiles sit
  83 mm off the 26 mm ring, so MakePipeShell's carried sections twist. c2-surf-workshop-pipe: guided loft of more than
  two profiles unsupported (a Gordon surface over n profiles: big kernel item).
- **Impeller speed P0 (qa 11-16 04:40): waiting on blend.** Delete face 2 3.9 -> 26.4 s from v0.4.48 to v0.4.53
  (release A/B beside asm's full7: a hint; qa reruns it quiet). Chamfer 26 now builds (Error -> Ok, as live), so OCCT's
  Defeaturing gets a bigger part (trimming faces 13.2 s, FF 6.4 s, EF 6.1 s). Root: Part 1 differs from Fillet 28 on
  (ours 34/89 vs live 35/100; Fillet 29 fails, live 39/111; both hang on Fillet 3 (27), blend's next item), so the face
  "Sketch Stroemungskanal" is imprinted on differs: our sketch's circles are split (4 regions) but Extrude impellerraum
  adds 3 faces (live 20) and Extrude 6's ring is 4 faces (live 21, same 2337 mm³). Move face 1's 4 SWEPT_FACEs of the
  ring's split sides don't exist in ours: it fails ("Could not offset ...", 8 s in the reblend's defeature) and Delete
  face 2 defeatures a part live never has. Turning the fuse's unify off changes nothing there. blend (thread 0e6e9942) tells me when
  27-29 match (blend's Fillet 3 needs f24c11cc on main to measure); then recheck from Extrude impellerraum (the probe now prints a sketch's regions: NOSHAPE_PROBE_SKETCH).
- Extrude 33 (asm-pattern-std 36b90f) P0: 150 ms on main (debug, loaded vibe5); re-time once asm's Boolean feature
  pattern fix lands (Linear pattern 1 still errors upstream).
- **Handed off 11-16:** c2-config-parts-screws memory (14.8 GB): Revolve 9's unify (usd_kept_volume -> OCCT UnifyFaces)
  on the invalid body Sweep 1's OCCT fallback left: bool takes Sweep 1's cut, bool2 the unify bound.
- **Parked: knife Revolve 1's 19th vertex (branch core-pole-v2 in core-bis, not landed).** Our sphere's pole there sits
  where live's vertex is, (114.5, 0, -12.56), inside the dish. "Pole shown where the face has a boundary" regressed
  cf-box-dovetail-joint, prize-wheel-tim, cf-surface-pattern; "boundary not only circles round the pole's axis" still
  regresses prize-wheel-tim's knob (our sphere's poles lie on a horizontal axis, (±4.9, 0, 24.5): our sphere frame,
  not live's) and cf-surface-pattern (259 -> 344 vertices). Needs live's surface type of the knife's dish
  (SPHERE or a spun B-spline: Parasolid may keep a vertex where a spun profile meets the axis) before another try.
- Probe (uncommitted, /root/core-scratch/probe-wip.patch): ops parity::bodies_after_feature_probe (+ NOSHAPE_PROBE_SKETCH: a
  sketch's regions)
  (NOSHAPE_PROBE_DOC = a copy WITH its -wal / -shm, NOSHAPE_PROBE_AFTER, NOSHAPE_PROBE_OUT, NOSHAPE_PROBE_PART,
  NOSHAPE_PROBE_TAB): bodies as .brep, a part's faces (surface, edge indices), edges and vertices with shown flags.
  Saved documents' part names are Part N, not live's.

## Waiting on others
- **c2-config-parts-nuts Sweep 1 (14/17, parked again 11-17 02:55; mine per bool 01:3x).** The tool's crest strip is
  a B-spline (1 x 11) exactly on the bore's cylinder (r 1.621113885844308 mm); OCCT intersects instead of seeing one
  surface, and its Cut removes more than the tool. Rust Boolean declines. Live 135.268 mm3 (29/59/32), target
  143.883, tool 11.888. WIP /root/core-scratch/sweep-cylinders-wip*.patch (op_pipe_shell faces_on_cylinders: strips
  made exact cylinders, unwrapped pcurves; ShapeFix_Shape makes it valid). Probe (vibe5 ~/tmp/core/probefan/nuts,
  src /root/core-scratch/brepq/nuts.cpp, dumps ~/agents/core-keep/nm-nuts-cyl): with exact cylinders the cut is
  still wrong (144.384 mm3, 18 non-manifold edges): each strip is ONE face spanning ~7 turns (u -21.3..23.6), which
  OCCT's Booleans can't place points on (they project into one period). ShapeUpgrade can't split it
  (SplitSurface::Init clamps a periodic range to one period); BRepAlgoAPI_Splitter by the seam half-plane gives 27
  faces but no separate solids. 11-17 03:15: per-turn split DONE in WIP /root/core-scratch/sweep-cylinder-turns-wip.patch
  (cylinder_turns: strips cut at u = 2 pi k, pcurves in [0, 2 pi], edges split via ReShape; wire reversed where the
  cylinder's normal opposes the B-spline's: valid with no ShapeFix). The cut is still wrong (155.4 mm3, invalid;
  TooSmallEdge, AcquiredSelfIntersection): the r 2.0 crest passes through the chamfer rims (r 2.0 at z 0 / 4.06).
  bool 04:3x: fixed on bool's side (bool-nuts b6466ab04: a turn face's helices 2.6e-8 rad short of a full turn were
  taken as its seam): valid cut, 42 faces, 135.267625 mm3 (live 135.268). After it lands (own gate): unpark the
  exact-cylinder patch and compare the doc's volume with live (tool axes 2-5e-15 off z: make them exact).
- c2-config-parts-standoffs Revolve 1 (13/32): Sketch 3 solved 4 x 22 mm, chamfers on the axis side; live 3 x 24 (Length),
  outer corners chamfered (sketches.json). Already in skasm's handoff (translator takes Sketch 3 back). Variables OK.
- **c2-sm-rocker-enclosure-body 15/28 (stable now):** Extrude 6's pocket is ~2 mm off live's and differently shaped
  (top face 89.76 vs live 66.95 mm²) because Sketch 6 isn't translated as live: its PROJECTED constraint finds no model
  geometry, its circular pattern isn't kept, and the solve (1.97 mm) is taken back. Sketch translation, not modelling
  (skasm's, paused until 11-17).
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
0. Impeller Move face 1 / Delete face 2 after blend's Fillet 3 / 28 / 29 (In flight). Left from 11-16's asks: c2-sm-rocker-enclosure-body moved 9 -> 15/28 between runs (A/B it before crediting).
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
