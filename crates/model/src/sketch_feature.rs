//! Data stored by a Sketch feature (the sketch itself plus expression-driven
//! dimensions, links to external 3D geometry, text boxes and UI placement).

use crate::reference::Reference;
use noshape_sketch::{EntityId, Sketch};
use noshape_units::ExprValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SketchData {
    pub sketch: noshape_sketch::Sketch,
    /// Expression text of driving dimensions, keyed by constraint id.
    #[serde(default)]
    pub dimension_exprs: BTreeMap<u32, ExprValue>,
    /// Sketch entities that are projections of 3D geometry ("Use" / convert),
    /// keyed by entity id; re-projected on every regeneration.
    #[serde(default)]
    pub external_refs: BTreeMap<u32, Reference>,
    /// Sketch text boxes. The box is construction geometry in the solver
    /// sketch; the glyph outlines are regenerated from it on evaluation.
    #[serde(default)]
    pub texts: Vec<SketchText>,
    /// Dimension label positions in sketch-plane coordinates, by constraint id.
    #[serde(default)]
    pub dimension_labels: BTreeMap<u32, [f64; 2]>,
    /// Intersection curves (Intersection tool): the model face and the
    /// sketch entities created from its section with the sketch plane;
    /// recomputed on every regeneration.
    #[serde(default)]
    pub intersections: Vec<IntersectionLink>,
    /// Constraint glyphs moved by the user: offset in sketch-plane
    /// coordinates, keyed by `"<constraint>:<entity>"`.
    #[serde(default)]
    pub glyph_offsets: BTreeMap<String, [f64; 2]>,
    /// Tolerance and display precision of dimensions, by constraint id.
    #[serde(default)]
    pub dimension_tolerances: BTreeMap<u32, DimTolerance>,
    /// Sketch patterns (linear / circular), editable from their glyph.
    #[serde(default)]
    pub patterns: Vec<SketchPattern>,
    /// Inserted images (reference only: no geometry is created).
    #[serde(default)]
    pub images: Vec<SketchImage>,
}

/// Tolerance type of a dimension (Onshape dimension context dialog).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToleranceKind {
    #[default]
    None,
    Symmetrical,
    Deviation,
    Limits,
    Min,
    Max,
    Basic,
}

/// Tolerance and precision of one dimension. `upper` / `lower` are SI
/// deviations (both positive for Deviation: +upper / -lower).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DimTolerance {
    pub kind: ToleranceKind,
    #[serde(default)]
    pub upper: f64,
    #[serde(default)]
    pub lower: f64,
    /// Decimal places (`None`: workspace precision).
    #[serde(default)]
    pub precision: Option<u8>,
}

/// A sketch pattern: the seed entities, the instance entities (per
/// instance, in seed order; deleted instances simply vanish) and its
/// parameters, so it can be edited later (Onshape "Edit pattern").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchPattern {
    pub id: u32,
    /// `true`: circular (center, total angle), `false`: linear (direction, spacing).
    pub circular: bool,
    pub seeds: Vec<u32>,
    pub instances: Vec<Vec<u32>>,
    pub count: usize,
    /// Linear: spacing and direction angle; circular: total angle.
    pub distance: f64,
    pub angle: f64,
    pub center: [f64; 2],
    /// Independent second direction of a linear sketch pattern. Old files
    /// remain a single row.
    #[serde(default = "one_pattern_row")]
    pub second_count: usize,
    #[serde(default)]
    pub second_distance: f64,
    #[serde(default)]
    pub second_angle: f64,
}

fn one_pattern_row() -> usize { 1 }

/// An inserted image: an encoded image file drawn in a box whose corners
/// are solver points (lower-left, lower-right, upper-right, upper-left).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchImage {
    pub id: u32,
    pub name: String,
    /// PNG / JPEG / GIF / BMP bytes, base64 encoded (shared, so the
    /// regenerated display and the GPU texture cache can key on it).
    pub data_base64: std::sync::Arc<str>,
    pub box_points: Vec<u32>,
    pub box_lines: Vec<u32>,
    /// Aspect-ratio constraint keeping the image proportions.
    #[serde(default)]
    pub ratio_constraint: Option<u32>,
}

