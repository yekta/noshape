//! The user's sheet metal forms library (live 2497: a folder's Set library ▸ My sheet metal forms): every Part Studio
//! of every document in the folder and its subfolders, at its latest version, checked as a form and listed in the
//! Form picker's Libraries tab as "‹user› sheet metal forms".

use noshape_model::configuration::{ConfigInputKind, Configuration, VariableKind};
use noshape_model::document::TabContent;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A built library: what the picker lists, as each document was at the version it was built from.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FormsLibrary {
    pub folder: PathBuf,
    /// Built this many times (live's library version "V1", "V2" … after each Update).
    pub version: u32,
    pub documents: Vec<LibraryDocument>,
}

/// One document of the library at its latest version.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryDocument {
    /// The folders between the library's folder and the document (live's Standard levels).
    pub folders: Vec<String>,
    pub name: String,
    pub path: PathBuf,
    /// The version it was built from ("Start" without a named one).
    pub version: String,
    pub microversion: noshape_store::MicroversionId,
    /// Its Part Studios (the forms).
    pub forms: Vec<LibraryForm>,
}

/// A form: a Part Studio of a library document.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryForm {
    pub tab: String,
    pub name: String,
    /// Its configuration inputs (the picker's fields under Form).
    pub configuration: Configuration,
}

impl LibraryDocument {
    /// A form's reference value: the Part Studio at the library's version (the Form's `formPartStudio`).
    pub fn value(&self, tab: &str) -> String {
        crate::derived::ExternalRef { path: self.path.clone(), microversion: self.microversion, tab: tab.to_string() }.key()
    }
}

impl FormsLibrary {
    /// The document and form a Form's `formPartStudio` (its Part Studio, at any version) is from.
    pub fn find(&self, value: &str) -> Option<(&LibraryDocument, &LibraryForm)> {
        let r = crate::derived::ExternalRef::parse(value.split('\t').next().unwrap_or(value))?;
        let tab = r.tab.split('#').next().unwrap_or(&r.tab);
        let doc = self.documents.iter().find(|d| d.path == r.path)?;
        Some((doc, doc.forms.iter().find(|f| f.tab == tab)?))
    }
}

fn path() -> PathBuf {
    noshape_scene::config_dir().join("sheet-metal-forms-library.json")
}

/// The library's name in the picker (live: "‹the user's full name› sheet metal forms").
pub fn title() -> String {
    format!("{} sheet metal forms", noshape_store::local_user_name())
}

/// The library, if a folder is set as it.
pub fn load() -> Option<FormsLibrary> {
    let folder = crate::prefs::LibraryFolders::load().0.get("sheet_metal_forms").cloned()?;
    last().filter(|lib| lib.folder == folder)
}

/// The library last built, set or since unset (a Form picked from an unset one shows "Library not available").
pub fn last() -> Option<FormsLibrary> {
    serde_json::from_slice(&std::fs::read(path()).ok()?).ok()
}

/// Builds the library from `folder` (Set library and Update my library): every document's Part Studios at its latest
/// version must be forms, or nothing changes and the problems come back in live's words, one per problem.
pub fn build(folder: &Path) -> Result<FormsLibrary, Vec<String>> {
    let mut docs = Vec::new();
    collect(folder, &mut Vec::new(), &mut docs);
    let mut problems = Vec::new();
    let mut documents = Vec::new();
    for (folders, path) in docs {
        let Ok(info) = crate::derived::external_doc_info(&path) else {
            problems.push(format!("Document \"{}\" could not be read.", path.file_stem().unwrap_or_default().to_string_lossy()));
            continue;
        };
        let (version, microversion) = match info.versions.iter().find(|v| v.label != "Current state") {
            Some(v) => (v.label.clone(), v.microversion),
            None => match info.start {
                Some(mv) => ("Start".to_string(), mv),
                None => continue,
            },
        };
        let Ok(doc) = crate::derived::external_snapshot(&path, microversion) else {
            problems.push(format!("Document \"{}\" could not be read at its latest version \"{version}\".", info.name));
            continue;
        };
        let mut forms = Vec::new();
        for tab in &doc.tabs {
            let TabContent::PartStudio(ps) = &tab.content else { continue };
            let fail = |why: &str| format!("Element \"{}\" in document \"{}\" at its latest version \"{version}\" failed validation: \"{why}\". Fix and create a new version.", tab.name, info.name);
            let thickness = ps.configuration.inputs.iter().any(|i| i.id == "thickness" && matches!(i.kind, ConfigInputKind::Variable { kind: VariableKind::Length, .. }));
            if !thickness {
                problems.push(fail("The Part Studio must be driven by a length configuration variable with FeatureScript ID \"thickness\""));
            }
            let tagged = ps.features.iter().any(|f| !f.suppressed && f.type_id == "tag" && f.params.is("tagPurpose", "FORM") && (!f.params.query("positivePart").is_empty() || !f.params.query("negativePart").is_empty()));
            if !tagged {
                problems.push(fail("The Part Studio should have at least one solid body marked as positive or negative"));
            }
            forms.push(LibraryForm { tab: tab.id.to_string(), name: tab.name.clone(), configuration: ps.configuration.clone() });
        }
        documents.push(LibraryDocument { folders, name: info.name, path, version, microversion, forms });
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let version = load().filter(|l| l.folder == folder).map_or(1, |l| l.version + 1);
    Ok(FormsLibrary { folder: folder.to_path_buf(), version, documents })
}

/// Saves a built library as the user's.
pub fn save(lib: &FormsLibrary) -> std::io::Result<()> {
    std::fs::write(path(), serde_json::to_vec_pretty(lib).unwrap_or_default())
}

/// The `.noshape` documents under `dir`, with the folders between, by name.
fn collect(dir: &Path, folders: &mut Vec<String>, out: &mut Vec<(Vec<String>, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            folders.push(name);
            collect(&p, folders, out);
            folders.pop();
        } else if p.extension().is_some_and(|e| e == "noshape") {
            out.push((folders.clone(), p));
        }
    }
}
