//! drawing_styles: the Styles panel on selected annotations.

use super::doc_field;
use super::drawing::{commit, drawing_field, reply, sheet_field};
use super::drawing_tools::target;
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::Ctx;
use noshape_model::drawing::sheet::{ArrowType, FONT_FAMILIES, TextFont, is_shx};
use noshape_model::drawing::{AnnStyle, Annotation, AnnotationKind, ChamferContent, StackDirection, TextJustify};
use serde_json::{Value, json};

fn key(name: &str) -> String {
    name.to_lowercase().replace([' ', '-'], "_")
}

fn keys<T: Copy>(all: &[T], name: fn(T) -> &'static str) -> Vec<String> {
    all.iter().map(|x| key(name(*x))).collect()
}

fn pick<T: Copy>(all: &[T], name: fn(T) -> &'static str, field: &str, v: &str) -> Result<T, OpError> {
    all.iter().copied().find(|x| key(name(*x)) == v).ok_or_else(|| OpError::param(field, &format!("one of {}", keys(all, name).join(", "))))
}

pub(super) fn ops() -> Vec<Op> {
    let nullable = |t: &str| json!({ "type": [t, "null"] });
    vec![Op {
        group: "drawing",
        verb: "styles",
        summary: "Styles panel: an annotation's own font, height, color, arrowheads and dimension text",
        description: "The Styles panel with `annotations` selected, one edit \"Style\". A value sets the selection's own style; null puts the drawing's back (\"(Drawing)\"); `revert`: Revert to drawing properties (all of them). All annotations: `font`, `bold`, `italic`, `height` (mm), `color` (hex RRGGBB), `arrow` (arrowhead size, mm), `arrow_type`. Dimensions only: `justify`, `alignment` (horizontal | with_line), `text_gap`, `geometry_gap`, `extension` (mm); linear dimensions: `show_first_extension`, `show_second_extension`; chamfer dimensions: `chamfer_content`. Datums: `datum_size`. Callouts: `stack`, `stack_gap`. A field the selection's panel doesn't have is refused. drawing_get lists each annotation's own `annotation_style`.",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            req("annotations", json!({ "type": "array", "items": { "type": "integer" }, "minItems": 1 }), "The selected annotations (ids, drawing_get)."),
            opt("revert", json!({ "type": "boolean", "default": false }), "Revert to drawing properties."),
            opt("font", json!({ "type": ["string", "null"], "enum": FONT_FAMILIES.iter().map(|f| json!(f)).chain([Value::Null]).collect::<Vec<_>>() }), "Font."),
            opt("bold", json!({ "type": "boolean" }), "Bold (not with an SHX font)."),
            opt("italic", json!({ "type": "boolean" }), "Italic (not with an SHX font)."),
            opt("height", nullable("number"), "Text height (mm)."),
            opt("color", nullable("string"), "Color: hex RRGGBB."),
            opt("arrow", nullable("number"), "Arrowhead size (mm)."),
            opt("arrow_type", json!({ "type": ["string", "null"], "enum": keys(&ArrowType::ALL, ArrowType::name).into_iter().map(Value::from).chain([Value::Null]).collect::<Vec<_>>() }), "Arrowhead type."),
            opt("justify", json!({ "type": ["string", "null"], "enum": keys(&TextJustify::ALL, TextJustify::name).into_iter().map(Value::from).chain([Value::Null]).collect::<Vec<_>>() }), "Dimensions: Text justification."),
            opt("alignment", json!({ "type": ["string", "null"], "enum": ["horizontal", "with_line", null] }), "Dimensions: Text alignment (Horizontal / Align with dimension line)."),
            opt("text_gap", nullable("number"), "Dimensions: Text gap (mm)."),
            opt("geometry_gap", nullable("number"), "Dimensions: Geometry gap (mm)."),
            opt("extension", nullable("number"), "Dimensions: Extension past line (mm)."),
            opt("show_first_extension", json!({ "type": "boolean" }), "Linear dimensions: Show first extension line."),
            opt("show_second_extension", json!({ "type": "boolean" }), "Linear dimensions: Show second extension line."),
            opt("chamfer_content", json!({ "type": ["string", "null"], "enum": keys(&ChamferContent::ALL, ChamferContent::name).into_iter().map(Value::from).chain([Value::Null]).collect::<Vec<_>>() }), "Chamfer dimensions: content."),
            opt("datum_size", nullable("number"), "Datums: Datum size (mm)."),
            opt("stack", json!({ "type": ["string", "null"], "enum": keys(&StackDirection::ALL, StackDirection::name).into_iter().map(Value::from).chain([Value::Null]).collect::<Vec<_>>() }), "Callouts: Stack direction."),
            opt("stack_gap", nullable("number"), "Callouts: Stack gap size (mm)."),
        ],
        mutates: true,
        destructive: false,
        run: styles,
    }, sketch_styles_op(), hatch_styles_op()]
}

