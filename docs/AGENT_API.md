# noshape agent API: CLI and MCP server

Everything a person does in noshape can be done by a script or an AI agent through `noshape-cli`, which is
both a command line tool and an MCP (Model Context Protocol) server. Both are generated from one operation
registry (`crates/ops`), and every operation calls the same model code as the app's UI: the same feature
parameters, regeneration, sketch tools and document history. An edit made through the API is one undoable
history entry, described exactly as the UI describes it (`Part Studio 1 :: Insert feature : Extrude 1`).

## Quick start

Build: `OCCT_ROOT=/path/to/occt cargo build --release -p noshape-cli` (no GUI dependencies).

### Claude Code

```sh
claude mcp add noshape -- /path/to/noshape-cli mcp
```

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

## Operations

`noshape-cli tools` (or the MCP `tools/list`) is the authoritative list with full schemas. Groups:

| group | operations |
|---|---|
| ops | `ops_list` |
| document | `document_create`, `document_open`, `document_info`, `document_close`, `document_set_units`, `document_protect`, `document_unprotect` |
| tab | `tab_create`, `tab_rename`, `tab_duplicate`, `tab_delete` |
| feature | `feature_types`, `feature_schema`, `feature_add`, `feature_edit`, `feature_get`, `feature_list`, `feature_delete`, `feature_suppress`, `feature_rollback`, `feature_rename`, `feature_reorder`, `feature_set_visibility`, `feature_show_all` |
| sketch | `sketch_create`, `sketch_draw`, `sketch_constrain`, `sketch_dimension`, `sketch_set_dimension`, `sketch_delete`, `sketch_set_construction`, `sketch_get`, `sketch_trim`, `sketch_extend`, `sketch_split`, `sketch_spline_point`, `sketch_fillet`, `sketch_chamfer`, `sketch_offset`, `sketch_mirror`, `sketch_linear_pattern`, `sketch_circular_pattern`, `sketch_transform`, `sketch_use`, `sketch_intersection`, `sketch_text`, `sketch_insert_dxf`, `sketch_insert_image` |
| variable | `variable_list`, `variable_create`, `variable_set`, `variable_delete`, `variable_move` |
| config | `config_list`, `config_add_input`, `config_edit_input`, `config_delete_input`, `config_add_option`, `config_edit_option`, `config_configure`, `config_set_value`, `config_activate`, `config_reorder_inputs` |
| assembly | `assembly_insert`, `assembly_list`, `assembly_entities`, `assembly_connectors`, `assembly_mate`, `assembly_transform`, `assembly_set_state`, `assembly_delete` |
| query | `query_parts`, `query_entities`, `query_datums`, `query_measure`, `query_mass_properties` |
| render | `render_image` |
| export | `export_parts` |
| history | `history_undo`, `history_redo`, `history_list`, `history_create_version`, `history_create_branch`, `history_switch_branch`, `history_merge`, `history_restore` |

Coming next: assembly patterns, groups, exploded views, named positions and BOM, drawings, Part Studio import,
analysis tools, and live mode (attaching to the running app so edits appear in its window).

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

### Workspace protection

`document_protect` turns on Onshape's workspace protection for a branch: edits to the model, tabs, settings,
undo/redo and restore fail with error code `protected`; versions, branches and merges still work.
`document_info` reports `protected`.

## Worked example (MCP tool calls)

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

## For developers

- Operations live in `crates/ops/src/ops/<group>.rs`; each is an `Op` (group, verb, summary, description,
  fields with JSON schemas, handler). Add one there and the CLI and MCP pick it up.
- Handlers call shared functions that the UI also calls (`noshape_ops::studio`, `docedit`, `analysis`,
  `export`, `session`, `noshape_scene::sketch_mode`). Never re-implement UI behaviour in an op: move it out of
  `crates/app` into one of these and call it from both.
- `crates/ops/src/coverage.rs` maps every UI command to its op; `crates/ops/tests/coverage.rs` fails when the
  UI gains a command with no entry, and the planned count may only go down.
