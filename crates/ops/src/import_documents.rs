//! The documents page's Import files… (Create ▾ › Import files…): CAD files into new documents. "Import to a single
//! document" makes one document (a group of files, or each file with Split); "Split into multiple documents" makes a
//! STEP assembly with subassemblies a folder of documents linked by version (live behaviours "dx 1742 (B)"), and
//! "Create one document per part or assembly" gives every part and assembly its own document there.

use crate::doc_library as lib;
use crate::import::{self, ImportChoice};
use crate::session::Session;
use noshape_model::assembly::{Assembly, Instance, InstanceId, InstanceSource, Placement};
use noshape_model::configuration::ConfigValues;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::features::importing::ImportPartInfo;
use noshape_model::file_tab::FileTab;
use noshape_model::reference::BodyId;
use noshape_model::{ParamValue, PartStudio};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The Import dialog's card on the documents page (Combine to a single Part Studio is `ImportChoice::combine`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageImport {
    Single,
    Split,
    PerPart,
}

/// What an import made: the document to open, every document, and the folder of a split file.
#[derive(Debug, Default)]
pub struct Imported {
    pub open: Option<PathBuf>,
    pub documents: Vec<PathBuf>,
    pub folder: Option<PathBuf>,
}

/// A new document for CAD files with the modal's options, named after the first file (with its extension, as live):
/// its Part Studios / Assemblies first, then "CAD Imports"; V1 made at once (live request 1742). The errors of files
/// that didn't translate next to ones that did come back with it; none translated is an error.
pub fn import_single(paths: &[PathBuf], choice: &ImportChoice, place: &Path, new_doc: Document) -> anyhow::Result<(PathBuf, Vec<String>)> {
    let mut doc = new_doc;
    let units = doc.settings.units;
    doc.tabs.clear();
    doc.settings.tab_order.clear();
    let (inputs, mut errors) = import::read_inputs(paths, choice, true, &units);
    let (_, failed) = import::apply(&mut doc, inputs, choice, None, &units);
    errors.extend(failed);
    if !doc.tabs.iter().any(|t| !matches!(t.content, TabContent::File(_))) {
        anyhow::bail!("{}", errors.join("\n"));
    }
    models_first(&mut doc);
    let (_, name) = import::import_names(&paths[0]);
    let target = free_document_path(place, &name);
    save_versioned(&target, doc)?;
    Ok((target, errors))
}

/// Split a STEP assembly into a folder named after the file (live 1742 (B), 2424): one document per subassembly
/// ("Sub": its Assembly, one Part Studio per part not made before, CAD Imports with their Parasolid files), deepest
/// first, and the top one named after the file (Assembly 1, its own parts' Part Studios, CAD Imports with the file),
/// each with V1; an assembly uses another document's subassembly and Part Studios at that V1 (a flat assembly is one
/// document). `per_part`: every part its own document too ("Part 1.x_t"), used at its V1. `None` when the file is no
/// assembly (one document then, as Import to a single document).
pub fn import_split(path: &Path, choice: &ImportChoice, per_part: bool, place: &Path, new_doc: &dyn Fn() -> Document) -> anyhow::Result<Option<Imported>> {
    use noshape_kernel as k;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if choice.combine || !matches!(ext.as_str(), "step" | "stp") {
        return Ok(None);
    }
    let Some(((root, parts, _), nesting)) = k::read_step_assembly_nested(path, k::LengthUnit::Meter)? else { return Ok(None) };
    // (A flat assembly is split too: one document, a Part Studio per part, live 2424 (3).)
    if nesting.subs.is_empty() && nesting.children.len() < 2 && !per_part {
        return Ok(None);
    }
    let (_, file) = import::import_names(path);
    let bytes = std::fs::read(path)?;
    let folder = lib::create_folder(place, &free_folder_name(place, &file))?;
    let units = new_doc().settings.units;
    let ctx = Split { file: &file, bytes: &bytes, choice, units, parts: &parts, nesting: &nesting, new_doc };
    let mut made = Imported { folder: Some(folder.clone()), ..Default::default() };
    // Each part's own document (per part), used by key.
    let mut part_keys: HashMap<usize, (String, BodyId)> = HashMap::new();
    if per_part {
        let mut used: Vec<usize> = nesting.children.iter().rev().filter_map(|(_, c)| c.part).collect();
        dedup(&mut used);
        for p in used {
            let mut doc = ctx.empty();
            let Some((studio, body)) = ctx.part_studio(&mut doc, p) else { continue };
            ctx.keep_files(&mut doc, false, &[p]);
            models_first(&mut doc);
            // (Live 2424 (4): "Part 1.x_t", after the Parasolid file it holds.)
            let target = free_document_path(&folder, &format!("{}.x_t", ctx.part_name(p)));
            let mv = save_versioned(&target, doc)?;
            part_keys.insert(p, (external_key(&target, mv, studio), body));
            made.documents.push(target);
        }
    }
    // The subassemblies, deepest first: each one's document is there when its owner's is made.
    let mut sub_keys: HashMap<usize, String> = HashMap::new();
    // A part's Part Studio is made once, in the first document that uses it: a later one uses it there at V1 (live
    // 2424 (2): a subassembly used twice is two documents, the second's Part Studios the first's).
    for s in deepest_first(&nesting) {
        let name = ctx.sub_name(s);
        let mut doc = ctx.empty();
        let (tab, studios) = ctx.assembly(&mut doc, Some(s), &name, &part_keys, &sub_keys);
        ctx.keep_files(&mut doc, false, &studios.iter().map(|x| x.0).collect::<Vec<_>>());
        models_first(&mut doc);
        let target = free_document_path(&folder, &name);
        let mv = save_versioned(&target, doc)?;
        sub_keys.insert(s, external_key(&target, mv, tab));
        for (p, studio, body) in studios {
            part_keys.insert(p, (external_key(&target, mv, studio), body));
        }
        made.documents.push(target);
    }
    let mut doc = ctx.empty();
    let (_, studios) = ctx.assembly(&mut doc, None, if root.is_empty() { "Assembly 1" } else { &root }, &part_keys, &sub_keys);
    ctx.keep_files(&mut doc, true, &studios.iter().map(|x| x.0).collect::<Vec<_>>());
    models_first(&mut doc);
    let target = free_document_path(&folder, &file);
    save_versioned(&target, doc)?;
    made.documents.push(target.clone());
    made.open = Some(target);
    Ok(Some(made))
}

