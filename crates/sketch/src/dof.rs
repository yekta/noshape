//! Degree-of-freedom analysis via rank-revealing (column pivoted) QR.
//!
//! For a component with Jacobian `J` (m equations x n unknowns) we factor
//! `J^T P = Q R`. The rank `r` is the number of significant diagonal entries
//! of `R`; the first `r` columns of `Q` span the row space of `J`, so an
//! unknown `i` has a component in the null space of `J` (i.e. it can still
//! move) iff `1 - sum_k<r Q[i,k]^2` is not negligible. Equations pivoted
//! after position `r` are linearly dependent on the others (redundant).

use nalgebra::DMatrix;
use smallvec::SmallVec;

pub(crate) struct ComponentAnalysis {
    pub rank: usize,
    /// Per unknown: can it move without violating constraints (to first order)?
    pub free: Vec<bool>,
    /// Indices of equations that are linearly dependent on the others.
    pub redundant_rows: Vec<usize>,
}

pub(crate) type SparseRow = SmallVec<[(u32, f64); 8]>;

const RANK_TOL: f64 = 1e-8;
const FREE_TOL: f64 = 1e-6;

/// Components with more unknowns than this use the sparse analysis.
const DENSE_LIMIT: usize = 150;

pub(crate) fn analyze_component(rows: &[SparseRow], n: usize) -> ComponentAnalysis {
    if n > DENSE_LIMIT {
        return analyze_sparse(rows, n);
    }
    analyze_dense(rows, n)
}

/// Sparse rank analysis: row-sequential Givens QR with threshold pivoting on
/// an RCM-ordered column set (R stays banded), then null-space vectors by
/// back substitution for every non-pivot column.
pub(crate) fn analyze_sparse(rows: &[SparseRow], n: usize) -> ComponentAnalysis {
    let m = rows.len();
    // column ordering (RCM over the variable adjacency graph)
    let mut edges = Vec::new();
    for row in rows {
        for a in 0..row.len() {
            for b in 0..a {
                edges.push((row[a].0 as usize, row[b].0 as usize));
            }
        }
    }
    let order = crate::linalg::rcm_order(n, &edges); // new -> old
    let mut newcol = vec![0usize; n];
    for (nw, &old) in order.iter().enumerate() {
        newcol[old] = nw;
    }
    // R rows indexed by pivot column: (start, values) dense segment
    let mut r: Vec<Option<(usize, Vec<f64>)>> = vec![None; n];
    let mut redundant_rows = Vec::new();
    let mut a: Vec<f64> = Vec::new();
    for (ri, row) in rows.iter().enumerate() {
        let norm: f64 = row.iter().map(|(_, v)| v * v).sum::<f64>().sqrt();
        if norm < 1e-14 || row.is_empty() {
            redundant_rows.push(ri);
            continue;
        }
        let lo = row.iter().map(|e| newcol[e.0 as usize]).min().unwrap();
        let hi = row.iter().map(|e| newcol[e.0 as usize]).max().unwrap();
        let mut a_start = lo;
        a.clear();
        a.resize(hi - lo + 1, 0.0);
        for &(c, v) in row {
            a[newcol[c as usize] - lo] += v / norm;
        }
        let mut j = a_start;
        let mut placed = false;
        while j < a_start + a.len() {
            let aj = a[j - a_start];
            if aj.abs() <= RANK_TOL {
                a[j - a_start] = 0.0;
                j += 1;
                continue;
            }
            match &mut r[j] {
                None => {
                    // trim leading zeros and store
                    let off = j - a_start;
                    let seg: Vec<f64> = a[off..].to_vec();
                    r[j] = Some((j, seg));
                    placed = true;
                    break;
                }
                Some((rs, rv)) => {
                    debug_assert_eq!(*rs, j);
                    // bring both to the same range [j, end]
                    let end = (*rs + rv.len()).max(a_start + a.len());
                    rv.resize(end - *rs, 0.0);
                    if a_start + a.len() < end {
                        a.resize(end - a_start, 0.0);
                    }
                    let rjj = rv[0];
                    let rho = rjj.hypot(aj);
                    let (c, s) = (rjj / rho, aj / rho);
                    for k in j..end {
                        let x = rv[k - j];
                        let y = a[k - a_start];
                        rv[k - j] = c * x + s * y;
                        a[k - a_start] = -s * x + c * y;
                    }
                    a[j - a_start] = 0.0;
                    // drop the leading part of `a`
                    let off = j + 1 - a_start;
                    a.drain(..off);
                    a_start = j + 1;
                    j = a_start;
                }
            }
        }
        if !placed {
            redundant_rows.push(ri);
        }
    }
    let rank = r.iter().filter(|x| x.is_some()).count();
    let mut free_new = vec![false; n];
    let mut x = vec![0.0; n];
    for f in 0..n {
        if r[f].is_some() {
            continue;
        }
        free_new[f] = true;
        x.iter_mut().for_each(|v| *v = 0.0);
        x[f] = 1.0;
        for j in (0..f).rev() {
            if let Some((_, rv)) = &r[j] {
                let mut s = 0.0;
                for (k, v) in rv.iter().enumerate().skip(1) {
                    s += v * x[j + k];
                }
                x[j] = -s / rv[0];
            }
        }
        let mx = x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        for (k, v) in x.iter().enumerate().take(f) {
            if v.abs() > 1e-8 * mx {
                free_new[k] = true;
            }
        }
    }
    let free = (0..n).map(|old| free_new[newcol[old]]).collect();
    let _ = m;
    ComponentAnalysis { rank, free, redundant_rows }
}

