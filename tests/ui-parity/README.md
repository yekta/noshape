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

A start's `"configuration"` (asm, 11-07): the Part Studio's configuration inputs, made after its features:
`[{"type": "list", "name": "Size", "rows": ["Small", "Large"]}]` (the first row the default; nothing configured by
them), as start `config-blocks`.

### remembered (psui, 11-06)

Onshape remembers a feature dialog's REMEMBER_PREVIOUS_VALUE parameters per account, saved on ✓ (behaviours 1987;
ours: `feature_settings`), so a Hole opens with what live's account last committed. A scenario whose recording
depends on that says what the account held, and our run starts from it (run.py writes it as the run's
`feature_settings.json`):

    "remembered": {"hole": {"": {"unitsSystem": "INCH", "isoHoleTableEx": ["Tapped", "Straight tap", "M6", "1.00 mm (Coarse)", "None"]},
                            "length:Millimeter": {"tappedDepthV3": "10 mm"}}}

Feature type → group ("" for values without units, "length:<unit>" / "angle:<unit>" per document unit) → parameter →
value: a list is a Lookup path, a bool a Bool, a string in a length / angle group (or a number) an expression, any
other string an Enum id.

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
- The move triad (asm, 11-07), shown at the pick once a part is clicked: `{"triad": "x" | "y" | "z"}` an arrow's shaft,
  `{"triad": "ring-z"}` the rotation ring at an arrow's tip, `{"triad": "origin"}` its centre (right-clicked for its
  menu: Align / Anti-align with Z, Rotate 90 / 180 deg, Move to origin).
- In an assembly: `{"instance": "Part 2 <1>"}` an instance list row by its text, else the one row starting with it and
  a space (`{"instance": "Assembly"}` for a new subassembly, "Assembly 2 <1>" in live's lane documents, "Assembly 1
  <1>" in ours: compared as one name, "Assembly N"): also the assembly's own row
  (`"start two-blocks asm"`), an instance folder (`"Folder 1"`) and `"Mate features"` (the folder, whatever its
  count); `{"mate": "Revolute 1"}` a row under Mate features (a mate, connector, relation, group or mate folder);
  `{"face": [x, y, z], "instance": "Part 2 <1>"}` a point in that instance's part coordinates.
- An assembly's right-hand panels (asm, 11-06): the strip's buttons by tooltip (`{"button": "Exploded views"}`,
  `"Named positions"`, `"BOM Table"`, `"Configuration panel"`), a panel's buttons by tooltip (`"Add exploded view"`,
  `"Add a new named position for this assembly"`), `{"panel_row": "Explode 1"}` a row of the open panel by its text
  (an exploded view, a named position), `{"panel_more": "Bill of materials"}` the open panel's ⋯; the assembly's
  Configuration panel takes the Part Studio's targets (`"Add configuration input"`, `config_more`).

A target a side can't find makes that step `"ok": false` with `"error"`; the run goes on.

### Configurations (psui, 11-06)

`{"button": "Configuration panel"}` (the right strip), `{"button": "Add configuration input"}` (Configure Part
Studio, or the panel's bottom button: by their shared tooltip), `{"config_more": "<input name>"}` an input's ⋯,
`{"config_more": ""}` the panel's own ⋯ (by Filter inputs), `{"config_row": "<row name>"}` a List input's row.

### Part Studio panels, picks and in context (psui, 11-07)

- `{"caret": "<button>"}`: the ▾ / ▲ beside a split button: `{"caret": "Add configuration input"}` (the ▲ of the
  configured panel's bottom button: List · Checkbox · Configuration variable · Paste inputs; the button itself adds a
  List input), `{"caret": "Insert and go to assembly"}` (the in-context banner's split button of a Part Studio created
  in context).
- `{"chip": n, "field": "<label>"}`: the n-th pick (from 0) in a selection field, as a target of its own (a right-click
  on it: Clear all · Clear missing references · Edit healthy moment).
- `{"panel_row": "<text>"}` in a Part Studio: a row of the open right panel by its text: the Appearance panel's
  (`"Part 1"`, `"1 face"`) and the Variable table's (by the variable's name, `"wall_thickness"`);
  `{"panel_more": "Variable table"}` the Variable table's ⋯.
- The appearance editor's colour fields: `{"field": "#"}`, `{"field": "R"}`, `{"field": "G"}`, `{"field": "B"}`.
- In context: `{"dropdown": "Context 1"}` the Assembly contexts section's selector (by the context it shows, or
  `"None"`), `{"context_more": "Context 1"}` its ⋯; an assembly's `{"instance": "Origin"}` is its Origin row.

