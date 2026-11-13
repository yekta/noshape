// Mesh bodies, as Onshape imports STL, OBJ, PVZ, glTF and 3MF ("view and
// reference meshes only"): a body is one face per connected piece of the
// mesh, a face with a triangulation and no surface, edges or vertices
// (Onshape's mesh part: 1 face of type OTHER, 0 edges). Closed pieces make a
// solid (cavities are further shells), open ones are surface bodies.
// OCCT meshes, measures (volume, area, bounds) and writes such faces from
// their triangulation; validity and transforms are handled here.

#include "internal.hpp"

#include <BRepBuilderAPI_GTransform.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_Plane.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepLib.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <TopLoc_Location.hxx>
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
  for (size_t k = 0; k < tris.size(); ++k)
    for (int j = 0; j < 3; ++j)
    {
      const int a = tris[k][j], c = tris[k][(j + 1) % 3];
      by_edge[{std::min(a, c), std::max(a, c)}].push_back((int)k);
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
    // Closed: every edge used an even number of times (Onshape's cone import has edges at the apex shared by four
    // slivers, two each way; a mesher's fold covers a few edges twice the same way, and live still reads a solid:
    // exchange suite, cf-sheet-metal-model-plus' STL).
    bool closed = true;
    for (const auto& t : ts)
      for (int j = 0; j < 3 && closed; ++j)
      {
        const int a = t[j], c = t[(j + 1) % 3];
        closed      = by_edge.at({std::min(a, c), std::max(a, c)}).size() % 2 == 0;
      }
    out.push_back({mesh_face(pts, ts), closed});
  }
  return out;
}

namespace {
// A closed piece's triangles (its face's triangulation, located) and box.
struct Closed
{
  std::vector<std::array<gp_Pnt, 3>> tris;
  Bnd_Box                            box;
};

Closed closed_of(const TopoDS_Face& f)
{
  Closed          c;
  TopLoc_Location loc;
  const auto      tri = BRep_Tool::Triangulation(f, loc);
  if (tri.IsNull())
    return c;
  const gp_Trsf t = loc.Transformation();
  for (int k = 1; k <= tri->NbTriangles(); ++k)
  {
    int i1, i2, i3;
    tri->Triangle(k).Get(i1, i2, i3);
    c.tris.push_back({tri->Node(i1).Transformed(t), tri->Node(i2).Transformed(t), tri->Node(i3).Transformed(t)});
    for (const gp_Pnt& p : c.tris.back())
      c.box.Add(p);
  }
  return c;
}

// Whether `p` is inside the closed piece `c`: a ray along +x crosses its triangles an odd number of times (the
// triangles' y-z projections, edges half-open so a crossing on an edge counts once).
bool inside(const Closed& c, const gp_Pnt& p)
{
  if (c.box.IsOut(p))
    return false;
  bool in = false;
  for (const auto& t : c.tris)
  {
    const double y0 = t[0].Y() - p.Y(), z0 = t[0].Z() - p.Z(), y1 = t[1].Y() - p.Y(), z1 = t[1].Z() - p.Z(), y2 = t[2].Y() - p.Y(), z2 = t[2].Z() - p.Z();
    // The signed areas of the origin with each edge: all one sign when the ray goes through.
    const double a = y0 * z1 - z0 * y1, b = y1 * z2 - z1 * y2, d = y2 * z0 - z2 * y0;
    const bool   pos = a > 0 || (a == 0 && (y1 - y0 > 0 || (y1 == y0 && z1 - z0 > 0)));
    const bool   pos_b = b > 0 || (b == 0 && (y2 - y1 > 0 || (y2 == y1 && z2 - z1 > 0)));
    const bool   pos_d = d > 0 || (d == 0 && (y0 - y2 > 0 || (y0 == y2 && z0 - z2 > 0)));
    if (pos != pos_b || pos != pos_d)
      continue;
    const double area = a + b + d;
    if (area == 0)
      continue;
    // Where the ray meets the triangle's plane, by the barycentric weights.
    const double x = (b * t[0].X() + d * t[1].X() + a * t[2].X()) / area;
    if (x > p.X())
      in = !in;
  }
  return in;
}
} // namespace

