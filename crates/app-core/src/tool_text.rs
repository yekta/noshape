//! Onshape's tool descriptions, word for word: the expanded toolbar tooltip
//! and the tool search's description pane. Copied from live (research
//! dom/ui-e-tool-descriptions-live.json: the toolbar endpoint's
//! expandedTooltip strings through tooltips-en_US.json), keyed by the tool's
//! title as the search lists it. Drawings have none (their toolbar tooltips
//! are titles only). A few hold HTML lists (`<br><ol><li>`, taken from the
//! endpoint's strings where the capture has the rendered text): the tooltip
//! and the search pane show them as numbered lists, as live does
//! ([`blocks`]).

/// Part Studio tools (features, sheet metal, frames, PCB), by title.
pub const PART_STUDIO: &[(&str, &str)] = &[
    ("3D fit spline", "Create a curve through a series of vertices or tangentially continuous edges. Specify direction, magnitude, and curvature conditions for the spline's start and end points."),
    ("Bend", "Bend a sheet metal model along a reference line, with additional bend control options."),
    ("Bend relief", "Modify an individual sheet metal bend relief. Specify bend relief type and scale."),
    ("Body draft", "Apply draft to a part by selecting edges, faces, or the part itself."),
    ("Boolean", "Perform a union, subtraction, or intersection on two or more parts or surfaces."),
    ("Boundary surface", "Create or add a surface specified by its boundary profiles."),
    ("Bridging curve", "Create a curve connecting any 2 points or vertices. Specify the continuity condition for each selection."),
    ("Chamfer", "Create beveled edges or faces. Specify measurement type (offset or tangent) and chamfer type (equal, two distances or distance/angle). Individual edges can be overridden to change the direction."),
    ("Circular pattern", "Create, add to, subtract from, or intersect multiple instances of parts, faces, or features. Specify the axis, angle, and number of instances. Optionally, specify instances to skip."),
    ("Composite curve", "Create a single curve from multiple sketch entities, adjacent edges, and other curves. Each selection must meet at their vertices."),
    ("Composite part", "Create a composite part from selected entities (parts, surfaces, curves, points, and meshes). Specify whether the composite part is open or closed."),
    ("Constrained surface", "Create a surface from a selection of points or mesh data within a specified tolerance. Optimization controls for performance or smoothness and deviation display controls."),
    ("Corner", "Modify an individual sheet metal corner by selecting an edge, vertex, or face of the existing corner. Specify corner relief type and scale."),
    ("Corner break", "Break the corner on existing sheet metal parts by applying a fillet or chamfer. Select a corner edge or vertex and specify corner break type and size. Recommended use after all flanges and joints of sheet metal model have been finalized."),
    ("Curve pattern", "Create, add to, subtract from, or intersect multiple instances of parts, faces, or features. Specify the path, spacing type, and number of instances. Optionally, specify instances to skip."),
    ("Cut list", "Create an open composite part from the selected segments with a corresponding table listing each segment's properties:<br><ol><li>Select all frames in the Part Studio or select each segment individually.</li><li>Override any segment's characteristic properties.</li></ol>"),
    ("Decal", "Apply an image onto an existing flat or cylindrical face of a part or surface. Specify the exact location, size and alignment of the applied image."),
    ("Delete face", "Remove one or more faces from a part. Specify whether to heal the surrounding faces, cap the void, or leave the void open. Useful for imported parts."),
    ("Delete part", "Delete one or more parts or surfaces. For composite parts, specify whether to delete, dissolve, or ignore."),
    ("Derived", "Using an associative link, insert parts, sketches, surfaces, curves, planes, or mate connectors from one Part Studio into another. Multiple locations can be selected for positioning to selected placement. Changes made to the parent entity are reflected in the derived entity."),
    ("Draft", "Add a draft angle to one or more faces by selecting a neutral plane or parting line."),
    ("Edit curve", "Edit an existing curve or create a new curve by selecting sketch entities, edges, and/or curves to apply a simplified approximation, elevate the degree, reposition control curve vertices, and/or planarize into any 2D plane."),
    ("Enclose", "Create, add to, subtract from, or intersect parts by selecting faces, surfaces and parts that enclose a region."),
    ("End cap", "Create one or more endcaps for frame segments.<br><ol><li>Select frame segment end faces to be capped.</li><li>Select endcap profile type.</li><li>Specify the endcap thickness.</li><li>Adjust the offset to make the endcap profile larger or smaller than the frame segment profile.</li><li>For rectangle types, specify the corner treatment.</li><li>For internal endcaps (endcaps inside the frame segment), specify the offset from the frame segment face.</li></ol>"),
    ("External thread", "Add an external thread to cylindrical parts by selecting a leading edge or edges. Specify the standard, size, pitch, length, end type, and optionally add a chamfer or undercut."),
    ("Extrude", "Create, add to, subtract from, or intersect parts (by extruding sketch regions or planar faces), surfaces (by extruding lines or curves), or thin parts (by extruding sketch regions, planar faces, lines, or curves)."),
    ("Face blend", "Blend 2 or more faces. Specify propagation, cross section type, measurement type, symmetry, control type, and trim type."),
    ("Fill", "Create or add to a surface by selecting a set of curves or edges to form a closed boundary. Specify the continuity condition for each selection."),
    ("Fillet", "Create constant, variable, partial, or asymmetric edge fillets, or create full round fillets. Specify measurement type (radius or width) and control type (distance, conic, or curvature)."),
    ("Finish flex PCB model", "Deactivate and optionally unfold the PCB model to continue editing with non-PCB features."),
    ("Finish sheet metal model", "Deactivate the sheet metal models to continue editing with non-sheet metal features. Features created on deactivated sheet metal parts are not represented in the sheet metal table or flat pattern."),
    ("Flange", "Create a wall, connected by a bend, for each selected edge of an existing sheet metal part. Specify flange alignment, end type, and angle control type."),
    ("Flex PCB model", "Create flexible PCB parts by converting existing parts, extruding sketches, or thickening faces or sketches. Operations on PCB models are automatically represented as a flat pattern, and joints and bends are listed in a table."),
    ("Form", "Create form features on existing sheet metal parts. Form tools are selected from a predefined library."),
    ("Frame", "Create a frame:<br><ol><li>Select the sweep profile.</li><li>Select the faces, edges, or vertices that define one or more sweep paths.</li><li>Adjust the profile using mirror, offset, and angle controls.</li><li>Select the joint type between consecutive segments in a frame and specify any per-joint overrides.</li><li>Specify faces, planes, and bodies with which to trim or extend the start and end segments.</li></ol>"),
    ("Frame trim", "Trim frame segments as ordered groups:<br><ol><li>Specify two or more groups. Segments in earlier groups are boolean subtracted from segments in all subsequent groups.</li></ol>Trim segments against faces:<br><ol><li>Select segments to be trimmed.</li><li>Select a face to use as split tool. After the split, the largest remnant is kept.</li></ol>"),
    ("Gusset", "Create gussets between adjacent frame segments:<br><ol><li>Select edges where the gussets should be created. The centerpoint of the edge is the centerpoint of the gusset.</li><li>Select the gusset profile, size, thickness, and offset from center.</li><li>Specify a chamfer size for the outer corner of rectangular gussets.</li></ol>"),
    ("Helix", "Create a helix using a conical or cylindrical face, helix axis, or circular edge. Specify helix type, start and end conditions, number of revolutions, and helical pitch."),
    ("Hem", "Create hems on existing sheet metal parts. Specify hem type, base alignment, and corner type."),
    ("Hole", "Create simple, countersink, or counterbore holes in parts by selecting sketch points or mate connectors. Specify hole size, termination type, and standard, as well as precision and tolerances for most parameters."),
    ("Intersection curve", "Create a curve at the intersection of 2 or more surfaces or faces. The selections must intersect."),
    ("Isocline", "Given input faces, angle and reference direction, creates isocline curves or split lines along isoclines."),
    ("Isoparametric curve", "Create U or V curves on an existing surface. Specify a number of evenly spaced curves or create a set of curves using a location value or selected point."),
    ("Jog", "Offset a sheet metal model at a reference line with two bends in opposite directions with additional jog controls."),
    ("Linear pattern", "Create, add to, subtract from, or intersect multiple instances of parts, faces, or features. Specify the direction, distance, and number of instances. Optionally, specify instances to skip."),
    ("Loft", "Create, add to, subtract from, or intersect parts, surfaces, or thin parts by placing a smooth face between ordered profiles."),
    ("Make joint", "Create a bend or rip joint on adjacent walls. Specify bend radius or rip style."),
    ("Mate connector", "Create a mate connector on or between parts or surfaces. Mate connectors are used to locate and orient entities within an assembly mate."),
    ("Mirror", "Create, add to, subtract from, or intersect a mirror copy of parts, faces, or features. Mirror about a plane or planar face."),
    ("Modify fillet", "Remove existing fillets and rounds, or change their radius. Useful for imported parts."),
    ("Move boundary", "Extend or trim the boundary edges of a surface."),
    ("Move face", "Translate, rotate, or offset selected faces. Specify end type and distance, or axis and rotation angle. Useful for imported parts."),
    ("Mutual trim", "Trim two adjacent contiguous sets of surfaces by extending intersections to complete the trim:<br><ol><li>Select the first set of contiguous surfaces.</li><li>Select the second set of contiguous surfaces.</li><li>Specify if generated surfaces should be merged into a single surface.</li></ol>"),
    ("Offset curve", "Create and extend and/or split a new curve by offsetting edges on surrounding faces."),
    ("Offset surface", "Create a new surface by offsetting an existing face, surface, or sketch region. To create a copy, set the offset distance to 0."),
    ("Plane", "Create a new construction plane by referencing existing geometry."),
    ("Projected curve", "Create a curve at the projection of an existing curve onto faces or 2 projected sketches by selecting 2 non-parallel sketches. The projections from 1 sketch must intersect the other."),
    ("Query variable", "Create a query variable in this Part Studio for use in other features."),
    ("Replace face", "Trim a face or extend a face to a new surface. Select faces to replace and the surface to use for the replacement. Useful for imported parts."),
    ("Revolve", "Create, add to, subtract from, or intersect parts (by revolving sketch regions or planar faces about a central axis), surfaces (by revolving lines and curves about a central axis), or thin parts (by revolving sketch regions, planar faces, lines, or curves about a central axis)."),
    ("Rib", "Create ribs from sketch curves on selected parts. Specify thickness and indicate whether to extend normal or parallel to the sketch plane."),
    ("Routing curve", "Create an interpolated spline or polyline with optional radii through points defined by dragging a manipulator, selecting mate connectors or vertices, sampling edges, or by importing a CSV file. Adjustments are made to the points or the segments created between them in order to create a continuous path."),
    ("Ruled surface", "Create or add a ruled surface from an existing edge, sketch line, or curve. Specify the ruled surface type, angle, distance, and corner type."),
    ("Sheet metal loft", "Create or add to existing sheet metal parts by selecting two profiles to loft. Adjust chordal tolerance to change loft resolution. Rips are automatically added to closed profiles. Operations on sheet metal models are automatically represented as a flat pattern, and joints and bends are listed in a table."),
    ("Sheet metal model", "Create sheet metal parts by converting existing parts, extruding sketches, or thickening faces or sketches. Operations on sheet metal models are automatically represented as a flat pattern, and joints and bends are listed in a table."),
    ("Shell", "Create a thin-walled part by removing one or more faces. Specify the wall thickness for the remaining faces."),
    ("Sketch", "A sketch is the basis for all parts. Select a plane, planar face, or mate connector on which to create a new sketch."),
    ("Split", "Select entities to split. For parts and surfaces, specify whether to keep or remove the split tool. For faces, select a sketch curve, curve, surface, or plane to use as the split tool."),
    ("Sweep", "Create, add to, subtract from, or intersect parts and surfaces (by sweeping sketch regions, planar faces, lines or curves along a path), or thin parts (by sweeping sketch regions, planar faces, lines, or curves along a path). Add optional profile control (by orientation, lock faces or lock direction), twist (by turns, angle or pitch), and scale."),
    ("Tab", "Add a tab to a sheet metal flange, remove interfering material, or bridge 2 flanges from the same sheet metal model."),
    ("Tag", "Tag entities for use in Frames, Sheet metal forms, or PCB Studio holes. Select geometry, define orientation, add properties."),
    ("Thicken", "Create, add to, subtract from, or intersect parts by thickening surfaces or faces of solids."),
    ("Transform", "Move, rotate, copy, or scale a part. Transform by mate connectors or translate by line, distance, or XYZ."),
    ("Trim curve", "Trim or extend existing curves by a distance or to a bounding entity."),
    ("Variable", "Create a variable in this Part Studio for use in dimensions and expressions. In any numeric field, use # and the variable name."),
    ("Wrap", "Wrap faces or sketch entities around a cylindrical or conical part or surface. Create, add to, subtract from, or intersect solids. Create surfaces or split the target face. Adjust the position of the result by specifying anchor points, angle, or shift."),
];

