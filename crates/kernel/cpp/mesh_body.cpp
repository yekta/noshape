// Mesh bodies, as Onshape imports STL, OBJ, PVZ, glTF and 3MF ("view and
// reference meshes only"): a body is one face per connected piece of the
// mesh, a face with a triangulation and no surface, edges or vertices
// (Onshape's mesh part: 1 face of type OTHER, 0 edges). Closed pieces make a
// solid (cavities are further shells), open ones are surface bodies.
// OCCT meshes, measures (volume, area, bounds) and writes such faces from
// their triangulation; validity and transforms are handled here.

#include "internal.hpp"

#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Poly_Triangulation.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Iterator.hxx>

#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp_GTrsf.hxx>

#include <algorithm>
#include <functional>
#include <map>
#include <numeric>

namespace noshape {

bool is_mesh_face(const TopoDS_Face& f)
{
  TopLoc_Location l;
  return BRep_Tool::Surface(f, l).IsNull() && !BRep_Tool::Triangulation(f, l).IsNull();
}

bool has_mesh_faces(const TopoDS_Shape& s)
{
  for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
    if (is_mesh_face(TopoDS::Face(e.Current())))
      return true;
  return false;
}

namespace {

TopoDS_Face mesh_face(const std::vector<gp_Pnt>& pts, const std::vector<std::array<int, 3>>& tris)
{
  // Only the nodes the triangles use, renumbered.
  std::vector<int> used(pts.size(), 0);
  for (const auto& t : tris)
    for (int i : t)
      used[i] = 1;
  std::vector<int> index(pts.size(), 0);
  int              n = 0;
  for (size_t i = 0; i < pts.size(); ++i)
    if (used[i])
      index[i] = ++n;
  occ::handle<Poly_Triangulation> tri = new Poly_Triangulation(n, (int)tris.size(), false);
  for (size_t i = 0; i < pts.size(); ++i)
    if (used[i])
      tri->SetNode(index[i], pts[i]);
  for (size_t k = 0; k < tris.size(); ++k)
    tri->SetTriangle((int)k + 1, Poly_Triangle(index[tris[k][0]], index[tris[k][1]], index[tris[k][2]]));
  BRep_Builder b;
  TopoDS_Face  f;
  b.MakeFace(f, tri);
  return f;
}

// The face's triangulation with its location applied, nodes mapped by `map`
// (triangles reversed when `flip`), as a new mesh face.
TopoDS_Face mapped_face(const TopoDS_Face& f, const std::function<gp_Pnt(const gp_Pnt&)>& map, bool flip)
{
  TopLoc_Location                        l;
  const occ::handle<Poly_Triangulation>& t = BRep_Tool::Triangulation(f, l);
  const gp_Trsf                          lt = l.Transformation();
  std::vector<gp_Pnt>                    pts;
  for (int i = 1; i <= t->NbNodes(); ++i)
    pts.push_back(map(t->Node(i).Transformed(lt)));
  // A reversed face's triangles are stored clockwise from outside: bake the
  // orientation into the new face (FORWARD).
  const bool reverse = flip != (f.Orientation() == TopAbs_REVERSED);
  std::vector<std::array<int, 3>> tris;
  for (int k = 1; k <= t->NbTriangles(); ++k)
  {
    int a, b, c;
    t->Triangle(k).Get(a, b, c);
    tris.push_back(reverse ? std::array<int, 3>{a - 1, c - 1, b - 1} : std::array<int, 3>{a - 1, b - 1, c - 1});
  }
  return mesh_face(pts, tris);
}

// `s` rebuilt with each mesh face mapped (B-rep faces through `brep`); the
// replaced faces recorded in `h`.
TopoDS_Shape rebuild(const TopoDS_Shape& s, const std::function<gp_Pnt(const gp_Pnt&)>& map, bool flip,
                     const std::function<TopoDS_Shape(const TopoDS_Shape&)>& brep, BRepTools_History& h)
{
  if (s.ShapeType() == TopAbs_FACE)
  {
    const TopoDS_Face& f = TopoDS::Face(s);
    TopoDS_Shape       r = is_mesh_face(f) ? TopoDS_Shape(mapped_face(f, map, flip)) : brep(f);
    h.AddModified(s, r);
    return r;
  }
  if (s.ShapeType() > TopAbs_FACE)
    return brep(s);
  BRep_Builder b;
  TopoDS_Shape r = s.EmptyCopied();
  r.Orientation(TopAbs_FORWARD);
  for (TopoDS_Iterator it(s, false, false); it.More(); it.Next())
  {
    TopoDS_Shape c = rebuild(it.Value().Located(s.Location() * it.Value().Location()), map, flip, brep, h);
    if (it.Value().Orientation() == TopAbs_REVERSED && c.ShapeType() != TopAbs_FACE)
      c.Reverse();
    b.Add(r, c);
  }
  r.Location(TopLoc_Location());
  if (s.ShapeType() == TopAbs_SOLID || s.ShapeType() == TopAbs_SHELL)
    r.Closed(s.Closed());
  if (s.ShapeType() != TopAbs_FACE && s.ShapeType() != TopAbs_COMPOUND)
    h.AddModified(s, r);
  return r;
}

} // namespace

TopoDS_Shape transform_mesh(const TopoDS_Shape& s, const gp_Trsf& t, BRepTools_History& h)
{
  return rebuild(
    s, [&t](const gp_Pnt& p) { return p.Transformed(t); }, t.IsNegative(),
    [&t](const TopoDS_Shape& x) { return BRepBuilderAPI_Transform(x, t, true).Shape(); }, h);
}

TopoDS_Shape gtransform_mesh(const TopoDS_Shape& s, const gp_GTrsf& g, BRepTools_History& h)
{
  const bool flip = g.VectorialPart().Determinant() < 0;
  return rebuild(
    s,
    [&g](const gp_Pnt& p) {
      gp_XYZ x = p.XYZ();
      g.Transforms(x);
      return gp_Pnt(x);
    },
    flip, [&g](const TopoDS_Shape& x) { return BRepBuilderAPI_GTransform(x, g, true).Shape(); }, h);
}

bool mesh_valid(const TopoDS_Shape& s)
{
  // Mesh faces: a triangulation with triangles; anything else: BRepCheck.
  bool brep = false;
  for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    if (!is_mesh_face(f))
    {
      brep = true;
      continue;
    }
    TopLoc_Location l;
    if (BRep_Tool::Triangulation(f, l)->NbTriangles() == 0)
      return false;
  }
  if (!brep)
    return true;
  for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
    if (!is_mesh_face(TopoDS::Face(e.Current())) && !BRepCheck_Analyzer(e.Current()).IsValid())
      return false;
  return true;
}