/// The documents page's Import files… for `paths` into `place` (the documents folder or a folder of it): each
/// group's document (all files one document; with Split or per part each file its own, an assembly with
/// subassemblies its folder of documents). Returns what was made and, per file, why it failed.
pub fn import_documents(paths: &[PathBuf], choice: &ImportChoice, how: PageImport, place: &Path, new_doc: &dyn Fn() -> Document) -> (Imported, Vec<(Vec<PathBuf>, String)>) {
    let mut made = Imported::default();
    let mut failed = Vec::new();
    let groups: Vec<Vec<PathBuf>> = if how == PageImport::Single { vec![paths.to_vec()] } else { paths.iter().map(|p| vec![p.clone()]).collect() };
    for g in groups {
        if how != PageImport::Single {
            match import_split(&g[0], choice, how == PageImport::PerPart, place, new_doc) {
                Ok(Some(m)) => {
                    made.open = made.open.or(m.open);
                    made.documents.extend(m.documents);
                    made.folder = made.folder.or(m.folder);
                    continue;
                }
                Ok(None) => {}
                Err(e) => {
                    failed.push((g, format!("{e:#}")));
                    continue;
                }
            }
        }
        match import_single(&g, choice, place, new_doc()) {
            Ok((p, errors)) => {
                made.open = made.open.or(Some(p.clone()));
                made.documents.push(p);
                if !errors.is_empty() {
                    failed.push((g, errors.join("\n")));
                }
            }
            Err(e) => failed.push((g, format!("{e:#}"))),
        }
    }
    (made, failed)
}

/// What every document of a split shares.
struct Split<'a> {
    file: &'a str,
    bytes: &'a [u8],
    choice: &'a ImportChoice,
    units: noshape_units::UnitPrefs,
    parts: &'a [noshape_kernel::ImportedShape],
    nesting: &'a noshape_kernel::StepNesting,
    new_doc: &'a dyn Fn() -> Document,
}

