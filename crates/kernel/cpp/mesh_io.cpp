// Mesh import (STL, OBJ; PVZ meshes through mesh_shape): triangles become planar faces of one connected
// shell (shared vertices and edges, no sewing pass), closed shells become
// solids, and coplanar neighbours are merged back into larger faces.
#include "internal.hpp"

#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRep_Builder.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <TopExp.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <functional>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <TopExp_Explorer.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepLib.hxx>
#include <BRep_Tool.hxx>
#include <Geom_Plane.hxx>
#include <Poly_Triangulation.hxx>
#include <RWObj.hxx>
#include <RWStl.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Pln.hxx>

#include <algorithm>
#include <array>
#include <map>
#include <unordered_map>
#include <vector>

namespace noshape {

namespace {

// Triangles whose edge runs through another vertex of the mesh (a
// T-junction: the faces of a per-face tessellation sample a shared edge
// differently) are split there, so the shell closes. Only edges without an
// opposite half-edge are looked at.
void split_t_junctions(const std::vector<gp_Pnt>& pts, std::vector<std::array<int, 3>>& tris, double tol)
{
  for (int pass = 0; pass < 8; ++pass)
  {
    std::map<std::pair<int, int>, int> half; // half-edge -> triangle
    for (size_t t = 0; t < tris.size(); ++t)
      for (int k = 0; k < 3; ++k)
        half[{tris[t][k], tris[t][(k + 1) % 3]}] = (int)t;
    std::vector<std::pair<int, int>> open;
    std::vector<int>                 on_open;
    for (const auto& [e, t] : half)
      if (!half.count({e.second, e.first}))
      {
        open.push_back(e);
        on_open.push_back(e.first);
        on_open.push_back(e.second);
      }
    if (open.empty())
      return;
    std::sort(on_open.begin(), on_open.end());
    on_open.erase(std::unique(on_open.begin(), on_open.end()), on_open.end());
    bool changed = false;
    std::vector<bool> done(tris.size(), false);
    for (const auto& [a, c] : open)
    {
      const int t = half[{a, c}];
      if (done[t])
        continue;
      const gp_Vec d(pts[a], pts[c]);
      const double l2 = d.SquareMagnitude();
      if (l2 <= tol * tol)
        continue;
      // The vertex on the edge nearest its start.
      int    best = -1;
      double bs   = 1.0;
      for (int w : on_open)
      {
        if (w == a || w == c)
          continue;
        const gp_Vec aw(pts[a], pts[w]);
        const double s = aw.Dot(d) / l2;
        if (s <= 0 || s >= bs || (aw - d * s).SquareMagnitude() > tol * tol || s * s * l2 <= tol * tol || (1 - s) * (1 - s) * l2 <= tol * tol)
          continue;
        best = w;
        bs   = s;
      }
      if (best < 0)
        continue;
      std::array<int, 3>& tr = tris[t];
      int                 k  = 0;
      while (!(tr[k] == a && tr[(k + 1) % 3] == c))
        ++k;
      const int o = tr[(k + 2) % 3];
      tr          = {a, best, o};
      tris.push_back({best, c, o});
      done[t] = true;
      done.push_back(true);
      changed = true;
    }
    if (!changed)
      return;
  }
}

} // namespace

void mesh_weld(const std::vector<gp_Pnt>& nodes, std::vector<std::array<int, 3>>& tris, double weld, std::vector<gp_Pnt>& pts)
{
  const int nn = (int)nodes.size();
  // Merge coincident nodes (STL repeats them per triangle).
  double scale = 0.0;
  for (const gp_Pnt& p : nodes)
    scale = std::max({scale, std::abs(p.X()), std::abs(p.Y()), std::abs(p.Z())});
  const double q = weld > 0 ? weld : std::max(scale, 1e-9) * 1e-7; // quantization step
  std::map<std::array<long long, 3>, int> by_pos;
  std::vector<int> remap(nn, 0);
  pts.clear();
  for (int i = 0; i < nn; ++i)
  {
    const gp_Pnt p = nodes[i];
    const std::array<long long, 3> key = {std::llround(p.X() / q), std::llround(p.Y() / q), std::llround(p.Z() / q)};
    auto it = by_pos.find(key);
    // With a weld distance, a node within it may have rounded to a neighbouring cell.
    for (int n = 0; weld > 0 && it == by_pos.end() && n < 27; ++n)
    {
      const std::array<long long, 3> k2 = {key[0] + n % 3 - 1, key[1] + n / 3 % 3 - 1, key[2] + n / 9 - 1};
      auto                           j  = by_pos.find(k2);
      if (j != by_pos.end() && pts[j->second].Distance(p) <= weld)
        it = j;
    }
    if (it != by_pos.end())
    {
      remap[i] = it->second;
      continue;
    }
    remap[i] = (int)pts.size();
    by_pos.emplace(key, remap[i]);
    pts.push_back(p);
  }
  for (auto& t : tris)
    for (int& i : t)
      i = remap[i];
  tris.erase(std::remove_if(tris.begin(), tris.end(), [](const std::array<int, 3>& t) { return t[0] == t[1] || t[1] == t[2] || t[0] == t[2]; }), tris.end());
  if (weld > 0)
    split_t_junctions(pts, tris, weld);
}

namespace {

// Shell (solid when closed) of planar triangle faces. `weld` > 0: nodes
// closer than that are one vertex and T-junctions are split; 0: nodes on the
// same 1e-7-of-the-size grid point are one vertex (STL / OBJ).
TopoDS_Shape shape_from_triangles(const std::vector<gp_Pnt>& nodes, std::vector<std::array<int, 3>> tris, uint32_t merge_limit, double weld)
{
  std::vector<gp_Pnt> vpts;
  mesh_weld(nodes, tris, weld, vpts);
  double scale = 0.0;
  for (const gp_Pnt& p : vpts)
    scale = std::max({scale, std::abs(p.X()), std::abs(p.Y()), std::abs(p.Z())});
  const double tol = weld > 0 ? weld : std::max(scale, 1.0) * 1e-9;
  BRep_Builder b;
  std::vector<TopoDS_Vertex> verts;
  for (const gp_Pnt& p : vpts)
  {
    TopoDS_Vertex v;
    b.MakeVertex(v, p, tol);
    verts.push_back(v);
  }
  std::unordered_map<unsigned long long, TopoDS_Edge> edges;
  auto edge = [&](int a, int c) -> TopoDS_Edge {
    const int lo = std::min(a, c), hi = std::max(a, c);
    const unsigned long long k = ((unsigned long long)lo << 32) | (unsigned)hi;
    auto it = edges.find(k);
    TopoDS_Edge e;
    if (it == edges.end())
    {
      e = BRepBuilderAPI_MakeEdge(verts[lo], verts[hi]).Edge();
      edges.emplace(k, e);
    }
    else
      e = it->second;
    return lo == a ? e : TopoDS::Edge(e.Reversed());
  };
  TopoDS_Shell shell;
  b.MakeShell(shell);
  int faces = 0;
  for (const auto& tr : tris)
  {
    const int a = tr[0], c = tr[1], d = tr[2];
    const gp_Pnt pa = BRep_Tool::Pnt(verts[a]), pc = BRep_Tool::Pnt(verts[c]), pd = BRep_Tool::Pnt(verts[d]);
    const gp_Vec n = gp_Vec(pa, pc).Crossed(gp_Vec(pa, pd));
    if (n.Magnitude() < tol * tol)
      continue;
    TopoDS_Wire w;
    b.MakeWire(w);
    b.Add(w, edge(a, c));
    b.Add(w, edge(c, d));
    b.Add(w, edge(d, a));
    BRepBuilderAPI_MakeFace mf(gp_Pln(pa, gp_Dir(n)), w, true);
    if (!mf.IsDone())
      continue;
    b.Add(shell, mf.Face());
    ++faces;
  }
  if (faces == 0)
    fail_io("the mesh has no valid triangles");
  BRepLib::BuildCurves3d(shell);
  TopoDS_Shape result = shell;
  shell.Closed(BRep_Tool::IsClosed(shell));
  if (shell.Closed())
  {
    TopoDS_Solid solid;
    b.MakeSolid(solid);
    b.Add(solid, shell);
    result = solid;
  }
  if ((uint32_t)faces <= merge_limit)
  {
    try
    {
      ShapeUpgrade_UnifySameDomain u(result, true, true, false);
      u.Build();
      if (!u.Shape().IsNull())
        result = u.Shape();
    }
    catch (...)
    {
      // Keep the triangle faces.
    }
  }
  return result;
}

TopoDS_Shape shape_from_triangulation(const occ::handle<Poly_Triangulation>& tri, uint32_t merge_limit)
{
  std::vector<gp_Pnt> nodes;
  for (int i = 1; i <= tri->NbNodes(); ++i)
    nodes.push_back(tri->Node(i));
  std::vector<std::array<int, 3>> tris;
  for (int t = 1; t <= tri->NbTriangles(); ++t)
  {
    int i1, i2, i3;
    tri->Triangle(t).Get(i1, i2, i3);
    tris.push_back({i1 - 1, i2 - 1, i3 - 1});
  }
  return shape_from_triangles(nodes, std::move(tris), merge_limit, 0.0);
}

} // namespace

TopoDS_Shape mesh_shape(const std::vector<gp_Pnt>& nodes, const std::vector<std::array<int, 3>>& tris, uint32_t merge_limit, double weld)
{
  return shape_from_triangles(nodes, tris, merge_limit, weld);
}

namespace {
occ::handle<Poly_Triangulation> read_triangulation(const std::string& p)
{
  std::string lower = p;
  for (auto& ch : lower)
    ch = (char)std::tolower((unsigned char)ch);
  occ::handle<Poly_Triangulation> tri;
  if (lower.size() >= 4 && lower.compare(lower.size() - 4, 4, ".obj") == 0)
    tri = RWObj::ReadFile(p.c_str());
  else
    tri = RWStl::ReadFile(p.c_str());
  if (tri.IsNull() || tri->NbTriangles() == 0)
    fail_io("could not read mesh file " + p);
  return tri;
}
} // namespace

// STL / OBJ as Onshape imports them: mesh bodies (mesh_body.cpp), coordinates
// as in the file; 3MF / glTF / GLB likewise, in millimetres.
std::unique_ptr<OcShape> read_mesh_bodies(rust::Str path)
{
  {
    // 3MF / glTF: their own readers (millimetres).
    std::vector<gp_Pnt>             nodes;
    std::vector<std::array<int, 3>> tris;
    if (read_mesh_format(std::string(path.data(), path.size()), nodes, tris))
      return wrap(mesh_compound(mesh_bodies(nodes, std::move(tris), 0.0)));
  }
  const occ::handle<Poly_Triangulation> tri = read_triangulation(std::string(path.data(), path.size()));
  std::vector<gp_Pnt>                   nodes;
  for (int i = 1; i <= tri->NbNodes(); ++i)
    nodes.push_back(tri->Node(i));
  std::vector<std::array<int, 3>> tris;
  for (int t = 1; t <= tri->NbTriangles(); ++t)
  {
    int a, b, c;
    tri->Triangle(t).Get(a, b, c);
    tris.push_back({a - 1, b - 1, c - 1});
  }
  return wrap(mesh_compound(mesh_bodies(nodes, std::move(tris), 0.0)));
}

bool is_mesh(const OcShape& s)
{
  return !s.shape.IsNull() && has_mesh_faces(s.shape);
}

std::unique_ptr<OcShape> faceted_mesh(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  MeshFacets m;
  return wrap(facet_mesh(s.shape, m));
}

// The sharp offset `r` (of planar faces) with its edges that are convex (outward, `d` > 0) or concave (inward)
// rounded at |d|: the true offset, whose edges there are arcs about the original's edges. `r` when OCCT can't.
static gp_Pnt edge_mid(const TopoDS_Edge& e)
{
  BRepAdaptor_Curve c(e);
  return c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
}

// `r`'s edges at `mids` rounded at `radius`; null when OCCT can't.
static TopoDS_Shape fillet_at(const TopoDS_Shape& r, const std::vector<gp_Pnt>& mids, double radius)
{
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(r, TopAbs_EDGE, edges);
  BRepFilletAPI_MakeFillet fil(r);
  int                      n = 0;
  for (int i = 1; i <= edges.Extent(); ++i)
  {
    const gp_Pnt m = edge_mid(TopoDS::Edge(edges(i)));
    if (std::any_of(mids.begin(), mids.end(), [&](const gp_Pnt& p) { return p.Distance(m) < 1e-9; }))
    {
      fil.Add(radius, TopoDS::Edge(edges(i)));
      ++n;
    }
  }
  if (n != (int)mids.size())
    return {};
  try
  {
    fil.Build();
    if (fil.IsDone() && BRepCheck_Analyzer(fil.Shape()).IsValid())
      return fil.Shape();
  }
  catch (const Standard_Failure&)
  {
  }
  return {};
}

// The sharp offset `r` (of planar faces) with its edges that are convex (outward, `d` > 0) or concave (inward)
// rounded at |d|: the true offset, whose edges there are arcs about the original's edges. All at once, else each
// connected run of them that OCCT can round (a hole's rim of facets it can't stays sharp).
static TopoDS_Shape rounded(const TopoDS_Shape& r, double d)
{
  TopTools_IndexedDataMapOfShapeListOfShape ef;
  TopExp::MapShapesAndAncestors(r, TopAbs_EDGE, TopAbs_FACE, ef);
  BRepClass3d_SolidClassifier cls(r);
  std::vector<TopoDS_Edge>    picked;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const TopTools_ListOfShape& fs = ef(i);
    if (fs.Extent() != 2)
      continue;
    const TopoDS_Edge& e = TopoDS::Edge(ef.FindKey(i));
    BRepAdaptor_Curve  c(e);
    gp_Pnt             p;
    gp_Vec             t;
    c.D1(0.5 * (c.FirstParameter() + c.LastParameter()), p, t);
    if (t.Magnitude() < 1e-12)
      continue;
    const double eps = 1e-3 * std::abs(d);
    gp_XYZ       side(0, 0, 0);
    bool         ok = true;
    for (const TopoDS_Shape& fsh : fs)
    {
      const occ::handle<Geom_Plane> g = occ::down_cast<Geom_Plane>(BRep_Tool::Surface(TopoDS::Face(fsh)));
      if (g.IsNull())
        return r;
      gp_Vec                   in = gp_Vec(g->Pln().Axis().Direction()).Crossed(t).Normalized();
      BRepClass_FaceClassifier fc(TopoDS::Face(fsh), p.Translated(in * eps), 1e-9);
      if (fc.State() != TopAbs_IN)
      {
        in.Reverse();
        BRepClass_FaceClassifier back(TopoDS::Face(fsh), p.Translated(in * eps), 1e-9);
        ok = ok && back.State() == TopAbs_IN;
      }
      side += in.XYZ();
    }
    if (!ok || side.Modulus() < 1e-9)
      continue;
    cls.Perform(p.Translated(gp_Vec(side * (0.5 * eps))), 1e-9);
    if ((cls.State() == TopAbs_IN) == (d > 0))
      picked.push_back(e);
  }
  if (picked.empty())
    return r;
  std::vector<gp_Pnt> all;
  for (const TopoDS_Edge& e : picked)
    all.push_back(edge_mid(e));
  const TopoDS_Shape once = fillet_at(r, all, std::abs(d));
  if (!once.IsNull())
    return once;
  // Connected runs: edges sharing a vertex.
  TopTools_IndexedMapOfShape verts;
  std::vector<int>           parent(picked.size());
  for (size_t i = 0; i < picked.size(); ++i)
    parent[i] = (int)i;
  std::function<int(int)> root = [&](int i) { return parent[i] == i ? i : parent[i] = root(parent[i]); };
  std::map<int, int>      owner;
  for (size_t i = 0; i < picked.size(); ++i)
    for (TopExp_Explorer v(picked[i], TopAbs_VERTEX); v.More(); v.Next())
    {
      const int k  = verts.Add(v.Current());
      auto      it = owner.find(k);
      if (it == owner.end())
        owner[k] = (int)i;
      else
        parent[root((int)i)] = root(it->second);
    }
  std::map<int, std::vector<gp_Pnt>> runs;
  for (size_t i = 0; i < picked.size(); ++i)
    runs[root((int)i)].push_back(all[i]);
  TopoDS_Shape out = r;
  for (const auto& [_, mids] : runs)
  {
    const TopoDS_Shape next = fillet_at(out, mids, std::abs(d));
    if (!next.IsNull())
      out = next;
  }
  return out;
}