std::vector<MeshPiece> mesh_bodies(const std::vector<gp_Pnt>& nodes, std::vector<std::array<int, 3>> tris, double weld)
{
  std::vector<gp_Pnt> pts;
  mesh_weld(nodes, tris, weld, pts);
  // Drop zero-area triangles.
  tris.erase(std::remove_if(tris.begin(), tris.end(),
                            [&pts](const std::array<int, 3>& t) {
                              return gp_Vec(pts[t[0]], pts[t[1]]).Crossed(gp_Vec(pts[t[0]], pts[t[2]])).SquareMagnitude() <= 0;
                            }),
             tris.end());
  // Connected pieces (triangles sharing an edge).
  std::vector<int> parent(tris.size());
  std::iota(parent.begin(), parent.end(), 0);
  std::function<int(int)> find = [&](int x) { return parent[x] == x ? x : parent[x] = find(parent[x]); };
  std::map<std::pair<int, int>, std::vector<int>> by_edge; // undirected edge -> triangles
  std::map<std::pair<int, int>, int>              half;    // directed edge -> count
  for (size_t k = 0; k < tris.size(); ++k)
    for (int j = 0; j < 3; ++j)
    {
      const int a = tris[k][j], c = tris[k][(j + 1) % 3];
      by_edge[{std::min(a, c), std::max(a, c)}].push_back((int)k);
      ++half[{a, c}];
    }
  for (const auto& [e, ts] : by_edge)
    for (size_t i = 1; i < ts.size(); ++i)
      parent[find(ts[i])] = find(ts[0]);
  std::map<int, std::vector<std::array<int, 3>>> pieces;
  for (size_t k = 0; k < tris.size(); ++k)
    pieces[find((int)k)].push_back(tris[k]);
  std::vector<MeshPiece> out;
  for (const auto& [root, ts] : pieces)
  {
    // Closed: every edge used as often one way as the other (Onshape's cone
    // import has edges at the apex shared by four slivers, two each way).
    bool closed = true;
    for (const auto& t : ts)
      for (int j = 0; j < 3 && closed; ++j)
      {
        const int  a = t[j], c = t[(j + 1) % 3];
        const auto f = half.find({a, c}), r = half.find({c, a});
        closed = f != half.end() && r != half.end() && f->second == r->second;
      }
    out.push_back({mesh_face(pts, ts), closed});
  }
  return out;
}

TopoDS_Shape mesh_compound(const std::vector<MeshPiece>& pieces)
{
  BRep_Builder    b;
  TopoDS_Compound c;
  b.MakeCompound(c);
  TopoDS_Solid solid;
  bool         any_closed = false;
  for (const MeshPiece& p : pieces)
  {
    TopoDS_Shell sh;
    b.MakeShell(sh);
    b.Add(sh, p.face);
    sh.Closed(p.closed);
    if (!p.closed)
    {
      b.Add(c, sh);
      continue;
    }
    if (!any_closed)
      b.MakeSolid(solid);
    any_closed = true;
    b.Add(solid, sh);
  }
  if (any_closed)
    b.Add(c, solid);
  return c;
}

} // namespace noshape