/// Sketch tools, by title.
pub const SKETCH: &[(&str, &str)] = &[
    ("3 point arc", "Sketch an arc by defining 2 end points and a radius point."),
    ("3 point circle", "Sketch a circle by defining 3 points along its circumference."),
    ("Aligned rectangle", "Sketch a rectangle, starting with a line and ending with the perpendicular distance of the aligned side."),
    ("Bezier", "Sketch a multi-point polygon to control a Bezier curve. Drag any point along its control polygon to make adjustments."),
    ("Center point arc", "Sketch an arc by defining its center, start, and end points."),
    ("Center point circle", "Sketch a circle, starting with its center point and ending with a point along its circumference."),
    ("Center point rectangle", "Sketch a rectangle, starting with its center point and ending with a corner point."),
    ("Chamfer", "Create distance/distance chamfer by selecting 2 sketch entities or a point that connects 2 sketch entities."),
    ("Circular pattern", "Create multiple instances of sketch entities uniformly about an axis. Specify the angle between instances and number of instances."),
    ("Circumscribed polygon", "Sketch a polygon defined on a circumscribed circle."),
    ("Coincident", "Make 2 or more sketch entities share the same location in a sketch."),
    ("Concentric", "Make any point coincident with the center of an arc or circle. Make selected arcs and circles share a center point."),
    ("Conic", "Sketch an ellipse, parabola, or hyperbola by defining its start, end, and control points. Specify its rho value."),
    ("Construction", "Sketch new construction geometry or convert existing sketch entities into construction geometry."),
    ("Corner rectangle", "Sketch a rectangle, starting with a corner point and ending with the diagonal corner point."),
    ("Curvature", "Create curvature continuous transitions between sketch splines or conics and surrounding geometry."),
    ("Dimension", "Add dimensions to and between sketch entities."),
    ("Ellipse", "Sketch an ellipse by defining its center point, major axis, and minor axis."),
    ("Elliptical arc", "Sketch an elliptical arc by defining its center point, major axis, minor axis, start and end points."),
    ("Equal", "Make multiple lines equal in length, or multiple arcs equal in radii."),
    ("Extend", "Extend a line or curve to the first intersecting point or bounding geometry. If no intersection or bounding geometry is found, the line or curve ends at the release point."),
    ("Extrude", "Create, add to, subtract from, or intersect parts (by extruding sketch regions or planar faces), surfaces (by extruding lines or curves), or thin parts (by extruding sketch regions, planar faces, lines, or curves)."),
    ("Fillet", "Create fillets or rounds by selecting 2 sketch entities, or a point that connects 2 sketch entities. Specify the radius."),
    ("Fix", "Lock a sketch entity onto the sketch plane so it does not move."),
    ("Horizontal", "Constrain 1 or more lines, or a set of points, to be horizontal."),
    ("Inscribed polygon", "Sketch a polygon defined on an inscribed circle."),
    ("Insert DXF or DWG", "Insert the contents of a DXF or DWG file into a sketch as sketch entities."),
    ("Insert image", "Insert an image to use as a sketch guide."),
    ("Intersection", "Project the intersection, of a surface or face and the active sketch plane, onto the sketch plane."),
    ("Line", "Sketch a line segment or series of line segments."),
    ("Linear pattern", "Create multiple instances of sketch entities uniformly in 1 or 2 linear directions. Specify the distance between instances and the number of instances."),
    ("Midpoint", "Constrain a sketch point to the midpoint of a line or arc."),
    ("Midpoint line", "Sketch a line starting symmetrically from its midpoint."),
    ("Mirror", "Create a mirrored copy of a sketch entity about a specified line or plane."),
    ("Normal", "Make a curve and line, or a curve and plane, perpendicular."),
    ("Offset", "Create a copy of the selected sketch entity at a specified distance and direction from the original."),
    ("Parallel", "Make 2 or more lines parallel."),
    ("Perpendicular", "Make 2 lines perpendicular."),
    ("Pierce", "Make a sketch entity coincident with another entity outside the active sketch plane."),
    ("Point", "Add a point to a sketch."),
    ("Revolve", "Create, add to, subtract from, or intersect parts (by revolving sketch regions or planar faces about a central axis), surfaces (by revolving lines and curves about a central axis), or thin parts (by revolving sketch regions, planar faces, lines, or curves about a central axis)."),
    ("Slot", "Create a slot around continuous sketch entities."),
    ("Spline", "Sketch a multi-point curve. Drag any point along its length to make adjustments."),
    ("Spline control point", "Add points to a spline or to a Bezier curve control polygon by clicking anywhere along it."),
    ("Split", "Split sketch curves into multiple segments. For open curves, select 1 or more points. For closed curves, select 2 or more points."),
    ("Symmetric", "Make 2 entities of the same type symmetric, relative to a line, plane, or linear edge."),
    ("Tangent", "Make a tangent relation between 2 sketch entities, or between a sketch entity and a plane."),
    ("Tangent arc", "Sketch an arc tangent to a selected line or arc."),
    ("Text", "Add text to a sketch by drawing a bounding box. Specify the font and text style."),
    ("Transform", "Adjust the location, orientation and scale of a sketch entity using a manipulator."),
    ("Trim", "Trim a line or curve to the first intersecting point or bounding geometry. If no intersection or bounding geometry is found, the entire entity is deleted."),
    ("Use", "Project edges or faces of an existing part or sketch onto the active sketch plane."),
    ("Vertical", "Constrain 1 or more lines, or a set of points, to be vertical."),
];

