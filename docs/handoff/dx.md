# dx handoff: agent API + drawings + data exchange (current state)

State as of 2026-11-20 07:4x UTC. Rewritten, not appended (DECISIONS.md 11-15); history in docs/handoff/archive/dx.md.

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md (generated), drawings and their export (crates/drawing,
crates/model-data/src/drawing), import/export, configurations, Variable Studio, Derived, start window and document
visuals, the parity translator (crates/ops/src/parity.rs) for my studios. The drawing UI is studios' (11-12).
Studio split with asm (lead 11-15): infento 1dc32b / 74d3d6 and bjr-config 5d8bc1 are mine; isaac-ball (3c5c58,
1e81ca, 90f78b) and config-assembly 20e932 are asm's. asm gates parity.rs changes before they land.
Before any crates/drawing land: the full `cargo test -p noshape-ops --test all e2e::` (the queue may skip ops).

## Now (12:5x 11-20): FRAME PROFILES (lead "build it") + vibex8 suite speed
- Suite speed done: Mac release r1 = 2 docs >= 2x/100 ms (impeller, config-screws); routed bool/blend/core/skasm.
  RUNNING: vibex8 release suite x1 (local log /root/dx-scratch/pm-x1.log, vibex8 ~/agents/dx-keep/pm-x1.log +
  pm-x1/report.json; script pmx.sh <tag> [only]); next: filter >= 2x & >= 100 ms, profile new ones, route, report.
  DON'T rb.sh dx-land to vibex8 until x1's build is done (resync). impeller Fillet 3 39/113/71 is load-dependent (blend).
- LANDING 3b980fee (QUEUED): Parasolid blend face side from tangent neighbour (replace-face Import 1 = live) +
  sketch_create on {"mate_connector": …} (hgr Bottom). Replace face 1 routed to core.
