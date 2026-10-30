//! The Part Studio's "Sheet metal table and flat view" panel as shared code
//! (live "qa5 sm" (8), "ui (10-24 a/b)"): the Bends and Other joints rows,
//! a bend's Radius / K factor edit (a Modify joint), Move up / Move down,
//! Convert to rip. The agent API's sheetmetal ops use it; the app's panel
//! (crates/app/src/part_studio/sheet_metal_panel.rs) has the same logic and
//! moves onto this module after the app split.

use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::{Feature, PartStudio};
use noshape_model::reference::{FeatureId, Reference};
use noshape_model::sheetmetal::{Bend, BendKind, SheetMetalData};
use noshape_units::{ExprValue, UnitPrefs};

/// One row of the Bends or Other joints table.
#[derive(Clone, Debug, PartialEq)]
pub struct JointRow {
    pub name: String,
    pub radius: f64,
    /// Degrees.
    pub angle: f64,
    /// Bend direction (from the flat pattern's bend line), when known.
    pub up: Option<bool>,
    pub k_factor: f64,
    pub style: &'static str,
}

/// The first bend of `i`'s joint (a bend a cut split is one joint: its
/// pieces share the joint attribute; live qa4 S1, one table row).
pub fn joint_of(data: &SheetMetalData, i: usize) -> usize {
    match &data.bends[i].joint {
        Some(j) => data.bends.iter().position(|b| b.joint.as_ref() == Some(j)).unwrap_or(i),
        None => i,
    }
}

