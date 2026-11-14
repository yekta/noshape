//! A version's own properties (live2 11_studios_abc (b), (c)). A version keeps the properties of its parts, assemblies
//! and drawings apart from its state: they can be edited later in the version (Description, Vendor, Project, Product
//! line, Title 1–3, Exclude from all BOMs, Unit of measure), and a release writes its part numbers, revisions and
//! states there, not into the workspace (the workspace's part keeps an empty Part number; the released drawing's title
//! block shows DWG NO. and REV only in the released version).
//!
//! Stored per microversion in the store's metadata ("version_props:<microversion>"), applied when a version is viewed.

use crate::session::Session;
use noshape_model::assembly::AssemblyProperties;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::drawing::DrawingMeta;
use noshape_model::partstudio::PartProperties;
use noshape_model::reference::BodyId;
use noshape_store::MicroversionId;
use serde::{Deserialize, Serialize};

const META_PREFIX: &str = "version_props:";

/// The properties a version holds over its state, whole per item.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VersionProps {
    pub parts: Vec<PartEntry>,
    pub assemblies: Vec<(TabId, AssemblyProperties)>,
    pub drawings: Vec<(TabId, DrawingMeta)>,
    /// A drawing's Drawn by, edited in the version.
    pub drawn_by: Vec<(TabId, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartEntry {
    pub tab: TabId,
    pub body: BodyId,
    pub props: PartProperties,
}

/// The fields a version keeps as they are (greyed in its Properties, live2 (c)); the others can be edited there.
pub const LOCKED: [&str; 8] = ["Name", "Category", "Part number", "Revision", "State", "Material", "Not revision managed", "Mass override"];

/// A drawing's fields a version keeps (live feel/release.json 12_studios_drawing_assembly_properties); Revision
/// description too unless the version released it ([`drawing_locked`]).
pub const DRAWING_LOCKED: [&str; 11] = ["Name", "Category", "Part number", "Revision", "State", "Approver", "Date approved", "Not revision managed", "Last changed date", "Last changed by", "Inspection count"];

/// An assembly's fields a version keeps (the same capture): its mass override with Override.
pub const ASSEMBLY_LOCKED: [&str; 7] = ["Name", "Category", "Part number", "Revision", "State", "Not revision managed", "Override"];

/// A drawing's greyed fields in its workspace (the same capture): set by releases or worked out.
pub const DRAWING_WORKSPACE_LOCKED: [&str; 5] = ["Approver", "Date approved", "Last changed date", "Last changed by", "Inspection count"];

/// The drawing fields greyed in a version: Revision description is editable only where the version released it.
pub fn drawing_locked(meta: &DrawingMeta) -> Vec<&'static str> {
    let mut out = DRAWING_LOCKED.to_vec();
    if meta.item.state.as_deref() != Some("Released") {
        out.push("Revision description");
    }
    out
}

impl VersionProps {
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty() && self.assemblies.is_empty() && self.drawings.is_empty() && self.drawn_by.is_empty()
    }

    pub fn set_part(&mut self, tab: TabId, body: BodyId, props: PartProperties) {
        self.parts.retain(|p| !(p.tab == tab && p.body == body));
        self.parts.push(PartEntry { tab, body, props });
    }

    pub fn set_assembly(&mut self, tab: TabId, props: AssemblyProperties) {
        self.assemblies.retain(|(t, _)| *t != tab);
        self.assemblies.push((tab, props));
    }

    pub fn set_drawing(&mut self, tab: TabId, meta: DrawingMeta) {
        self.drawings.retain(|(t, _)| *t != tab);
        self.drawings.push((tab, meta));
    }

    pub fn set_drawn_by(&mut self, tab: TabId, by: Option<String>) {
        self.drawn_by.retain(|(t, _)| *t != tab);
        self.drawn_by.push((tab, by));
    }

    /// Writes them over `doc` (a version's state as read from the store).
    pub fn apply(&self, doc: &mut Document) {
        for p in &self.parts {
            if let Some(TabContent::PartStudio(ps)) = doc.tab_mut(p.tab).map(|t| &mut t.content) {
                ps.part_properties.insert(p.body.clone(), p.props.clone());
            }
        }
        for (tab, props) in &self.assemblies {
            if let Some(TabContent::Assembly(a)) = doc.tab_mut(*tab).map(|t| &mut t.content) {
                *a.properties = props.clone();
            }
        }
        for (tab, meta) in &self.drawings {
            if let Some(TabContent::Drawing(d)) = doc.tab_mut(*tab).map(|t| &mut t.content) {
                *d.meta = meta.clone();
            }
        }
        for (tab, by) in &self.drawn_by {
            if let Some(TabContent::Drawing(d)) = doc.tab_mut(*tab).map(|t| &mut t.content) {
                d.drawn_by = by.clone();
            }
        }
    }
}

