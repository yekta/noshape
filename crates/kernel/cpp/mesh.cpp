// Tessellation: one BRepMesh call, then flat buffers filled in a single pass.
#include "internal.hpp"
#include "planar_cdt.hpp"

#include <BRepAdaptor_Curve.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_Context.hxx>
#include <BRepMesh_FaceDiscret.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <TopExp_Explorer.hxx>
#include <BRepMesh_MeshAlgoFactory.hxx>
#include <BRepMesh_ShapeTool.hxx>
#include <IMeshData_Curve.hxx>
#include <IMeshData_Edge.hxx>
#include <IMeshData_Face.hxx>
#include <IMeshData_PCurve.hxx>
#include <IMeshData_Wire.hxx>
#include <IMeshTools_MeshAlgo.hxx>
#include <IMeshTools_MeshAlgoFactory.hxx>
#include <IMeshData_Model.hxx>
#include <IMeshTools_ModelAlgo.hxx>
#include <IMeshTools_Parameters.hxx>
#include <Precision.hxx>
#include <Standard_Failure.hxx>

#include <BRep_Tool.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <Poly_Polygon3D.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangle.hxx>
#include <Poly_Triangulation.hxx>
#include <TopLoc_Location.hxx>

#include <algorithm>
#include <BRepAdaptor_Surface.hxx>
#include <functional>
#include <atomic>
#include <NCollection_DataMap.hxx>
#include <TopoDS_Compound.hxx>
#include <BRep_Builder.hxx>
#include <Geom_Line.hxx>
#include <array>
#include <deque>
#include <cstdlib>

namespace noshape {

namespace {
// The boundary nodes of `f` triangulated in its parameter plane (constrained
// Delaunay, no other nodes) into the face's triangulation; false (nothing
// written) when the face has fewer than `min_nodes` of them or isn't clean
// (self-intersecting or seam wires, a ring the triangulation refuses).
// `curved`: a curved surface, triangulated in its parameters scaled to
// lengths and kept only when every triangle's centre and edge midpoints lie
// within the face's deflection of the surface.
bool cdt_face(const IMeshData::IFaceHandle& f, size_t min_nodes, bool curved)
{
  size_t total = 0;
  for (int w = 0; w < f->WiresNb(); ++w)
  {
    const IMeshData::IWireHandle& W = f->GetWire(w);
    for (int e = 0; e < W->EdgesNb(); ++e)
      total += (size_t)W->GetEdge(e)->GetPCurve(f.get(), W->GetEdgeOrientation(e))->ParametersNb();
  }
  if (total < min_nodes)
    return false;
  std::vector<cdt::Pt>          uv;
  std::vector<gp_Pnt>           xyz;
  std::vector<std::vector<int>> rings;
  uv.reserve(total);
  xyz.reserve(total);
  auto same = [&](int node, const gp_Pnt2d& q) { return std::abs(uv[(size_t)node].x - q.X()) + std::abs(uv[(size_t)node].y - q.Y()) < 1e-12; };
  for (int w = 0; w < f->WiresNb(); ++w)
  {
    const IMeshData::IWireHandle& W = f->GetWire(w);
    if (W->IsSet(IMeshData_SelfIntersectingWire))
      return false;
    std::vector<int> ring;
    int              first = -1;
    for (int e = 0; e < W->EdgesNb(); ++e)
    {
      const IMeshData::IEdgeHandle E = W->GetEdge(e);
      const TopAbs_Orientation     o = W->GetEdgeOrientation(e);
      // One pcurve on this face (no seams), run one way.
      if ((o != TopAbs_FORWARD && o != TopAbs_REVERSED) || E->GetDegenerated() || E->GetPCurves(f.get()).Extent() != 1)
        return false;
      const IMeshData::IPCurveHandle& P = E->GetPCurve(f.get(), o);
      const IMeshData::ICurveHandle&  C = E->GetCurve();
      const int                       n = P->ParametersNb();
      if (n < 2 || C->ParametersNb() != n)
        return false;
      for (int k = 0; k < n; ++k)
      {
        // The points run along the edge; a reversed edge runs the wire backwards.
        const int      idx = o == TopAbs_REVERSED ? n - 1 - k : k;
        const gp_Pnt2d q   = P->GetPoint(idx);
        int            node;
        if (k == 0 && !ring.empty() && same(ring.back(), q))
          node = ring.back(); // the vertex shared with the edge before
        else if (k == n - 1 && e == W->EdgesNb() - 1 && first >= 0 && same(first, q))
          node = first; // back at the wire's start
        else
        {
          node = (int)uv.size();
          uv.push_back({q.X(), q.Y()});
          xyz.push_back(C->GetPoint(idx));
          ring.push_back(node);
          if (first < 0)
            first = node;
        }
        // Poly_PolygonOnTriangulation of the edge: its nodes in the face's triangulation.
        P->GetIndex(idx) = node + 1;
      }
    }
    rings.push_back(std::move(ring));
  }
  if (rings.empty())
    return false;
  // The outer ring first: the largest.
  auto area = [&](const std::vector<int>& r) {
    double s = 0;
    for (size_t k = 0, j = r.size() - 1; k < r.size(); j = k++)
      s += uv[(size_t)r[j]].x * uv[(size_t)r[k]].y - uv[(size_t)r[k]].x * uv[(size_t)r[j]].y;
    return std::abs(s);
  };
  size_t outer = 0;
  for (size_t k = 1; k < rings.size(); ++k)
    if (area(rings[k]) > area(rings[outer]))
      outer = k;
  std::swap(rings[0], rings[outer]);
  // Curved: triangulated in the parameters scaled to lengths at the
  // domain's centre (a thin band stays thin, not a sliver of the unit
  // square), kept only where the triangles lie on the surface.
  std::vector<cdt::Pt> plane = uv;
  if (curved)
  {
    double u0 = uv[0].x, u1 = u0, v0 = uv[0].y, v1 = v0;
    for (const cdt::Pt& q : uv)
    {
      u0 = std::min(u0, q.x), u1 = std::max(u1, q.x);
      v0 = std::min(v0, q.y), v1 = std::max(v1, q.y);
    }
    gp_Pnt p;
    gp_Vec du, dv;
    f->GetSurface()->D1((u0 + u1) / 2, (v0 + v1) / 2, p, du, dv);
    const double su = du.Magnitude(), sv = dv.Magnitude();
    if (!(su > 0) || !(sv > 0))
      return false;
    for (cdt::Pt& q : plane)
      q = {q.x * su, q.y * sv};
  }
  std::vector<int> tris;
  if (!cdt::triangulate(plane, rings, tris))
    return false;
  if (curved)
  {
    const BRepAdaptor_Surface& S   = *f->GetSurface();
    const double               tol = f->GetDeflection();
    auto off = [&](int a, int b, double wa, double wb) {
      const cdt::Pt& p = uv[(size_t)a];
      const cdt::Pt& q = uv[(size_t)b];
      const gp_XYZ   m = xyz[(size_t)a].XYZ() * wa + xyz[(size_t)b].XYZ() * wb;
      return S.Value(p.x * wa + q.x * wb, p.y * wa + q.y * wb).Distance(gp_Pnt(m)) > tol;
    };
    for (size_t t = 0; t < tris.size(); t += 3)
    {
      const int a = tris[t], b = tris[t + 1], c = tris[t + 2];
      const cdt::Pt& pa = uv[(size_t)a];
      const cdt::Pt& pb = uv[(size_t)b];
      const cdt::Pt& pc = uv[(size_t)c];
      const gp_Pnt   centre((xyz[(size_t)a].XYZ() + xyz[(size_t)b].XYZ() + xyz[(size_t)c].XYZ()) / 3);
      if (S.Value((pa.x + pb.x + pc.x) / 3, (pa.y + pb.y + pc.y) / 3).Distance(centre) > tol || off(a, b, 0.5, 0.5) || off(b, c, 0.5, 0.5) || off(c, a, 0.5, 0.5))
        return false;
    }
  }
  occ::handle<Poly_Triangulation> T = new Poly_Triangulation((int)uv.size(), (int)(tris.size() / 3), true);
  for (size_t i = 0; i < uv.size(); ++i)
  {
    T->SetNode((int)i + 1, xyz[i]);
    T->SetUVNode((int)i + 1, gp_Pnt2d(uv[i].x, uv[i].y));
  }
  for (size_t t = 0; t < tris.size(); t += 3)
    T->SetTriangle((int)(t / 3) + 1, Poly_Triangle(tris[t] + 1, tris[t + 1] + 1, tris[t + 2] + 1));
  BRepMesh_ShapeTool::AddInFace(f->GetFace(), T);
  return true;
}

// A planar face (any size: BRepMesh took 35-68 ms on a 144-node disk) through our
// constrained Delaunay triangulation (planar_cdt.hpp) of BRepMesh's own
// boundary nodes: the same triangles as BRepMesh's (Delaunay, no inner
// nodes on a plane) without BRepMesh's per-link bookkeeping (the 400-hole
// plate's faces 1.4 s → 0.1 s). Any face it can't take
// (self-intersecting or seam wires, a ring it refuses) goes to `fallback`
// (BRepMesh's own planar algorithm).
// Planar faces PlanarCdtAlgo gave to BRepMesh's algorithm (mesh_prepare
// reports its meshing's; tests).
std::atomic<size_t> g_planar_fallbacks{0};

class PlanarCdtAlgo : public IMeshTools_MeshAlgo
{
public:
  explicit PlanarCdtAlgo(const occ::handle<IMeshTools_MeshAlgo>& fallback) : myFallback(fallback) {}

