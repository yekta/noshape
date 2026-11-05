//! Onshape's tool search ("Search tools…", Alt+C): which tools a typed text
//! finds and in what order, for the Part Studio (features, or a sketch's
//! tools while sketching) and the Assembly. Shared by the app's search
//! popovers and the agent API's ui_search_tools.

use noshape_scene::sketch_mode::{ConstraintTool, Tool};

/// Live's score of a tool's name for the typed text (its client's command search, live 2064): each typed letter found
/// in order, 0.7 right after the last one, else 0.1 (0.9 at a word's start), 0.1 more in the same case; the mean of
/// that over the name's and the text's lengths, 0.15 more when the first letters agree (up to 0.85). 0: no match.
pub fn onshape_score(name: &str, query: &str) -> f64 {
    if name == query || query.is_empty() {
        return 1.0;
    }
    let e: Vec<char> = name.chars().collect();
    let t: Vec<char> = query.chars().collect();
    let low = |c: char| c.to_lowercase().next().unwrap_or(c);
    let (mut d, mut u) = (0.0, 0);
    for &q in &t {
        let Some(c) = (u..e.len()).find(|&i| low(e[i]) == low(q)) else { return 0.0 };
        let mut a = if c == u { 0.7 } else if c > 0 && e[c - 1] == ' ' { 0.9 } else { 0.1 };
        if e[c] == q {
            a += 0.1;
        }
        d += a;
        u = c + 1;
    }
    let mut l = 0.5 * (d / e.len() as f64 + d / t.len() as f64);
    if low(e[0]) == low(t[0]) && l < 0.85 {
        l += 0.15;
    }
    l
}

/// Live's ranking (live 2064): the tools whose names score above 0 (0.2 more for a tool with a built-in shortcut),
/// tools still on the toolbar before ones taken off it, then the score, ties in the given order; the first 10. An
/// empty text finds nothing (the caller lists the tools used last). Indices into `names`.
pub fn rank(names: &[&str], boosted: impl Fn(usize) -> bool, removed: impl Fn(usize) -> bool, query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(bool, f64, usize)> = names
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let s = onshape_score(n, query);
            (s > 0.0).then(|| (removed(i), s + if boosted(i) { 0.2 } else { 0.0 }, i))
        })
        .collect();
    hits.sort_by(|x, y| x.0.cmp(&y.0).then(y.1.total_cmp(&x.1)).then(x.2.cmp(&y.2)));
    hits.into_iter().take(10).map(|(.., i)| i).collect()
}

/// The Part Studio's tools with a built-in shortcut (live's table: New sketch, Extrude, Fillet, Revolve, Mate connector).
const STUDIO_BOOSTED: &[&str] = &["Sketch", "Extrude", "Fillet", "Revolve", "Mate connector"];

/// The Part Studio's ranking ([`rank`]): "fil" finds Fillet, Fill, Finish flex PCB model, … Indices into `names`.
pub fn studio_rank(names: &[&str], query: &str) -> Vec<usize> {
    rank(names, |i| STUDIO_BOOSTED.contains(&names[i]), |_| false, query)
}

/// The Assembly's ranking ([`rank`]; the tools with a shortcut are live's built-in ones: Insert, the mates, Mate
/// connector, Snap mode, Show mates mode). Indices into `names` ([`ASSEMBLY_TOOLS`]' names).
pub fn assembly_rank(names: &[&str], query: &str) -> Vec<usize> {
    let boosted = |i: usize| ASSEMBLY_TOOLS.iter().any(|t| t.1 == names[i] && t.2.is_some());
    rank(names, boosted, |_| false, query)
}

/// The Assembly toolbar's tools in toolbar order: (icon, name, shortcut).
pub const ASSEMBLY_TOOLS: &[(&str, &str, Option<&str>)] = &[
    ("update-all-button", "Update all references to latest versions", None),
    ("insert", "Insert", Some("i")),
    ("mate-connector-assembly", "Mate connector", Some("ctrl+m")),
    ("fastened", "Fastened mate", Some("m")),
    ("revolute", "Revolute mate", Some("m")),
    ("slider", "Slider mate", Some("m")),
    ("planar", "Planar mate", Some("m")),
    ("cylindrical", "Cylindrical mate", Some("m")),
    ("pin-slot", "Pin slot mate", Some("m")),
    ("ball", "Ball mate", Some("m")),
    ("parallel-mate", "Parallel mate", Some("m")),
    ("group", "Group", None),
    ("snap-mode", "Snap mode", Some("shift+s")),
    ("show-mates", "Show mates mode", Some("h")),
    ("replicate", "Replicate", None),
    ("replace-part", "Replace instance", None),
    ("linear-pattern", "Assembly linear pattern", None),
    ("circular-pattern", "Assembly circular pattern", None),
    ("mirror", "Assembly mirror", None),
    ("gear-relation", "Gear relation", None),
    ("rack-pinion", "Rack and pinion relation", None),
    ("screw-relation", "Screw relation", None),
    ("linear-relation", "Linear relation", None),
    ("display-states", "Display states", None),
    ("create-partstudio-incontext", "Create Part Studio in context", None),
    ("tangent-mate", "Tangent mate", None),
    ("width-mate", "Width mate", None),
];

