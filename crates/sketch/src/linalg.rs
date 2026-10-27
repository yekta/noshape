//! Sparse symmetric positive definite solver: reverse Cuthill-McKee ordering
//! followed by an envelope (profile) Cholesky factorization. Sketch
//! constraint graphs are sparse and nearly banded, so this keeps solve time
//! close to linear in the sketch size.

/// Envelope Cholesky solver with a fixed sparsity pattern.
#[derive(Clone, Debug)]
pub(crate) struct EnvelopeSolver {
    n: usize,
    /// new index -> old index
    perm: Vec<usize>,
    /// old index -> new index
    iperm: Vec<usize>,
    /// first stored column of each (new) row
    first: Vec<usize>,
    /// offset of each row in `data`
    start: Vec<usize>,
    data: Vec<f64>,
    scratch: Vec<f64>,
}

impl EnvelopeSolver {
    /// `edges` are the structurally nonzero off-diagonal pairs (old indices).
    pub fn new(n: usize, edges: &[(usize, usize)]) -> Self {
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(i, j) in edges {
            if i != j {
                adj[i].push(j);
                adj[j].push(i);
            }
        }
        for a in adj.iter_mut() {
            a.sort_unstable();
            a.dedup();
        }
        let perm = rcm(&adj);
        let mut iperm = vec![0; n];
        for (new, &old) in perm.iter().enumerate() {
            iperm[old] = new;
        }
        let mut first: Vec<usize> = (0..n).collect();
        for (old, a) in adj.iter().enumerate() {
            let i = iperm[old];
            for &o in a {
                let j = iperm[o];
                if j < i && j < first[i] {
                    first[i] = j;
                }
            }
        }
        let mut start = vec![0; n + 1];
        for i in 0..n {
            start[i + 1] = start[i] + (i - first[i] + 1);
        }
        let data = vec![0.0; start[n]];
        EnvelopeSolver { n, perm, iperm, first, start, data, scratch: vec![0.0; n] }
    }

    pub fn clear(&mut self) {
        self.data.iter_mut().for_each(|v| *v = 0.0);
    }

    pub fn snapshot(&self, out: &mut Vec<f64>) {
        out.clear();
        out.extend_from_slice(&self.data);
    }

    pub fn restore(&mut self, s: &[f64]) {
        self.data.copy_from_slice(s);
    }

    #[inline]
    fn diag_index(&self, i: usize) -> usize {
        self.start[i] + i - self.first[i]
    }

    pub fn trace(&self) -> f64 {
        (0..self.n).map(|i| self.data[self.diag_index(i)]).sum()
    }

    /// Marquardt damping: `d_i <- d_i (1 + lambda) + floor`.
    pub fn damp_diag(&mut self, lambda: f64, floor: f64) {
        for i in 0..self.n {
            let k = self.diag_index(i);
            self.data[k] = self.data[k] * (1.0 + lambda) + floor;
        }
    }

    /// Adds `v` to A[i][j] (old indices). For off-diagonal entries call once
    /// per unordered pair.
    #[inline]
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        let (mut a, mut b) = (self.iperm[i], self.iperm[j]);
        if b > a {
            std::mem::swap(&mut a, &mut b);
        }
        debug_assert!(b >= self.first[a]);
        self.data[self.start[a] + b - self.first[a]] += v;
    }

    /// In-place Cholesky factorization. Returns false if the matrix is not
    /// (numerically) positive definite.
    pub fn factor(&mut self) -> bool {
        let n = self.n;
        for i in 0..n {
            let fi = self.first[i];
            let si = self.start[i];
            for j in fi..i {
                let fj = self.first[j];
                let sj = self.start[j];
                let k0 = fi.max(fj);
                let mut s = self.data[si + j - fi];
                let ri = &self.data[si + k0 - fi..si + j - fi];
                let rj = &self.data[sj + k0 - fj..sj + j - fj];
                for (a, b) in ri.iter().zip(rj.iter()) {
                    s -= a * b;
                }
                let djj = self.data[sj + j - fj];
                self.data[si + j - fi] = s / djj;
            }
            let diag0 = self.data[si + i - fi];
            let mut s = diag0;
            for v in &self.data[si..si + i - fi] {
                s -= v * v;
            }
            if s <= 1e-13 * diag0.abs().max(1e-300) || !s.is_finite() {
                return false;
            }
            self.data[si + i - fi] = s.sqrt();
        }
        true
    }

    /// Solves A x = b in place (old indexing), after `factor`.
    pub fn solve(&mut self, b: &mut [f64]) {
        let n = self.n;
        let y = &mut self.scratch;
        for i in 0..n {
            y[i] = b[self.perm[i]];
        }
        for i in 0..n {
            let fi = self.first[i];
            let si = self.start[i];
            let mut s = y[i];
            for (k, v) in (fi..i).zip(&self.data[si..si + i - fi]) {
                s -= v * y[k];
            }
            y[i] = s / self.data[si + i - fi];
        }
        for i in (0..n).rev() {
            let fi = self.first[i];
            let si = self.start[i];
            let xi = y[i] / self.data[si + i - fi];
            y[i] = xi;
            for (k, v) in (fi..i).zip(&self.data[si..si + i - fi]) {
                y[k] -= v * xi;
            }
        }
        for i in 0..n {
            b[self.perm[i]] = y[i];
        }
    }
}

