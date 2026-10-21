//! Part Studio side of the customizable keyboard shortcuts: actions that
//! Onshape leaves unassigned get a key through Preferences › Keyboard
//! shortcuts and arrive as `shortcuts::RunShortcut` (General analysis and
//! list commands, Part Studio features, 3D view commands, sketch tools).

use super::PartStudioView;
use super::analysis::Analysis;
use super::analysis_tools::ToolKind;
use crate::scene_builder::{DisplayMode, TangentEdges};
use crate::shortcuts::{self, Tab};
use crate::sketch_mode::Tool;
use gpui_kit::*;
use noshape_render::StandardView;

/// Feature type of each Part Studio tab action without a GPUI action.
const FEATURES: &[(&str, &str)] = &[
    ("3D fit spline", "fitSpline"),
    ("Bend", "sheetMetalBend"),
    ("Bend relief", "sheetMetalBendRelief"),
    ("Body draft", "bodyDraft"),
    ("Boolean", "booleanBodies"),
    ("Boundary surface", "boundarySurface"),
    ("Bridging curve", "bridgingCurve"),
    ("Chamfer", "chamfer"),
    ("Circular pattern", "circularPattern"),
    ("Composite curve", "compositeCurve"),
    ("Composite part", "compositePart"),
    ("Constrained surface", "constrainedSurface"),
    ("Corner", "sheetMetalCorner"),
    ("Corner break", "sheetMetalCornerBreak"),
    ("Curve pattern", "curvePattern"),
    ("Cut list", "cutlist"),
    ("Decal", "decal"),
    ("Delete face", "deleteFace"),
    ("Delete part", "deleteBodies"),
    ("Derived", "importDerived"),
    ("Draft", "draft"),
    ("Edit curve", "editCurve"),
    ("Enclose", "enclose"),
    ("End cap", "endcap"),
    ("External thread", "externalThread"),
    ("Extrude", "extrude"),
    ("Face blend", "faceBlend"),
    ("Fill", "fill"),
    ("Fillet", "fillet"),
    ("Finish flex PCB model", "pcbEnd"),
    ("Finish sheet metal model", "sheetMetalEnd"),
    ("Flange", "sheetMetalFlange"),
    ("Flex PCB model", "pcbStart"),
    ("Form", "sheetMetalFormed"),
    ("Frame", "frame"),
    ("Frame trim", "frameTrim"),
    ("Gusset", "gusset"),
    ("Helix", "helix"),
    ("Hem", "sheetMetalHem"),
    ("Hole", "hole"),
    ("Intersection curve", "intersectionCurve"),
    ("Isocline", "isocline"),
    ("Isoparameteric curve", "isoparametricCurve"),
    ("Jog", "sheetMetalJog"),
    ("Linear pattern", "linearPattern"),
    ("Loft", "loft"),
    ("Make joint", "sheetMetalMakeJoint"),
    ("Mirror", "mirror"),
    ("Modify fillet", "modifyFillet"),
    ("Move boundary", "extendSurface"),
    ("Move face", "moveFace"),
    ("Mutual trim", "mutualTrim"),
    ("Offset curve", "offsetCurveOnFace"),
    ("Offset surface", "offsetSurface"),
    ("Plane", "cPlane"),
    ("Projected curve", "projectCurves"),
    ("Query variable", "queryVariable"),
    ("Replace face", "replaceFace"),
    ("Revolve", "revolve"),
    ("Rib", "rib"),
    ("Routing curve", "routingCurve"),
    ("Ruled surface", "ruledSurface"),
    ("Sheet metal loft", "sheetMetalLoft"),
    ("Sheet metal model", "sheetMetalStart"),
    ("Shell", "shell"),
    ("Sketch", "sketch"),
    ("Split", "splitPart"),
    ("Sweep", "sweep"),
    ("Tab", "sheetMetalTab"),
    ("Tag", "tag"),
    ("Thicken", "thicken"),
    ("Transform", "transform"),
    ("Trim curve", "trimCurve"),
    ("Variable", "assignVariable"),
    ("Wrap", "wrap"),
];

/// A sketch tool's name: its toolbar tooltip without the key ("Line").
pub(crate) fn tool_name(t: Tool) -> &'static str {
    let l = t.label();
    l.rsplit_once(" (").map_or(l, |(n, _)| n)
}

/// Every sketch tool (name, icon), for Preferences › Shortcut toolbars.
pub(crate) fn sketch_toolbar_tools() -> Vec<(&'static str, &'static str)> {
    super::sketch::all_sketch_tools().map(|t| (tool_name(t), t.icon())).collect()
}

/// Onshape's default sketch S toolbar, by name.
pub(crate) fn default_sketch_shortcut_names() -> Vec<&'static str> {
    super::sketch::SKETCH_SHORTCUT_TOOLS.iter().map(|t| tool_name(*t)).collect()
}

/// The sketch S toolbar's tools (Preferences › Shortcut toolbars › Sketch).
pub(super) fn sketch_shortcut_toolbar(prefs: &super::toolbar_prefs::ToolbarPrefs) -> Vec<Tool> {
    let defaults = default_sketch_shortcut_names();
    prefs.sketch_shortcut_tools(&defaults).iter().filter_map(|n| super::sketch::all_sketch_tools().find(|t| tool_name(*t) == n)).collect()
}

