//! Integration-style tests of the document store.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("noshape-store-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type Changes<'a> = &'a [(&'a str, Option<&'a [u8]>)];

fn new_doc(dir: &TempDir) -> (Document, BranchId) {
    let doc = Document::create(
        dir.file("doc.noshape"),
        &[
            ("document/meta", b"meta-v1".as_slice()),
            ("tab/a", b"a-v1".as_slice()),
            ("tab/b", b"b-v1".as_slice()),
        ],
    )
    .unwrap();
    let main = doc.default_branch().unwrap();
    (doc, main)
}

fn set<'a>(name: &'a str, data: &'a [u8]) -> (&'a str, Option<&'a [u8]>) {
    (name, Some(data))
}

fn del(name: &str) -> (&str, Option<&[u8]>) {
    (name, None)
}

fn read(doc: &Document, mv: MicroversionId, name: &str) -> Option<String> {
    doc.read(mv, name)
        .unwrap()
        .map(|b| String::from_utf8(b).unwrap())
}

#[test]
fn create_commit_read() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    assert_eq!(read(&doc, root, "tab/a").as_deref(), Some("a-v1"));
    assert_eq!(read(&doc, root, "missing"), None);
    let info = doc.microversion(root).unwrap();
    assert!(info.parents.is_empty());
    assert_eq!(info.branch, main);
    assert_eq!(info.description, "Create document");
    assert_eq!(info.author, local_user_name());

    let c: Changes = &[set("tab/a", b"a-v2"), set("tab/c", b"c-v1")];
    let mv1 = doc.commit(main, c, "Edit Extrude 1").unwrap();
    assert_eq!(doc.head(main).unwrap(), mv1);
    assert_eq!(read(&doc, mv1, "tab/a").as_deref(), Some("a-v2"));
    assert_eq!(read(&doc, mv1, "tab/b").as_deref(), Some("b-v1"));
    assert_eq!(read(&doc, mv1, "tab/c").as_deref(), Some("c-v1"));
    // The old state is unchanged.
    assert_eq!(read(&doc, root, "tab/a").as_deref(), Some("a-v1"));
    assert_eq!(read(&doc, root, "tab/c"), None);

    let mv2 = doc.commit(main, &[del("tab/b")], "Delete tab").unwrap();
    assert_eq!(
        doc.names(mv2).unwrap(),
        vec!["document/meta", "tab/a", "tab/c"]
    );
    assert_eq!(
        doc.diff(mv1, mv2).unwrap(),
        vec![NameChange {
            name: "tab/b".into(),
            kind: ChangeKind::Removed
        }]
    );
    let info = doc.microversion(mv2).unwrap();
    assert_eq!(info.parents, vec![mv1]);
    assert_eq!(info.description, "Delete tab");
    assert!(info.created >= doc.microversion(root).unwrap().created);

    // No-op commits do not create microversions.
    let same = doc
        .commit(main, &[set("tab/a", b"a-v2"), del("nope")], "noop")
        .unwrap();
    assert_eq!(same, mv2);
    let empty: Changes = &[];
    assert_eq!(doc.commit(main, empty, "noop").unwrap(), mv2);

    let all = doc.read_all(mv2).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all["tab/c"], b"c-v1");

    // Large and empty blobs.
    let big = vec![7u8; 100_000];
    let mv3 = doc
        .commit(main, &[set("big", &big), set("empty", b"")], "Big")
        .unwrap();
    assert_eq!(doc.read(mv3, "big").unwrap().unwrap(), big);
    assert_eq!(doc.read(mv3, "empty").unwrap().unwrap(), Vec::<u8>::new());

    assert!(matches!(
        doc.microversion(MicroversionId::generate()),
        Err(StoreError::UnknownMicroversion(_))
    ));
    assert!(matches!(
        doc.head(BranchId::generate()),
        Err(StoreError::UnknownBranch(_))
    ));
}

#[test]
fn commit_accepts_owned_changes() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let changes: Vec<(String, Option<Vec<u8>>)> = vec![
        ("tab/a".to_string(), Some(b"owned".to_vec())),
        ("tab/b".to_string(), None),
    ];
    let mv = doc.commit(main, &changes, "Owned").unwrap();
    assert_eq!(read(&doc, mv, "tab/a").as_deref(), Some("owned"));
    assert_eq!(read(&doc, mv, "tab/b"), None);
}

