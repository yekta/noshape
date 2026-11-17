# dx handoff: agent API + drawings + data exchange (current state)

State as of 2026-11-17 08:19 UTC. Rewritten, not appended (DECISIONS.md 11-15); history in docs/handoff/archive/dx.md.

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md (generated), drawings and their export (crates/drawing,
crates/model-data/src/drawing), import/export, configurations, Variable Studio, Derived, start window and document
visuals, the parity translator (crates/ops/src/parity.rs) for my studios. The drawing UI is studios' (11-12).
Studio split with asm (lead 11-15): infento 1dc32b / 74d3d6 and bjr-config 5d8bc1 are mine; isaac-ball (3c5c58,
1e81ca, 90f78b) and config-assembly 20e932 are asm's. asm gates parity.rs changes before they land.
Reduced mode until ~11-17 10:15: asm, skasm, bool2, studios, live, live2 paused: never message them; requests go to
/root/onshape-research/live/REQUESTS.md or their handoff.
Before any crates/drawing land: the full `cargo test -p noshape-ops --test all e2e::` (the queue may skip ops).

## In flight: data exchange (lead 08:0x)
Task: 10 corpus docs x every format + a drawing vs live's exports; re-import live's STEP / x_t; times vs live's.
Largest gaps first, one line per landing. Data /root/parity/exchange/<doc>/ (live.*, live-self.json, live-times.json,
live-drawing.dxf); suite parity::exchange_round_trip (dx-keep/bin/exch.sh). live2's 18:50 fetch of the
rest of set 2 failed (HTTP 401 on every doc; only variable-fillet's STEP / IGES came): REQUESTS line 3328 (21:04) asks
for a rerun after re-authenticating (live2 paused until ~11-17 10:15). Full suite rerun x1116n (vibe6, main 57885ecc
binary incl. 4dd88057, all 17 docs, snapshot ~/tmp/dx/exs, log ~/tmp/dx/exch-x1116n.log, started 21:28): rank from it.
Landed 11-16 / early 11-17 (exchange suite, x_t / ACIS / DXF fixes): see the archive, 11-17 05:52 block.
59c0be35 (landed 04:37): SAT writes a face going round its B-surface in ACIS's periodic form (clamped knots,
"periodic", no seam: live's own SAT) and reads that form back periodic (shared bsurface_as_periodic, xt_read);
RoundSplines {Split, Parasolid, Acis} replaces the periodic_splines bool, prepared() takes `offsets` apart and converts
extrusions for ACIS. Ours reads ours.sat: motor-mount-jack 16/24/0 and rings-loft Part 1 273/762/508 as the model
(Mac exs-all). T1 export::a_loft_wall_goes_round_as_one_acis_face.
9a19e358 (landed 05:00; lead: split from it, checked alone): a vertex where only seams cross isn't shown (live
behaviours.md "kern (10-25) seam counts": no seam vertices; free-form one-face bodies 1/0/0) and a free-form face's
box holds the highest point its climb finds (OCCT's was 1.9 µm short): o-ring generator's elliptic rings read back
1/0/0 as live. Model suite A/B vibe7 (~/tmp/dx/abK-*): identical but c2-loft-paths-guides Loft 3 vertices 27 -> 26
(live 23, edges same); hats/dock SAME on every tab (vibe5); psui 206/206, scene, kernel 480/480 on the Mac.
Left in SAT: cf-hose-routing 22/44/24 for 12/12/0 (sections split, as in x_t); rings-loft Part 2 (x_t's parked piece).
Suite x1117a (vibe6 00:04, with all of these): ours imports live's 59/75, live imports ours 46/48, ours reads ours
112/119, times 49/50, drawings' DXF 0/4.
Open, ranked:
1. Drawings DXF 0/4 (Mac exs-d, 4 docs, with the queued conic-tolerance commit): sm-louver ARC 68 / CIRCLE 2 as live,
   ELLIPSE 96/86, LINE 2539/2472, MTEXT 52/54 (scale 1:40 for live's 1:50: whole-studio bounds 2678 x 841 x 978 mm fit
   1:40 by every rule matching 1898's data; lead: no special case, REQUESTS 11-17 01:47 asks live for blocks across
   the boundary). housing-draft ARC 0/3, CIRCLE 5/6, ELLIPSE 16/15, LINE 159/154, SPLINE 11/7. extension-spring SPLINE
   37/47 (live splits more), LINE 120/117. rings-loft ARC 37/91, ELLIPSE 135/95, SPLINE 189/211: model (item 4; our
   true circles where live's model has B-splines get conics, live writes SPLINE).
   Tools: /root/dx-scratch/bin/dxfdiff.py ours live drawing.json (per view, bbox-aligned; live's -Z extrusion
   ELLIPSE / ARC handled), dxfplot2.py (a view side by side by type), dxfboxes.py / dxflayers.py (per-view boxes).
   housing-draft findings (11-17 03:00):
   - Placement: live centres each suggested view on the projection of the part's loose Parasolid box (live.jsonl
     final bbox z -3.685..5.0 mm, x +-16.44, from Rotieren 1's torus; the geometry is z -1.37..5.0, x +-15.3), ours on
     the exact box: Front / Right 1.158 mm (0.0456 in) and Iso 0.946 mm lower, Top exact. Matching it needs
     Parasolid's loose box of a torus face: left (counts unaffected).
   - Lug bottoms (torus x plane B-spline edges): live writes chords of a 3D tessellation (LINEs up to 0.13 mm off the
     curve; one chord where 1.8 mm shows) in Front and Right, cubic SPLINEs in Iso; ours SPLINE everywhere. No rule.
   - Right-view radial hole rim: live one CIRCLE (a B-spline rim on the hole's cylinder, seen along its axis); ours a
     70-point free SPLINE + an 8-point piece + three 2-point bits (our model has 8 LINE edges between cylinders, live
     none). Idea, unproven on other docs: a curve on a cylinder seen end-on is that circle's arc.
   - Model: our Rotieren 1 face is a surface of revolution (STEP SURFACE_OF_REVOLUTION + 8 extra plane / SoR LINEs)
     where live's is a degenerate torus; Iso lug-end LINE vs live ARC x2 follow from that.
   rings-loft own x_t Part 2 133/338/208 for 132/335/206: a 0.007 mm² cylinder piece shares part of the big face's seam
   edge (three uses), so UnifySameDomain leaves it: parked.
   SAT: live's own SAT splits periodic faces (rings-loft live.sat 445 faces for the model's 439), so the SAT rows wait
   on live's import of its SAT (REQUESTS 11-17 01:47, item 2, for live2). Ours writes closed B-surfaces split at mid (hose, o-ring's 4
   elliptic rings, motor-mount, rings-loft); ACIS has periodic B-surfaces (clamped knots + "periodic" form, faces with
   no seam): writer + reader would be needed.
2. Probes: a saved rings-loft doc on the Mac (~/noshape-host/dx-keep/rl-save; copy it before each use, export_parts
   saves into the open doc).
3. Our SAT read-backs: motor-mount-jack 24/26/2 for 16/24/0; hose 22/44/24 for 12/12/0; o-ring Part 3 4/7/2 for 1/0/0.
   live's: sm-louver Part 2 40/82/44 for 38/76/40; rings-loft live.sat/.igs 275/771/514 for 273/762/508.
4. Drawings DXF 0/4. rings-loft ARC 40 for 91 is model geometry: live's Shell walls are exact rational tubes (deg 3x2;
   their cuts with the cylinders 56 ELLIPSEs), ours are offsets written as approximations (deg 15x13, 7189 poles), the
   cuts B-splines (/root/dx-scratch/bin/step_edgefaces.py, step_bsurf.py). Reported to the lead for bool/core.
   housing-draft ARC 0/3, ELLIPSE 10/15, SPLINE 15/7; sm-louver ARC 101/68, ELLIPSE 50/86, SPLINE 31/0.
- Not bugs: sheet-metal-plus ours.3mf read back 554665 for 626770 mm³ (the suite's fallback expectation: Part 2 inside
  Part 1 is its hollow, as live's import of ours.obj does; wait for live's 3MF import of ours). ours.obj "3 parts, live
  2": ours reads the zip by groups (live refuses zips), live was given the bare .obj.
- Intermittent segfault (vibe6, binary ops-ex2 22:47 and 22:52, both right after rings-loft's imports, before its STEP
  export); not again in 8 runs (vibe5 gdb, vibe5/vibe6 plain, x1116j). gdb on vibe5: `handle SIG37 nostop noprint
  pass` (the checkpoint thread's signal). Watch for it.
- c2-config-parts-screws: hang fixed (b777cbd1), still in tests/parity-models/unscored.json: Sweep 1's 14.8 GB
  Boolean is bool's P0 (lead 11-16). Score it once fixed. Remove vibe5 ~/agents/dx/target-g13 when no longer needed.
- Exchange suite x1116n running (above); set 2 again when live2 refetches.

## Other in flight
- config-parts (vibe5 07:4x, ops-hx2, screws skipped; ~/tmp/dx/cfgparts-hx2.log): linear-rail 19/26, links 10/35, nuts
  8/17, pulleys 9/22, sprockets 10/24, standoffs 11/32. Landed 69b8e51c (Any variable = string), 5ee5d266 (std 736).
  Queued 16c51e2a (expressions compare strings: `"text"` where a value is expected, string variables via
  VariableScope::get_text / ctx.fs_variables; nuts' `#name2 == "Nyloc" ? …`) and 8d3a0ec2 (a configuration checkbox is
  a variable: sprockets' #Default_Thickness, pulleys' #Flanges). Left: live's sketches.json is at the workspace's
  current configuration: refetched at defaultValue 07:39 (sketches.json capturedConfiguration). But live.jsonl's
  per-feature lines are at the current configuration (nuts: Extrude 1 41.569 mm³ = M2 hex; final "M4 Nyloc" 31.406,
  REST at M4 148.47): asked live 07:5x to retake lines at defaultValue. Rerun (ops-cp, current branch) then: linear-rail
  19, links 10, nuts 8, pulleys 9, sprockets 11, standoffs 11 (~/tmp/dx/cfgparts-cp.log). live: screws 34 / links 28 refs absent at
  the default configuration, standoffs Hole 2 ERROR in live.
- driven-pattern 6/11: f6 Linear pattern 1 direction from Extrude 1's cylinder face: ours axis -z (origin at top), live
  +z: sent to core 06:35 (vibe5 ~/tmp/dx/qdp, mode 700).
- blend: impeller f26 matches on its 12b25c47e (not landed); f27 Fillet 3 next (blend). core: side-cuts-tab Corner break
  fix queued (7 -> 13/18).
- Hole internal bodies (committed 48a5ef471, asm OK, lands after the plane fix with its own corpus A/B): a SWEPT_FACE of
  `<hole>.holeBodies.*.opRevolve` maps to our Hole's faces by geometry (hole.fs' `core_line_2` THROUGH_FACE: narrowest
  cylinders; `start_line_2` counterbore wall: widest), every location (hole_body_faces in named_faces); a three-face
  INTERSECT vertex resolves sides through named_faces and common_vertex intersects the sets (gate: this also lets
  loft side sets in). TRACE lines "TRACE INTERSECT side / faces". 74d3d6 Plane 1 (vibe5 st74.sh, builds Q/S): all three
  sides translate (bore = Hole 8's cylinder, right), but no common vertex: our Multi angle joint sits near y 560 mm,
  live's centroid (20.3, 80.0, -4.7) mm, and our bore stops at the hexagon pocket (x 18) where live's crosses the
  Extrude 18 slot (y 560 ± 0.9): upstream placement (live's fetch of 74d3d6 is incomplete: 53 lines from Transform 16
  unfetched). Not mine until upstream matches. Not done: SWEPT_EDGE of holeBodies (1dc32b Fillet 5, refs cover it),
  start_line_1 (cbore floor vs csink cone).
- Plane feature planes before refs (committed, not landed): a `….planeOp` query (plain or packed DUMMY) maps to our
  plane before any refs geometry (live's all-feature refs read it as the plane body's face, which by_geometry can't
  find: c2-loft-guide-curves-wave-washer 2 -> 6/7). Gate: vibe6 corpus3.sh A/B (A build L, B build P md5 5e1d31c239eb)
  over corpus3 + live's newer corpora (~/tmp/dx/corpus3-{A,B}.{log,sum,untr}, out corpus3.out), queued in the slot
  since before 01:41 behind blend; land if SUM-SAME but for the planeOp documents improving. Then sort corpus3's first
  differences to owners.
- Second search (live, by sets): c2-parting-line-draft-tutorial 3/3; c2-parting-line-draft-parting-line 5/17, Draft 1
  (PARTING_LINE TWO_SIDED 20/20°): core's fix queued (the ruled sheet runs past the curve's ends; 367243.7 mm³, live
  367268.5): 9/17, next Extrude 2 surface 12113.30 mm² vs live 12115.66 (core's). c2-sm-hem-plus: sheetMetalStart / sheetMetalHem are custom (Feature Studios da39db76…,
  e5262684…) not fetched + its Derived source: asked live. c2-sm-side-cuts-tab 7/18: Corner break 1 (FILLET r 21 on two corners 42 mm apart, a full round; vertices = live's
  refs) fails in sheetmetal::corner_break: core (via blend). c2-sm-hem-plus 4/6 (custom features 2/2 with live's
  studios): Derived 1 differs because its source's Offset surface 1 makes 3 bodies for live's one: core; live fixes its
  "surface" bodyType in the derived lines.
  Lofts: helix-sweep 4/4, lofted-basic 6/6; wave-washer 6/7 with the plane fix (Loft 1: a loft path with more than
  two profiles, ours unsupported: core); paths-guides 4/29 (Loft 1 to a sketch point with TANGENT_TO_PROFILE: ours
  "Only planar profiles…": core; save vibe5 ~/tmp/dx/savelf); driven-pattern refetch pending. 7 config-parts to come
  (gears / bevels dropped: private studios).

## Open: my studios (studio-alone runs, vibe7)
- bjr-config 5d8bc1: 107/124 at Manufacture=false (23:42, live's retaken lines). First difference Extrude 55 "vanity
  plate front" 369/955/601, live 365/943/593 (bool / core, parked). Kerf Compensation 1 is suppressed at false (as live);
  Kerf compiles (live's 19:19 fetch); core's e138d597 redoes OCCT's empty metre offset in mm (their check: the true
  case 3704.237 -> 3745.156). Hole 95 matches live at false (dx/q-hole95). Text 1 / 2 blocked (private import
  403). Sheet metal model 1's REDG note is harmless (its first query, qSketchRegion, brings both regions; it builds).
  Left: Sketch 39's in-feature cPoint, Decal 1's image.
- infento 1dc32b: pulley = live through Fillet 5 (live's refs in studios/1dc32b….refs.jsonl, 15 lines, all hosts) and
  Draft 1 (core, -1306.543 mm³). Chamfer 3: blend (blend-ch3-wip, waits on live's mirrored-body side rule); Fillet 3
  after it.
- infento 74d3d6: 9/127 with loose boxes; first difference Extrude 4 "Knurled disc" 6/11/6, live 4/4/0 (core's up-to
  imprints, parked). Later: Sketch 15 (DIAMETER unsolved, constraints moved 49.98 mm, taken back).
- Studio refs' boxes are live's loose ones (evBox3d tight=false; only lines with "tight" in `source` are tight): the
  studio scripts run loosen.py (sets `looseBoxes`). The assembly suite matches studio parts by volume / centroid,
  not boxes: nothing to change there.
- General translator fix (open, mine): derivedFrom + ORIGINAL_DEPENDENCY queries resolve to the wrong type (1dc32b's
  Chamfer 4 EDGE became a hole wall, Fillet 5's became regions; patched with live's refs). They resolve silently, so
  find one through a first difference before working on it. A SWEPT_FACE / SWEPT_BODY of an opSweep has no named path
  (only opExtrude / opRevolve): live's per-feature geometry maps them when present (5d8bc1 Shell 1 does).

## Open: other
- c2-impeller-loft: f15's Boolean is a known live difference (bool, DECISIONS); 24/48, see Next 2.
- Exchange ("live imports ours") 43/48 (live re-import of the 14:51 files; the gear STL re-imported at 15° 22:0x,
  synced). Left: hose x_t (deliberate split rings); 65-tooth STEP area -0.17 mm² (lead: left); STL rows (gear,
  radijator, sheet metal plus): the suite exports Custom chord 100 / REST angle but Medium's 0.254 mm minimum facet
  width; live's REST has none that binds (radijator r 0.92 arcs in 0.08 mm segments). Floor 0.254: radijator 4,186
  triangles vs live 5,052; floor 0.0254: 5,338 (gear 11,124 vs 9,000). The rest is OCCT BRepMesh (analysis by 01:54,
  /root/dx-scratch/bin/stl{faces,loops,arcs,seg,curved}.py): (1) BRepMesh_CurveTessellator's OCC287 minimum of 4 points
  on a circle / conic (radijator's 18 2.5° arcs: 3 segments, live 1: +144 triangles); (2) the 45° cone (z 11-15) gets 3
  rows, live 1 (+150). Parked P3, no OCCT patch (lead, DECISIONS 7210b453e): STL rows compare by area, volume and
  deviation within live's error, not triangle counts. Live's REST STL default: 5°, 15° over 500
  exported faces (live2 /root/onshape-research/live/exchange-kw/README.md), the default chord (chord 100 in ours
  matches at 15° within 0.4 % per part); a tiny feature in a large export goes coarser (r 0.05 in a 520 mm studio,
  dx/q1137; none of our rows). Sync the hosts' live-ours*.json after every live re-import.
- cf-spur-gear 13/14 since live's 19:28 refs (Sketch 2 near-singular, "could not be solved" vs "over defined" by
  rounding): skasm's 62893b1ed takes back over-defining model-geometry constraints. Its in-feature cPoint (CAP_EDGE of
  the Spur Gear's own `….blank` extrude) is untranslated: low value.
- drw-centermark regression (qa, greens 45aa7a0e..f8cbf3c6): sent back pointing at studios' f2b9b1087 (take page
  height / click mapping); mine in the range don't touch centermarks.
- Drawing exports 84 / 84 (exp-1115r + the revision / spline / jog runs). drw-broken-out-section compares within ±1 px
  (DECISIONS 11-15). Studios' view fit (69f67433) in.
- Speed: cf-part-draft's Spur Gear 452 -> ~236 ms (live 227). Left there: its opFillet blend 65-120 ms (blend) and
  model-core's own 28 region profiles. cf-snap-hook ×3.4: Snap Hook Booleans + Shell 1 -> bool (took it), fillets ->
  blend (FYI). draft_robust whole-body is_valid: core's 77715251. The kernel profile's per-feature block also counts
  the suite's measurement after the feature (boxes, props, display mesh): compare with the feature's own regen ms.

- Corpus widening (lead via live): live captures 5 public documents for the least-covered features, approved by me
  (before 23:20): c2-surf-workshop-<studio>, c2-surf-intro-<studio>, c2-rib-wrap, c2-sm-tablet-stand,
  c2-sm-rocker-enclosure (in that order; assemblies only if asm wants them), each with its configuration definition,
  `__liveConfiguration` and refs.jsonl; then a second search (sweep twist, loft path/guides, sheet metal rip/corner/hem,
  a configured part). Run them alone once in, sort first differences to owners.

## Next, in order
1. x1117c (vibe6 04:46, main + 59c0be35 + 9a19e358): ours imports live's 59/75, live imports ours 46/48, ours reads
   ours 114/119, DXF 0/4, times 49/50. Queued abb5303e (lead 11-17 decision 1): an import row passes when ours splits
   no more than live's own import of the file (counts between the model's and live's, volume / area as live's import
   is held; PARITY-SUITES.md): rings-loft live.igs now passes (ours 275, live's own 279/787/524 with its ring 565.6 mm³
   for 440.8, model 273). Waiting on live2's own SAT imports (REQUESTS 01:47): rings-loft / sm-louver live.sat.
   Parked (lead 11-17 decision 2, "not now"): rings-loft live.step's revolved ring reads 1450.036 mm² for the model's
   1449.943 (band +-0.044): live.step has no pcurves and its plane / extrusion edges are B-splines ~3 µm off the true
   intersection (18 small planes 1.8157 mm² each for 1.8108); live's import gets the model's area, so it re-derives
   them. Re-deriving such edges from their two faces on import would be a healing feature: wait for more cases.
   extension-spring live.sat area +0.016 % (ACIS procedural approximations); hose SAT.
   Drawings: all four follow model differences (extension-spring Fillet 1 10/24/16 for live's 11/27/18: blend;
   rings-loft walls) or live rules without evidence (housing-draft lug chords, hole rim: lead, wait for more docs).
   View placement waits (lead).
2. Queued 4dd4d7ef: MCP-QUOTED-VALUE (sketch dimension values sent in quotes). P1 PARITY-CUSTOM-RUN-FAILURES
   (custom features 105/121), 14 of the 16 in config-parts (wait for live's retake), and:
   - cf-surface-pattern Surface Pattern 3 129/386/259 for 131/387/258: the same 85 sphere dimples (Part 2, r 14.3 mm;
     the nearest grid point 0.15 mm inside the face's edge), removed volume 18683.9 mm³ for live's 18681.6. Our
     Thicken 1 is already +24.3 mm³ / +5.5 mm² off (loft surface, in band); our faces: 86 sphere (one dimple in two),
     42 B-spline, 1 other, with knife-edge pieces (a 0.000625 mm² sphere sliver at (65.410, 43.071, 4.933), a
     0.000741 mm² island). Live 10:10 (dx/q-cfsp/faces.json): Loft 1's Surface 1 20480.0518 mm² (ours 20482.3596:
     our loft surface +2.31 mm², core's natural_loft); after Surface Pattern 3 85 SPHERE / 45 SPLINE / 1 OTHER (ours
     86 / 42 / 1), its 13 faces under 1 mm² all SPLINE, no sphere sliver. Follows the loft: core's, parked here.
   - cf-setback-fillet: parked (lead 11-05, skasm's RABC fill patch; TICKETS SETBACK-FILL-PATCH).
2. After ~10:17: live's answers to REQUESTS 01:47 (sm-louver scale, live's own SAT import) and 05:46; set 2 after
   live2's refetch.
3. Landed 11-17: 4ddf9f2d curve pattern along a helix (frames from the helix's axis; kernel fit's fallback pair, old
   pair kept when it works): driven-pattern 7/11 -> 11/11, the other 7 helix docs unchanged (vibe6 A/B). 2089f117
   Parasolid import heals a bodies-only file once (the assembly check stopped healing): rings-loft x_t suite import
   on the Mac 7.9 -> 6.6 s (live 8.6); qa's `documents import` on vibe6 not re-timed. SM-BOOLEAN-SPEED closed (Mac
   Up to Part 66 ms / live 160). Studios 06:27: 1dc32b 27/349 (was 17), Boolean 4 hub 232/632/400 vs 216/600/384,
   2666 ms vs 135: sent to bool 7fa8ceab 08:10 (repro vibe7 ~/agents/dx-keep/repro/1dc32b-b4); 74d3d6 9/180 (core's).
4. Writer guard for kinked closed seams (lead 11-17: under 1 mrad, no pole over 1 um = a tenth of live's 1e-5 m
   band, export only; uncommitted in dx-land xt.cpp periodic_bsurface smooth_seam + kernel test export::a_tube_wall_
   with_a_slightly_kinked_seam_goes_round_as_one_acis_face): prove hose x_t / sat read back 12/12/0 (were 12/14/4,
   22/44/24; seam 0.41 mrad, poles moved 0.21-0.52 um), no other exchange row changes. Core's SWEEP-SEAM-KINK filed.
   Then rings-loft "ours reads ours" x_t / sat Part 2 133/338/208 for 132/335/206.
5. asm (P2, 11-17): asm-robot-relations studio 0c3466 Import 1: the STEP (studios/foreign/0fc7dd7c….step, SolidWorks,
   one MANIFOLD_SOLID_BREP 'Combine1', one CLOSED_SHELL of 174 faces) is 6 disconnected pieces: live makes 6 parts
   all "REV-21-1650", ours one (171986.6 mm³), so Thicken 1 fails and the BOM's 6 rows become one. Split an imported
   solid's shell into its connected pieces (Shape::faces_oriented_consistently gives one shell per piece), each a
   solid named as the file's. asm's trace: vibe7 ~/agents/asm-keep/one/rr2.log.
6. config-parts after live's retake at defaultValue (16c51e2a / 8d3a0ec2 landed); 1dc32b / 74d3d6 studio reruns.

## How to run
- Builds and crate tests on the Mac: `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p <crate> …` (one slot).
  Linux ops binaries for vibe runs: `NOSHAPE_RB_HOST=vibe7 tools/tiers/rb.sh cargo test -p noshape-ops --test all
  --no-run`, then copy ~/agents/dx/target/debug/deps/all-<hash> to ~/agents/dx-keep/ops-exch (studio scripts' binary)
  or ~/tmp/dx/ops-A / ops-B for an A/B. rb.sh keeps mtimes: `touch` an edited file before the A rebuild, check md5.
  An A/B baseline is rebuilt from the same base (land.sh rebases).
- Heavy runs on vibe hosts through `~/bin/slot <cmd>` (one per host, waits for free memory; DECISIONS 11-15).
- Studio-alone runs (vibe7, scripts in ~/agents/dx-keep/bin, model dirs ~/agents/dx-keep/m*/st-*): st5d8.sh,
  st74.sh, st1dc14.sh copy the corpus studio into a model dir, run loosen.py, save to ~/tmp/dx/save*; one line of
  result in ~/tmp/dx/st*.log (grep "features match").
- Parity models: NOSHAPE_PARITY_ONLY=<docs,…> NOSHAPE_PARITY_MODELS=~/parity/models <ops binary> parity::parity_models
  --ignored --nocapture; NOSHAPE_PARITY_SAVE=<dir> saves the translated document; NOSHAPE_PARITY_TRACE=1;
  NOSHAPE_KERNEL_PROFILE=1 (kernel calls per feature), NOSHAPE_FS_PROFILE=fn (FeatureScript lines). A saved document:
  `~/agents/dx-keep/cli-main call feature_get '{"doc":…,"feature":…}'` on a copy (CLI batches save in place).
  cli-main is a release build of main 21:45 (rb.sh `cargo build --release -p noshape-cli` on vibe7, 20 min); a stale
  one regenerates without newer fixes. Tools: feature_list, feature_get, feature_edit, feature_rollback,
  query_parts, query_entities, sketch_get, export_parts; `batch` file of {"tool","args"} lines.
- Gates for custom-host changes: dx-keep/bin/gate-rf.sh (all 47 cf models, A then B, scores diffed).
- Exchange suite (vibe6): `cp -a ~/parity/exchange ~/tmp/dx/exs` (the suite writes ours.* there), then
  `~/bin/slot timeout 6000 ~/agents/dx-keep/bin/exch.sh x <tag> ~/tmp/dx/exs` (log ~/tmp/dx/exch-<tag>.log).
  /root/dx-scratch/bin/stlinfo.py <stl>: triangles, box.
- Drawing exports: /root/dx-scratch/exprun.sh <out> <scenario…> (EXPHOST=vibe7; builds the app on the host, runs
  ui-parity, compares with tools/drawing-export/compare.py against /root/parity/ui/live).
- perf gives no callers on these binaries and vibe7 has no gdb: attribute by NOSHAPE_KERNEL_PROFILE counts, or a
  temporary backtrace build.
- vibe7 → vibe6 scp fails: relay through /root/dx-scratch/bin-relay. Host ~/tmp is pruned after 6-12 h: keep scripts
  and binaries in ~/agents/dx-keep.
- Agent ids (message_agent needs them full; 11-17 09:25 restarts): core 6692ae35-758a-4442-81c8-29f67be2e21a,
  asm d8d597e0-675c-497b-a804-544ceab99bbd, blend 0e6e9942-0713-46c1-ae55-132fbbb4a14b, bool
  7fa8ceab-4549-499f-9b01-3a289c2c95b4, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, live
  e8549719-ef9a-42de-960c-e24b29f142a6, live2 3b9b1e66-33a0-4dfe-9e0a-eee216e354b2, studios
  58e9bca8-346c-4847-8012-e15006ae61ef, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b,
  psui 4973e295-9e92-463f-a13f-2ec5258bc5f9.
- The archive's "Parity translator changes by asm" section lists asm's translator fixes one line each (lead 11-07).
