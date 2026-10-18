//! Persistent topological naming.
//!
//! Given the names of the input shapes' faces/edges, the kernel's operation
//! history and deterministic geometric keys, assigns [`TopoName`]s to every
//! face of an operation's output. Edge and vertex names are then derived from
//! face adjacency (see `reference.rs` for the scheme).

use crate::reference::{FeatureId, Role, TopoName};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type Name = Arc<TopoName>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Face,
    Edge,
    Vertex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Relation {
    /// Output is the (possibly modified) continuation of the input entity.
    Modified,
    /// Output was generated from the input entity (e.g. edge -> fillet face).
    Generated,
    /// Output is the identical entity (same TShape) as the input.
    Same,
}

/// One kernel history record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistRecord {
    pub arg: u32,
    pub in_kind: Kind,
    pub in_index: u32,
    pub relation: Relation,
    pub out_kind: Kind,
    pub out_index: u32,
}

/// Names of one input argument's sub-shapes (indexed like the kernel's maps).
#[derive(Clone, Debug, Default)]
pub struct ShapeNames {
    pub faces: Vec<Name>,
    pub edges: Vec<Name>,
    pub vertices: Vec<Name>,
}

/// Deterministic ordering key for a sub-shape (centroid, size) so ordinals
/// assigned to split pieces are stable across regenerations.
pub type GeoKey = [f64; 4];

fn key_cmp(a: &GeoKey, b: &GeoKey) -> std::cmp::Ordering {
    // Quantize to 1e-9 m so tiny numeric noise does not reorder pieces.
    let q = |x: f64| (x * 1e9).round() as i64;
    for i in 0..4 {
        let c = q(a[i]).cmp(&q(b[i]));
        if c != std::cmp::Ordering::Equal {
            return c;
        }
    }
    std::cmp::Ordering::Equal
}

pub struct FaceNamingInput<'a> {
    pub feature: FeatureId,
    pub args: &'a [&'a ShapeNames],
    pub records: &'a [HistRecord],
    /// Number of output faces.
    pub out_count: usize,
    /// Geometric key of an output face; only called for faces that need
    /// ordering (split pieces, new faces, duplicates), so it may be costly.
    pub out_face_key: &'a dyn Fn(usize) -> GeoKey,
    /// Feature-specific roles for output faces (e.g. caps, sweep sides).
    /// Takes precedence over history-derived names.
    pub roles: &'a BTreeMap<u32, Role>,
    /// How to name a face generated from an input edge/vertex
    /// (default: `Role::Generated(edge name)`).
    pub generated_role: Option<&'a dyn Fn(&Name) -> Role>,
}

/// Name every output face.
pub fn name_faces(input: &FaceNamingInput) -> Vec<Name> {
    let n = input.out_count;
    let key_cache: std::cell::RefCell<std::collections::HashMap<usize, GeoKey>> = Default::default();
    let key = |i: usize| -> GeoKey { *key_cache.borrow_mut().entry(i).or_insert_with(|| (input.out_face_key)(i)) };
    // parents[j] = list of (relation, parent name)
    let mut parents: Vec<Vec<(Relation, Name)>> = vec![Vec::new(); n];
    // How many outputs each (arg, face) parent continues into.
    let mut fanout: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
    for r in input.records {
        if r.out_kind != Kind::Face || r.out_index as usize >= n {
            continue;
        }
        let Some(arg) = input.args.get(r.arg as usize) else { continue };
        let name = match r.in_kind {
            Kind::Face => arg.faces.get(r.in_index as usize).cloned(),
            Kind::Edge => arg.edges.get(r.in_index as usize).cloned(),
            Kind::Vertex => arg.vertices.get(r.in_index as usize).cloned(),
        };
        let Some(name) = name else { continue };
        if r.in_kind == Kind::Face && matches!(r.relation, Relation::Modified | Relation::Same) {
            fanout.entry((r.arg, r.in_index)).or_default().push(r.out_index);
            parents[r.out_index as usize].push((r.relation, name));
        } else {
            parents[r.out_index as usize].push((Relation::Generated, name));
        }
    }
    // Ordinal of an output among the outputs of the same parent face, by key.
    let mut split_ordinal: BTreeMap<(u32, u32, u32), u16> = BTreeMap::new();
    for ((arg, idx), outs) in &mut fanout {
        outs.sort_unstable();
        outs.dedup();
        if outs.len() > 1 {
            let mut sorted = outs.clone();
            sorted.sort_by(|a, b| key_cmp(&key(*a as usize), &key(*b as usize)).then(a.cmp(b)));
            for (ord, o) in sorted.iter().enumerate() {
                split_ordinal.insert((*arg, *idx, *o), ord as u16);
            }
        }
    }
    // Index of (arg, face) by parent name for split lookups.
    let mut parent_key: BTreeMap<*const TopoName, (u32, u32)> = BTreeMap::new();
    for (ai, a) in input.args.iter().enumerate() {
        for (fi, f) in a.faces.iter().enumerate() {
            parent_key.insert(Arc::as_ptr(f), (ai as u32, fi as u32));
        }
    }

    let mut out: Vec<Option<Name>> = vec![None; n];
    let mut unnamed: Vec<u32> = Vec::new();
    for j in 0..n {
        if let Some(role) = input.roles.get(&(j as u32)) {
            out[j] = Some(Arc::new(TopoName::face(input.feature, role.clone())));
            continue;
        }
        let ps = &parents[j];
        // Prefer face continuations, choose the smallest name for merged faces.
        let mut face_parents: Vec<&Name> = ps.iter().filter(|(r, _)| *r != Relation::Generated).map(|(_, n)| n).collect();
        face_parents.sort();
        if let Some(p) = face_parents.first() {
            let p = (*p).clone();
            let key = parent_key.get(&Arc::as_ptr(&p)).copied();
            let name = match key.and_then(|(a, i)| split_ordinal.get(&(a, i, j as u32))) {
                Some(ord) => Arc::new(TopoName::Split { parent: p, by: input.feature, ordinal: *ord }),
                None => p,
            };
            out[j] = Some(name);
            continue;
        }
        let mut gen_parents: Vec<&Name> = ps.iter().map(|(_, n)| n).collect();
        gen_parents.sort();
        if let Some(g) = gen_parents.first() {
            let role = match input.generated_role {
                Some(f) => f(g),
                None => Role::Generated((*g).clone()),
            };
            out[j] = Some(Arc::new(TopoName::face(input.feature, role)));
            continue;
        }
        unnamed.push(j as u32);
    }
    // New faces: ordinal by geometric key.
    unnamed.sort_by(|a, b| key_cmp(&key(*a as usize), &key(*b as usize)).then(a.cmp(b)));
    for (ord, j) in unnamed.iter().enumerate() {
        out[*j as usize] = Some(Arc::new(TopoName::face(input.feature, Role::New(ord as u32))));
    }
    // Disambiguate accidental duplicates (e.g. two outputs generated from the
    // same edge) with split ordinals.
    let names: Vec<Name> = out.into_iter().map(|n| n.expect("all faces named")).collect();
    disambiguate(names, &key, input.feature)
}

