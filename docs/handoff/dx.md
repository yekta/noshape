# dx handoff: agent API + drawings + data exchange (current state)

State as of 2026-11-21 20:46 UTC. Rewritten, not appended (DECISIONS.md 11-15); history in docs/handoff/archive/dx.md.

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md (generated), drawings and their export (crates/drawing,
crates/model-data/src/drawing), import/export, configurations, Variable Studio, Derived, start window and document
visuals, the parity translator (crates/ops/src/parity.rs) for my studios. The drawing UI is studios' (11-12).
Studio split with asm (lead 11-15): infento 1dc32b / 74d3d6 and bjr-config 5d8bc1 are mine; isaac-ball (3c5c58,
1e81ca, 90f78b) and config-assembly 20e932 are asm's. asm gates parity.rs changes before they land.
Before any crates/drawing land: the full `cargo test -p noshape-ops --test all e2e::` (the queue may skip ops).

## Fast open (lead 11-21 06:50, P1: infento <= 0.5 s shaded)
Display rows `display:<tag>` in the .noshape meta table (app-core display_cache: key = blake3(document, tag inputs,
build id)); steps 1-3 landed / queued (STATE). Step 4, B-reps for selection, only when the lead says.

## STATE 21:0x 11-21 (read first)
- HOSTS (user 18:5x, corrected 19:2x, DECISIONS a16b39297): default builds on vibe2; the Mac only via its slot (RB_HOST=mac
  waits), only when vibe2 is busy. No dx target on the Mac (dx-open 12 GB, dx-urdf 22 GB deleted 18:5x).
- LANDED a19a2e97 as 62694171 (19:09): step 4 (AsmPickMap per occurrence, picks 208 -> 24 ms). infento vibex6 fresh cache:
  rows 16 ms, decoded 157, cached display 192, scene build 58 (was 298), model frame 1033, click -> drawn 249. Step 3's
  "342 ms" timed a frame WITHOUT the model.
- CHECKING b51ca6b9 (queued 20:18): STEP "convert all surfaces to b-surfaces" (duct 38 B-surfaces; read back 41/47, was 40).
- QUEUED bbd56cd5 (21:03): P3 SETLIB-IMAGES (ops images_library.rs, op documents_images_library, sketch_insert_image
  `image`, select_item.rs Library ▾ "‹name› images"); carries b51ca6b9.
- COMMITTED d934ff40e (not queued): P3 DRW-VIEW-CENTRE (views.rs `Reach`: bodies' tight box + every >= half cylinder
  axis 0.09 in past its faces; ViewGeometry.reach / placed_origin(scale); one 3D point per source). Green: drawing lib
  265, drawing-ui 151, ops e2e 277, clippy drawing (Mac). Land after the A/B: /root/dx-scratch/vc/ab2.sh -> ab.out
  (vibex8 ~/agents/dx-keep/bin/ops-vcA / ops-vcB, dx_perf::timed_big_drawings x2; delete both after). Then
  vibex8 ~/agents/dx-keep/vcex.sh (duct + bottle exchange rows with ops-vcB) -> /root/dx-scratch/vc/vcex.out: expect
  bottle Front / Right at live's y, STL rows 9 parts.
- NEXT after: exchange leftovers (STL surface edges; bottle Front views 0.015 in low in DXF rows: recheck after
  DRW-VIEW-CENTRE).
- REPORT at queue empty (<=5 lines): step 4 numbers + step-3 correction; Mac targets freed; duct 41/47; STL split
  c497ac8c; SETLIB-IMAGES; DRW-VIEW-CENTRE.
- Probe: /root/dx-scratch/open4/openpick2.sh (vibex6 ~/agents/dx-keep); full run (no skip1) after any rebuild: rows
  are keyed by build. Clicks repeated at one spot hit the triad: probe artifact.
- TICKETS EDGE-CENTRE-SEAM (core, sent 14:17): our edge centre (linear_properties) is off on a rim crossing its ellipse's
  seam. Probe vibex6 ~/agents/dx-keep/sr/rlmid.py ./cli ~/agents/dx-keep/sr/rl-saved.noshape ~/parity/models/c2-rings-loft/live.jsonl.
- PARKED 15:4x: the housing compat IGES rim (36 / 80 / 45 vs live 36 / 79 / 44). OCCT's r 12.5 mm cylinder needs a seam
  ending at the rim's top; live's Parasolid face has none. Only a counting rule would match; P3 note in formats.md. WIP
  in /root/dx-scratch/igesseam-wip2.patch (changed no count).
- Drawing rows 2/10 (duct, louver). Mac suite dirs ~/noshape-host/dx-keep/exs-drw (10 docs, `bash exx.sh exs-drw
  exs-drw.log`), exs-rings. View centres: /root/dx-scratch/vc/pads.py <doc> (live drawing-live.json boundingBoxPoints).
## Exchange leftovers (11-21)
- Drawing rows (2/10): bottle Front's lip arc 41.8..180 deg (live 90..180): Part 1 is one revolved solid (14 closed
  edges), the arc is the lip torus's outline; its inner 41.8..90 part should be hidden by the torus's near side, ours
  draws it on the left only (right 0..90 right): the torus seam in HLR (probe vibex6 ~/tmp/dx/lip.py). Views 0.015 in
  low = DRW-VIEW-CENTRE (axes 0.09 in below, 0.06 above at 1:5); hex iso circles as
  LWPOLYLINE (live ELLIPSE); faceblend ELLIPSE vs SPLINE; the rest hidden-line geometry. Files /root/dx-scratch/drw.
- Open: STL split (duct 2 parts vs live's 9 sheets, bottle 2 vs 148: rule unknown, REQUESTS dx 11-21 14:06;
  /root/dx-scratch/stlcomp.py), housing IGES compat 80 / 45 edges / vertices vs 79 / 44 (one closed B-spline rim in two
  IGES curves: join two G1 B-spline pieces bounding the same two faces in boolean.cpp's merge, kernel). Rhino as live
  (46 / 118 / 76). CLI import probe: vibex6 ~/agents/dx-keep/sr/impprobe.py ./cli <file> [pp].
- vibex6 dx-keep trimmed 11-21 (12 -> 3.5 GB): bin/exch.sh's default ops-ex is gone (pass OPSBIN or use the Mac's exx.sh).

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
