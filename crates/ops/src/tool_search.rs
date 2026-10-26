//! Onshape's tool search ("Search tools…", Alt+C): which tools a typed text
//! finds and in what order, for the Part Studio (features, or a sketch's
//! tools while sketching) and the Assembly. Shared by the app's search
//! popovers and the agent API's ui_search_tools.

use noshape_scene::sketch_mode::{ConstraintTool, Tool};

/// The Part Studio's ranking (live, menus/tool-search-*.png): the tools
/// whose names contain the typed letters in order ("fil" finds Fillet,
/// Fill, Finish flex PCB model, …), by where the first letter matches, then
/// how tightly the letters sit, then the given order. Indices into `names`.
pub fn studio_rank(names: &[&str], query: &str) -> Vec<usize> {
    let q: Vec<char> = query.trim().to_lowercase().chars().collect();
    if q.is_empty() {
        return Vec::new();
    }
    // (first matched index, span of the match) for the in-order match of `q` in `name`.
    let score = |name: &str| -> Option<(usize, usize)> {
        let chars: Vec<char> = name.to_lowercase().chars().collect();
        let mut best: Option<(usize, usize)> = None;
        for start in 0..chars.len() {
            if chars[start] != q[0] {
                continue;
            }
            let mut k = 1;
            let mut end = start;
            for (i, c) in chars.iter().enumerate().skip(start + 1) {
                if k == q.len() {
                    break;
                }
                if *c == q[k] {
                    k += 1;
                    end = i;
                }
            }
            if k == q.len() {
                let cand = (start, end - start);
                if best.is_none_or(|b| cand < b) {
                    best = Some(cand);
                }
            }
        }
        best
    };
    let mut found: Vec<((usize, usize), usize)> = names.iter().enumerate().filter_map(|(i, n)| score(n).map(|s| (s, i))).collect();
    found.sort();
    found.into_iter().map(|(_, i)| i).collect()
}

/// The Assembly's ranking (live dom/assembly-tool-search.txt): names
/// starting with the text, then names containing it, then names holding
/// its letters in order; shorter names first within each tier, then the
/// given order; at most 10 results. Indices into `names`.
pub fn assembly_rank(names: &[&str], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let subsequence = |n: &str| {
        let mut it = n.chars();
        q.chars().filter(|c| !c.is_whitespace()).all(|c| it.any(|x| x == c))
    };
    let mut hits: Vec<(u8, usize, usize)> = names
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            let n = name.to_lowercase();
            let tier = if n.starts_with(&q) {
                0
            } else if n.contains(&q) {
                1
            } else if subsequence(&n) {
                2
            } else {
                return None;
            };
            Some((tier, n.chars().count(), i))
        })
        .collect();
    hits.sort();
    hits.into_iter().take(10).map(|(_, _, i)| i).collect()
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
];

/// Every sketch toolbar tool, in toolbar order (tool search).
pub fn all_sketch_tools() -> impl Iterator<Item = Tool> {
    let groups = SKETCH_DRAW_TOOLS.iter().chain(SKETCH_USE_TOOLS).chain(SKETCH_EDIT_TOOLS).chain(SKETCH_INSERT_TOOLS);
    groups.flat_map(|g| g.iter().copied()).chain(std::iter::once(Tool::Dimension)).chain(SKETCH_CONSTRAINT_TOOLS.iter().map(|c| Tool::Constraint(*c)))
}
