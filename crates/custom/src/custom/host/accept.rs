//! A query parameter's Filter applied when the feature runs: a selection it refuses is left out, a missing selection
//! (live ERR-SM-FILTER-FACE: a wall face in Flange's edges, "Missing Face of Flange 2", "Select edges to flange.",
//! 1 missing selection). Conditions not decided here accept.

use super::CtxState;
use crate::reference::Reference;
use noshape_featurescript::annot::Ann;

/// Whether `filter` accepts `r`.
pub(crate) fn accepts(st: &CtxState, filter: &Ann, r: &Reference) -> bool {
    decide(st, filter, r).unwrap_or(true)
}

/// `None`: a condition not decided here.
fn decide(st: &CtxState, filter: &Ann, r: &Reference) -> Option<bool> {
    match filter {
        Ann::And(a, b) => match (decide(st, a, r), decide(st, b, r)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        Ann::Or(a, b) => match (decide(st, a, r), decide(st, b, r)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        Ann::Not(a) => decide(st, a, r).map(|x| !x),
        Ann::Enum(t, m) => match t.as_str() {
            "EntityType" => entity_type(r).map(|k| k == m),
            "SheetMetalDefinitionEntityType" => entity_type(r).filter(|k| *k != "BODY").map(|_| definition_type(st, r) == Some(m.as_str())),
            _ => None,
        },
        _ => None,
    }
}

/// A part's face, edge, vertex or body (sketch entities, regions and planes: not decided).
fn entity_type(r: &Reference) -> Option<&'static str> {
    match r {
        Reference::Face(_) => Some("FACE"),
        Reference::Edge(_) => Some("EDGE"),
        Reference::Vertex(_) => Some("VERTEX"),
        Reference::Body(_) => Some("BODY"),
        _ => None,
    }
}

/// The type of the sheet metal definition entity `r` comes from (`None`: not a sheet metal part's): the entity on the
/// definition sheet sharing its association attribute.
fn definition_type(st: &CtxState, r: &Reference) -> Option<&'static str> {
    let own = super::sheetmetal::associations(st, r);
    if own.is_empty() {
        return None;
    }
    let tag = super::std_tag("sheetMetalAttribute.fs", "SMAssociationAttribute");
    let q = super::query::QueryCx::new(st);
    let on_sheet = |e: &Reference| q.from_ref(e).iter().any(|x| matches!(q.owner(x), Some(super::query::Ent::Body(b)) if st.ctx.bodies[b].kind != crate::display::BodyKind::Solid));
    st.attributes()
        .iter()
        .filter(|(e, v)| v.get("attribute").tag() == tag && own.contains(v.get("attribute")) && on_sheet(e))
        .find_map(|(e, _)| match e {
            Reference::Vertex(_) => Some("VERTEX"),
            Reference::Edge(_) => Some("EDGE"),
            Reference::Face(_) => Some("FACE"),
            _ => None,
        })
}