TopoDS_Shape mesh_compound(const std::vector<MeshPiece>& pieces)
{
  BRep_Builder    b;
  TopoDS_Compound c;
  b.MakeCompound(c);
  // Closed pieces: a solid each, with the pieces inside it as its cavities (Onshape's import: one part per connected
  // mesh, a mesh inside another the other's hollow; live exchange suite 2147).
  std::vector<size_t> closed;
  for (size_t i = 0; i < pieces.size(); ++i)
  {
    if (pieces[i].closed)
    {
      closed.push_back(i);
      continue;
    }
    TopoDS_Shell sh;
    b.MakeShell(sh);
    b.Add(sh, pieces[i].face);
    sh.Closed(false);
    b.Add(c, sh);
  }
  std::vector<Closed> geo;
  for (size_t i : closed)
    geo.push_back(closed_of(pieces[i].face));
  // Each piece's containers, the innermost the one contained by most: those that hold the centres of up to 64 of its
  // triangles spread over it (a piece through another's surface is a part of its own: cf-spur-gear's construction pin
  // half in its gear: live reads 5 parts).
  const size_t                     n = closed.size();
  std::vector<std::vector<size_t>> in(n);
  for (size_t i = 0; i < n; ++i)
  {
    const auto&  ts     = geo[i].tris;
    const size_t stride = std::max<size_t>(1, ts.size() / 64);
    for (size_t j = 0; j < n; ++j)
    {
      bool all = j != i && !ts.empty() && !geo[j].box.IsOut(geo[i].box);
      for (size_t k = 0; k < ts.size() && all; k += stride)
        all = inside(geo[j], gp_Pnt((ts[k][0].XYZ() + ts[k][1].XYZ() + ts[k][2].XYZ()) / 3));
      if (all)
        in[i].push_back(j);
    }
  }
  std::vector<int> solid_of(n, -1);
  std::vector<TopoDS_Solid> solids;
  auto shell = [&](size_t i) {
    TopoDS_Shell sh;
    b.MakeShell(sh);
    b.Add(sh, pieces[closed[i]].face);
    sh.Closed(true);
    return sh;
  };
  // Outer pieces (inside an even number of others) start a solid; one inside an odd number is a cavity of its
  // innermost container.
  for (size_t i = 0; i < n; ++i)
    if (in[i].size() % 2 == 0)
    {
      solid_of[i] = (int)solids.size();
      solids.emplace_back();
      b.MakeSolid(solids.back());
      b.Add(solids.back(), shell(i));
    }
  for (size_t i = 0; i < n; ++i)
  {
    if (in[i].size() % 2 == 0)
      continue;
    size_t outer = in[i].front();
    for (size_t j : in[i])
      if (in[j].size() > in[outer].size())
        outer = j;
    if (solid_of[outer] < 0)
      continue;
    // A cavity faces inward (an STL's pieces each face out).
    double vol = 0;
    for (const auto& t : geo[i].tris)
      vol += t[0].XYZ().Dot(t[1].XYZ().Crossed(t[2].XYZ()));
    const TopoDS_Shell sh = shell(i);
    b.Add(solids[solid_of[outer]], vol > 0 ? TopoDS::Shell(sh.Reversed()) : sh);
  }
  for (const TopoDS_Solid& s : solids)
    b.Add(c, s);
  return c;
}


// ---- booleans and splits with mesh bodies
//
// Onshape cuts and joins mesh parts (live G35/G35-retry: Split, Extrude Add
// and Boolean union give MIXED parts: the mesh face plus new planar B-rep
// faces). OCCT's booleans need surfaces, so for the operation each mesh face
// becomes planar triangle faces ("facets") sharing their edges, and the
// facets of the result become one mesh face again.

