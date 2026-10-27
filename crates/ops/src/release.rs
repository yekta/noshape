//! Release management (Release… on a tab or part, Revision history), shared
//! by the app's Release candidate dialog and the agent API: the document's
//! release log, the items a tab releases, and releasing a package (part
//! numbers onto the parts, a version named after it, the log).

use noshape_model::regen::derived::DerivedProvider;
use crate::session::Session;
use noshape_model::configuration::ConfigValues;
use noshape_model::document::{TabContent, TabId};
use noshape_model::reference::BodyId;
use noshape_model::release::{ItemKind, NumberingSchemes, Package, PackageKind, ReleaseItem, ReleaseLog};

/// Store meta key of the document's release log.
pub const META_KEY: &str = "releases";

fn numbering_path() -> std::path::PathBuf {
    noshape_scene::config_dir().join("numbering_schemes.json")
}

/// Company settings › Numbering schemes.
pub fn numbering_schemes() -> NumberingSchemes {
    std::fs::read(numbering_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn set_numbering_schemes(s: &NumberingSchemes) {
    let _ = std::fs::write(numbering_path(), serde_json::to_vec_pretty(s).unwrap_or_default());
}

/// Generates the next number of a sequential scheme (`pick` chooses the
/// scheme and object type) and saves the advanced counter.
pub fn next_number(pick: impl FnOnce(&mut NumberingSchemes) -> Option<String>) -> Option<String> {
    let mut n = numbering_schemes();
    let out = pick(&mut n)?;
    set_numbering_schemes(&n);
    Some(out)
}

/// The document's release log (with the settings' revision scheme and
/// part number uniqueness).
pub fn release_log(session: &Session) -> ReleaseLog {
    let mut log: ReleaseLog = session.store.get_meta(META_KEY).ok().flatten().map(|b| ReleaseLog::from_json(&b)).unwrap_or_default();
    let settings = crate::release_settings::release_settings();
    log.scheme = settings.scheme;
    log.drawing_reuse = numbering_schemes().drawing_reuse;
    log.auto_obsolete = settings.auto_obsolete;
    log
}

pub fn save_log(session: &mut Session, log: &ReleaseLog) -> anyhow::Result<()> {
    session.store.set_meta(META_KEY, Some(&log.to_json()))?;
    Ok(())
}

/// Release items for parts of a Part Studio (their names and part numbers).
/// `config`: the configuration of configured parts (empty: none); the
/// name then carries it ("Part 1 (Long)").
pub fn part_items(session: &Session, tab: TabId, parts: &[BodyId], names: &[String], config: &ConfigValues) -> Vec<ReleaseItem> {
    let log = release_log(session);
    let studio = session.doc.tab(tab).and_then(|t| match &t.content {
        TabContent::PartStudio(ps) => Some(ps),
        _ => None,
    });
    let props = studio.map(|ps| ps.part_properties.clone());
    // Inputs excluded from properties are released at their defaults.
    let config = &studio.map(|ps| ps.configuration.for_properties(config)).unwrap_or_else(|| config.clone());
    let desc = studio.map(|ps| ps.configuration.describe(config)).unwrap_or_default();
    parts
        .iter()
        .zip(names)
        .map(|(b, name)| {
            let p = props.as_ref().and_then(|p| p.get(b));
            let pn = p.and_then(|p| p.part_number.clone()).unwrap_or_default();
            // Release__typical_release_workflow: a part imported with a
            // revision keeps it in its first release.
            let nrm = p.is_some_and(|p| p.meta.not_revision_managed);
            let imported = p.and_then(|p| p.revision.clone()).filter(|r| !r.trim().is_empty() && log.revisions(&pn).is_empty());
            let name = if desc.is_empty() { name.clone() } else { format!("{name} ({desc})") };
            ReleaseItem { tab, part: Some(b.clone()), name, kind: ItemKind::Part, revision: if nrm { String::new() } else { imported.unwrap_or_else(|| log.next_revision(&pn)) }, part_number: pn, not_revision_managed: nrm, reverted: false, config: if desc.is_empty() { Default::default() } else { config.clone() }, document: None }
        })
        .collect()
}

/// Part number of a tab item: an Assembly's Properties, otherwise the
/// tab's metadata (store meta "tab_part_number:<id>").
pub fn tab_part_number(session: &Session, tab: TabId) -> String {
    match session.doc.tab(tab).map(|t| &t.content) {
        Some(TabContent::Assembly(a)) => a.properties.part_number.clone().unwrap_or_default(),
        _ => session.store.get_meta(&format!("tab_part_number:{}", tab.0)).ok().flatten().map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default(),
    }
}

/// The solid parts of a Part Studio tab (regenerated): ids and names.
pub fn studio_parts(provider: &dyn DerivedProvider, studio: TabId, config: &ConfigValues) -> (Vec<BodyId>, Vec<String>) {
    let ctx = if config.is_empty() { provider.context(&studio.to_string()) } else { provider.context_configured(&studio.to_string(), config) };
    let Some(ctx) = ctx else { return (Vec::new(), Vec::new()) };
    ctx.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| (b.id.clone(), b.name.clone())).unzip()
}

/// Release items for a tab (tab menu Release…; Release__typical_release_workflow):
/// a Part Studio's parts; an Assembly and everything in its Bill of
/// Materials (parts and subassemblies); a Drawing and the parts or
/// assemblies its views show; a Variable Studio or file itself.
pub fn tab_items(session: &Session, provider: &dyn DerivedProvider, tab: TabId) -> Vec<ReleaseItem> {
    let log = release_log(session);
    let mut out: Vec<ReleaseItem> = Vec::new();
    fn push(out: &mut Vec<ReleaseItem>, items: Vec<ReleaseItem>) {
        for it in items {
            if !out.iter().any(|o| o.same_object(&it)) {
                out.push(it);
            }
        }
    }
    let tab_item = |t: TabId, kind: ItemKind| {
        let name = session.doc.tab(t).map(|x| x.name.clone()).unwrap_or_default();
        let pn = tab_part_number(session, t);
        ReleaseItem { tab: t, part: None, name, kind, revision: log.next_revision(&pn), part_number: pn, not_revision_managed: false, reverted: false, config: Default::default(), document: None }
    };
    let all_parts = |studio: TabId, config: &ConfigValues| {
        let (ids, names) = studio_parts(provider, studio, config);
        part_items(session, studio, &ids, &names, config)
    };
    let one_part = |studio: TabId, b: &BodyId, config: &ConfigValues| {
        let (ids, names) = studio_parts(provider, studio, config);
        let name = ids.iter().position(|x| x == b).map(|i| names[i].clone()).unwrap_or_else(|| "Part".into());
        part_items(session, studio, std::slice::from_ref(b), &[name], config)
    };
    // Assemblies recurse into their subassemblies (guarded against cycles).
    type ItemFn<'a> = &'a dyn Fn(TabId, ItemKind) -> ReleaseItem;
    type PartsFn<'a> = &'a dyn Fn(TabId, &ConfigValues) -> Vec<ReleaseItem>;
    type PartFn<'a> = &'a dyn Fn(TabId, &BodyId, &ConfigValues) -> Vec<ReleaseItem>;
    fn assembly(session: &Session, a: TabId, seen: &mut Vec<TabId>, out: &mut Vec<ReleaseItem>, tab_item: ItemFn, all_parts: PartsFn, one_part: PartFn) {
        use noshape_model::assembly::InstanceSource;
        if seen.contains(&a) {
            return;
        }
        seen.push(a);
        push(out, vec![tab_item(a, ItemKind::Assembly)]);
        let Some(TabContent::Assembly(asm)) = session.doc.tab(a).map(|t| &t.content) else { return };
        for inst in &asm.instances {
            match &inst.source {
                // Only the configurations the assembly uses (Help: Releasing a configuration).
                InstanceSource::Part { studio, body, config } => push(out, one_part(*studio, body, config)),
                InstanceSource::PartStudio { studio, config } => push(out, all_parts(*studio, config)),
                InstanceSource::Assembly { assembly: sub, .. } => assembly(session, *sub, seen, out, tab_item, all_parts, one_part),
                _ => {}
            }
        }
    }
    match session.doc.tab(tab).map(|t| &t.content) {
        Some(TabContent::PartStudio(_)) => push(&mut out, all_parts(tab, &Default::default())),
        Some(TabContent::Assembly(_)) => assembly(session, tab, &mut Vec::new(), &mut out, &tab_item, &all_parts, &one_part),
        Some(TabContent::Drawing(d)) => {
            push(&mut out, vec![tab_item(tab, ItemKind::Drawing)]);
            for sheet in &d.sheets {
                for v in &sheet.views {
                    let noshape_model::drawing::ViewKind::Base { source, part, config, .. } = &v.kind else { continue };
                    match (session.doc.tab(*source).map(|t| &t.content), part) {
                        (Some(TabContent::Assembly(_)), _) => push(&mut out, vec![tab_item(*source, ItemKind::Assembly)]),
                        (Some(TabContent::PartStudio(_)), Some(b)) => push(&mut out, one_part(*source, b, config)),
                        (Some(TabContent::PartStudio(_)), None) => push(&mut out, all_parts(*source, config)),
                        _ => {}
                    }
                }
            }
        }
        Some(TabContent::VariableStudio(_)) => push(&mut out, vec![tab_item(tab, ItemKind::VariableStudio)]),
        Some(TabContent::File(_)) => push(&mut out, vec![tab_item(tab, ItemKind::File)]),
        _ => {}
    }
    out
}

/// Drawings of the document showing any of `items` (Include associated
/// drawings): a drawing whose base views show the item's part, Part Studio
/// or Assembly.
pub fn associated_drawings(session: &Session, provider: &dyn DerivedProvider, items: &[ReleaseItem]) -> Vec<ReleaseItem> {
    let mut out = Vec::new();
    for t in session.doc.ordered_tabs() {
        let TabContent::Drawing(d) = &t.content else { continue };
        let shows = d.sheets.iter().flat_map(|s| s.views.iter()).any(|v| match &v.kind {
            noshape_model::drawing::ViewKind::Base { source, part, .. } => items.iter().any(|it| it.tab == *source && (part.is_none() || it.part.is_none() || it.part == *part)),
            _ => false,
        });
        if shows {
            out.extend(tab_items(session, provider, t.id).into_iter().filter(|i| i.kind == ItemKind::Drawing));
        }
    }
    out
}

/// Items linked from versions of other documents: their releases go into
/// those documents' logs too.
pub fn mark_linked_versions(pkg: &Package) -> anyhow::Result<()> {
    let mut groups: Vec<(String, String)> = pkg.items.iter().filter(|i| !i.reverted).filter_map(|i| i.document.as_ref().filter(|l| !l.path.is_empty()).map(|l| (l.path.clone(), l.version.clone()))).collect();
    groups.sort();
    groups.dedup();
    let mut paths: Vec<String> = groups.iter().map(|(p, _)| p.clone()).collect();
    paths.dedup();
    for path in paths {
        let mut store = noshape_store::Document::open(&path)?;
        let mut log = store.get_meta(META_KEY).ok().flatten().map(|b| ReleaseLog::from_json(&b)).unwrap_or_default();
        for (_, v) in groups.iter().filter(|(p, _)| *p == path) {
            let items: Vec<ReleaseItem> = pkg.items.iter().filter(|i| !i.reverted && i.document.as_ref().is_some_and(|l| l.path == path && l.version == *v)).map(|i| ReleaseItem { document: None, ..i.clone() }).collect();
            log.record(Package { items, version: Some(v.clone()), ..pkg.clone() });
        }
        store.set_meta(META_KEY, Some(&log.to_json()))?;
    }
    Ok(())
}

/// The history text of a release's part number changes.
pub fn release_description(pkg: &Package) -> String {
    format!("Release {}", pkg.name)
}

/// Release (or obsolete) `pkg` as the dialog's Release / Obsolete button
/// does: a release puts its part numbers onto the parts (one change per
/// Part Studio) and the tab items, and is recorded at a new version named
/// after it; an obsoletion marks the version of the release it obsoletes.
/// Returns the package as recorded (its version set).
pub fn commit(session: &mut Session, mut pkg: Package) -> anyhow::Result<Package> {
    let kind = pkg.kind;
    if kind == PackageKind::Release {
        // Items linked from versions keep theirs (versions don't change).
        let mut tabs: Vec<TabId> = pkg.items.iter().filter(|i| i.document.is_none()).map(|i| i.tab).collect();
        tabs.dedup();
        for tab in tabs {
            let items: Vec<ReleaseItem> = pkg.items.iter().filter(|i| i.tab == tab && i.part.is_some() && i.document.is_none()).cloned().collect();
            let changed = session.doc.tab(tab).is_some_and(|t| match &t.content {
                TabContent::PartStudio(ps) => items.iter().any(|i| i.part.as_ref().and_then(|b| ps.part_properties.get(b)).and_then(|p| p.part_number.as_deref()) != Some(i.part_number.as_str())),
                _ => false,
            });
            if changed {
                session.edit_tab(tab, &release_description(&pkg), |c| {
                    if let TabContent::PartStudio(ps) = c {
                        for i in &items {
                            if let Some(b) = &i.part {
                                ps.part_properties.entry(b.clone()).or_default().part_number = Some(i.part_number.clone());
                            }
                        }
                    }
                })?;
            }
        }
        // Tab items keep their part numbers: an Assembly in its
        // Properties, other tabs in their metadata.
        for i in pkg.items.iter().filter(|i| i.part.is_none() && i.document.is_none()) {
            if tab_part_number(session, i.tab) == i.part_number {
                continue;
            }
            let is_assembly = matches!(session.doc.tab(i.tab).map(|t| &t.content), Some(TabContent::Assembly(_)));
            if is_assembly {
                let pn = i.part_number.clone();
                session.edit_tab(i.tab, &release_description(&pkg), |c| {
                    if let TabContent::Assembly(a) = c {
                        a.properties.part_number = Some(pn);
                    }
                })?;
            } else {
                session.store.set_meta(&format!("tab_part_number:{}", i.tab.0), Some(i.part_number.as_bytes()))?;
            }
        }
    }
    // An obsoletion marks the version of the release it obsoletes; a
    // release creates a version named after it.
    let released_at = (kind == PackageKind::Obsoletion).then(|| release_log(session)).and_then(|log| pkg.items.iter().find_map(|i| log.release_version_of(&i.part_number, &i.revision)));
    pkg.version = Some(match released_at {
        Some(v) => v,
        None => session.create_version(&pkg.name, &pkg.notes)?.to_string(),
    });
    if kind == PackageKind::Release {
        mark_linked_versions(&pkg)?;
    }
    let mut log = release_log(session);
    log.record(pkg.clone());
    save_log(session, &log)?;
    Ok(pkg)
}
