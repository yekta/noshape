noshape is a local parametric CAD app that works like Onshape. These tools do what a person does in the app; every edit is saved at once as an undoable history entry.

Workflow for a part:
1. `document_create` (path to a new .noshape file) or `document_open`. Later calls use that document unless you pass `doc`.
2. `sketch_create` on `plane:top` / `plane:front` / `plane:right` or a planar face id. Coordinates are sketch-plane (x, y) in workspace units (mm by default). On Top x=X, y=Y; on Front x=X, y=Z; on Right x=Y, y=Z.
3. `sketch_draw` with a tool and its clicks (corner_rectangle [[x1,y1],[x2,y2]], circle [[cx,cy],[rim x, rim y]], line [p0,p1,p2,...] — repeat p0 to close). It adds the same automatic constraints as the app. Then `sketch_dimension` (value "20 mm" or an expression "#width / 2") and `sketch_constrain`.
4. The `regions` returned by the sketch tools are profile ids for `feature_add` type `extrude` (params: entities = [region ids], depth = "10 mm", operationType NEW/ADD/REMOVE/INTERSECT, endBound BLIND/THROUGH_ALL/...). Call `feature_schema` for any feature's parameters, `feature_types` for the list (revolve, sweep, loft, fillet, chamfer, shell, hole, patterns, mirror, booleans, sheet metal...).
5. Select model geometry with `query_entities` (faces/edges/vertices, filter by surface/curve type, normal, feature, nearest point); pass their ids to selection parameters (e.g. fillet `entities`).
6. Check: each edit reply has `feature_status` and `regeneration.failed` (feature errors with Onshape's message). `query_parts` (volume, bounding box), `query_mass_properties`, `query_measure`, `render_image` (a PNG to look at).
7. `export_parts` writes STEP, STL, Parasolid, IGES, OBJ, 3MF, glTF...; `history_undo` undoes the last edit.

Values: expressions with units ("25 mm", "1 in", "30 deg", "2 * #t + 1 mm"); a bare number takes the workspace unit. Errors say which parameter is wrong and why.