pub(crate) fn analyze_dense(rows: &[SparseRow], n: usize) -> ComponentAnalysis {
    let m = rows.len();
    if n == 0 {
        return ComponentAnalysis { rank: 0, free: Vec::new(), redundant_rows: (0..m).collect() };
    }
    if m == 0 {
        return ComponentAnalysis { rank: 0, free: vec![true; n], redundant_rows: Vec::new() };
    }
    // J^T, rows normalized (row scaling preserves rank and null space).
    let mut jt = DMatrix::<f64>::zeros(n, m);
    let mut zero_rows = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let norm: f64 = row.iter().map(|(_, v)| v * v).sum::<f64>().sqrt();
        if norm < 1e-14 {
            zero_rows.push(i);
            continue;
        }
        for &(c, v) in row {
            jt[(c as usize, i)] += v / norm;
        }
    }
    let qr = jt.col_piv_qr();
    let r = qr.r();
    let q = qr.q();
    let k = n.min(m);
    let r00 = r[(0, 0)].abs().max(1e-300);
    let mut rank = 0;
    for d in 0..k {
        if r[(d, d)].abs() > RANK_TOL * r00.max(1.0) {
            rank += 1;
        } else {
            break;
        }
    }
    let mut free = vec![false; n];
    for (i, f) in free.iter_mut().enumerate() {
        let mut s = 0.0;
        for c in 0..rank {
            s += q[(i, c)] * q[(i, c)];
        }
        *f = 1.0 - s > FREE_TOL;
    }
    // pivot order of equations
    let mut idx = DMatrix::<f64>::from_fn(1, m, |_, j| j as f64);
    qr.p().permute_columns(&mut idx);
    let mut redundant_rows: Vec<usize> = (rank..m).map(|p| idx[(0, p)] as usize).collect();
    for z in zero_rows {
        if !redundant_rows.contains(&z) {
            redundant_rows.push(z);
        }
    }
    redundant_rows.sort_unstable();
    ComponentAnalysis { rank, free, redundant_rows }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    #[test]
    fn rank_and_null_space() {
        // x0 - x1 = 0, x1 - x2 = 0, x0 - x2 = 0 (redundant), unknowns x0..x3
        let rows: Vec<SparseRow> = vec![
            smallvec![(0, 1.0), (1, -1.0)],
            smallvec![(1, 1.0), (2, -1.0)],
            smallvec![(0, 1.0), (2, -1.0)],
        ];
        let a = analyze_component(&rows, 4);
        assert_eq!(a.rank, 2);
        assert!(a.free.iter().all(|f| *f));
        assert_eq!(a.redundant_rows.len(), 1);
        // fix x0 and x3: everything determined
        let mut rows2 = rows.clone();
        rows2.push(smallvec![(0, 1.0)]);
        rows2.push(smallvec![(3, 2.0)]);
        let b = analyze_component(&rows2, 4);
        assert_eq!(b.rank, 4);
        assert!(b.free.iter().all(|f| !*f));
        assert_eq!(b.redundant_rows.len(), 1);
    }

    #[test]
    fn permutation_semantics() {
        // A permuted QR must reproduce J^T P. Verify the pivot index mapping by
        // checking that the redundant row is the dependent duplicate.
        let rows: Vec<SparseRow> = vec![smallvec![(0, 1.0)], smallvec![(1, 5.0)], smallvec![(0, 3.0)]];
        let a = analyze_component(&rows, 2);
        assert_eq!(a.rank, 2);
        assert_eq!(a.redundant_rows.len(), 1);
        assert!(a.redundant_rows[0] == 0 || a.redundant_rows[0] == 2);
    }

    #[test]
    fn sparse_matches_dense() {
        let rows: Vec<SparseRow> = vec![
            smallvec![(0, 1.0), (1, -1.0)],
            smallvec![(1, 1.0), (2, -1.0)],
            smallvec![(0, 1.0), (2, -1.0)],
            smallvec![(3, 1.0), (4, 2.0)],
            smallvec![(0, 1.0)],
        ];
        for n in [5, 6] {
            let d = analyze_dense(&rows, n);
            let s = analyze_sparse(&rows, n);
            assert_eq!(d.rank, s.rank);
            assert_eq!(d.free, s.free);
            assert_eq!(d.redundant_rows.len(), s.redundant_rows.len());
        }
    }
}