- UNCOMMITTED -> committed locally as "frame profiles" (not landed): parity.rs library_profile / library_size /
  size_numbers (Onshape's standard profile docs -> frame-library:<std>/<profile>\tSize=…), unit test
  standard_frame_profiles_are_the_built_in_library; studio_sketch builds a derived/<element>/ studio on first use.
  RUNNING: vibex7 fx2x.sh (unit test + pmx fxf for hgr, saw-table, cov-frame-endcap/gusset) -> /root/dx-scratch/fx2.log
  (synced BEFORE the derived-studio change). live fetched hgr's derived/{292796f4…, aaabdce9…} (Frame 1/5 studios).
  Next: rerun fxf on vibex7 with all changes; check ANSI tube Rounded (ours: ISO only) and AISC "Default" size vs live;
  then land and report to lead.
- Mac off limits until 13:44 UTC (lead): no rb.sh/ssh/scp to the Mac.

## Earlier (00:10 11-20)
- Fan-blade HLR sampling: TICKETS P3. surf-intro: see the 03:0x bullet below (archive has the 00:10 rerun).
- LANDED 04:49 as d9b795e0 (c63e548b, 8 commits): fb8f84f42 (surf-intro refs + open-profile loft), BRAND-IN-UI-TEXT
  (qa verifies), hex chain_pieces both ways (9023ff0d, 5dd3534a 04:22), STEP one-loop torus, HLR P1 = OCCT patch 0036
  (HLRBRep_Data::RejectedPoint: an IN end crossing tangent to a boundary's middle, unpaired, rejected; snap-hook 7 stray
  pieces -> 0, rings-loft Right 159 -> 151; libs installed by the lead on vibe2/vibex6-8, /root/occt, Mac, *.pre0036).
- snap-hook model mirror (all 4 hooks about their own x; Shell OFFSET_EDGE hookVector from evAxis, our edge opposite
  Parasolid's, /root/dx-scratch/snaphook.fs line 96): reported to the lead 00:50. faceblend Right's extra line: blend's.
- sm-louver DXF (05:0x, PARKED, small): geometry as live except the Right view's flange tangent edge: HLR hides 0.64 mm
  of it at each louver (10 pieces; live one 0.73 in line) and the DXF loses its 17 mm bottom piece (0.35 mm on paper).
  Not on our STEP re-read (one piece) -> the model's own shape. The type-count gap is sub-0.001 in stubs (live writes
  tangent edges seen end-on as 0.0005 in LINEs: TANGENT 121/243). Probe diff /root/dx-scratch/smlv-probe.diff
  (DXPROBE in project_bodies + kernel hlr probe), Mac dx-keep/exone.sh (exchange suite on exs-one = sm-louver).
- surf-intro (03:0x, blend's 6b6b3b4a0 + fb8f84f42, Mac sitrace2.sh, si-blend.diff applied for the run): all 5 past
  Derived 1, rebuild 300-490 ms (live 359-1202). Next stop in 4: Tail fin extrude UP_TO_VERTEX 3483.23 vs 3567.19 mm²:
  the car's rear vertices (y 254, z 30.343) at x +-33.142, live +-33.941 (Fillet 1's end): sent to blend 03:0x.
  offset doc: Offset surfaces 1 9588.57 vs 9591.44 (likely the same). CLI queries: Mac dx-keep/siq.sh, si-q/.
- P1 Gear Lab DONE on my side (06:2x): 8f5622 18.1 s (live 1153 ms); interpreter ~60 ms, the rest 2 OCCT fuses:
  body ∪ its herringbone mirror 14.4 s (occt), core ∪ 40 teeth 3.3 s (occt_tree). Sent to bool 06:21 with the
  recorded cases vibex7 ~/agents/dx-keep/gl-bool (.brep/.txt). gl13 2d7dc1: 0.8 + 0.9 s, live 515 ms. Repro: vibex7
  ~/agents/dx-keep/gl.sh <tag> [gl40|gl13] (parity_models + NOSHAPE_FS_PROFILE; NOSHAPE_RECORD_BOOLEANS for cases).
- URDF-RIGID-SUBASM-SINGLE-LINK LANDED b7c2c4e8a (ticket [x]); open: live's file shape (REQUESTS dx 05:44). Nightly
  dump-test LANDED 3b9ec80cc; dx-nightly worktree + vibe2 target removed 07:4x.
- EXCHANGE TOP-3 LANDED 09:29 as 072a6c50 (2f406fa1). Rings "ours.stl" face 23 no triangles: bool's Shell builder, low.
- DXF SCORING (DECISIONS 11-20) QUEUED bdf50182 + 6047caa6 (with the hole-rim circle, the tangent-beside-a-curve HLR
  fix, the isaac speed fix, five-point ARC fit). Mac exall-b1: DXF 2/10 (duct, louver); ops e2e 262/262. Rows left:
  snap-hook 0.107 in of the Iso's tangent ellipse (edge 52) dropped as lying on the fillet's own silhouette, which
  touches it within 0.16 mm (model); live draws both. No tolerance separates it: on-curve tol x0.02 / x0.1 -> snap
  0.033 / 0.067 in but faceblend +0.027 / +0.028, rings-loft +0.029 / +0.019 in of tangent pieces live drops (parked); bottle-split
  Front's lip arc: P3 HLR-TORUS-SELF-OCCLUSION (lead 11-20: parked; collect other drawings showing it for one patch); extension-spring an extra seam LINE + an edge live splits
  into spline + line (model topology); hex-bookcase a 0.05 in rim quarter on the dowel mesh (coincident, our
  under-the-mesh rule hides it); housing-draft live's 4-LINE runs + 0.057 in bow (projection); faceblend ELLIPSE vs
  SPLINE (model); rings-loft model; clamp-guillem Top edges inside another part (known). Probe: model_edge eprintln
  of curve + face surfaces under DXEDGE (not committed; recipe in this line), Mac `export DXEDGE=1; exall.sh`.
  Tools: /root/dx-scratch/bin/near.py, dxfgeo.py; files /root/dx-scratch/dxf-a9/.
- DRAWING SPEED (lead 11-20): P1 ISAAC-DRAWING-VIEW-SPEED fixed, queued (bdf50182) (shared assembly evaluator: ops
  doc.rs prime_sources, app-core DocModel::assemblies, assembly_view / drawing_view take it; tell asm on landing):
  isaac Front 37.4 -> 1.60 s (live 10.7), Create 37.5 -> 2.49 (18.2); infento 19.5 -> 4.3. Timing:
  dx_perf::timed_saved_drawing_views (uncommitted) + Mac dx-keep/dvspeed.sh (copy /root/dx-scratch/dvspeed.sh).
  All others under live's (TICKETS row).
- The Mac's dx-main release target links a PRIVATE TKHLR (OCCT_ROOT=~/noshape-host/dx-keep/hlr/root, counters only
  with DXHLR=1); rebuild with the normal OCCT_ROOT for timings.