/// RCM ordering of a graph given by edges. Returns new -> old.
pub(crate) fn rcm_order(n: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(i, j) in edges {
        if i != j {
            adj[i].push(j);
            adj[j].push(i);
        }
    }
    for a in adj.iter_mut() {
        a.sort_unstable();
        a.dedup();
    }
    rcm(&adj)
}

/// Reverse Cuthill-McKee ordering. Returns new -> old.
fn rcm(adj: &[Vec<usize>]) -> Vec<usize> {
    let n = adj.len();
    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let deg: Vec<usize> = adj.iter().map(|a| a.len()).collect();
    let mut queue = std::collections::VecDeque::new();
    while order.len() < n {
        // start: unvisited node of minimum degree, then move to a
        // pseudo-peripheral node via one BFS sweep.
        let mut s = usize::MAX;
        for i in 0..n {
            if !visited[i] && (s == usize::MAX || deg[i] < deg[s]) {
                s = i;
            }
        }
        let s = pseudo_peripheral(adj, &deg, s, &visited);
        visited[s] = true;
        queue.push_back(s);
        while let Some(u) = queue.pop_front() {
            order.push(u);
            let mut nb: Vec<usize> = adj[u].iter().copied().filter(|v| !visited[*v]).collect();
            nb.sort_by_key(|v| (deg[*v], *v));
            for v in nb {
                visited[v] = true;
                queue.push_back(v);
            }
        }
    }
    order.reverse();
    order
}

fn pseudo_peripheral(adj: &[Vec<usize>], deg: &[usize], s: usize, blocked: &[bool]) -> usize {
    let n = adj.len();
    let mut level = vec![usize::MAX; n];
    let mut q = std::collections::VecDeque::new();
    level[s] = 0;
    q.push_back(s);
    let mut last = s;
    while let Some(u) = q.pop_front() {
        if level[u] > level[last] || (level[u] == level[last] && deg[u] < deg[last]) {
            last = u;
        }
        for &v in &adj[u] {
            if level[v] == usize::MAX && !blocked[v] {
                level[v] = level[u] + 1;
                q.push_back(v);
            }
        }
    }
    last
}

/// LU factorization with partial pivoting of a square matrix whose nonzeros
/// lie in a band (`kl` below, `ku` above the diagonal), as LAPACK's gbtrf:
/// the multipliers stay where they were computed and the row interchanges
/// are replayed in the solves, so factor and solves stay inside the band
/// (fit splines' interpolation systems: O(n) instead of O(n³)).
#[derive(Clone, Debug)]
pub(crate) struct BandLu {
    n: usize,
    kl: usize,
    /// Upper bandwidth of U (`ku + kl` after interchanges).
    ku: usize,
    /// Row-major n × n: U on and above the diagonal, the multipliers below.
    a: Vec<f64>,
    /// Row swapped with row k at step k.
    piv: Vec<usize>,
}

#[allow(clippy::needless_range_loop)]
impl BandLu {
    /// `a` row-major n × n. `None` if a pivot is exactly zero (singular).
    pub fn factor(mut a: Vec<f64>, n: usize, kl: usize, ku: usize) -> Option<Self> {
        let mut piv = vec![0; n];
        let ku = ku + kl;
        for k in 0..n {
            let rmax = n.min(k + kl + 1);
            let cmax = n.min(k + ku + 1);
            let mut p = k;
            for i in k + 1..rmax {
                if a[i * n + k].abs() > a[p * n + k].abs() {
                    p = i;
                }
            }
            piv[k] = p;
            let d = a[p * n + k];
            if d == 0.0 || !d.is_finite() {
                return None;
            }
            if p != k {
                for j in k..cmax {
                    a.swap(k * n + j, p * n + j);
                }
            }
            for i in k + 1..rmax {
                let f = a[i * n + k] / d;
                a[i * n + k] = f;
                if f != 0.0 {
                    for j in k + 1..cmax {
                        a[i * n + j] -= f * a[k * n + j];
                    }
                }
            }
        }
        Some(BandLu { n, kl, ku, a, piv })
    }