/// A sketch text box (Onshape `skText`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchText {
    pub id: u32,
    pub text: String,
    /// Font family (see [`noshape_sketch::text::FONTS`]).
    pub font: String,
    pub bold: bool,
    pub italic: bool,
    /// Corner point entity ids (lower-left, upper-right) in the solver sketch.
    pub corners: [u32; 2],
    pub construction: bool,
    /// Box corners: lower-left, lower-right, upper-right, upper-left.
    #[serde(default)]
    pub box_points: Vec<u32>,
    /// Box construction lines: bottom, right, top, left.
    #[serde(default)]
    pub box_lines: Vec<u32>,
    /// Aspect-ratio constraint keeping the box proportional to the text.
    #[serde(default)]
    pub ratio_constraint: Option<u32>,
    /// Flip about the box's horizontal center.
    #[serde(default)]
    pub mirror_horizontal: bool,
    /// Flip about the box's vertical center.
    #[serde(default)]
    pub mirror_vertical: bool,
    /// `text` is an expression (Convert to expression): string literals,
    /// variables and `~` concatenation, evaluated on regeneration.
    #[serde(default)]
    pub expression: bool,
}

impl SketchText {
    fn layout(&self) -> noshape_sketch::text::TextLayout {
        noshape_sketch::text::layout_text(&self.text, &self.font, self.bold, self.italic)
    }

    fn corner_ids(&self) -> Vec<EntityId> {
        self.box_points.iter().map(|p| EntityId(*p)).collect()
    }

    /// Box entities (points and lines) in the solver sketch.
    pub fn box_entities(&self) -> Vec<EntityId> {
        self.box_points.iter().chain(self.box_lines.iter()).map(|e| EntityId(*e)).collect()
    }
}

/// Section curves of a model face with the sketch plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntersectionLink {
    pub face: Reference,
    /// Created sketch entities, in section order.
    pub entities: Vec<u32>,
}

impl PartialEq for SketchData {
    fn eq(&self, other: &Self) -> bool {
        // The solver sketch has no structural equality; compare serialized forms.
        self.dimension_exprs == other.dimension_exprs
            && self.external_refs == other.external_refs
            && self.texts == other.texts
            && self.dimension_labels == other.dimension_labels
            && self.intersections == other.intersections
            && self.glyph_offsets == other.glyph_offsets
            && self.dimension_tolerances == other.dimension_tolerances
            && self.patterns == other.patterns
            && self.images == other.images
            && serde_json::to_value(&self.sketch).ok() == serde_json::to_value(&other.sketch).ok()
    }
}

impl SketchData {
    /// Adds a text box with its lower-left corner (first baseline start) at
    /// `origin` and the given height (baseline to ascender). Returns the
    /// text id.
    pub fn add_text(&mut self, text: &str, font: &str, bold: bool, italic: bool, origin: glam::DVec2, height: f64) -> Result<u32, String> {
        if text.trim().is_empty() {
            return Err("Sketch text cannot be empty".into());
        }
        let layout = noshape_sketch::text::layout_text(text, font, bold, italic);
        let tb = self.sketch.add_text_box(origin, height, layout.width).map_err(|e| e.to_string())?;
        let id = self.texts.iter().map(|t| t.id + 1).max().unwrap_or(1);
        self.texts.push(SketchText {
            id,
            text: text.to_string(),
            font: font.to_string(),
            bold,
            italic,
            corners: [tb.points[0].0, tb.points[2].0],
            construction: false,
            box_points: tb.points.iter().map(|p| p.0).collect(),
            box_lines: tb.lines.iter().map(|l| l.0).collect(),
            ratio_constraint: tb.ratio.map(|c| c.0),
            mirror_horizontal: false,
            mirror_vertical: false,
            expression: false,
        });
        Ok(id)
    }

    /// Edits a text box's content/style; the box keeps its height and the
    /// width follows the new text.
    pub fn update_text(&mut self, id: u32, f: impl FnOnce(&mut SketchText)) -> Result<(), String> {
        let Some(t) = self.texts.iter_mut().find(|t| t.id == id) else { return Err("The text no longer exists".into()) };
        let mut nt = t.clone();
        f(&mut nt);
        if nt.text.trim().is_empty() {
            return Err("Sketch text cannot be empty".into());
        }
        let width = nt.layout().width;
        *t = nt;
        let t = t.clone();
        if let Some(c) = t.ratio_constraint
            && width > 1e-6
        {
            self.sketch.set_length_ratio(noshape_sketch::ConstraintId(c), width).map_err(|e| e.to_string())?;
            // Keep the text height: move the right edge so the new ratio holds
            // before solving (the solver would otherwise split the change).
            if let (Some(pl), [_, p1, p2, _]) = (self.sketch.text_placement(&t.corner_ids(), false, false), t.box_points.as_slice()) {
                let r = pl.origin + pl.x * pl.height * width;
                let _ = self.sketch.set_point(EntityId(*p1), r);
                let _ = self.sketch.set_point(EntityId(*p2), r + pl.y * pl.height);
            }
        }
        Ok(())
    }