/// Which annotations a field's row is shown for.
fn row_for(field: &str, k: &AnnotationKind) -> bool {
    match field {
        "justify" | "alignment" | "text_gap" | "geometry_gap" | "extension" => k.is_dimension(),
        "show_first_extension" | "show_second_extension" => matches!(k, AnnotationKind::LinearDimension { .. }),
        "chamfer_content" => matches!(k, AnnotationKind::ChamferDimension { .. }),
        "datum_size" => matches!(k, AnnotationKind::Datum { .. }),
        "stack" | "stack_gap" => matches!(k, AnnotationKind::Balloon { .. }),
        _ => true,
    }
}

const FIELDS: [&str; 20] = ["font", "bold", "italic", "height", "color", "arrow", "arrow_type", "justify", "alignment", "text_gap", "geometry_gap", "extension", "show_first_extension", "show_second_extension", "chamfer_content", "datum_size", "stack", "stack_gap", "revert", "annotations"];

/// A length field: Some(None) for null (the drawing's), Some(Some(mm)) for a value > 0.
fn length(a: &Args, k: &str) -> Result<Option<Option<f64>>, OpError> {
    match a.value.get(k) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => v.as_f64().filter(|x| *x > 0.0).map(|x| Some(Some(x))).ok_or_else(|| OpError::param(k, "a length > 0 (mm) or null")),
    }
}

fn choice<T: Copy>(a: &Args, k: &str, all: &[T], name: fn(T) -> &'static str) -> Result<Option<Option<T>>, OpError> {
    match a.value.get(k) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => Ok(Some(Some(pick(all, name, k, v.as_str().unwrap_or(""))?))),
    }
}

/// `color`: Some(None) for null (the drawing's), Some(Some(rgb)) for hex RRGGBB.
fn color(a: &Args) -> Result<Option<Option<[u8; 3]>>, OpError> {
    match a.value.get("color") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => {
            let hex = v.as_str().unwrap_or("").trim().trim_start_matches('#');
            let n = (hex.len() == 6).then(|| u32::from_str_radix(hex, 16).ok()).flatten().ok_or_else(|| OpError::param("color", "a hex color RRGGBB, or null for the drawing's"))?;
            Ok(Some(Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])))
        }
    }
}

fn apply(a: &Args, x: &mut Annotation) -> Result<(), OpError> {
    if a.bool_or("revert", false)? {
        x.style = None;
    }
    let st: &mut AnnStyle = x.style.get_or_insert_with(Default::default);
    match a.value.get("font") {
        None => {}
        Some(Value::Null) => st.font = None,
        Some(v) => {
            let fam = v.as_str().and_then(|f| FONT_FAMILIES.iter().find(|x| **x == f)).ok_or_else(|| OpError::param("font", "not a font of the Font list"))?;
            let (b, i) = st.font.as_ref().map(|f| (f.bold, f.italic)).filter(|_| !is_shx(fam)).unwrap_or((false, false));
            st.font = Some(TextFont { family: (*fam).into(), bold: b, italic: i });
        }
    }
    for k in ["bold", "italic"] {
        let Some(v) = a.get(k).and_then(Value::as_bool) else { continue };
        let f = st.font.as_mut().ok_or_else(|| OpError::param(k, "pick a font first"))?;
        if is_shx(&f.family) {
            return Err(OpError::param(k, "SHX fonts have no bold or italic"));
        }
        if k == "bold" {
            f.bold = v;
        } else {
            f.italic = v;
        }
    }
    if let Some(v) = length(a, "height")? {
        st.height = v;
    }
    if let Some(c) = color(a)? {
        st.color = c;
    }
    if let Some(v) = length(a, "arrow")? {
        st.arrow = v;
    }
    if let Some(v) = choice(a, "arrow_type", &ArrowType::ALL, ArrowType::name)? {
        st.arrow_type = v;
    }
    if let Some(v) = choice(a, "justify", &TextJustify::ALL, TextJustify::name)? {
        st.justify = v;
    }
    match a.value.get("alignment") {
        None => {}
        Some(Value::Null) => st.horizontal = None,
        Some(v) => st.horizontal = Some(match v.as_str() {
            Some("horizontal") => true,
            Some("with_line") => false,
            _ => return Err(OpError::param("alignment", "horizontal or with_line")),
        }),
    }
    for (k, slot) in [("text_gap", &mut st.text_gap), ("geometry_gap", &mut st.geometry_gap), ("extension", &mut st.extension), ("datum_size", &mut st.datum_size), ("stack_gap", &mut st.stack_gap)] {
        if let Some(v) = length(a, k)? {
            *slot = v;
        }
    }
    for (i, k) in ["show_first_extension", "show_second_extension"].into_iter().enumerate() {
        if let Some(v) = a.get(k).and_then(Value::as_bool) {
            st.hide_ext[i] = !v;
        }
    }
    if let Some(v) = choice(a, "chamfer_content", &ChamferContent::ALL, ChamferContent::name)? {
        st.chamfer_content = v;
    }
    if let Some(v) = choice(a, "stack", &StackDirection::ALL, StackDirection::name)? {
        st.stack = v;
    }
    if x.style.as_ref().is_some_and(|s| s.is_empty()) {
        x.style = None;
    }
    Ok(())
}

