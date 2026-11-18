# dx handoff: agent API + drawings + data exchange (current state)

State as of 2026-11-18 11:00 UTC. Rewritten, not appended (DECISIONS.md 11-15); history in docs/handoff/archive/dx.md.

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md (generated), drawings and their export (crates/drawing,
crates/model-data/src/drawing), import/export, configurations, Variable Studio, Derived, start window and document
visuals, the parity translator (crates/ops/src/parity.rs) for my studios. The drawing UI is studios' (11-12).
Studio split with asm (lead 11-15): infento 1dc32b / 74d3d6 and bjr-config 5d8bc1 are mine; isaac-ball (3c5c58,
1e81ca, 90f78b) and config-assembly 20e932 are asm's. asm gates parity.rs changes before they land.
Before any crates/drawing land: the full `cargo test -p noshape-ops --test all e2e::` (the queue may skip ops).

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
- Not bugs: sheet-metal-plus ours.3mf read back 554665 for 626770 mm³ (the suite's fallback expectation: Part 2 inside
  Part 1 is its hollow, as live's import of ours.obj does; wait for live's 3MF import of ours). ours.obj "3 parts, live
  2": ours reads the zip by groups (live refuses zips), live was given the bare .obj.
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
  find: c2-loft-guide-curves-wave-washer 2 -> 6/7). Gate: vibex6 corpus3.sh A/B (A build L, B build P md5 5e1d31c239eb)
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
0. Speed vs live (lead 11-18 06:1x; release, vibex8, dx_perf, same run; live behaviours "speed: drawings", "dx speed:
   drawing creation and NIST import", "dx (2026-11-02, P1 drawing speed on big models)"):
   - bracket drawing, cold (no cached view): create 10 ms (live 30.1 s, server 3.0), insert view 7 ms (3.0 s),
     Update after a Fillet 1 change 12 ms (16.9 s), reopen + views 11 ms (tab revisit 3.2 s).
   - big: 2000-instance assembly views 2.95 s (19.7 s), BOM 0.34 s (13.1 s), dimension 540 -> 212 ms (0.8 s:
     old_state memoized, the reply's sheet model reused), 1000-part studio views 1.3 s, Update 1.5 s, export PDF 0.43 /
     DWG 0.16 / DXF 0.22 / PNG 1.05 s (7.2-18.3 s).
   - imports: NIST STEP 4.15 / x_t 2.16 s (38.7 / 24.5 s); live big asm STEP 2.8 / x_t 1.7 / IGES 7.1 s (33.5 / 12.9 /
     69.3 s); exchange suite 173/174 at or under live's.
   - Open: c2-rings-loft live.x_t "import" 17 s (live 8.6). Read 4.9 -> 2.6 s (xt_read.cpp tube() at 10 nm, Parasolid's
     precision: 102 stations, was 204; branch dx-tube bf5094af8; A/B over the 21 live.x_t (vibex8 ~/agents/dx-keep/xtab): 20 identical, rings-loft
     same counts, Extruded ring volume +0.00056 mm³ (1.3e-6, live's band ±3), import 39 -> 29 s: lead asked 12:4x). Left: query_parts ~13.5 s/pass on the
     two blend tubes (156 edges each); their pcurves (ShapeFix FixAddPCurve projection) carry ~84 knots per edge (STEP's
     15), so face_moments' Green integral runs 50.8k pieces (STEP 8.7k). Grouping C2 knots (common.cpp) broke gear
     volumes (cf-part-draft Spur Gear -0.36 %): reverted. Decision asked of the lead 11-18 11:0x: integrate the inner
     integral along the shorter direction (core's props) or find the dense pcurves' source in the reader.
1. Exchange m5 (vibex8 06:0x): imports 114/130, live imports ours 46/48, ours reads ours 148/154, DXF 1/10 (duct).
   DXF rows left follow model differences: faceblend full-round fillet edges B-spline in live (blend told 06:1x),
   extension-spring Fillet 1 10/24/16 vs 11/27/18 (blend), rings-loft walls, hex mesh outline splits (live's breaks
   follow its facets' normals noise: no rule), housing-draft lug chords, hole rim (lead: wait for more docs).
   Parked (lead 11-17 decision 2): rings-loft live.step revolved ring area (+0.093 mm², edges ~3 um off); extension-
   spring live.sat area +0.016 %.
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
6. config-parts after live's retake at defaultValue (16c51e2a / 8d3a0ec2 landed); 1dc32b / 74d3d6 studio reruns.

## Since 19:50 (this thread)
- DXF rows (lead 23:0x), exchange suite drawings 0/10 (vibex6 exch-dxf / exch-fix2 logs, /root/dx-scratch/bin/
  dxflayercount.py, dxfanntext.py, dxfents.py, dxfsheet.py (two DXFs side by side), hlrplot.py / hlrgrid.py).
  Landed 5076e98c (view-label MTEXT, tangent-only circles, mesh crash; archive) and 12a38f9e (03:23):
  - 46d2c427d mesh parts drawn as live, silhouettes only (live2 q-mesh-drawing: hex "imported" alone and the rings Iso
    show no folds between facets toward the viewer, not even 90 deg: the far half of a hole rim), edge-on facets
    count as away (MESH_FACING); a B-rep edge glimpsed past mesh facet corners (piece < deflection of a longer edge)
    dropped; no centermark on a wholly hidden circle; on_conic: ends meeting make a whole circle only past half its
    length. hex-bookcase 0 CIRCLE / 0 ELLIPSE / 211 LINE / 17 LWPOLYLINE for live's 0 / 2 / 206 / 25.
  - be583d88d edges two parts share in 3D (ModelEdge::shared, SheetGeometry::shared_lines) written for each part:
    duct-multipart matches (live 26 LINEs doubled); 2D-only coincidences still once (drw-bom-table pin). Centermarks:
    a part-way wall marks only if a hole's (faceblend's convex ends); arcs' union, not sum, for full circles.
  Open: live splits each mesh silhouette loop into ~4 open LWPOLYLINEs (ours one per loop; rule unknown); live's 2
  Iso ELLIPSEs in hex are B-rep hole rim quarters inside the mesh hole (ours hidden). 943e05e14: a mark only where
  the cylinders' edges go all the way round seen end-on (faceblend's pocket ends none, as live; the radial-hole e2e
  had cut a half groove, now drilled mid-height). faceblend left: ELLIPSE 25/13, SPLINE 8/23, LINE 108/103.
  Lead 11-18: the suite takes each drawing's stored scale and view positions (place_as_live in parity.rs); the
  suggested-views rule is judged only by live's Create Drawing captures (clamp Part 3 1:1 = ours; live's Iso centre
  7.786-8.061 in varies by part, ours fixed 7.810: rule unknown). clamp-guillem now 271 / 268 LINE only. sm-louver ARC/CIRCLE now match.
  Landed 05:55 (7db466c6) with 59d761674 no mark on a wholly hidden hole: hex CENTERMARKS gone, as live; c284709fa
  suite views placed by their silhouette box centre, live2 iso-place-1118); 943e05e14 landed 06:26 (8a74d626).
- Export feel (live2 export-feel-1118, behaviours "dx (lead 11-18 ~02:38, feel)"): in a1bc5ce3: solid exports
  (Part Studio, part, assembly) show the "<file name> is being prepared…" bubble (~5 s; assembly's message fades at
  4 s) and the bell's "Export completed" / "<name> was exported successfully." (no extension); drawing Options
  "Download and store file in a new tab". Landed bfac0a31: import_files keeps the active tab; the
  export UI test (export_feel_tests.rs; turned main red 04:19, parked T2 by the lead: it read doc.tabs, creation
  order, where Assembly 1 precedes the stored file; fix = ordered_tabs(), T1 again in 1b62e831).
- Import / Export dialogs (live import-export-1118): queued 15790a91: import reads its files off the UI
  thread, tabs come when done, bubble "<file> has been imported." / "<file> uploaded but failed to translate." with
  Open document (file kept in CAD Imports); Export dialog keys test (focus on the dialog, Esc, Enter in File name).
  Assembly export (live2 asm-export-1150): queued 15790a91: written off the UI thread (app-core export_store, the
  Part Studio's too), bubble 5 s (was 4); format list / PARASOLID default already live's.
- Inspection list (live inspection-list-1118): a part row's Export… Format "Inspection list" (last, after STL), note
  row, CSV with BOM / quoted / CRLF, rows = Model definitions characteristics; ops export_parts format "Inspection
  list" needs `parts`; feature params take `<param>__tolerance` {type, upper, lower, precision} (null clears) when
  tolerant; e2e a_parts_inspection_list_is_its_characteristics.
- config-parts at live's 18:0x retake: 4/6 to the end (143/156); nuts Sweep 1: core; pulleys Circular pattern 1:
  core's 06ecaff5a (refs are live's); standoffs Threads=true time: bool2 (profile vibex8
  ~/agents/dx-keep/stoff-prof.log).
- Tools: /root/dx-scratch/bin/xtdump.py (text x_t / xmm with live's embedded schema); vibex8 ~/agents/dx-land.

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
- Agent ids (message_agent needs them full; core + blend restarted 11-18): core e8af89fe-d460-49b3-b0bb-9572da230fed,
  asm d8d597e0-675c-497b-a804-544ceab99bbd, blend 0672fa20-964d-4fde-a19c-c5b4363729ba, bool
  7fa8ceab-4549-499f-9b01-3a289c2c95b4, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, live
  e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7, studios
  58e9bca8-346c-4847-8012-e15006ae61ef, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b,
  psui 4973e295-9e92-463f-a13f-2ec5258bc5f9.
- The archive's "Parity translator changes by asm" section lists asm's translator fixes one line each (lead 11-07).