## In flight: data exchange (lead 08:0x)
Task: 10 corpus docs x every format + a drawing vs live's exports; re-import live's STEP / x_t; times vs live's.
Data /root/parity/exchange/<doc>/ (live.*, live-self.json, live-times.json, live-drawing.dxf); suite
parity::exchange_round_trip (dx-keep/bin/exch.sh). Landed history: archive (11-17 05:52 and 16:1x blocks).
Open, ranked:
1. Drawings DXF 0/4 (Mac exs-d, 4 docs, with the queued conic-tolerance commit): sm-louver ARC 68 / CIRCLE 2 as live,
   ELLIPSE 96/86, LINE 2539/2472, MTEXT 52/54 (suggested scale now 1:50 as live: af6b564d). housing-draft ARC 0/3, CIRCLE 5/6, ELLIPSE 16/15, LINE 159/154, SPLINE 11/7. extension-spring SPLINE
   37/47 (live splits more), LINE 120/117. rings-loft ARC 37/91, ELLIPSE 135/95, SPLINE 189/211: model (item 4; our
   true circles where live's model has B-splines get conics, live writes SPLINE).
   Tools: /root/dx-scratch/bin/dxfdiff.py ours live drawing.json (per view, bbox-aligned; live's -Z extrusion
   ELLIPSE / ARC handled), dxfplot2.py (a view side by side by type), dxfboxes.py / dxflayers.py (per-view boxes).
   housing-draft placement / lug / rim findings (11-17 03:00): archive.
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
- c2-config-parts-screws: scored again (05:5x; vibex7 screws.sh: 362 MB peak, 2959 ms, live 266): 27/63, Sweep 1 "not
  a valid solid" (OCCT check) in 2409 ms (live 42): core / bool, via the lead.

## Other in flight
- config-parts: see "config-parts at live's 18:0x retake" below (the 11-17 vibe5 runs: archive).
- driven-pattern 6/11: f6 Linear pattern 1 direction (ours -z, live +z): core's (11-17 06:35).
- Landed: Hole internal bodies (8fd592298) and Plane feature planes before refs. corpus3 on main (vibex8 15:28,
  ~/tmp/dx/corpus3-A.{log,sum,untr}; corpus3.sh <ops-main-1119>): 14/39 to the end, 436/638 features. Mine: surf-intro
  boundary / fill / offset / ruled stop at 5 "Derived 1" (the derived source b7635b82… has no refs.jsonl: live's FS
  evaluation refused on a linked version; REQUESTS 16:1x to live2) and mutualtrim's Derived 1 (source not fetched).
  Others' (to the lead): lofts (wave-washer 6, paths-guides 16, surf-workshop-pipe 9 >2 profiles with guides,
  surf-intro-simple 14), fills (surf-intro-examples 5, surf-workshop-fill 3), Move boundary non-planar (moveboundary 6),
  surface split / sweep / mirror topology (britatap 4, cpcurves 9, singularities 2 Mirror 3: 2 parts for 1), sweep / loft
  area tails (knifehandle 3, manualblend 11, workshop-loft 6, workshop-sweep 2), Boundary surface conditions (bridging 7),
  parting-line Rib 1 (13), rib-wrap Wrap 1 (10), nuts Sweep 1 (14): core; mutualtrim Fillet 1 BOP (7): blend;
  rocker-enclosure-sm Extrude 2 (8): skasm.
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