#[test]
fn dedup_stores_only_new_content() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let big_a: Vec<u8> = (0..200_000u32)
        .flat_map(|i| i.wrapping_mul(7919).to_le_bytes())
        .collect();
    let big_b: Vec<u8> = (0..200_000u32)
        .flat_map(|i| i.wrapping_mul(104_729).to_le_bytes())
        .collect();
    doc.commit(
        main,
        &[set("tab/a", &big_a), set("tab/b", &big_b)],
        "Big tabs",
    )
    .unwrap();
    let s0 = doc.stats().unwrap();

    // Changing one tab stores one data blob + one manifest blob.
    doc.commit(main, &[set("tab/a", b"small change")], "Edit a")
        .unwrap();
    let s1 = doc.stats().unwrap();
    assert_eq!(s1.blobs, s0.blobs + 2);
    assert!(
        s1.stored_bytes - s0.stored_bytes < 1000,
        "tab/b must not be stored again"
    );

    // Restoring earlier content adds no data blob (content already known);
    // the manifest equals an earlier manifest too.
    doc.commit(main, &[set("tab/a", &big_a)], "Back").unwrap();
    let s2 = doc.stats().unwrap();
    assert_eq!(s2.blobs, s1.blobs);
    assert_eq!(s2.microversions, s1.microversions + 1);

    // Same content under another name is stored once.
    doc.commit(main, &[set("tab/copy", &big_b)], "Copy")
        .unwrap();
    let s3 = doc.stats().unwrap();
    assert_eq!(s3.blobs, s2.blobs + 1); // only the new manifest
    // Compression works on compressible data.
    let zeros = vec![0u8; 1 << 20];
    doc.commit(main, &[set("zeros", &zeros)], "Zeros").unwrap();
    let s4 = doc.stats().unwrap();
    assert!(s4.stored_bytes - s3.stored_bytes < 10_000);
    assert!(s4.raw_bytes - s3.raw_bytes >= 1 << 20);
}

#[test]
fn branches_crud() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let branches = doc.list_branches().unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, DEFAULT_BRANCH_NAME);
    assert!(branches[0].is_default);
    assert_eq!(branches[0].created_from, None);

    let feat = doc.create_branch(root, "feature").unwrap();
    assert_eq!(doc.head(feat).unwrap(), root);
    assert_eq!(doc.branch(feat).unwrap().created_from, Some(root));
    assert!(matches!(
        doc.create_branch(root, "feature"),
        Err(StoreError::BranchNameTaken(_))
    ));
    assert!(matches!(
        doc.create_branch(root, "Main"),
        Err(StoreError::BranchNameTaken(_))
    ));
    assert!(matches!(
        doc.create_branch(root, "  "),
        Err(StoreError::EmptyName)
    ));
    assert!(matches!(
        doc.create_branch(MicroversionId::generate(), "x"),
        Err(StoreError::UnknownMicroversion(_))
    ));

    doc.rename_branch(feat, " Feature 1 ").unwrap();
    assert_eq!(doc.branch(feat).unwrap().name, "Feature 1");
    doc.rename_branch(feat, "Feature 1").unwrap(); // same name is fine
    assert!(matches!(
        doc.rename_branch(feat, "Main"),
        Err(StoreError::BranchNameTaken(_))
    ));
    assert_eq!(doc.branch_by_name("Feature 1").unwrap().unwrap().id, feat);
    assert_eq!(
        doc.list_branches()
            .unwrap()
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Main", "Feature 1"]
    );

    // Commits on the branch do not move Main.
    let b1 = doc
        .commit(feat, &[set("tab/a", b"feat")], "Feature edit")
        .unwrap();
    assert_eq!(doc.head(main).unwrap(), root);
    assert_eq!(doc.microversion(b1).unwrap().branch, feat);

    assert!(matches!(
        doc.delete_branch(main),
        Err(StoreError::CannotDeleteDefaultBranch)
    ));
    doc.delete_branch(feat).unwrap();
    assert!(matches!(
        doc.delete_branch(feat),
        Err(StoreError::UnknownBranch(_))
    ));
    assert_eq!(doc.list_branches().unwrap().len(), 1);
    assert!(doc.branch(feat).unwrap().deleted);
    assert!(matches!(doc.head(feat), Err(StoreError::UnknownBranch(_))));
    assert!(matches!(
        doc.commit(feat, &[set("x", b"y")], "x"),
        Err(StoreError::UnknownBranch(_))
    ));
    // Its microversions remain readable and in the graph.
    assert_eq!(read(&doc, b1, "tab/a").as_deref(), Some("feat"));
    assert!(doc.graph().unwrap().get(b1).is_some());
    // The name can be reused.
    let again = doc.create_branch(b1, "Feature 1").unwrap();
    assert_ne!(again, feat);
    assert_eq!(doc.head(again).unwrap(), b1);
}

