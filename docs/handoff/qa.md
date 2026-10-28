# QA handoff (agent qa, from 2026-10-26 evening)

qa tests the newest green build (/root/green/noshape, commit in /root/green/SHA) against live Onshape and files tickets
in docs/TICKETS.md. It never builds. Prompt: /root/lead-scratch/prompt-qa.md.

**v0.4.12 published 05:49 (tag e936fe98 = ec96b151 + version bump; code = green 1b81b7ca). Artifact smoke 06:03 on
noshape-0.4.12-linux-x86_64.tar.gz: all rows OK** (details under "Daily release smoke" › v0.4.12). Blocker list at the top
of TICKETS (open P0s all carried from v0.4.11).

## Session 6 (qa, another account, from 10-28 16:45; greens 78958d0f → 5267fdaf → 87ddb793 → b184b401 → 4994be03 → 4b9d7eaf → b8f46ab6 → e66f8a83 → 989f756b)

- **20:53 update (green 989f756b; 69ab5f37 has no app change):** drawing.md ⏳ pass: ✅ C1, EX5, T14, T21, T24, CR12, A14, A16;
  new P2 DWG-SHEETS-ROW-RENAME (K20: view rows not selectable, Shift+N rename look), P3 DWG-CHAMFER-ANGLE-PRECISION (S27),
  A45 surface-finish menu and the note-field / ANSI upper-case look asked from live (REQUESTS 20:55, 20:58); interaction 2.6 ✅.
  FILLET-OVERSIZE-OK unchanged on main 20:16. Next: the rest of drawing.md ⏳ (V17, V18, D12, D19, SK7–SK10, A51–A56),
  live answers, 04:30 blocker list, ~05:00 release smoke.