### Drawings (dx, 11-05)
A start may add `"drawing": {"part": "Part 1", "template": "ANSI_A_INCH.dwt", "views": "suggested" | "none"}`: Create
Drawing of that part with that template (the dialog's Suggested views or No views), and the run opens on that drawing
tab (live's name "Part 1 Drawing 1"); `"assembly": true` instead of `"part"`: Create Drawing of the start's assembly. Targets in a drawing:
- `{"sheet_point": [x, y]}`: a point of the sheet, mm from its lower-left corner (an ANSI A sheet is 279.4 × 215.9);
- `{"view": "Front"}`: a view (its middle), by the name its properties show;
- `{"view_point": {"view": "Top", "at": [x, y, z]}}`: model point (mm) where it shows in that view;
  `{"view_edge": {…}}` the same picking the model edge drawn there;
- `{"annotation": "1.575"}`: an annotation by its shown text; `{"doc_tab": "Part 1 Drawing 1"}`: a document tab;
- `{"sheet_tab": "Sheet2"}`: a sheet by its name: the active sheet's label at the drawing's top left (inert, live2
  2180), or its row in the Sheets panel (Ctrl+S) when that is open (click, double-click, rclick);
- `{"break_line": {"view": "Front", "index": 0}}`: a break's first line (a model point there moves with the break);
- `"export": ["PDF", "DXF", "DWG"]` (a scenario key): after the last step each runner exports the drawing in those
  formats with the Export dialog's defaults (an entry `{"format": "DWG", "version": "2018"}` sets the Version, as
  live's dialog shows its account's last one), beside its recording as `<scenario>.pdf` / `.dxf` / `.dwg`;
  `tools/drawing-export/compare.py <live dir> <ours dir>` compares them (export fidelity, lead 11-06);
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
  a hole table isn't listed (live's exports have no entry for it, live 2134); an inspection item is an
  `inspectionsymbol` of its own, with a `null` text (hidden or not). While a dialog tool's panel is open with nothing
  else active, `tool` is that tool's ("Weld symbol", "Hatch region", "Format painter", "Inspection item"), and with
  inspection items hidden "Hide inspection items" (the toolbar's pressed toggle).
  `selection`: `VIEW` / `EDGE` items with the view as `owner` (live's recorder can't read an annotation's selection,
  so neither side records one).
- A drawing's dialogs and panels are the top-level `dialog`, in live's recorder's forms (live 2063): a modal (Create
  Drawing, Export Drawing) is `{"kind": "modal", "title", "fields": [{"label", "kind": "input" | "select" |
  "checkbox", "value"}], "buttons"}`, its label the label row's text ("File name View export rules"); a drawing panel
  (Insert view, Note, Section view) is `{"kind": "drawing-panel", "title", "text": [...]}`, every text it shows in
  reading order (a list's options too, at most 30).
- `selection` is what is drawn in selection colours: `VIEW` for a framed view (selected or hovered), `EDGE` for a view
  whose edges are drawn picked (a dimension tool's picked circle, or a line once it is the second pick; the edge a
  datum is being placed on, or the last surface finish symbol was placed on; a closed broken-out profile; the
  projection's parent). In a drawing the focus is `other` (live's drawing is an iframe; its context menus are in it
  too, so a drawing's `list` is null while one is open), `field` in a panel's text box.
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

11-06, 399 scenarios (main had 311 in the morning; batch 1 adds Mate connector, Curve pattern, Boolean subtract /
intersect, Mid plane, Chamfer and 15 sketch tools and constraints; batch 2 the face, part, feature and empty-space
context menus; batch 3 selection, the feature list's filter, Fillet, Revolve, Shell and Three point plane; batch 4 14
more sketch tools: the dropdowns' members, sketch fillet / chamfer, mirror, patterns, split, text; batch 5 Hole's tabs
and sizes, the patterns' types and axes, Mirror's types, Point normal and Line angle planes, Extrude's Through all and
flip, the feature menu's Suppress / Unsuppress and Delete):

| kind | covered | scenarios per item (median) |
|---|---|---|
| toolbar | 76/76 | 1 |
| sketch tool | 55/55 | 1 |
| menu | 92/117 | 1 |
| shortcut | 78/89 | 1 |

| most-used tool (lead 11-06) | scenarios |
|---|---|
| Extrude | 58 |
| Sketch | 146 |
| Fillet | 16 |
| Chamfer | 6 |
| Revolve | 5 |
| Shell | 5 |
| Hole | 9 |
| Linear pattern | 5 |
| Circular pattern | 4 |
| Curve pattern | 1 |
| Mirror | 5 |
| Boolean | 6 |
| Plane | 7 |
| Mate connector | 4 |
| sketch tools (all 55) | 137 scenarios, 55 tools covered |

### Drawings
`tools/ui-parity/coverage_drawing.py` counts the drw-* scenarios exercising each drawing tool and drawing menu item
(the "drawing:" rows of crates/ops/src/coverage.rs), Drawing shortcut and export format, by area. 11-06: 16 scenarios
covered 12/58 tools, 2/119 menu items, 0/14 shortcuts, 0/10 formats; batch 3 (20 scenarios, the most-used gaps:
centermarks and centerlines, hole callout, datum, GD&T, surface finish, callout, auxiliary view, the Dimension
dropdown's other kinds, ordinate dimension, revision table, table, export formats, a view's and a note's context
menus, the tools' keys), batch 4 (8 sheet scenarios: the sheet_tab target, the sheet menu, Insert sheet, the sheet keys)
and batch 5 (14: the remaining dimension, centerline, GD&T and table tools, a note's, a view's and a table's menu
items), batch 6 (16: the empty sheet's, a view's Show/hide, a note's order and the tables' properties menus) and
batch 7 (12: a dimension's, a view's and a general table's cells' menus) give (11-06 22:46; drw-revision-table dropped:
live's free plan has no Revision table tool):

85 drw-* scenarios

| kind | covered |
|---|---|
| tool | 42/58 |
| menu | 32/119 |
| shortcut | 10/14 |
| export | 4/10 |

| area | covered |
|---|---|
| GD&T | 5/10 |
| dimensions | 16/20 |
| export | 4/10 |
| insert views | 4/5 |
| notes | 12/16 |
| other | 23/94 |
| section / detail / broken-out | 3/17 |
| sheets | 13/13 |
| tables | 8/16 |

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