#[test]
fn merge_without_conflicts() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let feat = doc.create_branch(root, "feature").unwrap();

    let m1 = doc
        .commit(main, &[set("tab/a", b"a-main")], "Main edits a")
        .unwrap();
    doc.commit(feat, &[set("tab/b", b"b-feat")], "Feature edits b")
        .unwrap();
    let f2 = doc
        .commit(
            feat,
            &[set("tab/new", b"new"), del("document/meta")],
            "Feature adds",
        )
        .unwrap();

    assert_eq!(doc.merge_base(m1, f2).unwrap(), Some(root));
    let mut calls = 0;
    let out = doc
        .merge(feat, main, |_, _, _, _| {
            calls += 1;
            None
        })
        .unwrap();
    assert_eq!(calls, 0);
    assert!(!out.up_to_date);
    assert!(out.conflicts.is_empty());
    assert_eq!(out.base, root);
    assert_eq!(
        out.taken_from_source,
        vec!["document/meta", "tab/b", "tab/new"]
    );
    let mv = out.microversion;
    assert_eq!(doc.head(main).unwrap(), mv);
    let info = doc.microversion(mv).unwrap();
    assert_eq!(info.parents, vec![m1, f2]);
    assert!(info.is_merge());
    assert_eq!(info.branch, main);
    assert_eq!(info.description, "Merge \"feature\" into \"Main\"");
    assert_eq!(read(&doc, mv, "tab/a").as_deref(), Some("a-main"));
    assert_eq!(read(&doc, mv, "tab/b").as_deref(), Some("b-feat"));
    assert_eq!(read(&doc, mv, "tab/new").as_deref(), Some("new"));
    assert_eq!(read(&doc, mv, "document/meta"), None);
    // The source branch is untouched.
    assert_eq!(doc.head(feat).unwrap(), f2);

    // Merging again: nothing to do.
    let again = doc.merge(feat, main, take_theirs).unwrap();
    assert!(again.up_to_date);
    assert_eq!(again.microversion, mv);
    assert_eq!(doc.head(main).unwrap(), mv);

    // Continue on the feature branch and merge again: base is now f2.
    let f3 = doc
        .commit(feat, &[set("tab/b", b"b-feat-2")], "More")
        .unwrap();
    assert_eq!(doc.merge_base(mv, f3).unwrap(), Some(f2));
    let out2 = doc.merge(feat, main, take_ours).unwrap();
    assert_eq!(out2.base, f2);
    assert!(out2.conflicts.is_empty());
    assert_eq!(
        read(&doc, out2.microversion, "tab/b").as_deref(),
        Some("b-feat-2")
    );
    assert_eq!(
        read(&doc, out2.microversion, "tab/a").as_deref(),
        Some("a-main")
    );

    // Merging Main back into the feature branch picks up a-main.
    let back = doc.merge(main, feat, take_ours).unwrap();
    assert_eq!(back.base, f3);
    assert_eq!(
        read(&doc, back.microversion, "tab/a").as_deref(),
        Some("a-main")
    );
}

#[test]
fn merge_identical_changes_and_into_ancestor() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let feat = doc.create_branch(root, "feature").unwrap();
    doc.commit(main, &[set("tab/a", b"same")], "m").unwrap();
    let f = doc.commit(feat, &[set("tab/a", b"same")], "f").unwrap();
    let out = doc
        .merge(feat, main, |_, _, _, _| panic!("no conflict expected"))
        .unwrap();
    assert!(out.conflicts.is_empty());
    assert_eq!(
        read(&doc, out.microversion, "tab/a").as_deref(),
        Some("same")
    );

    // Main has not moved since branching: still recorded as a merge.
    let dir2 = TempDir::new();
    let (mut doc, main) = new_doc(&dir2);
    let root = doc.head(main).unwrap();
    let feat = doc.create_branch(root, "feature").unwrap();
    let f1 = doc
        .commit(feat, &[set("tab/a", b"x"), del("tab/b")], "f")
        .unwrap();
    let out = doc.merge(feat, main, take_ours).unwrap();
    assert!(!out.up_to_date);
    assert_eq!(out.base, root);
    assert_eq!(
        doc.microversion(out.microversion).unwrap().parents,
        vec![root, f1]
    );
    assert_eq!(
        *doc.manifest(out.microversion).unwrap(),
        *doc.manifest(f1).unwrap()
    );
    let _ = f;
}

