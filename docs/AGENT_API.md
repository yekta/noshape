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

The server's tool list is abridged so it fits an agent's context (about 35k characters with the
instructions, instead of 340k): the ~45 common tools (`crates/cli/src/core_tools.rs`) with short descriptions
and their main fields. Nothing is lost: those tools accept every field, `ops_list` lists every tool by
group, `ops_describe` returns any tool's full description, examples and schema, and `ops_call` runs any tool.
`noshape-cli mcp --all-tools` lists every tool in full (for clients that defer tool loading); the CLI's
`--help` is always complete.

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

### Lessons from agent runs

Five tasks driven through the MCP server with only these docs and the replies (a bracket from a text spec,
editing an early dimension, a 3-part assembly with mates, a drawing exported to PDF, a STEP import edited and
re-exported) gave these rules:

- **Get ids in one call.** `query_entities` filters by `surface` / `curve`, `normal` (faces), `direction`
  (edges along it, e.g. `[0,0,1]` for vertical edges), `created_by` and `near`; `at: [[x,y,z], ...]` returns the
  entity at each point, in order (three fillet edges: one call). `assembly_entities` has the same filters in
  assembly coordinates. Parts can be named instead of `body:…` ids (`assembly_insert {"part": "Bracket"}`).
- **Sketching on a face:** its sketch axes follow the face and may be turned (a +Z face can have x = −Y): read
  the `frame` in the `sketch_create` reply, or give `sketch_draw` model points `[x, y, z]`, which are projected
  onto the sketch plane.
- **Feature parameters:** find a type with `feature_types {"search": "move face"}`; `feature_schema` says for
  each conditional parameter whether the dialog shows it with the defaults and which values of which parameter
  show or hide it. Hidden parameters are ignored without an error, so pass the switch too (hole: `endStyleV2`
  `THROUGH` for a through hole) and, when unsure, check `visible` in `feature_get`.
- **Check every edit:** `feature_status` / `regeneration.failed` on feature edits, `failed` on sketch edits
  (a dimension change that breaks a later feature shows there); then compare `query_parts` volume and bounding
  box with a hand estimate and look at `render_image`. Status `Ok` does not mean the geometry is what you meant.
- **Dimensions** to edit later: `sketch_dimension` returns its `dim:…` id; `sketch_get` lists them among the
  `constraints` with their expressions; `sketch_set_dimension` changes one and the model regenerates.
- **Mates:** the first connector's instance moves. An offset runs along the mate's Z axis, which follows the
  first connector (flipped as the dialog does, `flip_primary` in `assembly_list`): check the moved instance's
  `translation` and negate the offset if it went the wrong way. `assembly_interference` lists overlaps.
- **Drawings:** `drawing_get` gives each view's `orientation` (projected views too: `top`, `right`...), bounds
  and drawn edges in sheet millimeters; dimension clicks go on those coordinates. Export PNG to look at the sheet.
- **Custom (FeatureScript) features:** a failing one carries its FeatureScript `notices` (message, stack of
  `line:col` and `module (function)`) and `println` `output` in `feature_add` / `feature_edit` failures,
  `feature_get` and `feature_list`; `featurestudio_monitor` shows a Part Studio's whole Monitor.
- **Undo:** `history_undo` names the edit it undid (`description`). `feature_delete` takes a list `features`.
- Replies give numbers to 12 significant digits (no `3.3000000000000007`, no `-0`).

A second run (the Part Studio workflows end to end: sketch, extrude, fillet, holes, a feature pattern, shell,
edits with undo / redo, a configured part, an assembly with mates) added:

- **Select by query, not by copied ids.** Any selection in `feature_add` / `feature_edit` takes
  `{"query": {query_entities arguments}}`, alone or among ids in a list: the four vertical edges are
  `"entities": {"query": {"kind": "edges", "direction": [0, 0, 1]}}`. A query that finds nothing is an error.
- **Patterns:** give a pattern the features to repeat as its `entities` (`feature:…` ids from `feature_list`):
  that makes it a Feature pattern; faces make it a Face pattern.
- **Configurations:** `query_parts` and `query_mass_properties` take `configuration` by input and option
  names (`{"Size": "Large"}`); no need to activate and switch back.
- **Finding tools:** `ops_list` alone is short (names and groups); `ops_list {"group": "config"}` (or
  "configuration") and `ops_list {"search": "pattern assembly"}` give summaries and examples.
- **Feature schemas** describe the parameters the dialog shows with the defaults; `more_when_shown` names the
  others (`counterbore`: `cBoreDiameterV3`...), `all: true` describes them too.
- **Default tabs follow the window:** tools without `tab` use the tab the window shows when it is of their kind
  (after `tab_activate`, or an import, which shows what it made: `first`), else the first of the kind.
- **Imports:** a STEP assembly becomes an Assembly plus a "<name> parts" Part Studio; instances are named
  "<part> <n>", numbered per name. `query_entities` `near` ranks by the distance to the face or edge itself.
- **Unknown tool names** get "did you mean" suggestions from the same group (`variable_edit` -> `variable_set`).
- **Edit replies stay small:** `regeneration.features` lists only the features that need a look (not `Ok`,
  or with a message, notices, under-defined, suppressed); `features_ok` counts the rest; `feature_list` lists all.

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
drawing_create    {"source": "Part Studio 1", "template": "ISO A3", "suggested_views": true}   -> tab "Part 1 Drawing 1" with suggested views
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

The operations by group. Field types are JSON types; lengths and angles take expressions with units.

