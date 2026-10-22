# noshape agent API: CLI and MCP server

Everything a person does in noshape can be done by a script or an AI agent through `noshape-cli`, which is
both a command line tool and an MCP (Model Context Protocol) server. Both are generated from one operation
registry (`crates/ops`), and every operation calls the same model code as the app's UI: the same feature
parameters, regeneration, sketch tools and document history. An edit made through the API is one undoable
history entry, described exactly as the UI describes it (`Part Studio 1 :: Insert feature : Extrude 1`).

## Quick start

`noshape-cli` ships in every release bundle next to the app (macOS: inside the app and loose in the dmg;
Linux tarball and Windows zip: next to `noshape` / `noshape.exe`). To build it yourself:
`OCCT_ROOT=/path/to/occt cargo build --release -p noshape-cli` (no GUI dependencies).

### Claude Code

```sh
# macOS (noshape.app in Applications)
claude mcp add noshape -- /Applications/noshape.app/Contents/MacOS/noshape-cli mcp
# Linux (the tarball unpacked to ~/noshape-<version>-linux-x86_64)
claude mcp add noshape -- ~/noshape-<version>-linux-x86_64/noshape-cli mcp
# Windows (the zip unpacked to C:\noshape-<version>-windows-x86_64)
claude mcp add noshape -- C:\noshape-<version>-windows-x86_64\noshape-cli.exe mcp
```

On macOS the downloaded app and CLI are ad-hoc signed, not notarized: clear the download flag once with
`xattr -dr com.apple.quarantine /Applications/noshape.app` (and `xattr -d com.apple.quarantine <path>` for a
loose `noshape-cli` copied from the dmg), else macOS refuses to run them.

or in a project's `.mcp.json` (any MCP client takes the same entry):

```json
{ "mcpServers": { "noshape": { "command": "/path/to/noshape-cli", "args": ["mcp"] } } }
```

Then ask for a part: *"Use noshape to model a 60 x 40 x 6 mm plate with four Ø5 corner holes in
/tmp/plate.noshape, export STEP and show me an image."* The server sends the agent short instructions on
how to use the tools (`crates/cli/src/mcp_instructions.md`).

### Command line

```sh
noshape-cli document create --path /tmp/plate.noshape
noshape-cli sketch create --doc /tmp/plate.noshape --plane plane:top
noshape-cli sketch draw --doc /tmp/plate.noshape --sketch "Sketch 1" --tool corner_rectangle --points '[[-30,-20],[30,20]]'
noshape-cli sketch get --doc /tmp/plate.noshape --sketch "Sketch 1"          # regions: [{"id": "region:…"}]
noshape-cli feature add --doc /tmp/plate.noshape --type extrude --params '{"entities":["region:…"],"depth":"6 mm"}'
noshape-cli export parts --doc /tmp/plate.noshape --format STEP --file /tmp/plate.step
noshape-cli render image --doc /tmp/plate.noshape --view isometric --file /tmp/plate.png
```

- Every command prints JSON. Failures print `{"error", "message", "param"}` and exit with status 2.
- Field values are JSON when they parse (numbers, arrays, objects, booleans) and strings otherwise;
  `--json '{...}'` passes all arguments at once.
- `noshape-cli call <tool> '<json>'` runs an operation by its MCP tool name.
- `noshape-cli batch file.jsonl` runs `{"tool": ..., "args": {...}}` lines in one session.
- `noshape-cli tools` prints the MCP tool list with schemas; `noshape-cli <group> <verb> --help` documents one.

## Concepts

- **Documents** are `.noshape` files holding every tab and the whole history. There is no Save: every edit
  is stored at once. Calls name a document with `doc` (a path), or use the current one (last created/opened).
- **Tabs** are named by name or id (`tab`, default: the first Part Studio).
- **Values** are expressions with units: `"25 mm"`, `"1 in"`, `"30 deg"`, `"#width / 2"`, `"2 * #t + 1 mm"`.
  A bare number takes the workspace unit (mm by default), as typing it into a dialog does. Reply lengths are
  in workspace units too (`units` in replies says which).
- **Selections are reference ids**, returned by the query and sketch tools, valid across calls and
  processes for as long as the UI's own selection would stay valid (they are persistent topological names):

  | id | entity |
  |---|---|
  | `origin`, `plane:top`, `plane:front`, `plane:right` | default geometry |
  | `sketch:F…`, `entity:F…:7`, `region:F…:0,14,…` | a sketch, a sketch entity, a sketch region |
  | `face:…`, `edge:…`, `vertex:…` | model topology (from `query_entities`) |
  | `body:F….0` | a part |
  | `fplane:F…`, `mc:F….0`, `feature:F…` | Plane feature, mate connector, feature |

- **Features** are given by type id and parameters exactly as their dialogs have them: `feature_types`
  lists the types (Extrude, Revolve, Sweep, Loft, Fillet, Chamfer, Hole, Shell, patterns, booleans, sheet
  metal, frames, surfaces, curves, custom FeatureScript features...), `feature_schema` gives a type's
  parameters (JSON schema with defaults, options and units). Parameters a dialog hides for the current
  values are ignored, as in the UI.
- **Regeneration**: every edit regenerates and the reply reports each feature's status. A feature that fails
  is still added (as in Onshape) and shows up in `feature_status` / `regeneration.failed` with the error
  message the feature list shows.
- **Sketches** are drawn with the sketch editor's tools on the clicks you give (`sketch_draw`), so they get
  the same inference snapping and automatic constraints as drawing by hand; constraints and dimensions use
  the constraint and dimension tools (`sketch_constrain`, `sketch_dimension`).

## Short example (MCP tool calls)

```text
document_create   {"path": "/tmp/plate.noshape"}
sketch_create     {"plane": "plane:top"}                                   -> sketch "F3a…"
sketch_draw       {"sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[-25,-15],[25,15]]}
sketch_draw       {"sketch": "Sketch 1", "tool": "circle", "points": [[0,0],[4,0]]}  -> created [{"id": "entity:F3a…:9", "type": "circle"}]
sketch_dimension  {"sketch": "Sketch 1", "kind": "diameter", "entities": ["entity:F3a…:9"], "value": "10 mm"}
sketch_get        {"sketch": "Sketch 1"}                                    -> regions [{"id": "region:…", "holes": 1}, ...]
feature_add       {"type": "extrude", "params": {"entities": ["region:…"], "depth": "10 mm"}}
query_entities    {"kind": "edges", "curve": "line"}                        -> edges with direction, length, ids
feature_add       {"type": "fillet", "params": {"entities": ["edge:…", ...], "radius": "3 mm"}}
query_parts       {}                                                        -> volume, bounding box, faces
export_parts      {"format": "STEP", "file": "/tmp/plate.step"}
render_image      {"view": "isometric", "file": "/tmp/plate.png"}           -> PNG (MCP image content)
```

This sequence is the end-to-end test `crates/ops/tests/e2e.rs`, which also builds a configured block
(`configurations`) and a pin mated into a block (`assembly_with_mates`).

## Operations