fn styles(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let si = t.sheet;
    let mut ids = Vec::new();
    for (i, v) in a.array("annotations")?.iter().enumerate() {
        let id = v.as_u64().filter(|id| t.d.sheets[si].annotations.iter().any(|x| x.id == *id)).ok_or_else(|| OpError::param(&format!("annotations[{i}]"), "an annotation id on the sheet"))?;
        ids.push(id);
    }
    if let Some(k) = a.value.as_object().and_then(|o| o.keys().find(|k| !FIELDS.contains(&k.as_str()) && !["doc", "drawing", "sheet"].contains(&k.as_str())).cloned()) {
        return Err(OpError::param(&k, "not a Styles panel field"));
    }
    for k in FIELDS {
        if a.value.get(k).is_none() {
            continue;
        }
        if let Some(x) = t.d.sheets[si].annotations.iter().find(|x| ids.contains(&x.id) && !row_for(k, &x.kind)) {
            return Err(OpError::param(k, &format!("the Styles panel has no such row for a {}", super::drawing::variant_name(&x.kind))));
        }
    }
    for x in t.d.sheets[si].annotations.iter_mut().filter(|x| ids.contains(&x.id)) {
        apply(a, x)?;
    }
    commit(od, t.tab, si, t.d, "Style")?;
    let mut out = reply(od, t.tab, si, false)?;
    out["annotations"] = json!(ids);
    Ok(out.into())
}

/// An annotation's own style for drawing_get (what the Styles panel shows without "(Drawing)").
pub(super) fn style_json(st: &AnnStyle) -> Value {
    let mut j = json!({});
    if let Some(f) = &st.font {
        j["font"] = json!(f.family);
        if f.bold {
            j["bold"] = json!(true);
        }
        if f.italic {
            j["italic"] = json!(true);
        }
    }
    let num = |j: &mut Value, k: &str, v: Option<f64>| {
        if let Some(v) = v {
            j[k] = json!(v);
        }
    };
    num(&mut j, "height", st.height);
    if let Some([r, g, b]) = st.color {
        j["color"] = json!(format!("{r:02X}{g:02X}{b:02X}"));
    }
    num(&mut j, "arrow", st.arrow);
    if let Some(v) = st.arrow_type {
        j["arrow_type"] = json!(key(v.name()));
    }
    if let Some(v) = st.justify {
        j["justify"] = json!(key(v.name()));
    }
    if let Some(h) = st.horizontal {
        j["alignment"] = json!(if h { "horizontal" } else { "with_line" });
    }
    num(&mut j, "text_gap", st.text_gap);
    num(&mut j, "geometry_gap", st.geometry_gap);
    num(&mut j, "extension", st.extension);
    num(&mut j, "datum_size", st.datum_size);
    num(&mut j, "stack_gap", st.stack_gap);
    if st.hide_ext[0] {
        j["show_first_extension"] = json!(false);
    }
    if st.hide_ext[1] {
        j["show_second_extension"] = json!(false);
    }
    if let Some(v) = st.chamfer_content {
        j["chamfer_content"] = json!(key(v.name()));
    }
    if let Some(v) = st.stack {
        j["stack"] = json!(key(v.name()));
    }
    j
}