    /// Solves A x = b in place.
    pub fn solve(&self, b: &mut [f64]) {
        let n = self.n;
        for k in 0..n {
            b.swap(k, self.piv[k]);
            let bk = b[k];
            for i in k + 1..n.min(k + self.kl + 1) {
                b[i] -= self.a[i * n + k] * bk;
            }
        }
        for i in (0..n).rev() {
            let mut s = b[i];
            for j in i + 1..n.min(i + self.ku + 1) {
                s -= self.a[i * n + j] * b[j];
            }
            b[i] = s / self.a[i * n + i];
        }
    }

    /// Solves Aᵀ x = b in place.
    pub fn solve_t(&self, b: &mut [f64]) {
        let n = self.n;
        for i in 0..n {
            let mut s = b[i];
            for j in i.saturating_sub(self.ku)..i {
                s -= self.a[j * n + i] * b[j];
            }
            b[i] = s / self.a[i * n + i];
        }
        for k in (0..n).rev() {
            let mut s = b[k];
            for i in k + 1..n.min(k + self.kl + 1) {
                s -= self.a[i * n + k] * b[i];
            }
            b[k] = s;
            b.swap(k, self.piv[k]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn envelope_cholesky_matches_dense() {
        // random sparse SPD: A = B B^T + I with B sparse
        let n = 30;
        let mut edges = Vec::new();
        let mut dense = vec![vec![0.0; n]; n];
        let mut seed = 12345u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f64) / (1u64 << 31) as f64 - 0.5
        };
        for i in 0..n {
            dense[i][i] += 4.0;
            for _ in 0..2 {
                let j = ((rnd() + 0.5) * n as f64) as usize % n;
                if j != i {
                    let v = rnd();
                    dense[i][j] += v;
                    dense[j][i] += v;
                    edges.push((i, j));
                }
            }
        }
        let mut s = EnvelopeSolver::new(n, &edges);
        for i in 0..n {
            for j in 0..=i {
                if dense[i][j] != 0.0 {
                    s.add(i, j, dense[i][j]);
                }
            }
        }
        assert!(s.factor());
        let x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.37).sin()).collect();
        let mut b: Vec<f64> = (0..n).map(|i| (0..n).map(|j| dense[i][j] * x[j]).sum()).collect();
        s.solve(&mut b);
        for i in 0..n {
            assert!((b[i] - x[i]).abs() < 1e-10);
        }
    }

    /// Banded LU against nalgebra on a band with zero diagonals (pivoting needed), both solves.
    #[test]
    #[allow(clippy::needless_range_loop)]
    fn band_lu_matches_dense() {
        let (n, kl, ku) = (40, 2, 3);
        let mut seed = 99u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f64) / (1u64 << 31) as f64 - 0.5
        };
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            for j in i.saturating_sub(kl)..n.min(i + ku + 1) {
                a[i * n + j] = if i == j && i % 3 == 0 { 0.0 } else { rnd() };
            }
        }
        let b: Vec<f64> = (0..n).map(|_| rnd()).collect();
        let m = nalgebra::DMatrix::from_row_slice(n, n, &a);
        let lu = BandLu::factor(a.clone(), n, kl, ku).unwrap();
        let x0 = m.clone().lu().solve(&nalgebra::DVector::from_column_slice(&b)).unwrap();
        let mut x = b.clone();
        lu.solve(&mut x);
        let t0 = m.transpose().lu().solve(&nalgebra::DVector::from_column_slice(&b)).unwrap();
        let mut t = b.clone();
        lu.solve_t(&mut t);
        for i in 0..n {
            assert!((x[i] - x0[i]).abs() < 1e-9 * (1.0 + x0[i].abs()), "solve {i}: {} vs {}", x[i], x0[i]);
            assert!((t[i] - t0[i]).abs() < 1e-9 * (1.0 + t0[i].abs()), "solve_t {i}: {} vs {}", t[i], t0[i]);
        }
    }
}
