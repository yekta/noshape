# Workflow: keyboard shortcuts, mouse navigation, view cube

Every entry of live's Shift+/ list (dom/keyboard-shortcuts.txt, 121 entries) that applies to a local app, plus the
mouse and view-cube behaviour from behaviours.md "View navigation" and "Keyboard shortcuts". Documents "qa5 advanced"
(Part Studio 3, Assembly 1) and "qa5 assembly" (drawing). See also docs/parity/navigation.md (nav agent).
Status: ✅ matches live, ✗ differs (finding #), ? not verified / live data missing, – not applicable (cloud).

**Last run: 2026-10-21 (session 13:37–14:29), qa5**, binary /tmp/qa5/noshape-1337.

## General

| Key | Live action | Ours | Status |
|---|---|---|---|
| Shift+/ | Keyboard shortcuts modal: tabs General, Part Studio, Assembly, 3D view, Sketch, Feature Studio, Drawing; search; legend | same (no ↗ pop-out) | ✅ |
| Enter / Shift+Enter / Esc | accept / accept & repeat / cancel | Plane: Shift+Enter commits Plane 2 and opens Plane 3 | ✅ |
| Space | clear selection | ✅ | ✅ |
| Delete / Backspace | delete selection | deletes a feature row (core F15 fixed) | ✅ |
| Ctrl+C / Ctrl+V | copy / paste | part copy → paste opens Derived (DOC3) | ✅ / ✗ DOC3 |
| Shift+C | Curve/surface analysis | opens (re-uses the last entity "Face of Fillet 1") | ✅ / ? |
| Shift+D | Dihedral analysis | opens | ✅ |
| Ctrl+Shift+F, Ctrl+U | FeatureScript search, Report a bug | not tested (cloud/FS) | – |
| a / q / k | Flip primary axis / Reorient secondary / Hide-show mate connectors (assembly) | not tested | ? |
| Shift+N | Rename selection | feature row: inline rename ✅; **part row: nothing** | ✗ KB1 |
| Ctrl+M | Mate connector | not tested in assembly | ? |
| [ | Measure | opens the Measure panel | ✅ |
| Ctrl+Space | Quick tab switching | not tested | ? |
| Ctrl+Y / Ctrl+Z | redo / undo | ✅ (earlier workflows) | ✅ |
| ` | Select other | list of faces/edges under the cursor | ✅ |
| s | Shortcut toolbar (Part Studio: Sketch, Extrude, Revolve, Sweep, Loft / Fillet, Chamfer, Hole, Boolean, Transform) | 5×2 toolbar at the cursor | ✅ |
| Alt+C | Search tools | ✅ | ✅ |
| Alt+T | Tab manager | opens the Tabs panel with thumbnails | ✅ |
| Ctrl+A | nothing in the Part Studio | nothing | ✅ |

## Part Studio / 3D view

| Key | Live | Ours | Status |
|---|---|---|---|
| Shift+E / Shift+F / Shift+W / Shift+S | Extrude / Fillet / Revolve / Sketch | Shift+E, Shift+F ✅ (earlier); Shift+S ✅ | ✅ (Shift+W ?) |
| Shift+H | hide/show sketches | no visible sketches to test | ? |
| ↑ ↓ on the rollback bar | move it | ✅ (core 4.5) | ✅ |
| Shift+1…7 | Front, Back, Left, Right, Top, Bottom, Isometric | all correct | ✅ |
| ← → ↑ ↓ | rotate 15°: from Front, Left brings Right in, Up brings Bottom in | same direction and step | ✅ |
| Shift+arrows / Ctrl+arrows / Ctrl+Shift+arrows | coarse / precision rotate / pan | Shift+← from Front → Right (90°), Ctrl+← a small step | ✅ / ? (angles) |
| Shift+P | hide all construction | planes hidden / back | ✅ |
| y / Shift+Y | hide / show selected parts | ✅ | ✅ |
| p | hide/show planes | ✅ | ✅ |
| Shift+I | Make isolate | Isolate dialog (Transparency / Component controls) | ✅ dialog; **planes stay visible** (live: other parts and planes/origin go translucent) | ✗ KB3 |
| Shift+T | Make transparent | "Make transparent" dialog | ✅ |
| Shift+V | Named views | panel "Named views": New view name, +, "No views created" | ✅; **Esc does not close it** (stays through several Esc + clicks) | ? KB2 |
| Shift+X | Section view | ✅ (see workflow-analysis AN3) | ✅ |
| Shift+R | view in high quality | not checked visually | ? |
| n | view normal to; second n flips | ✅ | ✅ |
| z / Shift+Z | zoom out / in about the view centre | ✅ | ✅ |
| f | zoom to fit | ✅ (a press during a view animation is ignored: workflow-surfaces) | ✅ / ? |
| w | zoom to window | not tested | ? |

## Sketch (all checked by the highlighted tool)

l Line ✅ · g Corner rectangle ✅ · r Centre point rectangle ✅ · c Centre point circle ✅ · a 3-point arc ✅ ·
d Dimension ✅ · e Equal ✅ · i Coincident ✅ · h Horizontal ✅ · v Vertical ✅ · b Parallel ✅ · t Tangent ✅ ·
x Extend ✅ · m Trim ✅ · o Offset ✅ · u Use ✅ · q Toggle construction ✅ (stays on, as a mode) · Shift+S Point ✅ ·
Shift+F Sketch fillet ✅ · Shift+O Concentric ✅ · Shift+Q Symmetric ✅. Not run: Shift+U, Shift+J, Shift+A,
Shift+M, Shift+K, Shift+L, Shift+G.

## Assembly

i Insert dialog ✅ · m Mate (Fastened 1 dialog) ✅ · h Show mates mode (yellow banner "Show mates mode: hover an
instance… [Done]") ✅ · Shift+S Snap mode (info bubble "Snap mode: click a face or mate connector on the instance to
move." + yellow [Done]) ✅ / ? (look) · j not tested.

## Drawing

d / n (workflow 3) ✅; p Projected view ✅; Shift+D, Shift+R, Ctrl+M, Shift+H all select the Dimension group ✅
(which variant is shown in the split button: ?); Home/End/Page Up/Page Down need several sheets: not run.

## Mouse and view cube

| Action | Live | Ours | Status |
|---|---|---|---|
| Right-drag 100 px left from Front | rotates about 55° (≈0.55°/px), Right face comes in | 50 px → ~45°, 100 px → 90°: **≈0.9°/px** | ✗ KB4 |
| Middle-drag | pan | pan | ✅ |
| Ctrl+right-drag | pan | pan | ✅ |
| View cube face hover | light-blue fill | ✅ | ✅ |
| View cube ▶ / ▲ | ? (step) | 15° per click | ? |
| Camera menu | Isometric … Section view… | same; "Turn off section view" while sectioned | ✅ / ? |

## Other observations in this run

- Documents list footer still "(0.1.0)" (core F27).
- Reopened drawing: title-block DRAWN name "ROOT" became "--" (D13, asb).

## Findings (2026-10-21)

| # | Finding | Repro | Owner |
|---|---|---|---|
| KB4 | Right-drag orbit is ~1.6× faster than Onshape (0.9°/px vs 0.55°/px) | Shift+1, right-drag 50 px left | perf / ui (nav) |
| KB3 | Isolate leaves the default planes visible (live hides planes/origin with the other parts) | Part 1, Shift+I | ui |
| KB1 | Shift+N does nothing on a selected part (works on feature rows) | select Part 1 in the parts list, Shift+N | psb |
| KB2 | Named views panel ignores Esc (verify live; Show dependencies / Appearance close on Esc) | Shift+V, Esc | ui |
| D13 | Drawing title block "DRAWN" name lost after closing and reopening the document | qa5 assembly drawing | asb |

## Re-run pass 3 (2026-10-21 15:15, /tmp/qa5/noshape-1448 = target-shared 14:48)

| Key | Ours | Status |
|---|---|---|
| Shift+N on a part row | nothing (feature rows rename inline ✅) | ✗ KB1 |
| Shift+I | Isolate dialog; planes stay fully visible | ✗ KB3 |
| Right-drag 100 px left from Front (Shift+1) | ends ≈ 85° (view cube almost Right) vs live ≈ 55° | ✗ KB4 |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| KB1 Shift+N part row | nothing | ✗ still |
| KB3 Isolate | default plane outlines fade (labels stay), Plane 1 stays opaque | ✗ still (partly fixed) |
| KB4 right-drag 100 px | ≈85° | ✗ still |

## KB4 settled (perf, 2026-10-21 17:57, same canvas size; /root/perf-scratch/kb4.sh, NOSHAPE_PERF `nav: rotate drag` log)
Onshape (webgl `onDrag`): 2π·3 / canvas diagonal per px. Ours in a 1919×1079 window (live's): graphics area **1673×973 =
live's canvas**; right-drag 100 px in 10 moves, held still, released: **55.80°** (live ≈55°). Same drag released while
moving (< 50 ms, inertia ×0.77/frame): 61.35°. Our document window opens at 1440×900 → 1194×794 → **75.32°** held,
≈83° with a quick xdotool release, which is qa5's ≈85°. So the rate is Onshape's; the difference was the window size
(plus release inertia). ✅ KB4 (no change). Remaining: Onshape turns inertia off while the interactive frame rate is
below 20 fps (`DETAIL_REDUCTION_THRESHOLD_FPS`), perf implementing.

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
| Row | Ours | Status |
|---|---|---|
| KB1 Shift+N on a part row | inline rename with the name selected | ✅ **fixed** |
| KB3 Isolate | planes now hidden (live hides them too, ui/pn-900); **other parts stay fully opaque** (live: translucent); **Default geometry rows not greyed** (live grey) | ✗ still (partly) |
| KB4 orbit rate | settled by perf 17:57 (rate = Onshape's per canvas diagonal) | ✅ |