impl Split<'_> {
    fn empty(&self) -> Document {
        let mut d = (self.new_doc)();
        d.tabs.clear();
        d.settings.tab_order.clear();
        d
    }

    /// A subassembly's name, less an exporter's instance number (live 2424 (1): Onshape's STEP names the product
    /// "SubA <1>"; the document, its tab and its instances read "SubA").
    fn sub_name(&self, s: usize) -> String {
        without_instance_number(&self.nesting.subs[s])
    }

    /// A part's name: the file's, else "Part <n>" by its place in the file.
    fn part_name(&self, p: usize) -> String {
        self.parts.get(p).and_then(|x| x.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Part {}", p + 1))
    }

    /// The part's own Part Studio (named after it, one Import feature holding it): its tab and its body.
    fn part_studio(&self, doc: &mut Document, p: usize) -> Option<(TabId, BodyId)> {
        use noshape_kernel as k;
        let part = self.parts.get(p)?;
        let info = vec![ImportPartInfo { name: part.name.clone(), color: part.color, opacity: part.opacity, faces: part.face_colors.clone() }];
        let shape = k::Shape::compound(std::slice::from_ref(&part.shape));
        let foreign = import::Foreign { brep: String::from_utf8_lossy(&shape.to_brep_bytes().ok()?).to_string(), parts: serde_json::to_string(&info).ok()? };
        let mut ps = PartStudio::new();
        import::add_import_feature(&mut ps, self.file, foreign, &self.units);
        let f = ps.features.last_mut()?;
        for (k, v) in [("importAppearances", self.choice.appearances), ("yAxisIsUp", self.choice.y_up), ("createComposite", self.choice.composite), ("joinAdjacentSurfaces", self.choice.join)] {
            f.params.set(k, ParamValue::Bool(v));
        }
        let counts = [k::ShapeType::Solid, k::ShapeType::Shell, k::ShapeType::Face, k::ShapeType::Edge].map(|t| part.shape.count(t));
        let body = noshape_model::native_assembly::bodies_by_order(f.id, &[counts]).into_iter().next().flatten()?;
        let studio = doc.add_tab(&noshape_model::native_assembly::unique_name(doc, &self.part_name(p)), TabContent::PartStudio(ps));
        Some((studio, body))
    }

    /// An assembly's tab (`owner`: None the root, else a subassembly) with its parts (their Part Studios here, or
    /// their documents' with `part_keys`) and its subassemblies' documents (`sub_keys`), placed as in the file and
    /// listed in the reverse of its order, as the single-document import.
    fn assembly(&self, doc: &mut Document, owner: Option<usize>, name: &str, part_keys: &HashMap<usize, (String, BodyId)>, sub_keys: &HashMap<usize, String>) -> (TabId, Vec<(usize, TabId, BodyId)>) {
        let children: Vec<&noshape_kernel::NestChild> = self.nesting.children.iter().rev().filter(|(o, _)| *o == owner).map(|(_, c)| c).collect();
        let mut studios: HashMap<usize, (TabId, BodyId)> = HashMap::new();
        let mut asm = Assembly::default();
        let mut count: HashMap<String, u32> = HashMap::new();
        for c in children {
            let (source, label) = match (c.part, c.sub) {
                (Some(p), _) => match part_keys.get(&p) {
                    Some((key, body)) => (InstanceSource::External { key: key.clone(), body: Some(body.clone()), config: ConfigValues::default() }, self.part_name(p)),
                    None => {
                        if !studios.contains_key(&p)
                            && let Some(s) = self.part_studio(doc, p)
                        {
                            studios.insert(p, s);
                        }
                        let Some((studio, body)) = studios.get(&p).cloned() else { continue };
                        (InstanceSource::Part { studio, body, config: ConfigValues::default() }, self.part_name(p))
                    }
                },
                (None, Some(s)) => match sub_keys.get(&s) {
                    Some(key) => (InstanceSource::External { key: key.clone(), body: None, config: ConfigValues::default() }, self.sub_name(s)),
                    None => continue,
                },
                _ => continue,
            };
            // (Numbered again here: "Sub <1>", "Sub <2>", live 2424 (2).)
            let base = if c.name.trim().is_empty() { label } else { without_instance_number(&c.name) };
            let n = count.entry(base.clone()).or_insert(0);
            *n += 1;
            let id = InstanceId(asm.next_id());
            asm.instances.push(Instance::new(id, format!("{base} <{n}>"), source, Placement::from_dmat4(&c.transform)));
        }
        let tab = doc.add_tab(name, TabContent::Assembly(Box::new(asm)));
        // The Assembly first, then its Part Studios (live: Sub, Part 2, Part 3).
        let order = &mut doc.settings.tab_order;
        order.retain(|t| *t != tab);
        order.insert(0, tab);
        let mut made: Vec<(usize, TabId, BodyId)> = studios.into_iter().map(|(p, (t, b))| (p, t, b)).collect();
        made.sort_by_key(|x| x.0);
        (tab, made)
    }

    /// "CAD Imports" (live 2424 (5)): a Parasolid file per part whose Part Studio this document has ("Part 2.x_t"),
    /// and the imported file itself in the top document only (`whole`).
    fn keep_files(&self, doc: &mut Document, whole: bool, parts: &[usize]) {
        let folder = import::cad_imports_folder(doc);
        if whole {
            import::add_file_tab(doc, FileTab::new(self.file, self.bytes, noshape_model::file_tab::now()), Some(folder), None);
        }
        for &p in parts {
            let Some(bytes) = self.parts.get(p).and_then(|x| parasolid_bytes(&x.shape)) else { continue };
            import::add_file_tab(doc, FileTab::new(&format!("{}.x_t", self.part_name(p)), &bytes, noshape_model::file_tab::now()), Some(folder), None);
        }
    }
}

