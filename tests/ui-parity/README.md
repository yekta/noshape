# Interaction parity suite (docs/PARITY-SUITES.md §2)

The same scripted user steps run in live Onshape and in noshape; after every step each side writes the same state,
and a runner compares them and names the first step that differs. Agreed 11-03 by qa (scenarios), live (live side)
and psui (noshape side); this file is the reference, the drafts it came from are qa's
/root/qa-scratch/ui-parity/README-draft.md and live's /root/parity/ui/STATE-FORMAT.md.

Layout:
- `tests/ui-parity/<name>.json`: a scenario (qa writes them; anyone may add one: every difference noticed becomes a
  scenario first).
- `tests/ui-parity/starts.json`: the start documents the scenarios name.
- `tests/ui-parity/live/<name>.json`: live's recording (live's runner, /root/live-pw/uip/run.mjs).
- noshape's recording goes to a scratch file (`NOSHAPE_UI_STATE_OUT`), never into the repo.

## Scenario

```json
{
  "name": "extrude-depth-variable-click-once",
  "about": "one sentence: what the user does and what we watch",
  "area": "psui",
  "ticket": "VAR-AUTOCOMPLETE-ENTER",
  "source": "user 11-02 / qa finding / everyday",
  "start": "vars",
  "steps": [
    {"do": "dblclick", "on": {"feature": "Extrude 1"}},
    {"do": "click", "on": {"field": "Depth"}},
    {"do": "type", "text": "#wall"},
    {"do": "key", "key": "Enter"}
  ],
  "expect": [
    {"after": 2, "field": "Depth", "selection": "all", "live": "behaviours 'Focus, click and selection' A"}
  ]
}
```

`expect` is optional: what qa already knows of live, to sanity-check a recording. The comparison is always against
the live recording.

### start
A name from `starts.json`: a units setting and a feature list in FeatureScript terms (std's feature types and
parameter ids; sketches as shapes with corner / centre points in mm on a named plane), so live builds it over REST
and noshape through crates/ops. `"empty"` is a new Part Studio in mm.

An assembly start adds, after the features, `"assembly":
{"instances": [{"part": "Part 1", "fixed": true}, {"part": "Part 2", "at": [60, 0, 0]}], "mates": [{"type":
"Revolute", "name": "Revolute 1", "a": {"instance": "Part 2 <1>", "face": [65, 0, 15], "inference": "mid_axis"},
"b": {…}}]}` (positions in the document's unit, a face by a point on it); ours builds it (ops `ui_parity::build_start`)
as the Assembly tab "start <name> asm" (live's name) and opens on it. Its state's `features.rows` are the instance
list's rows (the assembly's own, Origin, instances, "Mate features (n)", its rows); `assembly` is `{"instances":
[{"name", "fixed", "transform"}], "mates": [{"name", "type", "status"}]}` as live's REST gives them (type
"REVOLUTE", status "OK"); a mate selected in the list is a `FEATURE` item.

### steps
- `{"do": "click" | "dblclick" | "rclick" | "hover", "on": TARGET}`; `hover` waits `ms` (default 1500); a click may
  carry `"mods": "shift" | "mod" | "alt"`, held during the click.
- `{"do": "type", "text": "..."}` types characters one key at a time.
- `{"do": "key", "key": "Enter"}`: one key or chord; `mod` is Ctrl on Linux / Windows and Cmd on macOS (`mod+z`,
  `shift+e`, `alt+c`). Key names: `Enter`, `Escape`, `Tab`, `Backspace`, `Delete`, `ArrowUp` / `Down` / `Left` /
  `Right`, `Space`, `F2`, single characters.
- `{"do": "drag", "from": TARGET, "to": TARGET}`.
- `{"do": "wait", "until": "regenerated" | "dialog" | "ms", "ms": 500}`. Each side also waits, after every step,
  until its regeneration is idle (at most 30 s), so timing never decides a result.

### TARGET (by name, never by pixel)
- `{"tool": "Extrude"}`: a toolbar tool by its tooltip name; `{"tool": "Fillet", "menu": "Chamfer"}` in a dropdown.
- `{"field": "Depth"}` (`"index": n` when a label repeats), `{"checkbox": "Symmetric"}`, `{"tab": "Remove"}`,
  `{"dropdown": "Blind", "pick": "Up to next"}`, `{"button": "ok" | "cancel" | "?"}`, a chip's ×
  `{"button": "x", "field": "Entities to fillet"}`, other buttons by tooltip, a row's eye
  `{"button": "eye", "feature": "Sketch 1"}`.
