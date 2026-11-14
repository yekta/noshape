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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartEntry {
    pub tab: TabId,
    pub body: BodyId,
    pub props: PartProperties,
}

/// The fields a version keeps as they are (greyed in its Properties, live2 (c)); the others can be edited there.
pub const LOCKED: [&str; 8] = ["Name", "Category", "Part number", "Revision", "State", "Material", "Not revision managed", "Mass override"];

impl VersionProps {
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty() && self.assemblies.is_empty() && self.drawings.is_empty()
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
