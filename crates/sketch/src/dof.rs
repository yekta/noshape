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

pub(crate) fn analyze_component(rows: &[SparseRow], n: usize) -> ComponentAnalysis {
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
}