  void Perform(const IMeshData::IFaceHandle& f, const IMeshTools_Parameters& prm, const Message_ProgressRange& range) override
  {
    bool done = false;
    try
    {
      done = triangulate(f);
    }
    catch (const Standard_Failure&)
    {
    }
    if (!done)
    {
      ++g_planar_fallbacks;
      myFallback->Perform(f, prm, range);
    }
  }

private:
  static constexpr size_t MIN_NODES = 3;

  static bool triangulate(const IMeshData::IFaceHandle& f) { return cdt_face(f, MIN_NODES, false); }

  occ::handle<IMeshTools_MeshAlgo> myFallback;
};

// A curved face bounded by long rails of edge nodes and nothing to fill
// between them (an imported screw's thread: helical crest, root and flank
// bands, two helix edges of 2300-3000 nodes each and short ends): BRepMesh's
// Delaunay took 0.5-5 s a face in the long thin parameter strip (its circle
// cells crowded by the slivers' circumcircles, then the deflection
// control); the NIST assembly's fine mesh 10-15 s. The boundary nodes
// triangulated as on a plane in the parameters scaled to lengths
// (`cdt_face`, curved), kept when the triangles lie on the surface within
// the face's deflection, else `fallback`.
class BandCdtAlgo : public IMeshTools_MeshAlgo
{
public:
  explicit BandCdtAlgo(const occ::handle<IMeshTools_MeshAlgo>& fallback) : myFallback(fallback) {}

  void Perform(const IMeshData::IFaceHandle& f, const IMeshTools_Parameters& prm, const Message_ProgressRange& range) override
  {
    bool done = false;
    try
    {
      done = cdt_face(f, MIN_NODES, true);
    }
    catch (const Standard_Failure&)
    {
    }
    if (!done)
      myFallback->Perform(f, prm, range);
  }

private:
  // Only faces with many boundary nodes: smaller ones mesh fast as they are,
  // and usually need inner nodes.
  static constexpr size_t MIN_NODES = 1000;

  occ::handle<IMeshTools_MeshAlgo> myFallback;
};

// A cylindrical or conical face bounded by two arcs (or circles) of
// constant height and straight edges along the axis (a hole, a chamfer, a
// band between two planes): the two rings of BRepMesh's own edge nodes
// joined by triangles, no inner nodes. Both surfaces are straight along
// their height, so the triangles lie on the surface between the rings,
// whose sampling (the edges' angular deflection) BRepMesh's Delaunay kept
// as well; without its per-link bookkeeping (the 400-hole plate's 800 such
// faces were ~28 % of its display meshing). Any other face (a seam with
// inner nodes, a degenerated edge, curved trimming) goes to `fallback`.
// NOSHAPE_MESH_NO_STRIPS=1 turns it off (tests compare both).
class StripAlgo : public IMeshTools_MeshAlgo
{
public:
  /// `grid`: inner rows allowed (a torus, curved along v); cylinders and
  /// cones are straight along v and take the rims alone.
  StripAlgo(const occ::handle<IMeshTools_MeshAlgo>& fallback, bool grid) : myFallback(fallback), myGrid(grid) {}