## Open: my studios (studio-alone runs, vibex7)
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
## Next, in order
C. PARKED (lead 11-19 21:0x): TICKETS FAN-BLADE-VIEWS-HLR-SAMPLING has the numbers; work in /root/dx-scratch/hlr-retry.
Q. asm-robot-relations Import 1: closed by DECISIONS 11-17 (a valid solid stays a solid; ours one solid = the ruling).
H. Hidden lines vs live (live2 02:32, live/dx/louver-hidden-1119/: sm-louver Front + Top, Show hidden lines, DXF 2018).
   Hidden: live 1348 LINE + 3 ARC + 75 ELLIPSE (105.85 in), ours 774 LINE (105.34 in), each on the other's to 0.010 in.
   TANGENT_HIDDEN (03:4x, hidden tangent edges drawn dashed with tangent edges shown, not under a visible tangent
   edge): live 188 LINE 244.43 in, ours 244.42 in, each on the other's. /root/dx-scratch/bin/hidcmp2.py ours live tol
   LAYER. Hidden curves: live's 75 ELLIPSE / 3 ARC are noise (0.0002 in long, r <= 0.003 in: edges seen end-on), skipped. Runs: Mac exsh2.sh with
   /root/dx-scratch/dxhidden.patch applied (the exchange suite with DXHIDDEN=1), dumps /root/dx-scratch/dxfm/.
