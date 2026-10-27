//! The keyboard shortcuts registry lives in noshape-app-core (`shortcuts`), shared by the Part Studio, the
//! preferences and the workspace; what only the app does with it, running a drawing action, is here.

pub use noshape_app_core::shortcuts::*;
use gpui_kit::{App, Entity, Window};

/// Runs a drawing action (a [`RunShortcut`] reaching the workspace while
/// a drawing is active).
pub fn run_in_drawing(ed: &Entity<noshape_drawing_ui::drawing_view::DrawingEditor>, id: &str, window: &mut Window, cx: &mut App) {
    use noshape_drawing_ui::drawing_sketch::SkTool;
    use noshape_drawing_ui::drawing_view::{DimTool, TableTool, Tool};
    let Some(e) = entry(id) else { return };
    let dim = |kind| Some(Tool::Dimension { kind, picks: vec![] });
    let sk = |tool| Some(Tool::Sketch { tool, pts: vec![], from: None });
    let tool = match (e.tab, e.plain_name()) {
        (Tab::Sketch, "3 point circle") => sk(SkTool::Circle3),
        (Tab::Sketch, "Tangent arc") => sk(SkTool::TangentArc),
        (Tab::Drawing, name) => match name {
            "2-point centerline" => Some(Tool::Centerline { first: None }),
            "2-point circle centerline" => Some(Tool::CircleCenterline { three: false, points: vec![] }),
            "3-point circle centerline" => Some(Tool::CircleCenterline { three: true, points: vec![] }),
            "2-point linear dimension" | "Point to line dimension" | "Line to line dimension" => dim(DimTool::Linear),
            "3-point angular dimension" => dim(DimTool::Angle3),
            "Line to line angular dimension" => dim(DimTool::Angle),
            "Arc length dimension" => dim(DimTool::ArcLength),
            "Chamfer dimension" => dim(DimTool::Chamfer),
            "Aligned section view" => Some(Tool::AlignedSection { parent: None, center: None, a: None, b: None }),
            "Auxiliary view" => Some(Tool::Auxiliary { parent: None }),
            "BOM table" => Some(Tool::Table { kind: TableTool::Bom, view: None, origin: None }),
            "Hole table" => Some(Tool::Table { kind: TableTool::Hole, view: None, origin: None }),
            "Table" => Some(Tool::Table { kind: TableTool::General, view: None, origin: None }),
            "Break view" => Some(Tool::Break { parent: None, first: None, vertical: true }),
            "Broken-out section" => Some(Tool::BrokenOut { parent: None, points: vec![], done: false }),
            "Callout" => Some(Tool::Balloon { anchor: None }),
            "Centermark" => Some(Tool::Centermark),
            "Crop view" => Some(Tool::CropPoly { parent: None, points: vec![], spline: true }),
            "Datum" => Some(Tool::Datum { anchor: None }),
            "Detail view" => Some(Tool::Detail { parent: None, center: None, radius: None }),
            "Edge to edge centerline" => Some(Tool::EdgeCenterline { first: None }),
            "Geometric tolerance" => Some(Tool::Gdt),
            "Hole/thread callout" => Some(Tool::HoleCallout { circle: None }),
            "Ordinate dimension" => Some(Tool::Ordinate { origin: None, horizontal: None, group: None }),
            "Section view" => Some(Tool::Section { parent: None, p0: None, line: None, partial: false, jogs: vec![] }),
            "Sketch point" => sk(SkTool::Point),
            "Spline" => sk(SkTool::Spline),
            "Spline point" => sk(SkTool::SplinePoint),
            "Surface finish symbol" => Some(Tool::SurfaceFinish),
            "Virtual sharp" => Some(Tool::VirtualSharp { first: None }),
            _ => None,
        },
        _ => None,
    };
    if let Some(tool) = tool {
        ed.update(cx, |ed, cx| ed.set_tool(tool, cx));
        return;
    }
    match e.plain_name() {
        "Drawing properties panel" => ed.update(cx, |ed, cx| ed.open_drawing_props(cx)),
        "Format painter" => ed.update(cx, |ed, cx| ed.toggle_format_painter(cx)),
        "Hide inspection items" => ed.update(cx, |ed, cx| ed.toggle_hide_inspection(cx)),
        "Inspection item" => ed.update(cx, |ed, cx| ed.open_inspection(cx)),
        "Insert view" => ed.update(cx, |ed, cx| ed.open_insert_chooser(cx)),
        "Insert DXF or DWG" => ed.update(cx, |ed, cx| ed.insert_cad_file(window, cx)),
        _ => {}
    }
}