namespace
{

// A facet's plane, through the trimmed wrapper booleans may add.
const Geom_Surface* plane_of(const TopoDS_Face& f)
{
  TopLoc_Location           l;
  occ::handle<Geom_Surface> s = BRep_Tool::Surface(f, l);
  while (auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
    s = t->BasisSurface();
  return s.get();
}

// The faces of `owner` with its mesh faces as facets: a closed shell (a
// solid when `solid`) or a shell sewn with its B-rep faces.
TopoDS_Shape faceted(const TopoDS_Shape& owner, bool solid, MeshFacets& m)
{
  BRep_Builder                                     B;
  std::map<std::array<long long, 3>, TopoDS_Vertex> verts;
  std::map<std::pair<const void*, const void*>, TopoDS_Edge> edges;
  auto vertex = [&](const gp_Pnt& p) {
    const std::array<long long, 3> k{std::llround(p.X() * 1e9), std::llround(p.Y() * 1e9), std::llround(p.Z() * 1e9)};
    auto it = verts.find(k);
    if (it != verts.end())
      return it->second;
    TopoDS_Vertex v;
    B.MakeVertex(v, p, 1e-7);
    return verts[k] = v;
  };
  auto edge = [&](const TopoDS_Vertex& u, const TopoDS_Vertex& v) {
    const void* pu = u.TShape().get();
    const void* pv = v.TShape().get();
    const auto  k  = pu < pv ? std::make_pair(pu, pv) : std::make_pair(pv, pu);
    auto        it = edges.find(k);
    if (it == edges.end())
      it = edges.emplace(k, BRepBuilderAPI_MakeEdge(pu < pv ? u : v, pu < pv ? v : u).Edge()).first;
    return pu < pv ? it->second : TopoDS::Edge(it->second.Reversed());
  };
  std::vector<std::pair<TopoDS_Face, TopoDS_Face>> faces; // (source, face)
  bool                                             brep = false;
  for (TopExp_Explorer e(owner, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& F = TopoDS::Face(e.Current());
    if (!is_mesh_face(F))
    {
      faces.emplace_back(F, F);
      brep = true;
      continue;
    }
    TopLoc_Location                       l;
    const occ::handle<Poly_Triangulation> tri = BRep_Tool::Triangulation(F, l);
    const gp_Trsf                         t   = l.Transformation();
    const bool                            rev = F.Orientation() == TopAbs_REVERSED;
    std::vector<TopoDS_Vertex>            vs((size_t)tri->NbNodes() + 1);
    for (int i = 1; i <= tri->NbNodes(); ++i)
      vs[(size_t)i] = vertex(tri->Node(i).Transformed(t));
    for (int k = 1; k <= tri->NbTriangles(); ++k)
    {
      int a, b, c;
      tri->Triangle(k).Get(a, b, c);
      if (rev)
        std::swap(b, c);
      const TopoDS_Vertex &va = vs[(size_t)a], &vb = vs[(size_t)b], &vc = vs[(size_t)c];
      if (va.IsSame(vb) || vb.IsSame(vc) || vc.IsSame(va))
        continue;
      const gp_Pnt pa = BRep_Tool::Pnt(va), pb = BRep_Tool::Pnt(vb), pc = BRep_Tool::Pnt(vc);
      const gp_Vec n  = gp_Vec(pa, pb).Crossed(gp_Vec(pa, pc));
      if (n.Magnitude() < 1e-24)
        continue;
      BRepBuilderAPI_MakeWire mw;
      mw.Add(edge(va, vb));
      mw.Add(edge(vb, vc));
      mw.Add(edge(vc, va));
      if (!mw.IsDone())
        continue;
      const occ::handle<Geom_Plane> plane = new Geom_Plane(pa, gp_Dir(n));
      TopoDS_Face                   f;
      B.MakeFace(f, plane, 1e-7);
      B.Add(f, mw.Wire());
      m.source[plane.get()] = F;
      faces.emplace_back(F, f);
    }
  }
  std::vector<TopoDS_Shape> shells;
  if (!brep)
  {
    TopoDS_Shell sh;
    B.MakeShell(sh);
    for (const auto& [src, f] : faces)
    {
      B.Add(sh, f);
      m.to_facets->AddModified(src, f);
    }
    sh.Closed(BRep_Tool::IsClosed(sh));
    shells.push_back(sh);
  }
  else
  {
    // A part already MIXED: its B-rep faces sewn to the facets.
    BRepBuilderAPI_Sewing sew(1e-6);
    for (const auto& [src, f] : faces)
      sew.Add(f);
    sew.Perform();
    for (const auto& [src, f] : faces)
    {
      const TopoDS_Shape g = sew.IsModified(f) ? sew.Modified(f) : f;
      if (!src.IsSame(g))
        m.to_facets->AddModified(src, g);
    }
    for (TopExp_Explorer e(sew.SewedShape(), TopAbs_SHELL); e.More(); e.Next())
      shells.push_back(e.Current());
  }
  if (!solid)
  {
    if (shells.size() == 1)
      return shells.front();
    TopoDS_Compound c;
    B.MakeCompound(c);
    for (const auto& sh : shells)
      B.Add(c, sh);
    return c;
  }
  TopoDS_Solid so;
  B.MakeSolid(so);
  for (const auto& sh : shells)
    B.Add(so, sh);
  BRepLib::OrientClosedSolid(so);
  return so;
}

} // namespace

TopoDS_Shape facet_mesh(const TopoDS_Shape& s, MeshFacets& m)
{
  if (!has_mesh_faces(s))
    return s;
  BRep_Builder    B;
  TopoDS_Compound c;
  B.MakeCompound(c);
  int n = 0;
  for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next(), ++n)
    B.Add(c, faceted(e.Current(), true, m));
  for (TopExp_Explorer e(s, TopAbs_SHELL, TopAbs_SOLID); e.More(); e.Next(), ++n)
    B.Add(c, faceted(e.Current(), false, m));
  for (TopExp_Explorer e(s, TopAbs_FACE, TopAbs_SHELL); e.More(); e.Next(), ++n)
    B.Add(c, faceted(e.Current(), false, m));
  if (n == 1)
  {
    TopoDS_Iterator it(c);
    return it.Value();
  }
  return c;
}

TopoDS_Shape unfacet_mesh(const TopoDS_Shape& s, const MeshFacets& m, occ::handle<BRepTools_History>& h)
{
  const occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  bool                                 any = false;
  // Each shell's facets (from any input) become one mesh face.
  for (TopExp_Explorer es(s, TopAbs_SHELL); es.More(); es.Next())
  {
    std::vector<TopoDS_Face> facets;
    for (TopExp_Explorer ef(es.Current(), TopAbs_FACE); ef.More(); ef.Next())
      if (m.source.count(plane_of(TopoDS::Face(ef.Current()))))
        facets.push_back(TopoDS::Face(ef.Current()));
    if (facets.empty())
      continue;
    std::vector<gp_Pnt>                       pts;
    std::vector<std::array<int, 3>>           tris;
    std::map<std::array<long long, 3>, int> index;
    auto node = [&](const gp_Pnt& p) {
      const std::array<long long, 3> k{std::llround(p.X() * 1e9), std::llround(p.Y() * 1e9), std::llround(p.Z() * 1e9)};
      auto it = index.find(k);
      if (it != index.end())
        return it->second;
      pts.push_back(p);
      return index[k] = (int)pts.size() - 1;
    };
    for (const TopoDS_Face& f : facets)
    {
      TopLoc_Location                 l;
      occ::handle<Poly_Triangulation> tri = BRep_Tool::Triangulation(f, l);
      if (tri.IsNull())
      {
        BRepMesh_IncrementalMesh(f, 1e-7, false, 0.5);
        tri = BRep_Tool::Triangulation(f, l);
      }
      if (tri.IsNull())
        continue;
      const gp_Trsf t   = l.Transformation();
      const bool    rev = f.Orientation() == TopAbs_REVERSED;
      for (int k = 1; k <= tri->NbTriangles(); ++k)
      {
        int a, b, c;
        tri->Triangle(k).Get(a, b, c);
        if (rev)
          std::swap(b, c);
        const int ia = node(tri->Node(a).Transformed(t)), ib = node(tri->Node(b).Transformed(t)), ic = node(tri->Node(c).Transformed(t));
        if (ia != ib && ib != ic && ic != ia)
          tris.push_back({ia, ib, ic});
      }
    }
    if (tris.empty())
      continue;
    rs->Replace(facets.front(), mesh_face(pts, tris));
    for (size_t i = 1; i < facets.size(); ++i)
      rs->Remove(facets[i]);
    any = true;
  }
  if (!any)
    return s;
  const TopoDS_Shape out = rs->Apply(s);
  h                      = rs->History();
  return out;
}

} // namespace noshape