- **ops**: [`ops_list`](#ops_list), [`ops_describe`](#ops_describe), [`ops_call`](#ops_call)
- **document**: [`document_create`](#document_create), [`document_open`](#document_open), [`document_info`](#document_info), [`document_close`](#document_close), [`document_protect`](#document_protect), [`document_unprotect`](#document_unprotect), [`document_set_units`](#document_set_units)
- **tab**: [`tab_create`](#tab_create), [`tab_rename`](#tab_rename), [`tab_duplicate`](#tab_duplicate), [`tab_delete`](#tab_delete), [`tab_folder_create`](#tab_folder_create), [`tab_move`](#tab_move), [`tab_folder_unpack`](#tab_folder_unpack), [`tab_activate`](#tab_activate), [`tab_manager`](#tab_manager)
- **feature**: [`feature_types`](#feature_types), [`feature_schema`](#feature_schema), [`feature_add`](#feature_add), [`feature_edit`](#feature_edit), [`feature_get`](#feature_get), [`feature_list`](#feature_list), [`feature_delete`](#feature_delete), [`feature_suppress`](#feature_suppress), [`feature_dynamic_suppression`](#feature_dynamic_suppression), [`feature_clear_missing`](#feature_clear_missing), [`feature_dependencies`](#feature_dependencies), [`feature_add_folder`](#feature_add_folder), [`feature_rename_folder`](#feature_rename_folder), [`feature_unpack_folder`](#feature_unpack_folder), [`feature_rollback`](#feature_rollback), [`feature_rename`](#feature_rename), [`feature_reorder`](#feature_reorder), [`feature_set_visibility`](#feature_set_visibility), [`feature_size_plane`](#feature_size_plane), [`feature_show_all`](#feature_show_all)
- **sketch**: [`sketch_create`](#sketch_create), [`sketch_draw`](#sketch_draw), [`sketch_constrain`](#sketch_constrain), [`sketch_dimension`](#sketch_dimension), [`sketch_set_dimension`](#sketch_set_dimension), [`sketch_dimension_options`](#sketch_dimension_options), [`sketch_delete`](#sketch_delete), [`sketch_set_construction`](#sketch_set_construction), [`sketch_get`](#sketch_get), [`sketch_paste`](#sketch_paste), [`sketch_export`](#sketch_export), [`sketch_copy_entities`](#sketch_copy_entities), [`sketch_trim`](#sketch_trim), [`sketch_extend`](#sketch_extend), [`sketch_split`](#sketch_split), [`sketch_spline_point`](#sketch_spline_point), [`sketch_fillet`](#sketch_fillet), [`sketch_chamfer`](#sketch_chamfer), [`sketch_offset`](#sketch_offset), [`sketch_mirror`](#sketch_mirror), [`sketch_linear_pattern`](#sketch_linear_pattern), [`sketch_circular_pattern`](#sketch_circular_pattern), [`sketch_transform`](#sketch_transform), [`sketch_use`](#sketch_use), [`sketch_intersection`](#sketch_intersection), [`sketch_text`](#sketch_text), [`sketch_insert_dxf`](#sketch_insert_dxf), [`sketch_insert_image`](#sketch_insert_image)
- **variable**: [`variable_list`](#variable_list), [`variable_create`](#variable_create), [`variable_set`](#variable_set), [`variable_delete`](#variable_delete), [`variable_move`](#variable_move)
- **config**: [`config_list`](#config_list), [`config_add_input`](#config_add_input), [`config_edit_input`](#config_edit_input), [`config_delete_input`](#config_delete_input), [`config_add_option`](#config_add_option), [`config_edit_option`](#config_edit_option), [`config_configure`](#config_configure), [`config_set_value`](#config_set_value), [`config_activate`](#config_activate), [`config_copy_inputs`](#config_copy_inputs), [`config_duplicate_input`](#config_duplicate_input), [`config_propagate`](#config_propagate), [`config_edit_ids`](#config_edit_ids), [`config_visibility`](#config_visibility), [`config_reorder_inputs`](#config_reorder_inputs)
- **assembly**: [`assembly_insert`](#assembly_insert), [`assembly_list`](#assembly_list), [`assembly_entities`](#assembly_entities), [`assembly_connectors`](#assembly_connectors), [`assembly_mate`](#assembly_mate), [`assembly_transform`](#assembly_transform), [`assembly_set_state`](#assembly_set_state), [`assembly_delete`](#assembly_delete), [`assembly_pattern`](#assembly_pattern), [`assembly_dissolve_pattern`](#assembly_dissolve_pattern), [`assembly_bom`](#assembly_bom), [`assembly_bom_settings`](#assembly_bom_settings), [`assembly_interference`](#assembly_interference), [`assembly_change_to_version`](#assembly_change_to_version), [`assembly_group`](#assembly_group), [`assembly_relation`](#assembly_relation), [`assembly_mate_connector`](#assembly_mate_connector), [`assembly_replace`](#assembly_replace), [`assembly_rename`](#assembly_rename), [`assembly_set_feature_state`](#assembly_set_feature_state), [`assembly_named_position`](#assembly_named_position), [`assembly_named_position_drivers`](#assembly_named_position_drivers), [`assembly_exploded_view`](#assembly_exploded_view), [`assembly_explode_step`](#assembly_explode_step), [`assembly_animate`](#assembly_animate), [`assembly_display_state`](#assembly_display_state), [`assembly_move_to_subassembly`](#assembly_move_to_subassembly), [`assembly_create_subassembly`](#assembly_create_subassembly), [`assembly_dissolve_subassembly`](#assembly_dissolve_subassembly), [`assembly_add_folder`](#assembly_add_folder), [`assembly_unpack_folder`](#assembly_unpack_folder), [`assembly_copy`](#assembly_copy), [`assembly_set_configuration`](#assembly_set_configuration), [`assembly_lock`](#assembly_lock), [`assembly_instance_properties`](#assembly_instance_properties), [`assembly_dynamic_suppression`](#assembly_dynamic_suppression), [`assembly_standard_content`](#assembly_standard_content), [`assembly_insert_standard`](#assembly_insert_standard), [`assembly_edit_standard`](#assembly_edit_standard), [`assembly_edit_in_context`](#assembly_edit_in_context), [`assembly_create_part_studio_in_context`](#assembly_create_part_studio_in_context), [`assembly_update_context`](#assembly_update_context), [`assembly_set_primary_instance`](#assembly_set_primary_instance), [`assembly_switch_to_tab`](#assembly_switch_to_tab), [`assembly_open_linked_document`](#assembly_open_linked_document)
- **context**: [`context_list`](#context_list), [`context_show`](#context_show), [`context_update`](#context_update), [`context_rename`](#context_rename), [`context_delete`](#context_delete), [`context_exit`](#context_exit), [`context_insert_in_assembly`](#context_insert_in_assembly), [`context_go_to_assembly`](#context_go_to_assembly)
- **drawing**: [`drawing_templates`](#drawing_templates), [`drawing_create`](#drawing_create), [`drawing_get`](#drawing_get), [`drawing_insert_view`](#drawing_insert_view), [`drawing_projected_view`](#drawing_projected_view), [`drawing_dimension`](#drawing_dimension), [`drawing_update`](#drawing_update), [`drawing_export`](#drawing_export), [`drawing_section_view`](#drawing_section_view), [`drawing_jogged_section_view`](#drawing_jogged_section_view), [`drawing_aligned_section_view`](#drawing_aligned_section_view), [`drawing_detail_view`](#drawing_detail_view), [`drawing_edit_detail`](#drawing_edit_detail), [`drawing_auxiliary_view`](#drawing_auxiliary_view), [`drawing_break_view`](#drawing_break_view), [`drawing_crop_view`](#drawing_crop_view), [`drawing_broken_out_section`](#drawing_broken_out_section), [`drawing_edit_hatch`](#drawing_edit_hatch), [`drawing_title_field`](#drawing_title_field), [`drawing_hatch_region`](#drawing_hatch_region), [`drawing_custom_table`](#drawing_custom_table), [`drawing_insert_image`](#drawing_insert_image), [`drawing_insert_cad`](#drawing_insert_cad), [`drawing_sketch`](#drawing_sketch), [`drawing_delete`](#drawing_delete), [`drawing_edit_dimension`](#drawing_edit_dimension), [`drawing_format_painter`](#drawing_format_painter), [`drawing_inspection`](#drawing_inspection), [`drawing_properties`](#drawing_properties), [`drawing_centerline`](#drawing_centerline), [`drawing_virtual_sharp`](#drawing_virtual_sharp), [`drawing_hole_callout`](#drawing_hole_callout), [`drawing_callout`](#drawing_callout), [`drawing_ordinate`](#drawing_ordinate), [`drawing_datum`](#drawing_datum), [`drawing_centermark`](#drawing_centermark), [`drawing_note`](#drawing_note), [`drawing_gdt`](#drawing_gdt), [`drawing_surface_finish`](#drawing_surface_finish), [`drawing_weld`](#drawing_weld), [`drawing_revision_table`](#drawing_revision_table), [`drawing_revision_callout`](#drawing_revision_callout), [`drawing_table`](#drawing_table), [`drawing_table_cell`](#drawing_table_cell), [`drawing_view_table`](#drawing_view_table), [`drawing_view_display`](#drawing_view_display), [`drawing_view_edit`](#drawing_view_edit), [`drawing_view_properties`](#drawing_view_properties), [`drawing_view_show_hide`](#drawing_view_show_hide), [`drawing_view_linestyle`](#drawing_view_linestyle), [`drawing_sheet`](#drawing_sheet), [`drawing_table_edit`](#drawing_table_edit), [`drawing_annotation_edit`](#drawing_annotation_edit), [`drawing_mbd`](#drawing_mbd), [`drawing_table_properties`](#drawing_table_properties), [`drawing_copy`](#drawing_copy), [`drawing_paste`](#drawing_paste), [`drawing_paste_table`](#drawing_paste_table), [`drawing_replace`](#drawing_replace), [`drawing_replicate`](#drawing_replicate), [`drawing_switch_to_reference`](#drawing_switch_to_reference), [`drawing_open_feature_studio`](#drawing_open_feature_studio), [`drawing_switch_sheet`](#drawing_switch_sheet), [`drawing_sheets_panel`](#drawing_sheets_panel)
- **ui**: [`ui_state`](#ui_state), [`ui_preferences`](#ui_preferences), [`ui_toolbar`](#ui_toolbar), [`ui_search_tools`](#ui_search_tools), [`ui_shortcuts`](#ui_shortcuts), [`ui_select`](#ui_select), [`ui_list`](#ui_list)
- **history**: [`history_view`](#history_view), [`history_compare`](#history_compare), [`history_repair_view`](#history_repair_view), [`history_undo`](#history_undo), [`history_redo`](#history_redo), [`history_list`](#history_list), [`history_create_version`](#history_create_version), [`history_create_branch`](#history_create_branch), [`history_switch_branch`](#history_switch_branch), [`history_merge`](#history_merge), [`history_restore`](#history_restore)
- **part**: [`part_rename`](#part_rename), [`part_set_properties`](#part_set_properties), [`part_configured_properties`](#part_configured_properties), [`part_annotation`](#part_annotation), [`part_set_material`](#part_set_material), [`part_set_appearance`](#part_set_appearance), [`part_remove_appearance`](#part_remove_appearance), [`part_appearances`](#part_appearances)
- **material**: [`material_list`](#material_list)
- **query**: [`query_parts`](#query_parts), [`query_entities`](#query_entities), [`query_datums`](#query_datums), [`query_measure`](#query_measure), [`query_mass_properties`](#query_mass_properties)
- **analysis**: [`analysis_curvature_map`](#analysis_curvature_map), [`analysis_thickness`](#analysis_thickness), [`analysis_curvature`](#analysis_curvature), [`analysis_deviation`](#analysis_deviation), [`analysis_connection`](#analysis_connection), [`analysis_dihedral`](#analysis_dihedral), [`analysis_interference`](#analysis_interference), [`analysis_draft`](#analysis_draft), [`analysis_flatten`](#analysis_flatten)
- **render**: [`render_named_view`](#render_named_view), [`render_image`](#render_image)
- **view**: [`view_camera`](#view_camera), [`view_display`](#view_display)
- **export**: [`export_parts`](#export_parts)
- **import**: [`import_files`](#import_files)
- **featurestudio**: [`featurestudio_get`](#featurestudio_get), [`featurestudio_edit`](#featurestudio_edit), [`featurestudio_commit`](#featurestudio_commit), [`featurestudio_monitor`](#featurestudio_monitor)

### ops

#### ops_list

List every operation (tool) of the noshape API.

Without filters: every tool's name and the groups with their sizes (short). With `group` (a group's name, its start or a word in it: "configuration" finds config) or `search` (words all found in a tool's name or summary): each tool's name, CLI command, summary and examples. Use `feature_types` and `feature_schema` for the Part Studio features and their parameters, ops_describe for a tool's full documentation.

Examples:

```json
{}
{"group": "sketch"}
```

| field | type | | description |
|---|---|---|---|
| `group` | string |  | Only this group (document, tab, feature, sketch, variable, config, part, material, query, analysis, render, export, import, featurestudio, history, assembly, context, drawing, view, ui...). |
| `search` | string |  | Only tools whose name or summary holds all these words ("pattern assembly"). |

#### ops_describe

Full documentation of tools: description, examples and input schema.

Everything the CLI's --help says about each tool: its whole description with examples, and its JSON input schema with every field. The MCP server's tool list carries only short descriptions and the main fields; this gives the rest (any tool, listed or not; run unlisted ones with ops_call).

Examples:

```json
{"tools": ["drawing_section_view"]}
```

| field | type | | description |
|---|---|---|---|
| `tools` | array of string | required | Tool names, e.g. ["drawing_section_view", "config_add_input"]. |

#### ops_call

Run any tool by name with its arguments (edits the document (one undoable history entry); deletes or overwrites).

For tools the MCP tool list doesn't show (see ops_list): runs `tool` with `args` exactly as calling it directly would, and returns its reply.

Examples:

```json
{"tool": "drawing_section_view", "args": {"drawing": "Part 1 Drawing 1", "parent": 1, "point": [205.5, 160], "position": [300, 160]}}
```

| field | type | | description |
|---|---|---|---|
| `tool` | string | required | Tool name, e.g. "drawing_section_view". |
| `args` | object |  | Its arguments (see ops_describe). |

### document

#### document_create

Create a new document file (edits the document (one undoable history entry)).

Creates a .noshape document (one SQLite file holding every tab and the whole edit history). Like a new Onshape document it has "Part Studio 1" and "Assembly 1". Becomes the current document. Every later edit is saved immediately as a microversion (there is no Save).

Examples:

```json
{"path": "/tmp/bracket.noshape", "length_unit": "mm"}
```

| field | type | | description |
|---|---|---|---|
| `path` | string | required | Where to create it, e.g. "/tmp/bracket.noshape". Must not exist. |
| `length_unit` | one of `mm`, `cm`, `m`, `in`, `ft`, `yd`, `um` |  | Workspace length unit (default mm): bare numbers in length fields use it. |
| `angle_unit` | one of `deg`, `rad` |  | Workspace angle unit (default deg). |

#### document_open

Open a document file and make it current.

Opens an existing .noshape file. Later calls without `doc` use it.

Examples:

```json
{"path": "/tmp/bracket.noshape"}
```

| field | type | | description |
|---|---|---|---|
| `path` | string | required | Path of the .noshape file. |

#### document_info

Describe a document: tabs, units, history state.

Lists the tabs in tab-bar order (id, name, kind), the workspace units, the branch, whether it is protected (workspace protection) or read-only, and whether undo/redo are available.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |

#### document_close

Close an open document.

Forgets the document in this session (its file stays). Edits are already saved.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |

#### document_protect

Turn on workspace protection for a branch (edits the document (one undoable history entry)).

Versions and history › workspace row › Workspace protection… › Enable workspace protection. A protected workspace takes no direct model edits (geometry, drawings, tabs, settings, undo/redo, restore): those fail with error `protected`. Make changes on another branch and merge them in; versions, branches and merges stay allowed.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `branch` | string |  | Branch name or id (default: the current one). |

#### document_unprotect

Turn off workspace protection for a branch (edits the document (one undoable history entry)).

Clears Enable workspace protection: the workspace takes edits again.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `branch` | string |  | Branch name or id (default: the current one). |

#### document_set_units

Change the workspace units (edits the document (one undoable history entry)).

Document › Workspace units: the units bare numbers take and values are shown in.

Examples:

```json
{"length_unit": "in", "length_precision": 3}
```

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

Examples:

```json
{"kind": "Part Studio", "name": "Housing"}
{"kind": "Assembly"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `kind` | one of `partstudio`, `assembly`, `drawing`, `featurestudio`, `materiallibrary`, `variablestudio` | required | Tab kind. |
| `name` | string |  | Name (default: next default name). |
| `after` | string |  | Tab (name or id) to place the new tab after. |

#### tab_rename

Rename a tab (edits the document (one undoable history entry)).

Examples:

```json
{"tab": "Part Studio 1", "name": "Housing"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string | required | Tab name or id. |
| `name` | string | required | New name. |

#### tab_duplicate

Duplicate a tab (edits the document (one undoable history entry)).

Tab › Duplicate: "<name> Copy 1" right after it.

Examples:

```json
{"tab": "Housing"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string | required | Tab name or id. |

#### tab_delete

Delete a tab (edits the document (one undoable history entry); deletes or overwrites).

As in Onshape, undo does not bring it back (history restore does). Fails if it is the last tab. Other tabs that use it (assembly instances, drawings, derived parts) are listed in the reply; they will show errors.

Examples:

```json
{"tab": "Part Studio 2"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string | required | Tab name or id. |

#### tab_folder_create

Create a tab folder (edits the document (one undoable history entry)).

The tab bar's "+" › Create folder: "Folder 1" (next free number) unless `name` is given, inside `folder` (default: the root). Folders nest up to Onshape's depth limit.

Examples:

```json
{}
{"name": "Fixtures", "folder": "Folder 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `name` | string |  | Folder name (default: the next "Folder N"). |
| `folder` | string |  | Folder (name or id) to create it in (default: the root). |

#### tab_move

Move a tab or tab folder into a folder, out to its parent, or before another tab (edits the document (one undoable history entry)).

Dragging a tab onto a folder tab (or onto 🏠: the root), Tab › Move to parent folder (`to_parent`), or dragging it along the bar (`before`: it goes in front of that tab, in that tab's folder). Like Onshape, the tab order isn't undoable; moving between folders is. Refused for a folder into itself or past the depth limit.

Examples:

```json
{"tab": "Part Studio 1", "folder": "Folder 1"}
{"tab": "Part Studio 1", "to_parent": true}
{"tab": "Assembly 1", "before": "Part Studio 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string | required | Tab or folder (name or id) to move. |
| `folder` | string |  | Target folder (name or id); omit (without `to_parent` / `before`) for the root. |
| `to_parent` | boolean |  | Move to parent folder: out of its folder into the one around it. |
| `before` | string |  | Tab or folder (name or id) to put it in front of. |

#### tab_folder_unpack

Unpack a tab folder (edits the document (one undoable history entry)).

Folder › Unpack: the folder goes; its tabs and folders move to its parent, at the end of the tab list (as Onshape).

Examples:

```json
{"folder": "Folder 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `folder` | string | required | Folder name or id. |

#### tab_activate

Show a tab (click it in the tab bar).

The tab the window shows (`ui_state` › active_tab); with the app open (live mode) its window switches to it.

Examples:

```json
{"tab": "Assembly 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string | required | Tab name or id. |

#### tab_manager

The Tab manager: the tabs matching a search and type filters, sorted.

The Tab manager (Alt+T) as it lists the tabs: `search` (in the names, case-insensitive), `kinds` (type filters: partstudio, assembly, drawing, variables, featurestudio, materiallibrary, file), `sort` (tab_order with folders, name_asc, name_desc, type). `open` (default true) opens it in the window (live mode) with that search; false closes it. `activate` shows one of the rows (Enter on it).

Examples:

```json
{"search": "bracket"}
{"kinds": ["drawing"], "sort": "name_asc"}
{"open": false}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `search` | string |  | Search text. |
| `kinds` | array of string |  | Type filters. |
| `sort` | one of `tab_order`, `name_asc`, `name_desc`, `type` |  | Sort order (default tab_order). |
| `open` | boolean |  | Open (true) or close the panel. |
| `activate` | string |  | Show this tab (name or id). |

### feature

#### feature_types

List the Part Studio feature types.

Every feature of the Part Studio toolbar and tool search (Extrude, Revolve, Fillet, Hole, Sheet metal, Frames...), plus the document's custom FeatureScript features. Use the `type` id with `feature_add`.

Examples:

```json
{"search": "move face"}
{"category": "Detail"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `category` | string |  | Only this category: Sketch, Solid (Create), Detail, Pattern, Transform, Direct, Surface, Curve, Reference, SheetMetal, Frame, Variable, Import, Custom. |
| `search` | string |  | Only types whose name has every word of this (case-insensitive, word starts: "move face", "fill"), or whose id it is; when none, those whose description contains the words. |

#### feature_schema

The parameters of a feature type (JSON schema, defaults, options, units).

The same parameters, defaults, options and visibility rules as the feature's dialog. Parameters shown only for some values of others are ignored while hidden; each says whether the dialog shows it with the defaults and which other parameter's values show or hide it (e.g. hole `holeDepthV3`: "shown with the defaults; hidden, and ignored, when endStyleV2 is UP_TO_NEXT or UP_TO_ENTITY or THROUGH"). `feature_get` lists a feature's currently `visible` parameters. By default only the parameters the dialog shows with the defaults are described; `more_when_shown` names the others (`all` describes every one).

Examples:

```json
{"type": "extrude"}
{"type": "hole"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `type` | string | required | Feature type id, e.g. "extrude". |
| `all` | boolean |  | Describe every parameter, also those the dialog shows only for other values. |

#### feature_add

Add a feature to a Part Studio (what accepting its dialog does) (edits the document (one undoable history entry)).

Creates the feature with the dialog's defaults, applies `params`, makes the dialog's own choices for the fields not given (Extrude / Revolve / Sweep / Loft: Add into the one part the new body touches, with that part as merge scope, else New; a Remove Extrude on a part points into it; Thicken from the faces' parts; Hole: merge scope = the parts it reaches, flipped when more lie behind; a tapped hole's linked depths), inserts it at the rollback bar (or `index`) and regenerates. A selection may be given as a query instead of ids: `{"query": {query_entities arguments}}` (e.g. `"entities": {"query": {"kind": "edges", "direction": [0, 0, 1]}}`), alone or among ids in a list. A pattern's `entities` that are all features (`feature:…`) or faces make it a Feature or Face pattern. As in Onshape, a feature that fails is still added and shown with its error: check `feature_status` and `failed` in the reply. Sketches are made with `sketch_create`.

Examples:

```json
{"type": "extrude", "params": {"entities": ["region:…"], "depth": "25 mm"}}
{"type": "fillet", "params": {"entities": ["edge:…", "edge:…"], "radius": "2 mm"}}
{"type": "extrude", "params": {"entities": ["region:…"], "operationType": "REMOVE", "endBound": "THROUGH_ALL"}}
{"type": "circularPattern", "params": {"entities": ["feature:…"], "axis": ["edge:…"], "instanceCount": 6}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `type` | string | required | Feature type id (see `feature_types`). |
| `params` | object |  | Parameter values by parameter id (see `feature_schema`): booleans; enum option ids ("BLIND"); numbers as expression strings with units ("25 mm", "0.5 in", "30 deg", "#width / 2", "2 * #t + 1 mm"; a bare number uses the workspace units); selections as arrays of reference ids from the query and sketch tools ("region:…", "face:…", "edge:…", "plane:top", "body:…"); array parameters as arrays of objects of their item parameters. |
| `name` | string |  | Feature name (default: "Extrude 1", ...). |
| `index` | integer |  | Position in the feature list (default: at the rollback bar). |

#### feature_edit

Change a feature's parameters or name (what Edit and accepting its dialog does) (edits the document (one undoable history entry)).

Only the given parameters change. Configured parameters go to the active configuration's value (config_activate), as in the dialog.

Examples:

```json
{"feature": "Extrude 1", "params": {"depth": "30 mm"}}
{"feature": "Fillet 1", "name": "Edge rounds"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `feature` | string | required | Feature id or name. |
| `params` | object |  | Parameter values by parameter id (see `feature_schema`): booleans; enum option ids ("BLIND"); numbers as expression strings with units ("25 mm", "0.5 in", "30 deg", "#width / 2", "2 * #t + 1 mm"; a bare number uses the workspace units); selections as arrays of reference ids from the query and sketch tools ("region:…", "face:…", "edge:…", "plane:top", "body:…"); array parameters as arrays of objects of their item parameters. |
| `name` | string |  | New name. |

#### feature_get

A feature's type, parameters and regeneration state.

Parameters in the same JSON forms `feature_add` takes; `visible` lists the ones its dialog currently shows. A custom (FeatureScript) feature also has the `notices` of its last run (message, severity, stack of `at` line:col and `in` module (function), as the FeatureScript notices panel's Monitor shows them) and its `println` `output`.

Examples:

```json
{"feature": "Extrude 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `feature` | string | required | Feature id or name. |

#### feature_list

The feature list: every feature with its status, the parts, failures.

Regenerates the Part Studio and reports each feature (id, name, type, status Ok/Info/Warning/Error/Suppressed/RolledBack, error message; custom features their FeatureScript `notices` and `output`), the parts and the rollback bar.

Examples:

```json
{}
{"tab": "Housing"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |

#### feature_delete

Delete features (edits the document (one undoable history entry); deletes or overwrites).

Features that use them fail with "missing selection" errors, as in Onshape.

Examples:

```json
{"features": ["Fillet 1"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `features` | array of string | required | Feature ids or names. |

#### feature_suppress

Suppress or unsuppress features (edits the document (one undoable history entry)).

Unsuppress also removes a dynamic suppression expression (feature_dynamic_suppression), like the menu's Unsuppress on such a feature.

Examples:

```json
{"features": ["Chamfer 1"]}
{"features": ["Chamfer 1"], "suppressed": false}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `features` | array of string | required | Feature ids or names. |
| `suppressed` | boolean |  | true: suppress, false: unsuppress. |

#### feature_dynamic_suppression

Set or remove a feature's dynamic suppression expression (edits the document (one undoable history entry)).

The Dynamic suppression dialog (a feature's Edit expression… / Remove expression): the feature is active while `unsuppressed_if` is true (an expression of variables and configuration inputs, e.g. "#holes > 2"). Empty or omitted removes the expression (feature_suppress with suppressed false does too). With `folder` instead of `feature`: the folder row's Edit dynamic suppression, written onto every feature in the folder.

Examples:

```json
{"feature": "Hole 1", "unsuppressed_if": "#holes > 0"}
{"feature": "Hole 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `feature` | string |  | Feature id or name. |
| `folder` | string |  | Instead of `feature`: a folder's name (feature_list lists folders). |
| `unsuppressed_if` | string |  | Unsuppressed if: expression. |

#### feature_clear_missing

Clear missing references from a feature's selections (edits the document (one undoable history entry)).

A feature dialog selection's Clear missing references: removes from the feature's selections (`param`, or all of them, rows of lists included) the references whose creating feature was deleted or is suppressed (a derived part's source features count as present; Assembly context geometry is never missing). One edit, as the dialog's ✓ ("Edit : <feature>"). Returns how many were removed.

Examples:

```json
{"feature": "Fillet 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `feature` | string | required | Feature id or name. |
| `param` | string |  | Only this selection field (its parameter id, feature_get). |

#### feature_dependencies

Show dependencies: the features a feature uses and the features using it.

Parents are the earlier features whose geometry, sketches, planes or #variables the feature references; children are the later features referencing it. In history order.

Examples:

```json
{"feature": "Extrude 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `feature` | string | required | Feature id or name. |

#### feature_add_folder

Add features to a new folder (edits the document (one undoable history entry)).

Add selection to folder… (or, without `features`, the list's New folder: an empty folder at the rollback bar). Folders hold a contiguous run: the features gather after the first one. Features all in one folder make a nested folder. feature_list lists folders.

Examples:

```json
{"features": ["Sketch 1", "Extrude 1"], "name": "Base"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `features` | array of string |  | Feature ids or names. |
| `name` | string |  | Folder name. |

#### feature_rename_folder

Rename a feature folder (edits the document (one undoable history entry)).

Examples:

```json
{"folder": "Base", "name": "Base plate"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `folder` | integer or string | required | Folder id or name (feature_list). |
| `name` | string | required | New name. |

#### feature_unpack_folder

Unpack a feature folder (edits the document (one undoable history entry)).

Removes the folder; its features (and sub-folders) stay in place, in the enclosing folder.

Examples:

```json
{"folder": "Base plate"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `folder` | integer or string | required | Folder id or name (feature_list). |

#### feature_rollback

Move the rollback bar (edits the document (one undoable history entry)).

Give `after` (the bar goes below that feature), `index` (number of features before the bar) or neither (to the end). New features are inserted at the bar.

Examples:

```json
{"after": "Extrude 1"}
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `after` | string |  | Feature id or name. |
| `index` | integer |  | Features above the bar. |

#### feature_rename

Rename a feature (edits the document (one undoable history entry)).

Examples:

```json
{"feature": "Extrude 1", "name": "Base"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `feature` | string | required | Feature id or name. |
| `name` | string | required | New name. |

#### feature_reorder

Move features to another place in the list (edits the document (one undoable history entry)).

Like dragging rows in the feature list: a feature moved above something it depends on moves all the same and then fails on its missing selection (as Onshape).

Examples:

```json
{"features": ["Fillet 1"], "to": 3}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `features` | array of string | required | Feature ids or names to move. |
| `to` | integer | required | Row index to drop them at. |

#### feature_set_visibility

Hide or show parts, sketches, planes, mate connectors or default geometry (edits the document (one undoable history entry)).

Targets are reference ids: `body:…` parts (or any face/edge of one), `sketch:…`, `fplane:…`, `mc:…`, `feature:…` (a feature that made parts hides its parts), `plane:top|front|right`, `origin`.

Examples:

```json
{"targets": ["body:…"], "hidden": true}
{"targets": ["plane:top", "sketch:…"], "hidden": false}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `targets` | array of string | required | Reference ids. |
| `hidden` | boolean |  | true: hide, false: show. |

#### feature_size_plane

Size a plane to the Part Studio (plane menu) (edits the document (one undoable history entry)).

Size to Part Studio: the plane is drawn large enough to cover the shown parts (10 % over). `plane`: `plane:top|front|right` or a Plane feature's `fplane:…`.

Examples:

```json
{"plane": "plane:front"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `plane` | string | required | Plane reference id. |

#### feature_show_all

Show all parts, sketches, planes and default geometry (edits the document (one undoable history entry)).

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |

### sketch

#### sketch_create

Create a sketch on a plane or planar face (edits the document (one undoable history entry)).

Inserts a Sketch feature ("Insert feature : Sketch 1") at the rollback bar. Returns its id and plane frame: sketch coordinates (x, y) map to world = origin + x*xdir + y*ydir. On `plane:top` x = world X, y = world Y; on `plane:front` x = X, y = Z; on `plane:right` x = Y, y = Z.

Examples:

```json
{"plane": "plane:top"}
{"plane": "face:…", "name": "Boss profile"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `plane` | string | required | Plane: `plane:top`, `plane:front`, `plane:right`, a Plane feature (`fplane:…`), a planar face id or a mate connector. |
| `name` | string |  | Name (default "Sketch N"). |
| `disable_imprinting` | boolean |  | The dialog's Disable imprinting (default off). |

#### sketch_draw

Draw sketch geometry with a sketch tool (line, rectangle, circle, arc, spline, slot, polygon, ellipse, point...) (edits the document (one undoable history entry)).

Runs the sketch editor's drawing tool on the clicks you give, exactly as clicking in the sketch: inference snaps to existing points/curves within 9 µm and adds the same automatic constraints (coincident, horizontal, vertical, tangent...). Coordinates are sketch-plane (x, y) in workspace units, or length expressions ("10 mm"); a point of three coordinates [x, y, z] is a model (Part Studio) position, projected onto the sketch plane: easiest on a face, whose sketch axes may be turned (the reply's `frame` gives them). One call = one edit of the sketch ("Edit : Sketch 1"); `shapes` draws many shapes in that one edit. Returns the created entity ids (`entity:…`), the sketch's solve state and its regions (`region:…` ids for Extrude etc.). Tools:
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

Examples:

```json
{"sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[0, 0], [80, 40]]}
{"sketch": "Sketch 1", "tool": "circle", "points": [[40, 20], [45, 20]]}
{"sketch": "Sketch 1", "tool": "line", "points": [[0, 0], [30, 0], [30, 20]]}
{"sketch": "Sketch 2", "tool": "point", "points": [[10, 10, 6], [70, 10, 6]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `tool` | one of `line`, `midpoint_line`, `corner_rectangle`, `center_rectangle`, `aligned_rectangle`, `circle`, `three_point_circle`, `three_point_arc`, `tangent_arc`, `center_point_arc`, `point`, `spline`, `bezier`, `conic`, `slot`, `inscribed_polygon`, `circumscribed_polygon`, `ellipse`, `elliptical_arc` |  | Drawing tool (required without `shapes`). |
| `points` | array of array of number or string |  | The clicks, in order: [[x, y], ...] in sketch coordinates, or [[x, y, z], ...] model positions (required without `shapes`). |
| `shapes` | array of object |  | Many shapes in one edit, each drawn exactly as its own call would (instead of tool / points / sides / construction). Much faster for many shapes. |
| `construction` | boolean |  | Make the new geometry construction geometry. |
| `sides` | integer |  | Polygon sides. |
| `tangent_arc_switch` | array of integer |  | line: press the Line/tangent arc switch (Shift+A) before these clicks (0-based): that click draws a tangent arc, then the chain goes on with lines (as in Onshape). |

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

Examples:

```json
{"sketch": "Sketch 1", "constraint": "horizontal", "entities": ["entity:…"]}
{"sketch": "Sketch 1", "constraint": "coincident", "entities": ["entity:…", "origin"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `constraint` | one of `coincident`, `concentric`, `parallel`, `tangent`, `horizontal`, `vertical`, `midpoint`, `perpendicular`, `equal`, `symmetric`, `fix`, `normal`, `pierce`, `curvature` | required | Constraint. |
| `entities` | array of string | required | Sketch entity ids, or the origin (`origin`) to constrain to it. |

#### sketch_dimension

Add a driving dimension (value or expression) to sketch entities (edits the document (one undoable history entry)).

The Dimension tool: placed as a reference, made driving unless that over-constrains the sketch, then set to `value` when given ("20 mm", "#width", "45 deg"; a bare number uses workspace units). Kinds: auto (what the Dimension tool infers from the entities and `label` position), distance (point/line/circle to point/line/circle; `mode` center|minimum|maximum for circles), horizontal_distance, vertical_distance, length (line or arc), radius, diameter, angle (two lines), arc_angle, centerline (twice the distance to a line: revolve diameters).

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"], "value": "80 mm"}
{"sketch": "Sketch 1", "entities": ["entity:…", "origin"], "value": "#offset"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | One or two sketch entity ids (`entity:…`) or `origin`. |
| `value` | string or number |  | Value or expression. Without it the dimension keeps the measured value, as when the Dimension tool's value box is accepted unchanged. |
| `kind` | one of `auto`, `distance`, `horizontal_distance`, `vertical_distance`, `length`, `radius`, `diameter`, `angle`, `arc_angle`, `centerline` |  | Dimension kind (default auto). |
| `mode` | one of `center`, `minimum`, `maximum` |  | Distance to a circle/arc measured to its center (default), nearest or farthest point. |
| `label` | array of number or string |  | Label position in sketch coordinates (also picks the auto kind: above/below two points gives a horizontal distance, beside them a vertical one). |

#### sketch_set_dimension

Change a sketch dimension's value or expression (edits the document (one undoable history entry)).

Like double-clicking a dimension and typing a value. Negative distances and angles flip the geometry, as in Onshape.

Examples:

```json
{"sketch": "Sketch 1", "dimension": "dim:12", "value": "60 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `dimension` | string | required | Dimension id (`dim:…` from sketch_get / sketch_dimension). |
| `value` | string or number | required | Value or expression. |

#### sketch_dimension_options

A sketch dimension's menu: driving / driven, tolerance and precision, configure (edits the document (one undoable history entry)).

The sketch dimension's context menu: Change to driven / driving dimension (`driving`), Tolerance and precision… (`tolerance` none, symmetrical, deviation, limits, min, max or basic, with `upper` / `lower` values and `precision` decimals or "default"; not on angles of some kinds, as the menu), Configure dimension with a List or Checkbox `configure` input (each configuration starts with the current value; set them with config_set_value on the sketch with `param` the reply's `configured_param`), Unconfigure dimension (`unconfigure`).

Examples:

```json
{"sketch": "Sketch 1", "dimension": "dim:12", "driving": false}
{"sketch": "Sketch 1", "dimension": "dim:12", "tolerance": "symmetrical", "upper": "0.05 mm", "precision": 2}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `dimension` | string | required | Dimension id (`dim:…`). |
| `driving` | boolean |  | Driving (true) or driven (false). |
| `tolerance` | one of `none`, `symmetrical`, `deviation`, `limits`, `min`, `max`, `basic` |  | Tolerance type. |
| `upper` | string or number |  | Symmetrical ± / Deviation + / Limits upper value. |
| `lower` | string or number |  | Deviation − / Limits lower value. |
| `precision` | integer or string |  | Decimals 0–8, or "default" (the workspace precision). |
| `configure` | string |  | Configuration input (id or name) to configure the dimension with. |
| `unconfigure` | boolean |  | Unconfigure the dimension. |

#### sketch_delete

Delete sketch entities, constraints or dimensions (edits the document (one undoable history entry); deletes or overwrites).

Deleting a curve deletes its constraints; its points stay when other curves use them.

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"]}
{"sketch": "Sketch 1", "constraints": ["con:7"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string |  | Entity ids. |
| `constraints` | array of string |  | Constraint or dimension ids (`con:…`, `dim:…`). |

#### sketch_set_construction

Toggle construction geometry (edits the document (one undoable history entry)).

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Entity ids. |
| `construction` | boolean |  | true: construction. |

#### sketch_get

A sketch's entities, constraints, dimensions, solve state and regions.

Entities with ids and geometry (sketch coordinates, workspace units), constraints and dimensions (`dim:…`, value and expression), DOF and status (fully defined, under defined, over defined), regions (`region:…` ids, area) for Extrude / Revolve, and the plane frame.

Examples:

```json
{"sketch": "Sketch 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |

#### sketch_paste

Copy a sketch onto another plane (Copy sketch / Paste sketch) (edits the document (one undoable history entry)).

The context menus' Copy sketch then Paste sketch on a plane or planar face: a copy of `sketch` (its entities, constraints and dimensions) on `plane`, at the rollback bar of `to_tab` (default: the sketch's own Part Studio). One edit ("Paste sketch"). Returns the new sketch.

Examples:

```json
{"sketch": "Sketch 1", "plane": "plane:front"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name to copy. |
| `plane` | string | required | Plane to paste onto: `plane:top`, a Plane feature, a planar face id or a mate connector. |
| `to_tab` | string |  | Part Studio to paste into (default: the same one). |

#### sketch_export

Export a sketch or a planar face as DXF (Export as DXF/DWG…).

The context menus' Export as DXF/DWG… of a sketch (its curves; construction curves as hidden lines; external references left out) or of a planar face (its boundary), in the plane's coordinates, millimeters. `format` svg writes the same geometry as SVG.

Examples:

```json
{"sketch": "Sketch 1", "file": "/tmp/profile.dxf"}
{"face": "face:…", "file": "/tmp/face.svg", "format": "svg"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string |  | Sketch feature id or name. |
| `face` | string |  | Instead: a planar face id (`face:…`). |
| `file` | string | required | Output path. |
| `format` | one of `dxf`, `svg` |  | File format. |

#### sketch_copy_entities

Copy sketch entities and paste them into this or another sketch (edits the document (one undoable history entry)).

Copy sketch entities (the curve menu), then Paste sketch entities in the target sketch: the curves with the constraints among them, moved by `offset` (sketch coordinates of the target; the app pastes where they were and lets you drag them). Returns the pasted entities as `created`.

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"], "offset": [20, 0]}
{"sketch": "Sketch 1", "entities": ["entity:…"], "to": "Sketch 2"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Entity ids (`entity:…`) to copy. |
| `to` | string |  | Sketch to paste into (default: the same one). |
| `offset` | array of number or string |  | [x, y] move (lengths; default [0, 0]). |

#### sketch_trim

Trim sketch curves (the Trim tool) (edits the document (one undoable history entry); deletes or overwrites).

Each click in `points` removes the piece of the curve under it, up to the nearest intersections (a curve crossing nothing is deleted; a lone point is deleted). With `path`, the pointer is dragged along the points instead and every curve the path crosses is trimmed. Give `entities` (same length as `points`) to say which curve each click is on; otherwise the curve within 6 µm of the click is used.

Examples:

```json
{"sketch": "Sketch 1", "points": [[15, 0]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `points` | array of array of number or string |  | Clicks on the pieces to remove, in sketch coordinates. |
| `entities` | array of string |  | The curve under each click (optional). |
| `path` | array of array of number or string |  | Drag path (2 or more points): trims every curve it crosses. |

#### sketch_extend

Extend lines or arcs to the next curve (the Extend tool) (edits the document (one undoable history entry)).

Each click extends the line or arc under it, from its end nearest the click, to the next curve it meets. `entities` (optional, same length) names the curve under each click.

Examples:

```json
{"sketch": "Sketch 1", "points": [[29, 10]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `points` | array of array of number or string | required | Clicks near the end to extend. |
| `entities` | array of string |  | The curve under each click (optional). |

#### sketch_split

Split sketch curves at points (the Split tool) (edits the document (one undoable history entry); deletes or overwrites).

Each click splits the curve under it at the closest point on the curve. `entities` (optional, same length) names the curve under each click.

Examples:

```json
{"sketch": "Sketch 1", "points": [[20, 0]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `points` | array of array of number or string | required | Split positions. |
| `entities` | array of string |  | The curve under each click (optional). |

#### sketch_spline_point

Add a control point to a spline or bezier (edits the document (one undoable history entry)).

The Spline control point tool: inserts a point into the spline / bezier at the click.

Examples:

```json
{"sketch": "Sketch 1", "entity": "entity:…", "point": [12, 4]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entity` | string | required | Spline or bezier entity id. |
| `point` | array of number or string | required | Where on the curve (or its control polygon). |

#### sketch_fillet

Fillet sketch corners (the Sketch fillet tool) (edits the document (one undoable history entry)).

Each corner point in `entities` is filleted; two curves given one after the other are filleted where they meet (the tool's two-curve mode). All fillets of one call are one tool session: the first gets a radius dimension set to `radius` (a value or expression), later ones are constrained equal to it, as in Onshape.

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…", "entity:…"], "radius": "5 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Corner points (entity ids), or pairs of curves. |
| `radius` | string or number |  | Radius (default: the tool's, as in Onshape: 5 mm (0.5 in in inch documents) at first, then the last radius used in this Part Studio). |

#### sketch_chamfer

Chamfer sketch corners (the Sketch chamfer tool) (edits the document (one undoable history entry)).

Like sketch_fillet, with an equal-distance chamfer: each corner point (or pair of curves) gets a chamfer line, the first with distance dimensions set to `distance`.

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…", "entity:…"], "distance": "2 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Corner points, or pairs of curves. |
| `distance` | string or number |  | Chamfer distance (default: the tool's, 5 mm / 0.25 in). |

#### sketch_offset

Offset sketch curves (the Offset tool) (edits the document (one undoable history entry)).

Offsets the curves (a connected chain offsets as one) by `distance`. The default side is the one the tool's arrow shows first; `toward` picks the side whose offset passes nearer that point, `flip` flips the side.

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"], "distance": "3 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Curves to offset. |
| `distance` | string or number |  | Offset distance (default: the tool's, 5 mm / 0.25 in). |
| `toward` | array of number or string |  | A point on the side to offset to. |
| `flip` | boolean |  | Offset to the other side. |

#### sketch_mirror

Mirror sketch entities about a line (the Mirror tool) (edits the document (one undoable history entry)).

Copies the entities mirrored about `axis` and adds symmetric constraints. The axis is a sketch line, or model geometry (an edge, a plane) whose projection is used.

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"], "axis": "entity:…"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Entities to mirror. |
| `axis` | string | required | Mirror line: a sketch line id, or an edge / plane id. |

#### sketch_linear_pattern

Pattern sketch entities in a line or grid (the sketch Linear pattern tool) (edits the document (one undoable history entry)).

Copies the entities `count` times (the seed included) `distance` apart along `angle`, optionally in a second direction (`second_count`, `second_distance`, `second_angle`, default 90 deg). Counts are 1 to 100. Give `pattern` (an id from the reply or sketch_get) to edit an existing pattern instead.

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"], "count": 4, "distance": "15 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
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

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"], "count": 6, "center": [0, 0]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string |  | Seed entities (not needed when editing). |
| `center` | array or string |  | Center: [x, y], a point entity id or `origin` (default: the origin). |
| `count` | integer |  | Instances (default 3). |
| `angle` | string or number |  | Total angle (default 360 deg). |
| `pattern` | integer |  | Pattern id to edit. |

#### sketch_transform

Move, rotate or scale sketch entities (the Transform tool) (edits the document (one undoable history entry)).

Rotates by `rotate` and scales by `scale` about `center` (default: the selection's center), then translates by `translate`, as the tool's triad does.

Examples:

```json
{"sketch": "Sketch 1", "entities": ["entity:…"], "translate": [10, 0]}
{"sketch": "Sketch 1", "entities": ["entity:…"], "rotate": "45 deg"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `entities` | array of string | required | Entities to transform. |
| `translate` | array of number or string |  | Translation [dx, dy]. |
| `rotate` | string or number |  | Rotation angle (counter-clockwise). |
| `scale` | number |  | Scale factor. |
| `center` | array of number or string |  | Pivot [x, y]. |

#### sketch_use

Use (project) model geometry into the sketch (the Use tool) (edits the document (one undoable history entry)).

Projects each reference onto the sketch plane as fixed geometry linked to the model (it follows model changes): edges and vertices (`edge:…`, `vertex:…` from query_entities), other sketches' entities, a planar face (all its edges), a curved face (its silhouettes) or a plane (its trace line). Returns the created entity ids.

Examples:

```json
{"sketch": "Sketch 2", "references": ["edge:…"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `references` | array of string | required | Model reference ids. |

#### sketch_intersection

Add a face's intersection with the sketch plane (the Intersection tool) (edits the document (one undoable history entry)).

Sections the face with the sketch plane; the curves stay linked to the face.

Examples:

```json
{"sketch": "Sketch 2", "face": "face:…"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `face` | string | required | Face id (`face:…`) crossing the sketch plane. |

#### sketch_text

Add or edit sketch text (the Text tool) (edits the document (one undoable history entry)).

Places a text box with its lower-left corner at `corner` and `height` (default 10 mm), with the Text dialog's options; or, with `edit` (an entity of an existing text), changes that text. Text is limited to 250 characters.

Examples:

```json
{"sketch": "Sketch 1", "text": "REV A", "corner": [0, 0], "height": "5 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
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

Examples:

```json
{"sketch": "Sketch 1", "file": "/tmp/outline.dxf", "units": "mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `file` | string | required | Path of the .dxf / .dwg file. |
| `units` | one of `From file`, `Millimeter`, `Centimeter`, `Meter`, `Inch`, `Foot`, `Yard` |  | Drawing units (default "From file"; millimeters when the file has none). |
| `use_origin` | boolean |  | Keep the file's origin at the sketch origin. |

#### sketch_insert_image

Import an image into the sketch (Insert image) (edits the document (one undoable history entry)).

Places a PNG, JPEG, GIF or BMP image in the rectangle from the first corner toward the second (the aspect ratio is kept, as when drawing the rectangle).

Examples:

```json
{"sketch": "Sketch 1", "file": "/tmp/logo.png", "points": [[0, 0], [40, 20]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `sketch` | string | required | Sketch feature id or name. |
| `file` | string | required | Path of the image. |
| `points` | array of array of number or string | required | [corner, opposite corner]. |

### variable

#### variable_list

The variables of a Part Studio or a Variable Studio, with their values.

Part Studio: Variable features (#name) in feature order with kind, expression, evaluated value and description, plus the values of every variable visible at the end of the feature list (including Variable Studio and configuration variables). Variable Studio (`tab` names it): its table rows in order (name, type, expression, value, description).

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Variable Studio tab, by name or id. Default: the first Part Studio. |
| `copy` | boolean |  | Part Studio: the Variables panel's Copy table, `copied` being the text it puts on the clipboard (a header row, then name, type, value, description per variable, tab-separated). A row's Copy copies its description (`description` here). |

#### variable_create

Create a variable (#name) in a Part Studio or a Variable Studio (edits the document (one undoable history entry)).

Part Studio: the Variable table's add row, a Variable feature at the rollback bar. Variable Studio: the table's last row ("Add variable", a Length variable), then its type, value and description cells, one edit each, as typed in the table. Use it in any expression as #name.

Examples:

```json
{"name": "wall", "value": "2.5 mm"}
{"name": "holes", "kind": "INTEGER", "value": 4}
```

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

Examples:

```json
{"variable": "wall", "value": "3 mm"}
```

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

Examples:

```json
{"variable": "wall"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Variable Studio tab, by name or id. Default: the first Part Studio. |
| `variable` | string | required | Variable name or feature id. |

#### variable_move

Move a variable up or down in the Variable table or a Variable Studio (edits the document (one undoable history entry)).

Examples:

```json
{"variable": "wall", "by": -1}
```

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

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |

#### config_add_input

Add a configuration input (List, Checkbox or configuration Variable) (edits the document (one undoable history entry)).

"Configure Part Studio" › List / Checkbox / Variable. A List starts with one configuration "Default" (add more with config_add_option). A Variable input is a #variable whose value each configuration can set.

Examples:

```json
{"kind": "list", "name": "Size"}
{"kind": "variable", "name": "length", "value": "40 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `kind` | one of `list`, `checkbox`, `variable` | required | Input kind. |
| `name` | string |  | Name (default "Configuration", "List 2", "Checkbox 1"...; required for a variable: its #name). |
| `variable_kind` | one of `LENGTH`, `ANGLE`, `INTEGER`, `REAL`, `TEXT` |  | Variable: kind (default LENGTH). |
| `value` | string or number |  | Variable: default value expression. |
| `range` | array of string |  | Variable: [min, max] expressions. |

#### config_edit_input

Rename a configuration input or change a configuration variable's default and range (edits the document (one undoable history entry)).

Examples:

```json
{"input": "Size", "name": "Length class"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | Input id or name. |
| `name` | string |  | New name. |
| `value` | string or number |  | Variable input: new default. |
| `range` | array of string |  | Variable input: new [min, max] expressions (with `value`). |

#### config_delete_input

Delete a configuration input and what it configures (edits the document (one undoable history entry); deletes or overwrites).

Examples:

```json
{"input": "Size"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | Input id or name. |

#### config_add_option

Add a configuration to a List input (edits the document (one undoable history entry)).

Returns its id as `option`. Configured parameters take the default configuration's value in it until set with config_set_value.

Examples:

```json
{"input": "Size", "name": "Large"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | List input id or name. |
| `name` | string | required | Configuration name (e.g. "Large"). |

#### config_edit_option

Rename or delete a configuration of a List input (edits the document (one undoable history entry)).

A List keeps at least one configuration.

Examples:

```json
{"input": "Size", "option": "Large", "name": "XL"}
{"input": "Size", "option": "XL", "delete": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | List input id or name. |
| `option` | string | required | Configuration id or name. |
| `name` | string |  | New name. |
| `delete` | boolean |  | Delete it. |

#### config_configure

Make a parameter (or suppression) of a feature, assembly instance or mate configured by a List or Checkbox input (edits the document (one undoable history entry)).

Every configuration starts with the current value; then set values with config_set_value. Part Studio: `feature` and `param`, a parameter id of the feature (see feature_schema) or "suppressed" (Configure suppression). Assembly: `instance` with `param` "suppressed", or `mate` with "suppressed", "offset", "offset_x", "offset_y", "offset_z", "rotation" or "flip_primary".

Examples:

```json
{"feature": "Extrude 1", "param": "depth", "input": "Size"}
{"feature": "Fillet 1", "param": "suppressed", "input": "Lite"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `feature` | string |  | Feature id or name (Part Studio). |
| `instance` | string |  | Instance id or name (Assembly). |
| `mate` | string |  | Mate id or name (Assembly). |
| `param` | string | required | Parameter id, or "suppressed". |
| `input` | string | required | List or Checkbox input id or name. |

#### config_set_value

Set a configured parameter's value in one configuration (edits the document (one undoable history entry)).

`configuration` is the List configuration's id or name, or true/false for a Checkbox. `value` uses the feature parameter's JSON form ("40 mm", an enum id, true/false...).

Examples:

```json
{"feature": "Extrude 1", "param": "depth", "configuration": "Large", "value": "40 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `feature` | string |  | Feature id or name (Part Studio). |
| `instance` | string |  | Instance id or name (Assembly). |
| `mate` | string |  | Mate id or name (Assembly). |
| `param` | string | required | Parameter id, or "suppressed" (see config_configure). |
| `configuration` | string or boolean | required | Configuration id/name, or true/false. |
| `value` | any | required | The value. |

#### config_activate

Choose the configuration the Part Studio shows (the Configurations box).

Sets the active value of configuration inputs; later regenerations, queries, renders, exports and feature edits use it (feature_edit then edits that configuration's configured values). `values` maps each input (id or name) to a List configuration (id or name), true/false for a Checkbox, or an expression for a configuration Variable ("40 mm"). Inputs not given keep their current value; `reset: true` returns every input to its default first. View state: no history entry, as in the app.

Examples:

```json
{"values": {"Size": "Large"}}
{"reset": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `values` | object |  | Input -> value. |
| `reset` | boolean |  | Start from the default configuration. |

#### config_copy_inputs

Copy configuration inputs from another Part Studio or an Assembly (edits the document (one undoable history entry)).

Copy input / Copy all inputs in `from` (a Part Studio, or an Assembly's Copy all inputs), then Paste inputs in this Part Studio: the inputs whose ids this Part Studio doesn't have yet (with their options and visibility conditions; conditions on inputs not pasted are repaired). Inputs with the same id can then drive Derived features (config_propagate).

Examples:

```json
{"from": "Part Studio 1", "tab": "Part Studio 2"}
{"from": "Assembly 1", "tab": "Part Studio 1"}
{"from": "Part Studio 1", "tab": "Part Studio 2", "inputs": ["Size"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `from` | string | required | Part Studio or Assembly to copy from. |
| `inputs` | array of string |  | Input ids or names (default all). |

#### config_duplicate_input

Assembly input ⋯ › Copy input: a copy of the input (edits the document (one undoable history entry)).

The Assembly's Configuration panel, input ⋯ › Copy input: a copy "<name> copy" of `input` (new ids, the same options) after the others. One edit ("Copy configuration input"). In a Part Studio, Copy input copies to the clipboard: use config_copy_inputs.

Examples:

```json
{"tab": "Assembly 1", "input": "Configuration"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | Input id or name. |

#### config_propagate

Input propagation: drive Derived features' configuration from this Part Studio's input (edits the document (one undoable history entry)).

Input propagation…: for a List or Checkbox `input`, the Derived features whose source has an input with the same id and kind take each option's same option (a configured column per feature). `features` limits it (default all candidates). On an Assembly: the instances whose source Part Studio or assembly has an input with the same id and kind follow the assembly's input (`instances` limits it). The reply lists the candidates and which are propagated.

Examples:

```json
{"input": "Size"}
{"input": "Size", "features": ["Derived 1"]}
{"tab": "Assembly 1", "input": "Size", "instances": ["Part 1 <1>"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | Input id or name. |
| `features` | array of string |  | Part Studios: Derived features (ids or names). |
| `instances` | array of string |  | Assemblies: instances (ids or names). |

#### config_edit_ids

Edit FeatureScript IDs of a configuration input and its options (edits the document (one undoable history entry)).

Edit FeatureScript IDs…: the input's id and, for a List, its options' ids (`options`: {current option id or name: new id}); configured values follow.

Examples:

```json
{"input": "Size", "id": "size", "options": {"Large": "large"}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | Input id or name. |
| `id` | string |  | New input id. |
| `options` | object |  | Option renames: {option: new id}. |

#### config_visibility

Set an input's visibility conditions (edits the document (one undoable history entry)).

Visibility conditions…: when the input shows and which List options do. `visibility` is {"conditions": [{"input": id, "test": {"Is": [option ids or "true"/"false"]} | {"Between": [a, b]}}], "option_rules": [{"show": {"Options": [ids]} | {"Range": [a, b]}, "always": bool, "conditions": [...]}]}; null: always shown. config_list shows each input's `visible_when`.

Examples:

```json
{"input": "Hole size", "visibility": {"conditions": [{"input": "With_holes", "test": {"Is": ["true"]}}], "option_rules": []}}
{"input": "Hole size", "visibility": null}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | Input id or name. |
| `visibility` | object or null |  | The conditions; omitted or null: always shown. |

#### config_reorder_inputs

Move a configuration input onto another's place (edits the document (one undoable history entry)).

Examples:

```json
{"input": "Lite", "to": "Size"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `input` | string | required | Input to move. |
| `to` | string | required | Input whose place it takes. |

### assembly

#### assembly_insert

Insert a part (or a whole Part Studio, or a subassembly) into an Assembly (edits the document (one undoable history entry)).

The Insert dialog. Inserting the same item again offsets the new instance along X so they don't overlap. Returns the instance id; place it with mates or assembly_transform.

Examples:

```json
{"source": "Part Studio 1", "part": "Part 1"}
{"source": "Part Studio 1"}
{"source": "Bracket", "configuration": {"Size": "Large"}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `source` | string | required | Part Studio or Assembly tab (name or id) to insert from. |
| `part` | string |  | A part of that Part Studio (`body:…` id or part name); default: the whole Part Studio as one instance. |
| `sketch` | string |  | A sketch of that Part Studio (feature id or name): the Insert dialog's sketches. |
| `configuration` | object or string |  | Configuration of the source: {input id: value}, or the Insert dialog's "default" (Reset to default) / "last_viewed" (Set to last viewed: the configuration the source shows). |

#### assembly_list

An assembly's instances, mates and degrees of freedom.

Solves the assembly and lists the instances (id, name, source, placement: translation in workspace units and rotation quaternion xyzw, fixed/hidden/suppressed, remaining DOF) and the mates (id, name, type, ok, error message).

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |

#### assembly_entities

An instance's faces, edges or vertices, in assembly (world) coordinates, with ids for mates.

Like query_entities for one instance: ids, surface/curve type, centroid/midpoint, normal, axis and radius, all placed where the instance is in the assembly. The same filters, in assembly coordinates: `surface`, `curve`, `normal`, `direction`, `near` (closest first) and `at` (the closest entity to each point, in order).

Examples:

```json
{"instance": "Part 1 <1>", "kind": "faces", "surface": "cylinder"}
{"instance": "Bracket <1>", "kind": "faces", "at": [[0, 0, 5], [30, 8.8, 9]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `kind` | one of `faces`, `edges`, `vertices` | required | Which entities. |
| `surface` | string |  | Faces: only this surface type (plane, cylinder, ...). |
| `curve` | one of `line`, `circle`, `other` |  | Edges: only this curve type. |
| `normal` | array of number |  | Faces: outward normal in assembly coordinates. |
| `direction` | array of number |  | Edges: lines parallel to this direction (either way) or circles around it, within 1°. |
| `near` | array of number or string |  | Sort by distance from this point (assembly coordinates, workspace units). |
| `at` | array of array of number or string |  | Points: the closest entity to each, in order (replaces `near` / `limit`). |
| `limit` | integer |  | At most this many (default 200). |

#### assembly_connectors

The implicit mate connector points on an instance's face, edge or vertex.

The points Onshape offers while hovering an entity (centroid, vertices, edge midpoints, circle centers, cylinder axis ends...), each with its world position and Z axis, ready to pass as a mate connector.

Examples:

```json
{"instance": "Part 1 <1>", "entity": "face:…"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `entity` | string | required | Face, edge or vertex id from assembly_entities. |

#### assembly_mate

Add or edit a mate between mate connectors (Fastened, Revolute, Slider, Planar, Cylindrical, Pin slot, Ball, Parallel, Tangent, Width) (edits the document (one undoable history entry)).

The mate dialog: picks two connectors, solves the mate (the first connector's instance moves to the second, as in Onshape), and commits. A mate connector: {"instance": instance id or name, "entity": a face/edge/vertex id of that instance (from assembly_entities), "inference": where on it (center, centroid, midpoint, vertex, top_axis, mid_axis, bottom_axis, origin; default: the entity's first implicit point, its centroid or center)}, or a candidate from assembly_connectors {"instance", "reference", "align"}, or {"origin": true} for the assembly origin, or {"connector": an assembly mate connector id or name}. Optional "flip": true flips that connector's primary axis. Offsets are expressions ("5 mm", "90 deg").

Examples:

```json
{"type": "fastened", "a": {"instance": "Base <1>", "entity": "face:…"}, "b": {"origin": true}}
{"type": "revolute", "a": {"instance": "Pin <1>", "entity": "face:…"}, "b": {"instance": "Base <1>", "entity": "face:…"}}
{"mate": "Revolute 1", "limits": {"rz": {"min": "-30 deg", "max": "45 deg"}}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `mate` | string |  | Edit this existing mate (id `mate:…` or name), as its Edit… dialog: fields not given keep their values. Without it a new mate is added (`type`, `a`, `b` required). |
| `type` | one of `fastened`, `revolute`, `slider`, `planar`, `cylindrical`, `pinslot`, `ball`, `parallel`, `tangent`, `width` |  | Mate type. |
| `a` | object |  | First connector (its instance moves). Width: the first tab connector. |
| `b` | object |  | Second connector. Width: the first width connector. |
| `c` | object |  | Width: the second width connector. |
| `d` | object |  | Width: the second tab connector (optional). |
| `name` | string |  | Name (default "Fastened 1", ...). |
| `flip_primary` | boolean |  | Flip primary axis (default: keep the moving instance's current orientation, like the dialog). |
| `reorient_secondary` | integer |  | Reorient secondary axis: quarter turns about the primary axis (the dialog's button clicks, 0-3). |
| `offset` | object |  | Offset: {"x", "y", "z": length, "rotation": angle} expressions where the mate type allows them. |
| `rotation_axis` | one of `x`, `y`, `z` |  | Axis of the rotation offset. |
| `limits` | object |  | Limits: {"<axis>": {"min": expr, "max": expr}} for the mate type's axes (Revolute rz; Slider z; Cylindrical z, rz; Pin slot x, rz; Planar x, y, rz; Parallel z; Ball cone). |
| `tangent_propagation` | boolean |  | Tangent mates: Tangent propagation. |
| `solve_all` | boolean |  | Solve every mate (the dialog's Solve) instead of only moving this mate's first instance. |
| `reset` | boolean |  | With `mate`: the mate menu's Reset, its degrees of freedom back to 0 ("Reset : <mate>"; not Fastened, Tangent or Width). |

#### assembly_transform

Move an instance to a placement (edits the document (one undoable history entry)).

Sets the instance's translation (workspace units) and optional rotation (axis + angle) as when dragging it or using Transform; mates then pull mated instances.

Examples:

```json
{"instance": "Part 1 <2>", "translation": ["50 mm", 0, 0]}
{"instance": "Part 1 <2>", "translation": [0, 0, 0], "axis": [0, 0, 1], "angle": "90 deg"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `translation` | array of number or string | required | [x, y, z]. |
| `axis` | array of number |  | Rotation axis. |
| `angle` | number or string |  | Rotation angle ("90 deg"). |

#### assembly_set_state

Fix/unfix, hide/show or suppress/unsuppress instances (edits the document (one undoable history entry)).

Examples:

```json
{"instances": ["Base <1>"], "fixed": true}
{"instances": ["Cover <1>"], "hidden": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string | required | Instance ids or names. |
| `fixed` | boolean |  | Fix or unfix. |
| `hidden` | boolean |  | Hide or show. |
| `suppressed` | boolean |  | Suppress or unsuppress. |

#### assembly_delete

Delete instances, mates or other assembly features (edits the document (one undoable history entry); deletes or overwrites).

Examples:

```json
{"instances": ["Part 1 <3>"]}
{"mates": ["Fastened 2"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string |  | Instance ids or names. |
| `mates` | array of string |  | Mate, relation, group, mate connector or pattern ids or names (a pattern takes its instances with it). |

#### assembly_pattern

Pattern instances: linear, circular, mirror, part pattern or replicate (edits the document (one undoable history entry)).

The Linear / Circular pattern, Mirror, Part pattern and Replicate dialogs. Linear copies the seeds along a direction (an edge, a face's normal or a mate connector's Z axis), optionally in a second direction; circular copies them about an axis; mirror copies them across a plane. Part pattern re-creates the seed's mate on every copy of its target made by a Part Studio pattern; replicate re-creates it on all matching geometry (every hole of the same size). Give `edit` to change an existing pattern: fields left out keep their values. One edit ("Insert : Linear pattern 1").

Examples:

```json
{"kind": "circular", "seeds": ["Bolt <1>"], "direction": {"instance": "Base <1>", "entity": "edge:…"}, "count": 6}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
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

Examples:

```json
{"pattern": "Circular pattern 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `pattern` | string | required | Pattern id or name. |

#### assembly_bom

The Bill of Materials of an Assembly.

The BOM table with the assembly's columns and type (flattened or structured), as the BOM panel shows it; `csv` writes it the way ⋯ › Export to CSV does. `type`, `columns`, `show_excluded` and `show_top_level` override the stored table settings for this call only; assembly_bom_settings changes them.

Examples:

```json
{}
{"type": "structured", "csv": "/tmp/bom.csv"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `type` | one of `flattened`, `structured` |  | BOM type. |
| `columns` | array of string |  | Columns by name ("Item", "Quantity", "Part number", "Material", "Mass"...). |
| `show_excluded` | boolean |  | Show excluded and suppressed items. |
| `show_top_level` | boolean |  | Show the top level assembly row. |
| `csv` | string |  | Write the table to this CSV file. |
| `copy` | boolean |  | Copy table (the BOM's ⋯ menu): `copied` is the text it puts on the clipboard (tab-separated rows), with its toast message. |

#### assembly_bom_settings

Change the BOM table's type and columns (edits the document (one undoable history entry)).

The BOM panel's settings: BOM type, the columns (Item stays first) and the ⋯ menu's Show excluded/suppressed and Show top level assembly row. Each change is its own edit, as in the panel ("BOM type", "Add column Material", "Move left"...).

Examples:

```json
{"type": "flattened", "columns": ["Item", "Quantity", "Part number", "Name"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `type` | one of `flattened`, `structured` |  | BOM type. |
| `columns` | array of string |  | The columns in order (Item is always first). |
| `show_excluded` | boolean |  | Show excluded and suppressed items. |
| `show_top_level` | boolean |  | Show the top level assembly row. |

#### assembly_interference

Interference detection: overlaps between instances.

Analysis tools › Interference detection: the overlapping solid parts among the given top-level instances (default: all), with the overlap's volume and size. Parts of one Part Studio instance aren't checked against each other.

Examples:

```json
{}
{"instances": ["Bolt <1>", "Base <1>"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string |  | Top-level instances (ids or names); default all. |
| `include_standard` | boolean |  | Include standard content (fasteners). |
| `top_only` | boolean |  | Show top level only: one row per pair of top-level instances. |

#### assembly_change_to_version

Change to version: instances show a version of this document (edits the document (one undoable history entry)).

Instance › Change to version… (the Reference manager): the `instances` (default all) that reference this document's workspace (parts, Part Studios, subassemblies) show version `version` instead ("Change to version : V1"); instances already showing a version of this document move to `version` ("Update to V1"). Versions: history_create_version. Sketch, mirrored and standard content instances are left alone.

Examples:

```json
{"version": "V1"}
{"version": "V2", "instances": ["Part 1 <1>"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `version` | string | required | Version name. |
| `instances` | array of string |  | Instance ids or names (default all). |

#### assembly_group

Group instances so they move as one rigid body (edits the document (one undoable history entry)).

The Group dialog (at least two instances). Give `edit` to change an existing group's instances. One edit ("Insert : Group" / "Edit : Group").

Examples:

```json
{"instances": ["Cover <1>", "Screw <1>"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string | required | Instance ids or names (two or more). |
| `edit` | string |  | An existing group to edit (id or name). |

#### assembly_relation

Couple two mates: gear, rack and pinion, screw or linear relation (edits the document (one undoable history entry)).

The mate relation dialogs. Gear couples two revolute (or cylindrical) mates by `ratio`; rack and pinion a revolute and a slider by the distance per revolution; screw one cylindrical mate's rotation and translation (one mate); linear two sliders. The coupling is measured from the mates' current positions. One edit ("Insert : Gear relation").

Examples:

```json
{"type": "gear", "mates": ["Revolute 1", "Revolute 2"], "ratio": 2}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `type` | one of `gear`, `rack_and_pinion`, `screw`, `linear` | required | Relation type. |
| `mates` | array of string | required | The two mates (ids or names); screw: one cylindrical mate. |
| `ratio` | string or number |  | Gear / linear: the ratio (default 1); rack and pinion / screw: the distance per revolution (default 10 mm / 1 mm). |
| `reverse` | boolean |  | Reverse direction. |
| `edit` | string |  | An existing relation to edit (id or name). |

#### assembly_mate_connector

Create or edit an assembly mate connector (edits the document (one undoable history entry)).

The Mate connector dialog: a connector on an instance's geometry (or the assembly origin), moved with its instance; mates use it as {"connector": "mc:<id>" or its name}. One edit ("Insert : Mate connector 1").

Examples:

```json
{"on": {"instance": "Base <1>", "entity": "face:…", "inference": "center"}, "name": "Top center"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
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

Examples:

```json
{"instance": "Bolt <1>", "source": "Part Studio 2", "part": "Long bolt"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `source` | string | required | Part Studio or Assembly tab (name or id). |
| `part` | string |  | A part of that Part Studio (`body:…` id or part name); default: the whole Part Studio. |
| `configuration` | object |  | Configuration of the source: {input id: value}. |
| `all` | boolean |  | Replace all instances of the same source. |

#### assembly_rename

Rename an instance, mate, relation, group, pattern or mate connector (edits the document (one undoable history entry)).

Rename in the instance list. One edit ("Rename : <new name>").

Examples:

```json
{"instance": "Part 1 <1>", "name": "Base"}
{"feature": "Revolute 1", "name": "Hinge"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string |  | Instance id or name. |
| `feature` | string |  | Mate, relation, group, pattern or connector id or name. |
| `name` | string | required | The new name. |

#### assembly_set_feature_state

Suppress or unsuppress mates, relations, groups and patterns; show or hide mate connectors (edits the document (one undoable history entry)).

The list's Suppress / Unsuppress ("Suppress mate", "Suppress pattern"; a configured mate changes in the default configuration) and the mate connectors' eye ("Hide mate connector"). Features already in the asked state are left alone.

Examples:

```json
{"features": ["Slider 1"], "suppressed": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `features` | array of string | required | Feature ids or names. |
| `suppressed` | boolean |  | Suppress or unsuppress. |
| `hidden` | boolean |  | Mate connectors: hide or show. |

#### assembly_named_position

Named positions: add, update, apply, duplicate, rename, reorder, delete, set a driving value (edits the document (one undoable history entry)).

The Named positions panel. `add` stores the current state ("Named position 1"...); `update` stores the current state in an existing position; `apply` moves the assembly to a position (its driving mates' values are imposed, the other mates stay); `set_value` sets a driving mate's value in a position (see assembly_named_position_drivers). Each action is one edit with the panel's text ("Insert : Named position 1", "Apply named position : Open"...).

Examples:

```json
{"action": "add", "name": "Open"}
{"action": "apply", "position": "Open"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `action` | one of `add`, `update`, `apply`, `duplicate`, `delete`, `rename`, `move_up`, `move_down`, `set_value` | required | What to do. |
| `position` | string |  | The position: its name or 1-based row (all actions but add). |
| `name` | string |  | add: the new position's name; rename: the new name. |
| `mate` | string |  | set_value: the driving mate (id or name). |
| `dof` | one of `rz`, `tx`, `ty`, `tz` |  | set_value: the mate's value: rz = rotation about Z, tx / ty / tz = translation. |
| `value` | string or number |  | set_value: the value (angle or length expression). |

#### assembly_named_position_drivers

Choose the mates (and their degrees of freedom) that drive named positions (edits the document (one undoable history entry)).

The Driving mate selector: each driver is a mate and one of its free values (revolute: rz; slider: tz; cylindrical: tz, rz; pin slot: tx, rz; planar: tx, ty, rz; parallel: tx, ty, tz, rz). Replaces the list. One edit ("Driving mate selector").

Examples:

```json
{"drivers": [{"mate": "Revolute 1", "dof": "rz"}]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `drivers` | array of object | required | [{"mate": id or name, "dof": "rz" \| "tx" \| "ty" \| "tz"}] |

#### assembly_exploded_view

Exploded views: add, rename or delete a view or a step (edits the document (one undoable history entry); deletes or overwrites).

The Exploded views panel: `add` creates "Explode N" (renamed to `name` if given); `rename` renames a view (or its `step`); `delete` deletes a view (or its `step`); `lines` (with `on`) is the view's Show all explode lines / Hide all explode lines (every step's lines; one edit with that name). Add steps with assembly_explode_step.

Examples:

```json
{"action": "add", "name": "Exploded 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `action` | one of `add`, `rename`, `delete`, `lines` | required | What to do. |
| `on` | boolean |  | lines: show (true) or hide (false) all explode lines. |
| `view` | string |  | The view: its name or 1-based row. |
| `step` | integer |  | A step of the view (1-based). |
| `name` | string |  | add / rename: the name. |

#### assembly_explode_step

Add or edit an explode step: move or rotate instances (edits the document (one undoable history entry)).

An explode step of a view: translate the instances `distance` along `direction`, or rotate them by `angle` about `direction` (through `pivot`, the picked connector's origin, or the instances' center). `direction` is a world vector [x, y, z] or a mate connector (its Z axis). One edit ("Edit : Explode step").

Examples:

```json
{"view": "Exploded 1", "instances": ["Cover <1>"], "direction": {"instance": "Cover <1>", "entity": "face:…"}, "distance": "40 mm"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `view` | string | required | The view: its name or 1-based row. |
| `instances` | array of string | required | Instance ids or names. |
| `direction` | array or object | required | Direction / rotation axis: [x, y, z] or a mate connector (see assembly_mate). |
| `distance` | string or number |  | Translate by this distance. |
| `angle` | string or number |  | Rotate by this angle instead. |
| `pivot` | array of number |  | Rotation pivot [x, y, z] in document units. |
| `lines` | boolean |  | Show explode lines. |
| `step` | integer |  | Edit this step (1-based) instead of adding one. |

#### assembly_animate

Animate a mate: the assembly at a step of the Animate dialog's run (not saved).

The mate context menu's Animate… dialog: the mate's degree of freedom `dof` runs from `start` to `end` in `steps` steps (defaults as the dialog fills them: the mate's limits, else 0 → 360 deg or 0 → 25 mm; 300 steps). Returns the chooser's degrees of freedom, the run's values and the instances' placements at step `frame` (value = start + (end − start)·frame/steps; Single playback stops on the last step, Loop and Reciprocate repeat these). Like the dialog, nothing is saved: the assembly goes back to its positions when the dialog closes.

Examples:

```json
{"mate": "Revolute 1", "frame": 75}
{"tab": "Assembly 1", "mate": "Cylindrical 1", "dof": "tz", "start": "0 mm", "end": "40 mm", "steps": 40, "frame": 10}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `mate` | string |  | The mate (id or name); default the first unsuppressed mate with degrees of freedom. |
| `dof` | one of `rz`, `tx`, `ty`, `tz` |  | The degree of freedom ("Select degree of freedom"): rz = rotation about Z, tx / ty / tz = translation; needed when the mate has several. |
| `start` | string or number |  | Start value (angle or length expression). |
| `end` | string or number |  | End value. |
| `steps` | integer |  | Steps. |
| `frame` | integer |  | The step to show (0 = start; default the last step, the end). |

#### assembly_display_state

Display states: add, apply, update, rename, remove (edits the document (one undoable history entry)).

The Display states panel: `add` saves which instances and mates are hidden now as a new state `name` ("Add display state : Front"); `apply` shows and hides them as the state says (instances added since keep theirs); `update` saves the current visibility into the state; `rename` to `name`; `remove`. One edit each with the panel's text.

Examples:

```json
{"action": "add", "name": "Pin hidden"}
{"action": "apply", "state": "Pin hidden"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `action` | one of `add`, `apply`, `update`, `rename`, `remove` | required | What to do. |
| `state` | string |  | The display state: its name or 1-based row (all actions but add). |
| `name` | string |  | add: the new state's name; rename: the new name. |

#### assembly_move_to_subassembly

Move instances to a new subassembly (edits the document (one undoable history entry)).

Move to new subassembly: a new Assembly tab holds the instances and the mates among them, and one instance of it takes their place. Pattern and mirror copies stay.

Examples:

```json
{"instances": ["Wheel <1>", "Axle <1>"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string | required | Instance ids or names. |

#### assembly_create_subassembly

Create an empty subassembly (edits the document (one undoable history entry)).

Create empty subassembly: a new, empty Assembly tab and an instance of it here.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |

#### assembly_dissolve_subassembly

Dissolve subassemblies into this assembly (edits the document (one undoable history entry)).

Dissolve subassembly: the subassembly's instances and mates move up into this assembly; its tab is emptied when nothing else uses it.

Examples:

```json
{"instances": ["Assembly 2 <1>"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string | required | Instance ids or names. |

#### assembly_add_folder

Add instances or mate features to a new folder (edits the document (one undoable history entry)).

Add selection to folder… in the instance list or the Mate features list. Rename it with assembly_rename (`feature`: the folder), unpack it with assembly_unpack_folder.

Examples:

```json
{"instances": ["Bolt <1>", "Bolt <2>"], "name": "Fasteners"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string |  | Instance ids or names. |
| `features` | array of string |  | Mate, relation, group, pattern or connector ids or names. |
| `name` | string |  | Folder name. |

#### assembly_unpack_folder

Unpack an assembly folder (edits the document (one undoable history entry)).

The folder goes; its instances and features stay.

Examples:

```json
{"folder": "Fasteners"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `folder` | string | required | Folder id or name (assembly_list `folders`). |

#### assembly_copy

Copy and paste instances (edits the document (one undoable history entry)).

Copy then Paste (Ctrl+C, Ctrl+V): new instances named "<name> <n>", with the mates and mate connectors among the copied instances; never fixed. `offset` moves the copies (the app places them at the cursor).

Examples:

```json
{"instances": ["Bolt <1>"], "offset": ["30 mm", 0, 0]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string | required | Instance ids or names. |
| `to` | string |  | Assembly tab to paste into (default: the same one). |
| `offset` | array of number or string |  | [x, y, z] lengths (default [0, 0, 0]). |

#### assembly_set_configuration

Change an instance's configuration (edits the document (one undoable history entry)).

Change configuration… of an instance of a configured Part Studio or assembly.

Examples:

```json
{"instance": "Bracket <1>", "configuration": {"Size": "Large"}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string | required | Instance id or name. |
| `configuration` | object | required | {input id: value} (list inputs: the configuration id; checkboxes: true / false). |

#### assembly_lock

Lock a subassembly at a position or let it follow (edits the document (one undoable history entry)).

A subassembly instance's Lock / follow position menu: `flexible` (Unlock), `current` (lock at the current position) or a named position of the subassembly.

Examples:

```json
{"instance": "Arm <1>", "lock": "current"}
{"instance": "Arm <1>", "lock": "flexible"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string | required | Subassembly instance id or name. |
| `lock` | string | required | "flexible", "current" or a named position name. |

#### assembly_instance_properties

Set instance properties: reference designator, tessellation (edits the document (one undoable history entry)).

Instance properties… (Reference designator) and Use best available tessellation / Use automatic tessellation setting.

Examples:

```json
{"instances": ["Resistor <1>"], "reference_designator": "R1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string | required | Instance ids or names. |
| `reference_designator` | string |  | Reference designator (one instance; empty clears). |
| `best_tessellation` | boolean |  | Use best available tessellation (false: automatic). |

#### assembly_dynamic_suppression

Set or remove an instance, mate or folder's dynamic suppression (edits the document (one undoable history entry)).

Dynamic suppression ▸ Add expression…: suppressed unless `unsuppressed_if` is true. Empty or omitted removes it.

Examples:

```json
{"instance": "Option <1>", "unsuppressed_if": "#withOption"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string |  | Instance id or name. |
| `mate` | string |  | Mate id or name. |
| `folder` | string |  | Instance folder id or name. |
| `unsuppressed_if` | string |  | Unsuppressed if: expression. |

#### assembly_standard_content

Browse the standard content catalogue.

The Insert dialog's Standard content pickers. With nothing: the standards; with `standard`: its categories; then classes, components; with a full component: its sizes, lengths, other selects, materials and finishes and the defaults.

Examples:

```json
{}
{"standard": "ISO", "category": "Bolts & screws", "class": "Socket head screws", "component": "Hex socket head cap screw ISO 4762"}
```

| field | type | | description |
|---|---|---|---|
| `standard` | string |  | Standard. |
| `category` | string |  | Category. |
| `class` | string |  | Class. |
| `component` | string |  | Component. |
| `size` | string |  | Size (for its lengths; default the component's default size). |

#### assembly_insert_standard

Insert standard content (bolts, screws, nuts, washers, pins…) (edits the document (one undoable history entry)).

The component: `standard` ("ANSI", "ISO", "DIN"…), `category`, `class` and `component` as the tab's pickers name them (see assembly_standard_content; `component` alone is enough when its name is unique), `size`, `length`, `params` ({label: option} for the component's other selects, e.g. {"Head slot": "checked", "Series": "Wide"}), `material`, `finish`. Missing values are the dialog's defaults.

Examples:

```json
{"component": "Hex socket head cap screw ISO 4762", "holes": [{"instance": "Base <1>", "entity": "face:…"}], "auto_size": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `standard` | string |  | Standard. |
| `category` | string |  | Category. |
| `class` | string |  | Class. |
| `component` | string |  | Component. |
| `size` | string |  | Size (default: the component's default, or auto-size from the first hole with `auto_size`). |
| `length` | string |  | Length (default: the size's first). |
| `params` | object |  | Other selects: {label: option}. |
| `material` | string |  | Material. |
| `finish` | string |  | Finish. |
| `holes` | array of object |  | Faces or edges to fasten into: [{"instance", "entity"}] (a circular edge or cylindrical face: one hole; a planar face: every hole in it). One instance per hole, each with a Fastened mate. Without holes: one unplaced instance. |
| `toward` | array of number |  | For an edge or cylinder: the hole end whose face points most this way (assembly coordinates; the dialog uses the view direction). Default [0, 0, 1]. |
| `auto_size` | boolean |  | Auto-size: the size fitting the first hole (bolts and screws round down, nuts and washers up). |
| `placement` | one of `insert`, `closest`, `furthest` |  | Insert, Insert closest to selection (against the face, under what is there) or Insert furthest from selection (at the end of the stack). |
| `flip` | boolean |  | Flip the part on its mate connector (the `a` key). |

#### assembly_edit_standard

Edit standard content instances (size, length, selects, material, finish) (edits the document (one undoable history entry)).

Edit standard content instance…: the instances take the new values and keep their mates.

Examples:

```json
{"instances": ["Hex socket head cap screw M6x1.00 x 20 <1>"], "length": "30"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instances` | array of string | required | Standard content instance ids or names (same component). |
| `size` | string |  | Size (default: the component's default, or auto-size from the first hole with `auto_size`). |
| `length` | string |  | Length (default: the size's first). |
| `params` | object |  | Other selects: {label: option}. |
| `material` | string |  | Material. |
| `finish` | string |  | Finish. |

#### assembly_edit_in_context

Edit in context: open an instance's Part Studio with the assembly around it as context geometry (edits the document (one undoable history entry)).

Instance › Edit in context (help: Modeling in context): the instance's Part Studio shows the rest of the assembly, placed around the part, as its Assembly context. Without `context`, a new snapshot "Context N" (Edit in context, or Create new context once the Part Studio has contexts); with `context`, that existing context of this assembly (Edit Context N). List its geometry with query_entities `context: true` and reference it in features; a context no feature references goes away on context_exit. Not undoable, like the app. Replies with the Part Studio and the context (context_list's fields).

Examples:

```json
{"instance": "Part 1 <1>"}
{"instance": "Part 1 <1>", "context": "Context 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instance` | string | required | A part or Part Studio instance (id or name). |
| `context` | string |  | An existing context of this assembly to edit in (default: a new one). |

#### assembly_create_part_studio_in_context

Create Part Studio in context: a new Part Studio with the assembly as context, its origin at a connector (edits the document (one undoable history entry)).

Toolbar › Create Part Studio in context and its "Origin of new Part Studio" dialog: a new Part Studio ("Part Studio N", after the assembly) whose origin sits at `origin` (an assembly mate connector, id or name; default the assembly origin), showing the whole assembly as its Assembly context. Model there, then context_insert_in_assembly places its parts at that origin. One edit ("Create Part Studio in context").

Examples:

```json
{"origin": "Mate connector 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `origin` | string |  | An assembly mate connector (assembly_mate_connector) for the new Part Studio's origin. |

#### assembly_update_context

Update context: re-snapshot a Part Studio's Assembly context from the assembly as it is now (edits the document (one undoable history entry)).

Instance › Update context ▸ <context>: a new snapshot around the context's primary instance (or this instance when that one is gone); features referencing it regenerate. `out_of_date` in context_list says when an update is available. One edit ("Update context : Context 1").

Examples:

```json
{"instance": "Part 1 <1>", "context": "Context 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instance` | string | required | An instance of the context's Part Studio. |
| `context` | string | required | Context name ("Context 1") or id (context_list). |

#### assembly_set_primary_instance

Set as primary instance: re-anchor a context on another instance of the same part (edits the document (one undoable history entry)).

Instance › Set as primary instance ▸ <context> (other instances of the context's part): the context is re-anchored on this instance and re-snapshotted around it ("Update context : Context 1").

Examples:

```json
{"instance": "Part 1 <2>", "context": "Context 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the first Assembly. |
| `instance` | string | required | Another instance of the context's part. |
| `context` | string | required | Context name ("Context 1") or id (context_list). |

#### assembly_switch_to_tab

The Part Studio or Assembly an instance comes from (instance menu › Switch to …).

Instance (or BOM row) menu › Switch to <Part Studio> / <Assembly>: the source tab (id, name, kind), the part and the configuration the instance shows; the window shows the tab (live mode). Instances from another document: assembly_open_linked_document.

Examples:

```json
{"instance": "Bracket <1>"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string | required | Instance id or name. |

#### assembly_open_linked_document

The document version an instance comes from (instance menu › Open linked document).

For an instance inserted from another document: its path, the version and the tab; with the app open (live mode) that version opens in a window on the tab, as the menu item does.

Examples:

```json
{"instance": "Motor <1>"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Assembly tab, by name or id. Default: the tab the window shows if it is an Assembly (tab_activate, an import), else the first Assembly. |
| `instance` | string | required | Instance id or name. |

### context

#### context_list

A Part Studio's Assembly contexts.

The Assembly contexts section above the Feature list: each context's `id`, `name`, `assembly`, whether it is `shown` and `referenced` (only referenced contexts persist), `out_of_date` (an update is available), its `follow_position` (Follow position ▸, whose only choice is "Current assembly position"), its `primary` instance, whether it has an `origin` (created in context: context_insert_in_assembly works) and its `instances` (instance and part names).

Examples:

```json
{"tab": "Part Studio 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name or id. Default: the first Part Studio. |

#### context_show

Show an Assembly context, or None (edits the document (one undoable history entry)).

The Assembly contexts dropdown: show `context`, or None without it (the context stays). Not undoable ("Assembly context").

Examples:

```json
{"tab": "Part Studio 1", "context": "Context 1"}
{"tab": "Part Studio 1", "context": null}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name or id. Default: the first Part Studio. |
| `context` | string or null |  | Context name or id; omitted or null: None. |

#### context_update

Update context: re-snapshot the assembly around the context's primary instance (edits the document (one undoable history entry)).

Context ⋯ › Update context ("Update context : Context 1"); features referencing it regenerate.

Examples:

```json
{"tab": "Part Studio 1", "context": "Context 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name or id. Default: the first Part Studio. |
| `context` | string | required | Context name ("Context 1") or id (context_list). |

#### context_rename

Rename an Assembly context (edits the document (one undoable history entry)).

Context ⋯ › Rename… ("Rename context").

Examples:

```json
{"tab": "Part Studio 1", "context": "Context 1", "name": "Housing"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name or id. Default: the first Part Studio. |
| `context` | string | required | Context name ("Context 1") or id (context_list). |
| `name` | string | required | The new name. |

#### context_delete

Delete an Assembly context (edits the document (one undoable history entry); deletes or overwrites).

Context ⋯ › Delete ("Delete context"); features referencing its geometry lose those references.

Examples:

```json
{"tab": "Part Studio 1", "context": "Context 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name or id. Default: the first Part Studio. |
| `context` | string | required | Context name ("Context 1") or id (context_list). |

#### context_exit

Exit context: stop showing the assembly (edits the document (one undoable history entry)).

The in-context banner's ✕ (or going back to the assembly): no context shown; a context no feature references goes away. Not undoable ("Exit context").

Examples:

```json
{"tab": "Part Studio 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name or id. Default: the first Part Studio. |

#### context_insert_in_assembly

Insert and go to assembly: insert a Part Studio created in context at its origin (edits the document (one undoable history entry)).

The in-context banner's Insert and go to assembly and its "Select items to insert" dialog, in a Part Studio created in context (assembly_create_part_studio_in_context): `parts` and `sketches` (names) become instances of the context's assembly at the Part Studio's origin. One edit on the assembly ("Insert and go to assembly"); replies with the new instance ids.

Examples:

```json
{"tab": "Part Studio 2", "parts": ["Part 1"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name or id. Default: the first Part Studio. |
| `parts` | array of string |  | Part names (query_parts). |
| `sketches` | array of string |  | Sketch names. |

#### context_go_to_assembly

The assembly of the Assembly context a Part Studio shows (in-context banner › Go to assembly).

Modeling in context: the shown context's assembly tab; the window switches to it (live mode).

Examples:

```json
{"tab": "Part Studio 2"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab (default: the first). |

### drawing

#### drawing_templates

List the drawing templates.

Create Drawing's standard templates: file, sheet setup, standard (ANSI / ISO) and units.

Examples:

```json
{}
```

#### drawing_create

Create a Drawing tab of a part, Part Studio or assembly (edits the document (one undoable history entry)).

Create Drawing: a Drawing tab with the template's sheet, placed after the source tab. With `suggested_views` the source's suggested views (Front, Top, Right projections and an isometric, scaled to fit) are laid out, as the dialog's checkbox does. Returns the tab and its views.

Examples:

```json
{"source": "Part Studio 1", "template": "ISO A3", "suggested_views": true}
{"source": "Assembly 1", "custom_template": {"standard": "ANSI", "size": "B"}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `source` | string |  | Part Studio or assembly tab (name or id). Omit for an empty drawing. |
| `part` | string |  | Only this part (`body:…` id from query_parts, or its name). |
| `sketch` | string |  | Only this sketch of the source Part Studio (feature id or name): Create Drawing of Sketch N. |
| `configuration` | object |  | Configuration of the source: {input id: value}. |
| `template` | string |  | Existing templates: file or setup name (`drawing_templates`), e.g. "ANSI_A_INCH.dwt", "ISO A3". Default ANSI_A_INCH.dwt, the dialog's preselected row. |
| `custom_template` | object |  | The Custom template tab instead of `template`: {"standard": "ANSI" \| "ISO" \| "JIS" (default ISO in metric workspaces, else ANSI), "language": English (default), French, German, Spanish, Italian, Japanese, Korean, ChineseSimplified, ChineseTraditional or the list's own name, "size": ANSI A (default), A Portrait, B, C, D; ISO / JIS A4 (default), A4 Portrait, A3, A2, A1, A0, "units": Millimeters, Centimeter, Meter, Inches, Feet and inches, Foot, Yard, "decimal_separator": "period" \| "comma", "projection": "third_angle" \| "first_angle", "border": bool (default true), "horizontal_zones" / "vertical_zones": 0–20 even (default per size), "start_zones": "bottom_right" (default) \| "top_left", "title_block": bool (default true)}. Choosing a standard sets the units, separator and projection like the dialog (ANSI: Inches, period, third angle; ISO: Millimeters, comma, first angle; JIS: Millimeters, comma, third angle). |
| `name` | string |  | Tab name (default "<part> Drawing N" / "<sketch> Drawing N" / "Drawing N"). |
| `suggested_views` | boolean |  | Suggested views (true) or No views (false, the dialog's default; place views with drawing_insert_view). |

#### drawing_get

A drawing sheet's views (with their edges in sheet coordinates), annotations and tables.

Coordinates are sheet millimeters from the sheet's lower-left corner. Each view lists its kind, parent, scale, sheet bounds and the model edges as drawn (lines with end points, circles/arcs with center and radius, all in sheet mm) so you can click them with drawing_dimension.

Examples:

```json
{"drawing": "Part 1 Drawing 1"}
{"drawing": "Part 1 Drawing 1", "edges": false}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `edges` | boolean |  | Include each view's edges. |

#### drawing_insert_view

Insert a base view of a part or assembly (edits the document (one undoable history entry)).

Insert view: the view centered at `position` (sheet mm) in the given orientation, at `scale` (paper per model, e.g. 0.5 for 1:2; default the sheet scale). One edit ("Insert view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "source": "Part Studio 1", "orientation": "isometric", "position": [330, 230]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `source` | string | required | Part Studio or assembly tab. |
| `part` | string |  | Only this part (`body:…` id or name). |
| `sketch` | string |  | Insert a sketch of the source Part Studio (a sketch view, facing its plane). |
| `flat` | boolean |  | A sheet metal part's flat pattern. |
| `orientation` | one of `front`, `back`, `top`, `bottom`, `left`, `right`, `isometric` |  | View orientation. |
| `named_view` | string |  | View orientation › a named view of the source Part Studio (instead of `orientation`). |
| `position` | array of number | required | View center on the sheet (mm). |
| `scale` | number |  | View scale (paper / model). |
| `simplification` | one of `none`, `auto`, `absolute`, `ratio_to_studio`, `ratio_to_part` |  | View simplification: None, Automatic, Absolute size, Ratio to Part Studio / assembly size, Ratio to part size. |
| `simplification_value` | string |  | Its value: a length ("1 mm", default 1 mm) or a percentage ("1 %", default 1 %). |
| `sketch_appearances` | boolean |  | Show sketch appearances. |
| `model_annotations` | boolean |  | Show annotations from model. |
| `explode_position` | string |  | Assemblies: Explode/Position, an exploded view or named position name (default "Default"). |
| `display_state` | string |  | Assemblies: Display state name (default Show all). |

#### drawing_projected_view

Project a view from a parent view (edits the document (one undoable history entry)).

Projected view: the parent view projected toward `position` (sheet mm): above/below/left/right of the parent gives the orthographic projection (aligned with the parent), diagonal gives an isometric. One edit ("Projected view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "parent": 3, "position": [330, 120]}
```

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

Examples:

```json
{"drawing": "Part 1 Drawing 1", "picks": [[180, 180]], "place": [180, 200]}
{"drawing": "Part 1 Drawing 1", "kind": "diameter", "picks": [[205.5, 160]], "place": [220, 175]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `kind` | one of `smart`, `point_to_point`, `point_to_line`, `line_to_line`, `radius`, `diameter`, `angle`, `angle_3_point`, `arc_length`, `chamfer`, `max_min`, `foreshorten` |  | Dimension tool. |
| `picks` | array of array of number | required | The picking clicks (sheet mm). |
| `place` | array of number | required | Where the dimension text goes (sheet mm). |
| `mid_quad` | boolean |  | Shift+Q (Quad & mid snap point): the clicks also snap to edge midpoints and circle quadrant points, for this dimension. |

#### drawing_update

Update an out-of-date drawing from the current model (edits the document (one undoable history entry)).

After the model changes, a drawing's views keep the geometry they were made from and the drawing is out of date (`out_of_date` in drawing_get; the app's yellow Update button). Update remakes every view from the current document. One edit ("Update") when out of date; otherwise nothing changes.

Examples:

```json
{"drawing": "Part 1 Drawing 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |

#### drawing_export

Export a drawing (PDF, DWG, DXF, DWT, SVG, PNG, JPEG, TIFF, inspection lists) (edits the document (one undoable history entry)).

The Export Drawing modal with every option. PDF holds every sheet, DWT the first two, DXF / DWG a layout per sheet of `sheets`; SVG and the raster formats (300 dpi) the active sheet. `version` from the format's list (PDF 1.4–1.7, default 1.5; DWG / DXF Release 11-12 … 2018, default 2000 / Release 14; DWT 2013 / 2018). Inspection list / Drawing definition list are the CSV lists.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "format": "PDF", "file": "/tmp/drawing.pdf"}
{"drawing": "Part 1 Drawing 1", "format": "DXF", "file": "/tmp/drawing.dxf"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `format` | one of `PDF`, `DWG`, `DXF`, `DWT`, `SVG`, `PNG`, `JPEG`, `TIFF`, `Inspection list`, `Drawing definition list` | required | Format. |
| `file` | string |  | Output path (required unless `options` is "store"; then only its name is used, default "<drawing>.<ext>"). |
| `options` | one of `download`, `download_and_store`, `store` |  | Options: write `file`, and / or keep the file as a file tab right after the drawing ("Store <file>"). |
| `version` | string |  | The format's Version (e.g. "1.7", "2018", "Release 14"). |
| `sheets` | string |  | DWG / DXF: "all" (default), "current" or a range "1-3, 5" (PDF: every sheet; DWT: the first two; SVG and rasters: the active sheet). |
| `overridden_dimensions` | one of `show_underlines`, `hide_underlines` |  | Overridden dimensions. |
| `text` | one of `normal`, `selectable`, `export_as_text`, `explode_notes`, `explode_all` |  | PDF Text (normal / selectable) or DWG / DXF / DWT Text (export_as_text / explode_notes / explode_all). |
| `color` | one of `color`, `black_and_white`, `grayscale` |  | PDF Color (default color; only PDF has it). |
| `units` | one of `Use drawing units`, `Centimeter`, `Foot`, `Inch`, `Meter`, `Millimeter`, `Yard` |  | DWG / DXF Units (default "Use drawing units"). |
| `splines_as_polylines` | boolean |  | Export splines as polylines. |
| `z_zero` | boolean |  | DWG / DXF / DWT: Set z-height to zero and normals to positive. |
| `hide_inspection_items` | boolean |  | Hide inspection items. |
| `exclude_off_sheet` | boolean |  | DWG / DXF / DWT: Exclude off-sheet content. |

#### drawing_section_view

Section view of a view (full, partial, jogged by snap points) (edits the document (one undoable history entry)).

The Section view tool: the cutting line goes through `point` (sheet mm; a vertex or circle center there is snapped) across the whole parent view, `direction` vertical (the dialog's default), horizontal or angular (`angle`, default 45 deg). With `partial` the line runs from `point` to `end` (snapped to horizontal / vertical within 8 deg, as the second click) and only cuts along its length. `jogs`: snap points of the view the line jogs through. The view goes to `position`, projected perpendicular to the line. One edit ("Section view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "parent": 3, "point": [205.5, 160], "position": [300, 160]}
```

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

#### drawing_jogged_section_view

Jogged section view (an orthogonal cutting line through several points) (edits the document (one undoable history entry)).

The Jogged section view tool: clicks `points` (sheet mm, at least 2) on the parent view; each one continues the cutting line horizontally or vertically (in the view's frame) toward it, and finishing (Enter) extends both ends past the view. The section goes to `position`, projected from the first segment. One edit ("Section view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "parent": 3, "points": [[190, 150], [205, 150], [205, 170]], "position": [300, 160]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `parent` | integer | required | View to section. |
| `points` | array of array of number | required | The cutting line's clicks (sheet mm). |
| `position` | array of number | required | Where the section view goes (sheet mm). |

#### drawing_aligned_section_view

Aligned section view (two cutting legs about a center, unfolded) (edits the document (one undoable history entry)).

The Aligned section view tool: `center` (sheet mm) on the parent view, then `a` and `b`, the directions of the two cutting legs from it (each leg is extended past the view). The view goes to `position`, projected from the first leg. One edit ("Aligned section view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "parent": 3, "center": [200, 160], "a": [200, 190], "b": [230, 160], "position": [320, 160]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `parent` | integer | required | View to section. |
| `center` | array of number | required | The legs' common point (sheet mm). |
| `a` | array of number | required | A point along the first leg (sheet mm). |
| `b` | array of number | required | A point along the second leg (sheet mm). |
| `position` | array of number | required | Where the view goes (sheet mm). |

#### drawing_detail_view

Detail view of a region of a view (circle, rectangle, spline or polygon profile) (edits the document (one undoable history entry)).

The Detail view tool: the circle centered at `center` (sheet mm; a vertex or circle center there is snapped) with `radius` (sheet mm), or a rectangle / spline / polygon `profile` through `points`, on the parent view, shown at `scale` (default twice the parent's) at `position`. One edit ("Detail view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "parent": 3, "center": [205.5, 160], "radius": 12, "position": [380, 60]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `parent` | integer | required | View to detail. |
| `profile` | one of `circle`, `rectangle`, `spline`, `polygon` |  | The dialog's profile tool. |
| `center` | array of number |  | Circle: the center (sheet mm; a vertex or circle center there is snapped). |
| `radius` | number |  | Circle: the radius (sheet mm). |
| `points` | array of array of number |  | Rectangle: two corners; spline / polygon: at least 3 points (sheet mm), closed. |
| `position` | array of number | required | Where the detail view goes (sheet mm). |
| `scale` | number |  | View scale (default 2 x the parent's). |
| `label` | string |  | Detail id (default the sheet's next letter). |
| `style` | one of `in_line`, `connected`, `leader` |  | Label style (default Drawing properties'). |
| `show_profile` | boolean |  | Show the detail profile (default Drawing properties'). |
| `clip_border` | boolean |  | Clip the border (default Drawing properties'). |

#### drawing_edit_detail

Edit a detail view: move / resize its circle, its label and leader, Edit detail's options (edits the document (one undoable history entry)).

The detail circle's grips and menu and Edit detail... (live asb 10-23 views (1), (2)): `center` moves the circle (sheet mm, in its parent), `radius` resizes it (sheet mm; the detail view grows with it); `leader` true puts the label on a leader with an arrow to the circle (Add leader), false back beside it; `label_at` places the label on its leader (sheet mm); the Detail view panel's `scale`, `label`, `style`, `show_profile` and `clip_border`. One edit ("Edit detail view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 5, "center": [208, 160], "radius": 15}
{"drawing": "Part 1 Drawing 1", "view": 5, "leader": true, "scale": 3}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | The detail view (drawing_get). |
| `center` | array of number |  | New circle centre (sheet mm). |
| `radius` | number |  | New circle radius (sheet mm). |
| `leader` | boolean |  | Label on a leader (Add leader) or beside the circle. |
| `label_at` | array of number |  | The label's position on its leader (sheet mm). |
| `scale` | number |  | View scale. |
| `label` | string |  | View label (detail id). |
| `style` | one of `in_line`, `connected`, `leader` |  | Profile style. |
| `show_profile` | boolean |  | Show detail view profile. |
| `clip_border` | boolean |  | Clip border to geometry. |

#### drawing_auxiliary_view

Auxiliary view: normal to an edge, in line with a sketch line / centerline, or down a hole's axis (edits the document (one undoable history entry)).

The Auxiliary view tool. `edge` is a click (sheet mm) on the fold line for `mode`: normal, a straight model edge of the parent view (the view looks along it); in_line, a sheet sketch line or a centerline annotation; axis, a hole's axis seen side-on (its centerline or silhouette; the view looks down it). The view goes to `position` (at least 15 mm from the parent, perpendicular to the fold line). One edit ("Auxiliary view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "edge": [190, 175], "position": [260, 260]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `mode` | one of `normal`, `in_line`, `axis` |  | The dialog's tab. |
| `edge` | array of number | required | A point on the fold line (sheet mm). |
| `position` | array of number | required | Where the view goes (sheet mm). |
| `label` | string |  | Auxiliary id (default the sheet's next letter). |
| `reverse` | boolean |  | Reverse the direction. |
| `show_reference` | boolean |  | Show the view reference indicator. |

#### drawing_break_view

Break a view (remove a band between two points) (edits the document (one undoable history entry)).

The Break view tool: removes the band between `from` and `to` (sheet mm) across the view: `vertical` break lines cut along x, horizontal ones along y. One edit ("Break view").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 3, "from": [190, 150], "to": [220, 150]}
```

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

The Crop view tools: a rectangle with corners `from` and `to`, a circle centered at `from` through `to` (sheet mm; `from` snaps to a vertex there), or a closed polygon / spline through `points` (sheet mm, at least 3; vertices snap). One edit ("Crop view"); a view's new crop replaces its old one.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 3, "from": [180, 140], "to": [230, 180]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | View to crop. |
| `shape` | one of `rectangle`, `circle`, `polygon`, `spline` |  | Crop shape. |
| `from` | array of number |  | Rectangle corner / circle center (sheet mm). |
| `to` | array of number |  | Opposite corner / a point on the circle (sheet mm). |
| `points` | array of array of number |  | Polygon / spline points (sheet mm). |

#### drawing_broken_out_section

Broken-out section: cut away a closed profile of a view (edits the document (one undoable history entry)).

The Broken-out section tool: the closed spline profile through `points` (sheet mm, at least 3; vertices snap) cut into the view to a blind `depth` from the model's nearest point (default: half the model's depth along the line of sight, as the dialog), or up to the model point snapped at `depth_to`. `index` edits the view's existing broken-out section instead. One edit ("Broken-out section"). Projected views of the view show the cut too.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "points": [[195, 150], [215, 150], [215, 170], [195, 170]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer |  | View to cut (default: the view under the first point). |
| `points` | array of array of number | required | Profile points (sheet mm). |
| `depth` | string or number |  | Blind depth: a length ("5 mm"; a bare number is in workspace units). |
| `depth_to` | array of number |  | Up to entity: a model point (vertex, circle center...) of a view of the same model (sheet mm). |
| `index` | integer |  | Edit the view's broken-out section number `index` (0-based, drawing_get) instead of adding one. |

#### drawing_edit_hatch

Edit hatch section...: restyle one hatch region of a view's cut (edits the document (one undoable history entry)).

The view menu's Edit hatch section... (live asb 10-23 views (4)): the hatch region of view `view`'s cut under `at` (sheet mm) takes `standard` (ansi, iso, general) and `pattern` (its name as listed, e.g. "ANSI32 steel", "ISO02W100", "Solid"; general "None" leaves the region unhatched), `scale` and `angle` ("auto" or numbers; angle in degrees, the pattern's own when auto) and `color` ("#rrggbb", default the drawing's); `reset` puts the region back to the drawing's hatch. Each region is separate. One edit ("Edit hatch").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 4, "at": [300, 150], "pattern": "ANSI32 steel", "angle": 90}
{"drawing": "Part 1 Drawing 1", "view": 4, "at": [300, 150], "standard": "general", "pattern": "None"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | The view (drawing_get). |
| `at` | array of number | required | A point inside the hatch region (sheet mm). |
| `standard` | one of `ansi`, `iso`, `general` |  | Pattern group (the dialog's tab). |
| `pattern` | string |  | Pattern name (default the group's first in the dialog's order). |
| `scale` | string or number |  | Scale (auto = 1). |
| `angle` | string or number |  | Angle in degrees (auto = the pattern's own). |
| `color` | string |  | Color "#rrggbb" (default the drawing's). |
| `reset` | boolean |  | Back to the drawing's section hatch. |

#### drawing_title_field

Edit a title block field: its cell's note (edits the document (one undoable history entry)).

Double-click a title block cell (live qa4 round 5 s11 (4)): each field's cell is a note, editable while the formats are locked. `text` replaces field `field` (as drawing_get's `title_fields`, e.g. "Title", "Drawn by"; a key the layout lacks is added). Lines split on \n; property codes like {part.name} expand. One edit ("Edit title block").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "field": "Title", "text": "BRACKET"}
{"drawing": "Part 1 Drawing 1", "field": "Title", "text": "{part.name}\nBRACKET"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `field` | string | required | The field (drawing_get `title_fields`). |
| `text` | string | required | Its text. |

#### drawing_hatch_region

Hatch closed profiles of the sheet sketch or of views (edits the document (one undoable history entry)).

The Hatch region dialog: each of `points` (sheet mm) picks the smallest closed sheet-sketch profile around it, else the smallest closed region of the visible edges of the view there (not detail or break views); a region per profile. Pattern tab `standard` and `pattern` (its name as listed in the dialog, e.g. "ANSI31 general", or just its first word "ANSI31"; default the tab's default), `scale` and `angle` ("Auto" or a number; not on the General tab), `color` (default: the drawing's hatch color). `hatch` edits that hatch region instead (its profile replaced when `points` are given). One edit ("Hatch region" / "Edit hatch"); replies with the hatch ids.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "points": [[20, 20], [60, 20], [60, 50]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `points` | array of array of number |  | Points inside the profiles to hatch (sheet mm). |
| `standard` | one of `ansi`, `iso`, `general` |  | Pattern tab. |
| `pattern` | string |  | Pattern name. |
| `scale` | string or number |  | Pattern scale ("Auto" = 1). |
| `angle` | string or number |  | Pattern angle in degrees ("Auto": the pattern's own). |
| `color` | array of integer |  | Color [r, g, b] (default: the drawing's hatch color). |
| `hatch` | integer |  | Hatch region id to edit (drawing_get). |

#### drawing_custom_table

Insert a custom table, or edit a custom table's properties (edits the document (one undoable history entry)).

The Custom table dialog: custom table `table` (its name or display name) defined by a Feature Studio of this document, computed on Part Studio `studio` (default: the Part Studio of `view`, else the first one shown in the drawing), placed with its `corner` at `position` (sheet mm). With `id`: Custom table properties of that table (Part Studio, table, order, corner; unset fields kept). One edit ("Custom table" / "Custom table properties"). Its rows are in drawing_get's tables.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "table": "Hole report", "position": [20, 270]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `table` | string |  | Custom table name or display name. |
| `feature_studio` | string |  | Feature Studio tab (name or id) defining it, when several define the name. |
| `studio` | string |  | Part Studio tab (name or id). |
| `view` | integer |  | A view whose Part Studio to use. |
| `position` | array of number |  | Where the fixed corner goes (sheet mm; inserting). |
| `corner` | one of `top_left`, `top_right`, `bottom_left`, `bottom_right` |  | Fixed corner. |
| `bottom_to_top` | boolean |  | Row order bottom to top. |
| `id` | integer |  | Custom table to edit (drawing_get). |

#### drawing_insert_image

Insert an image on the sheet (edits the document (one undoable history entry)).

Toolbar › Insert image: image file tab `file` of the document (PNG, JPEG, GIF), or a local file `path` imported first as a file tab (its own edit "Import <name>", as the picker's Import). Placed as a drag from `position` to `to` (sheet mm; the image's aspect kept), or without `to` 30 mm high with its lower-left corner at `position`. One edit ("Insert image"); replies with the image id.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "file": "/tmp/logo.png", "position": [20, 20]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `file` | string |  | Image file tab (name or id). |
| `path` | string |  | Local image file to import and insert. |
| `position` | array of number | required | Drag start / lower-left corner (sheet mm). |
| `to` | array of number |  | Drag end (sheet mm). |

#### drawing_insert_cad

Insert a DXF or DWG as drawing sketch geometry (edits the document (one undoable history entry)).

Toolbar › Insert DXF or DWG: the model-space wireframe of DXF / DWG file tab `file` (or local file `path`, imported first as a file tab: its own edit "Import <name>") becomes sheet sketch geometry at the file's own coordinates (its unit, else the drawing's; blocks and polylines exploded, colors dropped). One edit ("Insert DXF or DWG"); replies with the number of entities.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "file": "/tmp/border.dxf"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `file` | string |  | DXF / DWG file tab (name or id). |
| `path` | string |  | Local DXF / DWG file to import and insert. |

#### drawing_sketch

Drawing sketch tools: line, circles, rectangles, tangent arc, spline, points, trim (edits the document (one undoable history entry)).

The drawing toolbar's sketch tools on the sheet sketch, on clicks `picks` (sheet mm) as in the editor: a click snaps to a sketch point (within 6 px), else to a view snap point (vertex, center...), and lines within 1.5° of horizontal / vertical get that constraint. line: a chain through the clicks; circle (center, then a point on it), corner_rectangle / center_rectangle (two corners / center and corner) and tangent_arc (a line or arc end point, then the arc's end): pairs of clicks; circle_3_point: triples; spline: one spline through all clicks; spline_point: each click on a spline adds a point there; point: a sketch point per click; trim: each click trims the curve under it (or deletes a point). One edit with the tool's name ("Line", "Center point circle", ...).

Examples:

```json
{"drawing": "Part 1 Drawing 1", "tool": "line", "picks": [[20, 20], [80, 20]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `tool` | one of `line`, `circle`, `circle_3_point`, `tangent_arc`, `corner_rectangle`, `center_rectangle`, `spline`, `spline_point`, `point`, `trim` | required | Sketch tool. |
| `picks` | array of array of number | required | Clicks (sheet mm). |

#### drawing_delete

Delete views, annotations, tables, images, hatches, sketch entities or view breaks (edits the document (one undoable history entry); deletes or overwrites).

Select and Delete on the sheet (ids from drawing_get). Deleting a view deletes its projected / section / detail children's link and its label; deleting an annotation deletes the callouts grouped with it; a datum shown from the model is hidden instead. One edit ("Delete").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "views": [5]}
{"drawing": "Part 1 Drawing 1", "annotations": [12, 13], "breaks": [{"view": 3, "index": 0}]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `views` | array of integer |  | View ids. |
| `annotations` | array of integer |  | Annotation ids (dimensions, notes, callouts...). |
| `tables` | array of integer |  | Table ids. |
| `images` | array of integer |  | Image ids. |
| `hatches` | array of integer |  | Hatch region ids. |
| `sketch_entities` | array of integer |  | Drawing sketch entity ids. |
| `breaks` | array of object |  | View breaks: [{"view": id, "index": n}] (drawing_get lists a view's breaks). |

#### drawing_edit_dimension

Edit a dimension or hole callout: text, precision, units, tolerance, dual, parentheses (edits the document (one undoable history entry)).

The dimension palette (double-click a dimension or hole callout): text above / below, prefix, suffix, the value (typing it overrides a linear dimension's text; empty puts the measured value back), precision, units, tolerance and its values, Parentheses, Inspection dimension, Dual dimension, a radial dimension's radius / diameter; hole callouts: Override precision/tolerance. Tolerance values are in the dimension's units (or with units: "0.05 mm"). One edit ("Edit dimension" / "Edit hole callout").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "annotation": 12, "tolerance": "Symmetrical", "tolerance_value": 0.1, "precision": 1}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `annotation` | integer | required | Dimension or hole callout id (drawing_get). |
| `above` | string |  | Text above. |
| `below` | string |  | Text below. |
| `prefix` | string |  | Prefix. |
| `suffix` | string |  | Suffix. |
| `value` | string |  | Linear dimensions: the value text (an override; empty: the measured value). |
| `precision` | integer or string |  | Decimals 0–8, or "drawing" (the drawing's). |
| `units` | string |  | Millimeters, Centimeters, Meters, Inches, Inches fractional, Feet and inches, Feet, Yards, or "drawing". |
| `tolerance` | string |  | None, Symmetrical, Deviation, Limits, Basic (not hole callouts), MIN, MAX, Fit, Fit with tolerance, Fit (Tolerance only). |
| `tolerance_value` | number or string |  | Symmetrical: ± value. |
| `upper` | number or string |  | Deviation / Limits: upper value. |
| `lower` | number or string |  | Deviation / Limits: lower value. |
| `hole_class` | string |  | Fits: hole class ("H7"). |
| `shaft_class` | string |  | Fits: shaft class ("g6"). |
| `parentheses` | boolean |  | Parentheses (a reference value). |
| `inspection` | boolean |  | Inspection dimension (oval frame). |
| `dual` | one of `single`, `drawing`, `dual` |  | Single unit, Single unit (Drawing) or Dual unit. |
| `diameter` | boolean |  | Radial dimensions: diameter (true) or radius (false). |
| `override_tolerance` | boolean |  | Hole callouts: Override precision/tolerance. |

#### drawing_format_painter

Format painter: copy a dimension's or annotation's format to others (edits the document (one undoable history entry)).

The Format painter dialog: from annotation `source` (a dimension, note, GD&T frame..., drawing_get ids) to each of `targets`, pasting its style (on by default), tolerance and precision, and / or content. One edit ("Format painter").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "source": 12, "targets": [13, 14], "tolerance_precision": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `source` | integer | required | Annotation to copy from. |
| `targets` | array of integer | required | Annotations to paint. |
| `style` | boolean |  | Paste style. |
| `tolerance_precision` | boolean |  | Paste tolerance and precision. |
| `content` | boolean |  | Paste content. |

#### drawing_inspection

Inspection items: number annotations (manual, reorder, automatic), edit, move, remove, hide, border shape (edits the document (one undoable history entry)).

The Inspection item tool and its menus. add: number `annotations` in order, from `number` (Reorder inspection items) or the drawing's next number ("Inspection item"). automatic: number every dimension and annotation of `scope` (drawing, sheet, or the view `view`) ("Inspection items"). edit: annotation `id` gets `number` ("Edit inspection item"). move: `id`'s number moves to annotation `to` ("Move inspection item"). remove: `annotations` lose theirs ("Remove inspection item"); remove_all ("Remove all inspection items"). hide / show: the toolbar's Hide inspection items toggle ("Hide inspection items" / "Show inspection items"). shape: the bubbles' border `shape` ("Inspection border shape"). drawing_get lists each annotation's `inspection` number.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "action": "automatic", "scope": "sheet"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `action` | one of `add`, `automatic`, `edit`, `move`, `remove`, `remove_all`, `hide`, `show`, `shape` | required | What to do. |
| `annotations` | array of integer |  | Annotations (add, remove). |
| `number` | integer |  | First number (add: reorder) / the number (edit). |
| `id` | integer |  | Annotation (edit, move). |
| `to` | integer |  | Annotation the number moves to (move). |
| `scope` | one of `drawing`, `sheet`, `view` |  | Automatic scope. |
| `view` | integer |  | View (scope view). |
| `shape` | one of `circle`, `oval`, `rectangle`, `triangle`, `flag` |  | Border shape (shape). |

#### drawing_properties

Read or change the Drawing properties (units, precision, text, arrows, views, tables, inspection...) (edits the document (one undoable history entry)).

The Drawing properties panel: without `set`, replies with the properties (`properties`: field names and values, lengths in mm; unset optional fields and false flags are left out). `set` changes the fields it names (same names and value shapes; null clears an optional one), as the panel does: with `locked` on before and after, nothing else changes; choosing `units` sets the drawing's metric flag. One edit ("Drawing properties").

Examples:

```json
{"drawing": "Part 1 Drawing 1"}
{"drawing": "Part 1 Drawing 1", "set": {"projection": "first_angle"}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `set` | object |  | Fields to change (see the reply without `set`). |

#### drawing_centerline

Centerlines: 2 point, edge to edge, 3 point / 2 point circle (edits the document (one undoable history entry)).

The centerline tools, on clicks `picks` (sheet mm): two_point (two snap points of one view), edge_to_edge (one cylinder silhouette edge gives its axis; or two edges give their middle line), circle_3_point (three snap points) and circle_2_point (center, then a point on the circle). Associative to the model points snapped. One edit with the tool's name.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "kind": "edge_to_edge", "picks": [[190, 150], [190, 170]]}
```

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

Examples:

```json
{"drawing": "Part 1 Drawing 1", "picks": [[190, 150], [200, 140]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `picks` | array of array of number | required | A point on each edge (sheet mm). |

#### drawing_hole_callout

Hole / thread callout on a hole seen as a circle (edits the document (one undoable history entry)).

The Hole/thread callout tool: `hole` is a click on the hole's circle (sheet mm); the callout text (diameter, depth, thread from the Hole feature) goes to `position`. One edit ("Hole callout").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "hole": [210.5, 160], "position": [230, 180]}
```

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

Examples:

```json
{"drawing": "Part 1 Drawing 1", "anchor": [200, 160], "position": [230, 185], "text": "1"}
```

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

The Ordinate dimension tool: `origin` is the datum (a vertex or circle center, sheet mm), `side` a click that sets the measured direction (beside the origin: horizontal values; above or below: vertical), then each of `points` adds an ordinate to the group. One edit per click, as with the mouse ("Ordinate dimension", then "Add to ordinate group"). With `group` (an ordinate dimension's id: its menu's Add to ordinate group), `points` go into that group, measured from its origin.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "origin": [180, 140], "side": [180, 130], "points": [[205.5, 160], [220, 140]]}
{"drawing": "Part 1 Drawing 1", "group": 9, "points": [[230, 140]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `origin` | array of number |  | The datum point (sheet mm); required without `group`. |
| `side` | array of number |  | Where the values line up (sheet mm); required without `group`. |
| `points` | array of array of number |  | Points to dimension (vertices, circle centers; sheet mm). |
| `group` | integer |  | An existing ordinate dimension to add `points` to. |

#### drawing_datum

Datum feature symbol (edits the document (one undoable history entry)).

The Datum tool: the triangle at `anchor` (sheet mm): on a GD&T frame edge or a dimension's lines when there (it follows them), else on the geometry (a vertex or edge point), the label box at `position` (at least 3 mm away). The label is the next free letter (A, B, …; I, O, Q, S, X, Z skipped) unless given. One edit ("Datum").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "anchor": [200, 140], "position": [200, 125], "label": "A"}
```

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

Examples:

```json
{"drawing": "Part 1 Drawing 1", "holes": [[205.5, 160]]}
```

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

Examples:

```json
{"drawing": "Part 1 Drawing 1", "text": "BREAK ALL SHARP EDGES", "position": [30, 40]}
```

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

The Geometric tolerance tool and dialog: the frame at `position` (sheet mm). Each row: `symbol` (position, concentricity, symmetry, parallelism, perpendicularity, angularity, cylindricity, flatness, roundness, straightness, profile_of_a_surface, profile_of_a_line, circular_runout, total_runout; later rows default to composite), `tolerance` with optional `before` / `after` (e.g. "Ø", "Ⓜ"), up to three `datums` ("A", "B Ⓜ"), and `prefix` / `suffix` texts; `upper` / `lower` texts above and below; `anchor`: its leader's point on the geometry. One edit ("Geometric tolerance").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "position": [240, 120], "rows": [{"symbol": "position", "tolerance": "⌀0.1", "datums": ["A", "B"]}]}
```

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
| `anchor` | array of number |  | Where the leader points (sheet mm: a view's edge, vertex or circle point snaps to it), as clicking the geometry first does; else no leader (drawing_annotation_edit add_leader adds more). |

#### drawing_surface_finish

Surface finish symbol (edits the document (one undoable history entry)).

The Surface finish symbol tool and panel: the symbol at `position` (sheet mm, on the edge it sits on) with the roughness text `value` (the panel starts with "Ra 3.2"). One edit ("Surface finish").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "position": [210, 175], "value": "Ra 1.6"}
```

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

Examples:

```json
{"drawing": "Part 1 Drawing 1", "target": [200, 150], "position": [230, 120], "spec": {"arrow": {"symbol": "fillet", "size": "5"}}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `target` | array of number | required | What the arrow points at (sheet mm). |
| `position` | array of number | required | Where the reference line starts (sheet mm). |
| `spec` | object |  | The Weld symbol dialog (see the description); empty: no joint symbol. |

#### drawing_revision_table

Revision table (rows from the document's releases) (edits the document (one undoable history entry)).

The Revision table tool and dialog: the table with its fixed corner at `position` (sheet mm). Its rows are the drawing's revisions from releases. One edit ("Revision table").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "position": [415, 290]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `position` | array of number | required | Where the fixed corner goes (sheet mm). |
| `corner` | one of `top_left`, `top_right`, `bottom_left`, `bottom_right` |  | The table's fixed corner. |
| `visible_rows` | integer |  | Rows shown. |
| `newest_top` | boolean |  | Newest revision at the top. |
| `see_sheet1` | boolean |  | Show "See sheet 1" on other sheets. |
| `hide_callouts` | boolean |  | Hide revision callouts. |

#### drawing_revision_callout

Revision callout (triangle with the revision) (edits the document (one undoable history entry)).

Add revision callout: the triangle at `position` (sheet mm) with `revision` (default: the latest revision, else "A"). One edit ("Revision callout").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "position": [100, 100], "revision": "B"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `position` | array of number | required | Where the callout goes (sheet mm). |
| `revision` | string |  | Revision label. |

#### drawing_table

General table (rows x columns of text) (edits the document (one undoable history entry)).

The Table tool and dialog: an empty table of `rows` x `columns` (defaults 2 x 2; 1–60 x 1–20) with an optional title row and header row, its fixed corner at `position` (sheet mm). One edit ("Table"). Fill it with drawing_table_cell.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "position": [20, 280], "rows": 3, "columns": 2, "header_row": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `position` | array of number | required | Where the fixed corner goes (sheet mm). |
| `rows` | integer |  | Rows. |
| `columns` | integer |  | Columns. |
| `title_row` | boolean |  | A title row on top. |
| `header_row` | boolean |  | A header row. |
| `corner` | one of `top_left`, `top_right`, `bottom_left`, `bottom_right` |  | The table's fixed corner. |

#### drawing_table_cell

Type text into a general table cell (edits the document (one undoable history entry)).

The cell editor of a general table: `text` into cell (`row`, `column`), counted from 0 (the header row, if any, is row 0). The table grows when the cell lies beyond it. One edit ("Edit cell").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "table": 5, "row": 0, "column": 0, "text": "Finish"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `table` | integer | required | Table id (drawing_get). |
| `row` | integer | required | Row. |
| `column` | integer | required | Column. |
| `text` | string | required | Cell text. |

#### drawing_view_table

Place a BOM, cut list or hole table of a view (edits the document (one undoable history entry)).

The BOM table, Cut list table and Hole table tools: the table of `view` with its anchor corner at `position` (sheet mm): the reply's `corner`, a BOM's top right as in Onshape (A3: [410, 287] is the frame's top right). A reply `warning` says when the table runs off the sheet. A hole table measures from `origin` (a vertex or circle center of the view, sheet mm). One edit with the tool's name.

Examples:

```json
{"drawing": "Assembly 1 Drawing 1", "kind": "bom", "view": 3, "position": [410, 287]}
{"drawing": "Part 1 Drawing 1", "kind": "hole", "view": 3, "origin": [180, 140], "position": [20, 280]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `kind` | one of `bom`, `cut_list`, `hole` | required | Table tool. |
| `view` | integer | required | The view the table lists. |
| `origin` | array of number |  | Hole table: the origin vertex (sheet mm). |
| `position` | array of number | required | Where the table goes (sheet mm). |

#### drawing_view_display

Show/hide, View orientation and Tangent edges of a view (edits the document (one undoable history entry)).

The view menu's Show/hide ▸ toggles (true shows, false hides; the menu only offers what applies to the view), View orientation ▸ and Tangent edges ▸. Each field given is one edit with the menu's edit text ("Hidden lines", "View orientation", "Tangent edges"...), in the order listed.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 3, "tangent_edges": "phantom", "hidden_lines": true}
{"drawing": "Part 1 Drawing 1", "view": 3, "orientation": "isometric"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | View id (drawing_get). |
| `tangent_edges` | one of `hidden`, `solid`, `phantom` |  | Tangent edges ▸ Hidden / Solid / Phantom. |
| `orientation` | string |  | View orientation ▸ (base views): front, back, top, bottom, left, right, isometric, or a named view of the source. |
| `hidden_lines` | boolean |  | Show/Hide hidden lines |
| `shaded` | boolean |  | Show/Hide shaded view |
| `threads` | boolean |  | Show/Hide threads |
| `faulty_parts` | boolean |  | Show/Hide faulty parts |
| `offset_cut_lines` | boolean |  | Show/Hide offset cut lines (section views: a line across the hatch where the cutting line steps) |
| `sketch_points` | boolean |  | Show/Hide sketch points (views showing sketches) |
| `auto_centermarks` | boolean |  | Show/Hide auto centermarks (showing them again brings back deleted ones) |
| `centerlines` | boolean |  | Show/Hide centerlines (views with cylinder axes seen from the side) |
| `center_of_mass` | boolean |  | Show/Hide center of mass (part views, not flat patterns) |
| `bend_lines` | boolean |  | Show/Hide bend lines (flat pattern views) |
| `bend_notes` | boolean |  | Show/Hide bend notes (flat pattern views) |

#### drawing_view_edit

Rotate, order, align, move to sheet or copy a view; remove its crop, breaks or broken-out sections (edits the document (one undoable history entry)).

The view menu's actions, one edit each:
- `rotate`: `by` degrees counter-clockwise (Rotate ▸ 90° counter-clockwise = 90, clockwise = -90, 180°) or `to` an absolute angle (Custom angle…; 0 = Reset rotation). "Rotate view".
- `rotate_to_edge`: Rotate view ▸ Vertical to… / Horizontal to… (`direction`): turn by the smallest angle so the straight edge of the view at `edge` (sheet mm) is vertical / horizontal; views without a parent or children.
- `order`: Order ▸ Bring to front / Send to back (`to`: front | back).
- `align`: Align views ▸ Vertically / Horizontally (`direction`): the view's center lines up with view `with`'s.
- `suppress_alignment`: (Un)suppress alignment with parent (`on`, default true); projected, auxiliary and section views; a rotated view can't be re-aligned.
- `flip_direction`: section views: Flip direction (the view looks the other way across the cutting line).
- `remove_crop`, `remove_breaks` (Remove all breaks), `remove_broken_out` (Remove section, broken-out section `index`); a removed crop or broken-out section leaves its boundary on the sheet as a sketch circle, polygon or spline (live).
- `move_to_sheet`: Move to sheet… `to_sheet` (number or name; omitted: a new sheet); the view moves alone, linked to its parent.
- `unpin_reference`: Unpin view reference (every view of the same source and part shows the workspace again).
- `copy`: Copy then Paste: the view with its derived views, annotations and tables, pasted centered `at` (sheet mm) on sheet `to_sheet` (default this one); "Paste views"; the reply's `pasted` lists the new view ids.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 3, "action": "rotate", "by": -90}
{"drawing": "Part 1 Drawing 1", "view": 4, "action": "align", "with": 3, "direction": "vertical"}
{"drawing": "Part 1 Drawing 1", "view": 3, "action": "move_to_sheet", "to_sheet": 2}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | View id (drawing_get). |
| `action` | one of `rotate`, `rotate_to_edge`, `order`, `align`, `suppress_alignment`, `flip_direction`, `remove_crop`, `remove_breaks`, `remove_broken_out`, `move_to_sheet`, `unpin_reference`, `copy` | required | The menu action. |
| `by` | number |  | rotate: degrees counter-clockwise to add. |
| `to` | number or string |  | rotate: the absolute angle (degrees); order: "front" or "back". |
| `edge` | array of number |  | rotate_to_edge: a point on a straight edge of the view (sheet mm). |
| `direction` | one of `vertical`, `horizontal` |  | rotate_to_edge, align. |
| `with` | integer |  | align: the view to line up with. |
| `on` | boolean |  | suppress_alignment: suppress (true) or unsuppress. |
| `to_sheet` | integer or string |  | move_to_sheet / copy: the target sheet, number (1 = first) or name. |
| `index` | integer |  | remove_broken_out: which broken-out section (0-based, drawing_get; default 0). |
| `at` | array of number |  | copy: where the copy's center goes (sheet mm; default 20 mm right of and below the view). |

#### drawing_view_properties

Read or change a view's View properties (edits the document (one undoable history entry)).

The View properties dialog of one view (live drawing/177). Without `set`, replies with its fields as the dialog opens (`properties`). `set` changes the fields it names (same names), then ✓: scale ("1:2", or null to inherit), sheet_scale (inherit from the sheet rather than the parent), rotation (degrees; views without a parent or children), tangent_edges, best_quality, orientation / named_view (base views), explode_position and display_state (assemblies; child views: null = Follow parent), suppress_alignment, cut_from_section, sketch_appearances, model_annotations, simplification + simplification_value, name, view_label, label, scale_label, sheet (moves the view, linked to its parent). One edit ("View properties").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 3}
{"drawing": "Part 1 Drawing 1", "view": 3, "set": {"scale": "1:2", "name": "Main", "view_label": true}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | View id (drawing_get). |
| `set` | object |  | Fields to change (see the reply without `set`). |

#### drawing_view_show_hide

Show/hide parts, sketches or edges of a view (edits the document (one undoable history entry)).

The Show/hide parts…, Show/hide sketches… and Show/hide edges… dialogs, ✓ each (one edit each: "Show/hide parts", "Show/hide sketches", "Show/hide edges"). `hidden_parts`: the parts left out of the view (names; views of a whole Part Studio or assembly). `shown_sketches`: the source's sketches shown (names; part views). `toggle_edges`: points on model edges of the view (sheet mm), each hidden, or shown again when hidden; `show_all_edges` first shows every hidden edge.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 3, "hidden_parts": ["Part 2"]}
{"drawing": "Part 1 Drawing 1", "view": 3, "toggle_edges": [[190, 175]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | View id (drawing_get). |
| `hidden_parts` | array of string |  | Part names to hide (replaces the list; [] shows all). |
| `shown_sketches` | array of string |  | Sketch names to show (replaces the list). |
| `toggle_edges` | array of array of number |  | Edge points (sheet mm) to hide / show. |
| `show_all_edges` | boolean |  | Show every hidden edge first. |

#### drawing_view_linestyle

Adjust linestyle of model edges in a view (edits the document (one undoable history entry)).

View menu › Adjust linestyle… (live drawing/443–446): the model edges of `view` at `edges` (points on them, sheet mm) get Linestyle `linestyle` (solid, phantom, center, dashed, dotted; default solid), Dashes `dashes` (scale, default 1; not for solid), Thickness `thickness` (mm, one of 0.13–2.00; default 0.35) and `color` ([r, g, b] 0–255; default the drawing's); `revert` instead puts them back to the drawing properties. One edit ("Adjust linestyle").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 3, "edges": [[190, 175]], "linestyle": "phantom", "thickness": 0.5}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer | required | View id (drawing_get). |
| `edges` | array of array of number | required | Points on the model edges (sheet mm). |
| `linestyle` | one of `solid`, `phantom`, `center`, `dashed`, `dotted` |  | Linestyle. |
| `dashes` | number |  | Dash scale (not for solid). |
| `thickness` | number |  | Line thickness, mm. |
| `color` | array or null |  | Line color. |
| `revert` | boolean |  | Revert to drawing properties. |

#### drawing_sheet

Add, rename, duplicate, copy, move or delete a sheet, or change its Sheet properties (edits the document (one undoable history entry)).

The Sheets panel's sheet menu and the Sheet properties dialog, on sheet `sheet` (default: the one shown):
- `add`: Insert sheet after it (same size, scale and title block; "Add sheet").
- `rename`: Rename… to `name` ("Rename sheet").
- `duplicate`: Duplicate, right after it, "<name> copy" ("Duplicate sheet").
- `copy`: Copy then Paste: appended at the end, "<name> (1)" ("Paste sheet").
- `move`: Move up / Move down to position `to` (1 = first; "Move sheet").
- `delete`: Delete (never the last sheet; "Delete sheet").
- `properties`: without `set`, replies with the dialog's fields (`properties`); `set` changes scale ("1:2"), size ("A3", "A Portrait"…, "Custom"), width / height (mm; Custom), zones ([horizontal, vertical], 0–20 even; Custom), zone_start (bottom_right | top_left), border (Custom), dwt (a DWT file tab: Select a DWT) or dwt: null (Format › Custom); one edit each ("Select a DWT", "Format", "Sheet scale", "Sheet size", "Sheet zones", "Sheet border").
Replies with the sheet acted on (`sheet`, 1 = first) and the sheet list.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "action": "add"}
{"drawing": "Part 1 Drawing 1", "sheet": 2, "action": "rename", "name": "Details"}
{"drawing": "Part 1 Drawing 1", "action": "properties", "set": {"scale": "1:2", "size": "A2"}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `action` | one of `add`, `rename`, `duplicate`, `copy`, `move`, `delete`, `properties` | required | The sheet menu action. |
| `name` | string |  | rename: the new name. |
| `to` | integer |  | move: the new position (1 = first). |
| `set` | object |  | properties: fields to change (see the reply without `set`). |

#### drawing_table_edit

Edit a table from its menus: cells, merge, rows / columns, size, split, order, corner, text height (edits the document (one undoable history entry)).

The table context menus, one edit each. Cells are (`row`, `column`) counted from 0 below the title (the header is row 0); a block of cells runs to (`to_row`, `to_column`).
- `format_cell`: Format cell… (`scope` cell | row | column | table; `align` left | center | right, `bold`, `height` mm or null for the table's) ("Format cells").
- `merge_right` / `merge_below` ("Merge cells"), `unmerge` ("Unmerge cells").
- General tables, a block selected: `merge` (`mode` all | horizontal | vertical), `insert` (`at` column_left | column_right | row_above | row_below), `remove` / `duplicate` (`what` columns | rows); edit texts the menu's ("All", "Column Left", "Rows"…).
- `columns_equally` / `rows_equally`: Size ▸ — of the block's columns / rows when a block is given (two or more), else the whole table.
- `resize`: Resize… column `column`'s `width` and the row `height` (mm) ("Resize cell").
- BOM, cut list and custom tables: `split` below displayed row `row` ("Split table"; the reply's `table` is the new part), `unsplit` ("Unsplit table").
- Hole tables: `flip_x`, `flip_y`, `precision` (`decimals` 0–4 or null for the drawing's).
- `order` (`to` top_to_bottom | bottom_to_top; BOM and cut list), `bom_type` (`to` flattened | structured).
- `fixed_corner` (`to` top_right | top_left | bottom_right | bottom_left; the table stays put), `text_height` (`to` 2.5 | 3 | 3.5 | 5 mm), `fit` (Fit to content).

Examples:

```json
{"drawing": "Part 1 Drawing 1", "table": 2, "action": "merge_right", "row": 1, "column": 0}
{"drawing": "Part 1 Drawing 1", "table": 2, "action": "insert", "row": 0, "column": 1, "at": "column_right"}
{"drawing": "Part 1 Drawing 1", "table": 4, "action": "split", "row": 5}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `table` | integer | required | Table id (drawing_get). |
| `action` | one of `format_cell`, `merge_right`, `merge_below`, `merge`, `unmerge`, `insert`, `remove`, `duplicate`, `columns_equally`, `rows_equally`, `resize`, `split`, `unsplit`, `flip_x`, `flip_y`, `precision`, `order`, `bom_type`, `fixed_corner`, `text_height`, `fit` | required | The menu action. |
| `row` | integer |  | The cell's row (0 = header / first row). |
| `column` | integer |  | The cell's column. |
| `to_row` | integer |  | A block's last row (default `row`). |
| `to_column` | integer |  | A block's last column (default `column`). |
| `scope` | one of `cell`, `row`, `column`, `table` |  | format_cell: what it applies to. |
| `align` | one of `left`, `center`, `right` |  | format_cell: horizontal alignment (default: the cell's). |
| `bold` | boolean |  | format_cell: bold (default: the cell's). |
| `height` | number or null |  | format_cell: text height mm (null: the table's); resize: row height mm. |
| `width` | number |  | resize: column width mm. |
| `mode` | one of `all`, `horizontal`, `vertical` |  | merge. |
| `at` | one of `column_left`, `column_right`, `row_above`, `row_below` |  | insert. |
| `what` | one of `columns`, `rows` |  | remove, duplicate. |
| `decimals` | integer or null |  | precision. |
| `to` | string or number |  | order, bom_type, fixed_corner, text_height: the choice. |

#### drawing_annotation_edit

Edit an annotation from its menus: jogs, leaders, nodes, foreshorten, groups, detach, align centermark, stacked callouts (edits the document (one undoable history entry)).

The annotation context menus, one edit each with the menu's edit text:
- Linear dimensions: `add_jog` / `remove_jog` on extension line `line` (0 / 1; a 45° jog, 11.2 mm; a short line first moves the dimension line out); linear and ordinate dimensions: `break` (`on`: Break / Unbreak dimension).
- `add_leader` to `point` (sheet mm; a vertex or edge point of a view, else the sheet) on GD&T frames, callouts, surface finish and weld symbols and notes (a note's starts at its landing's bend; a note's or callout's at `from_node` [leader, node]); `remove_leaders` (a callout's added ones).
- Note and callout leaders (a callout's leader 0 is the one to its anchor, then the added ones; drawing_get `leaders`): `add_node` on leader `leader` at `point` (on the leader line), `remove_node` (`leader`, `node`; its branches then start at the bend), `delete_leader` (`leader`; a callout's leader 0 leaves it in space), `move_leader` (Move by grip: leader `leader`'s end to `point`; onto a vertex, circle center or edge it attaches there, elsewhere it stays on the sheet; a callout's leader 0 only onto a model point).
- Radial dimensions: `foreshorten`; `remove_foreshorten`.
- Datum targets: `hide_dimension` (its position dimensions), `show_dimensions` (`all`: All targets, else This target).
- Notes and callouts: `group` with view `view` (they move with it; `with`: more annotation ids), `remove_from_group`; `group_annotation` with `with`, `remove_annotation_from_group`.
- `detach` (Detach from geometry: stays put, no longer follows the model).
- Centermarks: `align_centermark` to the straight edge at `point`.
- Callouts (with a BOM table in the drawing): `stacked_callouts` — the `parts` (names) stacked under it (Add stacked callouts…; [] clears).
- Ordinate dimensions: `reset_ordinate` (Reset ordinate; as in the editor nothing moves: the values always sit on the group's text line, so no edit).

Examples:

```json
{"drawing": "Part 1 Drawing 1", "annotation": 12, "action": "add_jog", "line": 0}
{"drawing": "Part 1 Drawing 1", "annotation": 14, "action": "add_leader", "point": [190, 175]}
{"drawing": "Part 1 Drawing 1", "annotation": 15, "action": "group_annotation", "with": [16]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `annotation` | integer | required | Annotation id (drawing_get). |
| `action` | one of `add_jog`, `remove_jog`, `add_leader`, `remove_leaders`, `add_node`, `remove_node`, `delete_leader`, `move_leader`, `foreshorten`, `remove_foreshorten`, `hide_dimension`, `show_dimensions`, `group`, `remove_from_group`, `group_annotation`, `remove_annotation_from_group`, `detach`, `align_centermark`, `stacked_callouts`, `break`, `reset_ordinate` | required | The menu action. |
| `line` | one of 0, 1 |  | add_jog / remove_jog: the extension line. |
| `point` | array of number |  | add_leader: where it points; add_node: on the leader line; move_leader: where the leader's end goes; align_centermark: on a straight edge (sheet mm). |
| `leader` | integer |  | add_node / remove_node / delete_leader / move_leader: the leader (a note's 0 = first; a callout's 0 = the one to its anchor). |
| `node` | integer |  | remove_node: the node (0 = nearest the note). |
| `from_node` | array of integer |  | add_leader on a note or callout: branch from [leader, node]. |
| `all` | boolean |  | show_dimensions: All targets. |
| `view` | integer |  | group: the view to group with. |
| `with` | array of integer |  | group / group_annotation: the other annotations. |
| `parts` | array of string |  | stacked_callouts: part names. |
| `on` | boolean |  | break: Break (true) or Unbreak (false) dimension. |

#### drawing_mbd

MBD status: the model datums of the drawing's Part Studios, and a row's Show (edits the document (one undoable history entry)).

The MBD status panel: each model datum (from the Part Studios the views show; `all_sheets` or the shown sheet's) with whether some view shows it. `show` (a row's Show: `name`) puts a hidden one back where it was; refused, as live, when its view shows no model annotations. One edit ("Show").

Examples:

```json
{"drawing": "Drawing 1"}
{"drawing": "Drawing 1", "show": "Datum A"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `all_sheets` | boolean |  | List every sheet's (the panel's All models), not only the shown sheet's. |
| `show` | string |  | Show this row (its name). |

#### drawing_table_properties

A placed table's properties (BOM, cut list, revision, custom or general table) (edits the document (one undoable history entry)).

The table's right-click › … table properties dialog, one edit with its name. BOM: `structured`, `bottom_to_top`, `corner` ("BOM table properties"); cut list: `bottom_to_top`, `corner` ("Cut list table properties"); revision: `see_sheet1`, `visible_rows` (1-100), `newest_top`, `hide_callouts`, `corner` ("Revision table properties"); custom: `bottom_to_top`, `corner` ("Custom table properties"; its table and Part Studio: drawing_custom_table with `id`); general and hole tables: Table properties… `corner`, the table staying where it is ("Table fixed corner"). Fields not given keep their values; a field the table's dialog doesn't have is refused.

Examples:

```json
{"drawing": "Assembly 1 Drawing 1", "table": 1, "structured": true, "corner": "top_left"}
{"drawing": "Part 1 Drawing 1", "table": 2, "visible_rows": 8, "newest_top": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `table` | integer | required | Table id (drawing_get). |
| `corner` | one of `top_left`, `top_right`, `bottom_left`, `bottom_right` |  | Fixed corner. |
| `structured` | boolean |  | BOM: Structured (else Flattened). |
| `bottom_to_top` | boolean |  | BOM, cut list, custom: rows bottom to top. |
| `see_sheet1` | boolean |  | Revision: "See sheet 1" on the other sheets (else Linked). |
| `visible_rows` | integer |  | Revision: visible rows. |
| `newest_top` | boolean |  | Revision: order Bottom to top (newest at the top). |
| `hide_callouts` | boolean |  | Revision: hide callouts no longer in the table. |

#### drawing_copy

Copy views (with their derived views, annotations and tables), or annotations and tables.

Copy (Ctrl+C, the view and annotation menus): with `views`, those views with the views derived from them and their annotations; else the `annotations` and `tables`. Kept for drawing_paste in this session, and returned as `clipboard` (the editor's clipboard text, to paste into another session or drawing).

Examples:

```json
{"drawing": "Part 1 Drawing 1", "views": [2]}
{"drawing": "Part 1 Drawing 1", "annotations": [3, 4]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `views` | array of integer |  | View ids. |
| `annotations` | array of integer |  | Annotation ids (without `views`). |
| `tables` | array of integer |  | Table ids (without `views`). |

#### drawing_paste

Paste copied views, annotations and tables centered at a point (edits the document (one undoable history entry)).

Paste (Ctrl+V, the drawing menus): what drawing_copy copied (or `clipboard` text), centered on `position` (sheet mm), one edit "Paste views". Returns the new view ids and annotation ids.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "position": [300, 150]}
{"drawing": "Part 1 Drawing 1", "sheet": 2, "position": [200, 150]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `position` | array of number | required | Where the copy's center goes (sheet mm). |
| `clipboard` | string |  | Clipboard text from drawing_copy (default: this session's copy). |

#### drawing_paste_table

Paste spreadsheet cells as a new table (edits the document (one undoable history entry)).

Paste as table: spreadsheet text (rows of tab-separated cells, as a spreadsheet copies them) becomes a general table with its top-left corner at `position` (sheet mm); one edit "Paste as table". Returns the table id.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "text": "Item\tQty\nBolt\t4", "position": [20, 280]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `text` | string | required | Rows separated by newlines, cells by tabs. |
| `position` | array of number | required | Top-left corner (sheet mm). |

#### drawing_replace

Replace a view's model reference, or a BOM table's assembly (edits the document (one undoable history entry)).

With `view`: the reference's Replace… (sheets panel reference row, view row): on every sheet, the base views showing that view's source (and part) show `source` (and `part`) instead. With `table` (a BOM table): its Replace, every base view on the sheet showing the table's assembly shows `source`. One edit "Replace".

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 1, "source": "Part Studio 2"}
{"drawing": "Assembly 1 Drawing 1", "table": 1, "source": "Assembly 2"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer |  | A view whose reference to replace. |
| `table` | integer |  | A BOM table whose source to replace (instead of `view`). |
| `source` | string | required | The new Part Studio or Assembly (tab name or id). |
| `part` | string |  | With `view`: the new part (id or name) of a Part Studio; default the whole Part Studio. |

#### drawing_replicate

Replicate annotations from one view onto another (beta) (edits the document (one undoable history entry)).

Select for replication ▸ View (or Region… with `region`), then Replicate annotations on view `to`: a copy of each annotation where `to` shows the same geometry (each reference the same model point, or a vertex / circle center at the mapped spot); the others are skipped. With a region, `at` is where its box's center goes on `to` (sheet mm; snapped to the nearest matching vertex or center within 3 mm). One edit "Replicate annotations"; the reply's message is the editor's ("Successfully replicated 7/7 annotations").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "from": 1, "to": 5}
{"drawing": "Part 1 Drawing 1", "from": 1, "region": [[180, 150], [230, 190]], "to": 5, "at": [320, 170]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `from` | integer | required | The view whose annotations to replicate. |
| `region` | array of array of number |  | Region…: two opposite corners (sheet mm); only the annotations drawn inside. |
| `to` | integer | required | The view to copy them onto. |
| `at` | array of number |  | With `region`: the box's center on `to` (sheet mm). |

#### drawing_switch_to_reference

The Part Studio, Assembly or Feature Studio a view or table refers to (Switch to …).

Switch to reference (view reference menu), Switch to <assembly> (BOM table), Switch to Custom tables : <Part Studio> / Switch to <Feature Studio> (custom table): the target tab (id, name, kind), the part a view shows, a pinned version. With the app open on the document (live mode) the window switches to that tab, as the menu item does.

Examples:

```json
{"drawing": "Part 1 Drawing 1", "view": 1}
{"drawing": "Part 1 Drawing 1", "table": 3, "feature_studio": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `view` | integer |  | A view. |
| `table` | integer |  | A BOM or custom table (instead of `view`). |
| `feature_studio` | boolean |  | Custom table: its Feature Studio instead of its Part Studio. |

#### drawing_open_feature_studio

The Feature Studio a custom table comes from (Open Feature Studio in linked document).

A custom table's Feature Studio: in a linked document (a table defined in another document's version) the document's path, the version and the Feature Studio tab, and with the app open (live mode) the document opens in a window, as "Open Feature Studio in linked document" does; in this document the tab, switched to ("Switch to <Feature Studio>").

Examples:

```json
{"drawing": "Part 1 Drawing 1", "table": 3}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `table` | integer | required | A custom table. |

#### drawing_switch_sheet

Show another sheet (sheet tabs; First / Last / Next / Previous sheet) (edits the document (one undoable history entry)).

The sheet the drawing shows: `sheet` (number or name) or `step` first | last | next | previous (Home / End / Page down / Page up). As in Onshape a sheet switch is an undoable change ("Switch sheet"); the window follows it (live mode).

Examples:

```json
{"drawing": "Part 1 Drawing 1", "step": "next"}
{"drawing": "Part 1 Drawing 1", "sheet": "Sheet2"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `sheet` | integer or string |  | Sheet: number (1 = first) or name ("Sheet1"). Default: the sheet the drawing shows. |
| `step` | one of `first`, `last`, `next`, `previous` |  | Instead of `sheet`. |

#### drawing_sheets_panel

The Sheets panel: every sheet with its views and tables (Ctrl+S, Display sheet menu).

What the Sheets panel lists: each sheet (number, name, the one shown) with its views (id, kind, label) and tables (id, kind). `open` (default true) opens it in the window (live mode); false closes it.

Examples:

```json
{"drawing": "Part 1 Drawing 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `drawing` | string | required | Drawing tab, by name ("Part 1 Drawing 1") or id. |
| `open` | boolean |  | Open (true) or close the panel. |

### ui

#### ui_state

The document window's state: the tab shown, panels, the version viewed, compare and repair.

What the window shows besides the model: the tab shown (`active_tab`), the version viewed (`viewing`, null: the workspace), the Tab manager (open, search), the drawings whose Sheets panel is open, compare mode (base and target) and the Repair panel. Set by the window tools (tab_activate, tab_manager, history_view, history_compare, history_repair_view, drawing_sheets_panel…); with the app open (live mode) they change its window too.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |

#### ui_preferences

Graphics preferences (the camera menu's Graphics preferences…).

The user's Graphics preferences: `graphics` without fields replies with them (ambient_occlusion 0–1, fov_deg 20–120, line widths in px 0.1–3, shininess 0.01–1, colors "#rrggbb", construction [dash, gap, dash, gap] px); with fields, those change and are saved (per user, for every document), as the dialog's ✓ does. render_image draws with them; with the app open (live mode) every window applies them at once.

Examples:

```json
{}
{"graphics": {"fov_deg": 60, "line_width": 1.5, "sketch_constrained": "#202020"}}
```

| field | type | | description |
|---|---|---|---|
| `graphics` | object |  | Preferences to change, by name (see the reply). |

#### ui_toolbar

The Part Studio's or Assembly's toolbar customization (Customize toolbar…) and the S shortcut toolbars.

The user's toolbar settings (per user, for every document): `removed` (tool ids taken off the Part Studio toolbar), `group_order` (the tool sets' order, indices of the default sets), `hide_search` (Hide "Search tools"), `shortcut_tools` (the S toolbar's feature tool ids; null: Onshape's ten), `sketch_shortcut_tools` / `drawing_shortcut_tools` (the sketch and drawing S toolbars' tools by name; null: the defaults). Without `set` replies with them; `set` changes the given ones and saves them, as Customize toolbar… and Preferences › Shortcut toolbars do. With `tab` an Assembly, the assembly toolbar's: `removed` (its tool ids, `sets` lists them), `order` (the tool sets' order), `hide_search`, `shortcut_tools` (Preferences › Shortcut toolbars › Assembly: tool ids; null: Onshape's eleven). With the app open the windows' toolbars follow.

Examples:

```json
{}
{"set": {"removed": ["helix"], "shortcut_tools": ["sketch", "extrude", "fillet"]}}
{"tab": "Assembly 1", "set": {"removed": ["tb-gear"], "hide_search": true}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | An Assembly tab for the assembly toolbar (default: the Part Studio toolbar). |
| `set` | object |  | Settings to change (see the reply). |

#### ui_search_tools

Search tools… (Alt+C): the tools a text finds, in Onshape's order.

The toolbar's Search tools box: in a Part Studio the tools (features, custom features included; with `sketch`, in a sketch: the sketch toolbar's tools by their plain names) whose names hold the typed letters in order, ranked by where the first letter matches, then how tightly the letters sit; in an Assembly the toolbar tools whose names start with the text, then contain it, then hold its letters in order (shorter names first; at most 10). Each with its id (feature_add `type` / the assembly tool's icon) and key. Start one with feature_add or the assembly ops.

Examples:

```json
{"text": "fil"}
{"text": "rect", "sketch": true}
{"tab": "Assembly 1", "text": "mate"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the window's tab. |
| `text` | string | required | The search text. |
| `sketch` | boolean |  | Part Studio: search as in a sketch (the sketch tools). |

#### ui_shortcuts

The keyboard shortcuts (Help › Keyboard shortcuts, the Shift+/ sheet).

The sheet's rows per tab (General, Part Studio, Assembly, 3D view, Sketch, Feature Studio, Drawing): each action with a key (the user's customized key, else Onshape's default; `customized`, `disabled`) and the Onshape-controlled ones (`locked`), by name. `tab` keeps one tab; `search` matches names and keys, as the sheet's search box.

Examples:

```json
{"tab": "Sketch"}
{"search": "fillet"}
```

| field | type | | description |
|---|---|---|---|
| `tab` | one of `General`, `Part Studio`, `Assembly`, `3D view`, `Sketch`, `Feature Studio`, `Drawing` |  | Only this tab. |
| `search` | string |  | Search text (names and keys). |

#### ui_select

The window's selection: select, clear, Select ▸ related entities, Select other…, same configuration.

What is selected in a Part Studio, Assembly or Drawing window (per tab; `get` reads it). Part Studios: `set` / `add` / `remove` entity ids (query_entities: parts, faces, edges, vertices, sketches, planes), `clear` (Clear selection), `related` (the context menu's Select ▸ item `criterion` grown from the first of `refs`; Bounded faces stops at the other `refs`, which stay selected), `create_selection` (Create selection…: the same from the `refs` (its Selection field), with `patterns` (Include patterns) the pattern instances too, then Add selection: added to the selection), `select_other` (Select other…: the faces under graphics-area point `at` [x, y] through the window's camera (view_camera), nearest first; `index` picks one, else the list is returned; in an assembly the instances). Assemblies: `set` / `add` / `remove` instances (ids or names), `clear`, `same_configuration` (Select instances with same configuration as the first of `refs`; `same_part` for same part and same configuration). Drawings: `clear`. With the app open the window selects the same.

Examples:

```json
{"action": "set", "refs": ["body:…"]}
{"action": "related", "refs": ["face:…"], "criterion": "Tangent connected faces"}
{"action": "select_other", "at": [640, 400], "index": 1}
{"tab": "Assembly 1", "action": "same_configuration", "refs": ["Part 1 <1>"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio, Assembly or Drawing tab, by name or id. Default: the window's tab. |
| `action` | one of `get`, `set`, `add`, `remove`, `clear`, `related`, `create_selection`, `select_other`, `same_configuration` | required | What to do. |
| `refs` | array of string |  | Entity ids, or in an assembly instance ids or names. |
| `criterion` | one of `Tangent connected faces`, `Loop/chain connected edges`, `Tangent connected edges`, `Parallel edges`, `Equal length/radius edges`, `All faces of part`, `Protrusion faces`, `Pocket faces`, `Hole faces`, `Fillet faces`, `Loop/chain connected edges`, `Bounded faces` |  | related: the Select ▸ item. |
| `at` | array of number |  | select_other: the point [x, y] in graphics-area pixels. |
| `index` | integer |  | select_other: select the index-th entity under the point (0 = nearest). |
| `same_part` | boolean |  | same_configuration: same part and same configuration. |
| `patterns` | boolean |  | create_selection: Include patterns. |

#### ui_list

The feature and instance lists: expand / collapse folders, subassemblies and mates, filter the feature list, find an instance.

View state of a tab's lists (`get` reads it). Part Studios: `expand_all` / `collapse_all` (Expand all in list / Collapse all in list: the folders), `expand` / `collapse` folders `items` (names), `filter` (Filter feature list: `text` as the list's search box takes it — names and type names, or `:errors`, `:suppressed`, `:variable "name"`, `:type x`, …; "" clears; returns the features shown). Assemblies: `expand_all` / `collapse_all` (the instance tree, patterns and mates), `expand` / `collapse` `items`: subassembly instances (Expand selected / Collapse selected) or mates (a mate row's Expand / Collapse), `find` (Find in instance list: selects the instance, its list row shown). With the app open the window shows the same.

Examples:

```json
{"action": "collapse_all"}
{"action": "filter", "text": ":variable \"width\""}
{"tab": "Assembly 1", "action": "expand", "items": ["Subassembly 1 <1>"]}
{"tab": "Assembly 1", "action": "find", "items": ["Part 1 <2>"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the window's tab. |
| `action` | one of `get`, `expand_all`, `collapse_all`, `expand`, `collapse`, `filter`, `find` | required | What to do. |
| `items` | array of string |  | expand / collapse / find: folder names (Part Studio); instance or mate ids or names (Assembly). |
| `text` | string |  | filter: the feature list's search text. |

### history

#### history_view

View the document at a version or history entry (Versions and history › Open), or return to the workspace (edits the document (one undoable history entry)).

Open a version or change read-only, as the history panel's Open does: every later call reads the document as it was then, edits are refused ("This is a read-only view of history…") until `to` "workspace". Replies with what the version holds (tabs; Part Studios' features and parts). With the app open (live mode) its window shows it.

Examples:

```json
{"to": "V1"}
{"to": "workspace"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `to` | string | required | A version name or id, a microversion id (history_list), or "workspace". |

#### history_compare

Compare two history points (Versions and history › Compare).

Compare mode between `base` (default: what the window shows, the workspace or the version viewed) and `target` (a version, a change or "workspace"): each tab's state (differs, same, not_supported for drawings, only_in_base) and, per Part Studio that differs, its features' changes (added, removed, modified, moved; parameters changed). `stop` leaves compare mode. With the app open (live mode) the window enters compare mode.

Examples:

```json
{"target": "V1"}
{"stop": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `target` | string |  | Version name or id, microversion, or "workspace". |
| `base` | string |  | The other side (default: what the window shows). |
| `stop` | boolean |  | Leave compare mode. |

#### history_repair_view

View a history entry in the Repair panel (Versions and history › View in repair).

The Part Studios as they were at `at` (a version or a change), to repair references broken since: their features (with status) and parts. With the app open (live mode) the Repair panel opens on it. `close` closes it.

Examples:

```json
{"at": "V1"}
{"close": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `at` | string |  | Version name or id, or microversion. |
| `close` | boolean |  | Close the Repair panel. |

#### history_undo

Undo the last edit (edits the document (one undoable history entry)).

With `tab`, undoes the last edit of that tab (Onshape's undo is per tab); otherwise the last edit of the document. `description` names the edit undone.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Tab whose edit to undo. |

#### history_redo

Redo the last undone edit (edits the document (one undoable history entry)).

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Tab whose edit to redo. |

#### history_list

The edit history of the current branch, newest first.

Microversions with their description ("Part Studio 1 :: Insert feature : Extrude 1"), author and time, plus versions and branches.

Examples:

```json
{"limit": 10}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `limit` | integer |  | At most this many entries (default 50). |

#### history_create_version

Create a named version of the current state (edits the document (one undoable history entry)).

Examples:

```json
{"name": "V1", "description": "First prototype"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `name` | string | required | Version name ("V1"). |
| `description` | string |  | Description. |

#### history_create_branch

Create a branch (workspace) from the current state and switch to it (edits the document (one undoable history entry)).

Examples:

```json
{"name": "experiment"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `name` | string | required | Branch name. |

#### history_switch_branch

Switch to another branch (edits the document (one undoable history entry)).

Examples:

```json
{"branch": "experiment"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `branch` | string | required | Branch name or id. |

#### history_merge

Merge another branch into the current one (edits the document (one undoable history entry)).

Part Studios merge feature by feature.

Examples:

```json
{"branch": "experiment"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `branch` | string | required | Branch to merge from (name or id). |

#### history_restore

Restore the document to an earlier microversion or version (edits the document (one undoable history entry)).

Adds a new microversion with that state (history is kept).

Examples:

```json
{"to": "V1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `to` | string | required | Microversion id or version name/id (from history_list). |

### part

#### part_rename

Rename a part (Parts list › Rename) (edits the document (one undoable history entry)).

The part's name in the parts list, BOMs and drawings ("Rename : <name>").

Examples:

```json
{"part": "Part 1", "name": "Base plate"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `part` | string | required | Part id (`body:…` from query_parts) or name. |
| `name` | string | required | New name. |

#### part_set_properties

Set a part's properties (the Properties dialog) (edits the document (one undoable history entry)).

Name, description, part number, revision, vendor, project, product line, titles and a mass override (kg). Fields not given keep their value; an empty text clears it. Fields configured for the active configuration keep the part's own value, as in the dialog.

Examples:

```json
{"part": "Part 1", "part_number": "BP-100", "revision": "A"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `part` | string | required | Part id (`body:…`) or name. |
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

#### part_configured_properties

Configured part properties: add a property column, set a value, delete a column or the table (edits the document (one undoable history entry)).

The Configuration panel's configured properties table of a part (Configure part properties): without `action`, replies with the table (`properties`, `rows`: each row's configuration and values). `add_property` adds a column ("Add configured property"; the first one creates the table with a row for the active configuration; values start from the part's own); `set` sets `property` to `value` in the row of the active configuration (or `configuration`: {input: value}), adding that row when missing ("Edit configured properties"); `delete_column` is a column's right-click › Delete column ("Delete configured property"); `delete_table` is ⋯ › Delete table ("Delete configured properties table").

Examples:

```json
{"part": "body:F1a2b.0", "action": "add_property", "property": "Description"}
{"part": "body:F1a2b.0", "action": "set", "property": "Description", "value": "Long bracket"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `part` | string | required | Part id (`body:…`) or name. |
| `action` | one of `add_property`, `set`, `delete_column`, `delete_table` |  | What to do (default: read the table). |
| `property` | one of `Appearance`, `Name`, `Description`, `Part number`, `Revision`, `Vendor`, `Project`, `Product line`, `Material`, `Title 1`, `Title 2`, `Title 3`, `Not revision managed`, `Exclude from all BOMs`, `Unit of measure`, `Mass`, `Tessellation quality` |  | The property (column). |
| `value` | string |  | set: the value ("true" / "false" for checkboxes). |
| `configuration` | object |  | set: the row's configuration, input id → value (default: the active one). |

#### part_annotation

Part Studio annotations: add a datum, list, delete (edits the document (one undoable history entry)).

Inspection panel › Datum and the annotations' right-click › Delete annotation (Model definitions; annotations aren't features). `add_datum` puts datum `letter` (default the next free letter) on planar face `face`, lying in the face's plane at its center ("Datum"); `delete` removes annotation `annotation` ("Delete annotation"). Without `action`, lists them.

Examples:

```json
{"action": "add_datum", "face": "face:eJyrVkr…"}
{"action": "delete", "annotation": 1}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `action` | one of `add_datum`, `delete` |  | What to do (default: list). |
| `face` | string |  | add_datum: a planar face id. |
| `letter` | string |  | add_datum: the datum letter. |
| `annotation` | integer |  | delete: the annotation id. |

#### part_set_material

Assign a material to parts (density for mass properties) (edits the document (one undoable history entry)).

The Material panel: a library material by name (`material_list`), or a custom material with `density`. One edit ("Assign material").

Examples:

```json
{"parts": ["Part 1"], "material": "Aluminum - 6061"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `parts` | array of string |  | Part ids (`body:…`) or names. |
| `part` | string |  | One part (instead of `parts`). |
| `material` | string |  | Library material name, e.g. "Aluminum - 6061"; or the custom material's name with `density`. |
| `library` | string |  | Library to take it from (default: the first that has it). |
| `density` | number |  | Custom material: density in kg/m³. |

#### part_set_appearance

Set the color and transparency of parts, faces or a feature (the Appearance editor) (edits the document (one undoable history entry)).

Give `parts`, `faces` or `feature`. Color as "#3b61b4" or [r, g, b] (0–255, sRGB); transparency 0 (opaque) to 1. Parts also take the tessellation quality. One edit ("Edit appearance").

Examples:

```json
{"parts": ["Part 1"], "color": "#3b61b4"}
{"faces": ["face:…"], "color": [200, 40, 40], "transparency": 0.3}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `parts` | array of string |  | Part ids or names. |
| `faces` | array of string |  | Face ids (`face:…`). |
| `feature` | string |  | Feature id or name (its faces; a sketch: its curves). |
| `color` | string or array | required | "#rrggbb" or [r, g, b]. |
| `transparency` | number |  | 0 opaque (default) … 1 invisible. |
| `tessellation` | one of `auto`, `coarse`, `medium`, `fine`, `very_fine` |  | Parts: tessellation quality (default auto). |

#### part_remove_appearance

Remove custom appearances (Appearance panel › Remove appearance) (edits the document (one undoable history entry); deletes or overwrites).

A part's own appearance (`parts`), or face / feature appearances by index (`appearances`, from part_appearances).

Examples:

```json
{"parts": ["Part 1"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `parts` | array of string |  | Part ids or names. |
| `appearances` | array of integer |  | Face / feature appearance indices. |

#### part_appearances

The Part Studio's custom appearances and materials (the Appearance panel).

Each part's material, density and own appearance, and the face / feature appearances with their index.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |

### material

#### material_list

List the materials of the material libraries.

The Material panel's libraries: the Onshape Material Library and the document's Material Library tabs, with each material's category and density (kg/m³).

Examples:

```json
{"search": "steel"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `search` | string |  | Only materials whose name or category contains this. |

### query

#### query_parts

The parts, surfaces and curves of a Part Studio.

Each body with its id (`body:…`), name, kind, volume, surface area, bounding box and face/edge/vertex counts. Lengths in the workspace units (see `units`).

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `configuration` | object |  | In this configuration: {input: option or value} by id or name (config_list); the other inputs as shown. |

#### query_entities

Faces, edges or vertices with stable ids and their geometry, to select for features.

Lists the faces / edges / vertices of the parts (or of `part`) with an id to pass to selection parameters, the feature that made them, and geometry: faces: surface type, area, centroid, normal (planar), axis and radius (cylinders, cones...); edges: curve type, length, endpoints, midpoint, center/radius (circles); vertices: position. Filters narrow the list: `surface`/`curve` type, `normal` (planar faces whose outward normal is within 1° of it), `direction` (line edges along it, either way, or circles around it), `created_by` (feature id or name), `near` (closest first: by the distance to the entity, then to its centroid; max `limit`). `at` picks several entities in one call: for each point, the closest entity that passes the filters (in the points' order). With `context: true`, the shown Assembly context's geometry instead (Modeling in context: assembly_edit_in_context); `part` is then an instance name.

Examples:

```json
{"kind": "faces", "surface": "plane", "normal": [0, 0, 1]}
{"kind": "edges", "created_by": "Extrude 1"}
{"kind": "edges", "direction": [0, 0, 1]}
{"kind": "edges", "at": [[0, 0, 3], [80, 0, 3], [40, 44, 6]]}
{"kind": "faces", "context": true, "surface": "plane"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `kind` | one of `faces`, `edges`, `vertices` | required | Which entities. |
| `part` | string |  | Only this part (`body:…` id or part name). |
| `surface` | one of `plane`, `cylinder`, `cone`, `sphere`, `torus`, `bspline`, `extruded`, `revolved`, `other` |  | Faces: only this surface type. |
| `curve` | one of `line`, `circle`, `ellipse`, `bspline`, `other` |  | Edges: only this curve type. |
| `normal` | array of number |  | Faces: outward normal direction, e.g. [0,0,1] for faces facing +Z. |
| `created_by` | string |  | Only entities made by this feature (id or name). |
| `direction` | array of number |  | Edges: lines parallel to this direction (either way) or circles whose axis it is, within 1°; e.g. [0,0,1] for vertical edges. |
| `near` | array of number or string |  | Sort by distance from this point (workspace units). |
| `at` | array of array of number or string |  | Points (workspace units): the closest entity to each, in order (replaces `near` / `limit`). |
| `limit` | integer |  | At most this many results (default 200). |
| `context` | boolean |  | The shown Assembly context's instances instead of the parts. |

#### query_datums

Planes, the origin, sketches and mate connectors of a Part Studio.

Default planes (`plane:top`, `plane:front`, `plane:right`), Plane features (`fplane:…`), sketches (`sketch:…`) with their plane frames, and mate connectors (`mc:…`).

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |

#### query_measure

Measure entities (lengths, areas, radii, distances, angles).

The Measure tool: one entity gives its length/area/radius...; two give the minimum distance (with dx, dy, dz) and the angle between them. Values in workspace units.

Examples:

```json
{"entities": ["face:…", "face:…"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `entities` | array of string | required | Reference ids. |

#### query_mass_properties

Mass properties: volume, surface area, mass, center of mass, inertia.

Of the given parts, or all parts. Mass, center of mass and inertia need a material (or density) on every part, as in Onshape; mass overrides apply. Inertia (kg·length²) is about the center of mass, or about mate connector `reference`'s axes.

Examples:

```json
{}
{"parts": ["Part 1"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `parts` | array of string |  | Part ids (`body:…`) or names; default all. |
| `reference` | string |  | Mate connector id (`mc:…`) for the inertia axes. |
| `configuration` | object |  | In this configuration: {input: option or value} by id or name (config_list); the other inputs as shown. |

### analysis

#### analysis_curvature_map

Curvature color map: the parts' surface curvature.

The Curvature color map (Analysis tools menu) as numbers: each part's surface sampled at its mesh vertices with the Curvature menu's `kind` — gaussian (1/length², default), mean (1/length), max_radius or min_radius (length) — giving each part's min / max over its curved surface, the share that is flat, and the `points` most extreme spots (largest curvature, smallest radius).

Examples:

```json
{}
{"parts": ["Part 1"], "kind": "min_radius", "points": 5}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `parts` | array of string |  | Part ids (`body:…`) or names; default all shown parts. |
| `kind` | one of `gaussian`, `mean`, `max_radius`, `min_radius` |  | Curvature. |
| `points` | integer |  | Extreme spots to list. |

#### analysis_thickness

Thickness analysis: the parts' wall thickness.

The Thickness analysis tool (Analysis tools menu): each part's surface sampled and measured with the color bar's Method — rolling_ball (default: the largest ball touching both walls), ray (straight through the part), or their _gradient forms (the rate of change, %). Returns each part's min / max / mean over its samples and, with `below`, the share of the surface thinner than that and up to `points` of the thinnest spots (location, thickness).

Examples:

```json
{}
{"parts": ["Part 1"], "below": "1.5 mm", "points": 5}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `parts` | array of string |  | Part ids (`body:…`) or names; default all shown parts. |
| `method` | one of `rolling_ball`, `rolling_ball_gradient`, `ray`, `ray_gradient` |  | Method. |
| `below` | number or string |  | Report surface thinner than this (a length, or % for gradients). |
| `points` | integer |  | Thinnest spots to list. |

#### analysis_curvature

Curvature of edges, sketch curves and faces (Curve/surface analysis).

The minimum radius of curvature over the selection and where it is, the inflection points of sketch curves, and the number of curvature combs (one per curve; U and V isoparametric curves per face).

Examples:

```json
{"entities": ["edge:…"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `entities` | array of string | required | Edges, faces (`edge:…`, `face:…`) or sketch curves (`entity:…`). |
| `extend` | boolean |  | Extend across face: include the tangent-connected faces. |
| `u_curves` | integer |  | Faces: U curves (default 8). |
| `v_curves` | integer |  | Faces: V curves (default 8). |

#### analysis_deviation

Deviation between two curve chains or two surfaces (Deviation analysis).

Minimum and maximum distance from the first selection to the second, with the points on each.

Examples:

```json
{"first": "face:…", "second": "face:…"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `first` | array of string | required | Edges / sketch curves (or faces with `surface`). |
| `second` | array of string | required | The chain / faces to measure to. |
| `surface` | boolean |  | Surface deviation (faces) instead of curves. |

#### analysis_connection

Continuity where curves and faces meet (Connection analysis).

Each joint between the curves (G0 position, G1 tangent, G2 curvature, G3, or none) and each shared edge segment between the faces, with Onshape's tolerances (G1 0.3°, G2 2.5 %, G3 5 % unless given).

Examples:

```json
{"entities": ["edge:…"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `entities` | array of string | required | Edges, sketch curves and faces. |
| `g1_deg` | number |  | G1 angle tolerance (degrees). |
| `g2` | number |  | G2 relative tolerance. |
| `g3` | number |  | G3 relative tolerance. |

#### analysis_dihedral

Dihedral angle along edges (Dihedral analysis).

The minimum and maximum angle between the faces along the edges (degrees).

Examples:

```json
{"entities": ["edge:…"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `entities` | array of string | required | Edges. |

#### analysis_interference

Interference detection between parts.

Every pair of the parts whose material overlaps, with the overlap volume and its bounding box.

Examples:

```json
{}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `parts` | array of string |  | Parts (`body:…`). Default: every part. |

#### analysis_draft

Draft analysis: each face's draft angle toward a pull direction.

The Draft analysis tool (Analysis tools menu): with mold split direction `pull` (a plane, planar face, sketch, mate connector or straight edge; `flip` reverses it), minimum draft `angle` (default 3 deg) and Show undercut regions (`undercut`, default on), each face of the checked `parts` (default all shown) is classed as Onshape's legend colors it — steep (under the minimum), undercut, side_1 (toward the pull) or side_2 — with its draft angle range (degrees). Faces mixing classes report the share of each.

Examples:

```json
{"pull": "plane:top", "angle": "3 deg"}
{"pull": "plane:top", "flip": true, "undercut": false, "parts": ["Part 1"]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `pull` | string | required | Mold split direction: a plane, planar face, sketch, mate connector or straight edge id. |
| `flip` | boolean |  | Reverse the pull direction. |
| `angle` | number or string |  | Minimum draft angle (0–90 deg). |
| `undercut` | boolean |  | Show undercut regions. |
| `parts` | array of string |  | Entities to check: part ids (`body:…`) or names; default all shown parts. |

#### analysis_flatten

Flatten surfaces: a planar sheet of faces with its distortion, optionally exported.

The Flatten surfaces tool (Analysis tools menu): `faces` (contiguous) opened along `rips` (edges; closed faces open along their seams) flattened with minimum strain. Returns the 3D and flat areas, the strain range (negative: compressed, positive: stretched), the flat size and the boundary loops. `export` writes the sheet: DXF or SVG (`inner_edges`: the faces' shared edges too), STL or PARASOLID placed on the XY plane (best fit to the faces' footprint, outer side up) or at mate connector `position` (`angle`, `offset`).

Examples:

```json
{"faces": ["face:eJyrVkr…"]}
{"faces": ["face:eJyrVkr…"], "rips": ["edge:eJyrVkr…"], "export": {"format": "DXF", "file": "/tmp/flat.dxf"}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `faces` | array of string | required | Faces to flatten. |
| `rips` | array of string |  | Rip edges. |
| `export` | object |  | {"format": "DXF" \| "SVG" \| "STL" \| "PARASOLID", "file": path, "inner_edges": bool, "position": mate connector id, "angle": degrees, "offset": length, "origin": vertex id} |

### render

#### render_named_view

Save or delete a named view of a Part Studio (edits the document (one undoable history entry)).

The Named views dialog's "+" (save the view, here a standard orientation zoomed to fit, with an optional section like render_image's) or its remove button (`delete`). An empty name gives "View 1", "View 2"... render_image and drawing_insert_view take named views.

Examples:

```json
{"name": "Section A", "view": "front", "section": [{"plane": "plane:right"}]}
{"name": "Section A", "delete": true}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `name` | string | required | View name (empty: the next "View n"). |
| `view` | string |  | Standard orientation to save (default isometric). |
| `section` | array of object |  | Section planes saved with the view (as render_image). |
| `delete` | boolean |  | Delete the named view instead. |

#### render_image

Render a Part Studio or an Assembly to a PNG image from a standard or named view.

Draws the parts, sketches, planes and origin as the 3D view does (Onshape's look: white background, shaded with edges), zoomed to fit. With `tab` an Assembly: the solved assembly in its active configuration (standard views; sections on plane:top|front|right; `section_exclude` instance names). The view menu's options as in the app: `perspective`, `tangent_edges`, `hidden_edges`, `boundary_edges`, `isolate` / `transparent` (Make isolate / Make transparent), `normal_to` (View normal to), `high_quality` (View in high quality), and the analysis displays `zebra` (Zebra stripes) and `reflection` (Reflection analysis). Returned as an image (MCP image content) and, with `file`, written to disk.

Examples:

```json
{"view": "isometric", "file": "/tmp/part.png"}
{"tab": "Assembly 1", "view": "isometric", "section": [{"plane": "plane:front"}]}
{"view": "front", "section": [{"plane": "plane:right"}], "display_mode": "hidden_line"}
{"view": "front", "perspective": true, "tangent_edges": "phantom", "hidden_edges": true}
{"isolate": ["Part 1"], "zebra": {"stripes": 20}}
{"normal_to": "plane:front", "reflection": {"image": "Blue sky", "rotation": 45}}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the first Part Studio. |
| `section` | array of object |  | Section view: up to 3 planes [{"plane": a plane / planar face / sketch / mate connector id, "offset": length along its normal (default 0), "flip": cut the other side}]; the side the normal points to is cut away. A named `view` keeps its own section unless this is given. |
| `section_exclude` | array of string |  | Section view › Items to exclude: parts (`body:…`) left whole. |
| `view` | string |  | isometric (default), front, back, left, right, top, bottom, dimetric, trimetric, the name of a saved named view, or "window": the window's camera (view_camera; perspective and fit options then don't apply). |
| `width` | integer |  | Pixels (default 800). |
| `height` | integer |  | Pixels (default 600). |
| `display_mode` | one of `shaded_with_edges`, `shaded`, `hidden_line`, `translucent` |  | Render style (default shaded_with_edges). |
| `show_sketches` | boolean |  | Draw sketches. |
| `show_planes` | boolean |  | Draw the default and construction planes. |
| `perspective` | boolean |  | Perspective view (default orthographic). |
| `tangent_edges` | one of `visible`, `phantom`, `hidden` |  | Tangent edges visible / phantom / hidden. |
| `hidden_edges` | boolean |  | Hide/show hidden edges: draw the hidden ones. |
| `boundary_edges` | boolean |  | Part Studios: Highlight boundary edges. |
| `isolate` | array of string |  | Make isolate: these parts (Part Studio ids or names) or instances (Assembly), the rest as ghosts. |
| `transparent` | array of string |  | Make transparent: these parts or instances as ghosts. |
| `normal_to` | string |  | Part Studios: View normal to a planar face, plane, sketch or mate connector (replaces `view`). |
| `high_quality` | boolean |  | View in high quality (finer tessellation). |
| `zebra` | object |  | Zebra stripes: {"stripes": 35, "flip": false, "show_edges": true}. |
| `reflection` | object |  | Reflection analysis: {"image": an environment map name ("Blue sky", default the first) or an image file path, "rotation": degrees, "opposite": bool, "blur": bool, "show_edges": bool (default false)}. |
| `file` | string |  | Also write the PNG here. |

### view

#### view_camera

Move the graphics area's camera as the view menus do (zoom to fit / selection / window, zoom in / out, previous view, standard views, view normal to, align vertical, perspective).

The camera of a Part Studio's or Assembly's graphics area, or a drawing's zoom, as the window has it (kept per tab for the session; `get` reads it). 3D tabs: `zoom_fit` (Zoom to fit: the shown parts, sketches, planes and origin; an assembly's instances), `view` (a standard view zoomed to fit; `view`, default isometric), `zoom_selection` (the `refs`, else the window's selection (ui_select): parts, faces, edges, vertices, sketches, planes; an assembly's instances), `zoom_in` / `zoom_out` (Z / Shift+Z: ×1.75 about the centre, within Onshape's limits), `zoom_window` (`box`: two corners in graphics-area pixels), `previous` (Previous view: back one change), `normal_to` (View normal to the first of `refs`: a planar face, plane, sketch or mate connector; in an assembly a planar face {"instance", "entity"}; again on the same plane turns to its back), `normal_to_sketch` (View normal to sketch plane: `refs` [the sketch]), `align_vertical` (Align view with vertical axis: `refs` [a straight edge]), `perspective` (`on`). Drawings: `zoom_fit` / `zoom_sheet`, `zoom_in` / `zoom_out` (×2 / ×½), `zoom_window`. The graphics area is 1280×800 px until `viewport` sets it. With the app open the window does the same move at its own size. Returns the camera: the viewing direction and up (world), the target point and the visible height (document units), projection; a drawing's zoom (px per mm) and the sheet point at the centre (mm).

Examples:

```json
{"action": "zoom_fit"}
{"action": "normal_to", "refs": ["plane:top"]}
{"tab": "Drawing 1", "action": "zoom_window", "box": [[400, 200], [800, 500]]}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio, Assembly or Drawing tab, by name or id. Default: the window's tab. |
| `action` | one of `get`, `zoom_fit`, `zoom_sheet`, `view`, `zoom_selection`, `zoom_in`, `zoom_out`, `zoom_window`, `previous`, `normal_to`, `normal_to_sketch`, `align_vertical`, `perspective` | required | What to do. |
| `view` | one of `isometric`, `front`, `back`, `left`, `right`, `top`, `bottom`, `dimetric`, `trimetric` |  | view: the standard view (default isometric). |
| `refs` | array of string or object |  | zoom_selection / normal_to / normal_to_sketch / align_vertical: entity ids (query_entities); in an assembly instance ids or names, and for normal_to {"instance", "entity": a planar face id}. |
| `box` | array of array of number |  | zoom_window: [[x, y], [x, y]] corners in graphics-area pixels from the top left. |
| `on` | boolean |  | perspective: on or off. |
| `viewport` | array of number |  | The graphics area's size [width, height] in pixels from now on (default 1280×800). |

#### view_display

Display settings of a Part Studio or Assembly window: isolate, transparent, boundary edges, orient on edit, thickness off, mates, snap mode.

What the view and context menus change without editing the document, kept per tab (`ui_state`; nothing given: read them). `isolate` (Isolate…: these parts, the rest as ghosts) or `transparent` (Make transparent…: these as ghosts) replace the current isolation; `exit_isolate` ends it. Part Studios: `highlight_boundary` (Highlight boundary edges), `orient_on_edit` (Orient normal to sketch on edit), `thickness_analysis: false` (Turn thickness analysis off), `context_hidden` (the Assembly context banner's Hide/show instances). Assemblies (instances for isolate / transparent): `show_mates` (Hide/show mates: the mate indicators), `show_mates_mode` (Show mates mode), `snap_mode` (Snap mode). With the app open the window does the same; render_image `view: "window"` draws with them.

Examples:

```json
{"isolate": ["Part 1"]}
{"exit_isolate": true, "highlight_boundary": true}
{"tab": "Assembly 1", "show_mates": true, "snap_mode": false}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio or Assembly tab, by name or id. Default: the window's tab. |
| `isolate` | array of string |  | Isolate…: parts (ids or names) or, in an assembly, instances. |
| `transparent` | array of string |  | Make transparent…: parts or instances. |
| `exit_isolate` | boolean |  | Exit isolate. |
| `highlight_boundary` | boolean |  | Part Studio: Highlight boundary edges. |
| `orient_on_edit` | boolean |  | Part Studio: Orient normal to sketch on edit. |
| `thickness_analysis` | boolean |  | Part Studio: false = Turn thickness analysis off (run the analysis with analysis_thickness). |
| `context_hidden` | array of string |  | Part Studio: the shown Assembly context's Hide/show instances: the top-level instances (names or ids, context_list) to hide; [] shows all. |
| `show_mates` | boolean |  | Assembly: Hide/show mates. |
| `show_mates_mode` | boolean |  | Assembly: Show mates mode. |
| `snap_mode` | boolean |  | Assembly: Snap mode. |

### export

#### export_parts

Export a Part Studio's parts or an Assembly to a CAD or mesh file (STEP, Parasolid, IGES, ACIS, STL, OBJ, 3MF, glTF, GLB, JT, PVZ, Rhino) (edits the document (one undoable history entry)).

Onshape's Export dialog. Exports the visible parts (or `parts`) to `file`; with `tab` an Assembly, its visible instances (or `parts`: instance ids or names) — STEP keeps the assembly (the root product, each part once, placed by its instances), the other formats get the placed parts. Options as in the dialog: `y_up` (models oriented Y axis up), `version` (format version: STEP AP242/AP214/AP203, Parasolid 38.1..., Rhino 8...), `units` (mesh formats and STEP with custom units), `resolution` (coarse/medium/fine for mesh formats), `stl_text`, `individual` (one file per part, zipped), `include_hidden`, STEP `preprocessing` / `derived_tolerances` / `mbd_text_height`, IGES `iges_compatibility`, PVZ `group_composites`, glTF `compress`, URDF `geometry_format`.

Examples:

```json
{"format": "STEP", "file": "/tmp/bracket.step"}
{"format": "STL", "file": "/tmp/bracket.stl", "parts": ["Part 1"]}
{"tab": "Assembly 1", "format": "STEP", "file": "/tmp/assembly.step"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Part Studio tab, by name ("Part Studio 1") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio. |
| `format` | one of `PARASOLID`, `ACIS`, `STEP`, `IGES`, `PVZ`, `JT`, `RHINO`, `GLTF`, `GLB`, `OBJ`, `3MF`, `STL` | required | File format. |
| `file` | string |  | Output path; without an extension the format's is added ("part" -> "part.x_t"); individual files go in a .zip. Required unless `options` is "store" (then only its name is used; default "<tab>.<ext>"). |
| `options` | one of `download`, `download_and_store`, `store` |  | Options: write `file` (Download), and / or keep the file as a file tab right after the tab (Store file in a new tab; one edit "Store <file>"). |
| `parts` | array of string |  | Part ids (`body:…`) or names; default all visible parts. |
| `y_up` | boolean |  | Export models oriented Y axis up. |
| `version` | string |  | Format version (e.g. "AP214", "35.1", "7"). |
| `units` | one of `Centimeter`, `Foot`, `Inch`, `Meter`, `Millimeter`, `Yard` |  | File units (STL, OBJ...; STEP: custom units). |
| `resolution` | one of `Coarse`, `Medium`, `Fine` |  | Mesh resolution (default Medium). |
| `stl_text` | boolean |  | STL as text instead of binary. |
| `individual` | boolean |  | Export unique parts as individual files (zipped). |
| `include_hidden` | boolean |  | Include hidden parts. |
| `preprocessing` | one of `None`, `Automatic`, `Remove small entities only (advanced)`, `Remove small entities only (basic)`, `Remove small entities only (advanced) and convert all surfaces to b-surfaces` |  | STEP: Preprocessing (default None). |
| `derived_tolerances` | boolean |  | STEP: Include derived tolerances in export. |
| `mbd_text_height` | boolean |  | STEP: Use custom annotation text height for MBD export. |
| `iges_compatibility` | boolean |  | IGES: Use compatibility mode for surfaces and curves. |
| `group_composites` | boolean |  | PVZ: Group composite parts into a single part file. |
| `compress` | boolean |  | glTF / GLB: Compress. |
| `geometry_format` | one of `STL`, `GLTF`, `GLB`, `OBJ` |  | URDF: Geometry format (default STL). |

### import

#### import_files

Import CAD or mesh files (STEP, IGES, Parasolid, ACIS, JT, Rhino, SolidWorks/Inventor/NX/Solid Edge parts and assembly zips, STL, OBJ, 3MF, glTF) (edits the document (one undoable history entry)).

The Import dialog: each file becomes a Part Studio with an Import feature (or all of them one Part Studio with `combine`); a native assembly zip becomes an Assembly and its Part Studio. The files are kept as file tabs in a "CAD Imports" folder unless `keep_files` is false. One edit ("Import <files>"). Returns the new tabs and the imported parts. The window then shows the first new tab (`first`), and tools without `tab` act on it.

Examples:

```json
{"files": ["/tmp/bracket.step"]}
{"files": ["/tmp/a.x_t", "/tmp/b.x_t"], "combine": true}
```

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

Examples:

```json
{"tab": "Feature Studio 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Feature Studio tab (name or id); default: the first. |

#### featurestudio_edit

Replace a Feature Studio's source (saved, not yet committed) (edits the document (one undoable history entry); deletes or overwrites).

Typing in the Feature Studio: the text is saved as a draft ("Edit Feature Studio"); Part Studios keep the committed source until featurestudio_commit. Returns the compile notices.

Examples:

```json
{"tab": "Feature Studio 1", "source": "FeatureScript 2581;\n…"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Feature Studio tab (name or id); default: the first. |
| `source` | string | required | The whole FeatureScript source. |

#### featurestudio_commit

Commit a Feature Studio (or all) so Part Studios use its features (edits the document (one undoable history entry)).

Commit (Ctrl+S) / Commit all (`all`): the draft becomes the source Part Studios use; one edit per Feature Studio ("Commit Feature Studio"). Returns the custom feature types now available to feature_add.

Examples:

```json
{"tab": "Feature Studio 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `tab` | string |  | Feature Studio tab (name or id); default: the first. |
| `all` | boolean |  | Commit every Feature Studio with uncommitted changes. |

#### featurestudio_monitor

The FeatureScript notices panel's Monitor for a Part Studio: notices and println output.

Regenerates the Part Studio and returns what its FeatureScript notices panel's Monitor shows: the compile warnings of the Feature Studios its custom features come from and each custom feature's runtime notices (message, severity, `line` / `column`, stack of `at` line:col and `in` module (function), ending with the Part Studio and feature), in Onshape's order, then the text the features printed (`println`).

Examples:

```json
{"part_studio": "Part Studio 1"}
```

| field | type | | description |
|---|---|---|---|
| `doc` | string |  | Path of the .noshape document. Default: the current document (the last one created or opened). |
| `part_studio` | string |  | Part Studio tab (name or id); default: the first. |
<!-- END GENERATED REFERENCE -->