Every operation is listed with its fields in the [reference](#reference) at the end of this document, generated
from the registry (`noshape-cli docs`; `noshape-cli tools` gives the same as MCP JSON schemas). The groups:
document, tab, feature, sketch, variable, config, part, assembly, drawing, query, analysis, render, export,
import, history.

## Example session

<!-- EXAMPLE SESSION -->

## Guides

### Sketch edit tools

Each runs the sketch editor's tool on the clicks and values you give, as one "Edit : Sketch N":

```text
sketch_fillet     {"sketch": "Sketch 1", "entities": ["entity:F…:3", "entity:F…:5"], "radius": "4 mm"}   corner points; the first gets R4, the rest are equal to it
sketch_offset     {"sketch": "Sketch 1", "entities": ["entity:F…:9"], "distance": "2 mm", "toward": [60, 30]}
sketch_trim       {"sketch": "Sketch 1", "points": [[5, 0]]}                       the piece under the click goes
sketch_linear_pattern {"sketch": "Sketch 1", "entities": ["entity:F…:9"], "count": 3, "distance": "10 mm", "angle": "90 deg"}   -> pattern id (edit with `pattern`)
sketch_use        {"sketch": "Sketch 2", "references": ["face:…"]}                  the face's edges, linked to the model
sketch_dimension  {"sketch": "Sketch 3", "kind": "distance", "entities": ["entity:F…:2", "edge:…"], "value": "15 mm"}   model geometry is referenced directly
```

### Drawings

Drawings work in sheet millimeters from the sheet's lower-left corner. `drawing_get` lists each view's model
edges as drawn (lines with end points, circles and arcs with center and radius); clicks on them pick the edge,
clicks on a vertex or circle center pick the point, exactly as with the mouse (the drawing tools are the app's).

```text
drawing_create    {"source": "Part Studio 1", "template": "ISO A3"}       -> tab "Part 1 Drawing 1" with suggested views
drawing_get       {"drawing": "Part 1 Drawing 1"}                          -> views [{id, kind, scale, bounds, edges: [...]}]
drawing_dimension {"drawing": "Part 1 Drawing 1", "kind": "diameter", "picks": [[205.5, 160.0]], "place": [220, 175]}   -> text "Ø10"
drawing_dimension {"drawing": "Part 1 Drawing 1", "picks": [[180.0, 180.0]], "place": [180, 200]}                         -> text "80"
drawing_insert_view    {"drawing": "…", "source": "Part Studio 1", "orientation": "isometric", "position": [330, 230]}
drawing_projected_view {"drawing": "…", "parent": 3, "position": [330, 120]}
drawing_section_view   {"drawing": "…", "parent": 3, "point": [205.5, 160], "position": [300, 160]}           -> Section A-A (vertical line, the dialog's default)
drawing_detail_view    {"drawing": "…", "parent": 3, "center": [205.5, 160], "radius": 12, "position": [380, 60]}
drawing_centerline     {"drawing": "…", "kind": "edge_to_edge", "picks": [[190, 150], [190, 170]]}
drawing_view_table     {"drawing": "…", "kind": "bom", "view": 3, "position": [20, 280]}
```

### Analysis

`analysis_curvature` (minimum radius, inflections), `analysis_deviation` (between curve chains or surfaces),
`analysis_connection` (G0–G3 continuity at joints and shared edges), `analysis_dihedral` (angle along edges) and
`analysis_interference` (overlapping parts, with the volume) run the Analysis tools menu's computations.

### Workspace protection

`document_protect` turns on Onshape's workspace protection for a branch: edits to the model, tabs, settings,
undo/redo and restore fail with error code `protected`; versions, branches and merges still work.
`document_info` reports `protected`.

### Configurations

```text
config_add_input  {"kind": "list", "name": "Size"}                         -> input id
config_add_option {"input": "Size", "name": "Large"}
config_configure  {"feature": "Extrude 1", "param": "depth", "input": "Size"}
config_set_value  {"feature": "Extrude 1", "param": "depth", "configuration": "Large", "value": "30 mm"}
config_activate   {"values": {"Size": "Large"}}                            -> later queries, renders, exports use Large
```

The active configuration is view state, as in the app's Configurations box: it lasts for the session (the
MCP server, or one `batch`), and `feature_edit` edits the active configuration's configured values.

### Assemblies

```text
assembly_insert    {"source": "Part Studio 1", "part": "body:F….0"}        -> instance id
assembly_set_state {"instances": ["…"], "fixed": true}
assembly_entities  {"instance": "…", "kind": "faces", "surface": "cylinder"} -> face ids with axis, normal, area
assembly_mate      {"type": "revolute", "a": {"instance": "…", "entity": "face:…"}, "b": {"instance": "…", "entity": "face:…"}}
assembly_list      {}                                                        -> instances, mates, degrees of freedom
```

### Import

`import_files` runs the Import dialog: STEP, IGES, Parasolid, ACIS, JT, Rhino, native SolidWorks / Inventor /
NX / Solid Edge parts (and assembly zips), STL, OBJ, 3MF and glTF. Each file becomes a Part Studio with an Import
feature (`combine` puts them all in one); the files are kept as file tabs in "CAD Imports".

```text
import_files {"files": ["/tmp/bracket.step"]}      -> tabs [{"name": "bracket", "kind": "Part Studio"}, {"name": "bracket.step", "kind": "File"}], parts [...]
query_parts  {"tab": "bracket"}
```

### Variable Studios

The `variable_*` operations work on a Variable Studio when `tab` names one: `variable_create` adds the table's
last row and then types its cells (type, value, description), `variable_set` edits cells, `variable_move` and
`variable_delete` use the row menu. Each cell is one edit, as in the table.

```text
tab_create      {"kind": "variablestudio", "name": "Variables"}
variable_create {"tab": "Variables", "name": "width", "value": "40 mm", "description": "Plate width"}
variable_list   {"tab": "Variables"}      -> rows [{name: "#width", type: "Length", expression: "40 mm", value: "40 mm"}]
```

### Many sketch shapes at once

`sketch_draw` with `shapes` draws a list of shapes in one sketch edit; each shape is drawn exactly as its own
call would be (same inference and constraints), but the sketch is solved, regenerated and reported once:
hundreds of shapes take about as long as one call.

```text
sketch_draw {"sketch": "Sketch 1", "shapes": [{"tool": "circle", "points": [[0, 0], [3, 0]]}, {"tool": "circle", "points": [[10, 0], [13, 0]]}]}
```

## Live mode (the app is open)

When the noshape app has a document open, the CLI and the MCP server send their calls for that document to
the app: edits show in the window at once and land on the window's undo stack (Ctrl+Z undoes them), and
queries see the window's current state. Nothing to configure: open the same `.noshape` path
(`document_open` or `doc`) and the call goes to the app; with the app closed, the same calls edit the file.

The app serves each open document on a localhost TCP port and writes a discovery file
`<temp dir>/noshape-live/<hash>.json` (port, random token, pid); closing the document removes it. Only
`document_create` is refused for a document the app has open; `document_close` just stops sending calls to
the app.

## For developers

- Operations live in `crates/ops/src/ops/<group>.rs`; each is an `Op` (group, verb, summary, description,
  fields with JSON schemas, handler). Add one there and the CLI and MCP pick it up.
- Handlers call shared functions that the UI also calls (`noshape_ops::studio`, `docedit`, `analysis`,
  `export`, `session`, `noshape_scene::sketch_mode`). Never re-implement UI behaviour in an op: move it out of
  `crates/app` into one of these and call it from both.
- `crates/ops/src/coverage.rs` maps every UI command to its op; `crates/ops/tests/coverage.rs` fails when the
  UI gains a command with no entry, and the planned count may only go down.

## Reference

<!-- BEGIN GENERATED REFERENCE (noshape-cli docs) -->

142 operations in 18 groups. Field types are JSON types; lengths and angles take expressions with units.

- **ops**: [`ops_list`](#ops_list)
- **document**: [`document_create`](#document_create), [`document_open`](#document_open), [`document_info`](#document_info), [`document_close`](#document_close), [`document_protect`](#document_protect), [`document_unprotect`](#document_unprotect), [`document_set_units`](#document_set_units)
- **tab**: [`tab_create`](#tab_create), [`tab_rename`](#tab_rename), [`tab_duplicate`](#tab_duplicate), [`tab_delete`](#tab_delete)
- **feature**: [`feature_types`](#feature_types), [`feature_schema`](#feature_schema), [`feature_add`](#feature_add), [`feature_edit`](#feature_edit), [`feature_get`](#feature_get), [`feature_list`](#feature_list), [`feature_delete`](#feature_delete), [`feature_suppress`](#feature_suppress), [`feature_rollback`](#feature_rollback), [`feature_rename`](#feature_rename), [`feature_reorder`](#feature_reorder), [`feature_set_visibility`](#feature_set_visibility), [`feature_show_all`](#feature_show_all)
- **sketch**: [`sketch_create`](#sketch_create), [`sketch_draw`](#sketch_draw), [`sketch_constrain`](#sketch_constrain), [`sketch_dimension`](#sketch_dimension), [`sketch_set_dimension`](#sketch_set_dimension), [`sketch_delete`](#sketch_delete), [`sketch_set_construction`](#sketch_set_construction), [`sketch_get`](#sketch_get), [`sketch_trim`](#sketch_trim), [`sketch_extend`](#sketch_extend), [`sketch_split`](#sketch_split), [`sketch_spline_point`](#sketch_spline_point), [`sketch_fillet`](#sketch_fillet), [`sketch_chamfer`](#sketch_chamfer), [`sketch_offset`](#sketch_offset), [`sketch_mirror`](#sketch_mirror), [`sketch_linear_pattern`](#sketch_linear_pattern), [`sketch_circular_pattern`](#sketch_circular_pattern), [`sketch_transform`](#sketch_transform), [`sketch_use`](#sketch_use), [`sketch_intersection`](#sketch_intersection), [`sketch_text`](#sketch_text), [`sketch_insert_dxf`](#sketch_insert_dxf), [`sketch_insert_image`](#sketch_insert_image)
- **variable**: [`variable_list`](#variable_list), [`variable_create`](#variable_create), [`variable_set`](#variable_set), [`variable_delete`](#variable_delete), [`variable_move`](#variable_move)
- **config**: [`config_list`](#config_list), [`config_add_input`](#config_add_input), [`config_edit_input`](#config_edit_input), [`config_delete_input`](#config_delete_input), [`config_add_option`](#config_add_option), [`config_edit_option`](#config_edit_option), [`config_configure`](#config_configure), [`config_set_value`](#config_set_value), [`config_activate`](#config_activate), [`config_reorder_inputs`](#config_reorder_inputs)
- **assembly**: [`assembly_insert`](#assembly_insert), [`assembly_list`](#assembly_list), [`assembly_entities`](#assembly_entities), [`assembly_connectors`](#assembly_connectors), [`assembly_mate`](#assembly_mate), [`assembly_transform`](#assembly_transform), [`assembly_set_state`](#assembly_set_state), [`assembly_delete`](#assembly_delete), [`assembly_pattern`](#assembly_pattern), [`assembly_dissolve_pattern`](#assembly_dissolve_pattern), [`assembly_bom`](#assembly_bom), [`assembly_bom_settings`](#assembly_bom_settings), [`assembly_interference`](#assembly_interference), [`assembly_group`](#assembly_group), [`assembly_relation`](#assembly_relation), [`assembly_mate_connector`](#assembly_mate_connector), [`assembly_replace`](#assembly_replace), [`assembly_rename`](#assembly_rename), [`assembly_set_feature_state`](#assembly_set_feature_state), [`assembly_named_position`](#assembly_named_position), [`assembly_named_position_drivers`](#assembly_named_position_drivers), [`assembly_exploded_view`](#assembly_exploded_view), [`assembly_explode_step`](#assembly_explode_step)
- **drawing**: [`drawing_templates`](#drawing_templates), [`drawing_create`](#drawing_create), [`drawing_get`](#drawing_get), [`drawing_insert_view`](#drawing_insert_view), [`drawing_projected_view`](#drawing_projected_view), [`drawing_dimension`](#drawing_dimension), [`drawing_export`](#drawing_export), [`drawing_section_view`](#drawing_section_view), [`drawing_detail_view`](#drawing_detail_view), [`drawing_auxiliary_view`](#drawing_auxiliary_view), [`drawing_break_view`](#drawing_break_view), [`drawing_crop_view`](#drawing_crop_view), [`drawing_centerline`](#drawing_centerline), [`drawing_virtual_sharp`](#drawing_virtual_sharp), [`drawing_hole_callout`](#drawing_hole_callout), [`drawing_callout`](#drawing_callout), [`drawing_ordinate`](#drawing_ordinate), [`drawing_datum`](#drawing_datum), [`drawing_centermark`](#drawing_centermark), [`drawing_note`](#drawing_note), [`drawing_gdt`](#drawing_gdt), [`drawing_surface_finish`](#drawing_surface_finish), [`drawing_weld`](#drawing_weld), [`drawing_view_table`](#drawing_view_table)
- **part**: [`part_rename`](#part_rename), [`part_set_properties`](#part_set_properties), [`part_set_material`](#part_set_material), [`part_set_appearance`](#part_set_appearance), [`part_remove_appearance`](#part_remove_appearance), [`part_appearances`](#part_appearances)
- **material**: [`material_list`](#material_list)
- **query**: [`query_parts`](#query_parts), [`query_entities`](#query_entities), [`query_datums`](#query_datums), [`query_measure`](#query_measure), [`query_mass_properties`](#query_mass_properties)
- **analysis**: [`analysis_curvature`](#analysis_curvature), [`analysis_deviation`](#analysis_deviation), [`analysis_connection`](#analysis_connection), [`analysis_dihedral`](#analysis_dihedral), [`analysis_interference`](#analysis_interference)
- **render**: [`render_image`](#render_image)
- **export**: [`export_parts`](#export_parts)
- **import**: [`import_files`](#import_files)
- **featurestudio**: [`featurestudio_get`](#featurestudio_get), [`featurestudio_edit`](#featurestudio_edit), [`featurestudio_commit`](#featurestudio_commit)
- **history**: [`history_undo`](#history_undo), [`history_redo`](#history_redo), [`history_list`](#history_list), [`history_create_version`](#history_create_version), [`history_create_branch`](#history_create_branch), [`history_switch_branch`](#history_switch_branch), [`history_merge`](#history_merge), [`history_restore`](#history_restore)

### ops

#### ops_list

List every operation (tool) of the noshape API.

Returns each operation's tool name, CLI command and summary, by group. Use `feature_types` and `feature_schema` for the Part Studio features and their parameters.

| field | type | | description |
|---|---|---|---|
| `group` | string |  | Only this group (document, tab, feature, sketch, query, render, export, history...). |

### document

#### document_create

Create a new document file (edits the document (one undoable history entry)).

Creates a .noshape document (one SQLite file holding every tab and the whole edit history). Like a new Onshape document it has "Part Studio 1" and "Assembly 1". Becomes the current document. Every later edit is saved immediately as a microversion (there is no Save).

| field | type | | description |
|---|---|---|---|
| `path` | string | required | Where to create it, e.g. "/tmp/bracket.noshape". Must not exist. |
| `length_unit` | one of `mm`, `cm`, `m`, `in`, `ft`, `yd`, `um` |  | Workspace length unit (default mm): bare numbers in length fields use it. |
| `angle_unit` | one of `deg`, `rad` |  | Workspace angle unit (default deg). |

#### document_open

Open a document file and make it current.

Opens an existing .noshape file. Later calls without `doc` use it.

| field | type | | description |
|---|---|---|---|
| `path` | string | required | Path of the .noshape file. |

#### document_info

Describe a document: tabs, units, history state.

Lists the tabs in tab-bar order (id, name, kind), the workspace units, the branch, whether it is protected (workspace protection) or read-only, and whether undo/redo are available.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |

#### document_close

Close an open document.

Forgets the document in this session (its file stays). Edits are already saved.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |

#### document_protect

Turn on workspace protection for a branch (edits the document (one undoable history entry)).

Versions and history › workspace row › Workspace protection… › Enable workspace protection. A protected workspace takes no direct model edits (geometry, drawings, tabs, settings, undo/redo, restore): those fail with error `protected`. Make changes on another branch and merge them in; versions, branches and merges stay allowed.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `branch` | string |  | Branch name or id (default: the current one). |

#### document_unprotect

Turn off workspace protection for a branch (edits the document (one undoable history entry)).

Clears Enable workspace protection: the workspace takes edits again.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `branch` | string |  | Branch name or id (default: the current one). |

#### document_set_units

Change the workspace units (edits the document (one undoable history entry)).

Document › Workspace units: the units bare numbers take and values are shown in.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `length_unit` | one of `mm`, `cm`, `m`, `in`, `ft`, `yd`, `um` |  | Length unit. |
| `angle_unit` | one of `deg`, `rad` |  | Angle unit. |
| `length_precision` | integer |  | Decimals shown for lengths. |
| `angle_precision` | integer |  | Decimals shown for angles. |

### tab

#### tab_create

Insert a new tab (Part Studio, Assembly, Drawing, Variable Studio, Feature Studio, Material Library) (edits the document (one undoable history entry)).

The tab bar's "+" menu. Gets the next default name ("Part Studio 2") unless `name` is given, and goes after `after` (default: the end).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `kind` | one of `partstudio`, `assembly`, `drawing`, `featurestudio`, `materiallibrary`, `variablestudio` | required | Tab kind. |
| `name` | string |  | Name (default: next default name). |
| `after` | string |  | Tab (name or id) to place the new tab after. |

#### tab_rename

Rename a tab (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string | required | Tab name or id. |
| `name` | string | required | New name. |

#### tab_duplicate

Duplicate a tab (edits the document (one undoable history entry)).

Tab › Duplicate: "<name> Copy 1" right after it.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string | required | Tab name or id. |

#### tab_delete

Delete a tab (edits the document (one undoable history entry); deletes or overwrites).

As in Onshape, undo does not bring it back (history restore does). Fails if it is the last tab. Other tabs that use it (assembly instances, drawings, derived parts) are listed in the reply; they will show errors.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string | required | Tab name or id. |

### feature

#### feature_types

List the Part Studio feature types.

Every feature of the Part Studio toolbar and tool search (Extrude, Revolve, Fillet, Hole, Sheet metal, Frames...), plus the document's custom FeatureScript features. Use the `type` id with `feature_add`.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `category` | string |  | Only this category: Sketch, Solid (Create), Detail, Pattern, Transform, Direct, Surface, Curve, Reference, SheetMetal, Frame, Variable, Import, Custom. |

#### feature_schema

The parameters of a feature type (JSON schema, defaults, options, units).

The same parameters, defaults, options and visibility rules as the feature's dialog. Parameters shown only for some values of others (e.g. `depth` only when `endBound` is BLIND) are ignored while hidden.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `type` | string | required | Feature type id, e.g. "extrude". |

#### feature_add

Add a feature to a Part Studio (what accepting its dialog does) (edits the document (one undoable history entry)).

Creates the feature with the dialog's defaults, applies `params`, inserts it at the rollback bar (or `index`) and regenerates. As in Onshape, a feature that fails is still added and shown with its error: check `feature_status` and `failed` in the reply. Sketches are made with `sketch_create`.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `type` | string | required | Feature type id (see `feature_types`). |
| `params` | object |  | Parameter values by parameter id (see `feature_schema`): booleans; enum option ids ("BLIND"); numbers as expression strings with units ("25 mm", "0.5 in", "30 deg", "#width / 2", "2 * #t + 1 mm"; a bare number uses the workspace units); selections as arrays of reference ids from the query and sketch tools ("region:…", "face:…", "edge:…", "plane:top", "body:…"); array parameters as arrays of objects of their item parameters. |
| `name` | string |  | Feature name (default: "Extrude 1", ...). |
| `index` | integer |  | Position in the feature list (default: at the rollback bar). |

#### feature_edit

Change a feature's parameters or name (what Edit and accepting its dialog does) (edits the document (one undoable history entry)).

Only the given parameters change. Configured parameters go to the active configuration's value (config_activate), as in the dialog.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `feature` | string | required | Feature id or name. |
| `params` | object |  | Parameter values by parameter id (see `feature_schema`): booleans; enum option ids ("BLIND"); numbers as expression strings with units ("25 mm", "0.5 in", "30 deg", "#width / 2", "2 * #t + 1 mm"; a bare number uses the workspace units); selections as arrays of reference ids from the query and sketch tools ("region:…", "face:…", "edge:…", "plane:top", "body:…"); array parameters as arrays of objects of their item parameters. |
| `name` | string |  | New name. |

#### feature_get

A feature's type, parameters and regeneration state.

Parameters in the same JSON forms `feature_add` takes; `visible` lists the ones its dialog currently shows.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `feature` | string | required | Feature id or name. |

#### feature_list

The feature list: every feature with its status, the parts, failures.

Regenerates the Part Studio and reports each feature (id, name, type, status Ok/Info/Warning/Error/Suppressed/RolledBack, error message), the parts and the rollback bar.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |

#### feature_delete

Delete features (edits the document (one undoable history entry); deletes or overwrites).

Features that use them fail with "missing selection" errors, as in Onshape.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `features` | array of string | required | Feature ids or names. |

#### feature_suppress

Suppress or unsuppress features (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `features` | array of string | required | Feature ids or names. |
| `suppressed` | boolean |  | true: suppress, false: unsuppress. |

#### feature_rollback

Move the rollback bar (edits the document (one undoable history entry)).

Give `after` (the bar goes below that feature), `index` (number of features before the bar) or neither (to the end). New features are inserted at the bar.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `after` | string |  | Feature id or name. |
| `index` | integer |  | Features above the bar. |

#### feature_rename

Rename a feature (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `feature` | string | required | Feature id or name. |
| `name` | string | required | New name. |

#### feature_reorder

Move features to another place in the list (edits the document (one undoable history entry)).

Like dragging rows in the feature list: refused when a feature would move above something it depends on.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `features` | array of string | required | Feature ids or names to move. |
| `to` | integer | required | Row index to drop them at. |

#### feature_set_visibility

Hide or show parts, sketches, planes, mate connectors or default geometry (edits the document (one undoable history entry)).

Targets are reference ids: `body:…` parts (or any face/edge of one), `sketch:…`, `fplane:…`, `mc:…`, `feature:…` (a feature that made parts hides its parts), `plane:top|front|right`, `origin`.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `targets` | array of string | required | Reference ids. |
| `hidden` | boolean |  | true: hide, false: show. |

#### feature_show_all

Show all parts, sketches, planes and default geometry (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |

### sketch

#### sketch_create

Create a sketch on a plane or planar face (edits the document (one undoable history entry)).

Inserts a Sketch feature ("Insert feature : Sketch 1") at the rollback bar. Returns its id and plane frame: sketch coordinates (x, y) map to world = origin + x*xdir + y*ydir. On `plane:top` x = world X, y = world Y; on `plane:front` x = X, y = Z; on `plane:right` x = Y, y = Z.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `plane` | string | required | Plane: `plane:top`, `plane:front`, `plane:right`, a Plane feature (`fplane:…`), a planar face id or a mate connector. |
| `name` | string |  | Name (default "Sketch N"). |

#### sketch_draw

Draw sketch geometry with a sketch tool (line, rectangle, circle, arc, spline, slot, polygon, ellipse, point...) (edits the document (one undoable history entry)).

Runs the sketch editor's drawing tool on the clicks you give, exactly as clicking in the sketch: inference snaps to existing points/curves within 9 µm and adds the same automatic constraints (coincident, horizontal, vertical, tangent...). Coordinates are sketch-plane (x, y) in workspace units, or length expressions ("10 mm"). One call = one edit of the sketch ("Edit : Sketch 1"); `shapes` draws many shapes in that one edit. Returns the created entity ids (`entity:…`), the sketch's solve state and its regions (`region:…` ids for Extrude etc.). Tools:
- line: Clicks chain segments: [p0, p1, p2...] makes p0-p1, p1-p2...; repeat the first point to close the loop.
- midpoint_line: [midpoint, end].
- corner_rectangle: [corner, opposite corner]. Horizontal/vertical and coincident constraints, like the UI.
- center_rectangle: [center, corner].
- aligned_rectangle: [corner, second corner (edge direction), third point (width)].
- circle: [center, point on the circle].
- three_point_circle: [p1, p2, p3] on the circle.
- three_point_arc: [start, end, point on the arc].
- tangent_arc: [start at the end of a curve, end].
- center_point_arc: [center, start, end] (counter-clockwise).
- point: One click per point.
- spline: Fit spline through the clicked points (2 or more).
- bezier: Bezier through control points.
- conic: [start, end, control].
- slot: Click on an existing line/arc (its centerline), then a point setting the width.
- inscribed_polygon: [center, vertex]; `sides` sets the number of sides (default 6).
- circumscribed_polygon: [center, edge midpoint]; `sides` as above.
- ellipse: [center, major axis end, point setting the minor radius].
- elliptical_arc: [center, major axis end, start point (also sets the minor radius), end point] (counter-clockwise).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `tool` | one of `line`, `midpoint_line`, `corner_rectangle`, `center_rectangle`, `aligned_rectangle`, `circle`, `three_point_circle`, `three_point_arc`, `tangent_arc`, `center_point_arc`, `point`, `spline`, `bezier`, `conic`, `slot`, `inscribed_polygon`, `circumscribed_polygon`, `ellipse`, `elliptical_arc` |  | Drawing tool (required without `shapes`). |
| `points` | array of array of number or string |  | The clicks, in order: [[x, y], ...] (required without `shapes`). |
| `shapes` | array of object |  | Many shapes in one edit, each drawn exactly as its own call would (instead of tool / points / sides / construction). Much faster for many shapes. |
| `construction` | boolean |  | Make the new geometry construction geometry. |
| `sides` | integer |  | Polygon sides. |

#### sketch_constrain

Add a geometric constraint between sketch entities (edits the document (one undoable history entry)).

Selects the entities and applies the constraint tool, as in the UI (select-first). Entity ids come from `sketch_draw` / `sketch_get` (`entity:…`; curves, or their points). Constraints:
- coincident: points/curves coincide (point on curve, collinear lines, same circle)
- concentric: centers of circles/arcs coincide
- parallel: lines parallel
- tangent: curves tangent
- horizontal: lines horizontal, or two points level
- vertical: lines vertical, or two points aligned vertically
- midpoint: a point at the midpoint of a line or arc
- perpendicular: lines perpendicular
- equal: equal lengths / radii
- symmetric: two entities symmetric about a line (the last entity)
- fix: fix in place
- normal: line normal to a curve
- pierce: point pierces external geometry
- curvature: curvature continuity at a joint

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `constraint` | one of `coincident`, `concentric`, `parallel`, `tangent`, `horizontal`, `vertical`, `midpoint`, `perpendicular`, `equal`, `symmetric`, `fix`, `normal`, `pierce`, `curvature` | required | Constraint. |
| `entities` | array of string | required | Sketch entity ids, or the origin (`origin`) to constrain to it. |

#### sketch_dimension

Add a driving dimension (value or expression) to sketch entities (edits the document (one undoable history entry)).

The Dimension tool: placed as a reference, made driving unless that over-constrains the sketch, then set to `value` when given ("20 mm", "#width", "45 deg"; a bare number uses workspace units). Kinds: auto (what the Dimension tool infers from the entities and `label` position), distance (point/line/circle to point/line/circle; `mode` center|minimum|maximum for circles), horizontal_distance, vertical_distance, length (line or arc), radius, diameter, angle (two lines), arc_angle, centerline (twice the distance to a line: revolve diameters).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | One or two sketch entity ids (`entity:…`) or `origin`. |
| `value` | string or number |  | Value or expression. Without it the dimension keeps the measured value, as when the Dimension tool's value box is accepted unchanged. |
| `kind` | one of `auto`, `distance`, `horizontal_distance`, `vertical_distance`, `length`, `radius`, `diameter`, `angle`, `arc_angle`, `centerline` |  | Dimension kind (default auto). |
| `mode` | one of `center`, `minimum`, `maximum` |  | Distance to a circle/arc measured to its center (default), nearest or farthest point. |
| `label` | array of number or string |  | Label position in sketch coordinates (also picks the auto kind: above/below two points gives a horizontal distance, beside them a vertical one). |

#### sketch_set_dimension

Change a sketch dimension's value or expression (edits the document (one undoable history entry)).

Like double-clicking a dimension and typing a value. Negative distances and angles flip the geometry, as in Onshape.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `dimension` | string | required | Dimension id (`dim:…` from sketch_get / sketch_dimension). |
| `value` | string or number | required | Value or expression. |

#### sketch_delete

Delete sketch entities, constraints or dimensions (edits the document (one undoable history entry); deletes or overwrites).

Deleting a curve deletes its constraints; its points stay when other curves use them.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string |  | Entity ids. |
| `constraints` | array of string |  | Constraint or dimension ids (`con:…`, `dim:…`). |

#### sketch_set_construction

Toggle construction geometry (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Entity ids. |
| `construction` | boolean |  | true: construction. |

#### sketch_get

A sketch's entities, constraints, dimensions, solve state and regions.

Entities with ids and geometry (sketch coordinates, workspace units), constraints and dimensions (`dim:…`, value and expression), DOF and status (fully defined, under defined, over defined), regions (`region:…` ids, area) for Extrude / Revolve, and the plane frame.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |

#### sketch_trim

Trim sketch curves (the Trim tool) (edits the document (one undoable history entry); deletes or overwrites).

Each click in `points` removes the piece of the curve under it, up to the nearest intersections (a curve crossing nothing is deleted; a lone point is deleted). With `path`, the pointer is dragged along the points instead and every curve the path crosses is trimmed. Give `entities` (same length as `points`) to say which curve each click is on; otherwise the curve within 6 µm of the click is used.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `points` | array of array of number or string |  | Clicks on the pieces to remove, in sketch coordinates. |
| `entities` | array of string |  | The curve under each click (optional). |
| `path` | array of array of number or string |  | Drag path (2 or more points): trims every curve it crosses. |

#### sketch_extend

Extend lines or arcs to the next curve (the Extend tool) (edits the document (one undoable history entry)).

Each click extends the line or arc under it, from its end nearest the click, to the next curve it meets. `entities` (optional, same length) names the curve under each click.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `points` | array of array of number or string | required | Clicks near the end to extend. |
| `entities` | array of string |  | The curve under each click (optional). |

#### sketch_split

Split sketch curves at points (the Split tool) (edits the document (one undoable history entry); deletes or overwrites).

Each click splits the curve under it at the closest point on the curve. `entities` (optional, same length) names the curve under each click.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `points` | array of array of number or string | required | Split positions. |
| `entities` | array of string |  | The curve under each click (optional). |

#### sketch_spline_point

Add a control point to a spline or bezier (edits the document (one undoable history entry)).

The Spline control point tool: inserts a point into the spline / bezier at the click.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entity` | string | required | Spline or bezier entity id. |
| `point` | array of number or string | required | Where on the curve (or its control polygon). |

#### sketch_fillet

Fillet sketch corners (the Sketch fillet tool) (edits the document (one undoable history entry)).

Each corner point in `entities` is filleted; two curves given one after the other are filleted where they meet (the tool's two-curve mode). All fillets of one call are one tool session: the first gets a radius dimension set to `radius` (a value or expression), later ones are constrained equal to it, as in Onshape.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Corner points (entity ids), or pairs of curves. |
| `radius` | string or number |  | Radius (default: the tool's, 20 mm / 0.5 in). |

#### sketch_chamfer

Chamfer sketch corners (the Sketch chamfer tool) (edits the document (one undoable history entry)).

Like sketch_fillet, with an equal-distance chamfer: each corner point (or pair of curves) gets a chamfer line, the first with distance dimensions set to `distance`.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Corner points, or pairs of curves. |
| `distance` | string or number |  | Chamfer distance (default: the tool's, 5 mm / 0.25 in). |

#### sketch_offset

Offset sketch curves (the Offset tool) (edits the document (one undoable history entry)).

Offsets the curves (a connected chain offsets as one) by `distance`. The default side is the one the tool's arrow shows first; `toward` picks the side whose offset passes nearer that point, `flip` flips the side.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Curves to offset. |
| `distance` | string or number |  | Offset distance (default: the tool's, 5 mm / 0.25 in). |
| `toward` | array of number or string |  | A point on the side to offset to. |
| `flip` | boolean |  | Offset to the other side. |

#### sketch_mirror

Mirror sketch entities about a line (the Mirror tool) (edits the document (one undoable history entry)).

Copies the entities mirrored about `axis` and adds symmetric constraints. The axis is a sketch line, or model geometry (an edge, a plane) whose projection is used.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Entities to mirror. |
| `axis` | string | required | Mirror line: a sketch line id, or an edge / plane id. |

#### sketch_linear_pattern

Pattern sketch entities in a line or grid (the sketch Linear pattern tool) (edits the document (one undoable history entry)).

Copies the entities `count` times (the seed included) `distance` apart along `angle`, optionally in a second direction (`second_count`, `second_distance`, `second_angle`, default 90 deg). Counts are 1 to 100. Give `pattern` (an id from the reply or sketch_get) to edit an existing pattern instead.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string |  | Seed entities (not needed when editing). |
| `count` | integer |  | Instances in the first direction (default 3). |
| `distance` | string or number |  | Spacing (default 25 mm / 1 in). |
| `angle` | string or number |  | First direction (default 0 deg: +x). |
| `second_count` | integer |  | Instances in the second direction (default 1). |
| `second_distance` | string or number |  | Second spacing. |
| `second_angle` | string or number |  | Second direction (default 90 deg). |
| `pattern` | integer |  | Pattern id to edit. |

#### sketch_circular_pattern

Pattern sketch entities around a center (the sketch Circular pattern tool) (edits the document (one undoable history entry)).

Copies the entities around `center`: `count` instances (the seed included, 2 to 100) over `angle` (default 360 deg, spread evenly; a partial angle puts the last instance at the angle). Give `pattern` to edit an existing pattern.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string |  | Seed entities (not needed when editing). |
| `center` | array or string |  | Center: [x, y], a point entity id or `origin` (default: the origin). |
| `count` | integer |  | Instances (default 3). |
| `angle` | string or number |  | Total angle (default 360 deg). |
| `pattern` | integer |  | Pattern id to edit. |

#### sketch_transform

Move, rotate or scale sketch entities (the Transform tool) (edits the document (one undoable history entry)).

Rotates by `rotate` and scales by `scale` about `center` (default: the selection's center), then translates by `translate`, as the tool's triad does.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Entities to transform. |
| `translate` | array of number or string |  | Translation [dx, dy]. |
| `rotate` | string or number |  | Rotation angle (counter-clockwise). |
| `scale` | number |  | Scale factor. |
| `center` | array of number or string |  | Pivot [x, y]. |

#### sketch_use

Use (project) model geometry into the sketch (the Use tool) (edits the document (one undoable history entry)).

Projects each reference onto the sketch plane as fixed geometry linked to the model (it follows model changes): edges and vertices (`edge:…`, `vertex:…` from query_entities), other sketches' entities, a planar face (all its edges), a curved face (its silhouettes) or a plane (its trace line). Returns the created entity ids.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `references` | array of string | required | Model reference ids. |

#### sketch_intersection

Add a face's intersection with the sketch plane (the Intersection tool) (edits the document (one undoable history entry)).

Sections the face with the sketch plane; the curves stay linked to the face.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `face` | string | required | Face id (`face:…`) crossing the sketch plane. |

#### sketch_text

Add or edit sketch text (the Text tool) (edits the document (one undoable history entry)).

Places a text box with its lower-left corner at `corner` and `height` (default 10 mm), with the Text dialog's options; or, with `edit` (an entity of an existing text), changes that text. Text is limited to 250 characters.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `text` | string |  | The text (default "Default text" for a new box). |
| `corner` | array of number or string |  | Lower-left corner (new text). |
| `height` | string or number |  | Height of the first line (new text). |
| `font` | string |  | Font family. |
| `bold` | boolean |  | Bold. |
| `italic` | boolean |  | Italic. |
| `flip_horizontal` | boolean |  | Mirror horizontally. |
| `flip_vertical` | boolean |  | Mirror vertically. |
| `expression` | boolean |  | Convert to expression (`#var` in the text shows its value). |
| `edit` | string |  | An entity id of the text to edit. |

#### sketch_insert_dxf

Import a DXF or DWG drawing into the sketch (Insert DXF or DWG) (edits the document (one undoable history entry)).

Adds the drawing's geometry, in the file's units unless `units` is given, centered on the sketch origin unless `use_origin`.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `file` | string | required | Path of the .dxf / .dwg file. |
| `units` | one of `From file`, `Millimeter`, `Centimeter`, `Meter`, `Inch`, `Foot`, `Yard` |  | Drawing units (default "From file"; millimeters when the file has none). |
| `use_origin` | boolean |  | Keep the file's origin at the sketch origin. |

#### sketch_insert_image

Import an image into the sketch (Insert image) (edits the document (one undoable history entry)).

Places a PNG, JPEG, GIF or BMP image in the rectangle from the first corner toward the second (the aspect ratio is kept, as when drawing the rectangle).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `file` | string | required | Path of the image. |
| `points` | array of array of number or string | required | [corner, opposite corner]. |

### variable

#### variable_list

The variables of a Part Studio or a Variable Studio, with their values.

Part Studio: Variable features (#name) in feature order with kind, expression, evaluated value and description, plus the values of every variable visible at the end of the feature list (including Variable Studio and configuration variables). Variable Studio (`tab` names it): its table rows in order (name, type, expression, value, description).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Variable Studio tab, by name or id. Default: the first Part Studio. |

#### variable_create

Create a variable (#name) in a Part Studio or a Variable Studio (edits the document (one undoable history entry)).

Part Studio: the Variable table's add row, a Variable feature at the rollback bar. Variable Studio: the table's last row ("Add variable", a Length variable), then its type, value and description cells, one edit each, as typed in the table. Use it in any expression as #name.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Variable Studio tab, by name or id. Default: the first Part Studio. |
| `name` | string | required | Identifier: a letter or underscore, then letters, digits or underscores. |
| `value` | string or number | required | Expression ("20 mm", "#a * 2"); a bare number takes the workspace unit. |
| `kind` | one of `LENGTH`, `ANGLE`, `NUMBER`, `ANY` |  | LENGTH (default), ANGLE, NUMBER or ANY. |
| `description` | string |  | Description. |

#### variable_set

Change a variable's value, kind, name or description (edits the document (one undoable history entry)).

Part Studio: renaming updates every expression that uses the variable, as the panel does. Variable Studio: each given field is one edit of its table cell ("Change variable type", "Edit variable", "Edit variable description", "Rename variable").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Variable Studio tab, by name or id. Default: the first Part Studio. |
| `variable` | string | required | Variable name (with or without #) or its feature id. |
| `value` | string or number |  | New expression. |
| `kind` | one of `LENGTH`, `ANGLE`, `NUMBER`, `ANY` |  | New kind. |
| `name` | string |  | New name. |
| `description` | string |  | New description. |

#### variable_delete

Delete a variable (edits the document (one undoable history entry); deletes or overwrites).

Expressions that use it fail to regenerate, as in Onshape.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Variable Studio tab, by name or id. Default: the first Part Studio. |
| `variable` | string | required | Variable name or feature id. |

#### variable_move

Move a variable up or down in the Variable table or a Variable Studio (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Variable Studio tab, by name or id. Default: the first Part Studio. |
| `variable` | string | required | Variable name or feature id. |
| `by` | integer | required | -1: up, 1: down. |

### config

#### config_list

The Part Studio's configuration inputs, configurations and configured parameters.

Inputs (List with its configurations, Checkbox, Variable with default and range) with ids, and every configured parameter with its value per configuration.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |

#### config_add_input

Add a configuration input (List, Checkbox or configuration Variable) (edits the document (one undoable history entry)).

"Configure Part Studio" › List / Checkbox / Variable. A List starts with one configuration "Default" (add more with config_add_option). A Variable input is a #variable whose value each configuration can set.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `kind` | one of `list`, `checkbox`, `variable` | required | Input kind. |
| `name` | string |  | Name (default "Configuration", "List 2", "Checkbox 1"...; required for a variable: its #name). |
| `variable_kind` | one of `LENGTH`, `ANGLE`, `INTEGER`, `REAL`, `TEXT` |  | Variable: kind (default LENGTH). |
| `value` | string or number |  | Variable: default value expression. |
| `range` | array of string |  | Variable: [min, max] expressions. |

#### config_edit_input

Rename a configuration input or change a configuration variable's default and range (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `input` | string | required | Input id or name. |
| `name` | string |  | New name. |
| `value` | string or number |  | Variable input: new default. |
| `range` | array of string |  | Variable input: new [min, max] expressions (with `value`). |

#### config_delete_input

Delete a configuration input and what it configures (edits the document (one undoable history entry); deletes or overwrites).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `input` | string | required | Input id or name. |

#### config_add_option

Add a configuration to a List input (edits the document (one undoable history entry)).

Configured parameters take the default configuration's value in it until set with config_set_value.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `input` | string | required | List input id or name. |
| `name` | string | required | Configuration name (e.g. "Large"). |

#### config_edit_option

Rename or delete a configuration of a List input (edits the document (one undoable history entry)).

A List keeps at least one configuration.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `input` | string | required | List input id or name. |
| `option` | string | required | Configuration id or name. |
| `name` | string |  | New name. |
| `delete` | boolean |  | Delete it. |

#### config_configure

Make a feature parameter (or a feature's suppression) configured by a List or Checkbox input (edits the document (one undoable history entry)).

Every configuration starts with the current value; then set values with config_set_value. `param` is a parameter id of the feature (see feature_schema) or "suppressed".

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `feature` | string | required | Feature id or name. |
| `param` | string | required | Parameter id, or "suppressed". |
| `input` | string | required | List or Checkbox input id or name. |

#### config_set_value

Set a configured parameter's value in one configuration (edits the document (one undoable history entry)).

`configuration` is the List configuration's id or name, or true/false for a Checkbox. `value` uses the feature parameter's JSON form ("40 mm", an enum id, true/false...).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `feature` | string | required | Feature id or name. |
| `param` | string | required | Parameter id, or "suppressed". |
| `configuration` | string or boolean | required | Configuration id/name, or true/false. |
| `value` | any | required | The value. |

#### config_activate

Choose the configuration the Part Studio shows (the Configurations box).

Sets the active value of configuration inputs; later regenerations, queries, renders, exports and feature edits use it (feature_edit then edits that configuration's configured values). `values` maps each input (id or name) to a List configuration (id or name), true/false for a Checkbox, or an expression for a configuration Variable ("40 mm"). Inputs not given keep their current value; `reset: true` returns every input to its default first. View state: no history entry, as in the app.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `values` | object |  | Input -> value. |
| `reset` | boolean |  | Start from the default configuration. |

#### config_reorder_inputs

Move a configuration input onto another's place (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `input` | string | required | Input to move. |
| `to` | string | required | Input whose place it takes. |

### assembly

#### assembly_insert

Insert a part (or a whole Part Studio, or a subassembly) into an Assembly (edits the document (one undoable history entry)).

The Insert dialog. Inserting the same item again offsets the new instance along X so they don't overlap. Returns the instance id; place it with mates or assembly_transform.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `source` | string | required | Part Studio or Assembly tab (name or id) to insert from. |
| `part` | string |  | A part of that Part Studio (`body:…` id or part name); default: the whole Part Studio as one instance. |
| `configuration` | object |  | Configuration of the source: {input id: value}. |

#### assembly_list

An assembly's instances, mates and degrees of freedom.

Solves the assembly and lists the instances (id, name, source, placement: translation in workspace units and rotation quaternion xyzw, fixed/hidden/suppressed, remaining DOF) and the mates (id, name, type, ok, error message).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |

#### assembly_entities

An instance's faces, edges or vertices, in assembly (world) coordinates, with ids for mates.

Like query_entities for one instance: ids, surface/curve type, centroid/midpoint, normal, axis and radius, all placed where the instance is in the assembly.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `kind` | one of `faces`, `edges`, `vertices` | required | Which entities. |
| `surface` | string |  | Faces: only this surface type (plane, cylinder, ...). |
| `normal` | array of number |  | Faces: outward normal in assembly coordinates. |
| `limit` | integer |  | At most this many (default 200). |

#### assembly_connectors

The implicit mate connector points on an instance's face, edge or vertex.

The points Onshape offers while hovering an entity (centroid, vertices, edge midpoints, circle centers, cylinder axis ends...), each with its world position and Z axis, ready to pass as a mate connector.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `entity` | string | required | Face, edge or vertex id from assembly_entities. |

#### assembly_mate

Add a mate between two mate connectors (Fastened, Revolute, Slider, Planar, Cylindrical, Pin slot, Ball, Parallel, Tangent, Width) (edits the document (one undoable history entry)).

The mate dialog: picks two connectors, solves the mate (the first connector's instance moves to the second, as in Onshape), and commits. A mate connector: {"instance": instance id or name, "entity": a face/edge/vertex id of that instance (from assembly_entities), "inference": where on it (center, centroid, midpoint, vertex, top_axis, mid_axis, bottom_axis, origin; default: the entity's first implicit point, its centroid or center)}, or a candidate from assembly_connectors {"instance", "reference", "align"}, or {"origin": true} for the assembly origin, or {"connector": an assembly mate connector id or name}. Optional "flip": true flips that connector's primary axis. Offsets are expressions ("5 mm", "90 deg").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `type` | one of `fastened`, `revolute`, `slider`, `planar`, `cylindrical`, `pinslot`, `ball`, `parallel`, `tangent`, `width` | required | Mate type. |
| `a` | object | required | First connector (its instance moves). |
| `b` | object | required | Second connector. |
| `name` | string |  | Name (default "Fastened 1", ...). |
| `flip_primary` | boolean |  | Flip the primary axis (default: keep the moving instance's current orientation, like the dialog). |
| `offset` | object |  | {"x", "y", "z": length, "rotation": angle} expressions where the mate type allows them. |
| `solve_all` | boolean |  | Solve every mate (the dialog's Solve) instead of only moving this mate's first instance. |

#### assembly_transform

Move an instance to a placement (edits the document (one undoable history entry)).

Sets the instance's translation (workspace units) and optional rotation (axis + angle) as when dragging it or using Transform; mates then pull mated instances.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `translation` | array of number or string | required | [x, y, z]. |
| `axis` | array of number |  | Rotation axis. |
| `angle` | number or string |  | Rotation angle ("90 deg"). |

#### assembly_set_state

Fix/unfix, hide/show or suppress/unsuppress instances (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instances` | array of string | required | Instance ids or names. |
| `fixed` | boolean |  | Fix or unfix. |
| `hidden` | boolean |  | Hide or show. |
| `suppressed` | boolean |  | Suppress or unsuppress. |

#### assembly_delete

Delete instances, mates or other assembly features (edits the document (one undoable history entry); deletes or overwrites).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instances` | array of string |  | Instance ids or names. |
| `mates` | array of string |  | Mate, relation, group, mate connector or pattern ids or names (a pattern takes its instances with it). |

#### assembly_pattern

Pattern instances: linear, circular, mirror, part pattern or replicate (edits the document (one undoable history entry)).

The Linear / Circular pattern, Mirror, Part pattern and Replicate dialogs. Linear copies the seeds along a direction (an edge, a face's normal or a mate connector's Z axis), optionally in a second direction; circular copies them about an axis; mirror copies them across a plane. Part pattern re-creates the seed's mate on every copy of its target made by a Part Studio pattern; replicate re-creates it on all matching geometry (every hole of the same size). Give `edit` to change an existing pattern: fields left out keep their values. One edit ("Insert : Linear pattern 1").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `kind` | one of `linear`, `circular`, `mirror`, `part_pattern`, `replicate` |  | Pattern type (required for a new pattern). |
| `seeds` | array of string |  | Instances to pattern (ids or names); part pattern / replicate use the first. |
| `direction` | object |  | Linear: the direction; circular: the axis; mirror: the plane. A mate connector, as in assembly_mate (an edge, a face or {"connector": ...}). |
| `distance` | string or number |  | Linear: the spacing (default 25 mm); circular: the angle (default 360 deg). |
| `count` | integer |  | Instance count, the seed included (default 2). |
| `opposite` | boolean |  | Opposite direction. |
| `equal_spacing` | boolean |  | Linear: `distance` is the total; circular: spread over the angle (default true for circular). |
| `second` | object |  | Linear: a second direction {"direction": connector, "distance", "count", "opposite"}. |
| `centered` | boolean |  | Linear / circular: the seed in the middle, `count` instances (the seed included) each way. |
| `handed` | boolean |  | Mirror: use mirrored copies of the parts ("Create mirrored parts") instead of rigid copies. |
| `mate` | string |  | Part pattern / replicate: the seed's mate to follow (default: the seed's only mate). |
| `scope` | one of `faces_in_parts`, `faces_in_features`, `individual_faces`, `edges_on_plane`, `edges_on_face`, `individual_edges` |  | Replicate: where to match (default: faces_in_parts, or edges_on_plane when the seed mate is on an edge). |
| `scope_refs` | array of object |  | Replicate: "Parts to find match in": [{"instance": id or name, "entity": a face / edge id (none: the whole part)}]. |
| `name` | string |  | Pattern name (default "Linear pattern 1"...). |
| `edit` | string |  | An existing pattern to edit (id or name). |

#### assembly_dissolve_pattern

Dissolve a pattern: its instances and mates become regular ones (edits the document (one undoable history entry)).

The pattern's context menu › Dissolve pattern. One edit ("Dissolve pattern").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `pattern` | string | required | Pattern id or name. |

#### assembly_bom

The Bill of Materials of an Assembly.

The BOM table with the assembly's columns and type (flattened or structured), as the BOM panel shows it; `csv` writes it the way ⋯ › Export to CSV does. `type`, `columns`, `show_excluded` and `show_top_level` override the stored table settings for this call only; assembly_bom_settings changes them.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `type` | one of `flattened`, `structured` |  | BOM type. |
| `columns` | array of string |  | Columns by name ("Item", "Quantity", "Part number", "Material", "Mass"...). |
| `show_excluded` | boolean |  | Show excluded and suppressed items. |
| `show_top_level` | boolean |  | Show the top level assembly row. |
| `csv` | string |  | Write the table to this CSV file. |

#### assembly_bom_settings

Change the BOM table's type and columns (edits the document (one undoable history entry)).

The BOM panel's settings: BOM type, the columns (Item stays first) and the ⋯ menu's Show excluded/suppressed and Show top level assembly row. Each change is its own edit, as in the panel ("BOM type", "Add column Material", "Move left"...).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `type` | one of `flattened`, `structured` |  | BOM type. |
| `columns` | array of string |  | The columns in order (Item is always first). |
| `show_excluded` | boolean |  | Show excluded and suppressed items. |
| `show_top_level` | boolean |  | Show the top level assembly row. |

#### assembly_interference

Interference detection: overlaps between instances.

Analysis tools › Interference detection: the overlapping solid parts among the given top-level instances (default: all), with the overlap's volume and size. Parts of one Part Studio instance aren't checked against each other.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instances` | array of string |  | Top-level instances (ids or names); default all. |
| `include_standard` | boolean |  | Include standard content (fasteners). |
| `top_only` | boolean |  | Show top level only: one row per pair of top-level instances. |

#### assembly_group

Group instances so they move as one rigid body (edits the document (one undoable history entry)).

The Group dialog (at least two instances). Give `edit` to change an existing group's instances. One edit ("Insert : Group" / "Edit : Group").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instances` | array of string | required | Instance ids or names (two or more). |
| `edit` | string |  | An existing group to edit (id or name). |

#### assembly_relation

Couple two mates: gear, rack and pinion, screw or linear relation (edits the document (one undoable history entry)).

The mate relation dialogs. Gear couples two revolute (or cylindrical) mates by `ratio`; rack and pinion a revolute and a slider by the distance per revolution; screw one cylindrical mate's rotation and translation (one mate); linear two sliders. The coupling is measured from the mates' current positions. One edit ("Insert : Gear relation").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `type` | one of `gear`, `rack_and_pinion`, `screw`, `linear` | required | Relation type. |
| `mates` | array of string | required | The two mates (ids or names); screw: one cylindrical mate. |
| `ratio` | string or number |  | Gear / linear: the ratio (default 1); rack and pinion / screw: the distance per revolution (default 10 mm / 1 mm). |
| `reverse` | boolean |  | Reverse direction. |
| `edit` | string |  | An existing relation to edit (id or name). |

#### assembly_mate_connector

Create or edit an assembly mate connector (edits the document (one undoable history entry)).

The Mate connector dialog: a connector on an instance's geometry (or the assembly origin), moved with its instance; mates use it as {"connector": "mc:<id>" or its name}. One edit ("Insert : Mate connector 1").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `on` | object |  | Origin entity (required for a new connector): a mate connector argument as in assembly_mate ({"instance", "entity", "inference"} or {"origin": true}). |
| `between` | object |  | Origin type Between entities: the origin is halfway between `on` and this entity (same instance). |
| `primary_axis` | object |  | Realign: the Z axis along this entity (face normal, edge direction). |
| `secondary_axis` | object |  | Realign: the X axis toward this entity's direction. |
| `flip_primary` | boolean |  | Flip primary axis. |
| `reorient` | integer |  | Reorient secondary axis: quarter turns about Z. |
| `offset` | object |  | Move: {"x", "y", "z": distances, "rotation": angle, "rotation_axis": "x" \| "y" \| "z" (default z)}, in the connector's frame. |
| `hidden` | boolean |  | Hide the connector. |
| `name` | string |  | Name (default "Mate connector 1"...). |
| `edit` | string |  | An existing assembly mate connector to edit (id or name). |

#### assembly_replace

Replace an instance with another part, Part Studio or assembly (edits the document (one undoable history entry)).

Replace instance: the instance (with `all`, every instance of the same source) gets the new source and keeps its position; mates whose faces / edges don't exist on the replacement are re-pointed to the nearest geometry of the same type. One edit ("Replace instance : Part 2").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `source` | string | required | Part Studio or Assembly tab (name or id). |
| `part` | string |  | A part of that Part Studio (`body:…` id or part name); default: the whole Part Studio. |
| `configuration` | object |  | Configuration of the source: {input id: value}. |
| `all` | boolean |  | Replace all instances of the same source. |

#### assembly_rename

Rename an instance, mate, relation, group, pattern or mate connector (edits the document (one undoable history entry)).

Rename in the instance list. One edit ("Rename : <new name>").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instance` | string |  | Instance id or name. |
| `feature` | string |  | Mate, relation, group, pattern or connector id or name. |
| `name` | string | required | The new name. |

#### assembly_set_feature_state

Suppress or unsuppress mates, relations, groups and patterns; show or hide mate connectors (edits the document (one undoable history entry)).

The list's Suppress / Unsuppress ("Suppress mate", "Suppress pattern"; a configured mate changes in the default configuration) and the mate connectors' eye ("Hide mate connector"). Features already in the asked state are left alone.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `features` | array of string | required | Feature ids or names. |
| `suppressed` | boolean |  | Suppress or unsuppress. |
| `hidden` | boolean |  | Mate connectors: hide or show. |

#### assembly_named_position

Named positions: add, update, apply, duplicate, rename, reorder, delete, set a driving value (edits the document (one undoable history entry)).

The Named positions panel. `add` stores the current state ("Named position 1"...); `update` stores the current state in an existing position; `apply` moves the assembly to a position (its driving mates' values are imposed, the other mates stay); `set_value` sets a driving mate's value in a position (see assembly_named_position_drivers). Each action is one edit with the panel's text ("Insert : Named position 1", "Apply named position : Open"...).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `action` | one of `add`, `update`, `apply`, `duplicate`, `delete`, `rename`, `move_up`, `move_down`, `set_value` | required | What to do. |
| `position` | string |  | The position: its name or 1-based row (all actions but add). |
| `name` | string |  | add: the new position's name; rename: the new name. |
| `mate` | string |  | set_value: the driving mate (id or name). |
| `dof` | one of `rz`, `tx`, `ty`, `tz` |  | set_value: the mate's value: rz = rotation about Z, tx / ty / tz = translation. |
| `value` | string or number |  | set_value: the value (angle or length expression). |

#### assembly_named_position_drivers

Choose the mates (and their degrees of freedom) that drive named positions (edits the document (one undoable history entry)).

The Driving mate selector: each driver is a mate and one of its free values (revolute: rz; slider: tz; cylindrical: tz, rz; pin slot: tx, rz; planar: tx, ty, rz; parallel: tx, ty, tz, rz). Replaces the list. One edit ("Driving mate selector").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `drivers` | array of object | required | [{"mate": id or name, "dof": "rz" \| "tx" \| "ty" \| "tz"}] |

#### assembly_exploded_view

Exploded views: add, rename or delete a view or a step (edits the document (one undoable history entry); deletes or overwrites).

The Exploded views panel: `add` creates "Explode N" (renamed to `name` if given); `rename` renames a view (or its `step`); `delete` deletes a view (or its `step`). Add steps with assembly_explode_step.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `action` | one of `add`, `rename`, `delete` | required | What to do. |
| `view` | string |  | The view: its name or 1-based row. |
| `step` | integer |  | A step of the view (1-based). |
| `name` | string |  | add / rename: the name. |

#### assembly_explode_step

Add or edit an explode step: move or rotate instances (edits the document (one undoable history entry)).

An explode step of a view: translate the instances `distance` along `direction`, or rotate them by `angle` about `direction` (through `pivot`, the picked connector's origin, or the instances' center). `direction` is a world vector [x, y, z] or a mate connector (its Z axis). One edit ("Edit : Explode step").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `view` | string | required | The view: its name or 1-based row. |
| `instances` | array of string | required | Instance ids or names. |
| `direction` | array or object | required | Direction / rotation axis: [x, y, z] or a mate connector (see assembly_mate). |
| `distance` | string or number |  | Translate by this distance. |
| `angle` | string or number |  | Rotate by this angle instead. |
| `pivot` | array of number |  | Rotation pivot [x, y, z] in document units. |
| `lines` | boolean |  | Show explode lines. |
| `step` | integer |  | Edit this step (1-based) instead of adding one. |

### drawing

#### drawing_templates

List the drawing templates.

Create Drawing's standard templates: file, sheet setup, standard (ANSI / ISO) and units.

#### drawing_create

Create a Drawing tab of a part, Part Studio or assembly (edits the document (one undoable history entry)).

Create Drawing: a Drawing tab with the template's sheet, placed after the source tab. With `suggested_views` (default true) the source's suggested views (Front, Top, Right projections and an isometric, scaled to fit) are laid out, as the dialog's checkbox does. Returns the tab and its views.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `source` | string |  | Part Studio or assembly tab (name or id). Omit for an empty drawing. |
| `part` | string |  | Only this part (`body:…` id from query_parts). |
| `template` | string |  | Template file or setup name (`drawing_templates`), e.g. "ANSI_A_INCH.dwt", "ISO A3". Default: ISO A3 in metric workspaces, ANSI B (inch) otherwise. |
| `name` | string |  | Tab name (default "<part> Drawing N" / "Drawing N"). |
| `suggested_views` | boolean |  | Lay out the suggested views. |

#### drawing_get

A drawing sheet's views (with their edges in sheet coordinates), annotations and tables.

Coordinates are sheet millimeters from the sheet's lower-left corner. Each view lists its kind, parent, scale, sheet bounds and the model edges as drawn (lines with end points, circles/arcs with center and radius, all in sheet mm) so you can click them with drawing_dimension.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `edges` | boolean |  | Include each view's edges. |

#### drawing_insert_view

Insert a base view of a part or assembly (edits the document (one undoable history entry)).

Insert view: the view centered at `position` (sheet mm) in the given orientation, at `scale` (paper per model, e.g. 0.5 for 1:2; default the sheet scale). One edit ("Insert view").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `source` | string | required | Part Studio or assembly tab. |
| `part` | string |  | Only this part (`body:…`). |
| `orientation` | one of `front`, `back`, `top`, `bottom`, `left`, `right`, `isometric` |  | View orientation. |
| `position` | array of number | required | View center on the sheet (mm). |
| `scale` | number |  | View scale (paper / model). |

#### drawing_projected_view

Project a view from a parent view (edits the document (one undoable history entry)).

Projected view: the parent view projected toward `position` (sheet mm): above/below/left/right of the parent gives the orthographic projection (aligned with the parent), diagonal gives an isometric. One edit ("Projected view").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `parent` | integer | required | Parent view id (drawing_get). |
| `position` | array of number | required | Where to place it (mm). |

#### drawing_dimension

Dimension view geometry (the drawing Dimension tools) (edits the document (one undoable history entry)).

Clicks the dimension tool on the view geometry at `picks` (sheet mm; click on an edge, or on a vertex / circle center to pick the point; drawing_get gives edge coordinates), then places the dimension at `place`. Linear dimensions are horizontal, vertical or aligned by where `place` is, as with the mouse. The dimension is associative to the model. Kinds:
- smart: the Dimension tool (d): click one edge (a line gives its length, a circle its diameter, an arc its radius) or two entities (points, lines, circles: distance, angle between lines, wall thickness of concentric circles)
- point_to_point: 2 point linear: two vertices / circle centers
- point_to_line: a point and a line
- line_to_line: two parallel lines
- radius: one circle or arc
- diameter: one circle or arc
- angle: two lines
- angle_3_point: vertex, then a point on each leg
- arc_length: one arc
- chamfer: the chamfer edge, then the edge it meets (Drawing properties decide a callout or two linear dimensions)
- max_min: a circle (where on it picks maximum or minimum), then a point or another circle
- foreshorten: a hole or cylinder axis seen side-on, then the edge or point the diameter reaches

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `kind` | one of `smart`, `point_to_point`, `point_to_line`, `line_to_line`, `radius`, `diameter`, `angle`, `angle_3_point`, `arc_length`, `chamfer`, `max_min`, `foreshorten` |  | Dimension tool. |
| `picks` | array of array of number | required | The picking clicks (sheet mm). |
| `place` | array of number | required | Where the dimension text goes (sheet mm). |

#### drawing_export

Export a drawing to PDF, DWG, DXF, DWT or SVG.

The Export Drawing modal: all sheets (or `sheets`: "current" or a range like "1-3, 5") to one file; PDF and DWT hold every sheet, other formats with several sheets are zipped (one file per sheet).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `format` | one of `PDF`, `DWG`, `DXF`, `DWT`, `SVG` | required | Format. |
| `file` | string | required | Output path. |
| `sheets` | string |  | "all" (default), "current" or a range "1-3, 5". |
| `color` | one of `color`, `black_and_white`, `grayscale` |  | PDF color (default color). |

#### drawing_section_view

Section view of a view (full, partial, jogged by snap points) (edits the document (one undoable history entry)).

The Section view tool: the cutting line goes through `point` (sheet mm; a vertex or circle center there is snapped) across the whole parent view, `direction` vertical (the dialog's default), horizontal or angular (`angle`, default 45 deg). With `partial` the line runs from `point` to `end` (snapped to horizontal / vertical within 8 deg, as the second click) and only cuts along its length. `jogs`: snap points of the view the line jogs through. The view goes to `position`, projected perpendicular to the line. One edit ("Section view").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `parent` | integer | required | View to section (drawing_get). |
| `point` | array of number | required | A point of the cutting line (sheet mm). |
| `direction` | one of `vertical`, `horizontal`, `angular` |  | Cutting line direction (full sections). |
| `angle` | number |  | Angular: the line's angle in degrees. |
| `opposite` | boolean |  | Angular: Opposite direction. |
| `partial` | boolean |  | Partial section: the line from `point` to `end` only. |
| `end` | array of number |  | Partial: the line's other end (sheet mm). |
| `jogs` | array of array of number |  | Full sections: snap points the line jogs through, in order. |
| `position` | array of number | required | Where the section view goes (sheet mm). |
| `label` | string |  | Section id (default the sheet's next letter). |
| `cut_only` | boolean |  | Show cut geometry only. |
| `flip` | boolean |  | Flip the view direction. |
| `depth` | number |  | Section depth (model mm). |
| `depth_to` | array of number |  | Section depth up to this vertex / circle center (sheet mm). |
| `exclude` | array of string |  | Parts excluded from the cut (`body:…`). |
| `cut_from_section` | boolean |  | Sectioning a section view: cut from the section. |

#### drawing_detail_view

Detail view of a circular region of a view (edits the document (one undoable history entry)).

The Detail view tool: the circle centered at `center` (sheet mm; a vertex or circle center there is snapped) with `radius` (sheet mm) on the parent view, shown at `scale` (default twice the parent's) at `position`. One edit ("Detail view").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `parent` | integer | required | View to detail. |
| `center` | array of number | required | Detail circle center (sheet mm). |
| `radius` | number | required | Detail circle radius (sheet mm). |
| `position` | array of number | required | Where the detail view goes (sheet mm). |
| `scale` | number |  | View scale (default 2 x the parent's). |
| `label` | string |  | Detail id (default the sheet's next letter). |
| `style` | one of `in_line`, `connected`, `leader` |  | Label style (default Drawing properties'). |
| `show_profile` | boolean |  | Show the detail profile (default Drawing properties'). |
| `clip_border` | boolean |  | Clip the border (default Drawing properties'). |

#### drawing_auxiliary_view

Auxiliary view looking normal to a straight edge (edits the document (one undoable history entry)).

The Auxiliary view tool (Normal): `edge` is a click on a straight model edge of the parent view (sheet mm); the view looks along it and goes to `position` (at least 15 mm from the parent, perpendicular to the edge). One edit ("Auxiliary view").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `edge` | array of number | required | A point on a straight edge of a view (sheet mm). |
| `position` | array of number | required | Where the view goes (sheet mm). |
| `label` | string |  | Auxiliary id (default the sheet's next letter). |
| `reverse` | boolean |  | Reverse the direction. |
| `show_reference` | boolean |  | Show the view reference indicator. |

#### drawing_break_view

Break a view (remove a band between two points) (edits the document (one undoable history entry)).

The Break view tool: removes the band between `from` and `to` (sheet mm) across the view: `vertical` break lines cut along x, horizontal ones along y. One edit ("Break view").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | View to break. |
| `from` | array of number | required | First break line (sheet mm). |
| `to` | array of number | required | Second break line (sheet mm). |
| `orientation` | one of `vertical`, `horizontal` |  | Break line orientation. |
| `style` | one of `zigzag`, `straight`, `curve`, `small_zigzag` |  | Break line style. |
| `gap` | number |  | Gap (sheet mm; default 6 mm, 0.25 in in inch drawings). |

#### drawing_crop_view

Crop a view to a rectangle or circle (edits the document (one undoable history entry)).

The Crop view tool: a rectangle with corners `from` and `to`, or a circle centered at `from` through `to` (sheet mm; `from` snaps to a vertex there). One edit ("Crop view").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | View to crop. |
| `shape` | one of `rectangle`, `circle` |  | Crop shape. |
| `from` | array of number | required | Rectangle corner / circle center (sheet mm). |
| `to` | array of number | required | Opposite corner / a point on the circle (sheet mm). |

#### drawing_centerline

Centerlines: 2 point, edge to edge, 3 point / 2 point circle (edits the document (one undoable history entry)).

The centerline tools, on clicks `picks` (sheet mm): two_point (two snap points of one view), edge_to_edge (one cylinder silhouette edge gives its axis; or two edges give their middle line), circle_3_point (three snap points) and circle_2_point (center, then a point on the circle). Associative to the model points snapped. One edit with the tool's name.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `kind` | one of `two_point`, `edge_to_edge`, `circle_3_point`, `circle_2_point` | required | Centerline tool. |
| `picks` | array of array of number | required | The clicks (sheet mm). |

#### drawing_virtual_sharp

Virtual sharp where two straight edges would meet (edits the document (one undoable history entry)).

The Virtual sharp tool: clicks on two straight edges of one view (sheet mm). Associative: dimensions can attach to it. One edit ("Virtual sharp").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `picks` | array of array of number | required | A point on each edge (sheet mm). |

#### drawing_hole_callout

Hole / thread callout on a hole seen as a circle (edits the document (one undoable history entry)).

The Hole/thread callout tool: `hole` is a click on the hole's circle (sheet mm); the callout text (diameter, depth, thread from the Hole feature) goes to `position`. One edit ("Hole callout").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `hole` | array of number | required | A point on the hole's circle (sheet mm). |
| `position` | array of number | required | Where the callout text goes (sheet mm). |

#### drawing_callout

Callout (balloon) with or without a leader (edits the document (one undoable history entry)).

The Callout tool and its dialog: with `anchor` (a click on an edge or point, sheet mm) the leader starts there (in an assembly view an empty `text` shows the part's BOM item number); without it the callout sits alone at `position`. One edit ("Callout").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `anchor` | array of number |  | Where the leader starts (sheet mm). Omit for a callout without a leader. |
| `position` | array of number | required | Where the callout goes (sheet mm). |
| `text` | string |  | Centre text (empty: the BOM item number in assembly views). |
| `above` | string |  | Text above. |
| `below` | string |  | Text below. |
| `left` | string |  | Text left. |
| `right` | string |  | Text right. |
| `border` | one of `none`, `circle`, `diamond`, `double_circle`, `flag`, `hexagon`, `octagon`, `rectangle`, `split_circle`, `square_circle`, `triangle`, `underline` |  | Border shape. |
| `chars` | integer |  | Border size in characters (0: tight fit). |
| `height` | number |  | Text height (mm). |

#### drawing_ordinate

Ordinate dimensions from a datum point (edits the document (one undoable history entry)).

The Ordinate dimension tool: `origin` is the datum (a vertex or circle center, sheet mm), `side` a click that sets the measured direction (beside the origin: horizontal values; above or below: vertical), then each of `points` adds an ordinate to the group. One edit per click, as with the mouse ("Ordinate dimension", then "Add to ordinate group").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `origin` | array of number | required | The datum point (sheet mm). |
| `side` | array of number | required | Where the values line up (sheet mm). |
| `points` | array of array of number |  | Points to dimension (vertices, circle centers; sheet mm). |

#### drawing_datum

Datum feature symbol (edits the document (one undoable history entry)).

The Datum tool: the triangle on the geometry at `anchor` (a vertex or edge point, sheet mm), the label box at `position` (at least 3 mm away). The label is the next free letter (A, B, …; I, O, Q, S, X, Z skipped) unless given. One edit ("Datum").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `anchor` | array of number | required | Where the triangle attaches (sheet mm). |
| `position` | array of number | required | Where the label box goes (sheet mm). |
| `label` | string |  | Datum letter. |

#### drawing_centermark

Centermarks: single, circular (bolt circle) or linear (edits the document (one undoable history entry)).

The Centermark tool and dialog: `holes` are clicks on hole circles of one view (sheet mm). single: a mark on each, one edit each ("Centermark"). circular: marks aligned to the pattern, with the bolt circle and reference center (and radial lines) as the dialog's options; linear: marks with connecting lines; one edit ("Circular centermark" / "Linear centermark").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `kind` | one of `single`, `circular`, `linear` |  | Centermark type. |
| `holes` | array of array of number | required | A point on each hole's circle (sheet mm). |
| `reference_center` | boolean |  | Circular: mark the pattern's center. |
| `bolt_circle` | boolean |  | Circular: the bolt circle through the holes. |
| `radial_lines` | boolean |  | Circular: lines from the center to each hole. |
| `align_to_pattern` | boolean |  | Circular: marks rotated to the pattern. |
| `connecting_lines` | boolean |  | Linear: lines between the marks. |

#### drawing_note

Note (text) with an optional leader (edits the document (one undoable history entry)).

The Note tool and panel: `text` (several lines with \n) placed at `position` (sheet mm); with `leader` (a vertex, circle center or edge point, sheet mm) a leader points there. `height` sets the text height (sheet mm; it becomes the drawing's note height default, as typing it in the panel does). One edit ("Note").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `text` | string | required | The note's text. |
| `position` | array of number | required | Where the text goes (sheet mm). |
| `leader` | array of number |  | Where the leader points (a snap point, sheet mm). |
| `height` | number |  | Text height (mm). |

#### drawing_gdt

Geometric tolerance (feature control frame) (edits the document (one undoable history entry)).

The Geometric tolerance tool and dialog: the frame at `position` (sheet mm). Each row: `symbol` (position, concentricity, symmetry, parallelism, perpendicularity, angularity, cylindricity, flatness, roundness, straightness, profile_of_a_surface, profile_of_a_line, circular_runout, total_runout; later rows default to composite), `tolerance` with optional `before` / `after` (e.g. "Ø", "Ⓜ"), up to three `datums` ("A", "B Ⓜ"), and `prefix` / `suffix` texts; `upper` / `lower` texts above and below. One edit ("Geometric tolerance").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `position` | array of number | required | Where the frame goes (sheet mm). |
| `rows` | array of object | required | Frame rows, top to bottom. |
| `upper` | string |  | Text above the frame. |
| `lower` | string |  | Text below the frame. |
| `leader` | one of `leader`, `all_around`, `all_over` |  | Leader type. |

#### drawing_surface_finish

Surface finish symbol (edits the document (one undoable history entry)).

The Surface finish symbol tool and panel: the symbol at `position` (sheet mm, on the edge it sits on) with the roughness text `value` (the panel starts with "Ra 3.2"). One edit ("Surface finish").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `position` | array of number | required | Where the symbol sits (sheet mm). |
| `value` | string |  | Roughness text. |

#### drawing_weld

Weld symbol (edits the document (one undoable history entry)).

The Weld symbol tool and dialog: the arrow points at `target` (a vertex, edge or point of a view, sheet mm), the reference line starts at `position`. `spec` is the dialog: `arrow` and `other` sides ({joint, size, depth, length, pitch, contour, finish, root, groove}; joints: None, SquareGroove, VGroove, UGroove, JGroove, BevelGroove, BevelButtBroadRoot, VFlareGroove, BevelFlareGroove, Bead, Fillet, PlugSlot, Seam, Spot; contours: None, Flat, Convex, Concave; finishes: "", C, G, H, M, R, U), `spacer` (None, DoubleV, DoubleBevel, DoubleU, DoubleJ, DoubleFlareV, DoubleFlareBevel), `stagger`, `stagger_right`, `symmetric`, `flip`, `flip_id_line`, `all_around`, `flag` (None, Up, Down), `reference` (tail text). One edit ("Weld symbol").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `target` | array of number | required | What the arrow points at (sheet mm). |
| `position` | array of number | required | Where the reference line starts (sheet mm). |
| `spec` | object |  | The Weld symbol dialog (see the description); empty: no joint symbol. |

#### drawing_view_table

Place a BOM, cut list or hole table of a view (edits the document (one undoable history entry)).

The BOM table, Cut list table and Hole table tools: the table of `view` with its top-left corner at `position` (sheet mm). A hole table measures from `origin` (a vertex or circle center of the view, sheet mm). One edit with the tool's name.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `kind` | one of `bom`, `cut_list`, `hole` | required | Table tool. |
| `view` | integer | required | The view the table lists. |
| `origin` | array of number |  | Hole table: the origin vertex (sheet mm). |
| `position` | array of number | required | Where the table goes (sheet mm). |

### part

#### part_rename

Rename a part (Parts list › Rename) (edits the document (one undoable history entry)).

The part's name in the parts list, BOMs and drawings ("Rename : <name>").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `part` | string | required | Part id (`body:…` from query_parts). |
| `name` | string | required | New name. |

#### part_set_properties

Set a part's properties (the Properties dialog) (edits the document (one undoable history entry)).

Name, description, part number, revision, vendor, project, product line, titles and a mass override (kg). Fields not given keep their value; an empty text clears it. Fields configured for the active configuration keep the part's own value, as in the dialog.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `part` | string | required | Part id (`body:…`). |
| `name` | string |  | Name. |
| `description` | string |  | Description. |
| `part_number` | string |  | Part number. |
| `revision` | string |  | Revision. |
| `vendor` | string |  | Vendor. |
| `project` | string |  | Project. |
| `product_line` | string |  | Product line. |
| `title1` | string |  | Title 1. |
| `title2` | string |  | Title 2. |
| `title3` | string |  | Title 3. |
| `mass_override` | number or null |  | Mass override in kg (null: computed). |

#### part_set_material

Assign a material to parts (density for mass properties) (edits the document (one undoable history entry)).

The Material panel: a library material by name (`material_list`), or a custom material with `density`. One edit ("Assign material").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `parts` | array of string | required | Part ids (`body:…`) or names. |
| `material` | string |  | Library material name, e.g. "Aluminum - 6061"; or the custom material's name with `density`. |
| `library` | string |  | Library to take it from (default: the first that has it). |
| `density` | number |  | Custom material: density in kg/m³. |

#### part_set_appearance

Set the color and transparency of parts, faces or a feature (the Appearance editor) (edits the document (one undoable history entry)).

Give `parts`, `faces` or `feature`. Color as "#3b61b4" or [r, g, b] (0–255, sRGB); transparency 0 (opaque) to 1. Parts also take the tessellation quality. One edit ("Edit appearance").

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `parts` | array of string |  | Part ids or names. |
| `faces` | array of string |  | Face ids (`face:…`). |
| `feature` | string |  | Feature id or name (its faces; a sketch: its curves). |
| `color` | string or array | required | "#rrggbb" or [r, g, b]. |
| `transparency` | number |  | 0 opaque (default) … 1 invisible. |
| `tessellation` | one of `auto`, `coarse`, `medium`, `fine`, `very_fine` |  | Parts: tessellation quality (default auto). |

#### part_remove_appearance

Remove custom appearances (Appearance panel › Remove appearance) (edits the document (one undoable history entry); deletes or overwrites).

A part's own appearance (`parts`), or face / feature appearances by index (`appearances`, from part_appearances).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `parts` | array of string |  | Part ids or names. |
| `appearances` | array of integer |  | Face / feature appearance indices. |

#### part_appearances

The Part Studio's custom appearances and materials (the Appearance panel).

Each part's material, density and own appearance, and the face / feature appearances with their index.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |

### material

#### material_list

List the materials of the material libraries.

The Material panel's libraries: the Onshape Material Library and the document's Material Library tabs, with each material's category and density (kg/m³).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `search` | string |  | Only materials whose name or category contains this. |

### query

#### query_parts

The parts, surfaces and curves of a Part Studio.

Each body with its id (`body:…`), name, kind, volume, surface area, bounding box and face/edge/vertex counts. Lengths in the workspace units (see `units`).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |

#### query_entities

Faces, edges or vertices with stable ids and their geometry, to select for features.

Lists the faces / edges / vertices of the parts (or of `part`) with an id to pass to selection parameters, the feature that made them, and geometry: faces: surface type, area, centroid, normal (planar), axis and radius (cylinders, cones...); edges: curve type, length, endpoints, midpoint, center/radius (circles); vertices: position. Filters narrow the list: `surface`/`curve` type, `normal` (planar faces whose outward normal is within 1° of it), `created_by` (feature id or name), `near` (closest first, max `limit`).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `kind` | one of `faces`, `edges`, `vertices` | required | Which entities. |
| `part` | string |  | Only this part (`body:…` id or part name). |
| `surface` | one of `plane`, `cylinder`, `cone`, `sphere`, `torus`, `bspline`, `other` |  | Faces: only this surface type. |
| `curve` | one of `line`, `circle`, `ellipse`, `bspline`, `other` |  | Edges: only this curve type. |
| `normal` | array of number |  | Faces: outward normal direction, e.g. [0,0,1] for faces facing +Z. |
| `created_by` | string |  | Only entities made by this feature (id or name). |
| `near` | array of number or string |  | Sort by distance from this point (workspace units). |
| `limit` | integer |  | At most this many results (default 200). |

#### query_datums

Planes, the origin, sketches and mate connectors of a Part Studio.

Default planes (`plane:top`, `plane:front`, `plane:right`), Plane features (`fplane:…`), sketches (`sketch:…`) with their plane frames, and mate connectors (`mc:…`).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |

#### query_measure

Measure entities (lengths, areas, radii, distances, angles).

The Measure tool: one entity gives its length/area/radius...; two give the minimum distance (with dx, dy, dz) and the angle between them. Values in workspace units.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `entities` | array of string | required | Reference ids. |

#### query_mass_properties

Mass properties: volume, surface area, mass, center of mass, inertia.

Of the given parts, or all parts. Mass, center of mass and inertia need a material (or density) on every part, as in Onshape; mass overrides apply. Inertia (kg·length²) is about the center of mass, or about mate connector `reference`'s axes.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `parts` | array of string |  | Part ids (`body:…`) or names; default all. |
| `reference` | string |  | Mate connector id (`mc:…`) for the inertia axes. |

### analysis

#### analysis_curvature

Curvature of edges, sketch curves and faces (Curve/surface analysis).

The minimum radius of curvature over the selection and where it is, the inflection points of sketch curves, and the number of curvature combs (one per curve; U and V isoparametric curves per face).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `entities` | array of string | required | Edges, faces (`edge:…`, `face:…`) or sketch curves (`entity:…`). |
| `extend` | boolean |  | Extend across face: include the tangent-connected faces. |
| `u_curves` | integer |  | Faces: U curves (default 8). |
| `v_curves` | integer |  | Faces: V curves (default 8). |

#### analysis_deviation

Deviation between two curve chains or two surfaces (Deviation analysis).

Minimum and maximum distance from the first selection to the second, with the points on each.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `first` | array of string | required | Edges / sketch curves (or faces with `surface`). |
| `second` | array of string | required | The chain / faces to measure to. |
| `surface` | boolean |  | Surface deviation (faces) instead of curves. |

#### analysis_connection

Continuity where curves and faces meet (Connection analysis).

Each joint between the curves (G0 position, G1 tangent, G2 curvature, G3, or none) and each shared edge segment between the faces, with Onshape's tolerances (G1 0.3°, G2 2.5 %, G3 5 % unless given).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `entities` | array of string | required | Edges, sketch curves and faces. |
| `g1_deg` | number |  | G1 angle tolerance (degrees). |
| `g2` | number |  | G2 relative tolerance. |
| `g3` | number |  | G3 relative tolerance. |

#### analysis_dihedral

Dihedral angle along edges (Dihedral analysis).

The minimum and maximum angle between the faces along the edges (degrees).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `entities` | array of string | required | Edges. |

#### analysis_interference

Interference detection between parts.

Every pair of the parts whose material overlaps, with the overlap volume and its bounding box.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `parts` | array of string |  | Parts (`body:…`). Default: every part. |

### render

#### render_image

Render a Part Studio to a PNG image from a standard or named view.

Draws the parts, sketches, planes and origin as the 3D view does (Onshape's look: white background, shaded with edges), zoomed to fit. Returned as an image (MCP image content) and, with `file`, written to disk.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `view` | string |  | isometric (default), front, back, left, right, top, bottom, dimetric, trimetric, or the name of a saved named view. |
| `width` | integer |  | Pixels (default 800). |
| `height` | integer |  | Pixels (default 600). |
| `display_mode` | one of `shaded_with_edges`, `shaded`, `hidden_line`, `translucent` |  | Render style (default shaded_with_edges). |
| `show_sketches` | boolean |  | Draw sketches. |
| `show_planes` | boolean |  | Draw the default and construction planes. |
| `file` | string |  | Also write the PNG here. |

### export

#### export_parts

Export a Part Studio's parts to a CAD or mesh file (STEP, Parasolid, IGES, ACIS, STL, OBJ, 3MF, glTF, GLB, JT, PVZ, Rhino).

Onshape's Export dialog. Exports the visible parts (or `parts`) to `file`. Options as in the dialog: `y_up` (models oriented Y axis up), `version` (format version: STEP AP242/AP214/AP203, Parasolid 38.1..., Rhino 8...), `units` (mesh formats and STEP with custom units), `resolution` (coarse/medium/fine for mesh formats), `stl_text`, `individual` (one file per part, zipped), `include_hidden`.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the first Part Studio. |
| `format` | one of `PARASOLID`, `ACIS`, `STEP`, `IGES`, `PVZ`, `JT`, `RHINO`, `GLTF`, `GLB`, `OBJ`, `3MF`, `STL` | required | File format. |
| `file` | string | required | Output path; without an extension the format's is added ("part" -> "part.x_t"); individual files go in a .zip. |
| `parts` | array of string |  | Part ids (`body:…`) or names; default all visible parts. |
| `y_up` | boolean |  | Export models oriented Y axis up. |
| `version` | string |  | Format version (e.g. "AP214", "35.1", "7"). |
| `units` | one of `Centimeter`, `Foot`, `Inch`, `Meter`, `Millimeter`, `Yard` |  | File units (STL, OBJ...; STEP: custom units). |
| `resolution` | one of `Coarse`, `Medium`, `Fine` |  | Mesh resolution (default Medium). |
| `stl_text` | boolean |  | STL as text instead of binary. |
| `individual` | boolean |  | Export unique parts as individual files (zipped). |
| `include_hidden` | boolean |  | Include hidden parts. |

### import

#### import_files

Import CAD or mesh files (STEP, IGES, Parasolid, ACIS, JT, Rhino, SolidWorks/Inventor/NX/Solid Edge parts and assembly zips, STL, OBJ, 3MF, glTF) (edits the document (one undoable history entry)).

The Import dialog: each file becomes a Part Studio with an Import feature (or all of them one Part Studio with `combine`); a native assembly zip becomes an Assembly and its Part Studio. The files are kept as file tabs in a "CAD Imports" folder unless `keep_files` is false. One edit ("Import <files>"). Returns the new tabs and the imported parts.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `files` | array of string | required | Paths of the files. |
| `combine` | boolean |  | Combine to a single Part Studio (default: one Part Studio per file). |
| `appearances` | boolean |  | Import appearances. |
| `density` | boolean |  | Import material density (native parts and assemblies). |
| `y_up` | boolean |  | Orient imported models with Y axis up. |
| `mesh_unit` | one of `Centimeter`, `Foot`, `Inch`, `Meter`, `Millimeter`, `Yard` |  | STL / OBJ units (default: the workspace unit; OBJ: Meter). |
| `composite` | boolean |  | Create a composite part. |
| `join` | boolean |  | Join adjacent surfaces. |
| `keep_files` | boolean |  | Keep the files as file tabs in "CAD Imports". |
| `after` | string |  | Tab to put the new tabs after (default: the end). |

### featurestudio

#### featurestudio_get

A Feature Studio's FeatureScript source, its compile notices and the features it defines.

The text the editor shows (the saved draft, else the committed source), whether it has uncommitted changes, the notices (errors, warnings, info with line and column) and the custom features it defines. Part Studios use the committed source.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Feature Studio tab (name or id); default: the first. |

#### featurestudio_edit

Replace a Feature Studio's source (saved, not yet committed) (edits the document (one undoable history entry); deletes or overwrites).

Typing in the Feature Studio: the text is saved as a draft ("Edit Feature Studio"); Part Studios keep the committed source until featurestudio_commit. Returns the compile notices.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Feature Studio tab (name or id); default: the first. |
| `source` | string | required | The whole FeatureScript source. |

#### featurestudio_commit

Commit a Feature Studio (or all) so Part Studios use its features (edits the document (one undoable history entry)).

Commit (Ctrl+S) / Commit all (`all`): the draft becomes the source Part Studios use; one edit per Feature Studio ("Commit Feature Studio"). Returns the custom feature types now available to feature_add.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Feature Studio tab (name or id); default: the first. |
| `all` | boolean |  | Commit every Feature Studio with uncommitted changes. |

### history

#### history_undo

Undo the last edit (edits the document (one undoable history entry)).

With `tab`, undoes the last edit of that tab (Onshape's undo is per tab); otherwise the last edit of the document.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Tab whose edit to undo. |

#### history_redo

Redo the last undone edit (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Tab whose edit to redo. |

#### history_list

The edit history of the current branch, newest first.

Microversions with their description ("Part Studio 1 :: Insert feature : Extrude 1"), author and time, plus versions and branches.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `limit` | integer |  | At most this many entries (default 50). |

#### history_create_version

Create a named version of the current state (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `name` | string | required | Version name ("V1"). |
| `description` | string |  | Description. |

#### history_create_branch

Create a branch (workspace) from the current state and switch to it (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `name` | string | required | Branch name. |

#### history_switch_branch

Switch to another branch (edits the document (one undoable history entry)).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `branch` | string | required | Branch name or id. |

#### history_merge

Merge another branch into the current one (edits the document (one undoable history entry)).

Part Studios merge feature by feature.

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `branch` | string | required | Branch to merge from (name or id). |

#### history_restore

Restore the document to an earlier microversion or version (edits the document (one undoable history entry)).

Adds a new microversion with that state (history is kept).

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `to` | string | required | Microversion id or version name/id (from history_list). |
<!-- END GENERATED REFERENCE -->