/// Assembly tools, by title.
pub const ASSEMBLY: &[(&str, &str)] = &[
    ("Assembly circular pattern", "Create multiple instances of a part, surface, sketch, or subassembly. Specify the axis, angle, and number of instances."),
    ("Assembly linear pattern", "Create multiple instances of a part, surface, sketch, or subassembly. Specify up to 3 directions, distances, and number of instances."),
    ("Assembly mirror", "Create mirrored instances of a part, surface or subassembly. Specify the mirror plane and select instances to mirror. Use mirror strategy table to control how instances are mirrored."),
    ("Ball mate", "Allow rotation about the X, Y, and Z axes. The 1st selection serves as the rotational movement point, and the 2nd selection serves as the stationary point."),
    ("Create Part Studio in context", "Create a new Part Studio in the context of this assembly. The assembly appears in the Part Studio as context graphics that can be used for reference."),
    ("Cylindrical mate", "Allow translation and rotation about the Z axis. The 1st selection serves as the translational and rotational point, and the 2nd as the stationary point."),
    ("Display states", "Assign a name to a specific display state of an assembly. Specify which parts, subassemblies, and mates are hidden or in view."),
    ("Fastened mate", "Remove all degrees of freedom between 2 entities."),
    ("Gear relation", "Create a constant ratio of angular rotation between 2 mates with rotational degrees of freedom."),
    ("Group", "Lock the position of selected entities relative to one another."),
    ("Insert", "Insert parts, assemblies, sketches, or surfaces into this Assembly. Insert from this document, other documents, or standard content."),
    ("Linear relation", "Create a constant ratio of motion between 2 mates with linear degrees of freedom."),
    ("Mate connector", "Create a mate connector on or between parts, surfaces, or sketches. Mate connectors are used to locate and orient entities within an assembly mate."),
    ("Parallel mate", "Allow 2 entities to rotate parallel to each other and translate freely."),
    ("Pin slot mate", "Allow rotation about the Z axis and translation along the X axis. The 1st selection serves as the pin and the rotational movement point, and the 2nd selection serves as the translational movement."),
    ("Planar mate", "Allow translation along the X and Y axes, and rotation about the Z axis. The 1st selection serves as the movement point, and the 2nd as the stationary point."),
    ("Rack and pinion relation", "Relate a mate with a rotational degree of freedom to a mate with a linear degree of freedom."),
    ("Replace instance", "Replace instances with a selected part, surface, sketch, or subassembly."),
    ("Replicate", "Using a seed instance as input, locate geometry identical to that which the seed is mated and replicate the seed on matching geometry."),
    ("Revolute mate", "Allow rotation about the Z axis. The 1st selection serves as the rotational point, and the 2nd as the stationary point."),
    ("Screw relation", "Relate the rotational and translational degrees of freedom of a Cylindrical Mate."),
    ("Show mates mode", "Hover over or select an instance to show its mates."),
    ("Slider mate", "Allow translation along the Z axis. The 1st selection serves as the sliding point, and the 2nd as the stationary point."),
    ("Snap mode", "Drag an entity to snap to a Mate connector on another entity. Specify the mate type and orientation."),
    ("Tangent mate", "Mate 2 entities tangentially, based on the selected faces, edges, or vertices."),
    ("Update all references to latest versions", "Update all unpinned references to the latest versions or revisions."),
    ("Width mate", "Allow translation on a slot’s center plane and rotation about an axis perpendicular to that plane. The 1st selection defines the item to be centered. The 2nd selection defines the center plane."),
];