- `{"feature": "Extrude 1"}`, `{"part": "Part 1"}`, `{"rollback_bar": true}`, `{"rollback_to": "Extrude 1"}`; a row's top
  or bottom edge `{"feature": "Extrude 1", "edge": "top"}` (a drag's drop above or below it).
- `{"list_row": "#wall_thickness"}`: a row of the open autocomplete / dropdown, by its first text.
- `{"menu": "Suppress"}`: an item of the open context menu.
- `{"face": [x, y, z]}`, `{"edge": [x, y, z]}`, `{"vertex": [x, y, z]}`: the entity through that model point (mm),
  picked where it shows in the current view; `{"sketch_point": [x, y]}` on the active sketch plane (mm);
  `{"plane": "Top"}`, `{"origin": true}`, `{"empty": true}` (empty graphics area).
- `{"view_cube": "Top"}`, `{"view_cube": "Isometric"}`.

A target a side can't find makes that step `"ok": false` with `"error"`; the run goes on.

### Drawings (dx, 11-05)
A start may add `"drawing": {"part": "Part 1", "template": "ANSI_A_INCH.dwt", "views": "suggested" | "none"}`: Create
Drawing of that part with that template (the dialog's Suggested views or No views), and the run opens on that drawing
tab (live's name "Part 1 Drawing 1"); `"assembly": true` instead of `"part"`: Create Drawing of the start's assembly. Targets in a drawing:
- `{"sheet_point": [x, y]}`: a point of the sheet, mm from its lower-left corner (an ANSI A sheet is 279.4 × 215.9);
- `{"view": "Front"}`: a view (its middle), by the name its properties show;
- `{"view_point": {"view": "Top", "at": [x, y, z]}}`: model point (mm) where it shows in that view;
  `{"view_edge": {…}}` the same picking the model edge drawn there;
- `{"annotation": "1.575"}`: an annotation by its shown text; `{"doc_tab": "Part 1 Drawing 1"}`: a document tab;
- `{"tool": …}` the drawing toolbar's tools by tooltip, as in a Part Studio.

Its state adds `"drawing"` (`null` outside a drawing tab):
```json
 "drawing": { "tab": "Part 1 Drawing 1", "sheet": "Sheet1", "sheets": ["Sheet1"], "tool": "Dimension",
              "views": [{"name": "Front", "kind": "projected", "parent": "Top", "scale": "1:1"}],
              "annotations": [{"kind": "dimension", "text": "1.575"}, {"kind": "note", "text": "CHECK SIZE"}],
              "selection": [{"kind": "VIEW", "owner": "Front"}] }
```
- `tool`: the active drawing tool's tooltip name without its shortcut ("Diameter dimension", "2 point linear
  dimension"), `null` when none. `views`: sorted by name (a section "Section", a detail "Detail"); `kind` one of
  `base`, `projected`, `isometric`, `section`, `detail`, `auxiliary`, `broken-out`; `parent` the view it was projected
  or cut from; `scale` as the view's properties show it. `annotations`: the sheet's, sorted by kind then text, `kind`
  one of `dimension`, `note`, `centermark`, `centerline`, `callout`, `datum`, `gdt`, `table`; automatic centermarks
  are left out; a dimension's text is its value as live's recorder reads it (no Ø / R, a leading zero: "0.394" for
  "Ø.394"); a table's is `null` on live's side (it can't read cells), so the runner compares tables by kind there;
  a hole table isn't listed (live's exports have no entry for it, live 2134).
  `selection`: `VIEW` / `EDGE` / `ANNOTATION` items with the view or the annotation's text as `owner`.
- A drawing's dialogs and panels are the top-level `dialog`, in live's recorder's forms (live 2063): a modal (Create
  Drawing, Export Drawing) is `{"kind": "modal", "title", "fields": [{"label", "kind": "input" | "select" |
  "checkbox", "value"}], "buttons"}`, its label the label row's text ("File name View export rules"); a drawing panel
  (Insert view, Note, Section view) is `{"kind": "drawing-panel", "title", "text": [...]}`, every text it shows in
  reading order (a list's options too, at most 30).
- `selection` is what is drawn in selection colours: `VIEW` for a framed view (selected or hovered), `EDGE` for a view
  whose edges are drawn picked (a dimension tool's picked circle, or its lines once two are picked; a closed
  broken-out profile; the projection's parent). In a drawing the focus is `other` (live's drawing is an iframe),
  `field` in a panel's text box.
- Compared in a drawing state: `drawing` and the keys above but `selection`, `undo` and `assembly` (live's recorder
  reads the main page's, which a drawing's iframe doesn't change: 0 and false throughout). The start's assembly is
  "start asm" in tab names and panel texts on both sides ("start pin-block asm Copy 2" / "start
  pin-block-asm-drawing asm").

## Recording (both sides write this)

```json
{ "scenario": "extrude-depth-variable-click-once", "recorded": "2026-11-03T01:00:00Z",
  "app": { "name": "live" | "noshape", "version": "1.2xx | 0.4.23 (sha)", "platform": "linux" },
  "start": { ...state... },
  "steps": [ { "step": 1, "do": {"do": "dblclick", "on": {"feature": "Extrude 1"}}, "ok": true, "state": { ...state... } } ] }
```
`start` is the state before step 1. live adds `"live": {"document", "element"}`.

### state
Every key always present (`null` / `[]` when there is nothing). Texts exactly as shown, trimmed, single spaces.
```json
{
 "focus":   { "kind": "field|dialog-name|filter|search-tools|feature-row|graphics|other|none",
              "label": "Depth", "text": "10 mm", "selection": [0, 5], "all": true },
 "dialog":  { "title": "Extrude 1", "kind": "feature|sketch|other",
              "tabs": [{"label": "Solid", "selected": true}, {"label": "Surface", "selected": false}],
              "fields": [
                {"label": "Depth", "kind": "number", "text": "10 mm", "state": "ok|error|disabled", "focused": true},
                {"label": "End type", "kind": "enum", "text": "Blind"},
                {"label": "Symmetric", "kind": "boolean", "checked": false},
                {"label": "Entities to extrude", "kind": "query", "items": ["Face of Sketch 1"], "active": true}
              ],
              "error": null },
 "list":    { "kind": "autocomplete|dropdown|context-menu|menu", "rows": ["#wall_thickness", "New variable…"],
              "highlighted": 0, "disabled": [] },
 "selection": { "count": 1, "items": [{"kind": "FACE|EDGE|VERTEX|BODY|FEATURE|SKETCH_ENTITY|PLANE", "owner": "Part 1"}] },
 "features": { "rows": [{"name": "Sketch 1", "state": "ok|info|warning|error|suppressed|rolled-back", "selected": false, "editing": false}],
               "rollbackAfter": "Extrude 1" },
 "parts":   ["Part 1"],
 "message": null,
 "tooltip": null
}
```
- `focus.selection`: the field's selected range [start, end] in characters (start = end: the caret); `all`: the
  whole text is selected. `other`: an input outside the dialogs (a sketch dimension's value box), no label.
- `dialog.fields`: only the visible ones, in dialog order; `label` as shown (a query box's placeholder).
- `list.rows`: each row's first text (`"#wall_thickness"`, not its value); `highlighted`: index into `rows`.
- `selection.items`: in selection order.
- `features.rows`: below "Default geometry" (origin and planes left out), as shown; `rollbackAfter`: the row the bar
  sits under.

Two keys added 11-03 (skasm, for the sketch scenarios; the runner compares each once a live recording has it):
```json
 "undo":   { "undo": true, "redo": false },
 "sketch": { "tool": "Line",
             "entities": ["construction line", "line", "line", "point"],
             "constraints": ["COINCIDENT", "COINCIDENT", "HORIZONTAL"],
             "dimensions": [{"kind": "LINEAR", "value": 30}, {"kind": "DIAMETER", "value": 12}] }
```
- `undo`: whether the toolbar's Undo and Redo are enabled (live: `[command-id=UNDO_A_CHANGE / REDO_A_CHANGE]` not
  disabled); in a sketch or dialog, its own history's.
- `sketch`: `null` unless a sketch is being edited. `tool`: the active sketch tool's tooltip name without its shortcut
  (`"Line"`, `"Corner rectangle"`, `"Dimension"`, `"Trim"`), `null` when none (select). `entities`: one entry per
  sketch entity as the sketch lists them (curve end and centre points aren't entities; a lone point is `"point"`):
  `line`, `circle`, `arc`, `ellipse`, `elliptical arc`, `spline`, `conic`, `point`, prefixed `construction ` when it
  is, sorted. `constraints`: one per relation, Onshape's type names (`COINCIDENT`, `HORIZONTAL`, `VERTICAL`,
  `PARALLEL`, `PERPENDICULAR`, `TANGENT`, `EQUAL`, `CONCENTRIC`, `MIDPOINT`, `SYMMETRIC` (REST `MIRROR`), `FIX`,
  `NORMAL`, `PIERCE`, …), sorted. `dimensions`: the driving ones as `{kind, value}`: `LINEAR` (a length or any
  distance, mm), `RADIUS` / `DIAMETER` (mm), `ANGLE` (degrees), values rounded to 6 decimals, sorted by kind then
  value.

Keys live's recorder added later (psui records them since 11-06; before, the runner skipped them, so they were never
compared):
- `dialog.ok`: the dialog's ✓, `"enabled"` / `"disabled"` (`null`: a dialog without one); `dialog.titleState`:
  `"ok"` / `"error"` (the red title).
- `dialogs`: every open dialog's state, the last one being `dialog`.
- `focus.dialog`: the title of the dialog the focused field is in (`null` outside dialogs); the Create variable popup
  is its own (`"Create variable"`).
- `partsHeading`: the parts list's heading as shown (`"Parts (2)"`).
- `list.detail`: each autocomplete row's whole text, its texts joined by single spaces (`"#width 30 mm"`,
  `"max( x , y )"`, `"New variable…"`).
- `sketch.dimensions[].expression`: the dimension's stored expression, `null` when its value was never typed; a typed
  plain number with the document unit (`"25 mm"`), a unitless expression wrapped (`"(2*15) mm"`). Before a plane is
  picked live's recorder can't read the sketch (`"error": "sketch model not reached"`, its lists `null`): not compared.
- Not compared: `focus.tag` and `features.rows[].cls` (live's DOM), `selection.items[].id` (live's entity ids).

## Coverage

`tools/ui-parity/coverage.py` (`--uncovered` lists what nothing exercises, `--markdown` prints the tables below)
counts the scenarios exercising each Part Studio toolbar tool, sketch tool, Part Studio and camera menu item
(crates/ops/src/coverage.rs) and General / Part Studio / 3D view / Sketch shortcut (the Keyboard shortcuts sheet). A
feature tool counts for a scenario that clicks it, presses its shortcut or double-clicks a feature of its type; a
shortcut that starts a tool counts for the tool too. Menu items labelled at run time ("Suppress Extrude 1") aren't in
the universe. Reported with the score (docs/parity/SCORE.md).

11-06, 369 scenarios (main had 311 in the morning; batch 1 adds Mate connector, Curve pattern, Boolean subtract /
intersect, Mid plane, Chamfer and 15 sketch tools and constraints; batch 2 the face, part, feature and empty-space
context menus; batch 3 selection, the feature list's filter, Fillet, Revolve, Shell and Three point plane):

| kind | covered | scenarios per item (median) |
|---|---|---|
| toolbar | 25/76 | 0 |
| sketch tool | 30/55 | 1 |
| menu | 26/117 | 0 |
| shortcut | 58/89 | 1 |

| most-used tool (lead 11-06) | scenarios |
|---|---|
| Extrude | 51 |
| Sketch | 107 |
| Fillet | 16 |
| Chamfer | 5 |
| Revolve | 5 |
| Shell | 5 |
| Hole | 6 |
| Linear pattern | 3 |
| Circular pattern | 2 |
| Curve pattern | 1 |
| Mirror | 4 |
| Boolean | 6 |
| Plane | 5 |
| Mate connector | 3 |
| sketch tools (all 55) | 120 scenarios, 30 tools covered |

## Comparison

`tools/ui-parity/run.py` compares the keys both sides record. A size that comes from where a click landed (a drawn
line's or circle's length in the sketch value box, or its dimension before a value is typed) is equal when within
10 %: each side's pointer is at whole pixels, so the same `sketch_point` steps draw lengths a pixel or two apart (a
10 mm circle is 9.53 mm in live). An assembly instance's transform (the `assembly` key) is equal within 0.5 mm and
0.002 per rotation entry, for the same reason after a drag; the instances are compared by name (live's REST order
isn't the list's). Live's root row reading "<name> Copy 1" (its runner opens a copy) is read as "<name>". Live's
menus lose the rows of cloud features noshape leaves out ("Add comment"); a menu that lost one is a row shorter, so it
may fit where live's had to move under the still pointer: its highlighted row isn't compared (face-rclick-menu).

## noshape side

`NOSHAPE_UI_SCRIPT=<scenario.json> NOSHAPE_UI_STATE_OUT=<recording.json> noshape` (crates/app/src/ui_script.rs;
starts from `starts.json` beside the scenario or `NOSHAPE_UI_STARTS`) builds the start document in a
scratch file, opens it, runs the steps through the window's real input path (keys and clicks dispatched to the
window, targets found by name), writes the recording and exits. The runner compares a recording with
`live/<name>.json` step by step and names the first step and key that differ; with no argument it runs every
scenario that has a live recording and prints the score (matched / total).

The whole suite in parallel shards (one Xvfb display each, ~4 minutes for 310 scenarios on a 16-core host):
`tools/ui-parity/shards.sh <noshape binary> <out dir> [shards] [first display]` (qa's daily score).
Run: `tools/ui-parity/run.py --app $CARGO_TARGET_DIR/debug/noshape --display :102 [names…]` (`--all` records every
scenario, `--compare-only` compares recordings already made). Named controls record their bounds through
app-core `ui_script::mark` (`tool:`, `field:`, `feature:`, `part:`, `tab:`, `checkbox:`, `dropdown:`, `list_row:`,
`menu:`, `button:`); geometry targets go through `PartStudioView::ui_target_point`, the state through
`PartStudioView::ui_state`. Under Xvfb without a window manager the runner focuses the window with xdotool (an
inactive window gets no focus events, so fields would not select their text on focus).
