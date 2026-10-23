//! Release management (Onshape "Release…", local: no approvers or
//! notifications). A release package lists items (parts, assemblies,
//! drawings) with their part number and revision; releasing records it
//! against a version of the document. Revisions follow the default
//! alphabetical scheme (A, B, C… skipping I O Q S X Z); a new revision of a
//! part number obsoletes the previous one automatically, and a revision can
//! be obsoleted on its own (an obsoletion package). The log is kept per
//! document, outside the editable history.

use crate::document::TabId;
use crate::reference::BodyId;
use serde::{Deserialize, Serialize};

/// Letters of the default revision scheme ("Alphabetical A B C skipping
/// I O Q S X Z").
const LETTERS: [char; 20] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'R', 'T', 'U', 'V', 'W', 'Y'];

/// Position of a revision label in the scheme (A = 0 … Y = 19, AA = 20 …),
/// `None` when it isn't a label of the scheme.
pub fn label_index(label: &str) -> Option<u64> {
    if label.is_empty() {
        return None;
    }
    let digits: Option<Vec<u64>> = label.chars().map(|c| LETTERS.iter().position(|l| *l == c).map(|p| p as u64)).collect();
    let digits = digits?;
    // Bijective base 20: A…Y, then AA…AY, BA… (like spreadsheet columns).
    let mut n = 0u64;
    for d in &digits {
        n = n * 20 + d + 1;
    }
    Some(n - 1)
}

/// The label at a position of the scheme.
pub fn label_at(mut index: u64) -> String {
    let mut out = Vec::new();
    loop {
        out.push(LETTERS[(index % 20) as usize]);
        if index < 20 {
            break;
        }
        index = index / 20 - 1;
    }
    out.iter().rev().collect()
}

/// The revision after `prev` (A for a first release).
pub fn next_label(prev: Option<&str>) -> String {
    match prev.and_then(label_index) {
        Some(i) => label_at(i + 1),
        None => label_at(0),
    }
}

/// Revision schemes of Release management settings (Revisions and part
/// numbers › Revision scheme; Help: "Alphabetical, Numeric, Custom").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum RevisionScheme {
    /// "Alphabetical A B C skipping I O Q S X Z" (Onshape's default).
    #[default]
    Alphabetical,
    /// Numbers, zero-padded to `width` digits ("1 2 3", "01 02 03"…).
    Numeric { width: u8 },
    /// The labels in order.
    Custom(Vec<String>),
}

impl RevisionScheme {
    /// The settings select, in order (custom last).
    pub const CHOICES: [&'static str; 5] = ["Alphabetical A B C skipping I O Q S X Z", "Numeric 1 2 3", "Numeric 01 02 03", "Numeric 001 002 003", "Custom"];

    pub fn label(&self) -> &'static str {
        match self {
            RevisionScheme::Alphabetical => Self::CHOICES[0],
            RevisionScheme::Numeric { width: 2 } => Self::CHOICES[2],
            RevisionScheme::Numeric { width: 3 } => Self::CHOICES[3],
            RevisionScheme::Numeric { .. } => Self::CHOICES[1],
            RevisionScheme::Custom(_) => Self::CHOICES[4],
        }
    }

    /// Position of a label in the scheme, `None` when it isn't one.
    pub fn index(&self, label: &str) -> Option<u64> {
        match self {
            RevisionScheme::Alphabetical => label_index(label),
            RevisionScheme::Numeric { width } => {
                let n: u64 = label.parse().ok()?;
                // Labels start at 1 and are written with at least `width` digits.
                (n >= 1 && label.len() >= usize::from(*width) && (label.len() == usize::from(*width) || !label.starts_with('0'))).then_some(n - 1)
            }
            RevisionScheme::Custom(list) => list.iter().position(|l| l == label).map(|p| p as u64),
        }
    }

    /// The label at a position (custom schemes past their end: none).
    pub fn at(&self, index: u64) -> Option<String> {
        match self {
            RevisionScheme::Alphabetical => Some(label_at(index)),
            RevisionScheme::Numeric { width } => Some(format!("{:0w$}", index + 1, w = usize::from(*width))),
            RevisionScheme::Custom(list) => list.get(index as usize).cloned(),
        }
    }