/// A sketch tool's name in the tool search: live's title, the label without
/// its shortcut ("Line (L)" -> "Line") and plain where the toolbar label
/// isn't ("Sketch fillet (Shift+F)" -> "Fillet", live qa5-ts-sketch-fillet).
pub fn sketch_search_name(t: Tool) -> &'static str {
    let l = t.label();
    match l.rsplit_once(" (").map_or(l, |(name, _)| name) {
        "Sketch fillet" => "Fillet",
        "Sketch chamfer" => "Chamfer",
        "Center rectangle" => "Center point rectangle",
        "Use / project edges" => "Use",
        name => name,
    }
}

/// The shortcut in a sketch tool's label ("Offset (O)" -> "O"), shown as the
/// search row's key badge (live: Fillet "shift f", Offset "o").
pub fn sketch_tool_shortcut(t: Tool) -> Option<&'static str> {
    t.label().rsplit_once(" (").and_then(|(_, k)| k.strip_suffix(')'))
}

/// Onshape's sketch toolbar groups (the button shows the group's last used tool).
pub const SKETCH_DRAW_TOOLS: &[&[Tool]] = &[
    &[Tool::Line, Tool::MidpointLine],
    &[Tool::CornerRect, Tool::CenterRect, Tool::AlignedRect],
    &[Tool::Circle, Tool::ThreePointCircle, Tool::Ellipse],
    &[Tool::Arc3, Tool::TangentArc, Tool::CenterArc, Tool::EllipticalArc, Tool::Conic],
    &[Tool::Polygon, Tool::CircumPolygon],
    &[Tool::Spline, Tool::Bezier, Tool::SplinePoint],
    &[Tool::Point],
    &[Tool::Text],
];

pub const SKETCH_USE_TOOLS: &[&[Tool]] = &[&[Tool::Use, Tool::Intersection]];

pub const SKETCH_EDIT_TOOLS: &[&[Tool]] = &[
    &[Tool::Fillet, Tool::Chamfer],
    &[Tool::Trim, Tool::Extend, Tool::Split],
    &[Tool::Offset, Tool::Slot],
    &[Tool::Mirror],
    &[Tool::LinearPattern, Tool::CircularPattern, Tool::Transform],
];

pub const SKETCH_INSERT_TOOLS: &[&[Tool]] = &[&[Tool::InsertDxf, Tool::InsertImage]];

pub const SKETCH_CONSTRAINT_TOOLS: &[ConstraintTool] = &[
    ConstraintTool::Coincident,
    ConstraintTool::Concentric,
    ConstraintTool::Parallel,
    ConstraintTool::Tangent,
    ConstraintTool::Horizontal,
    ConstraintTool::Vertical,
    ConstraintTool::Perpendicular,
    ConstraintTool::Equal,
    ConstraintTool::Midpoint,
    ConstraintTool::Normal,
    ConstraintTool::Pierce,
    ConstraintTool::Symmetric,
    ConstraintTool::Fix,
    ConstraintTool::Curvature,
    ConstraintTool::Flow,
];

/// What a sketch's tool search finds (live 2064): Extrude and Revolve, Construction, then every sketch toolbar tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchSearch {
    /// A Part Studio feature ("extrude", "revolve"): it finishes the sketch.
    Feature(&'static str),
    Construction,
    Tool(Tool),
}

impl SketchSearch {
    pub fn name(self) -> &'static str {
        match self {
            SketchSearch::Feature("revolve") => "Revolve",
            SketchSearch::Feature(_) => "Extrude",
            SketchSearch::Construction => "Construction",
            SketchSearch::Tool(t) => sketch_search_name(t),
        }
    }

    /// Its key badge.
    pub fn shortcut(self) -> Option<&'static str> {
        match self {
            SketchSearch::Feature("revolve") => Some("Shift+W"),
            SketchSearch::Feature(_) => Some("Shift+E"),
            SketchSearch::Construction => Some("Q"),
            SketchSearch::Tool(t) => sketch_tool_shortcut(t),
        }
    }

    /// In live's built-in shortcut table (the rest of the keys are preferences): 0.2 more in the ranking.
    fn boosted(self) -> bool {
        use ConstraintTool as C;
        match self {
            SketchSearch::Feature(_) | SketchSearch::Construction => true,
            SketchSearch::Tool(t) => matches!(
                t,
                Tool::Line | Tool::CornerRect | Tool::CenterRect | Tool::Circle | Tool::Arc3 | Tool::Fillet | Tool::Trim | Tool::Extend | Tool::Offset | Tool::Use | Tool::Dimension | Tool::Point
            ) || matches!(t, Tool::Constraint(C::Coincident | C::Parallel | C::Tangent | C::Horizontal | C::Vertical | C::Equal)),
        }
    }
}

