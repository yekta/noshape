# Kernel robustness (agent "kern")

Goal: every modelling operation that Onshape (Parasolid) completes must complete in noshape with the same solid
(valid, same volume, persistent face names), and fail with Onshape's message where Onshape fails. Ground truth:
Onshape's mass properties captured by the live agent (`/root/onshape-research/live/behaviours.md` → "Geometry
ground truth (kern)", built through Onshape's REST API with exact numbers; scripts in `/root/live-pw/kern/`;
requests in `live/REQUESTS.md`, lines starting `kern`). Build dir `/root/target-kern`, display `:106`. Scratch C++
experiments against OCCT directly (outside the repo): `/root/kern-scratch` (`make <name>`; `common.hpp`).

## kern (new account) 2026-10-21 19:00 → — state
- Pending-pattern-2 test (`pattern_fins::narrow_rib_feature_pattern_without_reapply_matches_onshape`) was already
  appended; it passes (2.1 s).
- **PERF1 root cause #1 fixed:** the 400-hole Extrude spent 115 of its 117 s in `find_region` rebuilding all sketch
  regions per referenced region (O(n²) arrangement). `eval.rs::resolve_region` now looks the id up in the sketch's
  stored `SketchResult::regions` (`noshape_sketch::regions::find_region_in`, new; `Sketch::find_region` uses it).
  Test `model/tests/big_plate.rs` (predecessor's, live "qa5 big plate": 400 holes / chamfer 400 edges / depth edit,
  each < 2 s). Before: holes 122.8 s (kernel cut 1.3 s), chamfer 5.5 s, depth edit 122.4 s.
- **Release blockers (lead 19:00) F8/F9/F26:**
  - Test `kernel_robustness::chamfer_and_shell_on_filleted_box_match_onshape` (live G46 numbers). Onshape's mass
    properties are approximate where fillets meet (G49 reports variance ±2.9 mm³; exact mitred fillet 44848.0752 vs
    its 44848.2156), so those volumes are checked to 0.3 mm³.
  - F8 chamfer 5: `op_chamfer`'s catch reports "chamfer distance too large" (→ "Distance of chamfer on face too
    large.") when the edge ends on a fillet cylinder of smaller radius.
  - F8 fillet after chamfer: `features.cpp::fillet_past_end_faces` (fallback when OCCT fails): fillet the part with
    the end faces removed (Defeaturing), then Common with the part. Probe: 44487.2158, 9/21/14 vs Onshape 44487.2085,
    9/21/14 (`/root/kern-scratch/g46/g46.cpp`).
  - F8 chamfer 2 after fillets (live G49 decoded): Onshape's blend patch (face JJC, a (1,3) B-spline) is ruled
    between the two fillet cylinders' sections by the vertical planes through the chamfer's top corners and the
    point where the fillets meet, rulings at equal cylinder angle; Onshape approximates the arcs by a fixed cubic
    (normalized poles (1,0), (1.0010623,0.2621644), (0.8924865,0.5217270), …) and trims by intersection.
    `features.cpp::chamfer_into_fillet_corners` builds the exact surface (rational) as part − tool (sewn tool:
    chamfer plane, section planes, patch). Probe `/root/kern-scratch/g46/patch.cpp`: 10 faces, valid, 44790.0752
    (2 mm) / 44833.5752 (1 mm); Onshape's x_t read by us 44789.9729 / 44833.5301; its mass props 44789.8502 /
    44833.4378. Face areas match to ≤0.007 mm².
  - F9 shell after those: verified only through the model test (OCCT's plain join shell returns the input).
  - F26 STEP: `kernel/src/step_out.rs`: OCCT writes each part (location baked, no assembly, no p-curves, metres),
    the file is rebuilt in Onshape's layout (G47/G48): one product per part (last first), names on product / shape
    rep / solid / context, colours in 1/255, AP214/AP203 context strings + PMI-validation properties, inch units,
    **entity numbering = ST-Developer's: types ordered by reverse first creation, creation order emulated (product
    data, units, context, SR placement, solid top-down with its style, faces surface-first, edges curve-then-vertices,
    ABSR, SRR), from #10**, line wrapping (refs/strings/enums pre-checked at col 72, enums indented, strings broken at
    col 70). G47's 3 cubes: same entity numbers as Onshape for the product layer. Header identity says NOSHAPE
    (not ST-DEVELOPER / ONSHAPE BY PTC INC; no ST-Developer comment): lead to decide. Tests
    `kernel/tests/export.rs::{step_layout_like_onshape, step_layout_variants_like_onshape}` (pass).
    `ops/src/export.rs`: STEP colours are the part colour's exact sRGB (was a 2.2 gamma).