    /// The revision after `prev` (the first label for a first release).
    pub fn next(&self, prev: Option<&str>) -> String {
        let i = prev.and_then(|p| self.index(p)).map_or(0, |i| i + 1);
        self.at(i).unwrap_or_default()
    }
}

/// What an item is (the release dialog's type column).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Part,
    Assembly,
    Drawing,
    /// Release__typical_release_workflow: "For any tab (Part Studio,
    /// Assembly, Drawing, Variable Studio, or imported file)".
    VariableStudio,
    File,
}

impl ItemKind {
    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Part => "Part",
            ItemKind::Assembly => "Assembly",
            ItemKind::Drawing => "Drawing",
            ItemKind::VariableStudio => "Variable Studio",
            ItemKind::File => "File",
        }
    }
}

/// One released (or obsoleted) item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReleaseItem {
    pub tab: TabId,
    /// The part, for part items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<BodyId>,
    pub name: String,
    pub kind: ItemKind,
    pub part_number: String,
    pub revision: String,
    /// Properties › "Not revision managed": released without a revision.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub not_revision_managed: bool,
    /// Reverted to its last revision (unchanged since): listed for
    /// reference, not released again (Help: "Reverting items to their last
    /// revision").
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reverted: bool,
    /// The configuration of a configured part or assembly: each one is its
    /// own revisionable object (Help: Releasing a configuration).
    #[serde(default, skip_serializing_if = "crate::configuration::ConfigValues::is_empty")]
    pub config: crate::configuration::ConfigValues,
    /// Linked from a version (Add Items › a version of this document, or
    /// Other documents): the item is that version's (Help
    /// Release__selecting_parts_for_a_release_candidate: "an icon appears
    /// beside the added item indicating it is linked from another version").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<LinkedDocument>,
}

/// The document version a linked release item comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedDocument {
    /// The document file; empty for a version of the releasing document.
    pub path: String,
    pub name: String,
    /// The version's microversion (store id as text) and name.
    pub version: String,
    pub version_name: String,
}

impl ReleaseItem {
    /// The same object (tab, part, type, configuration and source).
    pub fn same_object(&self, o: &ReleaseItem) -> bool {
        self.tab == o.tab && self.part == o.part && self.kind == o.kind && self.config == o.config && self.document == o.document
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageKind {
    Release,
    Obsoletion,
}

/// A release or obsoletion package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Package {
    pub id: u64,
    pub kind: PackageKind,
    pub name: String,
    pub notes: String,
    /// Milliseconds since the epoch.
    pub created: i64,
    /// The version it was released at (store ids as text).
    pub version: Option<String>,
    pub items: Vec<ReleaseItem>,
    /// Obsoletion: the revisions may be released again.
    #[serde(default)]
    pub re_releasable: bool,
}

/// A revision's place in a part number's history.
#[derive(Clone, Debug, PartialEq)]
pub struct RevisionEntry {
    pub revision: String,
    /// The release package (index into [`ReleaseLog::packages`]).
    pub release: usize,
    /// Obsoleted: by an obsoletion package (`Some(index)`), or automatically by
    /// a later revision (`auto`).
    pub obsoleted_by: Option<usize>,
    pub auto_obsolete: bool,
    pub re_releasable: bool,
}

/// One row of Revision tools' search: a revision of a part number released
/// as one type.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedRevision {
    pub revision: String,
    pub release_name: String,
    /// Milliseconds since the epoch.
    pub created: i64,
    pub state: ItemState,
    /// Obsoleted as re-releasable: the part number may be released again
    /// (and with another type: "Can change type?").
    pub re_releasable: bool,
}

/// Release state of an item (Properties › State).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemState {
    InProgress,
    Released,
    Obsolete,
}

impl ItemState {
    pub fn label(self) -> &'static str {
        match self {
            ItemState::InProgress => "In progress",
            ItemState::Released => "Released",
            ItemState::Obsolete => "Obsolete",
        }
    }
}