#[test]
fn merge_with_conflicts() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let feat = doc.create_branch(root, "feature").unwrap();
    doc.commit(
        main,
        &[set("tab/a", b"a-main"), set("tab/b", b"b-main")],
        "main",
    )
    .unwrap();
    doc.commit(
        feat,
        &[
            set("tab/a", b"a-feat"),
            del("tab/b"),
            set("tab/c", b"c-feat"),
        ],
        "feat",
    )
    .unwrap();
    // tab/c added on both sides with different content.
    doc.commit(main, &[set("tab/c", b"c-main")], "main c")
        .unwrap();

    let mut seen = Vec::new();
    let out = doc
        .merge(feat, main, |name, base, ours, theirs| {
            let s = |b: Option<&[u8]>| b.map(|b| String::from_utf8(b.to_vec()).unwrap());
            seen.push((name.to_string(), s(base), s(ours), s(theirs)));
            match name {
                "tab/a" => Some(b"a-resolved".to_vec()),
                "tab/b" => None, // accept deletion
                _ => ours.map(<[u8]>::to_vec),
            }
        })
        .unwrap();
    assert_eq!(out.conflicts, vec!["tab/a", "tab/b", "tab/c"]);
    assert_eq!(
        seen,
        vec![
            (
                "tab/a".into(),
                Some("a-v1".into()),
                Some("a-main".into()),
                Some("a-feat".into())
            ),
            (
                "tab/b".into(),
                Some("b-v1".into()),
                Some("b-main".into()),
                None
            ),
            (
                "tab/c".into(),
                None,
                Some("c-main".into()),
                Some("c-feat".into())
            ),
        ]
    );
    let mv = out.microversion;
    assert_eq!(read(&doc, mv, "tab/a").as_deref(), Some("a-resolved"));
    assert_eq!(read(&doc, mv, "tab/b"), None);
    assert_eq!(read(&doc, mv, "tab/c").as_deref(), Some("c-main"));
    assert_eq!(read(&doc, mv, "document/meta").as_deref(), Some("meta-v1"));

    // take_theirs
    let dir2 = TempDir::new();
    let (mut doc, main) = new_doc(&dir2);
    let root = doc.head(main).unwrap();
    let feat = doc.create_branch(root, "feature").unwrap();
    doc.commit(main, &[set("tab/a", b"ours")], "m").unwrap();
    doc.commit(feat, &[set("tab/a", b"theirs")], "f").unwrap();
    let out = doc.merge(feat, main, take_theirs).unwrap();
    assert_eq!(
        read(&doc, out.microversion, "tab/a").as_deref(),
        Some("theirs")
    );
}

#[test]
fn criss_cross_merge_base() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let feat = doc.create_branch(root, "feature").unwrap();
    let m1 = doc.commit(main, &[set("tab/a", b"m1")], "m1").unwrap();
    let f1 = doc.commit(feat, &[set("tab/b", b"f1")], "f1").unwrap();
    let mm = doc.merge(feat, main, take_ours).unwrap().microversion;
    let fm = doc.merge(main, feat, take_ours).unwrap().microversion;
    // fm merged main's merge, so it contains everything.
    assert!(doc.is_ancestor(mm, fm).unwrap());
    assert!(doc.is_ancestor(m1, fm).unwrap());
    assert!(doc.is_ancestor(f1, mm).unwrap());
    assert!(!doc.is_ancestor(fm, mm).unwrap());
    assert!(doc.is_ancestor(root, root).unwrap());
    let up = doc.merge(feat, main, take_ours).unwrap();
    assert!(!up.up_to_date, "fm is a new microversion on feature");
    assert_eq!(up.base, mm);
    assert_eq!(
        *doc.manifest(up.microversion).unwrap(),
        *doc.manifest(fm).unwrap()
    );
}

#[test]
fn restore_creates_new_microversion() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let mv1 = doc
        .commit(main, &[set("tab/a", b"a-v2"), del("tab/b")], "Edit")
        .unwrap();
    doc.create_version(root, "V1", "first").unwrap();
    let r = doc.restore(main, root).unwrap();
    assert_ne!(r, root);
    assert_eq!(doc.head(main).unwrap(), r);
    assert_eq!(*doc.manifest(r).unwrap(), *doc.manifest(root).unwrap());
    let info = doc.microversion(r).unwrap();
    assert_eq!(info.parents, vec![mv1]);
    assert_eq!(info.description, "Restore to version \"V1\"");
    assert_eq!(doc.history(main).unwrap().len(), 3);
    // Restoring to the current content is a no-op.
    assert_eq!(doc.restore(main, root).unwrap(), r);
    // Restore without a version uses the target description.
    let r2 = doc.restore(main, mv1).unwrap();
    assert_eq!(
        doc.microversion(r2).unwrap().description,
        "Restore to \"Edit\""
    );
    // Restore content from another branch.
    let feat = doc.create_branch(root, "feature").unwrap();
    let f = doc.commit(feat, &[set("tab/z", b"z")], "z").unwrap();
    let r3 = doc.restore(main, f).unwrap();
    assert_eq!(read(&doc, r3, "tab/z").as_deref(), Some("z"));
    assert_eq!(doc.microversion(r3).unwrap().branch, main);
}

