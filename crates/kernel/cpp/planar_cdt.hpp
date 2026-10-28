// Constrained Delaunay triangulation of a planar polygon with holes, for the
// display mesh of planar faces with many boundary nodes: every boundary node
// is a vertex, no others. Points inserted in z-order into a Delaunay
// triangulation (walk location, Lawson legalisation), boundary segments
// recovered by flips (Sloan), the triangles inside kept by flooding from the
// boundary, a last Lawson pass. False whenever the input or the result isn't
// clean (the caller meshes the face the usual way).
#pragma once
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <deque>
#include <vector>

namespace noshape::cdt {

struct Pt
{
  double x, y;
};

namespace detail {

inline double orient(const Pt& a, const Pt& b, const Pt& c) { return (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x); }

// > 0: d inside the circle through the counter-clockwise a, b, c.
inline double incircle(const Pt& a, const Pt& b, const Pt& c, const Pt& d)
{
  const double adx = a.x - d.x, ady = a.y - d.y, bdx = b.x - d.x, bdy = b.y - d.y, cdx = c.x - d.x, cdy = c.y - d.y;
  const double ad = adx * adx + ady * ady, bd = bdx * bdx + bdy * bdy, cd = cdx * cdx + cdy * cdy;
  return adx * (bdy * cd - bd * cdy) - ady * (bdx * cd - bd * cdx) + ad * (bdx * cdy - bdy * cdx);
}

// d clearly inside the circle through the counter-clockwise a, b, c: beyond
// the rounding of the determinant (Shewchuk's bound on its permanent), so
// cocircular points (hole rims) don't flip back and forth, at any scale.
inline bool in_circle(const Pt& a, const Pt& b, const Pt& c, const Pt& d)
{
  const double adx = a.x - d.x, ady = a.y - d.y, bdx = b.x - d.x, bdy = b.y - d.y, cdx = c.x - d.x, cdy = c.y - d.y;
  const double ad = adx * adx + ady * ady, bd = bdx * bdx + bdy * bdy, cd = cdx * cdx + cdy * cdy;
  const double det = adx * (bdy * cd - bd * cdy) - ady * (bdx * cd - bd * cdx) + ad * (bdx * cdy - bdy * cdx);
  const double perm = (std::abs(bdx * cdy) + std::abs(cdx * bdy)) * ad + (std::abs(cdx * ady) + std::abs(adx * cdy)) * bd +
                      (std::abs(adx * bdy) + std::abs(bdx * ady)) * cd;
  return det > 1e-14 * perm;
}

inline uint32_t spread(uint32_t v)
{
  v = (v | (v << 8)) & 0x00FF00FF;
  v = (v | (v << 4)) & 0x0F0F0F0F;
  v = (v | (v << 2)) & 0x33333333;
  return (v | (v << 1)) & 0x55555555;
}

// Triangles t: vertices v[3t..3t+2] counter-clockwise; half-edge e runs from
// v[e] to v[next(e)]; tw[e] its twin (-1 on the outside).
struct Mesh
{
  std::vector<Pt>  p;
  std::vector<int> v, tw, vedge; // vedge[i]: a half-edge leaving vertex i
  std::vector<char> fixed;       // per half-edge: a boundary segment
  std::vector<int> st;           // legalisation stack (reused)
  std::vector<std::pair<int, int>> recovered; // diagonals made by segment recovery
  size_t           walked = 0;

  static int next(int e) { return e % 3 == 2 ? e - 2 : e + 1; }
  static int prev(int e) { return e % 3 == 0 ? e + 2 : e - 1; }