/// Every package of a document, oldest first, plus saved drafts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReleaseLog {
    pub packages: Vec<Package>,
    #[serde(default)]
    pub drafts: Vec<Package>,
    #[serde(default)]
    next_id: u64,
    /// The revision scheme of the Release management settings (not stored
    /// with the log; set by the app).
    #[serde(skip)]
    pub scheme: RevisionScheme,
    /// Numbering schemes › Part number uniqueness: "Drawing can reuse part
    /// number from an assembly or part in the release" (set by the app).
    #[serde(skip)]
    pub drawing_reuse: bool,
    /// Release management › Obsoletion › "Automatically obsolete previous
    /// released revisions" (set by the app): a new revision obsoletes the
    /// earlier ones.
    #[serde(skip)]
    pub auto_obsolete: bool,
}

/// A problem with an item of a candidate ("N item(s) needs attention").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Issue {
    /// The part number is empty (required).
    MissingPartNumber,
    /// The revision isn't a label of the scheme.
    InvalidRevision,
    /// Lower than (or equal to) a released revision of the part number.
    RevisionTooLow { latest: String },
    /// The same part number appears twice in the candidate.
    DuplicatePartNumber,
    /// A Release management › Release condition holds (its message).
    Prevented(String),
}

impl Issue {
    pub fn message(&self) -> String {
        match self {
            Issue::MissingPartNumber => "Required field must not be empty.".into(),
            Issue::InvalidRevision => "Revision does not follow the revision scheme.".into(),
            Issue::RevisionTooLow { latest } => format!("Revision must be later than {latest}, the latest released revision."),
            Issue::DuplicatePartNumber => "Part number is used by another item of this release.".into(),
            Issue::Prevented(message) => message.clone(),
        }
    }
}

