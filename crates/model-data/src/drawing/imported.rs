//! Drawings made from an imported DWG / DXF file tab (Onshape: the file
//! tab's "Create Drawing of <file>…"; live behaviours.md "Imported DWG/DXF
//! drawings"): the first sheet is the file's model space, named "Model",
//! then one sheet per paper-space layout. Entities keep their display colours
//! (black is the foreground, ACI 7: white on the dark model-space background)
//! and texts.

use super::{Drawing, Sheet, SheetSize};
use noshape_sketch::cad::{CadSpace, parse_cad_spaces};
use noshape_sketch::dxf::DxfEntity;
use serde::{Deserialize, Serialize};

/// What an imported sheet shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportedSpace {
    /// Model space: no paper, the whole canvas in the model-space colour
    /// (Preferences › Drawings).
    pub model: bool,
    /// Polylines (sheet mm) with their colour.
    pub lines: Vec<(Vec<[f64; 2]>, [u8; 3])>,
    pub texts: Vec<ImportedText>,
    /// A paper space that can't be opened (R12 files): listed in grey.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportedText {
    pub pos: [f64; 2],
    pub height: f64,
    pub angle: f64,
    pub text: String,
    pub color: [u8; 3],
}

impl ImportedSpace {
    /// The bounding box of the geometry (min x, min y, max x, max y).
    pub fn extents(&self) -> Option<[f64; 4]> {
        let pts = self.lines.iter().flat_map(|l| l.0.iter().copied()).chain(self.texts.iter().map(|t| t.pos));
        pts.fold(None, |b: Option<[f64; 4]>, p| Some(match b {
            None => [p[0], p[1], p[0], p[1]],
            Some(b) => [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])],
        }))
    }
}

fn space_of(s: &CadSpace, mm: f64) -> ImportedSpace {
    let rgb = |c: (u8, u8, u8)| [c.0, c.1, c.2];
    let mut out = ImportedSpace { model: s.model, ..Default::default() };
    for (e, c) in &s.entities {
        match e {
            DxfEntity::Curve(curve) => {
                let pts: Vec<[f64; 2]> = curve.tessellate(0.02 / mm.max(1e-12)).into_iter().map(|p| [p.x * mm, p.y * mm]).collect();
                if pts.len() >= 2 {
                    out.lines.push((pts, rgb(*c)));
                }
            }
            DxfEntity::Point(p) => {
                // A point: a small cross, as CAD viewers draw it.
                let (x, y, r) = (p.x * mm, p.y * mm, 0.5);
                out.lines.push((vec![[x - r, y], [x + r, y]], rgb(*c)));
                out.lines.push((vec![[x, y - r], [x, y + r]], rgb(*c)));
            }
        }
    }
    out.texts = s.texts.iter().map(|t| ImportedText { pos: [t.position.x * mm, t.position.y * mm], height: t.height * mm, angle: t.rotation, text: t.text.clone(), color: rgb(t.color) }).collect();
    out
}

/// A drawing of a DWG / DXF file: "Model" first, then the layouts; `r12`
/// files' paper space can't be opened. It opens on the first layout that has
/// geometry, else on Model (the returned sheet index).
pub fn drawing_from_cad(bytes: &[u8]) -> Result<(Drawing, usize), String> {
    let (spaces, unit_scale) = parse_cad_spaces(bytes).map_err(|e| e.to_string())?;
    let mm = unit_scale.map(|m| m * 1000.0).unwrap_or(1.0);
    let r12 = bytes.windows(6).take(4096).any(|w| w == b"AC1009");
    let mut drawing = Drawing { sheets: Vec::new(), ..Drawing::default() };
    for s in &spaces {
        let mut space = space_of(s, mm);
        if !s.model && space.lines.is_empty() && space.texts.is_empty() {
            continue;
        }
        space.locked = !s.model && r12;
        let name = if s.model { "Model".to_string() } else if s.name.is_empty() { format!("Sheet {}", drawing.sheets.len()) } else { s.name.clone() };
        // Layouts: paper the size of their content (with a margin).
        let size = match space.extents() {
            Some(b) if !s.model => SheetSize::Custom { width_mm: (b[2].max(0.0) + 10.0).max(100.0), height_mm: (b[3].max(0.0) + 10.0).max(100.0) },
            _ => SheetSize::Custom { width_mm: 279.4, height_mm: 215.9 },
        };
        drawing.sheets.push(Sheet { name, size, imported: Some(space), ..Sheet::default() });
    }
    if drawing.sheets.is_empty() {
        return Err("the file contains no supported 2D geometry".into());
    }
    let open = drawing.sheets.iter().position(|s| s.imported.as_ref().is_some_and(|i| !i.model && !i.locked)).unwrap_or(0);
    drawing.active_sheet = open;
    Ok((drawing, open))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_space_sheet_keeps_colours() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n62\n1\n10\n0\n20\n0\n11\n10\n21\n0\n0\nCIRCLE\n8\n0\n62\n7\n10\n5\n20\n5\n40\n2\n0\nTEXT\n8\n0\n10\n1\n20\n8\n40\n2.5\n1\nHELLO\n0\nENDSEC\n0\nEOF\n";
        let (d, open) = drawing_from_cad(dxf.as_bytes()).unwrap();
        assert_eq!(open, 0);
        let s = &d.sheets[0];
        assert_eq!(s.name, "Model");
        let m = s.imported.as_ref().unwrap();
        assert!(m.model && m.lines.iter().any(|l| l.1 == [255, 0, 0]) && m.lines.iter().any(|l| l.1 == [0, 0, 0]));
        assert!(m.texts.iter().any(|t| t.text == "HELLO"));
    }
}