  int add_tri(int a, int b, int c)
  {
    const int t = (int)v.size();
    v.insert(v.end(), {a, b, c});
    tw.insert(tw.end(), {-1, -1, -1});
    fixed.insert(fixed.end(), {0, 0, 0});
    vedge[a] = t, vedge[b] = t + 1, vedge[c] = t + 2;
    return t;
  }
  void link(int a, int b)
  {
    if (a >= 0)
      tw[a] = b;
    if (b >= 0)
      tw[b] = a;
  }
  // Flip the edge of half-edge e (both triangles kept, reused). Returns the
  // new diagonal's half-edge in e's triangle (from d to c).
  int flip(int e)
  {
    const int o = tw[e];
    const int t = e - e % 3, u = o - o % 3;
    const int a = v[e], b = v[next(e)], c = v[prev(e)], d = v[prev(o)];
    const int bc = tw[next(e)], ca = tw[prev(e)], ad = tw[next(o)], db = tw[prev(o)];
    const char fbc = fixed[next(e)], fca = fixed[prev(e)], fad = fixed[next(o)], fdb = fixed[prev(o)];
    // t = (a, d, c), u = (d, b, c).
    v[t] = a, v[t + 1] = d, v[t + 2] = c;
    v[u] = d, v[u + 1] = b, v[u + 2] = c;
    link(t, ad), fixed[t] = fad;
    link(t + 1, u + 2), fixed[t + 1] = fixed[u + 2] = 0;
    link(t + 2, ca), fixed[t + 2] = fca;
    link(u, db), fixed[u] = fdb;
    link(u + 1, bc), fixed[u + 1] = fbc;
    vedge[a] = t, vedge[d] = t + 1, vedge[c] = t + 2, vedge[b] = u + 1;
    return t + 1;
  }
  // Lawson: legalise half-edges on the stack (an edge is fine when fixed,
  // on the outside or locally Delaunay).
  void legalize(std::vector<int>& st)
  {
    while (!st.empty())
    {
      const int e = st.back();
      st.pop_back();
      const int o = tw[e];
      if (o < 0 || fixed[e])
        continue;
      const int a = v[e], b = v[next(e)], c = v[prev(e)], d = v[prev(o)];
      if (!in_circle(p[a], p[b], p[c], p[d]))
        continue;
      if (orient(p[a], p[d], p[c]) <= 0 || orient(p[d], p[b], p[c]) <= 0)
        continue;
      const int ne = flip(e);
      const int t = ne - 1, u = tw[ne] - 2;
      st.insert(st.end(), {t, t + 2, u, u + 1});
    }
  }
  // Inserts vertex i; false when it can't be placed (duplicate, lost walk).
  bool insert(int i, int& start)
  {
    const Pt& q = p[i];
    int       t = start - start % 3;
    // Visibility walk, each step's edges tried from a varying one (a
    // stochastic walk: it ends even where rounding makes the plain walk
    // circle, as on a long thin band's slivers); past its bound, a scan.
    bool found = false;
    for (int steps = 0; steps <= (int)v.size(); ++steps)
    {
      ++walked;
      int       out = -1;
      const int k0  = (int)((uint32_t)(steps + i) * 2654435761u >> 30) % 3;
      for (int j = 0; j < 3; ++j)
      {
        const int k = (k0 + j) % 3;
        if (orient(p[v[t + k]], p[v[t + (k + 1) % 3]], q) < 0)
        {
          out = t + k;
          break;
        }
      }
      if (out < 0)
      {
        found = true;
        break;
      }
      if (tw[out] < 0)
        return false;
      t = tw[out] - tw[out] % 3;
    }
    if (!found)
    {
      for (t = 0; t < (int)v.size(); t += 3)
        if (orient(p[v[t]], p[v[t + 1]], q) >= 0 && orient(p[v[t + 1]], p[v[t + 2]], q) >= 0 && orient(p[v[t + 2]], p[v[t]], q) >= 0)
          break;
      if (t >= (int)v.size())
        return false;
    }
    for (int k = 0; k < 3; ++k)
    {
      const Pt& a = p[v[t + k]];
      if (a.x == q.x && a.y == q.y)
        return false;
    }
    st.clear();
    // On an edge: split the two triangles into four.
    int on = -1;
    for (int k = 0; k < 3; ++k)
      if (orient(p[v[t + k]], p[v[t + (k + 1) % 3]], q) == 0)
        on = t + k;
    if (on >= 0)
    {
      const int o = tw[on];
      if (o < 0)
        return false;
      const int a = v[on], b = v[next(on)], c = v[prev(on)], d = v[prev(o)];
      const int bc = tw[next(on)], ca = tw[prev(on)], ad = tw[next(o)], db = tw[prev(o)];
      const int t0 = on - on % 3, t1 = o - o % 3;
      // (a, i, c), (i, b, c) | (b, i, d), (i, a, d)
      v[t0] = a, v[t0 + 1] = i, v[t0 + 2] = c;
      v[t1] = b, v[t1 + 1] = i, v[t1 + 2] = d;
      const int t2 = add_tri(i, b, c), t3 = add_tri(i, a, d);
      link(t0 + 2, ca), link(t2 + 1, bc), link(t1 + 2, db), link(t3 + 1, ad);
      // a-i | i-a, i-c | c-i, i-b | b-i, i-d | d-i (segments are fixed later).
      link(t0, t3), link(t0 + 1, t2 + 2), link(t2, t1), link(t1 + 1, t3 + 2);
      vedge[a] = t0, vedge[i] = t0 + 1, vedge[c] = t0 + 2, vedge[b] = t1, vedge[d] = t1 + 2;
      st.insert(st.end(), {t0 + 2, t2 + 1, t1 + 2, t3 + 1});
      legalize(st);
      start = t0;
      return true;
    }
    const int a = v[t], b = v[t + 1], c = v[t + 2];
    const int ab = tw[t], bc = tw[t + 1], ca = tw[t + 2];
    // (a, b, i), (b, c, i), (c, a, i)
    v[t + 2] = i;
    const int t1 = add_tri(b, c, i), t2 = add_tri(c, a, i);
    link(t, ab), link(t1, bc), link(t2, ca);
    link(t + 1, t1 + 2), link(t1 + 1, t2 + 2), link(t2 + 1, t + 2);
    vedge[a] = t, vedge[b] = t + 1, vedge[i] = t + 2;
    st.insert(st.end(), {t, t1, t2});
    legalize(st);
    start = t;
    return true;
  }
  // Half-edge from a to b, or -1.
  int find_edge(int a, int b) const
  {
    const int e0 = vedge[a];
    int       e  = e0;
    // Round a counter-clockwise: prev(e) arrives at a, its twin leaves a.
    for (int k = 0; k < 4096; ++k)
    {
      if (v[next(e)] == b)
        return e;
      const int r = tw[prev(e)];
      if (r < 0)
        break;
      e = r;
      if (e == e0)
        return -1;
    }
    // Hit the outside going one way: go the other way from e0.
    e = e0;
    for (int k = 0; k < 4096; ++k)
    {
      const int o = tw[e];
      if (o < 0)
        return -1;
      e = next(o);
      if (v[next(e)] == b)
        return e;
      if (e == e0)
        return -1;
    }
    return -1;
  }
  static bool crosses(const Pt& a, const Pt& b, const Pt& c, const Pt& d)
  {
    const double o1 = orient(a, b, c), o2 = orient(a, b, d), o3 = orient(c, d, a), o4 = orient(c, d, b);
    return ((o1 > 0 && o2 < 0) || (o1 < 0 && o2 > 0)) && ((o3 > 0 && o4 < 0) || (o3 < 0 && o4 > 0));
  }
  // Sloan: make a-b an edge by flipping the edges crossing it.
  bool recover(int a, int b)
  {
    if (find_edge(a, b) >= 0)
      return true;
    // The edges crossing a-b, found by walking from a's fan.
    std::deque<int> cross;
    int             e0 = vedge[a], e = e0, start = -1;
    for (int k = 0; k < 4096; ++k)
    {
      // Triangle of e: (a, x, y); does a-b leave a through x-y?
      const int    x = v[next(e)], y = v[prev(e)];
      const double ox = orient(p[a], p[x], p[b]), oy = orient(p[a], p[y], p[b]);
      if (ox > 0 && oy < 0)
      {
        start = next(e);
        break;
      }
      // A vertex on a-b: not a clean segment. One on its line behind a (a
      // fin's edge in line with another's across the plate) is no matter.
      auto ahead = [&](int z) { return (p[z].x - p[a].x) * (p[b].x - p[a].x) + (p[z].y - p[a].y) * (p[b].y - p[a].y) > 0; };
      if ((ox == 0 && ahead(x)) || (oy == 0 && ahead(y)))
        return false;
      const int r = tw[prev(e)];
      if (r < 0)
        return false;
      e = r;
      if (e == e0)
        return false;
    }
    if (start < 0)
      return false;
    // Walk across the triangles to b.
    int h = start;
    for (int k = 0; k < (int)v.size(); ++k)
    {
      cross.push_back(h);
      const int o = tw[h];
      if (o < 0)
        return false;
      const int z = v[prev(o)];
      if (z == b)
        break;
      const double s = orient(p[a], p[b], p[z]);
      if (s == 0)
        return false;
      // Leave through the edge of o's triangle that a-b still crosses.
      h = s > 0 ? next(o) : prev(o);
      if (!crosses(p[a], p[b], p[v[h]], p[v[next(h)]]))
        h = s > 0 ? prev(o) : next(o);
    }
    // As vertex pairs: flips rewrite triangles in place, so half-edge
    // indices don't stay with their edges.
    std::deque<std::pair<int, int>> todo;
    for (int c : cross)
      todo.emplace_back(v[c], v[next(c)]);
    size_t guard = 0, limit = 64 * todo.size() + 64;
    while (!todo.empty())
    {
      if (++guard > limit)
        return false;
      const auto [x, y] = todo.front();
      todo.pop_front();
      const int h = find_edge(x, y);
      if (h < 0 || tw[h] < 0)
        return false;
      const int c = v[prev(h)], d = v[prev(tw[h])];
      // Convex quad x, d, y, c?
      if (orient(p[x], p[d], p[c]) <= 0 || orient(p[d], p[y], p[c]) <= 0)
      {
        todo.emplace_back(x, y);
        continue;
      }
      flip(h);
      recovered.emplace_back(d, c);
      if (crosses(p[a], p[b], p[d], p[c]))
        todo.emplace_back(d, c);
    }
    return find_edge(a, b) >= 0;
  }
};

} // namespace detail

// `rings[0]` the outer boundary, the rest holes: indices into `pts`, each
// ring once round. Triangles counter-clockwise into `tris`.
inline bool triangulate(const std::vector<Pt>& pts, const std::vector<std::vector<int>>& rings, std::vector<int>& tris)
{
  tris.clear();
  if (rings.empty())
    return false;
  size_t nv = 0;
  double minX = 1e300, minY = 1e300, maxX = -1e300, maxY = -1e300;
  std::vector<char> used(pts.size(), 0);
  for (const auto& r : rings)
  {
    if (r.size() < 3)
      return false;
    nv += r.size();
    for (int i : r)
    {
      if (i < 0 || i >= (int)pts.size() || used[(size_t)i])
        return false; // a node twice: not a simple polygon
      used[(size_t)i] = 1;
      minX = std::min(minX, pts[(size_t)i].x), maxX = std::max(maxX, pts[(size_t)i].x);
      minY = std::min(minY, pts[(size_t)i].y), maxY = std::max(maxY, pts[(size_t)i].y);
    }
  }
  const double size = std::max(maxX - minX, maxY - minY);
  if (!(size > 0) || !std::isfinite(size))
    return false;
  // Rings: the outer one counter-clockwise, holes clockwise (inside on the left).
  auto signed_area = [&](const std::vector<int>& r) {
    double s = 0;
    for (size_t k = 0, j = r.size() - 1; k < r.size(); j = k++)
      s += pts[(size_t)r[j]].x * pts[(size_t)r[k]].y - pts[(size_t)r[k]].x * pts[(size_t)r[j]].y;
    return s / 2;
  };
  std::vector<std::vector<int>> rr = rings;
  double want = 0;
  for (size_t k = 0; k < rr.size(); ++k)
  {
    const double a = signed_area(rr[k]);
    if (a == 0)
      return false;
    if ((k == 0) != (a > 0))
      std::reverse(rr[k].begin(), rr[k].end());
    want += k == 0 ? std::abs(a) : -std::abs(a);
  }
  if (!(want > 0))
    return false;
  // Worked on a grid of 2^22 steps across the points, the enclosing
  // triangle within a few widths of them: every coordinate an integer under
  // 2^24, so `orient`'s products stay under 2^52 and it is exact in doubles
  // (rounded orientations made walks circle and segment recovery fail on
  // long thin polygons). Distinct points that land on one grid point fail
  // the insertion (the caller meshes the face the usual way).
  detail::Mesh m;
  const size_t np   = pts.size();
  const double grid = 4194304.0 / size;
  m.p.reserve(np + 3);
  for (const Pt& q : pts)
    m.p.push_back({std::round((q.x - minX) * grid), std::round((q.y - minY) * grid)});
  const double cx = std::round((maxX - minX) * grid / 2), cy = std::round((maxY - minY) * grid / 2), big = 4194304.0;
  m.p.push_back({cx - 2 * big, cy - big});
  m.p.push_back({cx + 2 * big, cy - big});
  m.p.push_back({cx, cy + 2 * big});
  m.vedge.assign(np + 3, -1);
  m.v.reserve(6 * nv + 12);
  m.tw.reserve(6 * nv + 12);
  m.fixed.reserve(6 * nv + 12);
  m.add_tri((int)np, (int)np + 1, (int)np + 2);
  // Insertion order: BRIO (a fixed pseudo-random shuffle cut into rounds of
  // doubling size, each round in z-order): few flips per point (points in
  // sequence round a circle would each flip the fan inside it) and short walks.
  std::vector<std::pair<uint32_t, int>> order;
  order.reserve(nv);
  const double s = 65535 / size;
  for (const auto& r : rr)
    for (int i : r)
      order.emplace_back(detail::spread((uint32_t)std::clamp((pts[(size_t)i].x - minX) * s, 0.0, 65535.0)) |
                           (detail::spread((uint32_t)std::clamp((pts[(size_t)i].y - minY) * s, 0.0, 65535.0)) << 1),
                         i);
  uint64_t seed = 0x9E3779B97F4A7C15ull;
  for (size_t k = order.size(); k > 1; --k)
  {
    seed ^= seed << 13, seed ^= seed >> 7, seed ^= seed << 17;
    std::swap(order[k - 1], order[(size_t)(seed % k)]);
  }
  for (size_t lo = 0, len = 64; lo < order.size(); lo += len, len *= 2)
    std::sort(order.begin() + (std::ptrdiff_t)lo, order.begin() + (std::ptrdiff_t)std::min(order.size(), lo + len));
  int start = 0;
  for (const auto& z : order)
    if (!m.insert(z.second, start))
      return false;
  // Boundary segments.
  for (const auto& r : rr)
    for (size_t k = 0; k < r.size(); ++k)
    {
      const int a = r[k], b = r[(k + 1) % r.size()];
      if (!m.recover(a, b))
        return false;
      const int e = m.find_edge(a, b);
      m.fixed[(size_t)e] = 1;
      if (m.tw[(size_t)e] >= 0)
        m.fixed[(size_t)m.tw[(size_t)e]] = 1;
    }
  // Inside: flood from the left of every boundary segment, never across one.
  const size_t     nt = m.v.size() / 3;
  std::vector<char> in(nt, 0);
  std::vector<int>  st;
  for (const auto& r : rr)
    for (size_t k = 0; k < r.size(); ++k)
    {
      const int e = m.find_edge(r[k], r[(k + 1) % r.size()]);
      if (e < 0)
        return false;
      if (!in[(size_t)e / 3])
      {
        in[(size_t)e / 3] = 1;
        st.push_back(e / 3);
      }
    }
  while (!st.empty())
  {
    const int t = st.back();
    st.pop_back();
    for (int k = 0; k < 3; ++k)
    {
      const int e = 3 * t + k, o = m.tw[(size_t)e];
      if (o < 0 || m.fixed[(size_t)e] || in[(size_t)o / 3])
        continue;
      in[(size_t)o / 3] = 1;
      st.push_back(o / 3);
    }
  }
  // The outside of every segment must have stayed out.
  for (const auto& r : rr)
    for (size_t k = 0; k < r.size(); ++k)
    {
      const int e = m.find_edge(r[k], r[(k + 1) % r.size()]), o = m.tw[(size_t)e];
      if (o >= 0 && in[(size_t)o / 3])
        return false;
    }
  // Lawson again after the recovery flips (segments fixed), then the inside.
  // Only the recovery's diagonals (and what their flips touch) can be
  // non-Delaunay; segments are fixed, so flips stay on one side of them.
  {
    std::vector<int> st;
    for (const auto& [x, y] : m.recovered)
    {
      const int e = m.find_edge(x, y);
      if (e >= 0 && !m.fixed[(size_t)e] && m.tw[(size_t)e] >= 0)
        st.push_back(e);
    }
    m.legalize(st);
  }
  double sum = 0;
  for (size_t t = 0; t < nt; ++t)
    if (in[t])
    {
      const int a = m.v[3 * t], b = m.v[3 * t + 1], c = m.v[3 * t + 2];
      if (a >= (int)np || b >= (int)np || c >= (int)np)
        return false;
      const double ar = detail::orient(pts[(size_t)a], pts[(size_t)b], pts[(size_t)c]);
      if (!(ar > 0))
        return false;
      sum += ar / 2;
      tris.insert(tris.end(), {a, b, c});
    }
  // Every point used: V + 2 H - 2 triangles, covering the polygon.
  if (tris.size() / 3 != nv + 2 * (rr.size() - 1) - 2 || std::abs(sum - want) > 1e-9 * size * size)
    return false;
  return true;
}

} // namespace noshape::cdt