#[test]
fn undo_redo() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let mut undo = UndoStack::new(main);
    assert!(!undo.can_undo());
    assert_eq!(undo.undo(&mut doc).unwrap(), None);
    assert_eq!(undo.redo(&mut doc).unwrap(), None);

    let s1 = undo
        .commit(&mut doc, &[set("tab/a", b"1")], "Edit 1")
        .unwrap();
    let s2 = undo
        .commit(&mut doc, &[set("tab/a", b"2")], "Edit 2")
        .unwrap();
    let s3 = undo
        .commit(&mut doc, &[set("tab/b", b"3")], "Edit 3")
        .unwrap();
    // no-op commit is not recorded
    undo.commit(&mut doc, &[set("tab/b", b"3")], "noop")
        .unwrap();
    assert_eq!(undo.undo_len(), 3);
    assert_eq!(undo.undo_description(), Some("Edit 3"));

    let u1 = undo.undo(&mut doc).unwrap().unwrap();
    assert_eq!(*doc.manifest(u1).unwrap(), *doc.manifest(s2).unwrap());
    assert_eq!(doc.microversion(u1).unwrap().description, "Undo Edit 3");
    assert_eq!(doc.microversion(u1).unwrap().parents, vec![s3]);
    let u2 = undo.undo(&mut doc).unwrap().unwrap();
    assert_eq!(*doc.manifest(u2).unwrap(), *doc.manifest(s1).unwrap());
    assert_eq!(undo.redo_description(), Some("Edit 2"));
    assert!(undo.can_redo());

    let r1 = undo.redo(&mut doc).unwrap().unwrap();
    assert_eq!(*doc.manifest(r1).unwrap(), *doc.manifest(s2).unwrap());
    assert_eq!(doc.microversion(r1).unwrap().description, "Redo Edit 2");
    assert_eq!(undo.redo_len(), 1);

    // A new edit clears redo.
    undo.commit(&mut doc, &[set("tab/c", b"c")], "Edit 4")
        .unwrap();
    assert!(!undo.can_redo());
    assert_eq!(undo.undo_len(), 3);

    // Undo everything back to the initial state.
    while undo.undo(&mut doc).unwrap().is_some() {}
    let head = doc.head(main).unwrap();
    assert_eq!(*doc.manifest(head).unwrap(), *doc.manifest(root).unwrap());
    // History was never destroyed: every step is a microversion.
    assert_eq!(doc.history(main).unwrap().len(), 1 + 3 + 2 + 1 + 1 + 3);

    // Redo all.
    while undo.redo(&mut doc).unwrap().is_some() {}
    let head = doc.head(main).unwrap();
    assert_eq!(read(&doc, head, "tab/c").as_deref(), Some("c"));
    assert_eq!(read(&doc, head, "tab/a").as_deref(), Some("2"));
}

#[test]
fn undo_stale_and_record() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let mut undo = UndoStack::new(main);
    undo.commit(&mut doc, &[set("tab/a", b"1")], "Edit 1")
        .unwrap();
    // A commit outside the stack makes it stale.
    doc.commit(main, &[set("tab/b", b"x")], "External").unwrap();
    assert!(matches!(undo.undo(&mut doc), Err(StoreError::UndoStale)));
    assert!(!undo.can_undo());

    // Recording an external operation makes it undoable.
    let root = doc.history(main).unwrap().last().unwrap().id;
    let before = doc.head(main).unwrap();
    let after = doc.restore(main, root).unwrap();
    undo.record(before, after, "Restore");
    let u = undo.undo(&mut doc).unwrap().unwrap();
    assert_eq!(*doc.manifest(u).unwrap(), *doc.manifest(before).unwrap());

    // Limit drops the oldest entries.
    let mut small = UndoStack::new(main).with_limit(2);
    for i in 0..5u8 {
        small
            .commit(&mut doc, &[set("tab/a", &[i])], &format!("E{i}"))
            .unwrap();
    }
    assert_eq!(small.undo_len(), 2);
    assert_eq!(small.undo_description(), Some("E4"));
    small.clear();
    assert!(!small.can_undo());
    assert_eq!(small.branch(), main);
}

