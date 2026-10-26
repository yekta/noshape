//! The MCP server's default tool list: the tools most tasks use, each with a
//! short description and its main fields, so an agent can load the server
//! without spending most of its context on it. Every other tool, and every
//! other field of these, stays one call away: `ops_list` lists them,
//! `ops_describe` gives their full documentation, `ops_call` runs them (and
//! these tools accept all their fields; the compact schemas don't forbid the
//! unlisted ones). `noshape-cli mcp --all-tools` lists everything in full.

/// A tool of the compact list: its short description and the fields shown,
/// each with a short description ("" = the first sentence of the field's own).
pub struct Core {
    pub tool: &'static str,
    pub brief: &'static str,
    pub fields: &'static [(&'static str, &'static str)],
}

const fn c(tool: &'static str, brief: &'static str, fields: &'static [(&'static str, &'static str)]) -> Core {
    Core { tool, brief, fields }
}

pub const CORE: &[Core] = &[
    // Discovering the rest.
    c("ops_list", "Every tool, including the ones not in this list (run those with ops_call): names and groups; with `group` or `search`, summaries and examples.", &[("group", "Only this group: drawing, assembly, config, sketch, analysis, part, view, ui..."), ("search", "Only tools whose name or summary holds these words")]),
    c("ops_describe", "Full description, examples and input schema of tools (any tool; the list here is abridged).", &[("tools", "")]),
    c("ops_call", "Run any tool by name, e.g. one ops_list shows that isn't in this list.", &[("tool", ""), ("args", "")]),
    // Documents and tabs.
    c("document_create", "Create a .noshape document; it becomes current. New documents have \"Part Studio 1\" and \"Assembly 1\". Edits save at once.", &[("path", ""), ("length_unit", "Workspace length unit (default mm).")]),
    c("document_open", "Open a .noshape document and make it current.", &[("path", "")]),
    c("document_info", "The document's tabs (name, kind, id), units and undo state.", &[]),
    c("tab_create", "Add a tab: partstudio, assembly, drawing, variablestudio, featurestudio.", &[("kind", ""), ("name", "")]),
    // Features.
    c("feature_types", "Feature types (id, name, category); `search` words (\"fillet\", \"move face\") or `category` narrow the list.", &[("search", "Words of the name (or the type id)."), ("category", "Sketch, Solid, Detail, Pattern, Transform, Direct, Surface, SheetMetal...")]),
    c(
        "feature_schema",
        "A feature type's parameters: ids, types, defaults, enum options, units, and when the dialog shows each (hidden ones are ignored).",
        &[("type", "")],
    ),
    c(
        "feature_add",
        "Add a feature (accepting its dialog) and regenerate. `params` by parameter id (feature_schema): enum option ids, expressions with units (\"10 mm\", \"#w / 2\"), selections as id lists (region:… face:… edge:… body:… plane:top) or a query {\"query\": {query_entities args}} (e.g. the vertical edges: {\"query\": {\"kind\": \"edges\", \"direction\": [0, 0, 1]}}). A failing feature is still added: check `feature_status` and `regeneration.failed`. E.g. {\"type\": \"extrude\", \"params\": {\"entities\": [\"region:…\"], \"depth\": \"10 mm\"}}; {\"type\": \"fillet\", \"params\": {\"entities\": {\"query\": {\"kind\": \"edges\", \"direction\": [0, 0, 1]}}, \"radius\": \"3 mm\"}}; a feature pattern: {\"type\": \"linearPattern\", \"params\": {\"entities\": [\"feature:…\"], \"directionOne\": [\"edge:…\"], \"distance\": \"20 mm\", \"instanceCount\": 3}}.",
        &[("type", "Feature type id (feature_types)."), ("params", "Parameter values by id."), ("name", ""), ("tab", "")],
    ),
    c("feature_edit", "Change a feature's parameters (only those given) or name, and regenerate.", &[("feature", ""), ("params", "Parameter values by id."), ("name", ""), ("tab", "")]),
    c("feature_get", "A feature's parameters, the ones its dialog shows (`visible`) and its status; custom FeatureScript features also their `notices` (with stack) and `output`.", &[("feature", ""), ("tab", "")]),
    c("feature_list", "Every feature with its status and error, the parts and the rollback bar.", &[("tab", "")]),
    c("feature_delete", "Delete features.", &[("features", ""), ("tab", "")]),
    // Sketches.
    c(
        "sketch_create",
        "New sketch on plane:top / plane:front / plane:right, a planar face id or a Plane feature. Its `frame` gives the sketch axes: Top x=X y=Y, Front x=X y=Z, Right x=Y y=Z; on a face they may be turned (draw with [x, y, z] model points then).",
        &[("plane", ""), ("name", ""), ("tab", "")],
    ),
    c(
        "sketch_draw",
        "Draw with a sketch tool on the given clicks, with the UI's snapping and automatic constraints. A click is [x, y] (sketch) or [x, y, z] (a model point, projected). line [p0, p1, ...] (repeat p0 to close); corner_rectangle [corner, corner]; center_rectangle [center, corner]; circle [center, point on it]; three_point_arc [start, end, point on it]; center_point_arc [center, start, end]; tangent_arc [start at a curve end, end]; point [p, ...]; slot [click on a line/arc, width point]; inscribed_polygon [center, vertex] with `sides`; spline / ellipse ...: ops_describe. `shapes` [{tool, points, sides?, construction?}] draws many in one edit. Reply: created entity ids, `regions` (profile ids for extrude...), solve status, `failed` features.",
        &[("sketch", ""), ("tool", ""), ("points", "Clicks: [x, y] or [x, y, z]."), ("shapes", "Many shapes: [{tool, points, sides?, construction?}]."), ("sides", ""), ("construction", ""), ("tab", "")],
    ),
    c("sketch_constrain", "Add a constraint between sketch entity ids (or `origin`): coincident, horizontal, vertical, tangent, equal, parallel, perpendicular, concentric, midpoint, fix...", &[("sketch", ""), ("constraint", ""), ("entities", ""), ("tab", "")]),
    c(
        "sketch_dimension",
        "Add a driving dimension to one or two entities (sketch entities, `origin`, model edges / vertices); returns its `dim:` id. Kinds: auto (default), distance, horizontal_distance, vertical_distance, length, radius, diameter, angle...",
        &[("sketch", ""), ("entities", ""), ("value", "Value or expression (\"20 mm\", \"#w\")."), ("kind", ""), ("label", "Text position (sketch coordinates); also picks horizontal / vertical for auto."), ("tab", "")],
    ),
    c("sketch_set_dimension", "Change a sketch dimension's value or expression; the model regenerates (`failed` lists features it broke).", &[("sketch", ""), ("dimension", ""), ("value", ""), ("tab", "")]),
    c("sketch_get", "A sketch's entities, constraints and dimensions (`dim:` ids, expressions), solve state, regions and frame.", &[("sketch", ""), ("tab", "")]),
    c("variable_create", "Create a #variable usable in any expression (\"#width / 2\").", &[("name", ""), ("value", ""), ("kind", ""), ("tab", "")]),
    // Queries and output.
    c("query_parts", "The parts: id, name, volume, surface area, bounding box, face / edge counts.", &[("tab", ""), ("configuration", "")]),
    c(
        "query_entities",
        "Faces / edges / vertices with ids and geometry (centroid, normal, axis, radius, endpoints, direction...). Filters: surface, curve, normal (faces facing it), direction (edges along it), created_by, part, near (closest first); `at` [[x, y, z], ...] returns the entity at each point, in order (several picks in one call).",
        &[("kind", ""), ("at", "Points: the entity at each."), ("surface", ""), ("curve", ""), ("normal", ""), ("direction", ""), ("created_by", ""), ("part", ""), ("near", ""), ("limit", ""), ("tab", "")],
    ),
    c("query_measure", "Measure entities: lengths, areas, radii; distance and angle between two.", &[("entities", ""), ("tab", "")]),
    c("query_mass_properties", "Volume, surface area, mass, center of mass, inertia (mass needs a material on every part).", &[("parts", ""), ("tab", "")]),
    c("part_rename", "Rename a part; part arguments take names as well as `body:` ids.", &[("part", ""), ("name", ""), ("tab", "")]),
    c("part_set_material", "Assign a library material (\"Aluminum - 6061\", material_list) or a custom density.", &[("parts", ""), ("material", ""), ("density", "Custom material density in kg/m³."), ("tab", "")]),
    c("render_image", "A PNG of a Part Studio or Assembly, returned as an image (`file` also writes it). Sections, display modes, zebra, isolate...: ops_describe.", &[("view", ""), ("tab", ""), ("file", ""), ("width", ""), ("height", ""), ("display_mode", "")]),
    c("export_parts", "Write a Part Studio's parts or an Assembly to STEP, PARASOLID, IGES, ACIS, STL, OBJ, 3MF, GLTF, GLB, JT, PVZ (format options: ops_describe).", &[("format", ""), ("file", ""), ("parts", ""), ("units", "File units (STL, OBJ...)."), ("tab", "")]),
    c("import_files", "Import CAD or mesh files (STEP, IGES, Parasolid, SolidWorks..., STL, OBJ, 3MF, glTF): each becomes a Part Studio named after it (a STEP assembly: an Assembly and a \"<file> parts\" Part Studio). Reply: tabs and parts.", &[("files", ""), ("combine", "")]),
    c("history_undo", "Undo the last edit (of `tab`); the reply names it.", &[("tab", "")]),
    c("history_redo", "Redo the last undone edit.", &[("tab", "")]),
    // Assemblies.
    c("assembly_insert", "Insert a part (id or name) of a Part Studio, a whole Part Studio, or an Assembly; returns the instance id (names are \"<part> <1>\"...).", &[("source", ""), ("part", ""), ("tab", "")]),
    c("assembly_list", "Instances with placements, mates, degrees of freedom.", &[("tab", "")]),
    c("assembly_entities", "An instance's faces / edges / vertices in assembly coordinates, with ids for mates; filters as query_entities (surface, curve, normal, direction, near, at).", &[("instance", ""), ("kind", ""), ("at", "Points: the entity at each."), ("surface", ""), ("curve", ""), ("normal", ""), ("direction", ""), ("near", ""), ("tab", "")]),
    c(
        "assembly_mate",
        "Add or edit a mate. A connector: {\"instance\", \"entity\": a face / edge / vertex id from assembly_entities, \"inference\"?: center, centroid, ...} or {\"origin\": true}; `a`'s instance moves. An offset runs along the mate's Z, which follows `a` (flipped as the dialog does): check the moved instance's translation. Edit: `mate` plus the fields to change.",
        &[("type", ""), ("a", ""), ("b", ""), ("offset", "{\"x\", \"y\", \"z\", \"rotation\"} expressions."), ("mate", ""), ("flip_primary", ""), ("limits", ""), ("tab", "")],
    ),
    c("assembly_set_state", "Fix / unfix, hide / show, suppress / unsuppress instances.", &[("instances", ""), ("fixed", ""), ("hidden", ""), ("suppressed", ""), ("tab", "")]),
    c("assembly_transform", "Move an instance to a placement: translation [x, y, z], optional axis and angle.", &[("instance", ""), ("translation", ""), ("axis", ""), ("angle", ""), ("tab", "")]),
    c("assembly_interference", "Overlapping instances, with volumes.", &[("instances", ""), ("tab", "")]),
    // Drawings.
    c("drawing_create", "A Drawing tab of a Part Studio, part or assembly: `template` (\"ISO A3\", drawing_templates), `suggested_views` true for front / top / right and isometric views.", &[("source", ""), ("template", ""), ("suggested_views", ""), ("part", ""), ("name", "")]),
    c("drawing_get", "A sheet's views (id, kind, orientation, scale, bounds) with their drawn edges in sheet millimeters (lines, circles, arcs), annotations and tables.", &[("drawing", ""), ("edges", ""), ("sheet", "")]),
    c(
        "drawing_dimension",
        "The Dimension tools: click `picks` on drawn edges, vertices or circle centers (sheet mm, from drawing_get); the text goes at `place`. kind smart (default: a line's length, a circle's Ø, an arc's R, or two entities), point_to_point, line_to_line, radius, diameter, angle...",
        &[("drawing", ""), ("picks", ""), ("place", ""), ("kind", ""), ("sheet", "")],
    ),
    c("drawing_export", "Export a drawing: PDF (every sheet), DWG, DXF, SVG, PNG, JPEG, TIFF (format options: ops_describe).", &[("drawing", ""), ("format", ""), ("file", "")]),
    // FeatureScript.
    c("featurestudio_edit", "Replace a Feature Studio's FeatureScript source (a draft until committed); returns compile notices.", &[("source", ""), ("tab", "Feature Studio tab; default the first.")]),
    c("featurestudio_commit", "Commit Feature Studios so Part Studios use their custom features; returns the feature types (`custom:…` ids for feature_add).", &[("tab", "Feature Studio tab; default the first."), ("all", "")]),
    c("featurestudio_monitor", "A Part Studio's FeatureScript Monitor: compile warnings, each custom feature's runtime notices (message, stack line:col and module (function)) and println output.", &[("part_studio", "")]),
];