  void Perform(const IMeshData::IFaceHandle& f, const IMeshTools_Parameters& prm, const Message_ProgressRange& range) override
  {
    bool done = false;
    try
    {
      done = triangulate(f, myGrid);
    }
    catch (const Standard_Failure&)
    {
    }
    if (!done)
      myFallback->Perform(f, prm, range);
  }

private:
  struct Node
  {
    double u;
    gp_Pnt p;
  };

  static bool triangulate(const IMeshData::IFaceHandle& f, bool grid)
  {
    // The pcurves' points: the face's two constant-v rings (rims) and its
    // constant-u sides (seams or straight / arc edges at umin and umax).
    struct Side
    {
      IMeshData::IPCurveHandle P;
      IMeshData::ICurveHandle  C;
      int                      n;
    };
    std::vector<Side> sides;
    double            umin = 0, umax = 0, span = 0;
    bool              any  = false;
    for (int w = 0; w < f->WiresNb(); ++w)
    {
      const IMeshData::IWireHandle& W = f->GetWire(w);
      if (W->IsSet(IMeshData_SelfIntersectingWire))
        return false;
      for (int e = 0; e < W->EdgesNb(); ++e)
      {
        const IMeshData::IEdgeHandle E = W->GetEdge(e);
        if (E->GetDegenerated())
          return false;
        const IMeshData::IPCurveHandle& P = E->GetPCurve(f.get(), W->GetEdgeOrientation(e));
        const int                       n = P->ParametersNb();
        if (n < 2 || E->GetCurve()->ParametersNb() != n)
          return false;
        for (int k = 0; k < n; ++k)
        {
          const gp_Pnt2d q = P->GetPoint(k);
          umin             = any ? std::min(umin, q.X()) : q.X();
          umax             = any ? std::max(umax, q.X()) : q.X();
          any              = true;
        }
        sides.push_back({P, E->GetCurve(), n});
      }
    }
    if (!any || sides.size() < 2 || sides.size() > 64)
      return false;
    for (const Side& sd : sides)
      for (int k = 1; k < sd.n; ++k)
      {
        const gp_Pnt2d a = sd.P->GetPoint(0), b = sd.P->GetPoint(k);
        span             = std::max(span, std::max(std::abs(a.X() - b.X()), std::abs(a.Y() - b.Y())));
      }
    const double tol = 1e-9 * std::max(1.0, std::max(span, umax - umin));
    // (height, nodes) of the rims; (u, (v, point)) of the sides.
    std::vector<std::pair<double, std::vector<Node>>>                       rims;
    std::vector<std::pair<double, std::vector<std::pair<double, gp_Pnt>>>> cols;
    for (const Side& sd : sides)
    {
      double v0 = sd.P->GetPoint(0).Y(), v1 = v0, u0 = sd.P->GetPoint(0).X(), u1 = u0;
      for (int k = 1; k < sd.n; ++k)
      {
        v0 = std::min(v0, sd.P->GetPoint(k).Y());
        v1 = std::max(v1, sd.P->GetPoint(k).Y());
        u0 = std::min(u0, sd.P->GetPoint(k).X());
        u1 = std::max(u1, sd.P->GetPoint(k).X());
      }
      if (v1 - v0 <= tol)
      {
        auto it = std::find_if(rims.begin(), rims.end(), [&](const auto& r) { return std::abs(r.first - v0) <= tol; });
        if (it == rims.end())
          it = rims.insert(rims.end(), {v0, {}});
        for (int k = 0; k < sd.n; ++k)
          it->second.push_back({sd.P->GetPoint(k).X(), sd.C->GetPoint(k)});
      }
      else if (u1 - u0 <= tol)
      {
        auto it = std::find_if(cols.begin(), cols.end(), [&](const auto& c) { return std::abs(c.first - u0) <= tol; });
        if (it == cols.end())
          it = cols.insert(cols.end(), {u0, {}});
        for (int k = 0; k < sd.n; ++k)
          it->second.push_back({sd.P->GetPoint(k).Y(), sd.C->GetPoint(k)});
      }
      else
        return false;
    }
    if (rims.size() != 2 || cols.size() > 2)
      return false;
    if (rims[0].first > rims[1].first)
      std::swap(rims[0], rims[1]);
    const double vlo = rims[0].first, vhi = rims[1].first;
    // Each rim sorted by u, shared ends once, from umin to umax.
    for (auto& r : rims)
    {
      std::vector<Node>& nodes = r.second;
      std::sort(nodes.begin(), nodes.end(), [](const Node& a, const Node& b) { return a.u < b.u; });
      std::vector<Node> unique;
      for (const Node& nd : nodes)
        if (unique.empty() || nd.u - unique.back().u > tol)
          unique.push_back(nd);
      if (unique.size() < 2 || std::abs(unique.front().u - umin) > tol || std::abs(unique.back().u - umax) > tol)
        return false;
      nodes = std::move(unique);
    }
    // Each side at umin or umax, sorted by v, from vlo to vhi; both sides
    // with the same heights: the grid's rows.
    std::vector<double> rows;
    for (auto& c : cols)
    {
      if (std::abs(c.first - umin) > tol && std::abs(c.first - umax) > tol)
        return false;
      auto& pts = c.second;
      std::sort(pts.begin(), pts.end(), [](const auto& a, const auto& b) { return a.first < b.first; });
      std::vector<std::pair<double, gp_Pnt>> unique;
      for (const auto& pt : pts)
        if (unique.empty() || pt.first - unique.back().first > tol)
          unique.push_back(pt);
      if (unique.size() < 2 || std::abs(unique.front().first - vlo) > tol || std::abs(unique.back().first - vhi) > tol)
        return false;
      pts = std::move(unique);
      std::vector<double> heights;
      for (const auto& pt : pts)
        heights.push_back(pt.first);
      if (rows.empty())
        rows = std::move(heights);
      else if (heights.size() != rows.size() || !std::equal(rows.begin(), rows.end(), heights.begin(), [&](double a, double b) { return std::abs(a - b) <= tol; }))
        return false;
    }
    if (rows.empty())
      rows = {vlo, vhi};
    const std::vector<Node>& lo = rims[0].second;
    const std::vector<Node>& hi = rims[1].second;
    occ::handle<Poly_Triangulation> T;
    std::function<int(const gp_Pnt2d&)> node_of;
    if (rows.size() == 2)
    {
      // No inner rows (cylinders, cones): zip the rims (counter-clockwise in
      // uv), advancing on the rim whose next node comes first.
      const int nlo = (int)lo.size(), nhi = (int)hi.size();
      T             = new Poly_Triangulation(nlo + nhi, nlo + nhi - 2, true);
      for (int i = 0; i < nlo; ++i)
      {
        T->SetNode(i + 1, lo[(size_t)i].p);
        T->SetUVNode(i + 1, gp_Pnt2d(lo[(size_t)i].u, vlo));
      }
      for (int j = 0; j < nhi; ++j)
      {
        T->SetNode(nlo + j + 1, hi[(size_t)j].p);
        T->SetUVNode(nlo + j + 1, gp_Pnt2d(hi[(size_t)j].u, vhi));
      }
      int i = 0, j = 0, t = 0;
      while (i < nlo - 1 || j < nhi - 1)
      {
        const bool on_lo = j == nhi - 1 || (i < nlo - 1 && lo[(size_t)i + 1].u <= hi[(size_t)j + 1].u);
        if (on_lo)
        {
          T->SetTriangle(++t, Poly_Triangle(i + 1, i + 2, nlo + j + 1));
          ++i;
        }
        else
        {
          T->SetTriangle(++t, Poly_Triangle(i + 1, nlo + j + 2, nlo + j + 1));
          ++j;
        }
      }
      node_of = [&lo, &hi, vlo, nlo, tol](const gp_Pnt2d& q) -> int {
        const bool               low = std::abs(q.Y() - vlo) <= tol;
        const std::vector<Node>& r   = low ? lo : hi;
        auto it = std::lower_bound(r.begin(), r.end(), q.X() - tol, [](const Node& a, double u) { return a.u < u; });
        if (it == r.end() || std::abs(it->u - q.X()) > tol)
          return -1;
        return (int)(it - r.begin()) + (low ? 1 : nlo + 1);
      };
    }
    else
    {
      // Inner rows (a torus band): a grid, the rims' u by the sides' v,
      // inner nodes on the surface, two triangles per cell.
      if (!grid || lo.size() != hi.size() || cols.size() != 2)
        return false;
      for (size_t i = 0; i < lo.size(); ++i)
        if (std::abs(lo[i].u - hi[i].u) > tol)
          return false;
      const occ::handle<BRepAdaptor_Surface>& S = f->GetSurface();
      if (S.IsNull())
        return false;
      const auto& left  = std::abs(cols[0].first - umin) <= tol ? cols[0].second : cols[1].second;
      const auto& right = std::abs(cols[0].first - umin) <= tol ? cols[1].second : cols[0].second;
      const int   n = (int)lo.size(), k = (int)rows.size();
      T             = new Poly_Triangulation(n * k, 2 * (n - 1) * (k - 1), true);
      for (int j = 0; j < k; ++j)
        for (int i = 0; i < n; ++i)
        {
          const double u = lo[(size_t)i].u, v = rows[(size_t)j];
          gp_Pnt       p;
          if (j == 0)
            p = lo[(size_t)i].p;
          else if (j == k - 1)
            p = hi[(size_t)i].p;
          else if (i == 0)
            p = left[(size_t)j].second;
          else if (i == n - 1)
            p = right[(size_t)j].second;
          else
            p = S->Value(u, v);
          T->SetNode(j * n + i + 1, p);
          T->SetUVNode(j * n + i + 1, gp_Pnt2d(u, v));
        }
      int t = 0;
      for (int j = 0; j + 1 < k; ++j)
        for (int i = 0; i + 1 < n; ++i)
        {
          const int a = j * n + i + 1, b = a + 1, c = a + n + 1, d = a + n;
          T->SetTriangle(++t, Poly_Triangle(a, b, c));
          T->SetTriangle(++t, Poly_Triangle(a, c, d));
        }
      node_of = [&lo, &rows, n, tol](const gp_Pnt2d& q) -> int {
        auto r = std::lower_bound(rows.begin(), rows.end(), q.Y() - tol);
        auto c = std::lower_bound(lo.begin(), lo.end(), q.X() - tol, [](const Node& a, double u) { return a.u < u; });
        if (r == rows.end() || std::abs(*r - q.Y()) > tol || c == lo.end() || std::abs(c->u - q.X()) > tol)
          return -1;
        return (int)(r - rows.begin()) * n + (int)(c - lo.begin()) + 1;
      };
    }
    // Every pcurve point's node in this triangulation (the edges' polygons).
    std::vector<std::pair<IMeshData::IPCurveHandle, std::vector<int>>> indices;
    for (const Side& sd : sides)
    {
      std::vector<int> idx((size_t)sd.n);
      for (int k = 0; k < sd.n; ++k)
        if ((idx[(size_t)k] = node_of(sd.P->GetPoint(k))) < 0)
          return false;
      indices.push_back({sd.P, std::move(idx)});
    }
    for (auto& [P, idx] : indices)
      for (int k = 0; k < (int)idx.size(); ++k)
        P->GetIndex(k) = idx[(size_t)k];
    BRepMesh_ShapeTool::AddInFace(f->GetFace(), T);
    return true;
  }