#[test]
fn history_and_graph() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let a = doc.commit(main, &[set("tab/a", b"1")], "A").unwrap();
    let feat = doc.create_branch(a, "feature").unwrap();
    let f = doc.commit(feat, &[set("tab/b", b"f")], "F").unwrap();
    let b = doc.commit(main, &[set("tab/a", b"2")], "B").unwrap();
    let m = doc.merge(feat, main, take_ours).unwrap().microversion;
    let v = doc.create_version(b, "V1", "release").unwrap();

    let h: Vec<_> = doc
        .history(main)
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(h, vec![m, b, a, root]);
    let h: Vec<_> = doc
        .history(feat)
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(h, vec![f, a, root]);
    let limited = doc.history_from(m, Some(2)).unwrap();
    assert_eq!(limited.iter().map(|i| i.id).collect::<Vec<_>>(), vec![m, b]);
    assert!(doc.history_from(m, Some(0)).unwrap().is_empty());
    assert!(doc.history_from(MicroversionId::generate(), None).is_err());

    let g = doc.graph().unwrap();
    let ids: Vec<_> = g.microversions.iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![root, a, f, b, m]);
    // Parents always precede children.
    for (i, mv) in g.microversions.iter().enumerate() {
        for p in &mv.parents {
            assert!(ids[..i].contains(p));
        }
    }
    assert!(g.microversions.windows(2).all(|w| w[0].seq < w[1].seq));
    assert_eq!(g.children(a), vec![f, b]);
    assert_eq!(g.children(m), Vec::<MicroversionId>::new());
    let cm = g.child_map();
    assert_eq!(cm[&f], vec![m]);
    assert_eq!(cm[&b], vec![m]);
    assert_eq!(g.get(f).unwrap().branch, feat);
    assert_eq!(g.branches.len(), 2);
    assert_eq!(g.versions.len(), 1);
    assert_eq!(g.versions[0].id, v);
    assert_eq!(g.versions[0].microversion, b);
    // Graph is serializable for the UI.
    let json = serde_json::to_string(&g).unwrap();
    let back: VersionGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(back, g);
}

#[test]
fn versions() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let root = doc.head(main).unwrap();
    let mv = doc.commit(main, &[set("tab/a", b"x")], "x").unwrap();
    let v1 = doc.create_version(root, "V1", "Initial").unwrap();
    let v2 = doc.create_version(mv, "V2", "").unwrap();
    let list = doc.list_versions().unwrap();
    assert_eq!(list.iter().map(|v| v.id).collect::<Vec<_>>(), vec![v1, v2]);
    let info = doc.version(v1).unwrap();
    assert_eq!(info.name, "V1");
    assert_eq!(info.description, "Initial");
    assert_eq!(info.microversion, root);
    assert_eq!(info.author, local_user_name());
    assert!(matches!(
        doc.create_version(mv, "", ""),
        Err(StoreError::EmptyName)
    ));
    assert!(matches!(
        doc.create_version(MicroversionId::generate(), "V3", ""),
        Err(StoreError::UnknownMicroversion(_))
    ));
    assert!(matches!(
        doc.version(VersionId::generate()),
        Err(StoreError::UnknownVersion(_))
    ));
    // Branch from a version (Onshape: "Branch to create workspace").
    let b = doc.create_branch(info.microversion, "from V1").unwrap();
    assert_eq!(
        read(&doc, doc.head(b).unwrap(), "tab/a").as_deref(),
        Some("a-v1")
    );
}

