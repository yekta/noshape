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
- `{"feature": "Extrude 1"}`, `{"part": "Part 1"}`, `{"rollback_bar": true}`, `{"rollback_to": "Extrude 1"}`.
- `{"list_row": "#wall_thickness"}`: a row of the open autocomplete / dropdown, by its first text.
- `{"menu": "Suppress"}`: an item of the open context menu.
- `{"face": [x, y, z]}`, `{"edge": [x, y, z]}`, `{"vertex": [x, y, z]}`: the entity through that model point (mm),
  picked where it shows in the current view; `{"sketch_point": [x, y]}` on the active sketch plane (mm);
  `{"plane": "Top"}`, `{"origin": true}`, `{"empty": true}` (empty graphics area).
- `{"view_cube": "Top"}`, `{"view_cube": "Isometric"}`.

A target a side can't find makes that step `"ok": false` with `"error"`; the run goes on.

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
 "focus":   { "kind": "field|dialog-name|filter|search-tools|feature-row|graphics|none",
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
  whole text is selected.
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

## noshape side

`NOSHAPE_UI_SCRIPT=<scenario.json> NOSHAPE_UI_STATE_OUT=<recording.json> noshape` (crates/app/src/ui_script.rs;
starts from `starts.json` beside the scenario or `NOSHAPE_UI_STARTS`) builds the start document in a
scratch file, opens it, runs the steps through the window's real input path (keys and clicks dispatched to the
window, targets found by name), writes the recording and exits. The runner compares a recording with
`live/<name>.json` step by step and names the first step and key that differ; with no argument it runs every
scenario that has a live recording and prints the score (matched / total).

Run: `tools/ui-parity/run.py --app $CARGO_TARGET_DIR/debug/noshape --display :102 [names…]` (`--all` records every
scenario, `--compare-only` compares recordings already made). Named controls record their bounds through
app-core `ui_script::mark` (`tool:`, `field:`, `feature:`, `part:`, `tab:`, `checkbox:`, `dropdown:`, `list_row:`,
`menu:`, `button:`); geometry targets go through `PartStudioView::ui_target_point`, the state through
`PartStudioView::ui_state`. Under Xvfb without a window manager the runner focuses the window with xdotool (an
inactive window gets no focus events, so fields would not select their text on focus).