  occ::handle<IMeshTools_MeshAlgo> myFallback;
  bool                             myGrid;
};

// Planar faces through PlanarCdtAlgo (BRepMesh's own algorithm below its
// threshold and as its fallback; not Delabella, which crashed on macOS arm64),
// cylinders, cones and tori through StripAlgo, every curved face StripAlgo
// doesn't take through BandCdtAlgo, then BRepMesh's own.
class PlanarCdtAlgoFactory : public IMeshTools_MeshAlgoFactory
{
public:
  occ::handle<IMeshTools_MeshAlgo> GetAlgo(const GeomAbs_SurfaceType type, const IMeshTools_Parameters& prm) const override
  {
    if (type == GeomAbs_Plane)
      return new PlanarCdtAlgo(myDefault->GetAlgo(type, prm));
    const bool bands = std::getenv("NOSHAPE_MESH_NO_BANDS") == nullptr;
    occ::handle<IMeshTools_MeshAlgo> own = myDefault->GetAlgo(type, prm);
    if (bands)
      own = new BandCdtAlgo(own);
    if ((type == GeomAbs_Cylinder || type == GeomAbs_Cone || type == GeomAbs_Torus) && std::getenv("NOSHAPE_MESH_NO_STRIPS") == nullptr)
      return new StripAlgo(own, type == GeomAbs_Torus);
    return own;
  }

private:
  occ::handle<IMeshTools_MeshAlgoFactory> myDefault = new BRepMesh_MeshAlgoFactory();
};

// BRepMesh's model post-processing without its deflection estimate
// (BRepLib::UpdateDeflection per face, ~5 % of the 400-hole plate's display
// meshing): the edges' polygons on the new triangulations (or 3D polygons of
// free edges), as BRepMesh_ModelPostProcessor commits them. The display only
// reads the triangles and these polygons, and mesh_fill drops both right
// after. NOSHAPE_MESH_BREPMESH_POST=1 uses BRepMesh's own (tests compare).
class DisplayPostProcessor : public IMeshTools_ModelAlgo
{
protected:
  bool performInternal(const occ::handle<IMeshData_Model>& model, const IMeshTools_Parameters&, const Message_ProgressRange&) override
  {
    if (model.IsNull())
      return false;
    for (int e = 0; e < model->EdgesNb(); ++e)
      commit(model->GetEdge(e));
    return true;
  }

private:
  static occ::handle<Poly_PolygonOnTriangulation> polygon(const IMeshData::IPCurveHandle& P, double deflection)
  {
    NCollection_Array1<int>    nodes(1, P->ParametersNb());
    NCollection_Array1<double> params(1, P->ParametersNb());
    for (int i = 1; i <= P->ParametersNb(); ++i)
    {
      nodes(i)  = P->GetIndex(i - 1);
      params(i) = P->GetParameter(i - 1);
    }
    occ::handle<Poly_PolygonOnTriangulation> poly = new Poly_PolygonOnTriangulation(nodes, params);
    poly->Deflection(deflection);
    return poly;
  }

