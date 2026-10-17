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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
}
