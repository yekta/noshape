//! The user's sheet metal forms library (live 2497: a folder's Set library ▸ My sheet metal forms) and frame profiles
//! library (live 2530: ▸ My frame profiles library): every Part Studio of every document in the folder and its
//! subfolders, at its latest version, listed in the Form / Frame picker's Libraries tab as "‹user› sheet metal forms"
//! / "‹user› frame profiles". Forms are checked as forms; frame profiles aren't checked (an untagged one is kept).

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
    /// A frame profile's sketch: the one a Frame Tag names, else the Part Studio's first sketch.
    #[serde(default)]
    pub sketch: Option<noshape_model::FeatureId>,
}

impl LibraryDocument {
    /// A form's reference value: the Part Studio at the library's version (the Form's `formPartStudio`).
    pub fn value(&self, tab: &str) -> String {
        crate::derived::ExternalRef { path: self.path.clone(), microversion: self.microversion, tab: tab.to_string() }.key()
    }

    /// A frame profile's value: its sketch in that Part Studio (the Frame's `profileSketch`).
    pub fn profile_value(&self, form: &LibraryForm) -> Option<String> {
        Some(format!("{}#F{:x}", self.value(&form.tab), form.sketch?.0))
    }
}

impl FormsLibrary {
    /// The document and form a Form's `formPartStudio` or a Frame's `profileSketch` (its Part Studio, at any
    /// version) is from.
    pub fn find(&self, value: &str) -> Option<(&LibraryDocument, &LibraryForm)> {
        let base = value.split('\t').next().unwrap_or(value);
        // (A frame profile's value ends in its sketch, "#F‹id›".)
        let base = base.rsplit_once("#F").filter(|(_, f)| !f.is_empty() && f.chars().all(|c| c.is_ascii_hexdigit())).map_or(base, |(k, _)| k);
        let r = crate::derived::ExternalRef::parse(base)?;
        let tab = r.tab.split('#').next().unwrap_or(&r.tab);
        let doc = self.documents.iter().find(|d| d.path == r.path)?;
        Some((doc, doc.forms.iter().find(|f| f.tab == tab)?))
    }
}

/// Which of the user's Part Studio libraries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Forms,
    Frames,
}

impl Kind {
    /// Its key among the Set library ▸ libraries.
    pub fn key(self) -> &'static str {
        match self {
            Kind::Forms => "sheet_metal_forms",
            Kind::Frames => "frame_profiles",
        }
    }

    fn file(self) -> &'static str {
        match self {
            Kind::Forms => "sheet-metal-forms-library.json",
            Kind::Frames => "frame-profiles-library.json",
        }
    }
}

fn path(kind: Kind) -> PathBuf {
    noshape_scene::config_dir().join(kind.file())
}

/// The library's name in the picker (live: "‹the user's full name› sheet metal forms" / "… frame profiles").
pub fn title(kind: Kind) -> String {
    let what = match kind {
        Kind::Forms => "sheet metal forms",
        Kind::Frames => "frame profiles",
    };
    format!("{} {what}", noshape_store::local_user_name())
}

/// The library, if a folder is set as it.
pub fn load(kind: Kind) -> Option<FormsLibrary> {
    let folder = crate::prefs::LibraryFolders::load().0.get(kind.key()).cloned()?;
    last(kind).filter(|lib| lib.folder == folder)
}

/// The library last built, set or since unset (a pick from an unset one shows "Library not available").
pub fn last(kind: Kind) -> Option<FormsLibrary> {
    serde_json::from_slice(&std::fs::read(path(kind)).ok()?).ok()
}

