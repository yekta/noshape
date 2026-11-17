# dx handoff: agent API + drawings + data exchange (current state)

State as of 2026-11-17 00:15 UTC. Rewritten, not appended (DECISIONS.md 11-15); history in docs/handoff/archive/dx.md.

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
Last full run (exch-1116, 08:50 binary): ours imports live's 58/74, live imports ours 46/48, ours reads ours 108/117,
times at or under live's 48/48.
Landed 11-16: 503d5a2a6 suite; 4d5ba3177 OBJ zip import; ad4a3517e mesh part -> planar facets (x_t / ACIS / IGES /
JT / Rhino / PVZ crashed); 81ae5cce0 x_t seam-line + SweptToElementary fixes; b75687eb8 suite file / key fallbacks;
2ddc1b649 DXF layouts + 2 viewports; f184f26ae tangent edges by geometry; 7c4ea1412 short arcs as conics; 4058e93e
centermarks not on bends + STEP / IGES offsets as B-surfaces (rings-loft 440.81 mm³); 99d57322 x_t ring edge only
where every face has the arcs as a loop (cf-snap-hook one solid); 03467fc8 ACIS import keeps the file's edge range
where projection misses (cf-sheet-metal-model-plus' SAT: Part 1 was inside out -> import failed; now 10148.89 mm³,
live 10147.18); b777cbd1 the screws Split hang fix (bool's gate clean).
b3f549eb (landed): import_files reports a failed Import feature in `errors` (was 0 parts, no error).
Landed 11-16 pm: 5acd8e11 curve pattern along a 3D path in the corrected Frenet frame (driven-pattern Curve pattern 1:
instances 50 -> 2-4 µm off live, still 7/11: ours are exact screw copies of the seed, live's not quite: helix geometry?);
de3fb725 live's rings-loft x_t imports (periodic tube both ways, blend SP-curves not p-curves; Extruded ring 273 faces,
440.79 mm³ / 1193.08 mm² as live; 766/510 edges/vertices for 762/508 = the two ring faces' seams; no T1 test: the only
reproducer is live's 1 MB file, exchange suite covers it); e3109d3c drawings: B-spline edges on a circle / ellipse to
1 µm drawn as conics (rings-loft ELLIPSE 15 -> 96, live 95). 46ee2df2: an exact view whose shape loads no edges
(OCCT Update swallows the failure, at random in one of a sheet's parallel views) retries, then goes polygonal.
654ffaf5: x_t export writes offset surfaces as OFFSET_SURF over their basis's B-surface and puts each fin's SP-curve in
the period of the B-surface's range as written (rings-loft Part 1 read back 302.72 -> 440.65 mm³, 440.77; T1
export::offsets_of_a_periodic_bsurface_round_trip_through_parasolid).
Landed 11-16 late: 212337b1 x_t import: a blend's tube stations from its spine's curvature, halved until round to
1 nm (were r/16: 618-1235 stations; live's rings-loft x_t imported 7.1 -> 3.1 s on the Mac, same parts; query_parts' 4.7 s
is OCCT volume integration, generic). 4a77e41a x_t export joins a periodic face's pieces OCCT cut only at its seam
(face_joins) into one face and fuses a rim's arcs meeting at a vertex only they use (housing-draft x_t matches).
5d0813e0 ACIS / JT export: offsets as B-surfaces again (my 4dd88057 had broken rings-loft's SAT; T1
offset_walls_export_to_acis, area 0.17 % off: the B-surface approximation, open). Queued: x_t export: a B-surface
closed but for its end rows' weights' last digits (6e-12) is one periodic face (rings-loft Part 1 x_t matches; T1
a_loft_wall_closed_but_for_its_weights_last_digits_stays_one_face, fixture rings-loft-wall.brep).
Suite x1117a (vibe6 00:04, with all of these): ours imports live's 59/75, live imports ours 46/48, ours reads ours
112/119, times 49/50, drawings' DXF 0/4.
Open, ranked:
1. rings-loft own x_t Part 2 133/338/208 for 132/335/206 (a 0.007 mm² cylinder piece joined to its neighbour stays
   apart: seam_pieces_joined's unify refused?). rings-loft own SAT 275/776/517 + 133/339/209 (SAT writer splits the
   loft walls itself).
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
1. Curve pattern 1 path (driven-pattern, above): translator, mine.
2. Rank x1116n's gaps and fix the largest; set 2 after live2's refetch.
3. Drawing conics in the iso view (Open 1).
4. config-parts after live's retake at defaultValue (16c51e2a / 8d3a0ec2 landed); 1dc32b / 74d3d6 studio reruns.

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
- Agent ids (message_agent needs them full): core 6692ae35-758a-4442-81c8-29f67be2e21a,
  asm 75d6886b-30b5-45c0-b789-a6730354be97, blend 0e6e9942-0713-46c1-ae55-132fbbb4a14b, bool
  c25aee1d-f0a9-4fab-ac99-acca645c81c7, live b89af730-e8c6-4234-b83e-00a9e1ba6acc, live2
  c42ce02b-3429-4294-861b-ccbc6cbb41d2, studios 08f83230-1122-4c3c-a185-edf9f225d834, skasm
  a09d30a7-95db-42d1-8f2d-946c6fb8f232.
- The archive's "Parity translator changes by asm" section lists asm's translator fixes one line each (lead 11-07).