  static void commit(const IMeshData::IEdgeHandle& E)
  {
    const IMeshData::ICurveHandle& C = E->GetCurve();
    if (C->ParametersNb() == 0)
      return;
    if (E->IsFree())
    {
      if (E->IsSet(IMeshData_Reused))
        return;
      NCollection_Array1<gp_Pnt> points(1, C->ParametersNb());
      NCollection_Array1<double> params(1, C->ParametersNb());
      for (int i = 1; i <= C->ParametersNb(); ++i)
      {
        points(i) = C->GetPoint(i - 1);
        params(i) = C->GetParameter(i - 1);
      }
      occ::handle<Poly_Polygon3D> poly = new Poly_Polygon3D(points, params);
      poly->Deflection(E->GetDeflection());
      BRepMesh_ShapeTool::UpdateEdge(E->GetEdge(), poly);
      return;
    }
    // The edge's pcurves per face (two on a seam), in their order.
    std::vector<std::pair<IMeshData::IFacePtr, std::vector<IMeshData::IPCurveHandle>>> faces;
    for (int i = 0; i < E->PCurvesNb(); ++i)
    {
      const IMeshData::IPCurveHandle& P    = E->GetPCurve(i);
      const IMeshData::IFacePtr&      fptr = P->GetFace();
      const IMeshData::IFaceHandle    F    = fptr;
      if (F->IsSet(IMeshData_Failure) || F->IsSet(IMeshData_Reused))
        continue;
      auto it = std::find_if(faces.begin(), faces.end(), [&](const auto& g) { return g.first == fptr; });
      if (it == faces.end())
        faces.push_back({fptr, {P}});
      else
        it->second.push_back(P);
    }
    for (const auto& [fptr, pcurves] : faces)
    {
      TopLoc_Location                        loc;
      const occ::handle<Poly_Triangulation>& tri = BRep_Tool::Triangulation(fptr->GetFace(), loc);
      if (tri.IsNull())
        continue;
      if (pcurves.size() == 2)
        BRepMesh_ShapeTool::UpdateEdge(E->GetEdge(), polygon(pcurves.front(), E->GetDeflection()), polygon(pcurves.back(), E->GetDeflection()), tri, loc);
      else
        BRepMesh_ShapeTool::UpdateEdge(E->GetEdge(), polygon(pcurves.front(), E->GetDeflection()), tri, loc);
    }
  }
};

struct FaceTri
{
  occ::handle<Poly_Triangulation> tri;
  gp_Trsf                         trsf;
  bool                            identity = true;
  bool                            reversed = false;
};

// Display meshes made before, by face and edge (TShape and location) and the
// deflections: the faces a regeneration left alone take theirs from here, and
// BRepMesh meshes only the others. It re-meshed every face of a part after
// each feature (~340 ms on the 400-fin plate, as long again when every face
// already had a mesh); the shapes' own triangulations are dropped after each
// mesh (BRepTools::Clean below), so the meshes are kept here. A set serves
// absolute deflections within 1 % of its own (the display's scale with the
// part's diagonal, which a small feature barely changes); the latest three
// sets, each up to 20 M nodes. Under mesh_mutex. NOSHAPE_MESH_CACHE=0: off.
struct MeshSet
{
  double lin = 0, ang = 0;
  NCollection_DataMap<TopoDS_Shape, occ::handle<Poly_Triangulation>, TopTools_ShapeMapHasher> faces;
  NCollection_DataMap<TopoDS_Shape, std::vector<std::array<float, 3>>, TopTools_ShapeMapHasher> edges;
  size_t                                                                                        nodes = 0;
};

MeshSet* mesh_set(double lin, double ang)
{
  static const bool off = [] {
    const char* v = std::getenv("NOSHAPE_MESH_CACHE");
    return v != nullptr && std::string(v) == "0";
  }();
  if (off)
    return nullptr;
  static std::deque<MeshSet> sets;
  for (MeshSet& m : sets)
    if (m.ang == ang && std::abs(m.lin - lin) <= 0.01 * lin)
    {
      if (m.nodes > 20000000)
        m = MeshSet{lin, ang, {}, {}, 0};
      return &m;
    }
  sets.push_front(MeshSet{lin, ang, {}, {}, 0});
  while (sets.size() > 3)
    sets.pop_back();
  return &sets.front();
}
} // namespace

struct OcMeshFaces
{
  std::vector<FaceTri> faces;
};

std::unique_ptr<OcMesh> mesh_prepare(const OcShape& s, double lin, double ang, bool relative,
                                     bool parallel)
{
  require_non_null(s.shape, "shape");
  if (!(lin > 0) || !std::isfinite(lin))
    fail_invalid("linear deflection must be positive");
  if (!(ang > 0) || !std::isfinite(ang))
    fail_invalid("angular deflection must be positive");
  // The shape's maps and its edges' faces, kept on its OcShape (the
  // regeneration's naming made them already): walking a 1600-face part for
  // them took ~25 ms of each display mesh.
  std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("mesh.maps");
  auto m   = std::make_unique<OcMesh>(s);
  m->shape = s.shape;
  auto side = std::make_shared<OcMeshFaces>();
  side->faces.resize(m->faces.Extent());
  // Per edge (1-based) its faces' indices (0-based into m->faces).
  const auto& ef        = ancestors_cached(s, TopAbs_EDGE, TopAbs_FACE);
  auto        faces_of  = [&](int e) { return std::make_pair(ef.second.data() + ef.first[(size_t)e - 1], ef.second.data() + ef.first[(size_t)e]); };
  {
    const LockedSection locked_section_721;
    std::lock_guard<std::mutex> lock(mesh_mutex());
    IMeshTools_Parameters prm;
    prm.Deflection               = lin;
    prm.Angle                    = ang;
    prm.Relative                 = relative;
    prm.InParallel               = parallel;
    prm.MinSize                  = Precision::Confusion();
    // Faces whose stored triangulation already satisfies the deflection are
    // skipped by BRepMesh (mesh faces; display meshes are dropped below).
    // Not Delabella for planar faces: it dereferences null on some of them
    // on macOS arm64 (features_a pattern_instance_references_survive_upstream_edit,
    // v0.4.6 CI run 39827462890).
    occ::handle<BRepMesh_Context> ctx = new BRepMesh_Context();
    ctx->SetFaceDiscret(new BRepMesh_FaceDiscret(new PlanarCdtAlgoFactory()));
    if (std::getenv("NOSHAPE_MESH_BREPMESH_POST") == nullptr)
      ctx->SetPostProcessor(new DisplayPostProcessor());
    lap = std::make_unique<KernelTimer>("mesh.cache");
    // The faces the cache has (absolute deflections; no mesh faces, whose
    // triangulation is their geometry): only the others are meshed, alone,
    // when every edge they share with a cached face is straight (a curved
    // one is discretised for both its faces: it is meshed whole then).
    bool mesh_faces = false;
    for (int i = 1; i <= m->faces.Extent() && !mesh_faces; ++i)
      mesh_faces = is_mesh_face(TopoDS::Face(m->faces(i)));
    MeshSet*          set = relative || mesh_faces ? nullptr : mesh_set(lin, ang);
    std::vector<char> fresh((size_t)m->faces.Extent() + 1, 1);
    int               nfresh = m->faces.Extent();
    if (set != nullptr)
      for (int i = 1; i <= m->faces.Extent(); ++i)
        if (set->faces.IsBound(m->faces(i)))
          fresh[(size_t)i] = 0, --nfresh;
    bool partial = set != nullptr && nfresh < m->faces.Extent();
    for (int i = 1; i <= m->edges.Extent() && partial; ++i)
    {
      const TopoDS_Edge& e = TopoDS::Edge(m->edges(i));
      int                nf = 0, nc = 0;
      for (auto [f, fe] = faces_of(i); f != fe; ++f, ++nf)
        nc += fresh[(size_t)*f + 1] ? 0 : 1;
      if (nc == 0)
        continue;
      if (nc == nf)
      {
        partial = set->edges.IsBound(e); // its polyline, made with its faces
        continue;
      }
      double                        a, b;
      TopLoc_Location               l;
      const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, l, a, b);
      partial                         = !c.IsNull() && c->IsKind(STANDARD_TYPE(Geom_Line));
    }
    if (std::getenv("NOSHAPE_MESH_DEBUG"))
      std::fprintf(stderr, "mesh: %d faces, %d not cached, %s\n", m->faces.Extent(), nfresh, partial ? "partial" : set == nullptr ? "no cache" : "whole");
    TopoDS_Shape meshed = s.shape;
    if (!partial)
      std::fill(fresh.begin(), fresh.end(), 1);
    // Faces noshape's own mesher takes (psui, mesh_rust.cpp) aren't BRepMesh's.
    std::vector<char>                              taken((size_t)m->faces.Extent() + 1, 0);
    std::vector<occ::handle<Poly_Triangulation>>   rust_tris((size_t)m->faces.Extent() + 1);
    std::vector<std::vector<std::array<float, 3>>> rust_polys((size_t)m->edges.Extent());
    const int ntaken = !relative && !mesh_faces && rust_mesh_on() ? rust_mesh_faces(m->faces, m->edges, m->vertices, fresh, lin, ang, taken, rust_tris, rust_polys) : 0;
    m->rust_faces            = (size_t)ntaken;
    if (partial || ntaken > 0)
    {
      BRep_Builder    B;
      TopoDS_Compound c;
      B.MakeCompound(c);
      for (int i = 1; i <= m->faces.Extent(); ++i)
        if (fresh[(size_t)i] && !taken[(size_t)i])
          B.Add(c, m->faces(i));
      meshed = c;
    }
    if (meshed.ShapeType() != TopAbs_COMPOUND || meshed.NbChildren() > 0)
    {
      BRepMesh_IncrementalMesh mesher;
      mesher.SetShape(meshed);
      mesher.ChangeParameters() = prm;
      const KernelTimer timer("mesh.brepmesh");
      const size_t      fallbacks = g_planar_fallbacks.load();
      mesher.Perform(ctx);
      m->planar_fallbacks = g_planar_fallbacks.load() - fallbacks;
      if (std::getenv("NOSHAPE_MESH_DEBUG"))
        std::fprintf(stderr, "mesh: %zu planar faces to BRepMesh's algorithm\n", m->planar_fallbacks);
    }
    for (int i = 1; i <= m->faces.Extent(); ++i)
    {
      const TopoDS_Face& f = TopoDS::Face(m->faces(i));
      TopLoc_Location    loc;
      if (!fresh[(size_t)i])
      {
        // From the cache, in the face's frame.
        const occ::handle<Poly_Triangulation>& tri = set->faces.Find(f);
        FaceTri&                               ft  = side->faces[i - 1];
        ft.reversed                                = f.Orientation() == TopAbs_REVERSED;
        ft.tri                                     = tri;
        ft.identity                                = f.Location().IsIdentity();
        if (!ft.identity)
          ft.trsf = f.Location().Transformation();
        m->nodes += tri->NbNodes();
        m->indices += 3 * (size_t)tri->NbTriangles();
        continue;
      }
      // noshape's mesher's (in the face's frame), else BRepMesh's.
      const occ::handle<Poly_Triangulation> tri = taken[(size_t)i] ? rust_tris[(size_t)i] : BRep_Tool::Triangulation(f, loc);
      if (taken[(size_t)i])
        loc = f.Location();
      FaceTri& ft = side->faces[i - 1];
      ft.reversed = f.Orientation() == TopAbs_REVERSED;
      if (tri.IsNull())
        continue;
      if (is_mesh_face(f))
      {
        // A mesh body's face: drawn faceted (a normal per triangle), like Onshape's mesh parts.
        occ::handle<Poly_Triangulation> flat = new Poly_Triangulation(3 * tri->NbTriangles(), tri->NbTriangles(), false);
        flat->AddNormals();
        for (int k = 1; k <= tri->NbTriangles(); ++k)
        {
          int a, b, c;
          tri->Triangle(k).Get(a, b, c);
          const gp_Pnt pa = tri->Node(a), pb = tri->Node(b), pc = tri->Node(c);
          gp_Vec       n  = gp_Vec(pa, pb).Crossed(gp_Vec(pa, pc));
          if (n.Magnitude() > 0)
            n.Normalize();
          for (int j = 0; j < 3; ++j)
          {
            flat->SetNode(3 * k - 2 + j, j == 0 ? pa : j == 1 ? pb : pc);
            flat->SetNormal(3 * k - 2 + j, gp_Dir(n.Magnitude() > 0 ? n : gp_Vec(0, 0, 1)));
          }
          flat->SetTriangle(k, Poly_Triangle(3 * k - 2, 3 * k - 1, 3 * k));
        }
        ft.tri      = flat;
        ft.identity = loc.IsIdentity();
        if (!ft.identity)
          ft.trsf = loc.Transformation();
        m->nodes += flat->NbNodes();
        m->indices += 3 * (size_t)flat->NbTriangles();
        continue;
      }
      if (!tri->HasNormals())
        BRepLib_ToolTriangulatedShape::ComputeNormals(f, tri);
      ft.tri      = tri;
      ft.identity = loc.IsIdentity();
      if (!ft.identity)
        ft.trsf = loc.Transformation();
      m->nodes += tri->NbNodes();
      m->indices += 3 * (size_t)tri->NbTriangles();
      // Kept for the next mesh when the face's own frame is the one the
      // triangulation came in.
      if (set != nullptr && loc.IsEqual(f.Location()))
      {
        if (!set->faces.IsBound(f))
          set->nodes += (size_t)tri->NbNodes();
        set->faces.Bind(f, tri);
      }
    }

    lap = std::make_unique<KernelTimer>("mesh.edges");
    // Edge polylines (inside the lock: reads polygons stored on edges).
    m->edge_polys.resize(m->edges.Extent());
    for (int i = 1; i <= m->edges.Extent(); ++i)
    {
      const TopoDS_Edge& e    = TopoDS::Edge(m->edges(i));
      auto&              poly = m->edge_polys[i - 1];
      const auto [f0, f1] = faces_of(i);
      // Not edges in Onshape: not drawn, so not pickable (shown_topology): a
      // seam closes every face it bounds.
      bool seam = f0 != f1;
      for (const uint32_t* f = f0; f != f1 && seam; ++f)
        seam = BRep_Tool::IsClosed(e, TopoDS::Face(m->faces((int)*f + 1)));
      if (BRep_Tool::Degenerated(e) || seam)
        continue;
      // None of its faces meshed now: its polyline from the cache.
      bool cached = set != nullptr && f0 != f1;
      for (const uint32_t* f = f0; f != f1 && cached; ++f)
        cached = !fresh[(size_t)*f + 1];
      if (cached)
        if (const std::vector<std::array<float, 3>>* c = set->edges.Seek(e))
        {
          poly = *c;
          if (e.Orientation() == TopAbs_REVERSED)
            std::reverse(poly.begin(), poly.end());
          m->edge_points += poly.size();
          continue;
        }
      bool done = false;
      // noshape's mesher's nodes.
      if (!rust_polys[(size_t)i - 1].empty())
      {
        poly = rust_polys[(size_t)i - 1];
        done = true;
      }
      {
        for (const uint32_t* fi = f0; fi != f1 && !done; ++fi)
        {
          const TopoDS_Face& f = TopoDS::Face(m->faces((int)*fi + 1));
          TopLoc_Location    floc;
          const occ::handle<Poly_Triangulation>& tri = BRep_Tool::Triangulation(f, floc);
          if (tri.IsNull())
            continue;
          TopLoc_Location eloc;
          const occ::handle<Poly_PolygonOnTriangulation>& pot =
            BRep_Tool::PolygonOnTriangulation(e, tri, eloc);
          if (pot.IsNull())
            continue;
          const bool ident = eloc.IsIdentity();
          gp_Trsf    t     = ident ? gp_Trsf() : eloc.Transformation();
          poly.reserve(pot->NbNodes());
          for (int k = 1; k <= pot->NbNodes(); ++k)
          {
            const int ni = pot->Node(k);
            if (ni < 1 || ni > tri->NbNodes())
              continue;
            gp_Pnt p = tri->Node(ni);
            if (!ident)
              p.Transform(t);
            poly.push_back({(float)p.X(), (float)p.Y(), (float)p.Z()});
          }
          done = !poly.empty();
        }
      }
      if (!done)
      {
        TopLoc_Location                    l3;
        const occ::handle<Poly_Polygon3D>& p3 = BRep_Tool::Polygon3D(e, l3);
        if (!p3.IsNull())
        {
          gp_Trsf t = l3.IsIdentity() ? gp_Trsf() : l3.Transformation();
          for (int k = p3->Nodes().Lower(); k <= p3->Nodes().Upper(); ++k)
          {
            gp_Pnt p = p3->Nodes().Value(k);
            if (!l3.IsIdentity())
              p.Transform(t);
            poly.push_back({(float)p.X(), (float)p.Y(), (float)p.Z()});
          }
          done = !poly.empty();
        }
      }
      if (!done)
      {
        try
        {
          BRepAdaptor_Curve           c(e);
          GCPnts_TangentialDeflection d(c, ang, relative ? lin * 0.01 : lin);
          for (int k = 1; k <= d.NbPoints(); ++k)
          {
            gp_Pnt p = d.Value(k);
            poly.push_back({(float)p.X(), (float)p.Y(), (float)p.Z()});
          }
        }
        catch (const Standard_Failure&)
        {
          poly.clear();
        }
      }
      if (set != nullptr)
        set->edges.Bind(e, poly);
      if (e.Orientation() == TopAbs_REVERSED)
        std::reverse(poly.begin(), poly.end());
      m->edge_points += poly.size();
    }
    // The display mesh is copied out (the handles above keep it): drop it
    // from the model's faces. OCCT's booleans take their boxes from a
    // stored triangulation, enlarged by its deflection: a coarse display
    // mesh made a later boolean on the shown body 5x slower and deaf to an
    // abort for seconds (400 fins: 2.4 s -> 14 s; v0.4.8 delete-while-
    // regenerating gate). Mesh faces (no surface) keep theirs.
    lap = std::make_unique<KernelTimer>("mesh.clean");
    BRepTools::Clean(meshed);
  }
  m->cleaned  = true;
  m->faceData = side;
  return m;
}