#[test]
fn metadata() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let m = doc.meta().unwrap();
    assert_eq!(m.name, "doc");
    assert!(m.created > 0);
    assert!(m.modified >= m.created);
    assert_eq!(m.thumbnail, None);
    doc.set_name("Bracket").unwrap();
    doc.set_description("A bracket").unwrap();
    doc.set_default_units(r#"{"length":"Inch"}"#).unwrap();
    let png = [0x89u8, b'P', b'N', b'G', 1, 2, 3];
    doc.set_thumbnail(Some(&png)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    doc.commit(main, &[set("tab/a", b"m")], "edit").unwrap();
    let m2 = doc.meta().unwrap();
    assert_eq!(m2.name, "Bracket");
    assert_eq!(m2.description, "A bracket");
    assert_eq!(m2.default_units, r#"{"length":"Inch"}"#);
    assert_eq!(m2.thumbnail.as_deref(), Some(&png[..]));
    assert!(m2.modified > m.modified);
    doc.set_thumbnail(None).unwrap();
    assert_eq!(doc.meta().unwrap().thumbnail, None);
    doc.set_meta("custom", Some(b"v")).unwrap();
    assert_eq!(doc.get_meta("custom").unwrap().as_deref(), Some(&b"v"[..]));
    doc.set_meta("custom", None).unwrap();
    assert_eq!(doc.get_meta("custom").unwrap(), None);
}

#[test]
fn reopen_preserves_everything() {
    let dir = TempDir::new();
    let path = dir.file("doc.noshape");
    let (main, feat, v, history, graph);
    {
        let (mut doc, m) = new_doc(&dir);
        main = m;
        doc.set_author("alice");
        let root = doc.head(main).unwrap();
        doc.commit(main, &[set("tab/a", b"a2")], "Edit 1").unwrap();
        feat = doc.create_branch(root, "feature").unwrap();
        doc.commit(feat, &[set("tab/b", b"b2")], "Edit 2").unwrap();
        doc.merge(feat, main, take_ours).unwrap();
        v = doc
            .create_version(doc.head(main).unwrap(), "V1", "d")
            .unwrap();
        doc.set_name("Reopened").unwrap();
        history = doc.history(main).unwrap();
        graph = doc.graph().unwrap();
    }
    let doc = Document::open(&path).unwrap();
    assert_eq!(doc.default_branch().unwrap(), main);
    assert_eq!(doc.history(main).unwrap(), history);
    assert_eq!(doc.graph().unwrap(), graph);
    assert_eq!(doc.version(v).unwrap().name, "V1");
    assert_eq!(doc.branch(feat).unwrap().name, "feature");
    assert_eq!(doc.meta().unwrap().name, "Reopened");
    assert_eq!(history[0].author, "alice");
    let head = doc.head(main).unwrap();
    assert_eq!(read(&doc, head, "tab/a").as_deref(), Some("a2"));
    assert_eq!(read(&doc, head, "tab/b").as_deref(), Some("b2"));
    drop(doc);
    // The file is self-contained after closing (WAL checkpointed).
    assert!(!Path::new(&format!("{}-wal", path.display())).exists());
    // Read-only open with no writer around.
    let ro = Document::open_read_only(&path).unwrap();
    assert_eq!(ro.history(main).unwrap(), history);
}

#[test]
fn open_errors() {
    let dir = TempDir::new();
    let (doc, _) = new_doc(&dir);
    drop(doc);
    assert!(matches!(
        Document::create(dir.file("doc.noshape"), &[] as &[(&str, &[u8])]),
        Err(StoreError::AlreadyExists(_))
    ));
    assert!(matches!(
        Document::open(dir.file("missing.noshape")),
        Err(StoreError::Io(_))
    ));
    assert!(!dir.file("missing.noshape").exists());

    std::fs::write(
        dir.file("junk.noshape"),
        b"this is not sqlite at all, just some text.....",
    )
    .unwrap();
    assert!(matches!(
        Document::open(dir.file("junk.noshape")),
        Err(StoreError::NotADocument)
    ));

    // A valid SQLite file that is not a noshape document.
    {
        let c = rusqlite::Connection::open(dir.file("other.db")).unwrap();
        c.execute_batch("CREATE TABLE t (x INTEGER);").unwrap();
    }
    assert!(matches!(
        Document::open(dir.file("other.db")),
        Err(StoreError::NotADocument)
    ));

    // A document from a newer version of noshape.
    {
        let c = rusqlite::Connection::open(dir.file("doc.noshape")).unwrap();
        c.execute("UPDATE schema_version SET version = 99", [])
            .unwrap();
    }
    assert!(matches!(
        Document::open(dir.file("doc.noshape")),
        Err(StoreError::NewerSchema {
            found: 99,
            supported: SCHEMA_VERSION
        })
    ));
}

#[test]
fn read_only_handles_and_concurrent_readers() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let path = doc.path().to_path_buf();
    let mut ro = doc.reader().unwrap();
    assert!(ro.is_read_only());
    assert!(matches!(
        ro.commit(main, &[set("x", b"y")], "x"),
        Err(StoreError::ReadOnly)
    ));
    assert!(matches!(
        ro.create_branch(doc.head(main).unwrap(), "x"),
        Err(StoreError::ReadOnly)
    ));
    assert!(matches!(ro.set_name("x"), Err(StoreError::ReadOnly)));
    drop(ro);

    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let r = Document::open_read_only(&path).unwrap();
            let main = r.default_branch().unwrap();
            let mut reads = 0u32;
            let mut last_len = 0;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) || reads == 0 {
                let head = r.head(main).unwrap();
                let data = r.read(head, "tab/a").unwrap().unwrap();
                // Every read sees a complete, committed state.
                assert!(data == b"a-v1" || data.len() == 50_000, "torn read");
                let n = r.history(main).unwrap().len();
                assert!(n >= last_len, "history never shrinks");
                last_len = n;
                reads += 1;
            }
            reads
        })
    };
    for i in 0..30u8 {
        let data = vec![i; 50_000];
        doc.commit(main, &[set("tab/a", &data)], &format!("Edit {i}"))
            .unwrap();
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let reads = reader.join().unwrap();
    assert!(reads > 0);
    // A fresh reader sees all commits.
    let r = doc.reader().unwrap();
    assert_eq!(r.history(main).unwrap().len(), 31);
}