/// `name` less an exporter's trailing instance number (" <1>").
fn without_instance_number(name: &str) -> String {
    match name.rsplit_once(" <") {
        Some((base, rest)) if rest.strip_suffix('>').is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit())) && !base.trim().is_empty() => base.to_string(),
        _ => name.to_string(),
    }
}

/// A part as a Parasolid text file's bytes (written to a scratch file: the kernel writes to paths).
fn parasolid_bytes(shape: &noshape_kernel::Shape) -> Option<Vec<u8>> {
    let tmp = std::env::temp_dir().join(format!("noshape-split-{}-{:x}.x_t", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos()));
    noshape_kernel::write_parasolid(&tmp, std::slice::from_ref(shape), noshape_kernel::LengthUnit::Meter, false).ok()?;
    let bytes = std::fs::read(&tmp).ok();
    let _ = std::fs::remove_file(&tmp);
    bytes
}

/// The models before the files (the CAD Imports folder last).
fn models_first(doc: &mut Document) {
    let models: Vec<TabId> = doc.ordered_tabs().iter().filter(|t| !matches!(t.content, TabContent::File(_))).map(|t| t.id).collect();
    let rest: Vec<TabId> = doc.settings.tab_order.iter().copied().filter(|id| !models.contains(id)).collect();
    doc.settings.tab_order = models.into_iter().chain(rest).collect();
}

/// The document saved at `target` with V1 (live: Start → V1 → Main at once); V1's microversion.
fn save_versioned(target: &Path, doc: Document) -> anyhow::Result<noshape_store::MicroversionId> {
    let mut session = Session::create(target, doc)?;
    let mv = session.head()?;
    session.create_version("V1", "")?;
    Ok(mv)
}

/// Another document's tab at a version, as Insert › Other documents keys it.
fn external_key(path: &Path, microversion: noshape_store::MicroversionId, tab: TabId) -> String {
    crate::derived::ExternalRef { path: lib::canonical(path), microversion, tab: tab.to_string() }.key()
}

/// The subassemblies with the deepest first (a subassembly's own subassemblies before it).
fn deepest_first(n: &noshape_kernel::StepNesting) -> Vec<usize> {
    fn visit(n: &noshape_kernel::StepNesting, s: usize, out: &mut Vec<usize>, seen: &mut Vec<usize>) {
        if seen.contains(&s) {
            return;
        }
        seen.push(s);
        for (_, c) in n.children.iter().filter(|(o, _)| *o == Some(s)) {
            if let Some(child) = c.sub {
                visit(n, child, out, seen);
            }
        }
        out.push(s);
    }
    let (mut out, mut seen) = (Vec::new(), Vec::new());
    for s in 0..n.subs.len() {
        visit(n, s, &mut out, &mut seen);
    }
    out
}

fn dedup(v: &mut Vec<usize>) {
    let mut seen = Vec::new();
    v.retain(|x| {
        let new = !seen.contains(x);
        seen.push(*x);
        new
    });
}

/// A free path in `place` for a document named `name` ("name", "name (2)", … as the documents page names them).
pub fn free_document_path(place: &Path, name: &str) -> PathBuf {
    let clean = lib::sanitize_name(name);
    let mut target = place.join(format!("{clean}.{}", lib::EXT));
    let mut n = 2;
    while target.exists() {
        target = place.join(format!("{clean} ({n}).{}", lib::EXT));
        n += 1;
    }
    target
}

/// The folder's name: the file's, " (1)", " (2)"… when taken.
fn free_folder_name(place: &Path, file: &str) -> String {
    let clean = lib::sanitize_name(file);
    if !place.join(&clean).exists() {
        return clean;
    }
    (1..).map(|n| format!("{clean} ({n})")).find(|n| !place.join(n).exists()).unwrap_or(clean)
}