impl ReleaseLog {
    pub fn from_json(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap_or_default()
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The history of a part number, oldest first.
    pub fn revisions(&self, part_number: &str) -> Vec<RevisionEntry> {
        let mut out: Vec<RevisionEntry> = Vec::new();
        for (pi, p) in self.packages.iter().enumerate() {
            for it in p.items.iter().filter(|it| it.part_number == part_number && !it.not_revision_managed && !it.reverted) {
                match p.kind {
                    PackageKind::Release => {
                        // A newer revision obsoletes the earlier ones (when the
                        // company settings say so).
                        if self.auto_obsolete {
                            for e in out.iter_mut().filter(|e| e.obsoleted_by.is_none() && !e.auto_obsolete) {
                                e.auto_obsolete = true;
                            }
                        }
                        out.push(RevisionEntry { revision: it.revision.clone(), release: pi, obsoleted_by: None, auto_obsolete: false, re_releasable: false });
                    }
                    PackageKind::Obsoletion => {
                        if let Some(e) = out.iter_mut().rev().find(|e| e.revision == it.revision) {
                            e.obsoleted_by = Some(pi);
                            e.re_releasable = p.re_releasable;
                        }
                    }
                }
            }
        }
        out
    }

    /// The latest released revision of a part number.
    pub fn latest_revision(&self, part_number: &str) -> Option<String> {
        self.revisions(part_number).last().map(|e| e.revision.clone())
    }

    /// The revision a new release of the part number gets.
    pub fn next_revision(&self, part_number: &str) -> String {
        let hist = self.revisions(part_number);
        // Labels obsoleted as re-releasable can be used again.
        match hist.iter().rev().find(|e| !(e.obsoleted_by.is_some() && e.re_releasable)) {
            Some(e) => self.scheme.next(Some(&e.revision)),
            None => hist.last().map_or_else(|| self.scheme.next(None), |e| e.revision.clone()),
        }
    }

    /// State of a part number (in a workspace: In progress unless the
    /// latest revision is obsolete).
    pub fn state(&self, part_number: &str) -> ItemState {
        match self.revisions(part_number).last() {
            Some(e) if e.obsoleted_by.is_some() => ItemState::Obsolete,
            Some(_) => ItemState::Released,
            None => ItemState::InProgress,
        }
    }

    /// Problems of each item of a candidate.
    pub fn validate(&self, items: &[ReleaseItem]) -> Vec<Vec<Issue>> {
        items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let mut v = Vec::new();
                if it.part_number.trim().is_empty() {
                    v.push(Issue::MissingPartNumber);
                    return v;
                }
                // A drawing may share the number of a part or assembly when
                // the numbering scheme allows it.
                let reuse = |a: &ReleaseItem, b: &ReleaseItem| self.drawing_reuse && (a.kind == ItemKind::Drawing) != (b.kind == ItemKind::Drawing) && [a.kind, b.kind].iter().all(|k| matches!(k, ItemKind::Drawing | ItemKind::Part | ItemKind::Assembly));
                if items.iter().enumerate().any(|(j, o)| j != i && o.part_number == it.part_number && !reuse(it, o)) {
                    v.push(Issue::DuplicatePartNumber);
                }
                if it.not_revision_managed || it.reverted {
                    return v;
                }
                match self.scheme.index(&it.revision) {
                    None => v.push(Issue::InvalidRevision),
                    Some(r) => {
                        let hist = self.revisions(&it.part_number);
                        let blocking = hist.iter().filter(|e| !(e.obsoleted_by.is_some() && e.re_releasable)).filter_map(|e| self.scheme.index(&e.revision)).max();
                        if let Some(max) = blocking
                            && r <= max
                        {
                            v.push(Issue::RevisionTooLow { latest: self.scheme.at(max).unwrap_or_default() });
                        }
                    }
                }
                v
            })
            .collect()
    }

    /// Record a release (or obsoletion) package; returns its id. The caller
    /// checks [`Self::validate`] first.
    pub fn record(&mut self, mut package: Package) -> u64 {
        self.next_id = self.next_id.max(self.packages.iter().chain(&self.drafts).map(|p| p.id).max().unwrap_or(0)) + 1;
        package.id = self.next_id;
        self.drafts.retain(|d| d.name != package.name || d.kind != package.kind);
        self.packages.push(package);
        self.next_id
    }

    /// Delete a saved draft (Review Release › Save draft ▴ › Delete draft).
    pub fn delete_draft(&mut self, name: &str) {
        self.drafts.retain(|d| d.name != name);
    }

    /// Save (or replace, by name) a draft.
    pub fn save_draft(&mut self, draft: Package) {
        match self.drafts.iter_mut().find(|d| d.name == draft.name) {
            Some(d) => *d = draft,
            None => self.drafts.push(draft),
        }
    }

    /// The version of the release that released `revision` of `part_number`
    /// (an obsoletion marks that version, Release__typical_release_workflow:
    /// "the version is marked with a solid triangle with a dot").
    pub fn release_version_of(&self, part_number: &str, revision: &str) -> Option<String> {
        self.packages
            .iter()
            .rev()
            .filter(|p| p.kind == PackageKind::Release)
            .find(|p| p.items.iter().any(|i| i.part_number == part_number && i.revision == revision && !i.reverted))
            .and_then(|p| p.version.clone())
    }

    /// The packages released at a version.
    pub fn at_version(&self, version: &str) -> Vec<&Package> {
        self.packages.iter().filter(|p| p.version.as_deref() == Some(version)).collect()
    }

    /// Company settings › Revision tools › Find revisions: the revisions of
    /// `part_number` released as `kind`, newest first (Help revision_tools).
    pub fn typed_revisions(&self, part_number: &str, kind: ItemKind) -> Vec<TypedRevision> {
        let hist = self.revisions(part_number);
        let mut out: Vec<TypedRevision> = Vec::new();
        for e in &hist {
            let p = &self.packages[e.release];
            if !p.items.iter().any(|i| i.part_number == part_number && i.kind == kind && i.revision == e.revision && !i.reverted && !i.not_revision_managed) {
                continue;
            }
            out.push(TypedRevision {
                revision: e.revision.clone(),
                release_name: p.name.clone(),
                created: p.created,
                state: if e.obsoleted_by.is_some() { ItemState::Obsolete } else { ItemState::Released },
                re_releasable: e.obsoleted_by.is_some() && e.re_releasable,
            });
        }
        out.reverse();
        out
    }

    /// Revision tools › Reset part number type: obsoletes every revision of
    /// `part_number` released as `kind` that isn't already, as re-releasable,
    /// so the number can be released again with another type. Returns
    /// whether a package was recorded.
    pub fn reset_part_number_type(&mut self, part_number: &str, kind: ItemKind, created: i64) -> bool {
        let open: Vec<String> = self.typed_revisions(part_number, kind).into_iter().filter(|r| !r.re_releasable).map(|r| r.revision).collect();
        let items: Vec<ReleaseItem> = open
            .iter()
            .filter_map(|rev| self.packages.iter().rev().filter(|p| p.kind == PackageKind::Release).flat_map(|p| &p.items).find(|i| i.part_number == part_number && i.kind == kind && &i.revision == rev).cloned())
            .collect();
        if items.is_empty() {
            return false;
        }
        self.record(Package { id: 0, kind: PackageKind::Obsoletion, name: format!("Reset part number type {part_number}"), notes: String::new(), created, version: None, items, re_releasable: true });
        true
    }

    /// A suggested name for the next release ("Release 1", "Release 2"…).
    pub fn suggested_name(&self) -> String {
        let n = self.packages.iter().filter(|p| p.kind == PackageKind::Release).count() + 1;
        format!("Release {n}")
    }
}