/// A description as paragraphs and numbered list items: `(None, text)` or
/// `(Some(n), text)` (`<br>` breaks paragraphs, each `<ol>` numbers its
/// `<li>` items from 1).
pub fn blocks(text: &str) -> Vec<(Option<usize>, String)> {
    let mut out: Vec<(Option<usize>, String)> = Vec::new();
    let mut n = 0;
    let mut rest = text;
    let mut push = |item: Option<usize>, s: &str| {
        let s = s.trim();
        if !s.is_empty() {
            out.push((item, s.to_string()));
        }
    };
    while !rest.is_empty() {
        let next = ["<br>", "<ol>", "</ol>", "<li>"].iter().filter_map(|t| rest.find(t).map(|i| (i, *t))).min();
        let Some((i, tag)) = next else {
            push(None, rest);
            break;
        };
        push(None, &rest[..i]);
        rest = &rest[i + tag.len()..];
        match tag {
            "<ol>" => n = 0,
            "<li>" => {
                let end = rest.find("</li>").unwrap_or(rest.len());
                n += 1;
                push(Some(n), &rest[..end]);
                rest = rest.get(end + "</li>".len()..).unwrap_or("");
            }
            _ => {}
        }
    }
    out
}

fn find(table: &'static [(&'static str, &'static str)], title: &str) -> Option<&'static str> {
    table.iter().find(|(t, _)| *t == title).map(|(_, d)| *d)
}

/// The description of a Part Studio feature tool by feature type id (its
/// registry name is the live title).
pub fn description(id: &str) -> Option<&'static str> {
    find(PART_STUDIO, noshape_model::features::get(id)?.name)
}

