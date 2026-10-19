//! Format painter (Onshape): copy the style, the tolerance and precision
//! and / or the content of one dimension or annotation onto others. Only
//! what both kinds have is painted.

use super::{Annotation, AnnotationKind};

/// What the Format painter copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintOptions {
    pub style: bool,
    pub tolerance_precision: bool,
    pub content: bool,
}

impl Default for PaintOptions {
    fn default() -> Self {
        Self { style: true, tolerance_precision: false, content: false }
    }
}

fn note_height(k: &AnnotationKind) -> Option<f64> {
    match k {
        AnnotationKind::Note { height_mm, .. } => Some(*height_mm),
        _ => None,
    }
}

/// Paint `src`'s formats onto `dst`.
pub fn paint(src: &Annotation, dst: &mut Annotation, o: PaintOptions) {
    let (sd, dd) = (src.kind.is_dimension(), dst.kind.is_dimension());
    if o.style {
        // Text style: note height, dimension units display.
        if let (Some(h), AnnotationKind::Note { height_mm, .. }) = (note_height(&src.kind), &mut dst.kind) {
            *height_mm = h;
        }
        if sd && dd {
            dst.format.units = src.format.units;
            dst.format.show_units = src.format.show_units;
        }
    }
    if o.tolerance_precision && sd && dd {
        dst.format.precision = src.format.precision;
        dst.format.tolerance = src.format.tolerance.clone();
    }
    if o.content {
        if sd && dd {
            dst.format.prefix = src.format.prefix.clone();
            dst.format.suffix = src.format.suffix.clone();
        }
        if let (AnnotationKind::Note { text: s, .. }, AnnotationKind::Note { text: d, .. }) = (&src.kind, &mut dst.kind) {
            *d = s.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::{DimFormat, Tolerance, ViewId};

    #[test]
    fn painter_copies_only_the_chosen_and_common_formats() {
        let dim = |p: Option<u8>| Annotation {
            kind: AnnotationKind::LinearDimension { view: ViewId(1), a: [0.0, 0.0], b: [1.0, 0.0], offset: 5.0, orientation: crate::drawing::DimOrientation::Horizontal, override_text: None },
            format: DimFormat { precision: p, prefix: "2X ".into(), tolerance: Tolerance::Symmetric { value: 0.0001 }, show_units: true, ..Default::default() },
            ..Default::default()
        };
        let src = dim(Some(3));
        let mut dst = dim(None);
        dst.format = DimFormat::default();
        paint(&src, &mut dst, PaintOptions::default());
        assert!(dst.format.show_units && dst.format.precision.is_none() && dst.format.prefix.is_empty());
        paint(&src, &mut dst, PaintOptions { style: false, tolerance_precision: true, content: false });
        assert_eq!((dst.format.precision, dst.format.tolerance.clone()), (Some(3), src.format.tolerance.clone()));
        paint(&src, &mut dst, PaintOptions { style: false, tolerance_precision: false, content: true });
        assert_eq!(dst.format.prefix, "2X ");
        // A note gets the note height from another note, nothing from a dimension.
        let note = |h: f64, t: &str| Annotation { kind: AnnotationKind::note(t, h), ..Default::default() };
        let mut n = note(3.5, "A");
        paint(&note(5.0, "B"), &mut n, PaintOptions { style: true, tolerance_precision: true, content: true });
        assert_eq!(n.kind, AnnotationKind::note("B", 5.0));
        let before = n.clone();
        paint(&src, &mut n, PaintOptions { style: true, tolerance_precision: true, content: true });
        assert_eq!(n, before);
    }
}