// A mesh body offset by `d` as Onshape's Move face offsets its one face: the solid's true offset (convex edges
// rounded, live dl-bracket-1125: 1 mm grows the box 1 mm on every side, 25800.854 mm³; ours 25822.709, the holes'
// rims sharp), meshed back into a mesh body. OCCT's arc joins break on the facets: sharp joins, then fillets.
std::unique_ptr<OcShape> offset_mesh(const OcShape& s, double d)
{
  require_non_null(s.shape, "shape");
  MeshFacets   m;
  TopoDS_Shape f = facet_mesh(s.shape, m);
  ShapeUpgrade_UnifySameDomain u(f, true, true, false);
  u.Build();
  BRepOffsetAPI_MakeOffsetShape off;
  off.PerformByJoin(u.Shape(), d, 1e-7, BRepOffset_Skin, true, false, GeomAbs_Intersection);
  if (!off.IsDone())
    throw std::runtime_error("mesh offset failed");
  const TopoDS_Shape r = rounded(off.Shape(), d);
  BRepMesh_IncrementalMesh(r, std::abs(d) * 0.01, false, 0.1);
  std::vector<gp_Pnt>                     pts;
  std::vector<std::array<int, 3>>         tris;
  std::map<std::array<long long, 3>, int> index;
  auto node = [&](const gp_Pnt& p) {
    const std::array<long long, 3> k{std::llround(p.X() * 1e9), std::llround(p.Y() * 1e9), std::llround(p.Z() * 1e9)};
    auto it = index.find(k);
    if (it != index.end())
      return it->second;
    pts.push_back(p);
    return index[k] = (int)pts.size() - 1;
  };
  for (TopExp_Explorer e(r, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face&              face = TopoDS::Face(e.Current());
    TopLoc_Location                 l;
    occ::handle<Poly_Triangulation> t = BRep_Tool::Triangulation(face, l);
    if (t.IsNull())
      throw std::runtime_error("mesh offset: a face not meshed");
    const gp_Trsf at  = l.Transformation();
    const bool    rev = face.Orientation() == TopAbs_REVERSED;
    for (int k = 1; k <= t->NbTriangles(); ++k)
    {
      int a, b, c;
      t->Triangle(k).Get(a, b, c);
      if (rev)
        std::swap(b, c);
      const int ia = node(t->Node(a).Transformed(at)), ib = node(t->Node(b).Transformed(at)), ic = node(t->Node(c).Transformed(at));
      if (ia != ib && ib != ic && ic != ia)
        tris.push_back({ia, ib, ic});
    }
  }
  if (tris.empty())
    throw std::runtime_error("mesh offset: empty");
  const TopoDS_Shape c = mesh_compound(mesh_bodies(pts, std::move(tris), 1e-7));
  if (c.NbChildren() != 1)
    throw std::runtime_error("mesh offset: not one body");
  return wrap(TopoDS_Iterator(c).Value());
}

std::unique_ptr<OcShape> read_mesh(rust::Str path, uint32_t merge_limit)
{
  return wrap(shape_from_triangulation(read_triangulation(std::string(path.data(), path.size())), merge_limit));
}

} // namespace noshape