/// Feature types of the Part Studio toolbar, in toolbar order (the tools
/// the Part Studio S toolbar can show).
pub(crate) fn toolbar_feature_ids() -> Vec<&'static str> {
    super::TOOLBAR.iter().flat_map(|g| g.iter()).flat_map(|items| items.iter().copied()).collect()
}

/// The sketch tool of a Sketch tab action ("Center point rectangle*").
pub(super) fn sketch_tool(name: &str) -> Option<Tool> {
    let name = name.trim_end_matches('*');
    let label = match name {
        "Center point rectangle" => "Center rectangle",
        "Mirror sketch" => "Mirror",
        "Use" => "Use / project edges",
        other => other,
    };
    super::sketch::all_sketch_tools().find(|t| tool_name(*t) == label)
}

impl PartStudioView {
    /// Runs a customized shortcut of an action without a GPUI action.
    pub(crate) fn run_shortcut(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = shortcuts::entry(id) else { return };
        match e.tab {
            Tab::General => self.run_general(e.name, window, cx),
            Tab::PartStudio => {
                if let Some((_, ty)) = FEATURES.iter().find(|(n, _)| *n == e.name) {
                    self.start_feature(ty, window, cx);
                }
            }
            Tab::View3d => self.run_view(e.name, window, cx),
            Tab::Sketch => {
                if self.sketch.is_none() {
                    return;
                }
                match e.name {
                    "Constraint manager" => self.toggle_constraint_manager(cx),
                    "Profile inspector" => self.toggle_profile_inspector(cx),
                    name => {
                        if let Some(tool) = sketch_tool(name) {
                            self.activate_tool(tool, window, cx);
                        }
                    }
                }
            }
            Tab::Assembly | Tab::FeatureStudio | Tab::Drawing => {}
        }
    }

    fn run_general(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let tool = match name {
            "Connection analysis" => Some(ToolKind::Connection),
            "Deviation analysis" => Some(ToolKind::Deviation),
            "Draft analysis" => Some(ToolKind::Draft),
            "Flatten surfaces" => Some(ToolKind::Flatten),
            "Interference detection" => Some(ToolKind::Interference),
            "Reflection analysis" => Some(ToolKind::Reflection),
            "Zebra stripes" => Some(ToolKind::Zebra),
            _ => None,
        };
        if let Some(kind) = tool {
            self.toggle_analysis_tool(kind, window, cx);
            return;
        }
        match name {
            "Curvature color map" => self.toggle_analysis(Analysis::Curvature, window, cx),
            "Collapse all in list" => self.set_all_folders_collapsed(true, cx),
            "Expand all in list" => self.set_all_folders_collapsed(false, cx),
            "Mass and section properties" => {
                // As the Mass properties button at the bottom right.
                self.ui.measure_pinned = false;
                self.set_show_mass(!self.show_mass, cx);
            }
            _ => {}
        }
    }

    fn run_view(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mode = match name {
            "Shaded with edges" => Some(DisplayMode::ShadedWithEdges),
            "Shaded without edges" => Some(DisplayMode::Shaded),
            "Translucent" => Some(DisplayMode::Translucent),
            "Unshaded" => Some(DisplayMode::HiddenLine),
            _ => None,
        };
        if let Some(m) = mode {
            self.set_display_mode(m, window, cx);
            return;
        }
        let tangent = match name {
            "Tangent edges visible" => Some(TangentEdges::Visible),
            "Tangent edges phantom" => Some(TangentEdges::Phantom),
            "Tangent edges hidden" => Some(TangentEdges::Removed),
            _ => None,
        };
        if let Some(t) = tangent {
            self.opts.tangent_edges = t;
            let d = self.display.clone();
            self.apply_display(d, window, cx);
            return;
        }
        match name {
            "Dimetric" => self.view(StandardView::Dimetric, cx),
            "Trimetric" => self.view(StandardView::Trimetric, cx),
            "Graphics preferences" => self.open_graphics_prefs(window, cx),
            "Previous view" => self.previous_view(cx),
            "Hide/show hidden edges" => {
                self.opts.hidden_edges_visible = !self.opts.hidden_edges_visible;
                let d = self.display.clone();
                self.apply_display(d, window, cx);
            }
            "Highlight boundary edges" => {
                self.opts.highlight_boundary = !self.opts.highlight_boundary;
                let d = self.display.clone();
                self.apply_display(d, window, cx);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn every_part_studio_and_sketch_action_runs_something() {
        // Part Studio: a feature type for each action without a GPUI action
        // (custom features have none in noshape yet).
        for e in shortcuts::entries(Tab::PartStudio).filter(|e| e.action.is_none()) {
            let known = FEATURES.iter().find(|(n, _)| *n == e.name).map(|(_, id)| *id);
            match e.name {
                "Add custom features" => assert!(known.is_none()),
                _ => assert!(known.is_some_and(|id| noshape_model::features::get(id).is_some()), "{}", e.name),
            }
        }
        for e in shortcuts::entries(Tab::Sketch) {
            if !matches!(e.name, "Constraint manager" | "Profile inspector" | "Line/tangent arc switch" | "Toggle construction") {
                assert!(sketch_tool(e.name).is_some(), "{}", e.name);
            }
        }
        assert_eq!(sketch_tool("Center point rectangle*"), Some(Tool::CenterRect));
        assert_eq!(sketch_tool("Midpoint"), Some(Tool::Constraint(crate::sketch_mode::ConstraintTool::Midpoint)));
    }
}
