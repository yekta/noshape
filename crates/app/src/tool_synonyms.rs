//! Onshape's tool search synonyms (bundle: the CommandSearch component's
//! map per context, tool name → search terms). A term that matches offers
//! "Did you mean:" chips with the tools it stands for.

/// Partstudio tools and their synonyms.
pub const PART_STUDIO: &[(&str, &[&str])] = &[
    ("3D fit spline", &["3D curve", "3D sketch", "3D spline", "Curve in space", "Wire"]),
    ("Bend", &["Sketch Bend"]),
    ("Bend relief", &["Punch"]),
    ("Boolean", &["Add", "Combine", "Knit", "Intersect", "Join", "Merge", "Remove", "Subtract", "Union", "Unite"]),
    ("Boundary surface", &["Surface"]),
    ("Bridging curve", &["Curve", "Wire"]),
    ("Chamfer", &["Break", "Corner"]),
    ("Circular pattern", &["Array"]),
    ("Composite curve", &["Join curves", "Wire"]),
    ("Corner", &["Punch", "Relief"]),
    ("Curve pattern", &["Array"]),
    ("Cut list", &["Beams", "Weldments"]),
    ("Delete face", &["Cap", "Delete fillet", "Heal", "Leave open", "Remove face", "Void"]),
    ("Delete part", &["Delete body"]),
    ("Derived", &["Export", "Import", "Import", "Insert", "Link", "Reference", "Transfer"]),
    ("Draft ", &["Angle", "Taper"]),
    ("Enclose", &["Knit", "Make solid", "Solidify"]),
    ("Extrude", &["Base", "Boss", "Cut", "Pad", "Pocket", "Protrusion", "Punch", "Subtract", "Surface"]),
    ("Fill", &["N-sided boundary", "Patch", "Surface", "Surface fill", "Boundary surface"]),
    ("Fillet", &["Blend", "Break", "Corner", "Round"]),
    ("Finish sheet metal model", &[]),
    ("Flange", &["Bend", "Sheet Metal"]),
    ("Frame", &["Beams", "Weldments"]),
    ("Frame trim", &["Beams", "Weldments"]),
    ("Helix", &["Curve", "Spiral", "Spring", "Thread", "Wire"]),
    ("Hem", &[]),
    ("Hole", &["Bolt", "Bore", "Clearance", "Counterbore", "Countersink", "Drill", "Tap", "Taper", "Thread"]),
    ("Linear pattern", &["Array"]),
    ("Loft", &["Boundary", "Cut", "Boss", "Surface", "Swept", "Boundary surface"]),
    ("Make joint", &["Bend"]),
    ("Mate connector", &["Axis", "Construction", "Coordinate system", "Datum plane", "Mate joint", "Plane", "Reference plane", "Work"]),
    ("Mirror", &["Flip", "Reflect", "Symmetric"]),
    ("Modify fillet", &["Change fillet", "Edit fillet", "Remove fillet"]),
    ("Move face", &["Direct Edit"]),
    ("Move boundary", &["Trim", "Untrim", "Heal", "Surface trim", "Trim surface", "Extend", "Surface extend", "Extend surface", "Surface", "Edge extend", "Extend edge", "Edge trim", "Trim edge"]),
    ("Offset surface", &["Face", "Offset"]),
    ("Plane", &["Construction", "Coordinate system", "Datum plane", "Reference plane", "Work"]),
    ("Projected curve", &["Projection", "Wire"]),
    ("Replace face", &["Change", "Heal", "Merge", "Use face"]),
    ("Revolve", &["Cut", "Boss", "Groove", "Rotate", "Shaft", "Surface"]),
    ("Rib", &["Reinforcement", "Stiffener", "Web"]),
    ("Shell", &["Hollow", "Indent", "Offset"]),
    ("Sketch", &["Draw", "Layout"]),
    ("Split", &["Cut", "Boss", "Indent", "Trim"]),
    ("Sheet metal model", &[]),
    ("Sweep", &["Cut", "Boss", "Guide", "Path", "Rib", "Surface", "Swept", "Slot"]),
    ("Tab", &["Sheet Metal"]),
    ("Tag", &["Tag profile", "Beams", "Weldments"]),
    ("Tag profile", &["Beams", "Weldments"]),
    ("Thicken", &["Surface thicken"]),
    ("Transform", &["Copy", "Move", "Rotate", "Scale", "Translate", "Translation"]),
    ("Variable", &["Constant", "Expression"]),
    ("Wrap", &["Around", "Curved", "Deboss", "Emboss", "Imprint", "Roll", "Scribe", "Engrave", "Project"]),
];