#[test]
fn backup_copy() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    doc.commit(main, &[set("tab/a", b"x")], "x").unwrap();
    doc.checkpoint().unwrap();
    doc.backup_to(dir.file("copy.noshape")).unwrap();
    assert!(matches!(
        doc.backup_to(dir.file("copy.noshape")),
        Err(StoreError::AlreadyExists(_))
    ));
    let copy = Document::open(dir.file("copy.noshape")).unwrap();
    assert_eq!(copy.graph().unwrap(), doc.graph().unwrap());
}

#[test]
fn commit_one_megabyte_is_fast() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    // Semi-compressible payload resembling serialized geometry.
    let mut payload: Vec<u8> = Vec::with_capacity(1 << 20);
    let mut x = 0x1234_5678u32;
    while payload.len() < (1 << 20) {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        payload.extend_from_slice(&(x % 1000).to_le_bytes());
        payload.extend_from_slice(b"\"face\":");
    }
    // Warm up.
    doc.commit(main, &[set("tab/warm", &payload[..1000])], "warm")
        .unwrap();
    let mut times = Vec::new();
    for i in 0..5u8 {
        payload[0] = i;
        let t = Instant::now();
        doc.commit(main, &[set("tab/a", &payload)], "Edit").unwrap();
        times.push(t.elapsed());
    }
    times.sort();
    let median = times[2];
    eprintln!("1 MB commit: median {median:?}, all {times:?}");
    // Generous bound for slow/debug CI machines; typically a few ms.
    assert!(median.as_millis() < 250, "1 MB commit took {median:?}");
    let t = Instant::now();
    let head = doc.head(main).unwrap();
    let back = doc.read(head, "tab/a").unwrap().unwrap();
    eprintln!("1 MB read: {:?}", t.elapsed());
    assert_eq!(back, payload);
}

/// QA: after reopening a document, Ctrl+Z did nothing although the user
/// guide promises undo across sessions. The stack is rebuilt from history.
#[test]
fn undo_stack_rebuilt_from_history() {
    let dir = TempDir::new();
    let (mut doc, main) = new_doc(&dir);
    let mut undo = UndoStack::new(main);
    let s1 = undo
        .commit(&mut doc, &[set("tab/a", b"1")], "Edit 1")
        .unwrap();
    let s2 = undo
        .commit(&mut doc, &[set("tab/a", b"2")], "Edit 2")
        .unwrap();
    undo.commit(&mut doc, &[set("tab/a", b"3")], "Edit 3")
        .unwrap();
    undo.undo(&mut doc).unwrap();
    undo.undo(&mut doc).unwrap();
    undo.redo(&mut doc).unwrap();
    // Live stack: undo [Edit 1, Edit 2], redo [Edit 3].

    let mut re = UndoStack::from_history(&doc, main, 100).unwrap();
    assert_eq!(re.undo_len(), undo.undo_len());
    assert_eq!(re.redo_len(), undo.redo_len());
    assert_eq!(re.undo_description(), Some("Edit 2"));
    assert_eq!(re.redo_description(), Some("Edit 3"));
    let u = re.undo(&mut doc).unwrap().unwrap();
    assert_eq!(*doc.manifest(u).unwrap(), *doc.manifest(s1).unwrap());
    let r = re.redo(&mut doc).unwrap().unwrap();
    assert_eq!(*doc.manifest(r).unwrap(), *doc.manifest(s2).unwrap());

    // A new edit after undos clears redo, as in a live session.
    re.undo(&mut doc).unwrap();
    re.commit(&mut doc, &[set("tab/b", b"x")], "Edit 4").unwrap();
    let re2 = UndoStack::from_history(&doc, main, 100).unwrap();
    assert_eq!(re2.undo_description(), Some("Edit 4"));
    assert!(!re2.can_redo());
    assert_eq!(re2.undo_len(), 2);
    // The limit caps the number of steps.
    assert_eq!(UndoStack::from_history(&doc, main, 1).unwrap().undo_len(), 1);
}