/// The description of a sketch toolbar tool by its tooltip title ("Line",
/// "Corner rectangle"; a few of our titles carry Onshape's longer label).
pub fn sketch_description(name: &str) -> Option<&'static str> {
    let title = match name {
        "Use (Project/Convert)" => "Use",
        "Sketch fillet" => "Fillet",
        "Sketch chamfer" => "Chamfer",
        n => n,
    };
    find(SKETCH, title)
}

/// The description of an assembly tool by its title.
pub fn assembly_description(title: &str) -> Option<&'static str> {
    find(ASSEMBLY, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every live Part Studio title is a feature of ours, so its tooltip and
    /// search pane show the live text.
    #[test]
    fn every_part_studio_text_reaches_its_feature() {
        for (title, text) in PART_STUDIO {
            let ty = noshape_model::features::all().find(|f| f.name == *title).unwrap_or_else(|| panic!("no feature titled {title}"));
            assert_eq!(description(ty.id), Some(*text), "{title}");
        }
    }

    /// The tables are live's texts word for word (checked against the
    /// capture, whose list entries are the rendered text, when it is on this
    /// machine).
    #[test]
    fn the_tables_match_the_live_capture() {
        let visible = |s: &str| blocks(s).into_iter().map(|b| b.1).collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
        let Ok(raw) = std::fs::read_to_string("/root/onshape-research/live/dom/ui-e-tool-descriptions-live.json") else { return };
        let live: serde_json::Value = serde_json::from_str(&raw).unwrap();
        for (group, table) in [("partStudio", PART_STUDIO), ("sketch", SKETCH), ("assembly", ASSEMBLY)] {
            let map = live[group].as_object().unwrap();
            assert_eq!(map.len(), table.len(), "{group}");
            for (title, _) in table {
                let shown = map[*title].as_str().unwrap().split_whitespace().collect::<Vec<_>>().join(" ");
                assert_eq!(visible(text_of(title, table)), shown, "{group} {title}");
            }
        }
    }

    fn text_of(title: &str, table: &'static [(&'static str, &'static str)]) -> &'static str {
        find(table, title).unwrap()
    }

    #[test]
    fn lists_become_numbered_items() {
        let b = blocks(find(PART_STUDIO, "Gusset").unwrap());
        assert_eq!(b[0], (None, "Create gussets between adjacent frame segments:".to_string()));
        assert_eq!(b[1].0, Some(1));
        assert_eq!(b.last().unwrap().0, Some(3));
        assert_eq!(blocks("Plain text."), vec![(None, "Plain text.".to_string())]);
    }

    #[test]
    fn sketch_titles_with_our_longer_labels() {
        assert_eq!(sketch_description("Use (Project/Convert)"), find(SKETCH, "Use"));
        assert!(sketch_description("Line").is_some());
    }
}