fn key(mv: MicroversionId) -> String {
    format!("{META_PREFIX}{mv}")
}

/// The properties version state `mv` holds (none: as its state has them).
pub fn read(session: &Session, mv: MicroversionId) -> VersionProps {
    session.store.get_meta(&key(mv)).ok().flatten().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn write(session: &mut Session, mv: MicroversionId, props: &VersionProps) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(props)?;
    session.store.set_meta(&key(mv), Some(&bytes))?;
    Ok(())
}

/// The Properties dialog's values on part `body` of Part Studio `tab` in the version being viewed: kept in the
/// version's own properties and shown at once.
pub fn set_part_values(session: &mut Session, tab: TabId, body: &BodyId, values: &crate::studio::PartPropertyValues) -> anyhow::Result<()> {
    let Some(mv) = session.viewing else { anyhow::bail!("not viewing a version") };
    let Some(TabContent::PartStudio(ps)) = session.doc.tab(tab).map(|t| &t.content) else { anyhow::bail!("not a Part Studio") };
    let mut copy = ps.clone();
    crate::studio::set_part_properties(&mut copy, body, values, &[]);
    let mut props = read(session, mv);
    props.set_part(tab, body.clone(), copy.part_properties.get(body).cloned().unwrap_or_default());
    write(session, mv, &props)?;
    props.apply(&mut session.doc);
    Ok(())
}

/// Assembly `tab`'s Properties saved in the version being viewed: its locked fields ([`ASSEMBLY_LOCKED`]) stay as the
/// version has them.
pub fn set_assembly_values(session: &mut Session, tab: TabId, props: AssemblyProperties) -> anyhow::Result<()> {
    let Some(mv) = session.viewing else { anyhow::bail!("not viewing a version") };
    let Some(TabContent::Assembly(a)) = session.doc.tab(tab).map(|t| &t.content) else { anyhow::bail!("not an assembly") };
    let now = (*a.properties).clone();
    let mut p = props;
    (p.part_number, p.revision, p.mass_override) = (now.part_number, now.revision, now.mass_override);
    (p.meta.state, p.meta.not_revision_managed) = (now.meta.state, now.meta.not_revision_managed);
    let mut all = read(session, mv);
    all.set_assembly(tab, p);
    write(session, mv, &all)?;
    all.apply(&mut session.doc);
    Ok(())
}

/// Drawing `tab`'s Properties saved in the version being viewed (`drawn_by`: its Drawn by): the fields
/// [`drawing_locked`] names stay as the version has them.
pub fn set_drawing_values(session: &mut Session, tab: TabId, meta: DrawingMeta, drawn_by: Option<String>) -> anyhow::Result<()> {
    let Some(mv) = session.viewing else { anyhow::bail!("not viewing a version") };
    let Some(TabContent::Drawing(d)) = session.doc.tab(tab).map(|t| &t.content) else { anyhow::bail!("not a drawing") };
    let now = (*d.meta).clone();
    let mut m = meta;
    (m.number, m.revision, m.approver, m.approved) = (now.number.clone(), now.revision.clone(), now.approver.clone(), now.approved.clone());
    (m.item.state, m.item.not_revision_managed) = (now.item.state.clone(), now.item.not_revision_managed);
    if drawing_locked(&now).contains(&"Revision description") {
        m.revision_description = now.revision_description;
    }
    let mut all = read(session, mv);
    all.set_drawing(tab, m);
    all.set_drawn_by(tab, drawn_by);
    write(session, mv, &all)?;
    all.apply(&mut session.doc);
    Ok(())
}