/// Company settings › Numbering schemes (Help Plans/numbering_schemes,
/// accounts/co-numschemes-03.png): one tab per object type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumberingSchemes {
    pub release_objects: NumberScheme,
    pub standard_content: NumberScheme,
    pub items: NumberScheme,
    pub release_names: NumberScheme,
    /// Part number uniqueness: "Drawing can reuse part number from an
    /// assembly or part in the release" (else all must be unique).
    #[serde(default)]
    pub drawing_reuse: bool,
    #[serde(default)]
    pub propagation: Propagation,
}

impl Default for NumberingSchemes {
    fn default() -> Self {
        Self {
            release_objects: NumberScheme::new(&["", "", "", "", ""]),
            standard_content: NumberScheme::new(&[""]),
            items: NumberScheme::new(&[""]),
            release_names: NumberScheme::new(&[""]),
            drawing_reuse: false,
            propagation: Propagation::Never,
        }
    }
}

/// Part number propagation (Release objects only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Propagation {
    #[default]
    Never,
    OneWorkspace,
    AllWorkspaces,
}

impl Propagation {
    pub const ALL: [Propagation; 3] = [Propagation::Never, Propagation::OneWorkspace, Propagation::AllWorkspaces];

    pub fn label(self) -> &'static str {
        match self {
            Propagation::Never => "Never",
            Propagation::OneWorkspace => "One workspace",
            Propagation::AllWorkspaces => "All workspaces",
        }
    }
}

/// One tab of the Numbering schemes: Manual entry or Sequential part
/// number generation with a prefix per object type, a minimum length and
/// the next number. Object types of a tab share the counter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumberScheme {
    pub sequential: bool,
    pub prefixes: Vec<String>,
    pub min_length: usize,
    pub next: u64,
}

impl NumberScheme {
    fn new(prefixes: &[&str]) -> Self {
        Self { sequential: false, prefixes: prefixes.iter().map(|p| p.to_string()).collect(), min_length: 3, next: 1 }
    }

    /// The number the next generation gives the object type `ty` (index
    /// into the prefixes): "PRT-062".
    pub fn preview(&self, ty: usize) -> String {
        let prefix = self.prefixes.get(ty).map(String::as_str).unwrap_or_default();
        format!("{prefix}{:0width$}", self.next, width = self.min_length)
    }

    /// Generates a number for `ty` and advances the shared counter (numbers
    /// never go back, even when they are not used).
    pub fn generate(&mut self, ty: usize) -> Option<String> {
        if !self.sequential {
            return None;
        }
        let n = self.preview(ty);
        self.next += 1;
        Some(n)
    }
}