const PITCHES: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0];

fn sketch_styles_op() -> Op {
    use noshape_model::drawing::sheet_sketch::SketchLine;
    Op {
        group: "drawing",
        verb: "sketch_styles",
        summary: "Styles panel: sheet sketch entities' linestyle, pitch scale, thickness and color",
        description: "The Styles panel with sheet sketch entities selected (ids: drawing_get `sketch`), one edit \"Style\". `line`: Linestyle; `pitch`: Pitch scale (not for Solid lines; null: 1.00); `thickness` (mm, one of the list; null: the drawing's); `color` (hex RRGGBB; null: the drawing's); `revert`: Revert to drawing properties (Linestyle Solid, the drawing's color).",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            req("sketch_entities", json!({ "type": "array", "items": { "type": "integer" }, "minItems": 1 }), "The selected sketch entity ids."),
            opt("revert", json!({ "type": "boolean", "default": false }), "Revert to drawing properties."),
            opt("line", json!({ "type": "string", "enum": SketchLine::ALL.iter().map(|l| key(l.name())).collect::<Vec<_>>() }), "Linestyle."),
            opt("pitch", json!({ "type": ["number", "null"], "enum": PITCHES.iter().map(|p| json!(p)).chain([Value::Null]).collect::<Vec<_>>() }), "Pitch scale."),
            opt("thickness", json!({ "type": ["number", "null"], "enum": noshape_model::drawing::LINE_THICKNESSES.iter().map(|t| json!(t)).chain([Value::Null]).collect::<Vec<_>>() }), "Thickness (mm)."),
            opt("color", json!({ "type": ["string", "null"] }), "Color: hex RRGGBB."),
        ],
        mutates: true,
        destructive: false,
        run: sketch_styles,
    }
}

fn sketch_styles(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::drawing::sheet_sketch::SketchLine;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let si = t.sheet;
    let sk = &mut t.d.sheets[si].sketch;
    let mut ids = Vec::new();
    for (i, v) in a.array("sketch_entities")?.iter().enumerate() {
        let id = v.as_u64().and_then(|x| u32::try_from(x).ok()).filter(|x| sk.sketch.entity(noshape_sketch::EntityId(*x)).is_some_and(|e| !e.reference_only));
        ids.push(id.ok_or_else(|| OpError::param(&format!("sketch_entities[{i}]"), "a sheet sketch entity id (drawing_get `sketch`)"))?);
    }
    let line = match a.opt_str("line")? {
        Some(l) => Some(pick(&SketchLine::ALL, SketchLine::name, "line", l)?),
        None => None,
    };
    let pitch = match a.value.get("pitch") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(v.as_f64().filter(|p| PITCHES.iter().any(|x| (x - p).abs() < 1e-9)).ok_or_else(|| OpError::param("pitch", "one of 0.25, 0.5, 0.75, 1, 1.5, 2, 3, 5, or null"))?)),
    };
    let thickness = match a.value.get("thickness") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(v.as_f64().and_then(|w| noshape_model::drawing::LINE_THICKNESSES.iter().copied().find(|x| (x - w).abs() < 1e-9)).ok_or_else(|| OpError::param("thickness", "one of the Thickness list (mm), or null for the drawing's"))?)),
    };
    let color = color(a)?;
    let revert = a.bool_or("revert", false)?;
    for id in &ids {
        let mut st = sk.style(noshape_sketch::EntityId(*id));
        if revert {
            st.line = SketchLine::default();
            st.color = None;
        }
        if let Some(l) = line {
            st.line = l;
        }
        if let Some(p) = pitch {
            if st.line == SketchLine::Solid {
                return Err(OpError::param("pitch", "Solid lines have no Pitch scale"));
            }
            st.pitch = p.filter(|p| (p - 1.0).abs() > 1e-9);
        }
        if let Some(w) = thickness {
            st.thickness = w;
        }
        if let Some(c) = color {
            st.color = c;
        }
        sk.styles.insert(*id, st);
    }
    commit(od, t.tab, si, t.d, "Style")?;
    let mut out = reply(od, t.tab, si, false)?;
    out["sketch_entities"] = json!(ids);
    Ok(out.into())
}