/// The sketch tool search's tools in live's order (its ties keep it: live 2064's 36 lists).
pub fn searchable_sketch_tools() -> Vec<SketchSearch> {
    [SketchSearch::Feature("extrude"), SketchSearch::Feature("revolve"), SketchSearch::Construction].into_iter().chain(all_sketch_tools().map(SketchSearch::Tool)).collect()
}

/// A sketch's tool search ([`rank`]).
pub fn sketch_rank(query: &str) -> Vec<SketchSearch> {
    let tools = searchable_sketch_tools();
    let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    rank(&names, |i| tools[i].boosted(), |_| false, query).into_iter().map(|i| tools[i]).collect()
}

/// Every sketch toolbar tool, in toolbar order (tool search).
pub fn all_sketch_tools() -> impl Iterator<Item = Tool> {
    let groups = SKETCH_DRAW_TOOLS.iter().chain(SKETCH_USE_TOOLS).chain(SKETCH_EDIT_TOOLS).chain(SKETCH_INSERT_TOOLS);
    groups.flat_map(|g| g.iter().copied()).chain(std::iter::once(Tool::Dimension)).chain(SKETCH_CONSTRAINT_TOOLS.iter().map(|c| Tool::Constraint(*c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live 2064's sketch lists (a fresh sketch on Top), order and membership.
    #[test]
    fn a_sketchs_tool_search_ranks_as_lives() {
        let lists: &[(&str, &[&str])] = &[
            ("c", &["Coincident", "Construction", "Corner rectangle", "Center point circle", "Center point rectangle", "Conic", "Chamfer", "Curvature", "3 point circle", "Concentric"]),
            ("ci", &["Coincident", "Construction", "Center point circle", "Center point rectangle", "Circular pattern", "Circumscribed polygon", "3 point circle", "Conic", "Concentric", "Center point arc"]),
            ("e", &["Equal", "Extend", "Extrude", "Ellipse", "Elliptical arc", "Use", "Line", "Fillet", "Offset", "Revolve"]),
            ("ex", &["Extend", "Extrude", "Text", "Insert DXF or DWG"]),
            ("f", &["Fillet", "Fix", "Flow", "Offset", "Chamfer", "Transform", "Insert DXF or DWG"]),
            ("r", &["Revolve", "Aligned rectangle", "Trim", "Extrude", "Parallel", "Vertical", "Horizontal", "3 point arc", "Construction", "Corner rectangle"]),
            ("s", &["Slot", "Split", "Spline", "Symmetric", "Spline control point", "Use", "Offset", "Dimension", "Construction", "Ellipse"]),
            ("sp", &["Split", "Spline", "Spline control point", "Inscribed polygon", "Circumscribed polygon"]),
            ("t", &["Trim", "Tangent", "Text", "Transform", "Tangent arc", "Point", "Fillet", "Extend", "Offset", "Extrude"]),
            ("me", &["Dimension", "Midpoint line", "Chamfer", "Symmetric", "Insert image", "Circumscribed polygon"]),
            ("pa", &["Parallel", "3 point arc", "Center point arc", "Center point rectangle", "Linear pattern", "Circular pattern", "Perpendicular", "Elliptical arc"]),
        ];
        for (q, want) in lists {
            assert_eq!(sketch_rank(q).iter().map(|t| t.name()).collect::<Vec<_>>(), *want, "{q}");
        }
        assert!(sketch_rank("").is_empty());
    }

    /// Live's scores as its client prints them (live 2064: "c" Coincident 0.735 boosted, Conic 0.57).
    #[test]
    fn live_scores_names_letter_by_letter() {
        assert!((onshape_score("Coincident", "c") + 0.2 - 0.735).abs() < 1e-3);
        assert!((onshape_score("Conic", "c") - 0.57).abs() < 1e-3);
        assert!((onshape_score("Conic", "ci") - 0.465).abs() < 1e-3);
        assert_eq!(onshape_score("Fillet", "x"), 0.0);
    }

    /// The Part Studio's and Assembly's lists (live menus/tool-search-fil.png, dom/assembly-tool-search.txt).
    #[test]
    fn studio_and_assembly_searches_rank_as_lives() {
        let names = ["3D fit spline", "Fill", "Fillet", "Finish flex PCB model", "Finish sheet metal model", "Modify fillet", "Extrude"];
        assert_eq!(studio_rank(&names, "fil").iter().map(|&i| names[i]).collect::<Vec<_>>(), ["Fillet", "Fill", "Finish flex PCB model", "Finish sheet metal model", "3D fit spline", "Modify fillet"]);
        let asm: Vec<&str> = ASSEMBLY_TOOLS.iter().map(|t| t.1).collect();
        let found = |q: &str| assembly_rank(&asm, q).iter().map(|&i| asm[i]).collect::<Vec<_>>();
        assert_eq!(found("mate"), ["Mate connector", "Ball mate", "Slider mate", "Planar mate", "Fastened mate", "Revolute mate", "Pin slot mate", "Parallel mate", "Show mates mode", "Cylindrical mate"]);
        assert_eq!(found("pattern"), ["Assembly linear pattern", "Assembly circular pattern", "Update all references to latest versions"]);
    }
}
