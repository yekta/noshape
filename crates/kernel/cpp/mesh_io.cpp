// Mesh import (STL, OBJ; PVZ meshes through mesh_shape): triangles become planar faces of one connected
// shell (shared vertices and edges, no sewing pass), closed shells become
// solids, and coplanar neighbours are merged back into larger faces.
#include "internal.hpp"

#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRep_Builder.hxx>
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

// Shell (solid when closed) of planar triangle faces. `weld` > 0: nodes
// closer than that are one vertex and T-junctions are split; 0: nodes on the
// same 1e-7-of-the-size grid point are one vertex (STL / OBJ).
TopoDS_Shape shape_from_triangles(const std::vector<gp_Pnt>& nodes, std::vector<std::array<int, 3>> tris, uint32_t merge_limit, double weld)
{
  const int nn = (int)nodes.size();
  // Merge coincident nodes (STL repeats them per triangle).
  double scale = 0.0;
  for (const gp_Pnt& p : nodes)
    scale = std::max({scale, std::abs(p.X()), std::abs(p.Y()), std::abs(p.Z())});
  const double tol = weld > 0 ? weld : std::max(scale, 1.0) * 1e-9;
  const double q = weld > 0 ? weld : std::max(scale, 1e-9) * 1e-7; // quantization step
  std::map<std::array<long long, 3>, int> by_pos;
  std::vector<int> remap(nn, 0);
  std::vector<TopoDS_Vertex> verts;
  std::vector<gp_Pnt>        vpts;
  BRep_Builder b;
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
      if (j != by_pos.end() && vpts[j->second].Distance(p) <= weld)
        it = j;
    }
    if (it != by_pos.end())
    {
      remap[i] = it->second;
      continue;
    }
    TopoDS_Vertex v;
    b.MakeVertex(v, p, tol);
    remap[i] = (int)verts.size();
    by_pos.emplace(key, remap[i]);
    verts.push_back(v);
    vpts.push_back(p);
  }
  for (auto& t : tris)
    for (int& i : t)
      i = remap[i];
  tris.erase(std::remove_if(tris.begin(), tris.end(), [](const std::array<int, 3>& t) { return t[0] == t[1] || t[1] == t[2] || t[0] == t[2]; }), tris.end());
  if (weld > 0)
    split_t_junctions(vpts, tris, weld);
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

std::unique_ptr<OcShape> read_mesh(rust::Str path, uint32_t merge_limit)
{
  const std::string p(path.data(), path.size());
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
  return wrap(shape_from_triangulation(tri, merge_limit));
}

} // namespace noshape