    /// Text box owning an entity: one of its box points/lines or a derived
    /// glyph outline entity.
    pub fn text_of_entity(&self, e: EntityId) -> Option<u32> {
        if let Some(t) = noshape_sketch::text::glyph_text_id(e) {
            return self.texts.iter().find(|x| noshape_sketch::text::glyph_id_base(x.id) == noshape_sketch::text::glyph_id_base(t)).map(|x| x.id);
        }
        self.texts.iter().find(|t| t.box_points.contains(&e.0) || t.box_lines.contains(&e.0)).map(|t| t.id)
    }

    /// Drops texts whose box was deleted (and the rest of their box).
    pub fn prune_texts(&mut self) {
        let sk = &self.sketch;
        let (keep, gone): (Vec<SketchText>, Vec<SketchText>) = std::mem::take(&mut self.texts).into_iter().partition(|t| t.box_entities().iter().all(|e| sk.entity(*e).is_some()));
        self.texts = keep;
        for t in gone {
            let ids: Vec<EntityId> = t.box_entities().into_iter().filter(|e| self.sketch.entity(*e).is_some()).collect();
            self.sketch.remove_entities(&ids);
        }
    }

    /// Inserts an image in a box with its lower-left corner at `origin`, the
    /// given height and the image's aspect ratio (kept by a ratio
    /// constraint; the bottom edge is horizontal). Returns the image id.
    pub fn add_image(&mut self, name: &str, bytes: &[u8], origin: glam::DVec2, height: f64, aspect: f64) -> Result<u32, String> {
        if bytes.is_empty() {
            return Err("The image file is empty".into());
        }
        let tb = self.sketch.add_text_box(origin, height, aspect).map_err(|_| "The image rectangle could not be created with the selected points.".to_string())?;
        let id = self.images.iter().map(|t| t.id + 1).max().unwrap_or(1);
        self.images.push(SketchImage {
            id,
            name: name.to_string(),
            data_base64: base64_encode(bytes).into(),
            box_points: tb.points.iter().map(|p| p.0).collect(),
            box_lines: tb.lines.iter().map(|l| l.0).collect(),
            ratio_constraint: tb.ratio.map(|c| c.0),
        });
        Ok(id)
    }

    /// The image whose box contains entity `e`.
    pub fn image_of_entity(&self, e: EntityId) -> Option<u32> {
        self.images.iter().find(|i| i.box_points.contains(&e.0) || i.box_lines.contains(&e.0)).map(|i| i.id)
    }

    /// Removes images whose box was (partly) deleted, with the rest of their box.
    pub fn prune_images(&mut self) {
        let sk = &self.sketch;
        let (keep, gone): (Vec<SketchImage>, Vec<SketchImage>) = std::mem::take(&mut self.images).into_iter().partition(|i| i.box_points.iter().chain(&i.box_lines).all(|e| sk.entity(EntityId(*e)).is_some()));
        self.images = keep;
        for i in gone {
            let ids: Vec<EntityId> = i.box_points.iter().chain(&i.box_lines).map(|e| EntityId(*e)).filter(|e| self.sketch.entity(*e).is_some()).collect();
            self.sketch.remove_entities(&ids);
        }
    }

    /// Adds every text's glyph outlines to `sk` (a solved copy of the
    /// sketch) as fixed curves with stable ids.
    pub fn add_text_outlines(&self, sk: &mut Sketch) -> usize {
        let mut n = 0;
        for t in &self.texts {
            let Some(place) = sk.text_placement(&t.corner_ids(), t.mirror_horizontal, t.mirror_vertical) else { continue };
            let layout = t.layout();
            n += sk.add_text_outlines(&layout, &place, Some(noshape_sketch::text::glyph_id_base(t.id))).len();
        }
        n
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 (with padding) of `bytes`.
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(B64[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decodes standard base64 (padding optional, whitespace ignored).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for ch in text.bytes() {
        let v = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        } as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod image_tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for data in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar", &[0u8, 255, 128, 7]] {
            let e = base64_encode(data);
            assert_eq!(base64_decode(&e).unwrap(), data);
        }
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
    }

    #[test]
    fn images_follow_their_box() {
        let mut d = SketchData::default();
        let id = d.add_image("a.png", &[1, 2, 3], glam::DVec2::ZERO, 0.02, 2.0).unwrap();
        let img = d.images[0].clone();
        assert_eq!(img.id, id);
        assert_eq!(d.image_of_entity(EntityId(img.box_lines[0])), Some(id));
        d.sketch.solve();
        let bb = d.sketch.bbox();
        assert!((bb.size().x - 0.04).abs() < 1e-9 && (bb.size().y - 0.02).abs() < 1e-9);
        d.sketch.remove_entities(&[EntityId(img.box_lines[1])]);
        d.prune_images();
        assert!(d.images.is_empty());
        assert_eq!(d.sketch.entity_count(), 0);
    }
}