/// The table rows (live "ui (10-24 a)" (1), request 1604): one letter
/// sequence over all the model's joints in the order their definition
/// entities appeared (Onshape's transient-id order, an entity a later
/// feature rebuilt last), then (natively) feature place, bends first,
/// index; "<kind> <letter>", kind Bend / Hem / Joint. `order`: a feature's
/// place in the list. Rows in model order, one per bend (a split joint's
/// pieces share their first piece's letter); [`bend_row_order`] gives the
/// shown rows.
pub fn joint_rows(data: &SheetMetalData, up: &dyn Fn(usize) -> Option<bool>, order: &dyn Fn(FeatureId) -> usize) -> (Vec<JointRow>, Vec<JointRow>) {
    let letter = |i: usize| {
        let mut s = String::new();
        let mut n = i;
        loop {
            s.insert(0, (b'A' + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }
        s
    };
    // (birth, feature place, bends first, index) -> letter; a joint's pieces
    // one entry, at its earliest piece's birth.
    let birth = |i: usize| data.bends.iter().enumerate().filter(|(j, _)| joint_of(data, *j) == i).map(|(_, b)| b.birth).min().unwrap_or(data.bends[i].birth);
    let mut seq: Vec<(u64, usize, u8, usize)> = (0..data.bends.len())
        .filter(|&i| joint_of(data, i) == i)
        .map(|i| (birth(i), order(data.bends[i].feature), 0, i))
        .chain(data.rips.iter().enumerate().map(|(i, r)| (r.birth, order(r.feature), 1, i)))
        .collect();
    seq.sort();
    let name_of = |kind: u8, i: usize| {
        let i = if kind == 0 { joint_of(data, i) } else { i };
        seq.iter().position(|e| e.2 == kind && e.3 == i).map(letter).unwrap_or_default()
    };
    let mut joints = Vec::new();
    for (ri, r) in data.rips.iter().enumerate() {
        joints.push(JointRow {
            name: format!("Joint {}", name_of(1, ri)),
            radius: 0.0,
            angle: 0.0,
            up: None,
            k_factor: 0.0,
            style: match r.style.as_str() {
                "BUTT" => "Butt joint - Direction 1",
                "BUTT2" => "Butt joint - Direction 2",
                _ => "Edge joint",
            },
        });
    }
    let mut bends = Vec::new();
    for (i, b) in data.bends.iter().enumerate() {
        let kind = match b.kind {
            BendKind::Bend => "Bend",
            BendKind::Hem => "Hem",
            BendKind::Rolled => "Rolled",
        };
        bends.push(JointRow { name: format!("{kind} {}", name_of(0, i)), radius: b.radius, angle: b.angle.to_degrees(), up: up(i), k_factor: b.k_factor, style: "" });
    }
    (bends, joints)
}

/// Where a table edit's Modify joint goes (live "ui (10-24 b)"): at the
/// rollback bar, but before the model's Finish sheet metal model when that
/// comes first.
pub fn joint_edit_index(ps: &PartStudio, model: FeatureId) -> usize {
    let bar = ps.rollback_index();
    let from = ps.features.iter().position(|f| f.id == model).map_or(0, |i| i + 1);
    let finish = ps.features.iter().enumerate().skip(from).find(|(_, f)| f.type_id == "sheetMetalEnd" && !f.suppressed).map(|(i, _)| i);
    finish.map_or(bar, |f| f.min(bar))
}

/// Insert a table edit's feature at `at`, the rollback bar staying under it.
pub fn insert_joint_edit(ps: &mut PartStudio, f: Feature, at: usize) {
    let at = at.min(ps.features.len());
    ps.features.insert(at, f);
    if let Some(r) = ps.rollback
        && at <= r
    {
        ps.rollback = Some(r + 1);
    }
}

/// A bend's faces as references.
pub fn bend_faces(b: &Bend) -> Vec<Reference> {
    b.inner_faces.iter().chain(&b.outer_faces).map(|n| Reference::Face((**n).clone())).collect()
}

/// The face a bend's edits name (its first inner face, else outer).
pub fn bend_face(b: &Bend) -> Option<Reference> {
    b.inner_faces.first().or(b.outer_faces.first()).map(|n| Reference::Face((**n).clone()))
}

/// The Bends rows' order (indices into the model's bends): the stored Move
/// up / Move down order, then the rest in model order; a split joint once
/// (its first piece).
pub fn bend_row_order(data: &SheetMetalData, stored: &[Reference]) -> Vec<usize> {
    let key = |i: usize| bend_faces(&data.bends[i]).iter().filter_map(|r| stored.iter().position(|s| s == r)).min().unwrap_or(usize::MAX);
    let mut idx: Vec<usize> = (0..data.bends.len()).filter(|&i| joint_of(data, i) == i).collect();
    idx.sort_by_key(|&i| (key(i), i));
    idx
}

/// The model's stored Bends row order (Move up / Move down).
pub fn stored_order(ps: &PartStudio, model: FeatureId) -> Vec<Reference> {
    ps.bend_order.iter().find(|(m, _)| *m == model).map(|(_, v)| v.clone()).unwrap_or_default()
}

/// A table number as live writes it ("2", "0.01", "0.45", "180"): up to
/// six decimals, trailing zeros dropped.
pub fn plain(x: f64) -> String {
    let s = format!("{:.6}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// The Bends table columns an edit can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BendField {
    Radius,
    KFactor,
}

/// The history text of a bend edit.
pub const MODIFY_JOINT: &str = "Modify joint";

/// An edited Bends cell: live adds a "Modify joint N" for that bend with the
/// value overridden ("3" + Tab on Bend B's radius → Modify joint 1, radius
/// 3 mm) at the rollback bar or before the model's Finish; a second edit of
/// the bend changes that Modify joint (live "ui (10-24 b)"). A bare radius is
/// in the document's unit. False when the text is empty or the value as
/// shown.
pub fn edit_bend(ps: &mut PartStudio, model: FeatureId, data: &SheetMetalData, bend: usize, field: BendField, text: &str, units: &UnitPrefs) -> bool {
    let text = text.trim().to_string();
    let Some(b) = data.bends.get(bend) else { return false };
    let Some(face) = bend_face(b) else { return false };
    let current = match field {
        BendField::Radius => plain(b.radius / units.si_factor(noshape_units::Dim::LENGTH)),
        BendField::KFactor => plain(b.k_factor),
    };
    if text.is_empty() || text == current {
        return false;
    }
    let bare = text.parse::<f64>().is_ok();
    let value = if field == BendField::Radius && bare { format!("{text} {}", units.length.symbol()) } else { text };
    let faces = bend_faces(b);
    let set = |p: &mut ParamValues| match field {
        BendField::Radius => {
            p.set("useDefaultRadius", ParamValue::Bool(false));
            p.set("radius", ParamValue::Expr(ExprValue::new(&value)));
        }
        BendField::KFactor => {
            p.set("useDefaultKFactor", ParamValue::Bool(false));
            p.set("bendCalculationType", ParamValue::Enum("K_FACTOR".into()));
            p.set("kFactor", ParamValue::Expr(ExprValue::new(&value)));
        }
    };
    let at = joint_edit_index(ps, model);
    let from = ps.features.iter().position(|f| f.id == model).map_or(0, |i| i + 1);
    let is_this_bend = |f: &Feature| {
        f.type_id == "sheetMetalJoint"
            && !f.suppressed
            && matches!(f.params.0.get("jointType"), Some(ParamValue::Enum(t)) if t == "BEND")
            && matches!(f.params.0.get("entity"), Some(ParamValue::Query(q)) if q.iter().any(|r| faces.contains(r)))
    };
    if let Some(i) = (from..at.min(ps.features.len())).rev().find(|&i| is_this_bend(&ps.features[i])) {
        set(&mut ps.features[i].params);
        return true;
    }
    let Some(mut f) = ps.new_feature("sheetMetalJoint", units) else { return false };
    f.params.set("entity", ParamValue::Query(vec![face]));
    f.params.set("jointType", ParamValue::Enum("BEND".into()));
    set(&mut f.params);
    insert_joint_edit(ps, f, at);
    true
}

/// The history text of [`convert_to_rip`].
pub const CONVERT_TO_RIP: &str = "Convert joint to rip.";

/// A Bends row's "Convert … to rip": a Modify joint of the bend, Rip / Edge
/// joint (live "ui (10-24 b)").
pub fn convert_to_rip(ps: &mut PartStudio, model: FeatureId, face: Reference, units: &UnitPrefs) -> bool {
    let Some(mut f) = ps.new_feature("sheetMetalJoint", units) else { return false };
    f.params.set("entity", ParamValue::Query(vec![face]));
    f.params.set("jointType", ParamValue::Enum("RIP".into()));
    f.params.set("jointStyle", ParamValue::Enum("EDGE".into()));
    let at = joint_edit_index(ps, model);
    insert_joint_edit(ps, f, at);
    true
}

/// "Move up" / "Move down" (also the history text).
pub fn move_description(delta: isize) -> &'static str {
    if delta < 0 { "Move up" } else { "Move down" }
}

/// Move up (`delta` -1) / Move down (+1) bend `bend` (model index) in the
/// table: the whole shown order is stored. False at the table's end.
pub fn move_bend(ps: &mut PartStudio, model: FeatureId, data: &SheetMetalData, bend: usize, delta: isize) -> bool {
    let mut order = bend_row_order(data, &stored_order(ps, model));
    let Some(pos) = order.iter().position(|&i| i == bend) else { return false };
    let to = pos as isize + delta;
    if to < 0 || to as usize >= order.len() {
        return false;
    }
    order.swap(pos, to as usize);
    let list: Vec<Reference> = order.iter().filter_map(|&i| bend_faces(&data.bends[i]).into_iter().next()).collect();
    ps.bend_order.retain(|(m, _)| *m != model);
    ps.bend_order.push((model, list));
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn table_numbers_drop_trailing_zeros() {
        assert_eq!(super::plain(2.0), "2");
        assert_eq!(super::plain(0.45), "0.45");
        assert_eq!(super::plain(-0.0), "0");
    }
}