0. Speed vs live (lead 11-18 06:1x; release, vibex8, dx_perf, same run; live behaviours "speed: drawings", "dx speed:
   drawing creation and NIST import", "dx (2026-11-02, P1 drawing speed on big models)"):
   - bracket drawing, cold (no cached view): create 10 ms (live 30.1 s, server 3.0), insert view 7 ms (3.0 s),
     Update after a Fillet 1 change 12 ms (16.9 s), reopen + views 11 ms (tab revisit 3.2 s).
   - big: 2000-instance assembly views 2.95 s (19.7 s), BOM 0.34 s (13.1 s), dimension 540 -> 212 ms (0.8 s:
     old_state memoized, the reply's sheet model reused), 1000-part studio views 1.3 s, Update 1.5 s, export PDF 0.43 /
     DWG 0.16 / DXF 0.22 / PNG 1.05 s (7.2-18.3 s).
   - imports: NIST STEP 4.15 / x_t 2.16 s (38.7 / 24.5 s); live big asm STEP 2.8 / x_t 1.7 / IGES 7.1 s (33.5 / 12.9 /
     69.3 s); exchange suite 173/174 at or under live's.
   - Corpus drawing views (lead 11-18 18:0x; dx_perf timed_corpus_drawing_views, Mac release, cold views after the
     studio's regeneration; live2 asked in REQUESTS 18:07, no live times yet): impeller Front 150 / Top 121 / Right 170
     / Iso 127 ms, Update after a suppression 194; rings-loft 393 / 291 / 448 / 474, Update 612; snap-hook 22 / 10 / 31
     / 28, 36; xDev Bushing <2, guide adapter 6 / 4 / 17 / 6, 19. Fixed: the agent API's Insert view into an empty
     drawing regenerated the studio afresh (impeller 9.7 s): SheetModel::build primes `extra` (the app's Insert view
     already uses the open studio's committed_context). Next 8 by faces: fill-pattern 1.9 / 2.4 / 1.8 / Iso 11.3 s,
     Update 7.1 (62 % BRepTopAdaptor_FClass2d::Perform: every hole wire box-tested per classify, a BRepAdaptor_Surface
     per call: OCCT patch, wire-box grid); fan-blade-sweep ~5.5 s each, Update 8.6 (HLRBRep_Data::Classify's exact
     line / B-spline intersections); straight-spline 0.7-1.7 s, Update 2.0; drive-train 1.3-2.0 s; surface-pattern
     0.6-0.7; spur-gear 0.4-0.6; sm-louver < 0.14; socket-head < 0.01. Profiles: Mac dx-keep/dvs-<doc>.txt
     (dvsample.sh, /root/skasm-scratch/bin/sample_under.py). live2's times (behaviours "dx drawing views on corpus parts"): impeller / rings / snap-hook / xDev
     1.3-2.6 s a view, Update 4.4-4.9 (ours 10-20x faster); fill-pattern 7.7 / 4.0 / 3.9 / 7.2, Update 9.6 (ours Iso 11.3 =
     1.6x: under 9e5b481d4's 2x); fan-blade 2.0 / 1.7 / 1.6 / 2.1, Update 4.4 (ours ~5.5 / 8.6 = 2.6-3.4x: the one gap; C).
   - c2-rings-loft live.x_t import 17.5 -> 7.9 s on vibex8 (live 8.6; STEP 2.7): tube at 10 nm (f91b8d66) and intersection curves
     fit to 10 nm segment by segment (aad79780, queued fbf75841; blend faces' pcurves 13.7k -> 4.6k knots, STEP 2.4k).
     Left vs STEP: ShapeFix projects every pcurve with >= 23 knots and our 11-point curves with 45; STEP's edges are
     single cubic spans with ~10 um tolerances (ours 10 nm). Next lever if needed: pcurves on the tube from on_both's
     (u, v) at the 3D curve's parameters instead of ShapeFix's projection. A/B tool: vibex8 ~/agents/dx-keep/xtab
     (run.sh, ~/tmp/dx/xtcmp.py), dump patch idea in /root/dx-scratch.
1. DXF rows vs live2's model truth (behaviours "dx + studios (lead 11-18 ~13:45)", live/dx/drw-truth-1118). m9
   (vibex8 ~/agents/dx-keep/exch-m9.log, files ~/tmp/dx/exs-m9; tools /root/dx-scratch/bin/dxfdiff.py, dxfplot2.py,
   dxfonlonger.py; DXPROBE patch /root/dx-scratch/dxprobe.patch dumps drawing_get per doc), 90bc06a7 queued 17:4x
   (a107ed7e failed on psui's box-drag tests mid-land, they pass rebased):
   - Torus-only slanted outlines as SPLINE: snap-hook ARC 82 and bottle-split ELLIPSE match, faceblend ARC 18 / 17.
   - drop_lines_on_longer (a line wholly on a longer one, same view and style, not `shared`, written once):
     sm-louver Visible LINE 2175 / 2178 (Front's 144 pieces on the plate edge), clamp-guillem 263 / 268 (the shared
     pairs live keeps kept), extension-spring 114 / 117 (its 6 were overlaps on the extra end rectangle: core).
   - sm-louver TANGENT_LINES 117 / 243: Front same length (78.3 in) in fewer pieces, Top's 32 live end-on stubs
     (noise). Top's bend tangent (y 6.09203 in) FIXED 00:4x: the model edge was there, drop_covered_hidden took it
     as lying on the wall's outline 1 mm away (cover tol = deflection, 1.8 mm); exact straight-on-straight now within
     size x 1e-6 (HlrOut.exact says the view really was exact). DXF: louver LINE 2356 -> 2360, snap-hook 351 -> 352,
     clamp 263 -> 265, every added line live's but louver's x 7.361 / 7.591 tangents, re-cut (live: one line
     running through the visible stubs on it; ours pieces, before and after). Hidden edges grow (louver 4321 -> 5224).
   - Edges inside another part (live overlap-1119 boxes, overlap2 wedge, overlap3 round / chamfer, clamp-guillem): no
     single rule fits live (boxes hidden; wedge drawn in Top; a box through another's round hidden over all of it in Top,
     even the piece outside; clamp-guillem's plate edge drawn in Top, hidden in Iso). OCCT's plain hiding matches 4 of 5;
     the outline-rule pass (357d69ab) matched 3 and contradicted overlap3: reverted 11-19 06:32. clamp-guillem's Top stays
     a known difference. Probe /root/dx-scratch/cg/cgprobe.cpp (vibex8 rr/).
   - Mine after all (core 22:3x: the part = live through every Draft, 22 real circles, 36 lines): housing-draft's
     0.499 in edges bowing 0.3-0.5 mm (live LINEs) and the Right-view hole rim as a SPLINE (live CIRCLE) come from our
     projection (at most Rotieren 1's grazing cut, bool's parked BOOL-ROTIEREN-TANGENT); extension-spring's Top has a second end rectangle (x 2.230 / 2.268) and
     Fillet 1 (blend); bottle-split's lip outline is live's noise (18 tiny pieces); hex mesh splits (no rule).
1. Exchange m5 (vibex8 06:0x): imports 114/130, live imports ours 46/48, ours reads ours 148/154, DXF 1/10 (duct).
   DXF rows left follow model differences: faceblend full-round fillet edges B-spline in live (blend told 06:1x),
   extension-spring Fillet 1 10/24/16 vs 11/27/18 (blend), rings-loft walls, hex mesh outline splits (live's breaks
   follow its facets' normals noise: no rule), housing-draft lug chords, hole rim (lead: wait for more docs).
   Parked (lead 11-17 #2): rings-loft live.step ring area +0.093 mm²; extension-spring live.sat area +0.016 %.
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
6. config-parts after live's retake at defaultValue (16c51e2a / 8d3a0ec2 landed); 1dc32b / 74d3d6 studio reruns.

## How to run
- Builds, crate tests and exchange runs on vibex8 (lead 11-17 17:00; ~/parity/exchange + its 22 models copied there);
  vibex7 only for ~/lead-private; land with `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh` (lead 21:23).
  Linux ops binaries for vibe runs: `NOSHAPE_RB_HOST=vibex8 tools/tiers/rb.sh cargo test -p noshape-ops --test all
  --no-run`, then copy ~/agents/dx/target/debug/deps/all-<hash> to ~/agents/dx-keep/ops-exch (studio scripts' binary)
  or ~/tmp/dx/ops-A / ops-B for an A/B. rb.sh keeps mtimes: `touch` an edited file before the A rebuild, check md5.
  An A/B baseline is rebuilt from the same base (land.sh rebases).
- Heavy runs on vibe hosts through `~/bin/slot <cmd>` (one per host, waits for free memory; DECISIONS 11-15).
- Studio-alone runs (vibex7, scripts in ~/agents/dx-keep/bin, model dirs ~/agents/dx-keep/m*/st-*): st5d8.sh,
  st74.sh, st1dc14.sh copy the corpus studio into a model dir, run loosen.py, save to ~/tmp/dx/save*; one line of
  result in ~/tmp/dx/st*.log (grep "features match").
- Parity models: NOSHAPE_PARITY_ONLY=<docs,…> NOSHAPE_PARITY_MODELS=~/parity/models <ops binary> parity::parity_models
  --ignored --nocapture; NOSHAPE_PARITY_SAVE=<dir> saves the translated document; NOSHAPE_PARITY_TRACE=1;
  NOSHAPE_KERNEL_PROFILE=1 (kernel calls per feature), NOSHAPE_FS_PROFILE=fn (FeatureScript lines). A saved document:
  `~/agents/dx-keep/cli-main call feature_get '{"doc":…,"feature":…}'` on a copy (CLI batches save in place).
  cli-main is a release build of main 21:45 (rb.sh `cargo build --release -p noshape-cli` on vibex7, 20 min); a stale
  one regenerates without newer fixes. Tools: feature_list, feature_get, feature_edit, feature_rollback,
  query_parts, query_entities, sketch_get, export_parts; `batch` file of {"tool","args"} lines.
- Gates for custom-host changes: dx-keep/bin/gate-rf.sh (all 47 cf models, A then B, scores diffed).
- Exchange suite (vibex6): `cp -a ~/parity/exchange ~/tmp/dx/exs` (the suite writes ours.* there), then
  `~/bin/slot timeout 6000 ~/agents/dx-keep/bin/exch.sh x <tag> ~/tmp/dx/exs` (log ~/tmp/dx/exch-<tag>.log).
  /root/dx-scratch/bin/stlinfo.py <stl>: triangles, box.
- Drawing exports: /root/dx-scratch/exprun.sh <out> <scenario…> (EXPHOST=vibex7; builds the app on the host, runs
  ui-parity, compares with tools/drawing-export/compare.py against /root/parity/ui/live).
- perf gives no callers on these binaries and vibex7 has no gdb: attribute by NOSHAPE_KERNEL_PROFILE counts, or a
  temporary backtrace build.
- vibex7 → vibex6 scp fails: relay through /root/dx-scratch/bin-relay. Host ~/tmp is pruned after 6-12 h: keep scripts
  and binaries in ~/agents/dx-keep.
- Agent ids: only in /root/lead-scratch/roster.md (lead 11-19 22:4x): read it before every message_agent.
- Mac exchange runs: ~/noshape-host/dx-keep/exx.sh <dir> <log> (kernel tests, release build, suite; stops on a build
  error); dirs exs-all (all docs), exs-x (hose + snap-hook).
- The archive's "Parity translator changes by asm" section lists asm's translator fixes one line each (lead 11-07).