fn hatch_styles_op() -> Op {
    Op {
        group: "drawing",
        verb: "hatch_styles",
        summary: "Styles panel: hatch regions' pattern, scale, angle and color",
        description: "The Styles panel with hatch regions selected (ids: drawing_get), one edit \"Style\". `standard`: Hatch pattern group (another group takes its default pattern and Auto angle); `pattern`: its name as listed (or its first word); `scale`: \"Auto\" or 1–10; `angle`: \"Auto\" or 0–180 in 15° steps (the pattern's lines' angle on the sheet); no Scale or Angle on General; `color` (hex RRGGBB; null: the drawing's); `revert`: Revert to drawing properties (ANSI's default pattern, Auto scale and angle, the drawing's color).",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            req("hatches", json!({ "type": "array", "items": { "type": "integer" }, "minItems": 1 }), "The selected hatch region ids."),
            opt("revert", json!({ "type": "boolean", "default": false }), "Revert to drawing properties."),
            opt("standard", json!({ "type": "string", "enum": ["ansi", "iso", "general"] }), "Hatch pattern group."),
            opt("pattern", json!({ "type": "string" }), "Pattern name."),
            opt("scale", json!({ "type": ["string", "number"] }), "\"Auto\" or 1–10."),
            opt("angle", json!({ "type": ["string", "number"] }), "\"Auto\" or 0–180 (15° steps)."),
            opt("color", json!({ "type": ["string", "null"] }), "Color: hex RRGGBB."),
        ],
        mutates: true,
        destructive: false,
        run: hatch_styles,
    }
}

fn hatch_styles(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::drawing::hatch_pat;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let si = t.sheet;
    let hs = &mut t.d.sheets[si].sketch.hatches;
    let mut ids = Vec::new();
    for (i, v) in a.array("hatches")?.iter().enumerate() {
        ids.push(v.as_u64().filter(|id| hs.iter().any(|h| h.id == *id)).ok_or_else(|| OpError::param(&format!("hatches[{i}]"), "a hatch region id on the sheet"))?);
    }
    let auto = |k: &str, ok: &dyn Fn(f64) -> bool, what: &str| -> Result<Option<Option<f64>>, OpError> {
        match a.get(k) {
            None => Ok(None),
            Some(Value::String(s)) if s.eq_ignore_ascii_case("auto") => Ok(Some(None)),
            Some(v) => v.as_f64().filter(|x| ok(*x)).map(|x| Some(Some(x))).ok_or_else(|| OpError::param(k, what)),
        }
    };
    let scale = auto("scale", &|x| (1.0..=10.0).contains(&x) && x.fract() == 0.0, "\"Auto\" or a whole number 1 to 10")?;
    let angle = auto("angle", &|x| (0.0..=180.0).contains(&x) && (x / 15.0).fract() == 0.0, "\"Auto\" or 0 to 180 in steps of 15")?;
    let standard = match a.opt_str("standard")? {
        Some(_) => Some(super::drawing_tools::enum_arg(a, "standard", &[("ansi", 0), ("iso", 1), ("general", 2)], 0)?),
        None => None,
    };
    let color = color(a)?;
    let revert = a.bool_or("revert", false)?;
    for h in hs.iter_mut().filter(|h| ids.contains(&h.id)) {
        if revert {
            (h.standard, h.pattern, h.scale, h.angle, h.color) = (0, hatch_pat::default_pattern(0), 1.0, 0.0, None);
        }
        if let Some(g) = standard
            && h.standard != g
        {
            (h.standard, h.pattern, h.angle) = (g, hatch_pat::default_pattern(g), 0.0);
        }
        if let Some(name) = a.opt_str("pattern")? {
            h.pattern = super::drawing_tools::hatch_pattern_arg(h.standard, name)?;
        }
        if h.standard == 2 && (scale.is_some() || angle.is_some()) {
            return Err(OpError::param(if scale.is_some() { "scale" } else { "angle" }, "General patterns have no Scale or Angle"));
        }
        if let Some(s) = scale {
            h.scale = s.unwrap_or(1.0);
        }
        if let Some(an) = angle {
            h.angle = an.map_or(0.0, |an| an - hatch_pat::base_angle(h.standard, h.pattern));
        }
        if let Some(c) = color {
            h.color = c;
        }
    }
    commit(od, t.tab, si, t.d, "Style")?;
    let mut out = reply(od, t.tab, si, false)?;
    out["hatches"] = json!(ids);
    Ok(out.into())
}
