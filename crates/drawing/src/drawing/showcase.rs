//! A demo document exercising the drawing features: three sheets with
//! hole and BOM tables, linked callouts, ordinate/chamfer-style dimensions
//! with tolerances, GD&T, datums, surface finish, jogged and aligned
//! sections, broken, cropped and rotated views, an exploded assembly view,
//! a general table and property fields; a fourth sheet with broken-out,
//! partial and angular sections, a spline crop, max/min and foreshortened
//! dimensions, datum targets, a formatted table with merged cells and an
//! ISO 7200 title block (sheet 2 uses an ANSI title block).

use crate::document::TabId;
use super::views::{self, SourceKey, regenerate_sources};
use super::*;
use crate::document::{Document, TabContent};

fn studio_and_assembly(doc: &Document) -> Option<(TabId, TabId)> {
    let studio = doc.tabs.iter().find(|t| matches!(t.content, TabContent::PartStudio(_)))?.id;
    let asm = doc.tabs.iter().find(|t| matches!(t.content, TabContent::Assembly(_)))?.id;
    Some((studio, asm))
}

/// The demo drawing document extended with every drawing feature.
pub fn showcase_document() -> Document {
    let mut doc = crate::demo::demo_document_with_drawing();
    let Some((studio, asm)) = studio_and_assembly(&doc) else { return doc };
    // An exploded view in the assembly.
    {
        use crate::assembly::eval::{AssemblyEvaluator, SolveOptions};
        let mut ev = AssemblyEvaluator::new();
        if let Some(TabContent::Assembly(a)) = doc.tab(asm).map(|t| t.content.clone()) {
            let r = ev.evaluate(&doc, &a, &SolveOptions::default(), None);
            let e = crate::assembly::analysis::auto_exploded_view(&r, "Exploded 1", 1.2);
            if let Some(TabContent::Assembly(a)) = doc.tabs.iter_mut().find(|t| t.id == asm).map(|t| &mut t.content) {
                a.exploded_views.push(e);
            }
        }
    }
    let Some(di) = doc.tabs.iter().position(|t| matches!(t.content, TabContent::Drawing(_))) else { return doc };
    let TabContent::Drawing(mut drawing) = doc.tabs[di].content.clone() else { return doc };

    // ---- sheet 1: tolerances, GD&T, hole table, ordinates.
    {
        let s = &mut drawing.sheets[0];
        if let Some(a) = s.annotations.get_mut(0) {
            a.format = DimFormat { tolerance: Tolerance::Deviation { upper: 0.0001, lower: -0.0001 }, ..Default::default() };
        }
        if let Some(a) = s.annotations.get_mut(2) {
            a.format = DimFormat { tolerance: Tolerance::Fit { hole: "H7".into(), shaft: String::new() }, ..Default::default() };
        }
        if let Some(a) = s.annotations.get_mut(5) {
            a.format = DimFormat { tolerance: Tolerance::Symmetric { value: 0.00005 }, precision: Some(2), ..Default::default() };
        }
        for (k, v) in [("Title", "{part.name} ({model.name})"), ("Sheet", "{sheet.number} / {sheet.count}"), ("Date", "{date}")] {
            if let Some(f) = s.title_block.fields.iter_mut().find(|(key, _)| key == k) {
                f.1 = v.to_string();
            }
        }
        let mut id = s.next_annotation_id();
        let mut add = |s: &mut Sheet, kind: AnnotationKind, position: [f64; 2]| {
            s.annotations.push(Annotation { id, kind, position, ..Default::default() });
            id += 1;
        };
        add(s, AnnotationKind::Gdt { anchor: [24.0, 262.0], frame: vec!["⌖".into(), "Ø0.1Ⓜ".into(), "A".into(), "B".into()], leaders: vec![Leader { view: Some(ViewId(2)), point: [0.038 - 0.0048 * std::f64::consts::FRAC_1_SQRT_2, 0.018 + 0.0048 * std::f64::consts::FRAC_1_SQRT_2], nodes: Vec::new(), ..Default::default() }], extra: Default::default() }, [24.0, 262.0]);
        add(s, AnnotationKind::Gdt { anchor: [24.0, 112.0], frame: vec!["▱".into(), "0.05".into()], leaders: vec![], extra: Default::default() }, [24.0, 112.0]);
        add(s, AnnotationKind::Datum { anchor: [150.0, 144.0], label: "A".into(), on: None, at: None }, [150.0, 126.0]);
        add(s, AnnotationKind::Datum { anchor: [160.0, 200.0], label: "B".into(), on: None, at: None }, [175.0, 200.0]);
        add(s, AnnotationKind::SurfaceFinish { anchor: [95.0, 156.2], value: "Ra 1.6".into(), leaders: Vec::new(), on: None, spec: Default::default() }, [95.0, 156.2]);
        // A grouped ordinate dimension along the bottom of the top view.
        let o = [-0.05, -0.03];
        add(s, AnnotationKind::OrdinateGroup { view: ViewId(2), origin: o, points: vec![o, [-0.038, -0.018], [-0.0332, -0.018], [0.0, 0.0], [0.038, 0.018], [0.05, 0.03]], horizontal: true }, [110.0, 168.0]);
        s.tables.push(Table { corner: TableCorner::BottomLeft, ..Table::new(1, TableKind::Hole { view: ViewId(2), origin: [-0.05, -0.03], origin_ref: None, precision: Some(1), flip_x: false, flip_y: false, show_location: true, bottom_to_top: false }, [150.0, 46.0]) });
        // Ordinates now carry the bottom-left location: move the front view's notes down.
        if let Some(n) = s.annotations.iter_mut().find(|a| matches!(a.kind, AnnotationKind::Note { .. })) {
            n.position = [25.0, 60.0];
        }
    }

    // ---- sheet 2: assembly, exploded view, BOM and linked callouts.
    let s2 = drawing.add_sheet(0);
    drawing.rename_sheet(s2, "Assembly");
    {
        let s = &mut drawing.sheets[s2];
        s.views.push(DrawingView { id: ViewId(1), kind: ViewKind::Base { source: asm, config: Default::default(), orientation: ViewOrientation::Isometric, part: None }, position: [110.0, 190.0], label: Some("ASSEMBLED".into()), scale: Some(1.5), ..Default::default() });
        s.views.push(DrawingView { id: ViewId(2), kind: ViewKind::Base { source: asm, config: Default::default(), orientation: ViewOrientation::Isometric, part: None }, position: [300.0, 150.0], label: Some("EXPLODED".into()), explode: Some(0), scale: Some(0.5), ..Default::default() });
        s.views.push(DrawingView { id: ViewId(3), kind: ViewKind::Base { source: asm, config: Default::default(), orientation: ViewOrientation::Front, part: None }, position: [110.0, 80.0], style: RenderStyle::HiddenLinesVisible, ..Default::default() });
        s.tables.push(Table::new(1, TableKind::Bom { view: ViewId(1), structured: false, bottom_to_top: false, overrides: Vec::new(), default_template: false }, [421.8, 269.4]));
        s.annotations.push(Annotation { id: 1, kind: AnnotationKind::note("{model.name}: {part.name}\nSHEET {sheet.number} OF {sheet.count}", 3.5), position: [25.0, 40.0], ..Default::default() });
    }


    // ---- sheet 3: broken, cropped and rotated views, aligned section, general table.
    let s3 = drawing.add_sheet(s2);
    drawing.rename_sheet(s3, "Details");
    {
        let s = &mut drawing.sheets[s3];
        let base = |id: u64, orientation: ViewOrientation, position: [f64; 2]| DrawingView { id: ViewId(id), kind: ViewKind::Base { source: studio, config: Default::default(), orientation, part: None }, position, ..Default::default() };
        let mut broken = base(1, ViewOrientation::Top, [90.0, 225.0]);
        broken.breaks.push(ViewBreak { vertical: true, a: -0.025, b: 0.022, gap_mm: 6.0, style: BreakStyle::ZigZag });
        broken.label = Some("BROKEN VIEW".into());
        broken.show_centerlines = true;
        s.views.push(broken);
        let mut cropped = base(2, ViewOrientation::Top, [220.0, 225.0]);
        cropped.crop = Some(Crop::Polygon(vec![[0.012, -0.03], [0.05, -0.03], [0.05, 0.03], [0.012, 0.03]]));
        cropped.label = Some("CROPPED VIEW".into());
        cropped.scale = Some(1.5);
        s.views.push(cropped);
        let mut rotated = base(3, ViewOrientation::Front, [345.0, 180.0]);
        rotated.rotation = std::f64::consts::FRAC_PI_2;
        rotated.label = Some("ROTATED 90°".into());
        rotated.style = RenderStyle::HiddenLinesVisible;
        rotated.show_centerlines = true;
        s.views.push(rotated);
        let top = base(4, ViewOrientation::Top, [90.0, 120.0]);
        s.views.push(top);
        s.views.push(DrawingView { id: ViewId(5), kind: ViewKind::AlignedSection { parent: ViewId(4), center: [0.0, 0.0], a: [0.0, -0.045], b: [0.0475, 0.0225], label: "C".into() }, position: [200.0, 120.0], ..Default::default() });
        s.views.push(DrawingView { id: ViewId(6), kind: ViewKind::Section { parent: ViewId(4), line: vec![[-0.06, -0.018], [-0.038, -0.018], [-0.038, 0.0], [0.06, 0.0]], label: "D".into(), cuts: vec![] }, position: [90.0, 50.0], ..Default::default() });
        let mut t = Table::new(1, TableKind::General { cells: vec![vec!["REV".into(), "DESCRIPTION".into(), "DATE".into()], vec!["A".into(), "Initial release of {part.name}".into(), "{date}".into()], vec!["B".into(), "Added hole table\nand BOM".into(), "{date}".into()]], title: Some("REVISIONS".into()), header: true }, [410.0, 287.0]);
        t.corner = TableCorner::TopRight;
        s.tables.push(t);
    }
    // ---- sheet 4: broken-out, partial and angular sections, spline crop,
    // max/min and foreshortened dimensions, datum targets, a formatted
    // table, ISO 7200 title block.
    let s4 = drawing.add_sheet(s3);
    drawing.rename_sheet(s4, "Sections");
    templates::set_title_style(&mut drawing, s4, TitleBlockStyle::Iso);
    for (k, v) in [("Legal owner", "noshape"), ("Drawn by", "noshape"), ("Approved by", "QA")] {
        if let Some(f) = drawing.sheets[s4].title_block.fields.iter_mut().find(|(key, _)| key == k) {
            f.1 = v.to_string();
        }
    }
    {
        let s = &mut drawing.sheets[s4];
        let base = |id: u64, orientation: ViewOrientation, position: [f64; 2]| DrawingView { id: ViewId(id), kind: ViewKind::Base { source: studio, config: Default::default(), orientation, part: None }, position, ..Default::default() };
        let mut front = base(1, ViewOrientation::Front, [110.0, 245.0]);
        front.scale = Some(1.5);
        front.label = Some("BROKEN-OUT SECTION".into());
        front.broken_out.push(BrokenOut { profile: vec![[-0.024, -0.003], [0.0, -0.006], [0.024, -0.003], [0.026, 0.016], [0.0, 0.019], [-0.026, 0.016]], spline: true, depth: 0.03, depth_ref: None, offset: 0.0, exclude: Vec::new() });
        s.views.push(front);
        s.views.push(base(2, ViewOrientation::Top, [85.0, 150.0]));
        s.views.push(DrawingView { id: ViewId(3), kind: ViewKind::Section { parent: ViewId(2), line: vec![[-0.025, -0.012], [-0.025, 0.012]], label: "E".into(), cuts: vec![] }, position: [175.0, 150.0], partial: true, label: Some("PARTIAL".into()), ..Default::default() });
        s.views.push(DrawingView { id: ViewId(4), kind: ViewKind::Section { parent: ViewId(2), line: vec![[-0.06, -0.045], [0.06, 0.045]], label: "F".into(), cuts: vec![] }, position: [125.0, 62.0], ..Default::default() });
        let mut cropped = base(5, ViewOrientation::Top, [262.0, 215.0]);
        cropped.crop = Some(Crop::Spline(vec![[0.012, -0.03], [0.05, -0.022], [0.05, 0.03], [0.03, 0.03], [0.02, 0.005]]));
        cropped.scale = Some(1.5);
        cropped.label = Some("SPLINE CROP".into());
        s.views.push(cropped);
        let mut id = 1;
        let mut add = |s: &mut Sheet, kind: AnnotationKind, position: [f64; 2]| {
            s.annotations.push(Annotation { id, kind, position, ..Default::default() });
            id += 1;
        };
        add(s, AnnotationKind::MaxMinDimension { view: ViewId(2), a: [0.0, 0.0], ra: 0.012, ca: ArcCondition::Min, b: [-0.05, -0.03], rb: 0.0, cb: ArcCondition::Center, offset: -8.0, orientation: DimOrientation::Horizontal }, [0.0; 2]);
        add(s, AnnotationKind::MaxMinDimension { view: ViewId(2), a: [0.0, 0.0], ra: 0.012, ca: ArcCondition::Max, b: [0.038, 0.018], rb: 0.0048, cb: ArcCondition::Max, offset: 6.0, orientation: DimOrientation::Horizontal }, [0.0; 2]);
        add(s, AnnotationKind::ForeshortenedDiameter { view: ViewId(1), axis: [[0.0, 0.0], [0.0, 0.012]], edge: [0.012, 0.006], direct: false }, [150.0, 262.0]);
        add(s, AnnotationKind::ForeshortenedRadial { view: ViewId(5), center: [0.038, 0.018], radius: 0.0048, angle: -0.8, jog: [0.028, -0.012] }, [0.0; 2]);
        add(s, AnnotationKind::DatumTarget { view: ViewId(2), anchor: [-0.04, 0.02], label: "A1".into(), area: DatumTargetArea::Point }, [28.0, 190.0]);
        add(s, AnnotationKind::DatumTarget { view: ViewId(2), anchor: [0.03, -0.02], label: "A2".into(), area: DatumTargetArea::Circle { diameter: 0.008 } }, [150.0, 100.0]);
        add(s, AnnotationKind::Gdt { anchor: [30.0, 270.0], frame: vec!["⟂".into(), "0.02".into(), "A".into()], leaders: vec![Leader { view: Some(ViewId(1)), point: [-0.05, 0.006], nodes: Vec::new(), ..Default::default() }], extra: Default::default() }, [30.0, 270.0]);
        let mut t = Table::new(1, TableKind::General {
            cells: vec![
                vec!["PROPERTY".into(), "".into(), "VALUE".into()],
                vec!["Material".into(), "".into(), "{part.material}".into()],
                vec!["Mass".into(), "".into(), "0.16 kg".into()],
                vec!["Finish".into(), "Anodized".into(), "Ra 1.6".into()],
            ],
            title: Some("PART DATA".into()),
            header: true,
        }, [250.0, 140.0]);
        t.corner = TableCorner::TopLeft;
        t.merge_cells((0, 0), (0, 1));
        t.merge_cells((1, 0), (1, 1));
        t.merge_cells((2, 0), (2, 1));
        for r in 1..4 {
            t.set_cell_format(r, 0, CellFormat { align: CellAlign::Left, bold: true, ..Default::default() });
            t.set_cell_format(r, 2, CellFormat { align: CellAlign::Right, ..Default::default() });
        }
        t.set_cell_format(0, 0, CellFormat { height: Some(3.5), bold: true, ..Default::default() });
        t.set_cell_format(0, 2, CellFormat { height: Some(3.5), bold: true, ..Default::default() });
        s.tables.push(t);
    }

    // ANSI B sheet with an ASME Y14.1 title block (the drawing stays metric).
    drawing.sheets[s2].size = SheetSize::AnsiB;
    templates::set_title_style(&mut drawing, s2, TitleBlockStyle::Ansi);
    for (k, v) in [("Company", "noshape"), ("Drawn by", "noshape"), ("Weight", "0.21 kg")] {
        if let Some(f) = drawing.sheets[s2].title_block.fields.iter_mut().find(|(key, _)| key == k) {
            f.1 = v.to_string();
        }
    }

    // Callouts on the assembly view, linked to the BOM (empty text).
    let sources = regenerate_sources(&doc, &drawing);
    let cache = views::ViewCache::new();
    let g = views::compute_view(&drawing, &drawing.sheets[s2], ViewId(1), &sources, &cache, false);
    if let Some(src) = sources.get(&SourceKey::studio(asm)) {
        let mut placed: Vec<usize> = Vec::new();
        let mut next = 2;
        let pl = super::build::placement_of(&drawing.sheets[s2], ViewId(1), &g);
        // Topmost vertex of each part (visible in the isometric view).
        let mut snaps = g.snaps.clone();
        snaps.sort_by(|a, b| b.p[1].total_cmp(&a.p[1]));
        for sn in &snaps {
            let Some(PointRef::Vertex { body, .. }) = &sn.point else { continue };
            let Some(item) = src.bom_item(body) else { continue };
            if placed.contains(&item) {
                continue;
            }
            placed.push(item);
            let at = pl.map(|p| p.to_sheet(sn.p)).unwrap_or([100.0, 100.0]);
            let off = if item % 2 == 1 { [-30.0, 18.0] } else { [35.0, 28.0] };
            let top = drawing.sheets[s2].size.size_mm().1 - 20.0;
            let pos = [(at[0] + off[0]).clamp(25.0, 400.0), (at[1] + off[1]).clamp(25.0, top)];
            drawing.sheets[s2].annotations.push(Annotation { id: next, kind: AnnotationKind::balloon(ViewId(1), sn.p, ""), position: pos, refs: vec![AnnotationRef { slot: 0, point: sn.point.clone().unwrap() }], ..Default::default() });
            next += 1;
        }
    }
    doc.tabs[di].content = TabContent::Drawing(drawing);
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::build::{SheetInfo, render_sheet_with};

    #[test]
    fn showcase_builds_every_sheet() {
        let doc = showcase_document();
        let drawing = doc.tabs.iter().find_map(|t| if let TabContent::Drawing(d) = &t.content { Some(d.clone()) } else { None }).unwrap();
        assert_eq!(drawing.sheets.len(), 4);
        let sources = regenerate_sources(&doc, &drawing);
        let cache = views::ViewCache::new();
        let info = SheetInfo { doc_name: "Showcase".into(), drawing_name: "Drawing 1".into(), ..Default::default() };
        let _ = std::fs::create_dir_all("/tmp/drw");
        for si in 0..drawing.sheets.len() {
            let b = render_sheet_with(&drawing, si, &sources, &cache, true, &info).unwrap();
            assert!(b.dangling.is_empty(), "sheet {si}: dangling {:?}", b.dangling);
            for v in &drawing.sheets[si].views {
                let g = views::compute_view(&drawing, &drawing.sheets[si], v.id, &sources, &cache, true);
                assert!(g.error.is_none(), "sheet {si} view {:?}: {:?}", v.id, g.error);
            }
            std::fs::write(format!("/tmp/drw/showcase{}.svg", si + 1), export::to_svg(&b.geometry)).unwrap();
            if si == 1 {
                let t: Vec<&str> = b.geometry.texts.iter().map(|t| t.text.as_str()).collect();
                assert!(t.contains(&"Quantity") && t.contains(&"1") && t.contains(&"2"), "{t:?}");
                assert!(drawing.sheets[1].annotations.iter().filter(|a| matches!(a.kind, AnnotationKind::Balloon { .. })).count() == 2);
            }
        }
        let pages: Vec<_> = (0..4).map(|si| render_sheet_with(&drawing, si, &sources, &cache, true, &info).unwrap().geometry).collect();
        let pdf = export::to_pdf_pages(&pages);
        assert!(String::from_utf8_lossy(&pdf).contains("/Count 4"));
        std::fs::write("/tmp/drw/showcase.pdf", pdf).unwrap();
    }
}
