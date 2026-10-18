# Custom features (Feature Studios)

A **Feature Studio** tab holds script code that defines custom features for
the document, like Onshape's FeatureScript. Custom features appear in every
Part Studio of the document. They show up in the toolbar's last group
(**Custom features**) and in tool search (Alt+C). They open the generic
feature dialog with live preview, and they regenerate like built-in features.

Scripts are written in [Rhai](https://rhai.rs), a small, sandboxed
language. Its syntax is close to Rust and JavaScript. Insert a Feature Studio
from the tab bar **+** menu. It starts with a working example.

## How it works

* When a Feature Studio is compiled, its top level runs once. Each
  `feature(name, #{ ... })` call there declares one feature.
* A declared feature gets the type id `custom:<feature studio tab id>:<name>`.
  Its parameter schema is built from the declaration. The Part Studio dialog
  is generated from that schema.
* When a Part Studio regenerates, the feature's evaluate function is called
  as `evaluate(ctx, p)`. `p` holds the parameter values. `ctx` runs
  geometry operations on the model as it stands before the feature.
* Edits in the Feature Studio editor are saved to the document after a short
  pause (or right away with **Commit**). Part Studios that use the changed
  features then regenerate. Undo and redo work as for any other edit.
* Merging versions treats a Feature Studio as a whole. If only one side
  changed it, that side's source wins.

## Declaring a feature

```rhai
feature("Bolt circle", #{
    icon: "circular-pattern",            // optional: name of an app icon
    description: "Holes on a circle",    // optional
    params: [
        query("face", "Face", PLANAR_FACE, 1),
        int("count", "Holes", 4, 1, 36),
        length("pcd", "Bolt circle diameter", mm(28)),
        bool("through", "Through all", false),
        length("depth", "Depth", mm(6)).visible_when("through", false),
    ],
    evaluate: "bolt_circle",              // optional, see below
});

fn bolt_circle(ctx, p) { ... }
```

`evaluate` can be a function name (a string), a function pointer
(`Fn("name")`), or an anonymous function `|ctx, p| { ... }`. If you leave it
out, the function used is the feature name in snake case (`bolt_circle` for
"Bolt circle"). If that function doesn't exist, a function named `evaluate`
is used.

Feature names must be unique within a studio and must not contain `:`.
Parameter ids may contain letters, digits and `_`. Renaming a feature or a
parameter breaks Part Studio features that use the old name, as in Onshape.

### Parameters

All numbers are in SI units: lengths in **meters**, angles in **radians**.
Use the unit functions to write literal values.

| constructor | value in `p` |
|---|---|
| `length(id, label, default)` / `length(id, label, default, min, max)` | float, meters. Default bounds: 0 to 500 m. |
| `angle(id, label, default)` / `angle(id, label, default, min, max)` | float, radians. Default bounds: ±360°. |
| `real(id, label, default)` / `real(id, label, default, min, max)` | float |
| `int(id, label, default)` / `int(id, label, default, min, max)` | integer |
| `bool(id, label, default)` | `true` / `false` |
| `enum_(id, label, options)` / `enum_(id, label, options, default)` | the option id (string). `options` is `["A", "B"]` or `[["a", "Label A"], ...]`. |
| `text(id, label, default)` | string |
| `query(id, label, filter)` / `query(id, label, filter, max_picks)` | array of entities |

Numeric fields in the dialog accept expressions with units and variables
(`25 mm`, `#width / 2`). Values outside `min..max` make the feature fail with
a message.

Query filters (use them as constants at the top level, as strings, or
combined like `"FACE|EDGE"`): `FACE`, `PLANAR_FACE`, `CYLINDRICAL_FACE`,
`EDGE`, `LINEAR_EDGE`, `CIRCULAR_EDGE`, `VERTEX`, `POINT`, `BODY`, `SOLID`,
`SURFACE`, `REGION` (sketch regions), `PLANE`, `AXIS`, `DIRECTION`,
`MATE_CONNECTOR`, `SKETCH`.

Parameter modifiers:

* `.visible_when(other_id, value)` shows the parameter only while a bool
  parameter is `true`/`false`, or while an enum parameter has a given option.
* `.describe("tooltip text")` adds a description.

## Helpers

* Units to SI: `mm(x)`, `cm(x)`, `m(x)` / `meter(x)`, `inch(x)`, `ft(x)`, `deg(x)`, `rad(x)`.
  From SI: `to_mm(x)`, `to_inch(x)`, `to_deg(x)`.
* Vectors are arrays `[x, y, z]`. Helpers: `vec(x, y, z)`, `vadd(a, b)`,
  `vsub(a, b)`, `vscale(a, s)`, `vdot(a, b)`, `vcross(a, b)`, `vlen(a)`,
  `vnorm(a)`. Rhai's math functions (`sin`, `cos`, `abs`, `sqrt`, `PI()`...)
  are available.
* Errors: `throw "message"` fails the feature with that message.
  `ctx.warn("message")` shows a warning but keeps the result.
  `print(x)` writes to the application log.
* Top-level `let` variables are not visible inside functions (a Rhai rule).
  Use functions or `const` values with `global::NAME`.

## Entities

Entities (bodies, faces, edges, vertices, sketch regions, planes...) are
persistent references. They stay valid through later operations of the same
script run, and downstream features can reference them. `e.kind` returns
`"body"`, `"face"`, `"edge"`, `"vertex"`, `"region"`, `"plane"`, and so on.
Wherever an operation takes entities, it accepts one entity, an array, or
nested arrays. Arrays concatenate with `+`.

## Operations (`ctx` methods)

Operations that create or change bodies return the array of bodies they
created or modified.

### Solids without sketches

| method | |
|---|---|
| `ctx.box(corner, size [, opts])` | Axis-aligned box. Negative sizes extend the other way. |
| `ctx.cylinder(base_center, axis, radius, height [, opts])` | |
| `ctx.cone(base_center, axis, r1, r2, height [, opts])` | |
| `ctx.sphere(center, radius [, opts])` | |
| `ctx.torus(center, axis, major_radius, minor_radius [, opts])` | |

### From profiles

| method | |
|---|---|
| `ctx.extrude(profiles, depth [, opts])` | Sketch regions or planar faces. A negative depth extrudes the other way. Options: `direction: [x, y, z]`, `symmetric: true`. |
| `ctx.revolve(profiles, axis, angle [, opts])` | `axis` is an entity (linear edge, cylindrical face, sketch line, mate connector) or `#{ origin: [..], direction: [..] }`. |

The creation options are `op` and `scope`.

* `op: "new"` (the default) creates new parts. `"add"` fuses the result
  with the parts it touches (or the `scope` parts); with nothing to fuse,
  it creates a new part. `"remove"` and `"intersect"` work the same way
  (aliases: `"union"`, `"subtract"`).
* `scope: [bodies]` sets the parts the result merges with.
* `id: "name"` gives the operation a stable name (see below).

### Booleans, detail, transforms

| method | |
|---|---|
| `ctx.union(targets, tools [, #{ keep_tools: false }])` | Fuses everything into the first target. |
| `ctx.subtract(targets, tools [, opts])` | Tools are consumed unless `keep_tools: true`. |
| `ctx.intersect(targets, tools [, opts])` | |
| `ctx.fillet(edges, radius)` | A face stands for all of its edges. |
| `ctx.chamfer(edges, distance)` | Equal-distance chamfer. |
| `ctx.translate(bodies, vector)` | Moves bodies in place. |
| `ctx.rotate(bodies, origin, axis, angle)` | Rotates bodies in place. |
| `ctx.copy(bodies, vector)` | A translated copy. |
| `ctx.linear_pattern(bodies, direction, spacing, count [, opts])` | Returns the `count - 1` new copies. |
| `ctx.circular_pattern(bodies, origin, axis, count, angle [, opts])` | With `deg(360)`, copies are spread evenly. With any other angle, `angle` spans the first to the last instance. |
| `ctx.mirror(bodies, plane)` | A mirrored copy. `plane` is an entity or `#{ origin, normal }`. |
| `ctx.delete(bodies)` | |

### Resolving and measuring

| method | returns |
|---|---|
| `ctx.plane(entity)` | `#{ origin, normal, x, y }` (planar face, plane, mate connector, sketch) |
| `ctx.point(entity)` | `[x, y, z]`: vertex, sketch point or mate connector origin; the centroid of a face or body |
| `ctx.axis(entity)` | `#{ origin, direction }` |
| `ctx.direction(entity)` | `[x, y, z]` |
| `ctx.normal(face)` | the outward normal (at the face's center for curved faces) |
| `ctx.area(faces_or_bodies)` | total area, m² |
| `ctx.length(edges)` | total length, m |
| `ctx.volume(bodies)` | total volume, m³ |
| `ctx.bbox(entities)` | `#{ min, max, size, center }` |

### Queries over the current model

| method | returns |
|---|---|
| `ctx.bodies()` / `ctx.parts()` | All solid and surface bodies / only solids. This includes bodies created earlier in the script. |
| `ctx.faces(x)` | faces of bodies; faces next to edges |
| `ctx.edges(x)` | edges of bodies or faces |
| `ctx.vertices(x)` | vertices of bodies, faces or edges |
| `ctx.owner(entity)` | the body an entity belongs to |
| `ctx.geometry(entity)` | `"PLANE"`, `"CYLINDER"`, `"CONE"`, `"SPHERE"`, `"TORUS"`, `"SPLINE"` (faces); `"LINE"`, `"CIRCLE"`, `"ELLIPSE"` (edges); `"SOLID"`, `"SURFACE"` (bodies)... |
| `ctx.filter(entities, "CIRCLE")` | Entities of a geometry type or kind (`"face"`, `"edge"`...). Combine types with `"LINE\|CIRCLE"`. |

## Persistent naming

Script results go through the same naming machinery as built-in features
(`name_result`, `apply_operation`). Downstream features (fillets,
sketches on faces, mates) can therefore reference script-created geometry,
and those references survive parameter changes. The roles are keyed by
an *operation key*:

* Primitive faces are named by their position on the primitive. Planar faces
  are named by their outward normal along the primitive's axes (box: -X, +X,
  -Y, +Y, -Z, +Z). Curved faces are named by surface type.
* Extrude and revolve faces are named after the profile edge they sweep, and
  caps after the profile.
* Fillet and chamfer faces are named after the edge they replace. Boolean
  results keep the names of the faces they came from. Pattern copies are
  named after the instance and the original face.

By default, the operation key is the order of the operation within the run
(1, 2, 3...). Names stay stable as long as the script performs the same
sequence of operations. If earlier operations depend on parameters (a loop
over a count, an `if`), pass `id: "name"` in the options of the operations
that downstream features will reference. That key does not depend on order.

## Sandbox and errors

* No file system, network or module access (`import` is disabled), and no
  `eval`.
* Limits: 50 million operations per run, call depth 48, a time limit of
  about 20 s, and bounded string, array and map sizes. An infinite loop fails
  the feature with "Script exceeded the operation limit".
* Compile errors (syntax, invalid declarations) are listed under
  **Problems** below the editor, with line numbers. Click one to jump to it.
  Every Part Studio feature from a studio with errors fails with "Feature
  Studio has errors: Line N: ...". Those features stay editable: their
  dialogs use the last declarations that compiled.
* Runtime errors (a `throw`, a failed kernel operation, a wrong argument
  type) become the feature's error message, shown in red in the feature list
  and in the dialog. Downstream features evaluate against the model before
  the failed feature, as with any failed feature.
* If the Feature Studio, or the declared feature, no longer exists, the
  feature fails with a message and can't be edited until it is defined again.

## Example

The demo document (`noshape --demo`) contains a "Custom features" Feature
Studio and a "Flange" Part Studio that uses both of its features:

```rhai
feature("Flange", #{
    icon: "revolve",
    params: [
        length("diameter", "Diameter", mm(80)),
        length("thickness", "Thickness", mm(8)),
        int("bolts", "Bolt holes", 6, 0, 24),
        bool("round", "Round outer edges", true),
        length("radius", "Edge radius", mm(1.5)).visible_when("round", true),
        // ...
    ],
});

fn flange(ctx, p) {
    let disk = ctx.cylinder([0, 0, 0], [0, 0, 1], p.diameter / 2.0, p.thickness, #{ id: "disk" });
    ctx.cylinder([0, 0, p.thickness], [0, 0, 1], mm(18), mm(14), #{ id: "hub", op: "add" });
    let hole = ctx.cylinder([mm(29), 0, -mm(1)], [0, 0, 1], mm(4), p.thickness + mm(2), #{ id: "bolt" });
    let others = ctx.circular_pattern(hole, [0, 0, 0], [0, 0, 1], p.bolts, deg(360), #{ id: "bolts" });
    ctx.subtract(disk, hole + others);
    if p.round {
        let rim = [];
        for e in ctx.filter(ctx.edges(disk), "CIRCLE") {
            if abs(ctx.bbox(e).size[0] - p.diameter) < 1e-6 { rim.push(e); }
        }
        ctx.fillet(rim, p.radius);
    }
}
```

## Implementation notes

* Model: `crates/model/src/custom/`. `compile.rs` holds the sandboxed
  engine, the top-level run and the validation. `api.rs` holds the script
  functions. `eval.rs` is the evaluator. `schema.rs` builds and interns the
  run-time schemas. `provider.rs` holds `CustomFeatureProvider` and
  `DocumentScripts`.
* The feature registry (`features::get`) falls back to the active custom
  schemas. Strings and schemas are interned (leaked once per distinct
  declaration), so recompiling unchanged declarations allocates nothing.
* Regeneration gets compiled studios through
  `RegenOptions::custom_features` (like `derived`). Regenerations without a
  provider (assemblies, drawings) use the most recently compiled studios.
  The cache key of a custom feature includes its studio's source hash.
* App: `crates/app/src/feature_studio.rs` holds the tab view (code editor
  with Rhai highlighting, Problems list, Features list). The Part Studio
  toolbar and tool search list `DocModel::custom_features()`.