fn disambiguate(mut names: Vec<Name>, key: &dyn Fn(usize) -> GeoKey, feature: FeatureId) -> Vec<Name> {
    let mut groups: BTreeMap<Name, Vec<usize>> = BTreeMap::new();
    for (i, n) in names.iter().enumerate() {
        groups.entry(n.clone()).or_default().push(i);
    }
    for (name, mut idxs) in groups {
        if idxs.len() < 2 {
            continue;
        }
        idxs.sort_by(|a, b| key_cmp(&key(*a), &key(*b)).then(a.cmp(b)));
        for (ord, i) in idxs.into_iter().enumerate() {
            names[i] = Arc::new(TopoName::Split { parent: name.clone(), by: feature, ordinal: ord as u16 });
        }
    }
    names
}

/// Name edges from face adjacency: `edge_faces[e]` lists the face indices
/// adjacent to edge `e` (1 for boundary edges, 2 normally, more for
/// non-manifold). `edge_keys` orders multiple edges between the same faces.
pub fn name_edges(face_names: &[Name], edge_faces: &[Vec<u32>], edge_key: &dyn Fn(usize) -> GeoKey) -> Vec<Name> {
    let mut base: Vec<(Vec<Name>, usize)> = Vec::with_capacity(edge_faces.len());
    for (e, faces) in edge_faces.iter().enumerate() {
        let mut fs: Vec<Name> = faces.iter().filter_map(|f| face_names.get(*f as usize).cloned()).collect();
        fs.sort();
        base.push((fs, e));
    }
    let mut groups: BTreeMap<Vec<Name>, Vec<usize>> = BTreeMap::new();
    for (fs, e) in &base {
        groups.entry(fs.clone()).or_default().push(*e);
    }
    let mut out: Vec<Option<Name>> = vec![None; edge_faces.len()];
    for (fs, mut edges) in groups {
        if edges.len() > 1 {
            edges.sort_by_cached_key(|e| {
                let k = edge_key(*e);
                (k.map(|x| (x * 1e9).round() as i64), *e)
            });
        }
        for (ord, e) in edges.into_iter().enumerate() {
            let ord = ord as u16;
            let name = match fs.len() {
                0 => TopoName::Boundary { face: Arc::new(TopoName::face(FeatureId(0), Role::New(e as u32))), ordinal: ord },
                1 => TopoName::Boundary { face: fs[0].clone(), ordinal: ord },
                2 => TopoName::edge(fs[0].clone(), fs[1].clone(), ord),
                _ => TopoName::Vertex { faces: fs.clone().into(), ordinal: ord },
            };
            out[e] = Some(Arc::new(name));
        }
    }
    out.into_iter().map(|n| n.expect("all edges named")).collect()
}