MeshCounts mesh_counts(const OcMesh& m)
{
  MeshCounts c{};
  c.nodes       = m.nodes;
  c.indices     = m.indices;
  c.faces       = (size_t)m.faces.Extent();
  c.edge_points = m.edge_points;
  c.edges       = (size_t)m.edges.Extent();
  c.vertices    = (size_t)m.vertices.Extent();
  c.planar_fallbacks = m.planar_fallbacks;
  c.rust_faces       = m.rust_faces;
  return c;
}

void mesh_fill(const OcMesh& m, rust::Slice<float> positions, rust::Slice<float> normals,
               rust::Slice<uint32_t> indices, rust::Slice<uint32_t> face_ranges,
               rust::Slice<float> edge_points, rust::Slice<uint32_t> edge_ranges,
               rust::Slice<float> vertices)
{
  const std::shared_ptr<OcMeshFaces>& side = m.faceData;
  if (!side)
    fail_invalid("mesh has no face data");
  const size_t nf = (size_t)m.faces.Extent();
  if (positions.size() != 3 * m.nodes || normals.size() != 3 * m.nodes ||
      indices.size() != m.indices || face_ranges.size() != 4 * nf ||
      edge_points.size() != 3 * m.edge_points || edge_ranges.size() != 2 * m.edge_polys.size() ||
      vertices.size() != 3 * (size_t)m.vertices.Extent())
    fail_invalid("mesh buffer sizes do not match mesh counts");

  size_t vbase = 0, ibase = 0;
  for (size_t fi = 0; fi < nf; ++fi)
  {
    const FaceTri& ft = side->faces[fi];
    face_ranges[4 * fi + 0] = (uint32_t)ibase;
    face_ranges[4 * fi + 2] = (uint32_t)vbase;
    if (ft.tri.IsNull())
    {
      face_ranges[4 * fi + 1] = 0;
      face_ranges[4 * fi + 3] = 0;
      continue;
    }
    const Poly_Triangulation& tri = *ft.tri;
    const int                 nn  = tri.NbNodes();
    const int                 nt  = tri.NbTriangles();
    const bool                hasN = tri.HasNormals();
    const float               sgn  = ft.reversed ? -1.0f : 1.0f;
    for (int k = 1; k <= nn; ++k)
    {
      gp_Pnt p = tri.Node(k);
      if (!ft.identity)
        p.Transform(ft.trsf);
      float* pp = &positions[3 * (vbase + k - 1)];
      pp[0]     = (float)p.X();
      pp[1]     = (float)p.Y();
      pp[2]     = (float)p.Z();
      float* np = &normals[3 * (vbase + k - 1)];
      if (hasN)
      {
        NCollection_Vec3<float> n;
        tri.Normal(k, n);
        if (!ft.identity)
        {
          gp_Dir d(n.x(), n.y(), n.z());
          d.Transform(ft.trsf);
          n = NCollection_Vec3<float>((float)d.X(), (float)d.Y(), (float)d.Z());
        }
        float len = std::sqrt(n.x() * n.x() + n.y() * n.y() + n.z() * n.z());
        if (len > 0)
          len = 1.0f / len;
        np[0] = n.x() * len * sgn;
        np[1] = n.y() * len * sgn;
        np[2] = n.z() * len * sgn;
      }
      else
      {
        np[0] = 0;
        np[1] = 0;
        np[2] = 1;
      }
    }
    for (int t = 1; t <= nt; ++t)
    {
      int n1, n2, n3;
      tri.Triangle(t).Get(n1, n2, n3);
      if (ft.reversed)
        std::swap(n2, n3);
      uint32_t* ip = &indices[ibase + 3 * (t - 1)];
      ip[0]        = (uint32_t)(vbase + n1 - 1);
      ip[1]        = (uint32_t)(vbase + n2 - 1);
      ip[2]        = (uint32_t)(vbase + n3 - 1);
    }
    face_ranges[4 * fi + 1] = (uint32_t)(3 * nt);
    face_ranges[4 * fi + 3] = (uint32_t)nn;
    vbase += nn;
    ibase += 3 * (size_t)nt;
  }

  size_t ep = 0;
  for (size_t ei = 0; ei < m.edge_polys.size(); ++ei)
  {
    const auto& poly        = m.edge_polys[ei];
    edge_ranges[2 * ei + 0] = (uint32_t)ep;
    edge_ranges[2 * ei + 1] = (uint32_t)poly.size();
    for (const auto& p : poly)
    {
      edge_points[3 * ep + 0] = p[0];
      edge_points[3 * ep + 1] = p[1];
      edge_points[3 * ep + 2] = p[2];
      ++ep;
    }
  }
  for (int vi = 1; vi <= m.vertices.Extent(); ++vi)
  {
    gp_Pnt p                   = BRep_Tool::Pnt(TopoDS::Vertex(m.vertices(vi)));
    vertices[3 * (vi - 1) + 0] = (float)p.X();
    vertices[3 * (vi - 1) + 1] = (float)p.Y();
    vertices[3 * (vi - 1) + 2] = (float)p.Z();
  }
  // The buffers are filled: drop the triangulations BRepMesh stored on the
  // shape's faces and edges. The shape lives on (a regeneration's cached
  // context), and OCCT's booleans on a meshed shape are several times slower
  // (fins x400, undoing the delete of Linear pattern 2: its fuse took 12 s on
  // the shown body against 2.5 s; docs/handoff/viewport-performance.md).
  // Faces whose triangulation is their only geometry (mesh bodies) keep it
  // (BRepTools::Clean).
  if (m.cleaned)
    return; // mesh_prepare did
  const LockedSection locked_section_1121;
  std::lock_guard<std::mutex> lock(mesh_mutex());
  BRepTools::Clean(m.shape);
}

bool has_triangulation(const OcShape& s)
{
  for (TopExp_Explorer e(s.shape, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    TopLoc_Location    loc;
    if (!is_mesh_face(f) && !BRep_Tool::Triangulation(f, loc).IsNull())
      return true;
  }
  return false;
}

} // namespace noshape