impl ItemKind {
    /// Index of the kind's prefix in the Release objects scheme (Parts,
    /// Assemblies, Drawings, Variable Studios, Files).
    pub fn prefix_index(self) -> usize {
        match self {
            ItemKind::Part => 0,
            ItemKind::Assembly => 1,
            ItemKind::Drawing => 2,
            ItemKind::VariableStudio => 3,
            ItemKind::File => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_and_custom_revision_schemes() {
        let n = RevisionScheme::Numeric { width: 2 };
        assert_eq!(n.next(None), "01");
        assert_eq!(n.next(Some("09")), "10");
        assert_eq!(n.index("7"), None);
        let n1 = RevisionScheme::Numeric { width: 1 };
        assert_eq!(n1.next(Some("9")), "10");
        let c = RevisionScheme::Custom(vec!["X1".into(), "X2".into()]);
        assert_eq!(c.next(None), "X1");
        assert_eq!(c.next(Some("X1")), "X2");
        assert_eq!(c.next(Some("X2")), "");
        let mut log = ReleaseLog { scheme: n, ..Default::default() };
        assert_eq!(log.next_revision("PN"), "01");
        log.record(Package { id: 0, kind: PackageKind::Release, name: "R".into(), notes: String::new(), created: 0, version: None, items: vec![item("PN", "01")], re_releasable: false });
        assert_eq!(log.next_revision("PN"), "02");
        assert!(log.validate(&[item("PN", "01")])[0].iter().any(|i| matches!(i, Issue::RevisionTooLow { .. })));
        assert!(log.validate(&[item("PN", "A")])[0].contains(&Issue::InvalidRevision));
    }

    fn item(pn: &str, rev: &str) -> ReleaseItem {
        ReleaseItem { tab: TabId(1), part: None, name: "Part 1".into(), kind: ItemKind::Part, part_number: pn.into(), revision: rev.into(), not_revision_managed: false, reverted: false, config: Default::default(), document: None }
    }

    fn package(kind: PackageKind, name: &str, items: Vec<ReleaseItem>) -> Package {
        Package { id: 0, kind, name: name.into(), notes: String::new(), created: 0, version: Some(format!("v-{name}")), items, re_releasable: false }
    }

    #[test]
    fn revision_scheme_skips_letters() {
        assert_eq!(next_label(None), "A");
        assert_eq!(next_label(Some("H")), "J");
        assert_eq!(next_label(Some("N")), "P");
        assert_eq!(next_label(Some("R")), "T");
        assert_eq!(next_label(Some("W")), "Y");
        assert_eq!(next_label(Some("Y")), "AA");
        assert_eq!(next_label(Some("AY")), "BA");
        assert!(label_index("I").is_none() && label_index("O").is_none() && label_index("").is_none());
        for i in 0..500 {
            assert_eq!(label_index(&label_at(i)), Some(i));
        }
    }

    #[test]
    fn releases_obsolete_and_validate() {
        let mut log = ReleaseLog::default();
        assert_eq!(log.next_revision("P-1"), "A");
        assert_eq!(log.state("P-1"), ItemState::InProgress);
        log.record(package(PackageKind::Release, "R1", vec![item("P-1", "A")]));
        assert_eq!(log.latest_revision("P-1").as_deref(), Some("A"));
        assert_eq!(log.next_revision("P-1"), "B");
        assert_eq!(log.state("P-1"), ItemState::Released);
        // Releasing A again, or an earlier label, is refused; a later one is fine.
        assert_eq!(log.validate(&[item("P-1", "A")]), vec![vec![Issue::RevisionTooLow { latest: "A".into() }]]);
        assert!(log.validate(&[item("P-1", "C")])[0].is_empty());
        assert_eq!(log.validate(&[item("", "A")]), vec![vec![Issue::MissingPartNumber]]);
        assert_eq!(log.validate(&[item("P-1", "I")]), vec![vec![Issue::InvalidRevision]]);
        assert_eq!(log.validate(&[item("Q", "A"), item("Q", "A")])[0], vec![Issue::DuplicatePartNumber]);
        // B leaves A released, or auto-obsoletes it when the settings say so.
        log.record(package(PackageKind::Release, "R2", vec![item("P-1", "B")]));
        assert!(!log.revisions("P-1")[0].auto_obsolete);
        log.auto_obsolete = true;
        let h = log.revisions("P-1");
        assert!(h[0].auto_obsolete && !h[1].auto_obsolete);
        // Obsoleting B (re-releasable) makes B available again.
        let mut ob = package(PackageKind::Obsoletion, "O1", vec![item("P-1", "B")]);
        ob.re_releasable = true;
        log.record(ob);
        assert_eq!(log.state("P-1"), ItemState::Obsolete);
        assert_eq!(log.next_revision("P-1"), "B");
        assert!(log.validate(&[item("P-1", "B")])[0].is_empty());
        // JSON round trip; drafts are replaced by name and cleared on release.
        let mut log2 = ReleaseLog::from_json(&log.to_json());
        // The settings' flags aren't stored with the log.
        log2.auto_obsolete = log.auto_obsolete;
        assert_eq!(log2, log);
        log2.save_draft(package(PackageKind::Release, "R3", vec![item("P-2", "A")]));
        log2.save_draft(package(PackageKind::Release, "R3", vec![item("P-2", "B")]));
        assert_eq!(log2.drafts.len(), 1);
        log2.record(package(PackageKind::Release, "R3", vec![item("P-2", "A")]));
        assert!(log2.drafts.is_empty());
        assert_eq!(log2.at_version("v-R3").len(), 1);
        assert_eq!(log2.suggested_name(), "Release 4");
    }

    #[test]
    fn sequential_numbers_share_the_counter() {
        let mut n = NumberingSchemes::default();
        assert_eq!(n.release_objects.generate(0), None, "manual entry generates nothing");
        n.release_objects.sequential = true;
        n.release_objects.prefixes = ["PRT-", "ASM-", "DRW-", "VS-", "FILE-"].map(String::from).to_vec();
        n.release_objects.next = 62;
        assert_eq!(n.release_objects.preview(0), "PRT-062");
        assert_eq!(n.release_objects.generate(2).as_deref(), Some("DRW-062"));
        assert_eq!(n.release_objects.generate(0).as_deref(), Some("PRT-063"));
        n.release_objects.min_length = 1;
        assert_eq!(n.release_objects.preview(1), "ASM-64");
    }

    #[test]
    fn reset_part_number_type_obsoletes_that_type_only() {
        let item = |kind, rev: &str| ReleaseItem { tab: TabId(1), part: None, name: "x".into(), kind, part_number: "PRT-002".into(), revision: rev.into(), not_revision_managed: false, reverted: false, config: Default::default(), document: None };
        let pkg = |name: &str, items| Package { id: 0, kind: PackageKind::Release, name: name.into(), notes: String::new(), created: 1, version: None, items, re_releasable: false };
        let mut log = ReleaseLog::default();
        log.record(pkg("rel-1.0.2", vec![item(ItemKind::Part, "A")]));
        log.record(pkg("rel-A", vec![item(ItemKind::Part, "B")]));
        let found = log.typed_revisions("PRT-002", ItemKind::Part);
        assert_eq!(found.iter().map(|r| r.revision.as_str()).collect::<Vec<_>>(), ["B", "A"], "newest first");
        assert!(found.iter().all(|r| r.state == ItemState::Released && !r.re_releasable));
        assert!(log.typed_revisions("PRT-002", ItemKind::Assembly).is_empty());
        assert!(log.reset_part_number_type("PRT-002", ItemKind::Part, 2));
        let found = log.typed_revisions("PRT-002", ItemKind::Part);
        assert!(found.iter().all(|r| r.state == ItemState::Obsolete && r.re_releasable));
        assert!(!log.reset_part_number_type("PRT-002", ItemKind::Part, 3), "nothing left to reset");
        // The number can now be released again, as an assembly at A (Help
        // revision_tools' example).
        assert!(log.validate(&[item(ItemKind::Assembly, "A")])[0].is_empty());
    }

    #[test]
    fn drawings_may_reuse_a_part_number() {
        let item = |kind, pn: &str| ReleaseItem { tab: TabId(1), part: None, name: "x".into(), kind, part_number: pn.into(), revision: "A".into(), not_revision_managed: false, reverted: false, config: Default::default(), document: None };
        let items = [item(ItemKind::Part, "P1"), item(ItemKind::Drawing, "P1")];
        let mut log = ReleaseLog::default();
        assert!(log.validate(&items)[1].contains(&Issue::DuplicatePartNumber));
        log.drawing_reuse = true;
        assert!(log.validate(&items).iter().all(|v| v.is_empty()));
        let parts = [item(ItemKind::Part, "P1"), item(ItemKind::Part, "P1")];
        assert!(log.validate(&parts)[0].contains(&Issue::DuplicatePartNumber));
    }
}