/// Name vertices from their adjacent faces.
pub fn name_vertices(face_names: &[Name], vertex_faces: &[Vec<u32>], vertex_key: &dyn Fn(usize) -> GeoKey) -> Vec<Name> {
    let mut groups: BTreeMap<Vec<Name>, Vec<usize>> = BTreeMap::new();
    for (v, faces) in vertex_faces.iter().enumerate() {
        let mut fs: Vec<Name> = faces.iter().filter_map(|f| face_names.get(*f as usize).cloned()).collect();
        fs.sort();
        fs.dedup();
        groups.entry(fs).or_default().push(v);
    }
    let mut out: Vec<Option<Name>> = vec![None; vertex_faces.len()];
    for (fs, mut vs) in groups {
        if vs.len() > 1 {
            vs.sort_by_cached_key(|v| {
                let k = vertex_key(*v);
                (k.map(|x| (x * 1e9).round() as i64), *v)
            });
        }
        for (ord, v) in vs.into_iter().enumerate() {
            out[v] = Some(Arc::new(TopoName::vertex(fs.clone(), ord as u16)));
        }
    }
    out.into_iter().map(|n| n.expect("all vertices named")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::Source;

    fn face(f: u64, role: Role) -> Name {
        Arc::new(TopoName::face(FeatureId(f), role))
    }

    #[test]
    fn split_face_gets_ordered_ordinals_and_kept_faces_keep_names() {
        // Input body: 2 faces A, B. Output: A unchanged, B split in two.
        let a = face(1, Role::CapStart(None));
        let b = face(1, Role::CapEnd(None));
        let input = ShapeNames { faces: vec![a.clone(), b.clone()], ..Default::default() };
        let args = [&input];
        let records = [
            HistRecord { arg: 0, in_kind: Kind::Face, in_index: 0, relation: Relation::Same, out_kind: Kind::Face, out_index: 0 },
            HistRecord { arg: 0, in_kind: Kind::Face, in_index: 1, relation: Relation::Modified, out_kind: Kind::Face, out_index: 2 },
            HistRecord { arg: 0, in_kind: Kind::Face, in_index: 1, relation: Relation::Modified, out_kind: Kind::Face, out_index: 1 },
        ];
        let keys = [[0.0; 4], [2.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0]];
        let roles = BTreeMap::new();
        let names = name_faces(&FaceNamingInput { feature: FeatureId(9), args: &args, records: &records, out_count: keys.len(), out_face_key: &|i| keys[i], roles: &roles, generated_role: None });
        assert_eq!(names[0], a);
        // Face 2 has the smaller key -> ordinal 0.
        assert_eq!(*names[2], TopoName::Split { parent: b.clone(), by: FeatureId(9), ordinal: 0 });
        assert_eq!(*names[1], TopoName::Split { parent: b.clone(), by: FeatureId(9), ordinal: 1 });
        assert!(names[1].descends_from(&b));
    }

    #[test]
    fn generated_faces_and_roles() {
        let side_src = |e| Source::Sketch { sketch: FeatureId(2), entity: e, segment: 0 };
        let e0 = Arc::new(TopoName::Wire { feature: FeatureId(2), source: side_src(5) });
        let input = ShapeNames { edges: vec![e0.clone()], ..Default::default() };
        let args = [&input];
        let records = [HistRecord { arg: 0, in_kind: Kind::Edge, in_index: 0, relation: Relation::Generated, out_kind: Kind::Face, out_index: 1 }];
        let keys = [[0.0; 4], [1.0, 0.0, 0.0, 0.0], [5.0, 0.0, 0.0, 0.0]];
        let mut roles = BTreeMap::new();
        roles.insert(0, Role::CapStart(None));
        let side = |n: &Name| match &**n {
            TopoName::Wire { source, .. } => Role::Side(source.clone()),
            _ => Role::Generated(n.clone()),
        };
        let names = name_faces(&FaceNamingInput { feature: FeatureId(3), args: &args, records: &records, out_count: keys.len(), out_face_key: &|i| keys[i], roles: &roles, generated_role: Some(&side) });
        assert_eq!(*names[0], TopoName::face(FeatureId(3), Role::CapStart(None)));
        assert_eq!(*names[1], TopoName::face(FeatureId(3), Role::Side(side_src(5))));
        assert_eq!(*names[2], TopoName::face(FeatureId(3), Role::New(0)));
    }

    #[test]
    fn edges_named_by_face_pairs_with_ordinals() {
        let a = face(1, Role::CapStart(None));
        let b = face(1, Role::CapEnd(None));
        let faces = vec![a.clone(), b.clone()];
        let edge_faces = vec![vec![0, 1], vec![1, 0], vec![0]];
        let keys = [[1.0, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4]];
        let names = name_edges(&faces, &edge_faces, &|i| keys[i]);
        assert_eq!(*names[1], TopoName::edge(a.clone(), b.clone(), 0));
        assert_eq!(*names[0], TopoName::edge(a.clone(), b.clone(), 1));
        assert_eq!(*names[2], TopoName::Boundary { face: a, ordinal: 0 });
    }
}