/// Builds the library from `folder` (Set library and Update my library). Forms: every document's Part Studios at
/// its latest version must be forms, or nothing changes and the problems come back in live's words, one per problem.
/// Frame profiles: every Part Studio, unchecked (live 2530).
pub fn build(kind: Kind, folder: &Path) -> Result<FormsLibrary, Vec<String>> {
    let mut docs = Vec::new();
    collect(folder, &mut Vec::new(), &mut docs);
    if docs.is_empty() {
        return Err(vec![empty_folder(folder)]);
    }
    let mut problems = Vec::new();
    let mut documents = Vec::new();
    for (folders, path) in docs {
        let Some((info, version, microversion)) = latest(&path) else {
            problems.push(format!("Document \"{}\" could not be read.", path.file_stem().unwrap_or_default().to_string_lossy()));
            continue;
        };
        let Ok(doc) = crate::derived::external_snapshot(&path, microversion) else {
            problems.push(format!("Document \"{}\" could not be read at its latest version \"{version}\".", info.name));
            continue;
        };
        let mut forms = Vec::new();
        for tab in &doc.tabs {
            let TabContent::PartStudio(ps) = &tab.content else { continue };
            if kind == Kind::Frames {
                let tagged = ps.features.iter().filter(|f| !f.suppressed && f.type_id == "tag" && f.params.is("tagPurpose", "FRAME")).find_map(|f| f.params.query("sketch").first()?.depends_on());
                let sketch = tagged.or_else(|| ps.features.iter().find(|f| !f.suppressed && f.type_id == "sketch").map(|f| f.id));
                forms.push(LibraryForm { tab: tab.id.to_string(), name: tab.name.clone(), configuration: ps.configuration.clone(), sketch });
                continue;
            }
            let fail = |why: &str| format!("Element \"{}\" in document \"{}\" at its latest version \"{version}\" failed validation: \"{why}\". Fix and create a new version.", tab.name, info.name);
            let thickness = ps.configuration.inputs.iter().any(|i| i.id == "thickness" && matches!(i.kind, ConfigInputKind::Variable { kind: VariableKind::Length, .. }));
            if !thickness {
                problems.push(fail("The Part Studio must be driven by a length configuration variable with FeatureScript ID \"thickness\""));
            }
            let tagged = ps.features.iter().any(|f| !f.suppressed && f.type_id == "tag" && f.params.is("tagPurpose", "FORM") && (!f.params.query("positivePart").is_empty() || !f.params.query("negativePart").is_empty()));
            if !tagged {
                problems.push(fail("The Part Studio should have at least one solid body marked as positive or negative"));
            }
            forms.push(LibraryForm { tab: tab.id.to_string(), name: tab.name.clone(), configuration: ps.configuration.clone(), sketch: None });
        }
        documents.push(LibraryDocument { folders, name: info.name, path, version, microversion, forms });
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let version = load(kind).filter(|l| l.folder == folder).map_or(1, |l| l.version + 1);
    Ok(FormsLibrary { folder: folder.to_path_buf(), version, documents })
}

/// Live 2508: a library folder with no document.
fn empty_folder(folder: &Path) -> String {
    format!("Folder \"{}\" is empty.", folder.file_name().unwrap_or_default().to_string_lossy())
}

/// The newest named version of a document, else its first state ("Start").
fn latest(path: &Path) -> Option<(crate::derived::ExternalDocInfo, String, noshape_store::MicroversionId)> {
    let info = crate::derived::external_doc_info(path).ok()?;
    let (label, mv) = match info.versions.iter().find(|v| v.label != "Current state") {
        Some(v) => (v.label.clone(), v.microversion),
        None => ("Start".to_string(), info.start?),
    };
    Some((info, label, mv))
}

/// Checks a folder set as one of the other libraries (live 2508): it must hold a document; My images needs a
/// versioned image in each ("Document "…" has 0 versioned images. One versioned image is required.").
pub fn check_folder(key: &str, folder: &Path) -> Result<(), Vec<String>> {
    let mut docs = Vec::new();
    collect(folder, &mut Vec::new(), &mut docs);
    if docs.is_empty() {
        return Err(vec![empty_folder(folder)]);
    }
    if key != "images" {
        return Ok(());
    }
    let image = |n: &str| matches!(Path::new(n).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" | "svg" | "webp"));
    let problems: Vec<String> = docs
        .iter()
        .filter_map(|(_, path)| {
            let (info, label, mv) = latest(path)?;
            let doc = (label != "Start").then(|| crate::derived::external_snapshot(path, mv).ok()).flatten();
            let n = doc.map_or(0, |d| d.tabs.iter().filter(|t| matches!(&t.content, TabContent::File(f) if image(&f.file_name))).count());
            (n == 0).then(|| format!("Document \"{}\" has 0 versioned images. One versioned image is required.", info.name))
        })
        .collect();
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

impl FormsLibrary {
    /// Built from the same documents at the same versions (Update my library: "already up to date").
    pub fn same_as(&self, other: &FormsLibrary) -> bool {
        self.folder == other.folder && self.documents.len() == other.documents.len() && self.documents.iter().zip(&other.documents).all(|(a, b)| a.path == b.path && a.microversion == b.microversion)
    }
}

/// Saves a built library as the user's.
pub fn save(kind: Kind, lib: &FormsLibrary) -> std::io::Result<()> {
    std::fs::write(path(kind), serde_json::to_vec_pretty(lib).unwrap_or_default())
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
