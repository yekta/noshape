// Mesh import (STL, OBJ): triangles become planar faces of one connected
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

#include <map>
#include <unordered_map>
#include <vector>

namespace noshape {

namespace {

TopoDS_Shape shape_from_triangulation(const occ::handle<Poly_Triangulation>& tri, uint32_t merge_limit)
{
  const int nn = tri->NbNodes();
  const int nt = tri->NbTriangles();
  // Merge coincident nodes (STL repeats them per triangle).
  double scale = 0.0;
  for (int i = 1; i <= nn; ++i)
  {
    const gp_Pnt p = tri->Node(i);
    scale = std::max({scale, std::abs(p.X()), std::abs(p.Y()), std::abs(p.Z())});
  }
  const double tol = std::max(scale, 1.0) * 1e-9;
  const double q = std::max(scale, 1e-9) * 1e-7; // quantization step
  std::map<std::array<long long, 3>, int> by_pos;
  std::vector<int> remap(nn + 1, 0);
  std::vector<TopoDS_Vertex> verts;
  BRep_Builder b;
  for (int i = 1; i <= nn; ++i)
  {
    const gp_Pnt p = tri->Node(i);
    const std::array<long long, 3> key = {std::llround(p.X() / q), std::llround(p.Y() / q), std::llround(p.Z() / q)};
    auto it = by_pos.find(key);
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
  for (int t = 1; t <= nt; ++t)
  {
    int i1, i2, i3;
    tri->Triangle(t).Get(i1, i2, i3);
    const int a = remap[i1], c = remap[i2], d = remap[i3];
    if (a == c || c == d || a == d)
      continue; // degenerate
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

} // namespace

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