/// Sketch tools and their synonyms.
pub const SKETCH: &[(&str, &[&str])] = &[
    ("Center rectangle", &["Quadrilateral", "Square"]),
    ("Circumscribed polygon", &["Polygon", "Shape"]),
    ("Coincident", &["Join"]),
    ("Conic", &["Hyperbola", "Parabola"]),
    ("Control Point Spline", &["Curve"]),
    ("Corner rectangle", &["Square"]),
    ("Ellipse", &["Oval"]),
    ("Equal", &["Congruent"]),
    ("Extrude", &["Base", "Boss", "Cut", "Protrusion", "Punch", "Surface"]),
    ("Fillet", &["Blend", "Round"]),
    ("Horizontal", &["Align"]),
    ("Inscribed polygon", &["Polygon", "Shape"]),
    ("Insert DXF or DWG", &["File", "Image", "Picture"]),
    ("Insert image", &["File", "Picture"]),
    ("Parallel", &["Align"]),
    ("Revolve", &["Cut", "Rotate", "Surface"]),
    ("Slot", &["Keyway"]),
    ("Spline", &["Curve"]),
    ("Spline point", &[]),
    ("Spline handle", &[]),
    ("Text", &["Sketch text"]),
    ("Transform", &["Move"]),
    ("Use", &["Convert", "Project", "Projection", "entities"]),
    ("Vertical", &["Align"]),
];

/// Assembly tools and their synonyms.
pub const ASSEMBLY: &[(&str, &[&str])] = &[
    ("Assembly circular pattern", &[]),
    ("Assembly linear pattern", &[]),
    ("Assembly mirror", &[]),
    ("Ball mate", &["Hem", "Socket"]),
    ("Create Part Studio in context", &[]),
    ("Cylindrical mate", &["Translate"]),
    ("Display states", &["Hide", "View"]),
    ("Fastened mate", &["Coincident", "Fix", "Flange", "Stop"]),
    ("Gear relation", &["Joint", "Universal"]),
    ("Group", &["Lock", "Rigid"]),
    ("Linear relation", &["Coupler"]),
    ("Named positions", &["View"]),
    ("Parallel mate", &["Translate"]),
    ("Pin slot mate", &["Key", "Keyway"]),
    ("Planar mate", &["Translate"]),
    ("Rack and pinion relation", &[]),
    ("Replace instance", &["Switch"]),
    ("Replicate", &["Copy"]),
    ("Revolute mate", &["Angle", "Concentric", "Hinge"]),
    ("Screw relation", &["Thread"]),
    ("Simulation load", &["Load"]),
    ("Slider mate", &["Translate"]),
    ("Snap mode", &[]),
    ("Tangent mate", &["Cam", "Follower", "Path"]),
    ("Width mate", &[]),
];
/// The tools tool search suggests for `search` ("Did you mean:"). With
/// results (`exact`), the tools of that very term; without, the tools of
/// every term containing the search's letters in order (the bundle's
/// matcher). Terms are compared lowercased and trimmed, in the order they
/// first appear in `map`; each tool once.
pub fn suggestions(map: &[(&'static str, &[&'static str])], search: &str, exact: bool) -> Vec<&'static str> {
    let search = search.trim().to_lowercase();
    if search.is_empty() {
        return Vec::new();
    }
    let subsequence = |term: &str| {
        let mut rest = term.chars();
        search.chars().all(|c| rest.any(|t| t == c))
    };
    // Terms in first-appearance order, each with its tools.
    let mut terms: Vec<(String, Vec<&'static str>)> = Vec::new();
    for (tool, syns) in map {
        for s in syns.iter() {
            let s = s.trim().to_lowercase();
            match terms.iter_mut().find(|(t, _)| *t == s) {
                Some((_, tools)) => tools.push(tool),
                None => terms.push((s, vec![tool])),
            }
        }
    }
    let mut out: Vec<&'static str> = Vec::new();
    for (term, tools) in &terms {
        if (exact && *term == search) || (!exact && subsequence(term)) {
            for t in tools {
                if !out.contains(t) {
                    out.push(t);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_suggests_derived() {
        // Live menus/tool-search-export.png: "Did you mean: [Derived]".
        assert_eq!(suggestions(PART_STUDIO, "export", false), vec!["Derived"]);
        assert_eq!(suggestions(PART_STUDIO, "cut", true), vec!["Extrude", "Loft", "Revolve", "Split", "Sweep"]);
        // Live: these find nothing and suggest nothing.
        for term in ["material", "assign", "release", "revision", "measure", "mass", "selection", "properties", "appearance"] {
            assert!(suggestions(PART_STUDIO, term, false).is_empty(), "{term}");
        }
    }
}