- **F8/F9 status (10-22 after freeze):**
  - `chamfer_into_fillet_corners` now runs *before* OCCT (OCCT's retries "succeeded" with an 11-face solid). It
    accepts the fillets' mitre edge in the selection: it starts tangent to the chamfered edge, so tangent
    propagation selects it. Onshape's patch *is* the chamfer running on along that mitre. The patch is named from
    the mitre edge when it's selected. A chamfer ≥ the fillet radius returns "doesn't apply" → OCCT fails → "too large".
  - Test `kernel_robustness::chamfer_and_shell_on_filleted_box_match_onshape` (qa4's F8/F9 cases, passing):
    fillet; chamfer 5 = Onshape's error (live G46: Onshape fails too); chamfer 2 = 10 faces; shell 2.5 after it
    13946.59 / 21 faces; fillet after chamfer 5.
  - Open, `shell_details_match_onshape` (#[ignore]) + `shell_thickness_sweep` (ignored, prints):
    (a) shell 1 mm after chamfer 2: 6053.88 vs Onshape 6052.70: inner patch 3.891 vs 4.189 mm², inner fillet 1
    145.682 vs 145.437; (b) chamfer-then-fillet shell: Onshape's rim corner is one flat triangle at z=30 (the inner
    chamfer extended up to the open top), ours two sloped "extrusion" faces from BRepOffset (shell.cpp by_offset);
    ours fails at 1.10–1.25 mm and works ≥ 2.06 mm, Onshape works ≤ 2.05 and fails ≥ 2.06 (rule unknown; 7/(2+√2)
    = 2.0503 fits: rim-triangle leg t√2−1 = 2(3−t)). Live G50 data: dom/kern-g50-*-bodydetails.json,
    samples/g50-*.x_t (our reader: reorder file OK 5990.5231; the g46 one reads with broken offset faces).
  - `xt_read.cpp`: offset surfaces built without OCCT's C1 check (Parasolid offsets a C1-knotted blend patch).
  - Kernel test helpers: `export::report_xt` (NOSHAPE_XT, prints faces), `export::write_g47_step` (NOSHAPE_STEP_OUT).
- **Shell by regions (10-22):** `shell.cpp::by_regions` runs first for inward shells of convex solids (no concave
  edge per BRepOffset_Analyse) bounded by planes and convex cylinders, removed faces planar: cavity = bounding box
  − each plane's offset half-space − each fillet's sector beyond r−t (axial extent ± 2t). Probe
  `/root/kern-scratch/g46/cavity.cpp`: chamfer-then-fillet shell = Onshape's 20/50/32 at 1, 1.5, 2, 2.05 mm,
  volumes within 0.007 (= Onshape's bias; 1 mm = its x_t 5990.5231). Onshape fails ≥ 2.06 mm: rule unknown,
  live **G51** asked (thresholds for other chamfer/radius pairs).
- **IM4 (qa5 pass 12):** g50-g46-shell1.x_t imported as garbage (offset of Onshape's degenerate blend patch: edges
  through the apex row, where the offset normal is undefined, wander 23 km). Now: intersection refinement keeps
  chart points unless the refined point is near (100 × chordal_error or 1 % of chart spacing), and
  `check_rebuilt` fails a body whose edges leave its vertices' box (enlarged by its diagonal) with "parasolid: a
  curve of the file could not be rebuilt". Proper import open: rebuild such offsets as a fitted B-spline using
  the limit normals at the degenerate row (the offset apex becomes a curve).
- **Onshape mass properties carry error bars**: REST volume = [value, min, max]; g49 2 mm 44789.850 in
  [44786.647, 44793.054]. The "IM2-kind" +0.12/+0.09 (g49), +0.0023 (g48), +0.0035 (g50-reorder) are inside them;
  our numbers are the exact integrals (g50-reorder equals an independent exact construction).
- **G51 band (10-22):** `shell.cpp::in_onshape_mitre_band`: two equal perpendicular convex fillets meeting at a mitre
  under the removed face, a 45° chamfer face touching both, the fillets' corner P on the removed face at signed
  distance d ≤ 0 from the chamfer plane: Onshape fails for (2−√2)(r − d/√2) < t < r (rim-triangle leg c−2r+√2t =
  2(r−t)); fits all six G51 cases incl. (5,2) (d > 0: no mitre). `by_regions` handles t ≥ r (sector without
  cylinder). Tests: kernel `shell::chamfer_then_fillets_shell_like_onshape`, model
  `kernel_robustness::chamfer_then_fillet_shell_matches_onshape`. Still ignored:
  `shell_after_chamfer_patch_matches_onshape` (inner offset of the blend patch, 1.19 mm³).
- **F12 kernel half (10-22): fixed by the F8 chamfer** (psb's BREPs had OCCT's corrupt corner); engine F12 test
  tightened (must succeed).
- **PERF1 chamfer/fillet of many edges:** `boolean.cpp::blend_in_slabs` (declared in internal.hpp): ≥ 48 edges →
  split the part by planes in gaps between the edges' reach (2 × size), blend each slab in parallel, join on the
  shared cut faces, unify. Probe `/root/kern-scratch/plate/slabs.cpp`: 400 rims 4.3 s → ~1.1 s with glue + USD
  (join + merge_planar should be cheaper). Used first by checked `op_fillet` / `op_chamfer`.

## For the lead (2026-10-21 13:20): integration blockers fixed
- `cargo clippy -p noshape-model -p noshape-kernel --all-targets -- -D warnings`: clean (13:07).
- `cargo test -p noshape-kernel --test features`: 20/20 (13:18), incl. `booleans_multi_and_unify` (the merged
  faces' history; tries 12/13 snapshotted the tree before that fix). pattern.rs:967 type alias is in.
- Full kernel + model suites: running next; result below in "Lead fins".

## State (2026-10-18)

### Shell — done, every Onshape volume matched (S1–S5)
- New kernel op `noshape_kernel::shell_solid(solid, faces, t, ShellSide, ShellMethod)` in
  `crates/kernel/cpp/shell.cpp` (new file, listed in `build.rs`; bridge `op_shell_solid`). The old `k::shell`
  (MakeThickSolid) is unchanged; sheet metal Convert still uses it.
- Semantics = Parasolid's (verified): offsets of kept faces are **extended until they meet** (sharp at concave edges
  inward and at convex edges outward, no rolling-ball arcs), faces whose offset vanishes disappear, walls that
  overlap merge, the opening follows the removed faces.
- Two constructions, cheapest first, each checked (BRepCheck, ≥1 solid, `V(outer) − V(inner) = V(result)`):
  1. `BRepOffset_MakeOffset` of the whole solid (Intersection join, removed faces offset 0) → cavity, then
     `solid − cavity` (outward: `offset − solid`), UnifySameDomain. ~10–200 ms.
  2. Slab construction (when OCCT's offset fails: walls that overlap, offsets that vanish, split removed faces):
     every kept face thickened by t (planes: prism; others: `MakeThickSolidBySimple`, sign checked by
     classification, closed faces split at the seam), plus fillers swept along edges where offsets leave a gap
     (concave edges inward / convex outward; edges on a removed face up to the removed face's tangent plane),
     cone/cylinder bands on a removed face revolved as one section clipped at the axis (vanishing chamfer cones),
     vertex fillers (normal cone ∩ offset planes, `convex_polyhedron`), fused, `∩ solid` (outward: `− solid −
     half-spaces beyond planar removed faces`), UnifySameDomain. 0.1–1.5 s.
- Same history and topology from both (names don't change when the path changes, e.g. 1 → 2.5 mm): faces on
  original faces `Modified`, offset faces `Generated` by their face, the rim `Modified` from the removed face.
- Feature `regen/features/detail.rs::shell` calls it (qa3's join/ShapeFix workaround removed); failure message
  stays "Could not shell part with selections.".

### Booleans — Parasolid merges faces a boolean leaves on one surface
`regen/eval.rs::boolean_into` (Add/Remove/Intersect of every feature) now runs UnifySameDomain after the boolean
and names through it (only when faces merged). Onshape: a block added flush leaves one face (S4: the stepped
part's bottom and side are single faces). Cost: ~3–6 ms on a 36-face part (the cut itself 16–26 ms).

### Fillet / chamfer — failures fixed, some shapes still differ (see Open)
- Kernel: `fillet_checked`, `fillet_variable_checked`, `chamfer_checked` (`cpp/features.cpp`, `scaled_blend`):
  always a valid solid or an error. (a) Tried as is first (no copies; identical to the unchecked op on the happy
  path). (b) If that fails or isn't valid: at the **working scale** — `set_working_scale(1000)` (set by
  `Regenerator::regenerate`, model lengths are metres); OCCT's blend tolerances are absolute and tuned for mm
  parts, the same part in metres fails (R4 top loop over R3 corner fillets). Input scaled ×1000, blended, scaled
  back; history composed with `compose()` (BRepTools_History::Merge loses faces generated by edges of a modified
  copy). Scaling is NOT used when the as-is blend works: exact HLR lost fillet end arcs on scaled-back shapes
  (drawing test `demo_plate_views_have_expected_lines`). (c) Still failing: retry 1e-7/1e-6/1e-5 smaller (blends
  that consume a face exactly: R5 on a 10 mm block, a chamfer as wide as a corner fillet, R2.5 on 5 mm walls) and
  remove the sliver faces (`ShapeFix_FixSmallFace`). (d) An invalid input keeps OCCT's result (old
  `require_valid_blend` rule). Unchecked `fillet`/`chamfer` unchanged (sheet metal relies on them).
- Model: Fillet/Chamfer features use the checked ops. Chamfer wider than a face / an arc radius →
  "Distance of chamfer on face too large." (Onshape's text), other failures "Failed to fillet/chamfer selections.".
- Matched Onshape (≤1e-3 mm³): F1 R1/R2.9/R3, chamfer 1/3, chamfer 4/5 error; F2 R2/R4.99/R5, R6 and top-R12
  errors; F3 corner R3, chamfer 2, sequential x R2; F5 R2/R4.9. qa3's `blend_over_smaller_fillets_is_valid_or_fails`
  updated to Onshape's answers (5 mm fillet OK, chamfer message).

### Move face, sweep (live capture G1)
- Move face (Offset): when `k::offset_faces` fails or isn't a valid solid, `direct.rs::offset_by_booleans`: each
  face's slab (`face_slab`: thicken on the checked side; a cylinder offset past its axis = the filled cylinder) is
  added (outward) or cut (inward); a part that falls apart keeps its largest piece (Onshape keeps one part; equal
  volumes → the piece nearest the origin corner). Onshape O1 3607.3009 (x=40 face 20 mm in, through a hole), O4
  8000 (hole closes) / 2429.2037 (hole grown to R10 splits the block, x 0..20 half kept).
- Sweep: a result that self-intersects (new `Shape::self_intersects`, BOPAlgo_ArgumentAnalyzer, ~10 ms) fails with
  Onshape's "Could not create valid swept body, check input." (bend tighter than the profile: R2 bend, Ø6
  profile; R3 and R6 OK = Pappus volumes).
- G2 blends (pocket block, L bracket, fillet/chamfer all edges) all match Onshape (R2.5 on the L bracket within
  0.004 mm³ via the 1e-5 retry).

### Tests
`crates/kernel/tests/shell.rs`, `crates/kernel/tests/blend.rs` (metre-scale blends, history coverage), `crates/model/tests/kernel_robustness.rs` (shell cases, Onshape volume tables,
`blends_match_onshape`, `bench_kernel_ops` ignored bench: `cargo test -p noshape-model --test kernel_robustness --
--ignored --nocapture`). Full `cargo test -p noshape-model` green except other agents' in-progress unit tests
(`assembly::tests::sketch_instances_are_wire_occurrences_with_edges` at the last run; earlier
`flatten::tests::a_dome_flattens...`), none touching kernel ops. App checked on :106: QA3 case B pasted
(`write_kern_clips`) regenerates OK and looks like Onshape's part-studio/120 capture.

### Variable fillet — evaluator done, dialog params pending (for agent "ui")
`detail.rs::variable_fillet` (when `isVariable`): reads Onshape's `vertexSettings[] {vertex, vertexRadius}` and
`pointOnEdgeSettings[] {edge, edgeParameter, pointOnEdgeRadius}`, `smoothTransition` (off = linear law, sampled
densely because OCCT interpolates smoothly); the current plain `vertices` query takes the main radius. Edges are
filleted one by one (found again by name). **For ui** (reworking the Fillet dialog in `features/core.rs`, which I
didn't touch): replace `vertices` with the array `vertexSettings` of items { `vertex` (query1 VERTEX),
`vertexRadius`, `vertexOtherRadius`, `vertexFlipAsymmetric`, `variableRho`, `variableMagnitude` } and give
`pointOnEdgeSettings` items `pointOnEdgeOtherRadius`, `pointOnEdgeFlipAsymmetric`, `pointOnEdgeVariableRho`,
`pointOnEdgeVariableMagnitude` (Onshape's FeatureScript ids, /root/live-pw/kern/f4.js). Asymmetric, conic,
curvature, partial, edge overflow and smooth corners are still ignored by the evaluator.

### Round 2 (2026-10-19)
- **Draft** (`detail_a.rs::draft`, kernel `draft_wedges` in `cpp/shell.cpp`): Onshape's draft = each face parallel to
  the pull moves by h·tan(angle), vanishing faces disappear (verified exactly on D1 5°/30° both ways). OCCT's draft
  first; if it fails/is invalid, wedges (planar faces: prism ∩ half-box; cylinder bands: revolved section clipped at
  the axis) cut/added. Fillets touching drafted faces are rolled again (`blends_to_reapply`/`draft_reblend`: delete
  them, draft, fillet the support-pair edges in one op, old names kept): Reapply fillet off = only undrafted fillets
  beside the drafted faces (D3 7353.6194 exact), on = also the drafted fillets (7335.611 vs 7335.593).
- **Tangent propagation off** (OCCT always follows G1 chains): `blend_section.rs::section_blend` with `run_on` — the
  section runs straight on past ends that continue tangentially (F3: 7928.41/7898.847 vs 7928.43/7898.847). Also
  used when OCCT fails on a single straight edge. Lines between planes; arcs between plane/coaxial cylinder/cone
  (revolved); G7 asks Onshape about arcs.
- **Fillet options** (`blend_section.rs`, routed from `detail.rs::fillet`): conic (rational quadratic, w=rho/(1-rho),
  same tangent points as the circle of `nonCircularRadius`; Onshape within 0.13 mm³, Parasolid spline approximation),
  asymmetric (elliptic, w=cos(φ/2)), partial (Position/Offset/Entity bounds along the chain, flat caps; position 0 =
  the chain end with the smallest (x,y,z), Onshape's on a block), Width = chord 2R sin(φ/2) for any edge angle
  (hexagon exact). Smooth fillet corners and overflow off: same as Onshape (no change on G4 cases). Curvature
  control: errors until G5 gives the section's tangent positions. Chamfer: both Measurements and all types match;
  `directionOverrides` not evaluated yet.
- **Variable fillet**: Smooth transition = smoothstep pieces (Onshape G3 a/c exact within 0.02 on the law; OCCT's
  section adds ~1.6 mm³), off = linear (exact for two points; Onshape's law with points between unknown → G6 asks for
  sampled radii). Point positions from the edge end nearest the origin.
- **Mass properties bug fixed** (`common.cpp::volume_props`, used by `volume_properties` and the shell checks): OCCT's
  plain Gauss integration is wrong on rational NURBS (+0.9 % on a NURBS cylinder), the adaptive one on strongly rational
  extrusions (+4.5 % on a conic fillet prism). Both are computed; if they disagree a fine mesh volume picks.
- **Sweep**: twist (Turns/Angle/Pitch; `SweepOptions::twist`; auxiliary spine around the path from a rotation-
  minimizing frame, matched by normal planes) — direction of `ccw` to confirm (G9); OCCT pipe tolerances made
  relative to the path size (1e-4 absolute = 0.1 mm in metres). Self-intersecting results fail like Onshape.
- **Surface revolve** (`revolve.rs::surface`): wires through the solid revolve machinery.

### Round 3 (2026-10-19): live answers G5–G10 applied, exports
- **Twist direction** (G9): our default (`ccw` off = −90° about the path for a quarter turn) matches; test checks the
  lateral edge's midpoint (`kernel_robustness.rs::sweep_twist`).
- **Propagation off next to tangent edges** (G7): a straight edge's blend runs out onto the neighbour until the
  section leaves the part (`blend_section.rs`, `reach`: was a fixed 2·r, which cut a 1 mm chamfer's runout short);
  chamfer 1 mm 7904.4894 vs 7904.4852 (the exact prism solid is 7904.4847), fillet R2 7891.1580 vs 7891.1450
  (Parasolid's fillet runout takes 0.013 more: G11e asks for the seam). A curved edge continuing tangentially into an
  unselected edge errors "Adjoining edge not filleted." (`detail.rs::curved_edge_left_open`). Equal-distance chamfers
  without propagation use the section path too (OCCT's chamfer always propagates).
- **Variable radius law** (G6): smooth off = C1 cubic through the points, slope zero at a peak/dip, else the gentler
  neighbouring chord, zero curvature at the ends (`detail.rs::variable_law`, unit test on F4's samples within 0.003
  mm). OCCT's law is now always sampled densely (it doesn't interpolate even two values linearly: that was the old
  1.6 mm³ "section" difference). F4/G3 a–c within 0.07 mm³; G3(d) 0.56 (the end rule there differs: G11c).
- **Loft through 3+ sections** (G10): Parasolid interpolates sections with a natural cubic spline (Ø20 → □12 → Ø20:
  6125.607; OCCT's parabola 5917.6). `features.cpp::natural_loft` rebuilds each ThruSections face in place (topology
  and history kept): rows at the sections → natural spline in v (rows with differing weights pointwise, through
  polynomial approximations); pcurves re-attached; falls back to OCCT's loft if anything doesn't fit. Now 0.012 off.
  Two-section lofts match (a 0.027, b 0.001, cone exact). Triangle → square (c): Onshape 3920, we 4441.6 (G12).
- **Booleans merge faces** (round 2) broke `custom_features::query_param_extrudes_selected_face` (a boss flush with the
  cube's sides): test updated to Onshape's merged result (6 faces).
- **Parasolid export** (`cpp/xt.cpp`, `write_parasolid` / `read_parasolid`, app Export → Parasolid): text and neutral
  binary, schema 12006 from the published XT Format Reference (kept outside the repo). Analytic geometry as such,
  the rest B-splines (swept/revolved/offset surfaces converted with the same parameterisation, closed B-spline faces
  split); seams/degenerate edges of periodic analytic faces dropped; accurate edges where the curve lies on its faces
  within 1e-8 m, otherwise tolerant (trimmed SP-curves from the pcurves). The reader rebuilds bodies with the file's
  orientations (no repair): every test part round-trips exactly (rounded block, cavity, crossing cylinders, sphere,
  torus, cones), loft 1.1e-6, revolved spline 2.9e-6 (`tests/export.rs`). Onshape import check: live G13
  (`write_xt_samples` writes /root/kern-xt-samples). XT conventions not stated in the reference (to confirm by G13):
  B-surface vertices with v varying fastest; ELLIPSE carrying `sense` like every curve; fin SP-curves along the edge.
- **STEP options** (round 2 end): `write_step_with` schema AP242/AP214/AP203 + file unit; app wired (Yard → mm).
  Preprocessing options (ui2's request) not done.

### Round 4 (2026-10-19): more live answers, ACIS, Rhino, CI
- **Curvature blend section** (G11, exact): quintic B-spline, knots 0⁶ ½² 1⁶, control points in setback units
  (1,0) (1−0.3m,0) (1−0.6m,0) (u,v) (v,u) (0,1−0.6m) (0,1−0.3m) (0,1), u = ⅔(1−m), v from the midpoint at
  (1−m)/2; v clamped to 0.001 (m = 0.8). `blend_section.rs::curvature_profile`; the fillet no longer errors.
- **Loft vertex matching** (G12): polygon sections with different vertex counts get vertices inserted at the
  reference section's arc-length positions no own vertex is nearest to (triangle top edge split at its middle:
  3920 exact); `features.cpp::match_vertex_counts`, history composed through the split edges. Circles keep
  OCCT's polar matching (Onshape's too).
- **ACIS export** (`cpp/sat.cpp`, `write_acis`, app Export → ACIS): all 12 samples import in Onshape with
  matching volumes (G14).
- **Parasolid**: crossing cylinders fixed (closed non-conic edges split: an open trimmed basis may not start
  where it ends); the pointed cone still fails in Onshape: neither no apex vertex (1 fault) nor an isolated
  apex loop (no geometry) is right — G13c asks for Onshape's own pointed-cone x_t to compare.
- **Rhino export** (`cpp/rhino.cpp`, `write_rhino` / `rhino_brep_count`): OCCT → NURBS (exact) → ON_Brep with
  trims (seams, singular trims), units mm, versions 2–8. openNURBS is built outside the repo
  (`scripts/build-opennurbs.sh`, pinned commit; here /root/opennurbs), `OPENNURBS_ROOT` for dev builds; CI
  and releases always build it (`.github/actions/setup-opennurbs`, `NOSHAPE_REQUIRE_OPENNURBS=1` makes the
  kernel build fail without it). Link gotchas: openNURBS' zlib needs `zcalloc`/`zcfree` after it
  (`cpp/rhino_zmem.cpp`); `AddManagedModelGeometryComponent` double-frees (copy instead).
- **JT export** (`cpp/jt.cpp` + `cpp/jt_writer.{hpp,cpp}`, `write_jt`, app Export → JT): JT 9.5 laid out like
  Onshape's own files (decoded from G16 samples): LSG, per part a tri-strip Shape LOD (topologically compressed,
  lossless floats, fixed-width bit-length codec, lookup2 checksums) and an XT B-rep segment holding our Parasolid
  neutral-binary body (`parasolid_binary_body`; faces grouped in XT face order). zlib streams use stored deflate
  blocks (no zlib dependency). The writer is a C++ port of a Python prototype verified byte-for-byte on 16 meshes;
  the prototype with B-rep imported into Onshape as an exact solid (G19); our files: live G21. Spec digest and
  tools: /root/kern-docs/jt (outside the repo).
  **G21: all 12 test parts import into Onshape as exact B-rep solids with matching volumes.**
- **PVZ export** (`cpp/pvz.cpp` + `cpp/pvz_writer.{hpp,cpp}`, `write_pvz`, app Export → PVZ): decoded from Onshape
  samples (G15, G18): zip of a binary PVS product tree + per part an OL member (gzip sections: header/colour,
  per-face mesh, exact B-rep with analytic + NURBS surfaces, NURBS edges, pcurves). Laid out like Onshape's:
  closed faces split in halves, NURBS forms from OCCT's conversion normalised to [0, 1] with their pcurves,
  analytic records for planes/cylinders/spheres. The writer rebuilds all 15 Onshape samples identically after
  inflating (stored deflate blocks, no zlib dependency). Format notes/tools: /root/kern-docs/pvz. Onshape
  import: live G22 (prototype), G23 (our 12 parts). Unknown: where opacity goes (Onshape doesn't export it).
- **G20**: pointed-cone.x_t, sphere.3dm, torus.3dm now import as exact solids: every export test part imports
  in Onshape from Parasolid, ACIS and Rhino with matching volumes.
- Variable-law end rule (G11c): with smooth off, at each point one adjacent span is exactly linear and the
  other curves into it with the same slope; at the ends the chord slope, 1.5× next to a peak/dip (done, all 8
  Onshape cases within 0.1 mm³).

### Round 5 (2026-10-19, kern resumed on a new account)
- **Y Threads** (`custom_features::real_onshape_feature_y_threads`, un-ignored, passes): the helix's 3D curve was
  approximated to an absolute 1e-6 (1 µm in metres), enough for the tooth's edges on the core cylinder to wobble
  across it and break the union; now relative to the radius (`build.cpp::make_helix`). `sweep_sections` falls back to
  the Frenet frame when OCCT's corrected Frenet fails on a helix (the helix's Frenet frame is Parasolid's screw
  motion). Kernel test `tests/thread.rs` (core + clipped teeth = Pappus volume within 0.01 mm³).
- **G21 (JT volumes)**: our volumes are right: an independent divergence-theorem quadrature over the exact surfaces
  (`/root/kern-scratch/divvol.cpp`) gives loft 6125.618663 and revolved spline 1726.080764 = OCCT. Onshape's own mass
  properties of spline faces are approximate (its Rhino imports of the same exact NURBS show the same −0.055 /
  +0.018). What was ours, fixed in the XT writer (used by .x_t/.x_b and JT): (a) the revolved spline is now one
  **spun surface** face (XT type 68, periodic like Parasolid's own; was split in two B-spline halves → 4 faces);
  `exchange_prepared(shape, keep_spun)`; reader handles type 68 (round trip asserts face counts now). (b) OCCT's
  approximated intersection edges (crossing cylinders, 4e-8 off) get a curve **refitted through exact
  surface-surface points** (`xt.cpp::refit`, Newton on both faces) so they're accurate instead of tolerant (+0.032 mm³
  in Onshape). ACIS still splits the spun face (G14: 4 faces); G26 asks for Onshape's own SAT of the part to copy.
- **PVZ**: G22/G23 = Onshape imports PVZ as meshes only, and our writer's files import exactly like Onshape's. New
  **PVZ reader** (`cpp/pvz_reader.{hpp,cpp}`, std-only incl. inflate; `read_pvz` in io.rs): every part instance's
  tessellation (OL section 1), placed by the PVS tree, nodes within Parasolid's precision 1e-8 m welded,
  T-junctions split (`mesh_io.cpp::mesh_shape`); closed → solid, open → shell. Reproduces Onshape's import volumes
  of all samples to 1e-8 mm³ (block 7999.999463558, fillet 7484.089596687, shapes cylinder/cone/torus) and leaves
  Onshape's loft (face meshes 7.8 µm apart) open, as Onshape does (it shows a massless "Composite part 1"; G27 asks
  what's inside). App: `.pvz` in IMPORT_EXTS / `read_foreign` / Import feature / `FileKind::Cad`.
- **G8 corner — exact** (`cpp/vertex_blend.cpp`, `exact_vertex_blends`, called after OCCT's fillet in `op_fillet`):
  Parasolid's corner face where a loop filleted R runs over corner fillets Rc < R is a **rolling-ball blend of
  radius Rc between the two R cylinders**: the ball centre runs on the intersection of the two cylinders offset
  inwards by R − Rc (spine(θ) = q + l0 cosθ b + (R−Rc) sinθ n, q = where the R axes meet, l0 = distance to the
  corner axis), sections are circles of radius Rc normal to the spine, the sides are ellipses on the R cylinders,
  the face closes in a point at the top face's corner. Found from Onshape's STEP export of the face (G25: a
  rational B-spline, cubic along the spine angle 2π..2.5π, exact circle arcs across, weights cos(half arc)); our
  corner face areas match Onshape's to 1e-7 for all of G24 a–e and F1 R4/R6, section points to their 1e-4 mm.
  OCCT's patch (same topology) is replaced in place: new face (rational B-spline through 64+ exact sections, checked
  between them to 2.5e-8 R) and side/bottom edges with pcurves on the neighbours (ShapeFix_Edge::FixAddPCurve +
  SameParameter), all-or-nothing per corner, whole result BRepCheck-valid or OCCT's kept; ReShape history composed
  into the fillet's. Kernel test `blend.rs::vertex_blends_match_onshape`. Volumes: our volume is this geometry's exact
  volume (divergence theorem over the exact faces, `/root/kern-scratch/g8/exactvol.py`, = OCCT's to 1e-6); Onshape's
  differ by −0.04..+0.10 mm³ with varying sign: its mass properties of the procedural face are approximate (like the
  loft's −0.055 on an exactly known volume), so the area (1e-7) is the exact check. Gotchas met on the way: replace
  vertices as FORWARD shapes (TopExp::LastVertex is REVERSED; ReShape composes orientations → unclosed wires
  everywhere), give new vertices the old tolerance (OCCT's untouched edges end 1e-4 away), clear the SameParameter
  flag before BRepLib::SameParameter on an edge that got a new pcurve. Scratch: /root/kern-scratch/vb.cpp, g8/*.py.

- **compose() fix** (`features.cpp`): a shape removed in step 1 (a filleted edge) lost its *generated* images
  (the fillet faces) when composing with a second step; it never showed because blends composed only after a scaling
  step. Now its Generated images go through step 2.
- **Yard** (`LengthUnit::Yard`, 914.4 mm): STEP export in yards (OCCT has no yard: shapes written a third the size as
  feet, then the file's FOOT conversion unit becomes YARD 914.4 mm; read back exactly), app Export STEP "Yard" and STL/OBJ
  import in yards (was feet).
- **Sweep "Lock profile faces"** (was an error): `sweep_locked_to_faces` (kernel, OCCT spine-support mode: the
  trihedron's binormal follows the faces' normal; each path edge gets a pcurve on the face it lies on, on a copy of the
  path); model `solid_a.rs::sweep` uses `lockFaces`. A path off the faces fails ("the sweep path must lie on the faces
  to lock"; Onshape's text unknown — G33a). Kernel test `features.rs::sweep_locked_to_cylinder_face`.
- **Live requests filed, pending**: G26 (JT/XT recheck + Onshape's own SAT of a spun face), G27 (PVZ import composite
  details), G28 (offset surface with vanishing faces = old "O3"), G29 (STEP preprocessing reference files), G30 (chamfer
  two-distance face rule + directionOverrides), G31 (corner screenshots), G32 (loft end conditions/guides/path exact
  definitions), G33 (sweep lock faces, curve pattern normal-to-face, rib draft, body draft, face blend options).

**Round 5 state / next (kern, 2026-10-19 evening):** all of the above compiles and is tested: `noshape-kernel` full suite
green (blend 8, export 7, features 20, thread, shell, …), `noshape-model` full suite green, kernel clippy `-D warnings`
clean. In-app check of the F1 R4 corner done (/tmp/kern/f1r4-corner-noshape.png; side by side pending G31). Earlier note: in-app visual check of the F1 R4 corner (`write_kern_clips` now also writes
`noshape-clip-f1-r4.json`; paste with Ctrl+V) — the app build was blocked by another agent's `fs_reference.rs`.
Curve pattern "Normal to face" is now evaluated (`pattern.rs::curve_instances`: frame = tangent + the faces' normal
made perpendicular to it, mapped from the seed's frame) — to verify against G33b. Next, in order: apply G26–G33 as they
arrive (ACIS whole spun face from Onshape's own SAT; PVZ open-mesh → Onshape's massless "Composite part 1"; offset
surface vanishing faces; STEP preprocessing; chamfer two-distance face rule + directionOverrides; loft end
conditions/guides/path; rib draft; body draft; face blend asymmetric/conic/curvature and non-adjacent faces), then
sheet metal gaps (closed corners, side-face joins, loft into existing), slab-shell speed: profiled 19 Oct on an idle machine: `bench_kernel_ops` full regen 1.2 s (22 hole extrudes 0.56 s, shell 0.44 s, fillet 0.07 s); the 3–5 s seen earlier was machine load. Flat profile (BRepMesh, Gauss integration, malloc), no single hotspot; `perf` is installed (/usr/bin/perf; use `--call-graph dwarf`, frame pointers are off).

### Round 6 (2026-10-19 night): import readers, mesh bodies, native CAD, chamfer rule
- **Parasolid reader rewritten** (`cpp/xt_read.cpp`, the old reader in xt.cpp removed; `read_xt_parts(path)` /
  `read_xt_data(bytes)`; Rust `read_parasolid_parts`): text and binary, embedded schemas (Copy/Delete/Insert/Append
  edits against base SCH_13006), files without one (layouts per version 25.0–38.1, `version_schemas()`), bare
  partition streams ("PS\0\0…"), NX's '+' before the binary magic. Surfaces 50–54, 56 (rolling ball), 59, 60, 67,
  68, 124 (periodic knots handled); curves 30–32, 38 (chart + limits refined onto both surfaces), 133, 134, 137;
  assemblies (instances placed), SDL/TYSA names and colours. Schema tables generated: `/root/kern-scratch/xt/
  gen_schema.py` (base + reference corrections) **then** `gen_versions.py` (from versions.json, made by infer4.py
  from the G36 files) — run in that order, both rewrite `cpp/xt_schema.inc`.
  Found against real files, all fixed in the reader **and our writer**:
  - **Cone axis**: Parasolid's cone axis points the way the cone *widens* (Onshape's cone sample: apex at +10 mm,
    axis −z; NX chamfers agree; D2MI pin = its STEP to 1e-6). The XT reference's formula has the sign the other
    way; reader and writer both followed it, so round trips passed while our exported cones were wrong in
    Onshape. Now R = P + vA + (X cos u + Y sin u)(r + v tan a), Y = A × X; OCCT `gp_Ax3(P, A, X)`.
  - **SP-curves run along their fin** (NX's tolerant edges: the '−' fin's SP-curve runs v2→v1); trimmed curves
    take the direction from the *basis* curve's sense (parm_2 < parm_1 when '−'). Writer: a negative fin's
    SP-curve is written with sense '−', trimmed end→start.
  - Knot arrays padded with null knots (trimmed to n_knots), periodic B-curves (XT unclamped form → OCCT
    periodic), helix forms (163 HELIX_SU_FORM, 184), FIELD_NAMES (99), KEY (102) added to the base schema.
  - Blend surfaces: GeomFill_Pipe was 0.3 mm (and on cliff-edge blends up to 2 cm) off; now `tube()` = exact
    rational circle sections in a rotation-minimizing frame, rows interpolated along the spine (exact at stations
    r/16 apart). Intersection spines are marched past their chart (a blend on a blend reaches past it).
  - Healing: one edge/face that BRepLib::SameParameter or ShapeFix_Shape throws on no longer leaves the whole body
    unhealed (falls back to edge-by-edge / face-by-face).
  Corpus (harness `/root/kern-scratch/xth`, `make xt; ./xt files`; diag.cpp = BRepCheck histogram): all 50 G36 +
  G25 + Onshape cone exact; SolidWorks 2009–2015+ x_t/x_b; 75/75 valid. **Not yet:** Parasolid V9/V11 (pre-V14
  schemas: the 4 Unigraphics `nist_ug_*` files), cliff-edge blends ('E', fluxpad_102: 7 faces still bad, G38 will
  give Onshape's volume), blends other than constant-radius rolling balls.
- **Native CAD import** (`cpp/native.cpp`, Rust `read_native_parts`), the exact geometry the files embed, as
  Onshape's translators read them: **SolidWorks** .sldprt (OLE2 through 2014: `Contents/Config-N-Partition`;
  2015+: own section container — marker 14 00 06 00 08 00, checksum, sizes, nibble-swapped names, raw deflate; a
  section counts only if it inflates to its size; Config-0 else the first configuration), **Solid Edge** .par/.psm
  (`PARASOLID/STREAM0.P_B`), **NX** .prt (OLE `UG_PART/UG_PART` and SPLMSSTR files: text header, optional '+',
  zlib). Samples /root/kern-native-samples (public NIST/GitHub files): 20/20 SolidWorks parts (19 valid), 6/6 Solid
  Edge, 12/12 modern NX valid; NX/SW assemblies (`.prt` without bodies, `.sldasm`) report "no geometry" for now.
  Kernel test `export.rs::native_cad_parts` (skipped when the samples are absent). Deflate (`pvz_reader.cpp`)
  hardened against garbage input (puff's over-subscribed/incomplete code checks; was an out-of-bounds read).
  **Inventor** .ipt (OLE): the body is ShapeManager binary ("ASM BinaryFile4", an ACIS SAB fork) in the segment
  whose meta stream `RSeStorage/M<id>` names "PmBRepSegment"; its data stream `RSeStorage/B<id>` = header + zlib. Read
  by the ACIS reader: 10/10 samples valid (respira_back also holds a 50 L 10-face construction body — which bodies
  Onshape keeps is unverified). Pending: CATIA/Creo feasibility report, assembly imports, app wiring
  of the native extensions (sldprt/par/psm/prt/x_t/x_b/3dm in IMPORT_EXTS / read_foreign / importing.rs).
- **ACIS reader** (`cpp/sat_read.cpp`, Rust `read_acis_parts`, app .sat/.sab): text and binary (SAB tags; ASM
  variant), SAT 5.0–2x.0 record layouts per version (prefix fields as the InventorLoader reference reads them:
  ENTITY = attrib [+ id from 7.0]; topology/geometry + 1 field from 7.0, + 1 more from 11.0 except ASM), records
  placed by their "-n" numbers, counted strings ("n text" before 7.0, "@n text"), subtype refs ("{ ref n }", numbered
  in file order). Analytic surfaces exactly — cone: radius r + t·sine/cosine along the axis, normal outward when
  cosine > 0 (flipped by the cone's own sense), sphere/torus inward when the radius is negative; splines from the
  stored B-spline (exactsur; a procedural surface's approximation is the last surface-shaped B-spline at its level);
  intcurves from their 3D B-spline, or for "parcur summary" from the 2D curve on its spline/plane surface; a
  procedural face saved without data (vertexblendsur) is filled from its edges. pcurves are not read (healing projects
  them, `heal_import`, shared with the XT reader). Names (ATTRIB_XACIS_NAME / ATTRIB_XSTEP_PRODUCT_ID), rgb colours.
  Results: all 12 of our SAT exports back with exact volumes (test `export.rs::acis_import`), Onshape's cone exact and
  revolved spline 1726.08063, Cubit B3 (ACIS 24, 274 faces) valid, Abaqus hull sheet imports (1 face's loop
  orientation off). Onshape's G25-b SAT gives 7585.98 vs its XT's 7583.00: Techsoft's export put the corner-blend
  vertices 0.03 mm off as tolerant vertices (tolerance 0.042 mm) — we keep the file's vertices.
  Gotcha met: `gp_XYZ(num(), num(), num())` — argument evaluation order is unspecified (GCC: right to left).
- **Mesh bodies (G27, end to end)**: kernel mesh body = a face with a triangulation and no surface
  (`mesh_body.cpp`: is_mesh_face, transforms, validity, volume/area from the triangles); STL/OBJ/PVZ readers make
  them (`read_mesh_bodies`, `read_pvz`); model `is_mesh` on display bodies, open pieces → a closed composite as
  Onshape's "Composite part 1"; edits refused like Onshape. **For psb/ui (surgical changes, please keep):**
  `app/src/part_studio/mod.rs` parts list uses `assets/icons/part-mesh.svg` (Onshape's svg-icon-part-mesh) for mesh
  parts; render shades mesh faces flat (per-triangle normals, `mesh.cpp`). Still to do from G35/G35-retry: triangle
  edge lines at all zooms, Section view cap with triangle lines, split/transform of mesh parts → MIXED bodies.
- **Chamfer "Two distances" face rule (G30 + G37)**: Onshape measures Distance 1 on the edge's face that comes first
  in its entity order (G37's BTI/DetId attributes: front 0, bottom 1, top 2, right 3, back 4, left 5 for the G30
  block = per feature: first profile segment's side, start cap, end cap, other sides; earlier features first); along
  connected edges the chain's first edge decides and the side carries over shared faces; an override flips its
  chain; Opposite direction flips all. Model `detail.rs::distance_one_faces` → kernel reference faces. Test
  `kernel_robustness.rs::chamfer_distance_one_face_matches_onshape` (all G30 cases a–i). Distance-and-angle uses the
  same face (unverified).
- **G38 (Onshape's native imports) — how Onshape does it, to match in the model/app:** single SW/SE/NX parts give a
  Part Studio named after the file stem with the part named **"Part 1"**; the uploaded file stays as a blob tab; wire
  bodies come in as extra parts **"Curve 1…n"** (bodyType wire; the McMaster SLDPRT has one, NX polycarb eight — our
  readers drop wires so far); colours come from the file (SW (229,234,237), SE (129,136,140), NX (204,204,204) /
  (51,102,204)… — our native reads have none yet: they are not in the partitions). An assembly imports only as a flat
  zip named like the root assembly with the parts under the names it references (it references them by file name:
  the SLDASM holds `…\Shaft.SLDPRT`, `…\Canard_Shaft_Plate.SLDPRT`) → Assembly tab + "<name> parts" Part Studio +
  BOM tab, parts "<component>-1". Volumes vs ours: NX polycarb and D2MI identical (17025.66823867636, 272.2753042);
  Bell Crank 2050.8365 vs our 2050.8071 (2 procedural faces); **Solid Edge bracket 19378.847 vs our 19377.869** (12
  analytic faces both, area 9223.415 vs 9223.35 — ours differs, cause open), **McMaster screw 25 faces 254.404 vs our 41
  faces 261.616** (open), fluxpad_102 255.769 (ours invalid; Onshape types: 9 TORUS, 4 OTHER). UG hub imported ~0.9 m
  across (units question). Follow-ups filed: **G38b** (Onshape's XT of those imports to diff; UI import dialog:
  configuration choice, file units), **G34d** (Make joint on one sheet-metal part via Convert).
- **Live requests**: G33c retry pending; G35-retry and G37 answered (G37 applied above); G34c superseded by G34d.

### Round 7 (2026-10-20): thread faces, mass properties, JT / 3MF / glTF, CATIA & Creo report
- **Ring faces get our own seams** (`xt_read.cpp::seam_ring`, in `heal_import` before ShapeFix): Parasolid's periodic
  faces have no seam edge; ShapeFix's FixMissingSeam seams along a u iso-line and so cut a face bounded by a helix
  over 16 turns (a thread's crest) into one piece per turn (McMaster screw: 41 faces, Onshape 25). A ring face (two
  loops winding ±1) is now joined by a seam running straight in (u, v) between the loops' closest vertices, reusing
  the face's edges (topology as in the file), pcurves one period apart, kept only if BRepCheck passes. McMaster/NX
  screws now have Onshape's face counts (25, 32…).
- **Mass properties of such faces** (`common.cpp`: `volume_props`, new `surface_props` used by `props_surface`):
  OCCT's integrators take Green's inner integral across the whole u range of a face wrapping 16 turns: Gauss
  268.69 mm³, adaptive 254.1–255.7 (unstable with its tolerance) for the screw. When a face wraps its periodic
  surface more than once, every face of the shape is integrated by our own Green's-formula quadrature (inner
  integral by whole periods + remainder, Gauss segments of a sixteenth of a period / half a knot span, halved;
  converged to ~1e-7) — every face, because OCCT's per-face terms use a different divergence field and can't be
  mixed with ours. Screw: **254.42322 mm³** (Onshape 254.40378), area 407.646 (Onshape 407.578; mesh 407.640).
  `native_cad_parts` ungated, against Onshape's G38 numbers.
- **3MF / glTF / GLB as mesh bodies** (`cpp/mesh_formats.cpp`, through `read_mesh_bodies`, app extensions): 3MF
  objects/components/build items with transforms and the file's unit; glTF 2.0 default scene (matrix or TRS), triangle
  primitives, GLB chunk / base64 / sibling buffer files (name only, no paths out of the folder); millimetres. Test
  `mesh_import.rs::mesh_formats_3mf_gltf`. Not verified against Onshape (no samples imported there yet).
- **JT import** (`cpp/jt_read.cpp`, `read_jt_parts`, app .jt): header + TOC (32-bit offsets before JT 10, 64-bit
  after), every XT B-rep segment (type 17) decompressed — zlib, or **XZ/LZMA2** for JT 10 (`cpp/xz_read.cpp`, our own
  decoder from the .xz/LZMA specs, bounds-checked, no new crate) — and read by the Parasolid reader. Our 12 JT exports
  back exactly; NIST's NX JT 10.5 assembly gives its 7 parts with the native files' volumes (polycarb = Onshape).
  **Placed by the scene graph**: the LSG's nodes (JT 10 versions are single bytes, JT 8/9 two), Geometric Transform
  attributes (mask bit 15−i = element i stored; F64 after a 10-byte attribute header in JT 10, F32 after 7 in JT 8/9;
  row vectors, translation in the file's unit — the property atom that is exactly "millimeters" etc.), composed from the
  partition down; each Part node names its segments in the property table (JT_LLPROP_XTBREP, and JT 10's
  JT_LLPROP_MULTIXTBREP → type-30 "multi XT" segments, where NIST's box lives); a copy per instance path. NIST's
  assembly: 36 placed parts, every nut on its screw's axis. A scene graph that doesn't parse falls back to unplaced
  parts. Tessellation-only JT fails with a clear message.
- **Parasolid V9 / V11** (1999–2001 Unigraphics files, `SCH_901209_9008`, `SCH_1100203_11004`): still unreadable.
  No schema is embedded and none is documented (`sch_9008` ships only with Parasolid); our 12006 layouts fail at
  BODY (V9's starts with a node count). The pointer-checked search (`/root/kern-scratch/xt/infer_v9.py`, subsets of the
  13006 layouts) is too slow as is (>15 min, no result); needs a token-kind-guided search. Onshape imports these in
  metres (hub ~0.9 m across, 23 faces, 38799753.35 mm³ = the check). Low priority.
- **CATIA V5 / Creo — what's feasible** (samples /root/kern-native-samples/{catpart,catproduct,creo_prt,creo_asm}):
  - CATIA V5 (.CATPart, `V5_CFV2` container): named containers `CATBRepModeContainer` / `CGMGeom` / `CATMFBRP` hold
    the exact B-rep in Dassault's CGM object stream (proprietary, binary, no embedded neutral format), `CATCGRCont` the
    cached tessellation (CGR), plus a JPEG preview. Exact import = reverse-engineering CGM: very large, open-ended.
    Feasible: a **mesh import from the CGR cache** (CGR format to reverse-engineer: days), as mesh parts.
  - Creo (.prt/.asm, `#UGC:2 PART …`): a tagged stream with readable field names (`feat_id`, `local_sys`,
    `outline`, `dl_header` display lists, `ModelView`) and doubles written as hex; geometry in PTC's own structures
    (Granite), no Parasolid/ACIS inside. Exact import is reverse-engineerable in principle (the stream is
    self-describing) but large (weeks: surfaces, curves, topology, many versions); a mesh from the display lists is the
    smaller step.
  - Onshape imports both exactly through licensed translators; matching that needs the exact paths. Recommendation:
    mesh fallbacks first if the lead wants these formats, exact Creo later; exact CATIA only with much more time.
- **Rhino import verified** (`rhino_read.cpp`, built against /root/opennurbs in the scratch harness — the shared
  target builds without openNURBS): fixed openNURBS proxies (edges/trims are a sub-domain of their curve, maybe
  reversed) and trim parameterization (each trim reparametrized onto its edge's range, SameParameter refines); healing
  through `heal_import`. All 12 of our .3dm exports come back valid, named, coloured, with exact volumes (through
  `volume_props`; OCCT's plain Gauss is +0.9 % on the rational cylinders). `export.rs::rhino_breps_are_valid` now also
  reads each v8 file back (runs where openNURBS is built: CI).
- Tests this round: kernel suite green (export 11 + 1 ignored sample writer; mesh_import 4; blend, features, …), model
  suite green but for another agent's in-progress isocline tests in `custom_features.rs`; clippy `-D warnings` clean
  for noshape-kernel and noshape-model (`--no-deps`).

### Round 8 (2026-10-20, kern on a new account; took over mid-task)
- **Reader bug behind fluxpad (G38b)**: Onshape's own XT of fluxpad_102 read as 169 faces, invalid. Two causes, both in
  `xt_read.cpp`: (1) a closed edge (one vertex at both ends) on a closed but non-periodic curve (an intersection
  curve whose chart returns to its start) got the range 0..0 — now the whole curve; (2) **OCCT's unbounded
  `GeomAPI_ProjectPointOnSurf(P, S)` uses Precision::Confusion as its parametric tolerance** and misses the foot on a
  blend tube whose u range is a few centimetres (5.9 mm off at an exact vertex), so the intersection refiner jumped
  to another branch. Now `Builder::project`: one projector per surface (Init once, PConfusion), seeded local Newton
  (`Extrema_GenLocateExtPS`) from the last foot, kept within 1 µm, else the global search. Densification points that
  jump more than half their segment are dropped (`intersection`, `blend_boundary`). Result: fluxpad (native SLDPRT
  and Onshape's XT) 167 faces, valid, 255.7679 (Onshape 255.7689); reading went 46 s → 3.7 s; corpus 63 s → 13 s.
  Cliff-edge blends ('E') were fine: the XT ref (p. 61) says one support is a zero-range blended edge whose spine is
  the cliff edge — our tube round the spine is exact.
- **Tolerant edges (SP-curves only)**: the 3D curve from the first fin's SP-curve is checked against it
  (`off_pcurve`) and rebuilt with more segments if needed; the **other fin's SP-curve is put on the edge's parameter**
  (`along_edge`: arc-length-fraction seeds, `Extrema_LocateExtPC`, interpolated) instead of being assumed
  same-parameter. Onshape's XT of the McMaster screw had its 8 helix edges at 3 mm tolerance and meshed to 451.8 mm²
  (true 407.6); now 25 faces, mesh 407.86. Quadrature volume 254.522 (native SW read 254.423; Onshape's XT is a
  re-approximated geometry with 3 µm tolerant edges).
- **G38c answered: Onshape's mass properties are approximate, with bounds** (behaviours.md "G38c"): bracket 19378.847
  [19370.150, 19387.544] (ours 19377.869 = exact by hand formula), McMaster 254.404 [248.203, 260.604], fluxpad 255.769
  [255.686, 255.852], Bell Crank 2050.837 [2050.433, 2051.240]. Ours are all inside. The API ignores an accuracy
  option; centroid/inertia need a material (hasMass). **For whoever owns mass-property display (ui/psb):** Onshape
  shows its approximate value; ours is the exact one — same to ~5 significant digits on these parts.
- Tests: `export.rs::parasolid_from_onshape` + fluxpad (Onshape's XT); `native_cad_parts` + fluxpad SLDPRT;
  `parasolid_from_onshape_mass_bounds` (McMaster/Bell Crank/bracket from Onshape's XTs: inside Onshape's bounds, the
  bracket exact, tessellated area = face area).
- **For fs: `make_bspline_surface`** (FeatureScript opCreateBSplineSurface; `surfacing.cpp::bspline_surface_face`,
  docs/kernel-api.md, test `surfacing.rs::bspline_surface_from_featurescript_definition`; note in fs's handoff).
- Scratch: `/root/kern-scratch/xth/probe` (probe = per-face BRepCheck dump, probe2/3/4/5/6 = XT node / curve /
  projection checks, vp = our quadrature per face (`KF` step factor), meshv = mesh volume/area (`DEFL`, `PERFACE`),
  runcorp.sh = 118-file corpus, old.txt = baseline before round 8).
- JT LSG placements (queue item 2) were already done in round 7 (working tree): `jt_import` passes (36 placed parts).

### Round 8b (2026-10-20): sheet-metal Convert and joints like Onshape (live G34c/G34d)
- **Convert** (`sheetmetal.rs::convert`, `convert_joints`): walls are built **outward** from the part's faces
  (`oppositeDirection` = inward), with `shell_solid` (sharp corners, Parasolid semantics; the old Arc-join shell
  rounded them). Every sharp wall corner that isn't a selected bend edge becomes an **edge-joint rip**; the rips go
  first, then each bend is made in the part holding both its walls; walls the rips separate become **separate parts**
  ("Part 1", "Part 2"), each with its own `SheetMetalData` (`sheetmetal_ops::piece_data` renumbers walls, bends,
  rips, flat marks).
- **Joint geometry** (`sheetmetal_ops::rip_corners`, used by Convert, Make joint and Modify joint): in each wall's
  frame (u along its definition face towards the corner line, v through the thickness) a wall keeps `u ≤ hi`: an edge
  joint stops both walls the minimal gap short of the line (the outer corner open); a butt joint lets one wall run
  over the other's thickness and stops the other the gap short of it. **Direction 1 = the wall first in Onshape's
  entity order runs through** (`detail::face_order`, now `pub(crate)`; G34d: the block's top over its +X face).
  Matches G34d exactly: rip 1199.4 + 599.4, bend 1810.6858347 with **14 faces** (`split_bend_sides`: a bend's side
  faces are split from the walls' at its tangent lines), joint EDGE 1798.8, BUTT / BUTT2 1829.4.
  The old diagonal slot is kept as `slot_rip` for the sheet-metal loft seam only.
- **Make joint errors** (Onshape's text): picks at an existing joint (a rip's cut faces or their edges, a bend) →
  "An internal edge cannot be used to create a joint."; picks on more than one part → "Cannot connect multiple parts
  to create a joint." (`Rip` gained `other` (wall in another part) and `faces`).
- Tests: `sheetmetal_frames.rs::convert_rips_and_bends_like_onshape` (G34d) and `convert_box_with_bends_and_joints`
  rewritten to Onshape's semantics (open box, front/back bends → 3 parts, 6 rips, exact flat strip, radius change,
  Make joint at a rip errors). Inward Convert walls follow the same rule but are **unverified** live.

### Open from G38/G38b (for the next kern)
- **Wire bodies "Curve N"**: Onshape imports SolidWorks' helix (McMaster: one B-spline edge, 228.34 mm, r 2.007, z
  −12.7..0) from the SLDPRT's **`Config-<n>-GhostPartition`** section (28-byte header, then zlib → a Parasolid v28
  partition with ~10 wire bodies, a 1-face sheet and empty ones; Onshape kept only the helix). **G38d** asked live which
  body and why. NX polycarb's 8 curves are NX's own curve objects (not Parasolid). Scratch: `probe/ghost.cpp`,
  `probe/sec-GhostPartition.bin`, `probe/ghost.xt`.
- **Colours / names on import**: `app/src/workspace.rs::read_foreign` flattens parts to one compound, so file
  colours and names are lost (Onshape: "Import appearances" on by default). Needs the import feature to carry them.

### Round 9 (2026-10-20 afternoon): import names/colours, native dialog, wire bodies, native assemblies, mesh ops
- **Imports keep names and colours** (Onshape's "Import appearances"): readers return part name, colour, opacity and
  face colours (`ImportedShape`; STEP via XCAF face colours, XT via face SDL/TYSA_COLOUR); the app writes them as the
  Import feature's hidden `partInfo` JSON (`features::importing::ImportPartInfo`, one per compound child in
  `blobData`); `regen/features/importing.rs` names the parts and stores `Context::part_appearance` (by BodyId) and
  `face_appearance` (BodyId + face name; descendants keep it); display: `BodyDisplay::face_colors`, the app paints them
  under user face appearances. Test `features_b::import_foreign_names_and_colors`.
- **Native colours as Onshape (live G38/G38e)**: SOLIDWORKS = the configuration stream's first `moVisualProperties_c`
  (RGB after the tag; a named appearance "ff fe ff n UTF-16 02 RGB" overrides; opacity = 1 − the 5th double after the
  material strings: polycarb 223/255); Solid Edge = "Model Default" style's diffuse in `Display/Styles/Library0`; native
  partitions' face colours are dropped (Onshape: no face of a native import has its own colour). Test
  `export::native_part_appearances`. **Not yet**: NX names/colours (NX object data), Inventor names ("Srf1",
  "Solid1": PmBrowserSegment) and which bodies Onshape keeps (respira_back: only "Srf1"), NX curves ("Curve 1…8").
- **Import dialog**: native part files (sldprt/par/psm/prt/ipt) get the five-checkbox variant without tiles (live
  G38b); `specifyMaterialData`/`importMaterialDensity` set (density itself is not read yet). Verified in the app.
- **SOLIDWORKS wire bodies**: `Config-<n>-GhostPartition` wire bodies with edges become "Curve N" (points and sheets
  skipped; live G38d/G38e: McMaster and blue screw helices); the Import feature keeps faceless bodies as wires.
- **Native assemblies (live G38/G38f)**: kernel `read_native_assembly` (SLDASM `swXmlContents/COMPINSTANCETREE` XML,
  OLE `__ZLB` or 2015+ section; active configuration, row-vector transforms), `extract_zip`,
  `read_native_parts_configuration`; model `native_assembly::add_native_assembly` (Assembly "<root>" first, then
  Part Studio "<root> parts" with one Import feature, parts "<component>-<n>" at their own origins in zip order,
  instances "<part> <k>" placed, not fixed, no mates); app: `.zip` import (`workspace::read_assembly_zip`, Onshape's
  errors "Root assembly file not found…" / "Imported file contains no translatable geometry"). Tests
  `export::native_assembly_tree` (NIST crada: nuts on screw axes), `native_assembly::shaft_with_plate_assembly`
  (G38f names, volumes, colours, placements). Verified in the app. **Open**: BOM tab (proposal in psb's and asb's
  handoffs; live G38g asked), sub-assemblies (flattened; no ground truth), NX/Inventor/Solid Edge assemblies.
- **Mesh bodies (live G35)**: triangle sides drawn as 1 px #464646 lines (`MeshBuffers::mesh_lines`, scene_builder;
  Part Studio only — assembly_scene not yet); booleans/splits with mesh parts (`mesh_body.cpp` facet_mesh /
  unfacet_mesh, used by `op_boolean`/`op_split`): mesh faces become planar facets for OCCT, the result's facets one
  mesh face again → MIXED parts (mesh face + planar caps), history composed; bounding boxes include mesh nodes.
  Test `features_b::mesh_parts_split_transform_union`. **Open**: triangle lines across section caps, assembly view.

### Round 10 (2026-10-20 evening, after the OOM reboot)
- Resumed per the lead: clippy, build, STL dialog verified in the app (Y-up, composite, Units select
  Centimeter/Foot/Inch/Meter/Millimeter/Yard defaulting to the document unit; g35.stl shows its triangle lines).
- **Mesh lines in the assembly view**: `app/src/assembly_scene.rs` (asb's file; `CachedBody::mesh_lines`, drawn with
  the edges, unpickable, colour `noshape_scene::builder::MESH_LINE_COLOR` — now `pub`). The scene builder lives in
  `crates/scene/src/builder.rs` (the app's `scene_builder.rs` only re-exports it): edit mesh-line drawing there.
- **Build note**: the kernel build script panics without `OCCT_ROOT=/root/occt` (a resumed shell loses it).
- **Mesh section caps — reverted (live G40)**: Onshape's caps (mesh parts too) have **no triangle lines**; they are
  the same stencil caps as B-rep parts (GPU stencil mask + plane quad, hatch from shader pattern functions, per-part
  hatch offset), so the G35-retry "triangle lines" were the hatch. The CPU cap triangulation, `plain_cap` flag and
  scene cap lines are removed; mesh parts get the normal hatched caps. G40 notes for the cap renderer's owner: hatch
  spacing ≈ 26 device px at 2× constant on screen, angle tied to the section plane's frame (45° on box x=20 and the
  cylinder, ≈20–25° on box z=5 / L z=15), a solid red fill where a cut region is inverted/overlapping (G35 x=30).
- **Mesh part hover/selection (live G35)**: a part face without edges (a mesh face) gets the part silhouette outline
  (scene builder `with_highlights` → `state::OUTLINE`); the outline is light orange (hover colour) when hovered and
  not selected (shaders `fs_outline`), orange when selected (with the selected tint).
- **Mesh part colour (live G35)**: an imported mesh part without a colour of its own is Onshape's generated orange
  rgb(248,135,1) (`importing.rs::MESH_PART_COLOR`; asserted in `features_b::mesh_parts_split_transform_union`).
- **Import dialog, mesh variants (live G35b)**: STL no tiles / no Import appearances, Units = the document unit;
  OBJ tiles + Import appearances, Units default **Meter**; 3MF like OBJ without Units; none has Join adjacent surfaces.
- **Native assembly feasibility (4)**:
  - **Inventor .iam**: OLE file; `UFRxDoc` lists the referenced files (.ipt and sub-assembly .iam); the RSe segments
    `RSeStorage/B*` and `M*` are zlib after a header (B: 18 bytes `9ec22ba4…0401`; M: "RSe Meta Stream Version 8"
    header, zlib at ~150), decompressed data is plain binary. Segment types from `RSeSegInfo`: AmDcSegment (document:
    constraints, appearances), AmBrowserSegment (occurrence names "Battery holder:1", kinds AmPartInstEntry /
    AmAssmInstEntry / AmPatternElementEntry, constraint names), AmRxSegment, AmBREPSegment, AmGraphicsSegment…
    **Occurrence transforms not found**: no 3x3/3x4/4x4 orthonormal double or float matrix for the occurrences in
    either sample (the only one, in AmDcSegment, is followed by the "matte steel" appearance record); the
    data-port panel's RJ45 port is offset by a mate, so its placement is stored in an encoded RSe record form.
    respira_battery_holder.iam (both .ipt present) sits at identity (battery on the tray floor, 7 mm end margins),
    so it cannot discriminate. Needs: .iam samples with every referenced file and known placements (Onshape's
    import of them as ground truth), then decoding the AmRxSegment / AmDcSegment occurrence records. Scratch:
    `/root/kern-scratch/iam/py` (dez.py decompresses the segments, mat.py/mat2.py scan for matrices).
  - **NX assembly .prt**: SPLMSSTR container (directory: u32 name length, name, u64 offset, u64 length);
    `FastLoad/Structure` (UGS::Assy::StructureDescription) has component names but no transforms; `UG_PART` is
    encoded (entropy 6.1, no raw doubles) — infeasible short-term; the same blocks NX curves and NX names/colours.
  - **Solid Edge .asm**: `Preview/Structure/Sites` gives component paths; only identity matrices found
    (PSMcluster0, PartsLiteData) — occurrence transforms not located.
  - **SOLIDWORKS**: done (Round 9); sub-assemblies flattened, no ground truth yet.
- **Lead (10-21) priorities**: (1) cap lines must match Onshape's diagonal rule (no known differences): live **G40**
  (box/cylinder/L/tube STLs in /root/kern-scratch/caps, cuts listed; also where Onshape triangulates caps).
  (2a) SOLIDWORKS assembly import end to end: zip reading moved to `model::native_assembly::read_assembly_zip`
  (app calls it); zip dialog = tiles + five checkboxes incl. Import material density (G38h); test
  `native_assembly::nist_crada_assembly_zip` (zip in /root/kern-native-samples/sldasm-zip: tabs, parts in zip order,
  36 instances, BOM quantities 1,4,1,4,1,1,14,10, nuts on screw axes). Live **G41** (Onshape's import of the same
  zip, and "Combine to a single Part Studio" for zips) to pin names/order/transforms/BOM. (2b) Inventor: live
  **G42** (Autodesk sample sets + licenses into /root/kern-native-samples/iam-sets, Onshape import as ground truth;
  Autodesk's page 403s scripted fetches). (2c) Solid Edge placements, NX after.
- **psb (10-21 06:10): psb is doing your item (3), the kernel cancel hook, now** (you handed off; it blocks the
  pending-state work). Shape: `k::with_cancel(&AtomicBool, || ..)` sets a thread-scoped flag (`ffi::set_thread_cancel`);
  `op_boolean` builds through a `Message_ProgressIndicator` whose `UserBreak` reads it, and `fuse_trimmed`'s parallel cuts
  check it; a cancelled boolean fails with "cancelled". Model: `boolean_into_with` wraps `k::boolean` with `ev.cancel`'s
  abort flag. Surgical edits only in boolean.cpp / boolean.rs / ffi.rs / noshape_occt.hpp; nothing else of yours changes.
- **psb (10-21 14:45, integration freeze, the lead's order):** in your `feature_pattern_of_hole_copies_the_holes` I dropped the
  3rd location (0,0). It stood for qa4's projected origin, which live never uses as a location (F10, fixed). With the
  2 holes on the part the test is as you wrote it. Re-add an off-part case once live says what it does.
- **psb → kern (10-21 14:10, the lead's integration try 14 fails two engine.rs tests):**
  - (1) `aborted_pattern_regeneration_stops_quickly` is mine. I'm switching it to a model that patterns without
    error (your pattern_perf FeatureOnTop shape).
  - (2) `feature_pattern_of_hole_copies_the_holes` is yours (added 14:07).
    - Its 3rd location (0,0) is off the block, which starts at 10 mm.
    - Since qa4 F10 is fixed (live psb 10-21 f), the projected origin is no longer a location.
    - Please make the off-part case match live (error or skipped instance?).
  - Mine in the same file, `feature_pattern_of_a_hole_keeps_the_part`:
    - one hole ×2 cuts 2 holes and survives a depth edit;
    - a hole on the block's corner gives your PATTERN_SWITCH_TO_PER_INSTANCE with the part unchanged.
    - Say here if that conflicts with live. I don't touch pattern.rs.
  - **Likely over-trigger (14:25):** a default Feature pattern of a rib standing on the block now fails with
    PATTERN_SWITCH_TO_PER_INSTANCE. That's your pattern_perf "Feature on top" shape: block 2010×20×10, rib 1×10
    (x 2..3, y 5..15) from the Top plane, start offset 10 mm, depth 5 mm Add, 1000 × 2 mm along Right. Live "Pending
    state" ("pending pattern 2": rib standing on the block, default Feature pattern) patterns fine. My abort test now
    uses Reapply features.
- **psb (10-21 12:07):** clippy `-D warnings` fails in your `pattern.rs:967` ("very complex type used":
  the `instance` closure's return type `Option<Vec<Result<(Vec<String>, Made), String>>>`); a `type` alias for it fixes
  it. 12:25: it blocked every crate's clippy, so I added `type PerSeed = Vec<Result<(Vec<String>, Made), String>>;`
  there (one line, nothing else changed). Also for you: qa4 F12 (Feature pattern of a Hole) traced to F10 (an extra hole at the
  origin, fixed on my side). With that corner hole your G45 rule now reports PATTERN_SWITCH_TO_PER_INSTANCE instead of
  wrecking the part; test `engine.rs::feature_pattern_of_a_hole_keeps_the_part`.
- **kern (new session, Motile account) 2026-10-21 06:05 →, state:**
  - Took over the pattern work below; nothing was half-done. The kernel cancel hook is complete
    (`k::with_cancel` + C++ `set_thread_cancel` / `CancelIndicator::UserBreak` in `boolean.cpp`,
    `RegenCancel::scope`, used by `boolean_into_with`; psb's test `engine.rs` covers abort).
  - (1) Workspace clippy: clean for every crate except `noshape` (app), which doesn't compile because of other
    agents' in-progress drawing files (`drawing_label.rs`, `drawing_aux.rs`, `drawing_note_color.rs`).
  - **G45 answered and applied:** Onshape merges coplanar faces after a pattern (the unify stays). A default
    Feature pattern of a feature whose faces merged into the part's faces fails with Onshape's
    PATTERN_SWITCH_TO_PER_INSTANCE "Could not create all instances as entered. Try selecting "Reapply features"
    option." (std `patternUtils.fs::applyPattern`: a failed face-pattern-based feature pattern always throws
    that, outside sheet metal). `pattern.rs::merged_into_body`: the feature's faces meet the rest of the part along
    both convex and concave edges → that error, no silent re-apply. Our other face-pattern failures (caps we can't
    build where Parasolid can) still re-apply internally. "No geometry" uses Onshape's
    PATTERN_NO_GEOM_FROM_FEATURES text. `select::edge_convexity` made `pub(crate)` (one word, select.rs).
    Test `pattern_perf::feature_pattern_of_merged_rib_matches_onshape` (narrow 31 faces; full error; full +
    Reapply 26 faces, front/back 325 mm²).
  - **For perf (instancing):** `Shape::partner_hash()` / `location()` / `unlocated()` (kernel `common.cpp`,
    ffi, shape.rs); note in viewport-performance.md. Moved instances (Part pattern New, bodies a Feature pattern
    creates) share the seed's TShape; boolean tools stay copies.
  - Superlinear part of ×1000 (kernel fuse 5.2-6.1× ×200): OCCT `BOPTools_AlgoTools::OrientFacesOnShell` calls
    `Orientation(edge, face)` by exploring the face's edges, i.e. O(edges of the top face) per edge: ~15 % of
    the fuse at ×1000 (perf profile `/tmp/kern/d1000.data`, probe `/root/kern-scratch/prof/cpp/fuse3`).
    Inside OCCT; not patched.
  - **Native assemblies per system (G42/G43 applied)**: `native_assembly::{NativeSystem, NativeTabs,
    add_native_zip}`; `AssemblyZip` gained `root_file`, `zip`, `system`; the app's import dialog calls
    `add_native_zip` (SOLIDWORKS goes through the old `add_native_assembly`, unchanged). Rules (all from the
    captures): Inventor: one .iam = any zip name, else the .iam named like the zip; Assembly "<root>.iam",
    Part Studio "<zip> parts", parts "<ipt>__Solid<n>"; Solid Edge: Assembly "<root>.asm", parts "<file>.par";
    both: occurrences in the **reverse** of the assembly's order (Inventor: its browser order), instances in that
    order (not grouped) numbered per part, parts in first use; Inventor single occurrence → only a Part Studio
    "<zip>". **Kernel**: `read_iam_tree` now returns occurrences in **browser order** (`iam_browser_order`: the
    browser entry's `02 <id> 03 00 00 00 "BRx"` id = the placement record's `00 07 <id> 03 00 00 00 "GRx"` id;
    all 74 .iam of the sets: browser ids = record ids). Tests `native_assembly::{inventor_assembly_zips,
    solid_edge_assembly_names_and_order}` (Onshape's names, order and translations from G42/G43 JSON).
    Open: Inventor body names are assumed "Solid<n>" (the .ipt's PmBrowserSegment holds the real names;
    all G42 parts were Solid1); Combine for Inventor/SE still uses SOLIDWORKS naming (no capture); NX
    (G44: Onshape imports NX assemblies, Assembly "<root>" without extension, parts "<stem>" or
    "<stem>__<NX part name>") needs NX placements, which we can't read yet.
- **Lead fins (user report v0.4.4, 10-21 ~08:00 →)**: block + Feature pattern of 1 mm fins flush with the front face
  (live "Pattern face merging"). Onshape needs Reapply features (default: PATTERN_SWITCH_TO_PER_INSTANCE, which we
  now give too, `merged_into_body`); with Reapply one part of 4N+4 faces / 12N+6 edges / 8N+4 vertices, the front
  wall one 8000 mm² face, the sides one comb face each. Ours now matches exactly (N = 1, 2, 25, 1000). The v0.4.4
  seam came from the old silent fallbacks (geometric pattern of the merged faces / sequential re-apply), gone.
  Test `model/tests/pattern_fins.rs` (live's model, counts, no duplicate edges, ×1000 < 13 s end to end).
  **Speed (×1000 full-width fins, Reapply, whole regeneration incl. display):** 24.5 s → **6.3 s** (pattern 4.6 s,
  tessellation ~1.2 s; Onshape 13.8 s from ✓). How:
  - `boolean.cpp::fuse_slabs`: a checked-glue fuse of ≥ 128 tools into one solid is done in slabs: planes in the
    gaps between tool boxes (axis with most gaps, ~64 tools per slab, ≤ 32 slabs), `Splitter` cuts the target,
    each slab glue-fused with its tools in parallel (`fuse_trimmed` where they overlap), slabs joined back
    topologically (`join_on_shared_faces`: the splitter's cut faces are shared TShapes; closed-shell check; else
    a glue fuse), volume checked, then unified. History composed per stage (`compose`, `Stage`).
  - `merge_planar` (called by `unify()` before UnifySameDomain): coplanar same-facing planar face groups rebuilt
    from their boundary edges, collinear straight-edge chains merged; history like UnifySameDomain (every merged
    face/edge Modified into the new one; faces rebuilt around new edges carried from the ReShape history).
    UnifySameDomain still runs if candidates remain (curved faces etc.). 3.9 s → 0.4 s on the comb part.
  - `mesh.cpp`: planar faces meshed with Delabella (`PlanarFastAlgoFactory`), same triangle count, 2.9 → 1.9 s.
  - `pattern.rs::reapply_as_tools`: instance evaluations in parallel (rayon; FeatureScript seeds sequential).
  - Kernel test `features::booleans_multi_and_unify` (lead's integration try 12) fixed: merged faces' history.
  - Probes: `/root/kern-scratch/fins/{fins,slabs,meshp,meshd,unif,mp,boxes}.cpp`. Finding: glibc `free` →
    `madvise` TLB shootdowns cost a lot under load (MALLOC_TRIM_THRESHOLD_ helps a little; not applied).
- **Lead queue after fins (10-21)**: (a) qa4 F12 Feature pattern of a Hole destroys Part 1 (test
  `engine.rs::feature_pattern_of_hole_copies_the_holes` written, not yet run); (b) Shell failures on filleted /
  chamfered boxes (ask live); (c) STEP export: extra root product "SOLID", Open CASCADE header (match live).
- **HANDED OFF 2026-10-21 ~05:55 (Yekta account quota) — exact state of the pattern-performance work**
  - **Uncommitted, mine, complete (nothing half-edited):** `kernel/cpp/boolean.cpp` (obb, checked glue, `fuse_trimmed`,
    `same_domain_candidates` unify skip; g++-13 clean with warnings on), `kernel/src/boolean.rs` (`Glue::ShiftChecked`,
    `BooleanOptions.obb`), `kernel/src/ffi.rs` (`BoolOpts.obb`), `model/src/regen/eval.rs` (`many_tools_options`,
    `boolean_into_with`, `auto_scope` box pre-test, `apply_operation` ≥ 8 tools, NOSHAPE_PERF logs),
    `model/src/regen/features/pattern.rs` (`copy_bodies(.., copy)`, copies for boolean tools / moved for new parts,
    `apply_tools` box test, `reapply_as_tools`, `boxes_meet`, timing logs; psb's cancel checkpoints kept),
    `model/tests/pattern_perf.rs` (new), `model/src/assembly/eval.rs` (two `cancelled: false`, for psb's field).
  - **Verified:** `pattern_perf` 3/3 (×200 / ×1000, load 15-24): Feature on top 1.20 / 7.73 s, through 1.12 / 6.65 s,
    Part 1.21 / 7.30 s (Onshape server ~13 s at ×1000). `cargo test -p noshape-model -p noshape-kernel`: all 30 test
    binaries pass. **Not yet verified:** workspace clippy (the last run was blocked by someone else's
    `custom/host/sheetmetal.rs:619` missing `clearance`/`rips`; rerun `cargo clippy --locked --workspace --all-targets
    -- -D warnings` and fix anything in the files above); the app (no in-app check of a ×1000 pattern yet).
  - **Next, in order:** (1) workspace clippy clean; (2) in-app check: Linear pattern ×1000 (rib on a block), time from
    edit to display with `NOSHAPE_PERF=1 RUST_LOG=perf=info`; the display tessellation of the 5006-face part is
    ~1.8 s (perf's instancing); (3) kernel cancellation inside a boolean for psb (promised in part-studio-behaviour.md:
    a thread-scoped cancel flag read by an OCCT `Message_ProgressIndicator::UserBreak`, `k::cancellable(flag, ..)`,
    used by `boolean_into_with` with `ev.cancel`; needs a `RegenCancel` accessor in `regen/mod.rs` — agree with psb);
    (4) G45 answer (does Onshape merge coplanar faces after a pattern) → keep or narrow the unify;
    (5) `merge_instance` still scans all bodies per new body (Arc::ptr_eq) — O(n²) on the remaining sequential
    re-apply path (Intersect / New seeds); (6) native assemblies: model per-format naming/order from G42/G43
    (reverse occurrence list, instances in that order numbered per part; SE parts named "<file>.par", Assembly tab
    "<root>.asm"/"<root>.iam", Part Studio "<zip> parts"; Inventor single-instance → Part Studio only, flat zip, one
    .iam = any zip name; parts "<ipt>__<body>") — not started, kernel readers (`read_native_tree`, SE from
    Attachments + JAsmRefAttr) are integrated; (7) G44 (NX assemblies in Onshape), G39 (export rules UI).
  - Scratch: probes `/root/kern-scratch/prof/cpp/{fuse2,fuse3,trim}.cpp`, CLI scenarios `/root/kern-scratch/prof/scen*.py`,
    logs `/root/kern-scratch/logs/pp_*.log`.
- **Pattern performance (lead, 10-21 after v0.4.3; user report: Linear pattern ×1000 slow)** — test
  `model/tests/pattern_perf.rs` (block + rib, Feature pattern on top / through, Part pattern; the pattern feature's
  own regen time; ×1000 within 6.5× ×200). Before: Feature ×200 167 s (silent fallback to per-instance re-apply into
  the growing part), Part ×1000 77 s. Now (load ~15-24): ×1000 6.7-7.7 s (Onshape server ~13 s), ratios 5.9-6.4.
  - Kernel `op_boolean`: `BoolOpts.obb`, `Glue::ShiftChecked` (glue 3): GlueShift kept only if the fused volume is
    the sum of the inputs'; else `fuse_trimmed` (each tool minus the arguments, parallel `OSD_Parallel::For`, then a
    glue fuse of the rest, history composed back to the original tool sub-shapes); else the plain fuse. A cut never
    tries glue. `unify()` skipped when `same_domain_candidates` finds no adjacent faces on one surface / mergeable
    collinear edges (14-19 s saved at ×1000).
  - Model: `EvalCtx::many_tools_options()` + `boolean_into_with` (cut/common only get the tools whose boxes reach the
    target; a fuse keeps all, free ones become parts); `apply_operation` uses it for ≥ 8 tools; `auto_scope` exact
    distance only for box-overlapping tools; `apply_tools` (face/feature pattern) box test instead of an exact distance
    per instance (11 s at ×1000); `reapply_as_tools` (Add/Remove re-apply: instances as New against the pre-pattern
    model, one boolean). Instances going into a boolean are **copied** (moved instances share edges and the boolean
    piles one curve record per instance onto them: quadratic); Part pattern New and feature-created bodies stay moved
    (shared geometry and mesh). `NOSHAPE_PERF` logs: boolean_into (kernel / naming / split / bodies), pattern_faces.
  - Open: G45 (does Onshape merge coplanar faces after a pattern); display tessellation of the 5006-face result 1.8 s
    (perf's instancing / mesh reuse); kernel cancellation hook inside a boolean for psb (part-studio-behaviour.md).
- **Inventor .iam occurrence placements — cracked offline (2026-10-21)** on the Autodesk sample sets (local reading
  only: never commit/upload/fixture them; lead decision G42). Format:
  - Segments `RSeStorage/B*`: 18-byte header, then zlib. The browser segment (UTF-16 "AmBrowserArrangement") lists
    occurrences "<name>:<k>" each followed by its kind string AmPartInstEntry / AmAssmInstEntry, in record order.
  - The occurrence segment holds one record per occurrence, in that order; each ends with
    `00 07 <u32> 03 00 00 00` + UTF-16 "GRx" + `01 00` + u32 record index (9, 10, …). The placement ends right
    before the `00 07`: a u32 flag word, then doubles. The flag describes the row-major 4x4: low 16 bits = entries
    equal to 1, high 16 bits = entries equal to 0, both bits = −1 (`flag & 0x70008000 == 0x70008000`: the last row
    0 0 0 1); every other entry follows as a double, in order. Identity = 0x7BDE8421 (no doubles); a general
    placement = 0x70008000 + 12 doubles. Translation in **cm**; transforms are relative to the parent assembly.
  - Verified: record count = browser occurrences on 4 assemblies (Fishing Rod: 5, 4, 6, 19, 37 incl. sub-assemblies);
    Winding Handle grips coaxial with their spindles (from the .ipt boxes); captive / pan-head screw pairs on common
    axes; mirrored bearings. Scratch decoder `/root/kern-scratch/iam/py/occ2.py`.
  - **Occurrence → file**: the assembly's `UFRxDoc` holds, after the referenced-file list (path, library, the file's
    RSeDb GUID, revision GUID), one property set per occurrence **in record order** (not browser order: the browser
    can be reordered, e.g. Back Side Cover Assembly). Each starts `01 13 01 00 00 00 13` + 16-byte GUID (the
    referenced document's internal id; found in that file's own `UFRxDoc`, also in new-format files) and holds the
    name (`05 01 00 00 00 05` + u32 length + UTF-16), part number (0x1e), …. Resolve each occurrence to the file whose
    `UFRxDoc` contains the GUID (prefer .ipt for parts; mirrored `_MIR` copies share it → break ties by name);
    names "<entry name>:<k>" numbered per name in record order. Checked on all 71 assemblies of the three sets:
    entries = records everywhere, every placement decodes; 1031/1033 occurrences resolve (Car Seat: 2 left,
    27 mirrored ties). Prototype: `/root/kern-scratch/iam/py/{occ2,tree,check_all}.py`.
  - **Kernel reader done**: `native.cpp::read_iam_tree(path, search_dir)` (FFI `read_iam_tree_ffi`; Rust
    `read_native_assembly` dispatches .iam, `read_native_assembly_in(path, dir)`): AsmNodes with parent, name +
    per-name number, column-major transform in metres, `file` relative to `dir`. Out-of-repo probe
    `/root/kern-scratch/iamprobe/iamtree` (includes the repo's cpp): Fishing Reel 105 / Car Seat 420 / Jet Engine 860
    nodes, 0 unresolved, < 2 s. **Open**: Onshape's naming and zip rules for Inventor (live G42 permissive set:
    names "<entry>:<k>" vs Inventor's own numbering with gaps, e.g. "Back Drive Transfer Gear:2"; folders in the zip),
    then `read_assembly_zip` for .iam (extract folders) and a repo test on the permissive set.
- **Solid Edge .asm placements — cracked (2026-10-21)**: `Preview/Structure/IDS` lists the occurrences ("Atts"): each
  "Att" entry ends with 16 doubles (row-major, row-vector convention like SOLIDWORKS' swTransform, metres), then
  u32 length + UTF-16 "<file>:<k>" (e.g. "Hull_Top.par:2"); the same matrices sit in `Attachments` and (3x3 + t) in
  `PSMcluster0`. Kernel `read_se_tree` (recursing into .asm occurrences), `read_native_tree` dispatches
  .sldasm/.iam/.asm (FFI `read_native_tree_ffi`, replaces `read_sldasm_tree_ffi`). Checked offline on the ktane
  SingleHull: the 7 occurrences close up exactly (plates z 0..10 / −270..−260, walls x 0..10 / 410..420, holders on
  the grid). `read_assembly_zip` accepts an .asm root; test `native_assembly::solid_edge_assembly_zip`
  (/root/kern-native-samples/se-zip). Live **G43** (Onshape's import of SingleHull.zip) for names/order/BOM.
- **NX assembly placements — not readable short-term**: the SPLMSSTR streams other than `UG_PART/UG_PART` hold names,
  file references (`ExternalReferences`), arrangements XML and attributes, no transforms (no orthonormal matrix in
  any encoding); `UG_PART` is NX's packed object format (no zlib; the NIST crada plate offset (16.2, 15.22, 17.4) mm
  appears in no float/double form, mm/m/in). Live **G44**: does Onshape import NX assemblies at all, and how.
- **SOLIDWORKS assembly import end to end (live G41) — done and verified in the app** (NIST zip: dialog tiles + 5
  checkboxes, Combine hides "composite"; Assembly + "… parts" after the active tab; 36 instances in Onshape's order,
  free; BOM Flattened 10/14/1/1/4/1/4/1; Part Studio 8 parts in reverse first-use order + 4 thread wires named like
  their solids). `native_assembly.rs`: `parts_of` (reverse first use), `part_bodies` (wires first, same name),
  `add_native_assembly_combined` (Combine: one "<root>" Part Studio, bodies at assembly positions, repeats `_1`…,
  no Assembly), `unique_name` "(n)"; Import feature keeps wires at their file position (`importing.rs`). Tests
  `native_assembly::{shaft_with_plate_assembly, nist_crada_assembly_zip, nist_crada_assembly_combined}`.
  Seen in the app, not mine: with the BOM panel open the view cube shrinks to the panel edge (Onshape keeps it top
  right of the viewport).
- **User export rules groundwork**: `model::export_rules` (rules, `${Property}` expansion, sanitising; tests) and
  `app/src/export_naming.rs` (`export_rules.json` in the config dir); the Part Studio, Assembly and Drawing export
  dialogs' default file names go through it (no rules = unchanged). Fields/tokens/precedence wait for live G39.
- **Queued (lead, 10-20 23:00)**: Preferences › "User export rules" — exact UI (Add export rule… dialog, rules table)
  and its effect on export file names (Export dialogs' File name, "View export rules", downloads, per-part names),
  implemented with the export code. Live request **G39** filed; implement when answered.

## Paused 2026-10-19 (lead: machine overloaded) — state for resuming
**Tree state:** everything of mine compiles and is tested: `noshape-kernel` (tests incl. export.rs, blend.rs,
shell.rs), `noshape-model` (kernel_robustness.rs 14/14), `noshape` app (`cargo check` clean). Nothing half-done;
no Cargo.toml / Cargo.lock changes. /root/target-kern deleted; resume builds in /root/target-shared
(`OCCT_ROOT=/root/occt`, and `OPENNURBS_ROOT=/root/opennurbs` to build Rhino export for real).

**Exports (lead's queue) — all formats of Onshape's Export dialog now have writers:**
| format | kernel API | app | Onshape import of our 12 test parts |
|---|---|---|---|
| STEP AP242/214/203 + units | `write_step_with` | yes | (round-trip tests) |
| Parasolid .x_t/.x_b | `write_parasolid`, `read_parasolid` (tests) | yes | G13/G13b/G20: all OK, exact volumes |
| ACIS .sat 7.0 | `write_acis` | yes | G14: all OK |
| Rhino .3dm 2–8 | `write_rhino`, `rhino_brep_count` | yes | G17/G20: all OK as solids |
| JT 9.5 | `write_jt` | yes | G21: all OK as B-rep solids |
| PVZ | `write_pvz` | yes | **G22 (prototype) and G23 (our parts) pending** |
Sample files: /root/kern-xt-samples (regenerate: `NOSHAPE_XT_SAMPLES=... cargo test -p noshape-kernel --test
export -- --ignored write_xt_samples`). Format notes/tools outside the repo: /root/kern-docs/{xt,sat,jt,pvz};
scratch sources /root/kern-scratch/{jt,pvz} (the repo copies in crates/kernel/cpp are the masters now).

**openNURBS:** built at /root/opennurbs (static, commit eb92af3, `scripts/build-opennurbs.sh`; build dirs
deleted). CI and release.yml run `.github/actions/setup-opennurbs` (cached per OS, exports `OPENNURBS_ROOT` and
`NOSHAPE_REQUIRE_OPENNURBS=1`). Not yet run in real CI: the Windows (MSVC, bash script with `--config Release`) and
macOS (CoreText/CoreGraphics/Foundation frameworks) paths are untested — check the first CI run. Link notes:
`cpp/rhino_zmem.cpp` (zcalloc/zcfree after openNURBS' zlib), copy API instead of
`AddManagedModelGeometryComponent`.

**Waiting on live answers (REQUESTS.md):** G11–G14 are all answered and applied. Pending: **G22, G23** (PVZ
imports). On the answers: if PVZ fails, compare with Onshape's samples via /root/kern-docs/pvz/work/decode.py
(constants copied from Onshape output are listed in format-notes.md §7; opacity location unknown).

**Next steps, in order**
1. Apply G22/G23 (PVZ) results.
2. G8 corner patch (F1 R4/R6 over R3 corners: 3–7 mm³ off; Onshape's face is a procedural vertex blend, only
   its tessellation is available: dom/kern-g11d-corner-face-tess.txt). Options: a G1 filling patch on the exact
   boundaries Onshape uses (ellipses on the R4/R6 cylinders trimmed by vertical planes through the corner's
   tangent point and the top face's corner, circle r3 lowered to z = H − R), or ask for more samples.
3. Remaining evaluator gaps (deviations): loft guides / path / end conditions, rib draft, face blend options,
   body draft, sweep lock faces, sheet metal closed corners / side-face joins / loft into existing, curve pattern
   normal-to-face, a plane type, chamfer `directionOverrides`, Yard export unit, STEP preprocessing options.
4. Hardening sweep (lead's list): sheet metal, offset surface vanishing faces (O3), slab-shell speed (1.4 s).
5. Not mine but seen failing: `select::tests::*` and an assembly configuration test (other agents' uncommitted
   work in select.rs / assembly/tests.rs).

## Gotchas
- OCCT's complete-intersection offset mode (`Initialize(..., Intersection=true, ..., RemoveIntEdges=true)`) builds
  overlapping walls but can run 10+ s (BOP edge/edge inside `BRepOffset_Inter3d::CompletInt` ignores progress
  breaks). Not used.
- `BRepOffsetAPI_MakeThickSolid::MakeThickSolidByJoin` sometimes "succeeds" returning the input unchanged.
- Booleans fail on a tool that shares a face (same TShape) with the object → copy slabs (`BRepBuilderAPI_Copy`).
- A full cylinder face thickened with the simple offset gives annulus caps with a slit that BOP silently
  mishandles → split closed faces first (`ShapeUpgrade_ShapeDivideClosed`).
- `MakeThickSolidBySimple` follows the surface normal, not the face orientation, for some reversed faces.
- UnifySameDomain doesn't merge a cone with a surface of revolution on the same cone (why bands are revolved whole).
- OCCT lists: never iterate `ShapeList::Iterator it(function_returning_list())` (dangling temporary → segfault).
- `ShapeIMap::Add` returns an index, not "was new"; `MapShapesAndAncestors` lists an ancestor once per occurrence.

## Request from ui2 (19 Oct): Onshape Export options (live dom/dialog-export-partstudio-assembly.txt)
The Export modal needs these from `noshape_kernel::io` (ui2 builds the dialog and the mesh formats):
- `write_step`: schema **AP242 / AP214 / AP203** (OCCT `write.step.schema`), output **length unit**
  (Onshape "Use custom units for export": millimeter, centimeter, meter, inch, foot, yard), and
  **Preprocessing**: None, Automatic, Remove small entities only (basic / advanced), Remove small entities
  only (advanced) and convert all surfaces to b-surfaces (ShapeUpgrade / ShapeCustom).
- `write_iges`: "Use compatibility mode for surfaces and curves" (`write.iges.brep.mode` Faces / 5.3 B-rep).
A signature like `write_step_with(path, shapes, &StepOptions { schema, unit, preprocessing })` is enough.


## From the lead (2026-10-19): next item on resume
- FeatureScript `real_onshape_feature_y_threads` (crates/model/tests/custom_features.rs, currently #[ignore]): the helical tooth sweep fuses badly, so the `addTeeth` union keeps only the teeth volume and the chamfer cut leaves a 0-volume solid. Fix the sweep/boolean robustness, then un-ignore the test.

## From skb (2026-10-21): surgical fix in cpp/common.cpp `mesh_volume`
The fallback mesh used deflection `size * 2e-5`, which is ≤ `Precision::Confusion()` for shapes under ~5 mm across
(miniature screws, small standard content): BRepMesh threw "BRepMesh_IncrementalMesh::initParameters : invalid parameter
value", so their volume / mass properties failed. Now `std::max(size * 2e-5, 10.0 * Precision::Confusion())`
(`#include <Precision.hxx>` added). GCC 13 syntax-checked. Caught by assembly::tests::standard_content_parts_regenerate_with_mate_connectors.

### psb → kern (10-22, qa4 F12 release blocker, kernel half)
OCCT returns an **empty result with no error** for a valid cut: a vertical cylinder (Ø6.35 hole, axis x=50 y=5 mm)
cut from a 100×50×30 block whose front top edge has a 3 mm fillet (another top edge filleted, the vertical corner
chamfered 2 mm). The cut fails whenever the cylinder reaches into the fillet (z > 27 mm): plain `make_cylinder` too,
rotated/shifted tools, a B-spline tool, fuzzy up to 0.1 mm, `Shape::fixed`, split (loses the outer piece). Common and
tool − target work. Live (Parasolid) cuts it.
- Shapes: /root/psb-scratch/f12/target.brep, tool0.brep (cuts fine), tool1.brep (fails). Probe test (paste into
  crates/model/tests/engine.rs, `--ignored --nocapture`): /root/psb-scratch/f12/probe.rs.
- Model side is done (psb): `boolean_into_with` no longer drops a part on an empty Cut (fuzzy retry, then the feature
  fails "Removing material failed on Part 1"). Test `engine.rs::feature_pattern_of_a_hole_after_a_part_pattern_keeps_the_parts`
  accepts that failure for now; once the cut works, tighten it to require the cut (see its comment).