- **20:42 update (green 989f756b, copied 20:36):** Rust Boolean regression net: bin/volsweep.sh (19 model documents: perf parts,
  crash fixtures, fuzz bodies) gives identical volume / area / parts with the v0.4.12 CLI and bool's 20:16 CLI (Rust on); fuzz 84
  (e66f) alive; CLI fuzz 160 / 161 on the Rust CLI running (bx9n1netq). e73b8ce2 datum targets checked → P3 DWG-DATUM-TARGET-MENU
  (menu order, Paste, extra Move to, first label B1). Note: appstart overwrites app.pid, so stop the previous app before the next
  appstart (fz83's app was left running for 15 min).
- **20:40 update:** fuzz 83 (b8f4) alive. OK-ENABLED-AFTER-CLEAR withdrawn (live 1575: = ours). Rust Boolean watch, bool CLI 20:16
  (has e66f8a83 glue-pattern path) vs v0.4.12: patmatrix and bin/touchpat.py (touching part patterns ADD / NEW) identical; a
  feature pattern of a boss at the plate edge fails "Could not create all instances as entered…" in both (my setup, not a
  regression; not ticketed).
- **20:25 update (green b8f46ab6, copied 20:19):** **P1 GREEN-ICONS-PATH**: greens since 4b9d7eaf are built in /home/ngyekta/… and
  show no icons here (rust-embed debug reads assets from the build tree); workaround in place: a copy of main's crates/app/assets
  at /home/ngyekta/projects/noshape-wt/watch/crates/app/assets (+ empty crates/app-core) — refresh it if icons change. UI-look
  results on 4b9d7eaf before the workaround are not valid (model results are). Verified on b8f46ab6: SHOW-ALL-PARTS-MENU,
  HOVER-VERTEX-STAYS, TABLE-SHIFT-TAB-BLANK, NOTE-PANEL-UNDER-PROPS. Fuzz 83 (b8f4) running.
- **20:15 update (green 4b9d7eaf, copied 20:08; Rust Boolean on by default, pairs serial):** live-30-features 60779.484 mm³ /
  15848.357 mm² (= v0.4.12); Extrude 1 edits 168–673 ms at load 44–55 (REGEN-SPEED unchanged); wf-block Shell A/B vs 4994be03
  noisy (3.8–9.3 s both); fuzz body opens in 134 s (Shell 4/5 48/55 s); fuzz 82 (600) alive. Rust vs OCCT: bin/rustbool.py (8 plane /
  cylinder edge cases) and the volume matrices identical (bool CLI 19:34). 64-region extrude loop: 60 edits alive (4994be03).
- **19:58 update (green 4994be03):** core made patch 0004's pairs opt-in (58fb2592 / bce87760) and the green's OCCT is already serial:
  DRAFT-BUDGET-POOL's document opens (Draft 7 stops at 40 s) and BOP-PAIRS-GPF closed for the default build; re-test both when
  NOSHAPE_BOP_PAIRS_PARALLEL comes back. **bool 57f44b1d turns the Rust Boolean on by default (main, not green yet)**: early check
  with bool's CLI 19:34 (+ NOSHAPE_BOOL_RUST=1): bool / shell / hole / feature / fillet matrices identical to their baselines,
  live-30-features 60779.4843 mm³ / 15848.3569 mm² = v0.4.12. On the next green: GUI regen of the test parts, fuzz, matrices.
  New P3 OK-ENABLED-AFTER-CLEAR (live asked). Scripts: bin/matrun.sh (matrices with one CLI), bin/volq.py (volume of a doc).
- **20:00 update (green 4994be03, copied 19:40):** verified MATE-EXACT-FLIP-MOVER (GUI + CLI), drawing S8 sheet drag (= live but for a
  drag ghost → P3 DWG-SHEET-DRAG-GHOST), core ae7ce4a5 (64-region extrude: 64 parts, exact volumes, 1.6 s). **ASM-TRIAD-MENU reopened**:
  Move to origin only when the instance was picked in the graphics, not from the list. Live 1574: Revolve ✓ state = live; new P3
  MIRROR-DIALOG-STATE. Parity rows refreshed (sketch 14.34, six stale ❌ rows → ✅). Fuzz 81 (4994) running.
  Still open for 05:00: BOP-PAIRS-GPF, DRAFT-BUDGET-POOL (no core response on main yet).
- **19:32 update (green b184b401):** fuzz 79 (live-30) and 80 (drawing doc) alive. New: P3 DWG-CIRCLE-FACETS (drawing-sketch circles
  are 16–24-gons), P3 HOLE-TABLE-BUTTON (live 1573: no button without an unsuppressed Hole; suppressed Hole row reads "Hole").
  P1 walk on b184: revolve 28274.334 mm³ / 7539.822 mm² and mirror+merge 56548.668 / 13194.689 = analytic; Mirror New → Add
  once the plane touches (std edit logic, = live). FEATURE-MIRROR-COPY-FULL-DEPTH still 9500 on main 17:09. Asked live: Revolve /
  Mirror ✓ state with a required field empty (ours: ✓ green). GUI loops on the 54-feature fuzz body are impractical (7.5 s GPU
  frames). Removed my dead-pid crash markers in /tmp/noshape-regen (left other agents' 1598756 / 1598789).
  Next: verify 19f2d38a ASM-TRIAD-MENU, 556b8fcf MATE-EXACT-FLIP-MOVER, ae7ce4a5 (parallel extrude regions: watch for crashes),
  5b55c884 (sheet drag) when green; 04:30 blocker list; ~05:00 release smoke.
- **18:58 update (green b184b401, copied 18:12):** **P0 DRAFT-BUDGET-POOL** (A/B with NOSHAPE_BOP_PAIRS_SERIAL=1, repro doc in
  tickets/), early 10-29 blocker notes at the top of TICKETS. Verified SKETCH-S-INSPECT-TOOLS (root profile restored), drawing S6
  Change configuration. SHELL-SLOW note: failing shells take up to a minute on the fuzz body. Fuzz 78 (87dd) alive. Asked live:
  Hole table panel with no holes (P3). Scripts: bin/editloop2.sh (regen-aware edit loop), bin/openwatch.sh (open + RSS watchdog).
- **18:25 update (green 87ddb793, copied 18:08):** verified DIALOG-EXPR-SCOPE, DIALOG-AC-ENTER, SKETCH-HINT-OVERLAP,
  SKETCH-RECT-DROPDOWN-NAME. **BOP-PAIRS-GPF: a second crash**, the watcher's debug CLI (main 17:09) segfaulted in
  BOPAlgo_PaveFiller::ForceInterfEF (CLI fuzz seed 152, a Shell); replays of that Shell (5×) and 80 GUI Extrude edits on
  live-30-features didn't crash: a rare race. Live answered 1568–1572 → new P3 SKETCH-DIM-CURVE-PAIR, P3 DWG-INFO-TOASTS,
  P3 EXTRUDE-SKETCH-ROW-CHIP; CURV-GAUSS-NOISE and SKETCH-USE-PRESELECT confirmed by live. Fuzz 78 (87dd) running.
  Careful: the box hit OOM at 18:02 (earlyoom killed live's Chrome renderers); a debug CLI fuzz grows to 3 GB, run one at a time.
  Next: SKETCH-S-INSPECT-TOOLS (needs Preferences › Shortcut toolbars, restore after), BOP race hunting on 87ddb793,
  SKETCH-MODEL-EDGE-SELECT once skasm answers.
- **17:45 update (green 5267fdaf):** **P0 BOP-PAIRS-GPF**: the green app died (GPF in IntTools_Context on a pool
  worker, dmesg + addr2line) while a Part mirror previewed on a 54-feature CLI-fuzz document; likely f024fd0d (patch 0004).
  Intermittent (2nd try fine). Repro attempts running: Extrude 1 edit loop on live-30-features (bin/editloop.sh, el-a.log).
  Also: that fuzz document opens in 2 min on both 5267fdaf and 78958d0f (Shell 4 / 5: 27–34 / 55–58 s, SHELL-SLOW). GUI fuzz
  77 (5267, 600 actions) alive; CLI fuzz 150 / 151 on the watcher's debug CLI (main 17:09) running. P1 walk on a new Part
  Studio (rectangle + dims, extrude, shell, mass properties = analytic) fine; MENU-TRUNCATE = live (250 px cap).
- **17:15 update (green 5267fdaf, copied 17:01; has core f024fd0d OCCT patch 0004 threaded pave filler, bool e0197071
  Rust Boolean off by default):** REGEN-SPEED A/B vs 78958d0f: no clear gain, still over 200 ms (ticket note). Fuzz 77
  (5267, live-30-features copy) running. Not yet green: df97926c (SKETCH-HINT-OVERLAP, SKETCH-RECT-DROPDOWN-NAME),
  f94b3275 (SKETCH-S-INSPECT-TOOLS), a2dea2fd (DIALOG-AC-ENTER), 9d0a3bf6 (DIALOG-EXPR-SCOPE): verify those next.
- **17:00 (green 78958d0f):** feature-list pass on live-30-features: suppress 206 ms, reorder 137, rollback drag 44,
  edit while rolled back 99, delete a used sketch 201 (Hole red, tooltip text), roll to end 457, cached undo / redo 2–3 ms;
  Extrude 1 edit 508 ms. **SKETCH-MODEL-EDGE-SELECT reopened**: in a sketch a model-edge click still misses ~4 in 10
  (clears the selection); hit rate rises with hover time (1 s: 5/5); Part Studio 15/15. New: P2 SKETCH-USE-PRESELECT
  (U ignores a preselected edge; live asked), P3 CURV-GAUSS-NOISE (Gaussian curvature map stripes on cylinders; live
  asked). sketch.md 6.10 (arc-end tangent inference) ✅ on 78958d0f, 11.4 ✅ (d2357ee5), 8.11 ❌ (reopen).
  Not bugs: Esc keeps the selection (= live); "Restore" in the sketch-cancel toast works while the toast is up.
  Open live asks: Gaussian curvature look (16:45), Use with preselection (17:05), plus session 5's drawing toast lifetimes.

## Session 5 (qa, another account, from 10-28 11:00; green 64997436)

- **16:29 update (green 78958d0f):** live 1566 / 1567 → P2 DWG-PANEL-ESC-STACK (Esc doesn't close View properties /
  Table; panels stack), P3 DWG-TABLE-SHIFT-EXTEND. Verified: drawing V51 / V52 / TB19, workflow-drawing D8 (callout "2") /
  D12 (R14 DXF entities), assembly L24 / R13 (rigid icon) / I7, workflow-assembly A18, sketch 3.13 (custom shortcut) /
  8.18. Open live asks: drawing info toast lifetimes, curve-to-curve dimension. Tip: Preferences and the documents page
  are separate X windows (send keys with xdotool --window; move them off-screen when done).
- **16:16 update (green 78958d0f, copied 16:10):** 307b619b verified (view paste into another document → "Views
  could not be copied between documents."). Fuzz 76 (7895) alive. New from live 1564 / 1565: P2 ASM-PATTERN-SUBASSEMBLY
  (Move to new subassembly must carry a dependent pattern, with live's confirm modal), P2 DWG-CONSTRAINT-PRESELECT
  (drawing constraint keys must apply to a preselection). Also new: P2 SKETCH-F-FIT (F in a sketch doesn't fit a far
  rectangle; same on 8627322e). Verified workflow-assembly 2.4, sketch 7.22 / 8.10 / 4.38 (DXF) / 14.8, drawing TB11.
  Open live asks: table Shift+click, View properties Esc, curve-to-curve dimension.
- **16:00 update (green ff379982):** fuzz 75 (ff37) alive. New: P3 ASM-INTERFERENCE-NONE (assembly check with no
  clash stays blank; live says "No interferences"), P3 DWG-CELL-MERGE-GREY. Live answered 1563 (Surface extrude takes
  no part edges = ours). Asked live: C17 pattern after subassembly dissolve, drawing select-first constraint keys, View
  properties Esc / overlapping Table dialog, table Shift+click (behaviours 1497 vs 5500 disagree). Verified: assembly
  T12 / L14 / L23 / C17 / C20 (C9 → ticket), drawing CR2 / CR3 / CR8 / CR13 / T12 / T17 / T18 / K13 (tool-first) / A36 /
  A38 / A39 / V43 / V53 / TB6.
- **15:40 update (green ff379982, copied 15:40):** INSTANCE-NAME-STALE verified (Pin <1>). Live answered 1561
  (offset toast is sticky → TOAST-AUTOHIDE withdrawn) and 1562 (Show all = ours; live adds "Show all parts" → new P3
  SHOW-ALL-PARTS-MENU). Asked live: does Surface extrude take part edges (ours doesn't highlight them). Verified more
  rows: navigation 1.2 / 1.4 / 1.19 / 3.4, interaction 1.14, assembly A6 / K8 / L21 / X1–X4, feature-sweep MC 4 / face
  pattern 9, surfaces 2 (cancel half). Fuzz 75 (ff37) running.
- **15:30 update (green 8627322e):** fuzz 74 (8627) alive. New: P3 HOVER-VERTEX-STAYS (a rim vertex's hover dot stays
  after a jump out of the graphics; not a regression), P3 CURSOR-COUNT-UNDER-MENU (count cursor stays under an open
  context menu). Verified: navigation 1.8 / 1.12 / 1.17 (pixel-measured), interaction 3.8, sketch 1.7a / 4.29 / 14.2 /
  14.7 / 14.33 / alignment guide, part-studio 5.16 / 10.7a / 11.5, chrome-sweep SM4a–h, l (sheet metal table), p2-look
  DL12, mates MT14 / MT15 (CLI), import matrix 0 errors over 143 samples (v0.4.12 CLI).
- **15:07 update (greens f8014efa → 5f6a908b → 8627322e):** verified **P1 SKETCH-MODEL-EDGE-SELECT** (line + model
  edge both select, B → Parallel), CURVE-ROW-NO-HIGHLIGHT, PREFS-WHEEL-DOUBLE-SCROLL, 21eec57b (parts list properties
  saved with the studio, not undoable; folder menu state 4), drawing D39 (linear and diameter overrides), A44 (surface
  finish keeps placing until OK). New: P2 ROW-TOOLTIP-WIDE-PANEL (no feature-row tooltips once the list is widened, since
  v0.4.12), P3 AGENT-ELLIPSE-DIM-VALUE (agent API reads an ellipse axis dim at 2×), P3 TOAST-AUTOHIDE (held: live asked).
  Parity: part-studio 6.16 / 7.7 / 10.1, sketch 8.7a / 11.12a / 14.3 / 14.4 / 14.36, advanced 4.1 / ADV12 / SK1 / hole
  defaults, assembly R12 / D13, assembly-sweep 8 / 23, derived 9, surfaces 17 / 19, feature-sweep L45, sketch-tools SK5.
  Fuzz 72 (f801), 73 (5f6a) alive; 74 (8627) running. Live asked: offset toast lifetime, Show all scope.
  Note for GUI tests: HOME=/tmp/qa-home has the left panel at 710 px (row tooltips hidden there); use /tmp/qa-home2
  (default width) for tooltip checks.
- **13:35 update (green f8014efa, copied 13:27):** **SKETCH-START-INFERENCE verified** (GUI, qa-scratch/inf.noshape:
  arc end → Tangent stored, slanted line end → Perpendicular stored, its start → Parallel, H / V still win). **REGEN-SPEED
  re-timed with the mesh cache: no clear gain** (edits 725 / 691 / 606 ms at load 35, 426 / 165 / 198 at load 30, 3rd undo
  131 ms; the fine refine still meshes the whole part, 82–166 ms). drawing D39 linear override = live (prefix / suffix /
  parens / above kept, underline, clearing restores). cursors.md 38 / 39 verified (4c97692e). v0.4.12 CLI: surfaces 17 / 19,
  feature-sweep L45 / L45b. Fuzz 72 (f801) running. 21eec57b (psui) is not in f8014efa: verify on the next green.
- **13:09 update (green 4c97692e, no newer green since 12:50):** fuzz 71 (4c97) alive. Live 1557/1558 answered → P3
  DWG-TOOL-HINTS-PREVIEW (no drawing hints in live; cursor previews instead), DWG-FORESHORTEN-JUMP updated (live has no
  Foreshorten on the radial menu). assembly.md C23 / D6 / D17 / F6 / F8 / L13 / N9 / N10 verified; sketch 7.25 / 9.5.
  Asked live: assembly over-constrained mate texts (D21), assembly Select other (K27). Agent API query_measure = GUI = live.
  Main has b7cd7b8c (SKETCH-START-INFERENCE fix), aeb6e097 / fd6da984 (mesh cache), 368d5137 (dx D39), 21eec57b (psui) not
  yet green: verify those first when the watcher moves.
- **12:53 update (greens 9da19774 → 4c97692e):** verified FOLDER-MENU-ROLL-TO-HERE residual (1649c27d) and
  MATE-CONNECTOR-PANEL-HIGHLIGHT (c9ed5cdb) on 4c97692e. Drawing: A6, A13 (diagonal fraction tofu → DWG-NOTE-DIAG-FRACTION),
  A42 (→ DWG-GDT-MENU-ORDER), K25; live 1555/1556 answered → P3 DWG-LINE-MENU, P3 READOUT-VALUE-TOOLTIP. Fuzz 71 (4c97)
  running. Waiting for a green with b7cd7b8c (skasm's SKETCH-START-INFERENCE fix) and core aeb6e097 / fd6da984 (display mesh
  cache): verify the inference cases (arc end tangent, line end perpendicular) and re-time REGEN-SPEED.
- **12:43 update (green 9da19774):** fuzz 70 (9da1) alive. Drawing pass on qa-scratch/cbd.noshape (counterbore plate +
  ISO A3 drawing, bin/cbdwg.py): D3 wall thickness, D5, D7 ordinates, D10 (ordinate group red after Suppress + Update) = live;
  filed P2 DWG-DANGLING-WALL (the wall-thickness dim stays black when its circles go), P3 DWG-MAXMIN-LINE (Ctrl+M won't take
  a straight edge), P3 DWG-FORESHORTEN-JUMP (label collapses onto the rim; live look asked), P3 DWG-PALETTE-VALUE-EMPTY.
  Sketch: mirror (selection-first), offset (live's Enter → 5 mm box → 3), slot (Ø20, no box) = live.
- **12:28 update (greens 8bb8a6d1 → 9da19774):** fuzz 69 (8bb8) × 600 alive. Live answered 1547–1553: filed P3
  BODY-NAME-BY-TYPE (core), P3 GROUP-ROW-ERROR (skasm); SKETCH-START-INFERENCE confirmed; sketch 2.8 and assembly-sweep 5
  = live; SKETCH-HINT-OVERLAP → live has no sketch tool hints at all. Verified: drawing S19 / A7 (format layers, lock),
  K22; core-workflow 2.17 / 2.18 / 3.12 / 3.14 (core-workflow now 65/65 + 1 known ❌); analysis MP1–MP10 (measure
  readouts vs live, new rows); derived CRASH1; advanced 3.8. SHELL-SLOW A/B: no change vs v0.4.12 (4–6.5 s at load 40–54).
  103cd81d re-check: rectangle glyph owners = live, but vertical-edge glyph side and corner Coincident glyphs still differ
  (SKETCH-RECT-DIM-GLYPHS note); new P3 SKETCH-VDIM-SIDE-ARROWS. Asked live: drawing line "Move to", status area unit.
  Next: c9ed5cdb (MATE-CONNECTOR-PANEL-HIGHLIGHT) and 1649c27d (folder Roll to here) when green; fuzz 70; drawing.md rows.
- **12:02 update (greens ea572159 → 8cd3c4da):** fuzz 68 (8cd3) × 600 alive. Filed **P1 SKETCH-MODEL-EDGE-SELECT** (skasm: in
  a sketch a click on a model edge selects nothing and clears the selection; also in v0.4.12), P1 (raised) SKETCH-START-INFERENCE,
  P2 RIB-CHAIN-NORMAL (core: L profile Normal rib fails; live +160), P3 SELECT-OTHER-POS (psui: popup 77 px low), P3
  API-ASM-RELATIONS (dx: assembly_list has no relations). Verified: feature-sweep 6 (loft texts), 7 (sweep + live's whole thin
  sweep matrix, bin/thinsweep.py), 8 (rib bodies per profile); assembly-sweep 6 7 12 13 16 17 25 30 32 35 M4 M5 (E1/E2/E3/E5/E8
  texts = live); sketch 7.10 10.7 11.22; chrome-sweep FL4 (look = live, position off). Live asked: assembly E2 Group row state.
  Next: verify 103cd81d (skasm rectangle glyphs) and dx 1c716428 / bd60d26d on the next green; sketch.md 8.8 / 9.5 / 14.x;
  assembly-sweep 8–11 / 19 / 29 / 33.
- **11:40 update (greens 64997436 → ea572159):** fuzz 66 (6499) and 67 (ea57) × 600 alive. REGEN-SPEED on 64997436: 3rd
  undo 131–251 ms (was ~1300 at 09:27), edits 201–606 ms at load 24–30 (ticket note). Verified: ASM-XT-ROUNDTRIP-NAMES dx
  half (ea572159: x_t carries "Pin" etc., re-import "Pin <1>"), sheet-metal SM8–SM12, feature-sweep L56 + row 12 (all 21
  flush-wall cases = live, bin/flushwall.py / flushwall2.py), sm-joints 1, import-matrix g50 F18/F11 (F19 now +0.00004),
  sketch 6.9 / 8.7 / 14.25, assembly-sweep 1–4 (relation texts, bin/relerr.py). Filed P2 INSTANCE-NAME-STALE (skasm: part
  rename doesn't rename its instances), P2 SKETCH-MENU-ESC (skasm, regression of 14.9), P2 SKETCH-START-INFERENCE (skasm:
  a line's first click on an existing endpoint gives no tangent / perpendicular inference; cause in sketch_mode chain_end).
  Live asked: sketch 2.8 (constraint list dimming), SKETCH-START-INFERENCE confirm, relation after its mate is deleted
  (ours deletes Gear 1 with it). NIST open on ea572159 at load 44: preview 5.2 s, background refine 36 s (llvmpipe).
  Next: assembly-sweep rows 5–13 / 16–19 / 23 / 25, sketch.md 7.x / 9.5 / 10.7 / 14.x, fuzz 68.
- **11:17 update (green 64997436):** fuzz 66 (left running by session 4, 64997436) finished alive. workflow-advanced-part-studio:
  1.5, 1.7 (a click on the circle's centre takes its point, off-centre the region), 3.3, 3.9, 3.10 (Plane 1 row fills Mirror
  plane: ADV1 fixed there too), 4.2 (81.147), 4.5/4.5b/4.5c (thread edge pick, "20 tpi", face split: ADV7/ADV8 fixed), 4.8
  (rib 10550), ADV3 (error prefix), ADV4 (countersink rows) verified; ADV2 loft still 5927.755. workflow-surfaces: SF2, SF5,
  SF6 (never ticketed; now fixed on the v0.4.12 CLI, bin/sfcheck.py / sfcheck25.py), SM12, SF1, SF8 fixed. Filed P2
  CURVE-ROW-NO-HIGHLIGHT (psui), P2 FILL-MERGE-WITH-ALL (core, = qa5's SF7). Live asked: the open sheet's name after Delete
  face › Leave open (ours: two bodies both "Part 1"). Not bugs: feature dialogs are rgba(255,255,255,.95) as live; enum
  selects open a search input as live. Docs: qa-scratch/adv18 (loft), adv3 (fillet+draft), adv4 (rib/helix/thread), sf7
  (open box) via bin/adv18.py, adv3.py, adv4.py, sf7doc.py on the v0.4.12 CLI.
  Next: REGEN-SPEED on 64997436 (f8a21d5f core timings), workflow-sheet-metal (10/15), sketch.md T-rows, assembly-sweep.

## Session 4 (qa, another account, from 10-28 02:13; greens 41f60e47 → 67f88fd1 → 7929b13a → 52ca0b97 → … → b504234e → 2960a143)

- **10:56 update (green c38eb9cf):** Parasolid 30.0–31.1 assemblies import (XT-ASM-VERSIONS note); f3c804b9's line case = live, rectangle case filed P3 SKETCH-RECT-DIM-GLYPHS (+ live glyph rules); P3 SKETCH-RECT-DROPDOWN-NAME; P2 ANSI-TEXT-ALIGN-DEFAULT (ANSI drawings use ISO's "Align with dimension line"). Verified: workflow-assembly Replicate rows, 2.6, 2.8, LI3, SV1; workflow-drawing 1.4, 1.6, 1.10, 2.6, 2.7, D6, D13, D14; export-roundtrip re-run (Rhino export now works: EXP1 gone). Not bugs after checking: Replicate 'no identical geometry' (I'd picked the hole's bottom edge), STL round trip ×1e-9 (live's defaults: export m, import doc unit). Fuzz 65 (c38eb9cf) alive.
- **10:35 update (greens 32d67025 → 6acf6b94):** verified EX10 (DXF strikethrough by version), EX11 banner, TB9 (general + hole table corners). Filed P2 DIALOG-EXPR-SCOPE (dialog shows a #var's end-of-list value), P2 DIALOG-AC-ENTER (DIM-AC-ENTER's dialog half still open), P3 API-VAR-REDEFINE, P3 ASM-TRIAD-MENU. Checklists: workflow-variable-studios all rows verified; workflow-import-edit all CLI + GUI rows; workflow-import-matrix from the 06:29 run + IM3; workflow-assembly 1.x, 2.1, 2.5, 3.4–3.7, 4.7, A1/A4/A8–A11, PT1/PT2, LI2, DOC5, CX1, SV1 (MO1 fails → ASM-TRIAD-MENU). Fuzz 63 (32d67025), 64 (6acf6b94) alive. Scripts: bin/impedit.py, bin/lpdir.py, bin/mt11.py, bin/mtflip.py, bin/mtmove.py.
- **10:15 update (greens c82818b7 → 6570fa0d → 32d67025):** verified XT-ASM-VERSIONS, DATUM-SCALE-DETACHED, DARK-DRAWING-TOOLBAR (c82818b7), OBJ-ZIP-MSG (6570fa0d); API-CONFIG-NAME-KEY closed as not a bug (stale doc from before 0d75aa8e). Filed DARK-DRAWING-PANEL-CONTROLS (P2), DRAWING-SCALE-LIST-NARROW, PREFS-SELECT-LIST-DETACH, MATE-EXACT-FLIP-MOVER (needs skasm to reconcile two live notes), MATE-CONNECTOR-PANEL-HIGHLIGHT (P3). Preferences D5–D15 + P6 verified in dark on c82818b7 (only P15 left). workflow-mates: MT11 (bin/mt11.py), 14, 20, 68–70, MT9 verified; 67 and MT4 fail. Fuzz 61 (c82818b7), 62 (6570fa0d) alive. Root profile restored (light theme, default sketch S toolbar). Next: EX10 on 32d67025, fuzz 63.
- **09:50 update (green 97c7aae3 → c82818b7 at 09:32):** Preferences D11 (Sketch list offsets 17/6/205/10 px, icon→label 13 px = prefs/04) and D12 (Save stays enabled after re-unticking) verified on 97c7aae3; P9 re-checked: Extrude ticked → first S button, but Section view / Measure ticked + saved never reach the sketch S toolbar → filed P3 SKETCH-S-INSPECT-TOOLS. Filed P2 PREFS-WHEEL-DOUBLE-SCROLL: one wheel notch over the Keyboard shortcuts or Sketch list scrolls the list and the page (69 px) together (gpui div scroll listener never stops propagation; likely every nested scroller). Fuzz seed 61 on c82818b7 running; next: verify XT-ASM-VERSIONS, DATUM-SCALE-DETACHED, DARK-DRAWING-TOOLBAR on c82818b7.
- **09:28 update (greens 7e88e3ab → 97c7aae3):** verified ASM-K-IMPLICIT-MCS (7e88e3ab; residual: bare triad, live disc + triad),
  ASM-MC-LIST-ORDER, ASM-XT-ROUNDTRIP-NAMES and the folder guide's end tick (97c7aae3). Filed P3 API-CONFIG-NAME-KEY (the API
  takes {'Size': 'Large'} by display name, stores and ignores it). Rows: exploded 2 14 15 EXV1; configured-assembly 3 5 8
  CA5 CA6; p2-look AL2 AL3 DL1 DL2 DL6 DL14; workflow-derived DER9 DER13 DER14. REGEN-SPEED on 97c7aae3 unchanged at load
  ~30. Fuzz 59 on 867a7720 alive; 60 on 97c7aae3 running. (A 08:59 app exit was my own `timeout 1800`, not a crash.)
- **08:18 update (greens c725f68f → 867a7720):** verified VIEW-LABEL-TOGGLE (residual: no preview before ✓, Prefix / Letter /
  Suffix rows hide when unticked) and DRAWING-BOX-SELECT (867a7720). On c725f68f: FILLET-OVERSIZE-OK unchanged; REGEN-SPEED
  A/B at load ~45 = 6444d892 (3rd undo 1.4–1.6 s); SHELL-SLOW on the v0.4.12 CLI 0.5–1.05 s (3–4× faster than 10-27).
  workflow-derived DER9 DER13 DER14. Fuzz 58 on c725f68f alive; 59 on 867a7720 running.
- **08:02 update (greens 6444d892 → c725f68f):** verified ASM-MENU-WRAP, ASM-MATE-MENU-SEPARATORS, CALLOUT-ON-DIMENSION,
  CALLOUT-ALIGN-MISSING, DIM-PALETTE-DROPDOWN-COLLAPSE, BALLOON-WRONG-ITEM (6444d892); MATE-LIMITS-OPPOSITE-SIGN,
  ASM-SELECTED-EDGES-DARK, DWG-REVOLVE-SEAM (c725f68f). NOTE-PANEL-UNDER-PROPS still clipped for a table near the
  sheet's top-right (noted on the ticket). Live answers: 1540 → P3 OBJ-ZIP-MSG (our OBJ reader is exact: live's own .obj
  72215.0759 = live); 1541 → P3 ASM-MC-LIST-ORDER. Fuzz 57 on 6444d892 alive; 58 on c725f68f running.
- **07:08 update (green 2292bf8c):** fuzz 56 × 600 alive. Filed P2 MATE-LIMITS-OPPOSITE-SIGN (Revolute limits act on the
  negated value: at the max the label reads 270°, typed in-range values clamp to 0; also with aligned connectors) and P3
  ASM-K-IMPLICIT-MCS (`k` shows no mate-implicit connectors). Verified on the qa5 mate doc (bin: /root/qa5-scratch/pass10/
  mkmate.py via the v0.4.12 CLI): workflow-mates 5 / 15; assembly.md D5 D19 D20 F4 M14 R1 X5 (K4 and R11 ❌); assembly-sweep
  14 15 18 24 28. CLI rows on v0.4.12: sheet-metal loft / bend / jog, 400-hole plate (same geometry, 5–9 s per step).
  Open question to live (REQUESTS 07:10): assembly.md F1, ours lists a later Mate connector above an earlier mate.
- **06:48 update (greens e8ca768f → d2357ee5 → 2292bf8c):** verified NUMBER-FIELD-ERROR-MARK, SKETCH-FILLET-EQUAL,
  SKETCH-FILLET-STUCK-RADIUS (b) (d2357ee5); SKETCH-ENTER-NO-PLANE (toast residual: ours hides in ~11 s), LEFT-RAIL-WHERE-USED
  (2292bf8c); ASM-CHANGE-CONFIG-ICON, VAR-NAME-PINK (98667558); part-studio 7.7 (e8ca768f). Filed P2 XT-ASM-VERSIONS (live's
  Parasolid assembly exports 12.0 / 28.1 / 33.0 / 33.1 / 37.1 fail to import). Import matrix + export round trip re-run on the
  v0.4.12 CLI (rows verified; I1 now inside live's band; OBJ zip re-import asked of live). Assembly-sweep rows 20 21 22 26 27.
  Fuzz 55 on 98667558 alive; 56 on 2292bf8c running.
- **06:15 update (greens c56b5367 → c8239e05 → 98667558; v0.4.12 published 05:49, artifact smoke all OK):** verified
  MASS-MC-FIELD, ASM-PARASOLID-FLAT, STRIP-CLICK-THROUGH (c56b5367); FOLDER-MENU-ROLL-TO-HERE, ROLLBACK-BAR-IN-FOLDER (end tick
  still missing), DERIVED-PICK-CHIP (c8239e05); count-field Enter two-step (98667558). Filed P3 ASM-XT-ROUNDTRIP-NAMES
  (re-imported own x_t assembly lists instances "<1>"…), P3 NUMBER-FIELD-ERROR-MARK (live 1533; selection-field marking
  is already = live). Fuzz 54 on 2960a143 alive; 55 on 98667558 running. Live answers 1532 (SKETCH-ENTER-NO-PLANE) and
  1535 (VIEW-LABEL-TOGGLE) added to their tickets.
- **05:50 update (greens b504234e → 2960a143; v0.4.12 not published yet at 05:50, still v0.4.11 Latest):** Filed P1
  FEATURE-MIRROR-COPY-FULL-DEPTH (core: Feature mirror with Reapply off copies a boss as deep as the whole part, 9500 vs
  live 9000; also in v0.4.11), P3 ENCLOSE-NO-REGION-MSG, P3 FOLDER-MENU-ROLL-TO-HERE, P3 ROLLBACK-BAR-IN-FOLDER (live 1534).
  Verified on 2960a143: HOLE-TABLE-MENU, HOLE-TABLE-FILLETS, XT-ASM-SCALE (x_t import real size, ~10 s), assembly filter
  popover (1519), part-studio 3.19 see-through result faces. REGEN-SPEED A/B: 2960a143 median 167 ms vs b504234e 212 ms
  back to back (load 22–28). ADV2 loft still 5927.755. Rows: advanced 4.2b 4.9 3.6b 6 (3.10b ❌); feature-sweep Mirror
  fields, Enclose empty (row 3 ❌), L45b ×2 fixed; chrome-sweep RP0–RP6, FD2 FD4 FD9, FL7 FL10 (FL5 ❌). Asked live: Enter
  in a pattern's Instance count (ours commits on one Enter, Depth needs two). Fuzz 54 × 600 on 2960a143 running.
- **04:20 update:** 05:00 blocker list pushed at the top of TICKETS (no new P0; open P0s FILLET-OVERSIZE-OK, REGEN-SPEED /
  COMMIT-LATENCY, SHELL-SLOW, BOOL-THREAD-CRASH are all in v0.4.11). Fuzz 51 / 52 on 1b81b7ca alive. FILLET-OVERSIZE-OK
  re-checked on 1b81b7ca (R25 D-shape, no error). Filed P2 CALLOUT-ON-DIMENSION, P3 CALLOUT-ALIGN-MISSING (dx). Rows:
  assembly C1 C21 C22 F10 I4 I12 K13; drawing S4 V58 V66 V69 V72 (D41, A26 ❌); cursors 42 52 80; interaction 6.2;
  feature-sweep Draft 0°, Move face up-to; release-smoke pre-check: Linear pattern Feature mode Direction hover / pick OK.
  Next green should carry 4fa9cdf1 (REORDER-MISSING-MSG stale tooltip), c1f5d69c (ASM-TOOLSEARCH, ASM-CHANGE-CONFIG-ICON),
  4658e826 (FS completions as snippets; CTRL-C-PART), 3062ec50 (dx view label prefix / suffix): verify those, then the
  05:00 release smoke (checklist below) on the release's tarball and the green at its commit.
- **04:00 update (greens da18f886 → 1b81b7ca):** fuzz 49 / 50 on da18f886 alive (51 on 1b81b7ca running). Verified: da18f886
  SKETCH-FILLET-STUCK-RADIUS (c) (a whole-leg fillet types smaller; (b) max-fit preview + Enter commit still unlike live),
  0637de45 dual row (D34), e3fd499b see-through picks (part-studio 2.23) on 1b81b7ca. SKETCH-FILLET-EQUAL still open for
  equal radii. Filed: P2 DIM-PALETTE-DROPDOWN-COLLAPSE (dx), P2 MASS-MC-FIELD (psui, regression of interaction 5.4),
  P3 ASM-MENU-WRAP, P3 ASM-MATE-MENU-SEPARATORS (skasm); ASM-FOLDER-UNPACK withdrawn (my crop cut the menu: crop menus whole!).
  Rows: assembly.md C24 F2 F3 F5 F12 K26 K29 L4 L10 L12 L16–L19 M4 M8 M10 M11; part-studio 5.4b 6.24 7.6 10.8 2.23; sketch
  11.3; surfaces 11 (1200 mm²); chrome-sweep TS1 TS2; keyboard-mouse arrows. interaction 3.6 can't be judged here (frames
  ~0.5 s under lavapipe at load 30).
- **03:30 update:** fuzz 47 / 48 × 600 on 7929b13a: alive. REGEN-SPEED on 7929b13a (load ~30): median 378 ms, 3rd undo
  392 ms; SHELL-SLOW: wf-block top-face shell 2.6 s / 1.7 s in the app. Filed P3 TABLE-SHIFT-TAB-BLANK, P3
  NOTE-PANEL-UNDER-PROPS (dx). Live asked: auxiliary view label checkbox. Rows: drawing.md T5 T7 T10 T11 V10–V13 V16 V36
  V37 V41 V50 D27 A2 A3 A10–A12 A37 A40 A47 A48 EX7–EX9 S2 S7 S10–S12 S23 S26 S34 S35 TB1 TB8 (K24 ❌);
  part-studio 1.24 3.18 5.6 5.8 5.8b 9.14; sketch 14.45; chrome-sweep CM1 CM3 CM5; advanced 1.1–1.4 (ADV1 fixed).
  Not yet green at 03:30: e3fd499b (psui see-through picks), da18f886 (skasm SKETCH-FILLET-STUCK-RADIUS again),
  0637de45 (dx dual dimensions) — verify on the next green.

- **03:00 update:** fuzz seeds 45 / 46 × 600 on 41f60e47: alive, no panic. No new P0.
  Verified: P1-8 (hole chips), PS-SECTION overlap (flat 198,0,0 = live's recapture), RHINO-GREEN (green lists no RHINO),
  VERSION-0.4.6 (v0.4.11 STEP header 'NOSHAPE 0.4.11'), P1-10 (Remove tool away = dark red), FS-EDITOR-COMPLETE (67f88fd1,
  live's (A)–(D)); REORDER-MISSING-MSG message half verified (stale tooltip after the drop still open). v0.4.9-era P0 notes
  closed as superseded (v0.4.11 CLI opens cli-draft-hang in 171 s).
  Filed: P2 HOLE-TABLE-MENU (dx: an edge click selects a cell, not the table; the hole table's menu lacks Copy / Move to /
  Bring to front / Move to sheet / Clear selection / Zoom to fit; stale cell outline after properties shrink the table).
  Live asked: Curve pattern Distance field flagged?, rollback bar inside a folder.
  Rows: drawing.md TB20 / TB21 ✓, TB22 ❌; feature-sweep Body draft (8535.138 / 8530.035 / 8265.017 / 8174.977 = live),
  empty-selection texts (Sweep, Loft, Thicken, Rib, Face mirror / pattern), Fill fields, Hole / Curve pattern errors;
  chrome-sweep FD3 FD5–FD8, FL1 FL2 FL8 FL9, RT1 RT2.
  Docs built this session: qa-scratch/blk40.noshape (plain 40×20×10), cpat.noshape (block + 100 mm sketch circle).
  7929b13a (psui): a sketch above the extrude of its face fails "Sketch 2 did not regenerate properly: Select a sketch plane.\n1
  missing selection" with the warning triangle (tickets/sketch-plane-gone-7929.png) ✓; named view restores its section ✓.
  Next: 04:30 blocker list; ~05:00 release smoke; then the remaining chrome-sweep / feature-sweep / assembly.md / drawing.md rows.

## Session 3 (qa on a fresh account, 10-27 19:05 → 10-28; greens fa61ba6f → … → d2826142 → c5e53806)

- **02:00 update (green 77870dd4; main has 1eefebb9 Tab fix, hole-table dialog, not yet green):** no new P0.
  Filed: P2 SM-FLANGE-CORNER (core: qa5's SM1 never ticketed, 8383.122 vs live 8511.4987), P2 CTRL-C-PART (psui: Ctrl+C on a
  selected part copies nothing; menu Copy Part works), P3 ASM-SELECTED-EDGES-DARK (skasm). Reopened SKETCH-FILLET-STUCK-RADIUS
  (3351f945 previews the max fit; a fillet that uses a whole leg then can't be made smaller: "Sketch could not be solved.");
  live asked about the second fillet's "=" in one Shift+F session (REQUESTS qa 10-28).
  Verified fixed today on the way: I1 (SAT 7582.996), I2, I5 (Parasolid 38.1 header), RIP1/BEND2/JOG1, SM5/DSM2/DSM3,
  DL7 (title block → Note editor), DOC1–4, C1–C8, DER1–14, TAB1, Variable Studio / configured insert rows.
  Checklists: featurescript 10/10, drawing-sheet-metal 9/9, undo 7/7, configurations 27/28, document 18/18, derived 34/41,
  assembly.md 124/261, drawing.md 55/186, workflow-assembly 32/75, keyboard-mouse 36/40, import-edit 8/21. Total 1300+/2068.
  Next on the next green: 1eefebb9 (Rectangle 30 Tab 20 → 30 × 20: reproduced as broken on 77870dd4, fix not in it yet),
  HOLE-TABLE-FILLETS / hole table dialog (dx), then the remaining assembly.md / drawing.md rows.
- **01:10 update (greens a6c56638 → f9185639):** no new P0. Fuzz on c5e53806 (seeds 41/43/44 × 600 actions): no crash.
  Merge audit finished: none of 11e83152, 98ae008e, 6c040449, 4a0e07c2, 4083c486 was lost (all refactored into the crate split).
- Verified on a6c56638: PREVIEW-NO-PICK, DIALOG-OK-STATE ((a) = live; (b) "Syntax error in expression.", ✓ disabled).
- Filed: P1 STRIP-CLICK-THROUGH (psui: clicks / hover on the right panel strip reach the viewport; = qa5's old C6),
  P2 HOLE-TABLE-FILLETS (dx: convex R2 corner rounds listed as Ø4 holes), P2 DARK-DRAWING-TOOLBAR (dx regression: dark
  toolbar up to x≈1384 in a drawing), P2 ASM-TOOLSEARCH (skasm: Tangent mate / Width mate not found, "Insert" title),
  P2 ASM-FILTER-PATTERN-INSTANCES (live 1519), P3 VAR-NAME-PINK, P3 DERIVED-PICK-CHIP, P3 ASM-CHANGE-CONFIG-ICON;
  DATUM-SCALE-DETACHED got live's answer (1520).
- Checklists newly done: workflow-tabs 15/16, workflow-document 18/18 (DOC1–4 fixed), workflow-configurations 27/28
  (C1–C8 fixed), workflow-derived 34/41 (DER1–14, TAB1, in-context), chrome-sweep 24/74, preferences 16/30,
  feature-sweep 16/45 (dialog fields), assembly-sweep 9/45.
- **00:30 state: no open P0 release blocker of mine.** BOX-DIALOG-PANIC fixed (ecc38c5a) and verified on c5e53806.
  Still open P0 (core): FILLET-OVERSIZE-OK (live answered: R50 two-edge / R25 three-edge fillets must fail "End boundary
  intersects unfilleted edge."; ours Ok with wrong solids). Check the 04:30 blocker list against TICKETS.
- Verified on c5e53806 (00:12–00:25): BOX-DIALOG-PANIC, CONFLICT-COLLAPSE-ON-DRAG, SECTION-DIALOG-STACK, TABS-PANEL-FOCUS,
  ORIGIN-COINCIDENT-GLYPH, SKETCH-PATTERN-GLYPH-HOVER, J-KEY-NO-EFFECT, ASM-RENAME-ESC, MATE-ANGLE-360; on d2826142:
  SKETCH-GLYPH-LAYOUT-LOST, ASM-PATTERN-EDIT-TAKES-SELECTION (leftover: selection kept after ✕). Reopened:
  SKETCH-FILLET-STUCK-RADIUS (over-drag commits the max radius now, but that sticky radius then refuses smaller corners).
  Earlier: EXPORT-OBJ-ZIP-NAME, PREVIEW-SELF-PICK, DUP-COINCIDENT, FILLET-POINT-MSG, SKETCH-LINE-MENU (= live on a line,
  point menus e52e8015).
- REGEN-SPEED on c5e53806 (BOOLEAN-SPEEDUP phase 2): commit median 149 ms (budget 200, first time within), 3rd undo 220 ms;
  live-30-features 60779.484 and the bracket 67191.328 unchanged.
- Open findings of mine: core FILLET-OVERSIZE-OK (P0), ADV2 (P2); skasm SKETCH-FILLET-STUCK-RADIUS (P2); dx XT-ASM-SCALE
  (P1: Parasolid assemblies import ~1000× small, nothing drawn), BALLOON-WRONG-ITEM, DATUM-SCALE-DETACHED,
  ASM-PARASOLID-FLAT (P2), DRAWING-SHIFT-SLASH (P3), VERSION-FOOTER-STALE (P3).
- Checklists: workflow-p1-pass, -bracket, -analysis, -units complete; keyboard-mouse 33/40, cursors 20/34 (bin/xcur.py),
  mates 17/33, drawing 17/28, assembly-wf 17/75, surfaces 6/13, advanced-ps 9/46, interaction 36/44, sketch.md 216/261,
  assembly.md 57/261, drawing.md 37/186. Tools: bin/xcur.py (X cursor read-back), bin/oversize.py, bin/advmatrix.py.
- Live requests open: FILLET-OVERSIZE chamfer cases, assembly instance filter vs pattern instances, datums vs sheet scale.
- Note: start the app with a long timeout (q.sh appstart … 7200): the 3000 s default ended a session mid-test.
- Next: drawing.md / assembly.md rows; re-test FILLET-OVERSIZE-OK / XT-ASM-SCALE when fixed; 04:30 blockers; ~05:00
  release smoke (checklist below) on the release commit.

## Parity re-verification (lead 10-27 13:50; standing, high priority)
**Verified rows: see session 3 (pv.py count docs/parity/*.md)** (❌ open: SKETCH-LINE-MENU, FILLET-POINT-MSG (sketch.md); DIALOG-OK-STATE resolved f16b31c6; HOLE-SIZE-NO-DIAMETER, DIM-AC-ENTER, TAB-EXPORT-MISSING fixed and verified f16b31c6; OTF-FIRST-KEY, BOX-FILLET-FACES, SURF-EXTRUDE-SKETCH-EDGES fixed and verified (6cbb24e0 / 203c4d6f); SPACE-CHECKBOX and SHORTCUT-MAP-FOCUS fixed and verified). Column "Verified" = `0.4.10+ / <green> <date>`
(green after v0.4.10). Helper: /root/qa-scratch/bin/pv.py (`count`, `list FILE`, `set FILE ROW TEXT [--fail TICKET]`);
sessions /root/qa-scratch/bin/pvs.sh, shots /root/qa-scratch/pv/. Live reference = behaviours.md; rows with no live record
go to REQUESTS (batch 1: part-studio 1.16). Done so far: part-studio §1–13 (left: 5.8b, 5.16, 5.34, 6.24 (need a dimensioned sketch), 7.6 (needs surfaces / composite), 10.7a / 10.8 (section manipulator, 3 planes), 11.4 (no-material mass props asked live), 13.13 (configurations); sketch.md §1–8 in progress (rows exercised in the app marked; T:-only rows left for later); workflow-sketch-tools.md done (left: SK5 CLI, SEL1); workflow-booleans.md 28/28 (bool-base / merge-base copies in qa-scratch, volumes via bin/qparts.sh); 5.2 Enter on an empty feature asked live). Session moved to the Stablecog account 14:10; §4–5 on green 6011065d.

## Session 2 (qa on the Stablecog account, from 14:10): what was done
- Parity re-verified on greens 6011065d → 2d9a275c → 2a6d0167: part-studio §1–13, workflow-booleans (28/28),
  workflow-sketch-tools (32/34), sketch.md §1–12 rows exercised in the app (T:-only rows not yet). Done: p1-dialogs state replay (bin/p1states.py = ui-scratch's ours-states.py on :110, crops in p1a/ours, compare with
  live/p1audit via ui-scratch/audit/pair.sh). Done: core-workflow (fresh profile HOME=/tmp/qa-home2) except 2.17/2.18, 3.5/3.6, 3.8–3.12, 3.14, 3.16, 4.2–4.8, 5.6–5.11, 6.3, 7.2. workflow-p1-pass: Part Studio rows on qa-scratch/wf-block.noshape (40×20×10, Fillet R2, Chamfer, 2-point Hole + feature pattern; bin/mkblock.py); assembly rows on qa-scratch/pin.noshape (plate 60×20×5 with 3 Ø10 holes + Ø10×30 pin, built by bin/mkpin.py); left: drawing / BOM / explode / pattern / replicate rows. Next: the rest of interaction / navigation (touch / macOS / Windows rows need those platforms),
  workflow-p1-pass, workflow-undo, sketch.md rest.
- Filed (evening, greens 203c4d6f): P2 PREVIEW-SELF-PICK (psui: clicks on a feature preview's own faces give a self-reference), P2 DUP-COINCIDENT, P2 SKETCH-LINE-MENU (regression), P3 FILLET-POINT-MSG (skasm).
- Filed (later, greens 2a6d0167 / 1c853f78): P1 HOLE-SIZE-NO-DIAMETER (drill size doesn't set the diameter), P1 TAB-EXPORT-MISSING, P2 DIM-AC-ENTER (value boxes need an extra Enter when the autocomplete is open), P2 EXPR-PARSE-JUNK ("1 in26" → 126 in), P3 VERSION-FOOTER-STALE.
- Filed: P1 DIALOG-OK-STATE, P1 SHORTCUT-MAP-FOCUS, P1 SURF-EXTRUDE-SKETCH-EDGES (core), P1 OTF-FIRST-KEY (skasm, also
  in v0.4.10), P2 EXPR-ENTER-COMMITS (fixed by psui), P2 SKETCH-S-EXTRUDE, P2 PLANE-MENU-SELECTED, P2 SKETCH-RMB-ITEMS,
  P2 VAR-BARE-UNIT, P3 MENU-TRUNCATE; P1 SPACE-CHECKBOX filed and verified fixed.
- Verified fixed on f16b31c6 (18:40–18:46): HOLE-SIZE-NO-DIAMETER, TAB-EXPORT-MISSING, DIM-AC-ENTER, EXPR-PARSE-JUNK, VAR-BARE-UNIT, PLANE-MENU-SELECTED, SKETCH-RMB-ITEMS, LOFT-PROFILES-PINK, SKETCH-S-EXTRUDE, ASM-DIR-NO-HOVER; DIALOG-OK-STATE resolved (live = ours). On 203c4d6f: SURF-EXTRUDE-SKETCH-EDGES, BOX-FILLET-FACES. On 6cbb24e0: OTF-FIRST-KEY.
- Verified fixed on 1c853f78: SHORTCUT-MAP-FOCUS, EXPR-ENTER-COMMITS.
- Verified fixed on 2a6d0167: SPACE-CHECKBOX, EDGE-ON-PLANE-PICK, PARTS-SHIFT-RANGE, PART-HOVER-RED, REGEN-NO-CANCEL;
  SKETCH-SOLVE-SPLINE-SLOW on the lead's CLI (0.28 / 0.48 s, were 175 / 295 s).
- Note for psui: p1-dialogs.md says live's ✓ follows canSubmit (enabled once dirty), which contradicts DIALOG-OK-STATE (a);
  psui's live request decides; I left the ticket open.
- Shared prefs touched by testing: ~/.config/noshape/preferences.json got `featureDialogWidths.extrude 278` from a resize
  test (5.27); part_studio_toolbar.json `sketch_shortcut_tools` was set back to null.

## Requests to the lead (qa, updated 2026-10-28 21:05)
- **P0 DRAFT-BUDGET-POOL is back (21:05):** green e4cccdff (core c1f98ed6, OCCT patch 0005) never finishes opening
  tickets/draft-pool-hang-2133.noshape (memory climbing); 989f756b opens it in 26 s. The defeaturing's face/face stage runs on the OCCT
  pool even with serial pairs, and 0005 stops the budget from interrupting it. Revert 0005 or give pool work a real cancellation
  before the 05:00 cut.
- **P1 GREEN-ICONS-PATH (20:25):** greens since 4b9d7eaf are built in /home/ngyekta/projects/noshape-wt/watch; their rust-embed (no
  debug-embed) reads icons from that path at run time, so on this box the app has no icons. qa copied main's crates/app/assets there
  as a stopgap (see TICKETS › Lead / build). Please build the green where it runs or turn on rust-embed's debug-embed.
- **P0 DRAFT-BUDGET-POOL (core, regression of f024fd0d, 18:55):** with the pave-filler pairs on the pool, a Draft's defeaturing
  runs past its 30 s budget forever (memory > 4 GB): tickets/draft-pool-hang-2133.noshape never finishes opening; with
  NOSHAPE_BOP_PAIRS_SERIAL=1 it stops at 30 s and opens in 57 s. Together with BOP-PAIRS-GPF: please make the pairs serial
  by default before the 05:00 cut unless core fixes both.
- **P0 BOP-PAIRS-GPF (core, new in green 5267fdaf):** a GPF inside OCCT's IntTools_Context on a thread-pool worker
  killed the app during a Mirror preview; it arrived with f024fd0d (patch 0004, threaded pave-filler pairs). Unless core
  fixes it, please consider NOSHAPE_BOP_PAIRS_SERIAL=1 behaviour as the default before the 05:00 cut.
- **REGEN-SPEED (P0):** green f8014efa with the mesh cache still commits Extrude 1 edits on live-30-features in 165–725 ms
  (load 30–35) and the 3rd undo re-evaluates 29 features (131 ms). Booleans dominate under load (25–97 ms each).
- **P1 SKETCH-MODEL-EDGE-SELECT (skasm, new 10-28, also in v0.4.12):** in a sketch a click on a model edge selects nothing
  and clears the selection, so "line + model edge → constraint key" (an everyday flow) can't be done; tool-first works.

## Earlier requests (qa, 2026-10-27 20:44)
- **P0 BOX-DIALOG-PANIC (psui, new 20:42)**: since df28c320 (greens dd1b5037, daf1f50c) any box selection with a feature dialog open
  panics the app (FeatureDialog read inside its own update, behaviors.rs:1052). One-line fix; it must not ship at 05:00.
- P0 FILLET-OVERSIZE-OK (core): overlapping two-edge fillets / chamfers report Ok with impossible solids (live asked).
- Conflicting parity rows: sketch.md 14.29 says "Show constraints starts off in a new session" (live sketch/00), core-workflow.md 2.2 says ☑ (re-checked 10-23, was F1); live's behaviours 120 also shows ☑ (likely remembered). Ours: ☑ on a fresh profile. Needs one decision (and the other row changed).
- core-workflow.md 3.10 expects part-pattern copies in the colour rotation; workflow-p1-pass P1-13 (live) says copies keep the seed colour, which ours does. 3.10 should be rewritten.
- OTF-FIRST-KEY (P1 skasm) breaks typing a value right after drawing (the most common sketch flow); it is also in v0.4.10.
- SURF-EXTRUDE-SKETCH-EDGES (P1, core or psui): a surface/thin extrude of a whole sketch picked from the list fails.
- BUDGET-LONGJMP (add45a95, in green 4e568f3f): re-running seeds 120 / 131 / 72 on the watcher CLI 12:59 (logs
  cli/clifuzz-{120,131,72}L.log); results go into the ticket.
- v0.4.10 (e8e0f62d = e8858e61) fixes the built-in dialogs' picks (LP-DIRECTION-PICK, checked on its artifact) but not
  custom features' compound edge filters (`EDGE && GeometryType.LINE`, `EDGE && ALLOWS_AXIS` take sketch edges only):
  psui's 1e3c419c came after. The user's custom-feature example stays broken until the next release.
- BUDGET-LONGJMP (P0 core) has no owner commit: the blend / draft budgets siglongjmp out of OCCT from a timer signal; a
  fuzz CLI deadlocked in malloc after one (seed 120, twice), seed 131 hangs at a delete with the budget on. It ships in
  v0.4.9 and v0.4.10, and is the likeliest cause of BOOL-THREAD-CRASH (segfaults in BOPAlgo only on CLIs with the budget).
- SKETCH-SOLVE-SPLINE-SLOW (P0 skasm, new 12:25): in a 273-entity sketch with 2 fit splines one coincident takes 176 s and
  the next dimension 295 s (CLI 12:08); the solver's numeric gradient re-fits the spline per variable. Needs an owner.
- REGEN-NO-CANCEL (P0 psui) has no owner commit yet: a rollback waits up to 30 s+ behind a blend that's burning its budget.
- New today (owners): API-ASM-CONFIG-NAMES (P2 dx), CONFIG-INPUT-ID (P3 psui), FS-EDITOR-COMPLETE (P2 psui).
- EDGE-HIGHLIGHT-LOOK filed (P1 psui) with 1× / 2× profiles of v0.4.10 vs live. Note: live's own 1× capture
  (behaviours 18762, psui 1456) has a dark outline around the hovered edge too, so "unlike live" may be a 2× / Retina
  difference; the pending live capture decides. **BLACK-REGION not reproduced** on v0.4.10 Linux 1× / 2× (corner zoom
  to the vertex, tilts above / below the top face, bottom view), so not filed: please get the user's document (or note
  it's macOS / Metal). While building the repro: FEATURE-PATTERN-ADD-Y (P1 core): a Feature pattern of an Add extrude
  drops its Y-direction / second-direction instances (7 × 5 → 7 bosses, status OK) — likely what the user's plate
  with two linear patterns ran into.

## Release blockers seen by qa (updated 2026-10-27 06:10, green 4756f9be; see the newest "Green" section)
- Verified in the green app bcf401ca: the five crash documents open (DOC-POISON guard), TWO-PROCESS-LOST-EDIT (both edits
  kept). Still open P0: DRAFT-HANG (draft defeaturing spins; the app's regen never ends and blocks later ones), FILLET-HANG
  (budget 97081557 on main, checked on the watcher CLI: fillet / chamfer fail after ~30 s; not in green yet), SHELL-SLOW,
  REGEN-SPEED (commit 1.0–1.7 s at load ~35; undo 3 back 886 ms).

### Earlier (P0, green 4d2f6b0c)
- DOC-POISON + SHELL-SLAB-GPF + FILLET-SEGV: ordinary Shell / Fillet / Delete operations can segfault the green app and
  leave a document that crashes on every open; v0.4.8 opens the same documents. DRAFT-SEGV-DOC also hits v0.4.8.
- SHELL-WALLS: wrong solid with status OK (also in v0.4.8).

## 2026-10-28 release smoke (pre-cut, 04:35–04:42 on green 3f4e10d9 = `last-green` at 04:30; redo on the artifact once published)
- **Results at the top of the qa doc for the morning: all smoke rows OK on 3f4e10d9.**
- live-30-features: open regen 176 ms; Extrude 1 edits 256 / 167 / 135 ms (median 167, load ~20); Ctrl+Z 1 / 3 / 134 ms.
- Part › Export STEP (AP242) → + › Import → "smoke-3f4e" Part Studio (+ CAD Imports folder): 60779.484 mm³ = live.
- kill -9 0.3 s after Suppress Boss 1 → reopened with Boss 1 suppressed.
- Drawing (wf-block, two sheets) › Export PDF → 2 pages, letter landscape, sheet content right.
- asm5: dragging the Revolute pin rotates it in place (stays on its hole).
- Hover + pick in feature dialogs: Linear pattern Feature / Part / Face mode Direction takes the edge at its middle and
  the top face (not the plane); cfs.noshape "Edge pick" (EDGE && ALLOWS_AXIS) and "Plain edge pick" take two edges each.
- Fuzz seeds 45–52 × 600 on today's greens: no crash.
- Re-smoke on b504234e (last-green from 04:44, adds only UI fixes, all verified: REORDER stale tooltip, ASM-TOOLSEARCH,
  ASM-FILTER-PATTERN-INSTANCES): live-30 open regen 431 ms, edits 184 / 169 / 239 ms at load ~32 (median 184), Ctrl+Z
  6 / 2 / 208 ms; fuzz seed 53 × 600 alive.

## Daily release smoke
### Checklist rows run on every release (in addition to the open / regen / export / drawing / assembly / kill -9 rows below)
- **hover + pick in a feature dialog** (LP-DIRECTION-PICK regression, from 2026-10-27): lpd.noshape (bin/lpdoc.py) →
  Linear pattern in Feature, Part and Face mode: Direction pre-highlights and picks an edge at its middle and a face (not
  the plane behind it); Fillet › Edges and Shell › Faces hover + pick; cfs.noshape custom feature "Edge pick"
  (`EDGE && ALLOWS_AXIS`, 2 picks) and "Plain edge pick" both take two edges.
### v0.4.12 (tag e936fe98, published 05:49; Linux tarball smoked 05:53–06:03 on the box at load 20–27)
- Start page "(0.4.12.e936fe98a8b4)"; CLI STEP header 'NOSHAPE 0.4.12'; STEP export → import_files round trip of the
  l30 copy 83341.148087 = 83341.148087 (this l30 file is the 34-feature variant; v0.4.11 gives the same).
- live-30 (l30 copy) open regen 982 ms; Extrude 1 → 15 / 16 / 17: 140 / 107 / 120 ms (median 120); Ctrl+Z ×3: 1 / 1 /
  317 ms (3rd over 50, REGEN-SPEED). kill -9 0.3 s after Suppress Boss 1 → reopens with Boss 1 suppressed.
- Hover + pick: lpd Linear pattern › Feature pattern › Direction pre-highlights the bottom edge at its middle and takes
  "Edge of Extrude 1", then the top face "Face of Extrude 1"; Fillet edge, Shell face; cfs "Edge pick" and "Plain edge
  pick" take two edges each.
- wf-block › Create Drawing (ANSI_A_MM, suggested views) › Export › PDF → 1 page, letter landscape, content right.
- asm5 › Assembly 1: dragging the revolute pin leaves it in place (no translation).
- Fuzz: same code as green 1b81b7ca (seeds 51 / 52 alive there).

### v0.4.10 (tag e8e0f62d = main e8858e61 + "Release v0.4.10"; published 11:01, its Linux tarball is byte-identical to the CI run 40486478859 artifact smoked here; Linux, macOS, Windows assets), smoke 10:54
- Has 4e55b01f (LP-DIRECTION-PICK), a117663d (DRAFT-HANG budget), 011aaf43 (serial booleans), c30baf56 (markers);
  **not** 1e3c419c (custom features' compound edge filters: `EDGE && GeometryType.LINE` / `ALLOWS_AXIS` still take
  sketch edges only) and still the longjmp budgets (BUDGET-LONGJMP, BOOL-THREAD-CRASH).
- OK: start page "0.4.10.e8e0f62df5d6"; **hover + pick in a feature dialog**: Linear pattern › Feature pattern ›
  Extrude 2 › Direction takes the edge at its midpoint ("Edge of Extrude 1", highlighted on hover) and the top face
  ("Face of Extrude 1"); Fillet edge, Shell face (tickets/lp-pick-v0410-*.png); kill -9 0.3 s after Suppress keeps
  it; drawing (ANSI A suggested views) → PDF 1 page letter, DXF, DWG; STEP / Parasolid export → import 24800 exact;
  mate DOF matrix 8 / 8.
- All 14 crash / hang documents open through the v0.4.10 CLI (13.8–151.7 s; cli-draft-hang now finishes, 152 s;
  their failing features fail with messages). Geometry matrices on the v0.4.10 CLI: fillet 32 / 32, featmatrix 14 / 14,
  hole 12 / 12, shell 96 / 96 (walls everywhere), boolean 8 / 9 (the 9th is the empty touching intersection the script
  can't measure), transform 5 / 5. 2× (GPUI_X11_SCALE_FACTOR=2) UI renders right.
- Found on it: EDGE-HIGHLIGHT-LOOK (user report, reproduced), FEATURE-PATTERN-ADD-Y (also on main).

### v0.4.9 recut (release/v0.4.9 7ee597e0 = main 502416cc + "Release v0.4.9"), 2026-10-27 05:00, smoke on green 502416cc
- **The earlier blocker is gone**: the recut contains bf8aaee2 / 87e8275f (crash guard, OCCT patch, DOC-POISON rollback),
  7a04ba4d (SHELL-WALLS) and 97081557 (blend budget). The five crash documents open in the green app (bcf401ca and later);
  cli-fillet-hang.noshape opens in 47 s (Fillet 2 fails after the 30 s budget) instead of hanging.
- OK: 30-feature part full regen 634 ms on open; edit commits 707 / 396 / 344 / 200 / 318 / 498 ms (load ~25, REGEN-SPEED
  still over 200 ms); undo 1 / 1 ms, then 618 / 390 ms past two states; Create Drawing (suggested views, 1:2) + PDF (1 page
  letter, 1.5); STEP and Parasolid export → import 60779.484 = live (48 faces); STL / 3MF / OBJ meshes closed, 3MF manifold;
  assembly opens, a revolute pin drags without moving off its hole; kill -9 right after a Suppress keeps it; geometry
  matrices on the watcher CLI 04:48: fillet 32/32, boolean 11/11, shell 96/96 (walls everywhere; hole cases 3.5–7.4 s),
  hollow 60/60, rim 36/36, transform 5/5, split / mid plane exact; UI Part mirror about Right = 121558.969 (2×).
  featmatrix 14/14 once its REMOVE passes oppositeDirection (API-REMOVE-DIR: by design, lead's decision); patmatrix's ×50/×100 overlapping holes now fail "Could not
  create all instances as entered" on purpose (live's rule, pattern.rs copies_fail); holematrix (now styleV2) 12/12 exact.
- Also OK (green 525c619b, 05:20–05:50): drawing Update after a depth change (views follow, the dimension stays, the
  button goes dim), STL import (+ › Import…: "l30" Part Studio, mesh 60774.111 = the file), UI sketch on a face → circle
  → Ø dimension 10 → Remove extrude (34044.055 exact), dimension editor, undo empty after reopen (= live).
- **New P0 seen while smoking: BOOL-THREAD-CRASH** (OCCT boolean segfaults on pool threads, nondeterministic; fuzz seed
  72 twice on the watcher CLI 04:06); REGEN-NO-CANCEL (P0). **The release CI's Windows build failed** (TICKETS).
- Known open in the release: DRAFT-HANG (P0: a draft can spin forever; the app's regen then never ends), SHELL-SLOW (a
  failing shell 52 s), REGEN-SPEED, API-REMOVE-DIR (P1: scripted REMOVE extrudes miss; featmatrix's hole patterns fail).
- CI artifact (run 40452492018, Linux; checked 08:05): version 0.4.9.7ee597e05af9; the five crash documents open;
  fillet-hang opens in 68 s; **draft-hang never finishes (DRAFT-HANG ships)**; **Windows build fails (cxxabi.h)**;
  cli-delete-crash opens in 148 s (Shell 3 120 s, SHELL-SLOW).

### v0.4.9 candidate (release/v0.4.9 2650df69 = main 34ab98f7 + CI fix; CI run 40434076347 Linux artifact), 2026-10-27 03:10
- **Blocker:** the build segfaults opening 4 of qa's 5 crash documents (SHELL-SLAB-GPF, FILLET-SEGV, FILLET-SEGV-BLEND,
  DRAFT-SEGV-DOC); it predates bf8aaee2 / 87e8275f / 7a04ba4d. Green 19555ca3 opens all five. See TICKETS "Release v0.4.9".
- OK: 30-feature part opens (full regen 618 ms at load ~40), 60779.484 mm³ = live; edit Extrude 1 → 12 commits (710 ms),
  undo cached (2 ms); new Part Studio: centre rectangle + dims, extrude, fillet, shell; Create Drawing (suggested views,
  1:2, 4 views) in ~10 s; drawing PDF export (1 page letter, PDF 1.5).
- Not run: assembly, import/export round trip, kill -9 durability (done on green the day before).

Checklist (30 min, on /root/green/noshape at the release commit; numbers to compare in brackets):
1. tests/perf/live-30-features.noshape: opens, mass properties 60779.484 mm³; edit Extrude 1 10 → 12, ✓, Ctrl+Z, Ctrl+Y
   (regen line in the NOSHAPE_PERF log; undo/redo cached).
2. New Part Studio: centre rectangle 80×50 + dims, extrude 10; fillet top face R2; chamfer 1; sketch points + Hole
   (counterbore); Shell top 2.5 on the plate **without** a hole (14467.382 mm³); linear pattern; mirror; boolean.
3. Sketch: fillet R5 → R20, trim, offset, dimension edit, Esc / ✓.
4. Assembly: insert 2 parts, revolute mate via tool search, drag (only the dragged part moves).
5. Drawing: Create Drawing (suggested views), Update after a model change, export PDF + DXF (R14, mm).
6. Part › Export STEP, + › Import it back (same volume); Documents window › Create › Document.
7. Durability: kill -9 right after an edit, reopen, the edit is there.
8. Re-run /root/qa-scratch/bin/{shellmatrix,hollowmatrix,holematrix,rimmatrix,boolmatrix}.py if a CLI is shipped.

## Setup that works
- Start the app with q.sh `appstart <bin> <doc> [secs] [log]` (saves the timeout's PID in /root/qa-scratch/app.pid) and
  stop it with `appstop`; fuzzers run detached with `timeout` and are polled via their logs. (The lead's 09:40
  no-approval-prompt rule was withdrawn at 09:45: commands needing approval are approved by the lead.)
- **Crash markers** (`/tmp/noshape-regen/*.marker`, CRASH-MARKER-COPIES): a process killed mid-regen (kill -9, `timeout`)
  leaves one, and the next document opened with that feature id (any copy of the same fixture) opens rolled back. After
  a kill test, expect the next copy of that fixture to open rolled back, or remove your own dead pids' markers first.
- **Give the app window X focus first** (`xdotool windowfocus --sync <id>`, q.sh `focusw`): :110 has no window manager, so
  without it GPUI sends no focus/blur events (Tab / click-away commits never fire; TAB-NO-PREVIEW was this artifact).
- App: `cp /root/green/noshape /root/qa-scratch/noshape-<sha8>`, run with `NOSHAPE_PERF=1 NOSHAPE_PERF_PERIOD=1
  NOSHAPE_NO_NATIVE_DIALOGS=1 DISPLAY=:110`, window resized to 1600×1000 at 0,0 (xdotool). Helpers:
  /root/qa-scratch/bin/q.sh (shot, crop, clk, dclk, rclk, key, typ, settle). Regen times per feature come from the
  `regen[...]` lines of the perf log.
- Documents are SQLite (`.noshape` + `-wal`); to hand one over while the app runs:
  `python3 -c "import sqlite3; s=sqlite3.connect('file:X?mode=ro',uri=True); d=sqlite3.connect('Y'); s.backup(d)"`.
- Live: my own Chrome tab via /root/live-pw/rqa.mjs (copy of r.mjs; tab id in /root/qa-scratch/.qa-tab; 180 s CDP
  connect timeout). Each run costs ~2 min just to attach on this loaded box, and page.goto needs `timeout: 0`; batch
  checks into one script, or ask the live agent in REQUESTS.md.
- CLI (v0.4.8 release, /root/qa-scratch/rel/…/noshape-cli) through a small MCP client: /root/qa-scratch/bin/nscli.py
  (`Cli()('feature_add', {...})`). Matrices with analytic volumes: shellmatrix.py, hollowmatrix.py, filletmatrix.py,
  featmatrix.py, holematrix.py (same dir). Hole style needs `styleV2`; a Remove extrude needs `booleanScope`.
- tools/lead/disk-loop.sh deletes files named noshape* > 100 MB older than 2 h under /root/*-scratch (maxdepth 4): keep
  binaries at depth ≥ 5: green copy /root/qa-scratch/rel/k/green/x/noshape-<sha8> (symlinked from /root/qa-scratch),
  v0.4.8 in /root/qa-scratch/rel/k/v048/…, the watcher's main CLI in /root/qa-scratch/rel/k/main/x/.
- The watcher's own CLI build (target-lead, ~20:18, main d62f25e2/c9540ae6, not green) is copied to
  /root/qa-scratch/rel/k/main/x/noshape-cli (`NSCLI=… python3 …` with nscli.Cli(os.environ['NSCLI'])).
- "View in high quality" on this llvmpipe box makes viewport frames take > 10 s ("GPU frame still running after 10s;
  waiting"); the UI then stops taking input for a minute and replays the queued clicks. Click Disable on the blue bar.
- nscli.py starts the CLI with PDEATHSIG, so a `timeout`-killed fuzz run no longer leaves an orphan CLI; an orphan
  still burning CPU is a hang (seed 33 → FILLET-HANG).
- CLI modelling fuzzer: /root/qa-scratch/bin/clifuzz_dbg.py <seed> <n> (NSCLI=<cli>; random fillet / chamfer / shell /
  extrude / draft / pattern / mirror / suppress / delete / undo; log cli/clifuzz-<seed>.log; NSCLI_ERR=<file> keeps
  stderr); clifuzz_bisect.py replays a seed with a cold open in a fresh process after every step. Seed 1, 5 → DRAFT-SEGV-DOC; seed 7 → SHELL-SLAB-GPF; seed 9 → FILLET-SEGV; seed 11 (no drafts) → SHELL-SLAB abort; seeds 2, 3, 4, 6, 8 clean. The v0.4.8 release CLI
  (seeds 21–24, FZTAG=r): only the Draft crash (seed 22), so the Shell / Fillet crashes are new since v0.4.8.
  clifuzz_nodraft.py skips drafts; main seeds 30–35: 32 → FILLET-SEGV-BLEND (also v0.4.8), 33 timed out, rest clean.
  Sketch fuzzer skfuzz.py (draw / constrain / dimension / trim / offset / delete / undo): seeds 1–3 × 200 ops on main, no
  crash. Crash rate of the modelling fuzz on main: ~1 per 100–150 random operations.
- UI fuzzer: /root/qa-scratch/bin/fuzz.py <seed> <n> (random keys / clicks / drags on :110 against the pid in
  /tmp/qa_fzpid; actions logged to /root/qa-scratch/fuzz-actions.log). Seeds 1–3 (800 actions): no crash.
- Don't run the app with NOSHAPE_PERF=1 when testing section views (PERF-SECTION-PANIC freezes the viewport).
- Mind dialog layout shifts: once a selection chip is added, every row below moves down ~16 px; screenshot before
  clicking a field.

## Tested
### 2026-10-26, green 4d2f6b0c (13:26), box load 35–40
- 30-feature part: open (full regen 970 ms), edit Extrude 1 → commit 1007 ms (REGEN-SPEED), undo/redo 7/5 ms, mass
  properties 60779.484 mm³ = live.
- New studio: centre rectangle + Ø20 circle, dims, extrude 10, fillet top face R2, chamfer 2 bottom edges 1 mm, 2 sketch
  points + Hole, Shell top face → SHELL-WALLS (P0) and SHELL-SLOW (P0). Same plate without the hole shells fine.
- 20:30–20:52: kill -9 right after a Suppress → reopen shows it (durable); STEP export → import round trip exact (14467.382
  mm³); Revolve about a sketch line, Boolean union of disjoint parts (live's info text), counterbore hole −634.522 mm³
  (= hand calc), part pattern ×100 in 73 ms, sketch fillet R5 default then sticky, new document from the Documents page
  (Part Studio 1 + Assembly 1), "Close document" checkpoints the WAL into the .noshape. New: PATTERN-OFFPART-PARTS (P0,
  live asked), TAB-NO-PREVIEW (P1), DELETE-TAB-DIALOG (P2), VERSION-0.4.6, a 2nd SHELL-WALLS repro. Big plate (400 holes):
  regen 3.0 s (Extrude 2 2458 ms) vs live 617 ms, on a box at load ~40.
- 21:00–21:40: CLI matrices on the v0.4.8 release: fillet / chamfer of all 12 box edges (32 cases), extrude draft (inward,
  4 + 2 cases), partial revolve (5), feature pattern of a hole ×2…200 (0.86 s for 200), hole Simple / Counterbore /
  Countersink × Through / Blind (12): all exact. Shell: 96-case matrix → SHELL-WALLS pattern (hole + R < t ≲ 1.5R) and
  SHELL-SLOW (8–17 s); hollow / open-top shells of a filleted box with t > R wrong (SHELL-FILLETED-BOX, live confirmed).
  Import g25-b x_t / sat / step: three volumes (I1). PDF export of a drawing OK (1 page letter, v1.5). Measure readouts
  (face + edge "Perp line and plane angle") = live. New tickets: DCLICK-HEAVY, PERF-SECTION-PANIC, LOFT-PROFILE-LIST,
  RHINO-GREEN, API-HOLE-STYLE, API-REMOVE-SCOPE; PATTERN-OFFPART-PARTS closed (live does the same).
- 22:00–22:40: two app processes on one file → TWO-PROCESS-LOST-EDIT (P0); bad files open silently (OPEN-ERROR-SILENT);
  UI fuzzer 1300 random actions over Part Studio / drawing / assembly tabs, no crash (PANEL-BLEED found); feature-row drag
  reorder, rollback-bar drag, Roll to end, Variable Studio (#var autocomplete, change propagates), material assign +
  mass (0.191 kg), Duplicate / Delete of a tab with an open dialog, chamfer types (equal / two / distance-angle),
  up-to-next / face / part ends, thin extrudes, draft (neutral plane), split by plane: all as expected. Slot tool = live.
- 22:40–23:00: Add-extrude merge scope with two touched parts (empty + "No merge scope selected.", switches back to New
  when a depth change reaches a second part) = live; configurations via CLI (list input, configured depth and
  suppression: 11882.891 / 30000.000); sketch offset / constraints / dimensions via CLI exact; sweep along line + tangent
  arc exact (self-intersecting → "Could not create valid swept body, check input."); circular pattern exact; sketch
  cancel → Restore reopens the sketch with the edits; view cube arrows 15° per click (= live). New: HOLE-MISS-MSG,
  PATTERN-REAPPLY-CUT-ONLY.
- Checked and same as live: R = centre-point rectangle, Shift+C = Curve/surface analysis (no Chamfer shortcut), Enter in
  Depth keeps the dialog, Esc in an open end-type list closes only the list, Hole dialog defaults (Inch tab, 1/4, 12 mm,
  118 deg), "Features …" + red badge while a new feature is invalid.

## Open findings (mine) by area
- core: **DOC-POISON, DRAFT-SEGV-DOC, SHELL-SLAB-GPF** (P0 crashes that leave unopenable documents; SHELL-SLAB-GPF is a
  regressions since v0.4.8, all three crash the green app on open), SHELL-WALLS, SHELL-FILLETED-BOX, SHELL-SLOW,
  REGEN-SPEED, TWO-PROCESS-LOST-EDIT (all P0); I1 (P2); carried over qa5's P1-16 / P1-6.
- psui: TAB-NO-PREVIEW, DCLICK-HEAVY (P1); DELETE-TAB-DIALOG, LOFT-PROFILE-LIST, PERF-SECTION-PANIC (P2); carried over
  qa5's P1-8 caveat, P1-10.
- dx: API-HOLE-STYLE (P1), API-REMOVE-SCOPE, VERSION-0.4.6, RHINO-GREEN (P2), USER_GUIDE undo sentence (P3).
- Live requests: none open (title block: live's ANSI templates have no MATERIAL/WEIGHT fields either = ours).
- skasm: carried over qa5's P1-4 data.

## qa4 round-5 routed items re-checked on green 4d2f6b0c (2026-10-26 22:00)
Fixed ✅: DRAG-RUNAWAY (19-cube planar stack: only the dragged cube moves), SUPPRESSED-MENU, SKETCH-UNCONSUMED (docs made
by this build; docs made by older builds keep a sticky Hide), TOOLTIP-PARENT, ERR-MISSING-REGION ("Select face or sketch
region to extrude." + "1 missing selection"), CHAMFER-OVERFLOW (25 on a 20 mm edge OK; v0.4.8 still fails), SECTION-MANIP
(ring + arrow + rotate glyph), empty rename → default name, SELECT-OTHER, BOX-PREHIGHLIGHT, MIDPOINT-MARKER (dark ring at the hovered
edge's midpoint), READOUT-AXES, EXT-2ND-DEPTH, EXPORT-DXF-OPTIONS (DXF R14, mm, 75 LINE / 40 MTEXT / 4 CIRCLE / 4 ELLIPSE).
FOLDER-EXPAND-UNDO, FOLDER-MENU (Rename | Add selection to folder… | Suppress | Dynamic suppression ▸ | Expand ▸ | Collapse ▸ |
Delete | Unpack folder). Still open (tickets): EXT-DIRECTION, CHIP-HOVER, FOLDER-NAME-INLINE. Not re-checked yet: SEL-REJECT-TOAST (couldn't trigger: an edge click
in Extrude's regions field picks the face), ORD-BASE.

## 13:05–13:45 (greens 4e568f3f → eb4338a8)
- Verified on eb4338a8: EDGE-HIGHLIGHT-LOOK (1× profiles = live's new capture to a few units), SIL-AXIS-VIEW-GONE (gone
  again), REGEN-NO-CANCEL (stale run aborted; the thumbnail then re-runs the head in the background, by design).
- 4e568f3f: tool-open smoke 87 / 87 dialogs pixel-identical to 0d75aa8e; pick sweep 28 / 28 dialogs identical (hover
  crops differ only by load timing; checked by hand); featmatrix 14 / 14, holematrix 12 / 12 on the watcher CLI 12:59.
- BUDGET-LONGJMP fuzz on the watcher CLI 12:59 (has add45a95): seed 120 ran all 150 steps (it deadlocked at step 28
  twice before); seed 72 died once at step 63 (delete; marker Mirror 8; no kernel trap, so an abort; the saved document
  opens fine, tickets/cli-72L-died.noshape) and its replay with stderr ran all 106 steps clean: nondeterministic, i.e.
  BOOL-THREAD-CRASH still there; seed 131 see TICKETS.
- UI walk on eb4338a8: Split by Front plane (halves 14137.167 = half the tube), Transform › Translate by XYZ (type
  list = live's order), Plane offset; drawing of the tubes (suggested views) → DWG-REVOLVE-SEAM (P1 dx). New:
  PART-HOVER-RED (P2 psui). Asked live: part names after Split (ours renumber every default-named part), and the
  sketch line under an edge-on plane.

## 12:40–13:05 (greens e598c3a5 → 4e568f3f; qa moved to the Motile account at 12:40)
- Verified: OK-DISABLED-HOVER, CONFIG-CELL-EDIT-LOOK, EXPORT-OBJ-MTL (e598c3a5), REPLICATE-EDGE-DEFAULT (4e568f3f).
- 30-feature part on e598c3a5 (load 45–68): open 1.18 s; Extrude 1 edits 460 / 1056 / 1327 ms; 3rd undo back 883 ms
  (REGEN-SPEED unchanged); section view (Plane 1, tan preview caps, "Section plane 1 (Top)" = live).
- New doc walk: Front sketch, rectangle + dims, Revolve about a sketch line → 28274.334 mm³ / 7539.822 mm² exact; Part
  mirror about Top, Part linear pattern 3 × 50 (Tab updates the preview, = live), mass properties of 2 = 56548.668.
- New: SIL-AXIS-VIEW-GONE (P1 psui), PARTS-SHIFT-RANGE (P2 psui), EXPORT-OBJ-ZIP-NAME (P3 dx). Doc tickets/tubes6.noshape.
- Not filed (unsure of live): a sketch line lying on the Right plane's edge-on line in the Front view can't be hovered /
  picked as Revolve axis (picks fine once orbited).

## 11:35–12:03 (green e8858e61 still; watcher build 0a25dd1a = all six app crates split, 11:32)
- **Green 0d75aa8e (12:04, all six app crates split)**: tool-open smoke 87 names: every crop pixel-identical to green
  e8858e61's (79 dialogs, 8 hidden types absent); pick sweep 27 / 28 as before (Hole refuses an edge); verified
  API-ASM-CONFIG-NAMES, CONFIG-INPUT-ID (lists), PERF-TOTAL-MISMATCH (1.13 s both), Ctrl+Space in the FS editor. New
  P0 SKETCH-SOLVE-SPLINE-SLOW from the timed sketch fuzz (also v0.4.10: 61 s for one coincident). (12:31)
- The split's hooks (main.rs-registered calls from the split UI crates) all open on 0a25dd1a: Part context menu ›
  Assign material (Material panel, Onshape Material Library), Export (Parasolid 38.1), Release (Create Release
  candidate), Create Drawing (templates, creates "Part 1 Drawing 1" with Insert view); right strip Variable table;
  help › Keyboard shortcuts; Derived (Part Studio field); + › Create Variable Studio / Feature Studio (template
  FeatureScript 3095 common.fs); Assembly › Insert › Current / Other documents › Owned by me (the library lists the
  documents at Start); document menu; Release drafts panel; tab manager. Shots tickets/split-0a25/.
- P1 Part Studio walk on 0a25dd1a (12:08): face preselected → Sketch on it, N normal view; corner rectangle (G), circle
  (C), dimensions (D) width 25 and Ø8 (Esc on a value box keeps the measured value, as live); Offset (O) pick the
  circle, move out, click → 5 mm box, kept with its dimension; Q toggles construction; ✓; Shift+E, picking the
  ring region switches New → Add (as live), Enter with nothing picked stays open; rename via menu, Suppress (its
  sketch shows again), Ctrl+Z ×2 / Ctrl+Y right, rollback bar drag above a feature, Delete of a rolled-back feature,
  undo. No findings beyond those filed. (Offset with the right side still selected says "Can only offset a single
  chain at once." like live.)
- Verified on 0a25dd1a: LP-DIRECTION-PICK compound filters, MEASURE-MAX-DIST, EMPTY-COMMIT, CONFIG-CELL-EDIT; on green:
  LOFT-SKETCH-HIDE, PANEL-BLEED, P1-4; on v0.4.10: SKETCH-DRAG-BIG (7–16 ms steps at load 47). REGEN-NO-CANCEL half
  (rollback instant, stale run not aborted), PS-SECTION-RED reopened (overlap is a hole). New P3: OK-DISABLED-HOVER,
  CONFIG-CELL-EDIT-LOOK. 3f5f69ed / cc6c4b54 are CI test items (T1/T2), not UI-verifiable here.

## 10:10–10:45 (green e8858e61)
- LP-DIRECTION-PICK: v0.4.9 reproduced, green verified (pattern Direction in all three modes, Fillet, Shell, plain custom
  edge query); compound custom filters (`EDGE && LINE`, `EDGE && ALLOWS_AXIS`) still take sketch edges only (ticket,
  cause in studio.rs). Release smoke row added.
- Feature tool open smoke (bin/toolsmoke.sh, 87 names from featnames.txt via Search tools, lpd.noshape): 79 dialogs open
  and cancel cleanly (Esc leaves the 8 base features), no panic in the log; the other 8 (Derived mirror, Import, Point,
  Recognize, Rip, Cube, Sphere, Copy part) aren't in Search tools because they're `.hidden()` types (internal /
  deprecated / FS primitives), as expected. Contact sheets: /root/qa-scratch/ts/sheet0–3.png.
- Pick sweep (bin/picksweep.sh, lpd.noshape, green e8858e61): 28 dialogs, one click on the part's top edge or top face
  into the first field: 27 take it ("Edge of Extrude 1" / "Face of Extrude 1" / "Part 1" in part fields: Chamfer, Fillet,
  Offset curve, Composite curve, Extrude, Shell, Draft, Plane, Mate connector, Move / Delete / Replace face, Offset
  surface, Thicken, Isocline, Face blend, Split, Boolean, Transform, Mirror, Linear / Circular pattern, Body draft,
  Intersection curve, Delete part, Enclose, Composite part); Hole (sketch points) refuses an edge, as it should. Crops
  /root/qa-scratch/pk/d0.png, d1.png. Good release-smoke companion to the "hover + pick" row.
- Custom features geometry (bin/cfgeom.py / cfgeom2.py, CLI 09:50): fCuboid (+ opFillet all edges), fCylinder +
  opBoolean in a loop, sketch + opRevolve torus: volumes exact (rounded box 3839.032 = Minkowski formula); param edit,
  undo / redo, suppress, re-commit of changed source all right; a syntax error committed puts every instance in Error
  "Feature Studio has errors: Line 45: …". Not ticketed: an opBoolean SUBTRACT that consumes the whole target leaves no
  part and status OK (live unknown).
- Assembly Fastened mate on asm5: hover on a hole rim shows the connector, both picks go in (tickets/pick-green-asm-*.png).
- Baseline before the app split reaches green (screens in /root/qa-scratch/baseline-e8858e61/): Feature Studio editor
  (+ Ctrl+Shift+F search panel), Performance and Document notes panels, help menu, start page (logo), user menu,
  Preferences window. Compare after the next green. New: FS-EDITOR-COMPLETE (P2). Main's Cargo version is still 0.4.6
  (start page footer "0.4.6.e8858e61fb4d"); only the release branch bumps it.
- Configurations (CLI 09:50; bin/cfgmatrix.py, cfgasm*.py, cfgbom.py, vstest.py): Part Studio List × Checkbox ×
  config Variable 8 / 8 exact; Variable Studio list-configured #boss inserted by name or id right; the "Insert into all"
  refusal message matches live. New: API-ASM-CONFIG-NAMES (P2 dx: assembly_insert / set_configuration / drawing_create
  ignore inputs given by name), CONFIG-INPUT-ID (P3 psui). The app's Change configuration… dialog is right.
- Mate DOF matrix (bin/matedof.py, asm5 plate fixed + a new pin, circle edge to hole edge): fastened 0, revolute 1,
  slider 1, cylindrical 2, pin slot 2, planar 3, ball 3, parallel 4: all as expected, every mate ok.
- bin/hangwatch.sh runs (pid in /root/qa-scratch/hangwatch.pid): gdb backtrace of any fuzz CLI that stalls (no CPU
  180 s) or spins (its fuzz log idle 600 s) into cli/hang-<pid>-<time>.txt. Seed 131 rerun with the budget on is at
  step 66 (delete) at 97 % CPU.
- Assembly export round trip (bin/asmrt.py, asm5): STEP → import = Assembly (3 instances, same placements; the two pin
  instances swap <1>/<2>) + a Part Studio with each part once (39430.089 / 2356.194 exact); IGES and Parasolid → 3 placed
  parts, exact; GLB writes. No findings.
- Fuzz: batch 9 (seeds 120–127, watcher CLI 08:30): seed 120 hit the 1500 s timeout again (the BUDGET-LONGJMP malloc
  deadlock seed), 121–127 clean. Budget A/B (seeds 130–137, CLI 09:50, on vs NOSHAPE_BLEND_BUDGET=100000): 130 clean on
  both; still running (~23 min a seed). Cold-open fuzz 140–141 consistent.

## 10:00–10:10 (green 3197362b, watcher CLI 09:50; other agents paused for the app split)
- Running: A/B budget fuzz (seeds 130–137, NOSHAPE_BLEND_BUDGET default vs 100000; logs cli/clifuzz-13*on/off.log,
  results cli/ab-on.out / ab-off.out, stderr err-on/off-*.txt); cold-open consistency fuzzer bin/clifuzz_cold.py (every 10
  steps a fresh CLI opens a backup and must match volumes / statuses; seeds 140–141, so far all OK); batch 9 (120–127).
- Sheet metal vs live "fs (10-23 05:00)": volumes, boxes, flat patterns (80.126106 / 76.126106 × 20) exact; SM-FACES (P3:
  10 faces vs 14). UI: construction toggle (q), Measure details panel, assembly fuzz on 3197362b (600 actions) clean.

## 09:40–10:00 (greens dacce5e7 → 3197362b, watcher CLI 09:50)
- Verified: UNDO-NOOP-SHIFT (minimal repro + bin/undochk_noop.py seed 5 full path; note history_list returns 50 entries,
  the checkers now track the head id), API-MATE-RETYPE-NAME.
- Live answers applied: CONFIG-CELL-EDIT confirmed; an unchanged ✓ is an edit in live too.
- New: PLDRAFT-VOLUME (P3: curved parting-line drafts +2e-6..4e-6 of live's 4 cases, bin/pldraft.py).
- v0.4.9 fcd301e2 CI artifact: crash documents open; DRAFT-HANG still ships.
- OK: depth field 0 / 500 m / −5 mm as live, tab Duplicate (independent copy) + inline rename, assembly STEP export →
  import (3 instances, positions kept), assembly tab after the Assembly-UI crate split.

## 09:10–09:40 (green dacce5e7 → eab98680, watcher CLI 08:30)
- **BUDGET-LONGJMP (P0 core, new)**: the budgets siglongjmp out of OCCT from a timer signal. Fuzz seed 120 deadlocked
  on glibc's main_arena lock (backtrace + 2.6 GB core in tickets/). Seed 93 with NOSHAPE_BLEND_BUDGET=100000 ran
  through its usual crash step (seed 72 no-budget replays running): supports it as BOOL-THREAD-CRASH's cause.
- Re-checked old tickets: BOOL-TOUCH-INFO (now INFO but the parts are kept; live deletes them), HOLE-MISS-MSG (ours now
  auto-flips the hole; live doesn't), PATTERN-REAPPLY-CUT-ONLY and ADV2 unchanged.
- New P2/P3: API-MATE-RETYPE-NAME, LEFT-RAIL-WHERE-USED, PERF-TOTAL-MISMATCH; MASS-INERTIA-APPROX matrix (torus Ixx off at
  3 decimals, tube Ixy); DWG-DIM-TWO-EDGES: the API's smart dimension is right.
- OK: versions (create V1, view-only V1, back to Main), feature list filter, Show dependencies, mate DOF per type,
  interference volume, drawing exports (DXF R14 / SVG / PNG / PDF), Extrude end-type dialogs = live captures, measure
  diameters / centres / lengths exact, no memory growth over 30 commits (~1 GB plateau).
- release/v0.4.9 is now fcd301e2 (Windows fix in; DRAFT-HANG budget not in; carries the longjmp budget); its CI is running.

## Green dacce5e7 / watcher CLIs 08:12–08:30 (2026-10-27 08:30–09:10)
- New: MEASURE-MAX-DIST (P1: max distance between curved faces / circles is sampled, 94.850 vs 94.868, 89.981 vs 90),
  MASS-INERTIA-APPROX (P2: curved parts' inertia / CoM approximate; boxes exact), UNDO-NOOP-SHIFT (P1: a no-op op pushes an
  undo step without a history entry, undo labels shift; app: an unchanged ✓ records an "Edit" entry, live asked),
  CONFIG-CELL-EDIT (P2), and from live's batch 2: EMPTY-COMMIT, REPLICATE-EDGE-DEFAULT, DWG-DIM-TWO-EDGES (P2),
  EXPORT-OBJ-MTL (P3); LOFT-SKETCH-HIDE confirmed by live.
- BOOL-THREAD-CRASH: still dies with serial booleans (replay B of seed 72 at step 60, no segfault line); replays with
  stderr running. SHELL-SLOW: with cb84d673 holes plates ~1 s (was 1.6–2), the failing shell 35–40 s.
- OK: UI counterbore hole (11448.955 exact), undo/redo paths without no-ops (3 seeds), Line-angle plane, measure min /
  centre distances, surface areas, Steel mass, box inertia.

## Green 4854cad4 (2026-10-27 08:17–08:30)
- REGEN-SPEED: commit median ~250 ms at load ~27 (serial booleans didn't slow it). Replays of seeds 72 / 93 on the
  serial-boolean CLI (08:12) running to check BOOL-THREAD-CRASH.
- UI workflows OK: Assembly from scratch (Insert dialog, 2 parts, Fastened mate by edge centres), configurations (Configure
  Part Studio, row Big, Instance count › Configure, value set through the dialog: Parts (5)) but CONFIG-CELL-EDIT (P2),
  Variable feature (#len = 12 mm, (x) row; used before its definition → pink field, red title), drawing BOM table
  (Item / Quantity / Part number / Description, quantities right), drawing Detail view (DETAIL A, SCALE 1:1), 12 feature
  dialogs side by side with live's captures (Boolean, Draft, Split, Transform, Circular pattern, Thicken, Sweep, Helix,
  Rib, Move face, Delete face, Offset surface): identical. UI fuzz on the drawing tab 600 actions clean.

## Green 136c6738 (2026-10-27 08:10–08:20)
- UI workflows OK: Revolve (dialog = live revolve-01; full revolve of a 10×30 rectangle about a sketch line 28274.334
  exact), Part linear pattern ×3 / 50 mm along the Right plane's normal (3 parts, exact), drawing Section view (Vertical,
  label A, placed beside the parent: hatched 20×82), Shell dialog = live shell-02 (✓ waits for the slow preview).
- Fuzz: batches 7 clean; 6 had BOOL-THREAD-CRASH #4 (seed 93) and a 5-min blend stall (seed 95); batch 8 running.

## Green 4756f9be (2026-10-27 06:05–06:10)
- Verified: DRAFT-HANG (both docs open in 27 / 45 s; each failing draft still stalls ~11 s), CRASH-MARKER-COPIES,
  TWO-PROCESS-LOST-EDIT's live half (the other window follows within 2.5 s).
- Since 05:22: BOOL-THREAD-CRASH (P0 core, 3 crashes of fuzz seed 72 on the watcher CLIs, v0.4.8 survives it);
  LOFT-SKETCH-HIDE widened (an extrude whose region is picked on edit); API-REMOVE-DIR closed by the lead (by design);
  release Windows CI failed; my scripts: holematrix → styleV2, featmatrix → explicit oppositeDirection + X edge.
- Open P0 now: BOOL-THREAD-CRASH, REGEN-NO-CANCEL, SHELL-SLOW (52 s failing shell), REGEN-SPEED.

## Green 502416cc (2026-10-27 04:53–05:22)
- Verified: FILLET-HANG (the hang doc opens in 47 s; residual 30 s stall per failing blend), EXPORT-MESH-SHARED,
  DEPTH-RANGE (pink only after blur, live also while focused), CHIP-HOVER.
- New: REGEN-NO-CANCEL (P0 psui: a newer regen waits for the stale one), CRASH-MARKER-COPIES (P2 dx), API-REMOVE-DIR
  raised to P1 (breaks scripted cuts; featmatrix fallout), DRAFT-HANG second occurrence.
- Checked = live: Line-angle plane normals (30° / 120° exactly live's), transform ×5, split, mid plane, UI mirror,
  mass properties panel (no material: CoM blank + toast), delete a used sketch (dependent red, undo restores), inline
  rename, new document from the Documents window → sketch (R) → extrude → fillet (volume exact), UI fuzz 2×700 clean.

## Green bcf401ca (2026-10-27 04:10–04:40)
- Verified: OPEN-ERROR-SILENT (my 16cdcf6d reopen was wrong: that build lacked 3dfa5710), SKETCH-PASTE, PERF-SECTION-PANIC,
  MENU-SKETCH-TAG, crash documents in the app, TWO-PROCESS-LOST-EDIT, API-REMOVE-SCOPE, USER_GUIDE; P1-16 / P1-6 shell on the
  watcher CLI 04:06 (live's numbers).
- New: API-REMOVE-DIR (P2 dx), EXPORT-MESH-SHARED (P2 dx: 3MF/OBJ not welded); FILLET-HANG gained a chamfer repro
  (bin/chrepro.py + tickets/cli-chamfer-hang-*). STL export OK (watertight, −0.009 % volume at Medium).
- Watcher CLI copies: rel/k/main/y = 19555ca3, rel/k/main/z = 04:06 build (has the blend budget).

## Green 16cdcf6d (2026-10-27 03:25–03:55)
- Verified: LOFT-PROFILE-LIST (› entries, ↑↓, nothing under the field), DRAWING-SCALE (1:2, centre mark on the hole only).
- Reopened: OPEN-ERROR-SILENT (one-line message right, but the bubble goes away by itself ~6–9 s after launch).
- New: DRAFT-HANG (P0 core, fuzz seed 60 step 76; the app opens the doc rolled back above Draft 8, rolling forward hangs),
  LOFT-SKETCH-HIDE (P2 psui: only the first profile's sketch is hidden).
- REGEN-SPEED re-timed (above). Not yet in green: SKETCH-PASTE fix ac0645bc, P1-16 shell fix 239fa250.

## Verified on green dc3a03f6 (2026-10-27 01:55)
qa5's P1-17 (mass properties blank with nothing measured, no toast), P1-18 (⚠ badge on a missing-selection row), P2-D3
(drawing Note tool previews "NOTE" at the pointer), P1-8 (one "Vertex of Sketch N" chip per clicked hole point), P1-9
(feature-pattern chip "Ø 6.35 mm ↧ 12 mm"), P2-A5 (suppressed mate row italic + strike-through, green 19555ca3);
TAB-NO-PREVIEW (with X focus); API-DERIVED-BY-NAME; crash documents open with the watcher CLI 19555ca3 (app pending
the next green, bf8aaee2). EXT-DIRECTION still open with focus.

## qa5 items (fixed on main by qa5 after green 4d2f6b0c): all verified
P1-8, P1-9, P1-17, P1-18, P2-A5, P2-D3 on dc3a03f6 / 19555ca3 (above). On green 16cdcf6d (2026-10-27 04:05, asm5.noshape:
plate with 4 holes, pin, Cylindrical → Revolute, linear pattern): P1-1 ("Features (9)" with a new sketch on Top open),
P2-A2 (1st Enter in a mate limit applies "30 deg" and keeps the dialog, 2nd accepts), P2-A3 (Esc in explode editing = Done:
toolbar back, unexploded, plain row; double-click re-enters exploded), P2-A4 (pattern copy stays drawn, no z-fighting, while
the seed's mate is edited and retyped), P2-A6 ("Instances (3)" = Part 1, Part 2, Linear pattern 1), P2-A7 (pending
"Replicate 1" row with red dot at open, mate auto-filled), P2-A8 (no "instance(s) created" bubble), P2-D2 (assembly drawing
Update stays dim after a sketch in an unused Part Studio 2; lights up after the plate's depth changes).

## Next
Revolve / patterns / mirror / boolean on the new plate; feature-list reorder/delete/rollback on the 30-feature part;
selection, measure, section view; then P2 assembly and drawing passes; verify the list above once green moves.
