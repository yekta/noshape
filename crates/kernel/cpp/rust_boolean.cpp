// noshape's own Boolean (crates/brep, docs/BOOLEAN-SPEEDUP.md phase 3): the arguments as flat bodies, the
// result and its history back as shapes. op_boolean runs it first with NOSHAPE_BOOL_RUST=1; it declines
// anything but solids bounded by planes and straight edges, and OCCT runs as before.
#include "internal.hpp"

#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepTools_History.hxx>
#include <ElCLib.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <TopExp.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Wire.hxx>


namespace noshape {

bool rust_boolean_on() { return brep_enabled(); }

namespace {

// Declines (the replay's strict mode: fails).
bool decline(const char* why)
{
  if (why != nullptr)
    brep_decline(why);
  if (brep_strict())
    fail_unsupported("noshape's Boolean declined");
  return false;
}

// Inputs with a larger tolerance are tolerant shapes: OCCT's.
constexpr double kMaxTol = 1e-5;

occ::handle<Geom_Line> line_of(const occ::handle<Geom_Curve>& c)
{
  occ::handle<Geom_Curve> b = c;
  while (true)
  {
    occ::handle<Geom_TrimmedCurve> t = occ::down_cast<Geom_TrimmedCurve>(b);
    if (t.IsNull())
      break;
    b = t->BasisCurve();
  }
  return occ::down_cast<Geom_Line>(b);
}

// `s` as a flat body (its maps' indices are the body's), or why not.
const char* to_body(const OcShape& s, BrepBody& b, double& max_tol)
{
  const ShapeIMap& fm = s.map(TopAbs_FACE);
  const ShapeIMap& em = s.map(TopAbs_EDGE);
  const ShapeIMap& vm = s.map(TopAbs_VERTEX);
  const ShapeIMap& sm = s.map(TopAbs_SOLID);
  if (sm.Extent() == 0 || fm.Extent() == 0)
    return "no solid";
  b.vertices.reserve(3 * vm.Extent());
  for (int i = 1; i <= vm.Extent(); ++i)
  {
    const TopoDS_Vertex& v = TopoDS::Vertex(vm(i));
    const gp_Pnt         p = BRep_Tool::Pnt(v);
    max_tol                = std::max(max_tol, BRep_Tool::Tolerance(v));
    b.vertices.push_back(p.X());
    b.vertices.push_back(p.Y());
    b.vertices.push_back(p.Z());
  }
  b.edges.reserve(2 * em.Extent());
  for (int i = 1; i <= em.Extent(); ++i)
  {
    const TopoDS_Edge e = TopoDS::Edge(em(i).Oriented(TopAbs_FORWARD));
    if (BRep_Tool::Degenerated(e))
      return "a degenerate edge";
    TopLoc_Location               loc;
    double                        f = 0, l = 0;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, loc, f, l);
    if (c.IsNull() || line_of(c).IsNull())
      return "a curved edge";
    TopoDS_Vertex v1, v2;
    TopExp::Vertices(e, v1, v2);
    if (v1.IsNull() || v2.IsNull() || v1.IsSame(v2))
      return "a closed edge";
    max_tol = std::max(max_tol, BRep_Tool::Tolerance(e));
    b.edges.push_back(static_cast<uint32_t>(vm.FindIndex(v1) - 1));
    b.edges.push_back(static_cast<uint32_t>(vm.FindIndex(v2) - 1));
  }
  b.face_loops.push_back(0);
  b.loop_uses.push_back(0);
  std::vector<int> seen(em.Extent() + 1, 0);
  for (int i = 1; i <= fm.Extent(); ++i)
  {
    const TopoDS_Face&      f = TopoDS::Face(fm(i));
    TopLoc_Location         loc;
    occ::handle<Geom_Plane> pl = occ::down_cast<Geom_Plane>(BRep_Tool::Surface(f, loc));
    if (pl.IsNull())
      return "a curved face";
    if (f.Orientation() != TopAbs_FORWARD && f.Orientation() != TopAbs_REVERSED)
      return "an internal face";
    max_tol            = std::max(max_tol, BRep_Tool::Tolerance(f));
    const gp_Pln   p   = pl->Pln().Transformed(loc.Transformation());
    const gp_Pnt   o   = p.Location();
    const gp_Dir   n   = p.Axis().Direction();
    for (double x : {o.X(), o.Y(), o.Z(), n.X(), n.Y(), n.Z()})
      b.planes.push_back(x);
    b.reversed.push_back(f.Orientation() == TopAbs_REVERSED);
    for (TopoDS_Iterator wi(f); wi.More(); wi.Next())
    {
      if (wi.Value().ShapeType() != TopAbs_WIRE)
        return "a vertex in a face";
      for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
      {
        const TopoDS_Shape& e = ei.Value();
        const int           k = em.FindIndex(e);
        if (k <= 0 || (e.Orientation() != TopAbs_FORWARD && e.Orientation() != TopAbs_REVERSED) || seen[k] == i)
          return "a seam or an internal edge";
        seen[k] = i;
        b.uses.push_back(static_cast<uint32_t>(2 * (k - 1) + (e.Orientation() == TopAbs_REVERSED ? 1 : 0)));
      }
      b.loop_uses.push_back(static_cast<uint32_t>(b.uses.size()));
    }
    b.face_loops.push_back(static_cast<uint32_t>(b.loop_uses.size() - 1));
  }
  b.solid_faces.push_back(0);
  for (int i = 1; i <= sm.Extent(); ++i)
  {
    int shells = 0;
    for (TopoDS_Iterator si(sm(i)); si.More(); si.Next())
    {
      if (si.Value().ShapeType() != TopAbs_SHELL || ++shells > 1)
        return "a solid with voids";
      for (TopoDS_Iterator fi(si.Value()); fi.More(); fi.Next())
      {
        const int k = fm.FindIndex(fi.Value());
        if (k <= 0)
          return "a shell's face not mapped";
        b.faces_of_solids.push_back(static_cast<uint32_t>(k - 1));
      }
    }
    b.solid_faces.push_back(static_cast<uint32_t>(b.faces_of_solids.size()));
  }
  if (b.faces_of_solids.size() != static_cast<size_t>(fm.Extent()))
    return "faces outside the solids";
  return nullptr;
}

gp_Pnt point_of(const BrepBody& b, uint32_t v)
{
  return gp_Pnt(b.vertices[3 * v], b.vertices[3 * v + 1], b.vertices[3 * v + 2]);
}

const TopoDS_Shape& input(const std::vector<const OcShape*>& srcs, uint32_t arg, TopAbs_ShapeEnum kind, uint32_t index)
{
  return srcs[arg]->map(kind)(static_cast<int>(index) + 1);
}

TopoDS_Shape build(const BrepResult& r, const std::vector<const OcShape*>& srcs, double tol, occ::handle<BRepTools_History>& hist)
{
  const BrepBody& b = r.body;
  BRep_Builder    bb;
  const size_t    nv = b.vertices.size() / 3, ne = b.edges.size() / 2, nf = b.reversed.size();
  std::vector<TopoDS_Vertex> vs(nv);
  for (size_t i = 0; i < nv; ++i)
  {
    if (r.vertex_origin[3 * i] == 0)
      vs[i] = TopoDS::Vertex(input(srcs, r.vertex_origin[3 * i + 1], TopAbs_VERTEX, r.vertex_origin[3 * i + 2]).Oriented(TopAbs_FORWARD));
    else
      bb.MakeVertex(vs[i], point_of(b, static_cast<uint32_t>(i)), tol);
  }
  std::vector<TopoDS_Edge> es(ne);
  for (size_t i = 0; i < ne; ++i)
  {
    const uint32_t kind = r.edge_origin[3 * i];
    if (kind == 0)
    {
      es[i] = TopoDS::Edge(input(srcs, r.edge_origin[3 * i + 1], TopAbs_EDGE, r.edge_origin[3 * i + 2]).Oriented(TopAbs_FORWARD));
      continue;
    }
    const uint32_t v0 = b.edges[2 * i], v1 = b.edges[2 * i + 1];
    const gp_Pnt   p0 = point_of(b, v0), p1 = point_of(b, v1);
    TopoDS_Edge    e;
    double         t0 = 0, t1 = p0.Distance(p1);
    if (kind == 1)
    {
      const TopoDS_Edge src = TopoDS::Edge(input(srcs, r.edge_origin[3 * i + 1], TopAbs_EDGE, r.edge_origin[3 * i + 2]));
      TopLoc_Location               loc;
      double                        f = 0, l = 0;
      const occ::handle<Geom_Curve> c   = BRep_Tool::Curve(src, loc, f, l);
      const gp_Lin                  lin = line_of(c)->Lin().Transformed(loc.Transformation());
      t0                                = ElCLib::Parameter(lin, p0);
      t1                                = ElCLib::Parameter(lin, p1);
      bb.MakeEdge(e, c, loc, BRep_Tool::Tolerance(src));
    }
    else
    {
      const occ::handle<Geom_Line> c = new Geom_Line(p0, gp_Dir(gp_Vec(p0, p1)));
      bb.MakeEdge(e, c, tol);
    }
    if (!(t0 < t1))
      fail_op("brep: an edge against its curve");
    bb.Add(e, vs[v0].Oriented(TopAbs_FORWARD));
    bb.Add(e, vs[v1].Oriented(TopAbs_REVERSED));
    bb.Range(e, t0, t1);
    es[i] = e;
  }
  std::vector<TopoDS_Face> fs(nf);
  for (size_t i = 0; i < nf; ++i)
  {
    const uint32_t kind = r.face_origin[3 * i];
    const TopoDS_Face& src = TopoDS::Face(input(srcs, r.face_origin[3 * i + 1], TopAbs_FACE, r.face_origin[3 * i + 2]));
    const bool rev = b.reversed[i];
    if (kind == 0)
    {
      // A cut's tool face is the result's the other way round.
      fs[i] = (src.Orientation() == TopAbs_REVERSED) == rev ? src : TopoDS::Face(src.Reversed());
      continue;
    }
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> s = BRep_Tool::Surface(src, loc);
    TopoDS_Face                     f;
    bb.MakeFace(f, s, loc, BRep_Tool::Tolerance(src));
    for (uint32_t l = b.face_loops[i]; l < b.face_loops[i + 1]; ++l)
    {
      TopoDS_Wire w;
      bb.MakeWire(w);
      const uint32_t u0 = b.loop_uses[l], u1 = b.loop_uses[l + 1];
      // Built on the surface's side: a reversed face's loops run the other way.
      for (uint32_t k = 0; k < u1 - u0; ++k)
      {
        const uint32_t u    = b.uses[rev ? u1 - 1 - k : u0 + k];
        const bool     back = (u % 2 == 1) != rev;
        bb.Add(w, es[u / 2].Oriented(back ? TopAbs_REVERSED : TopAbs_FORWARD));
      }
      w.Closed(true);
      bb.Add(f, w);
    }
    fs[i] = rev ? TopoDS::Face(f.Reversed()) : f;
  }
  TopoDS_Compound out;
  bb.MakeCompound(out);
  std::vector<TopoDS_Solid> sol(b.solid_faces.size() - 1);
  for (size_t s = 0; s + 1 < b.solid_faces.size(); ++s)
  {
    if (r.solid_origin[3 * s] == 0)
    {
      sol[s] = TopoDS::Solid(input(srcs, r.solid_origin[3 * s + 1], TopAbs_SOLID, r.solid_origin[3 * s + 2]));
      bb.Add(out, sol[s]);
      continue;
    }
    TopoDS_Shell sh;
    bb.MakeShell(sh);
    for (uint32_t k = b.solid_faces[s]; k < b.solid_faces[s + 1]; ++k)
      bb.Add(sh, fs[b.faces_of_solids[k]]);
    sh.Closed(true);
    bb.MakeSolid(sol[s]);
    bb.Add(sol[s], sh);
    bb.Add(out, sol[s]);
  }
  static const TopAbs_ShapeEnum kinds[] = {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX};
  auto output = [&](uint32_t kind, uint32_t i) -> TopoDS_Shape {
    switch (kind)
    {
      case 0:
        return sol[i];
      case 1:
        return fs[i];
      case 2:
        return es[i];
      default:
        return vs[i];
    }
  };
  hist = new BRepTools_History();
  for (size_t k = 0; k + 5 < r.history.size(); k += 6)
  {
    const uint32_t*     h  = &r.history[k];
    const TopoDS_Shape& in = input(srcs, h[1], kinds[h[0]], h[2]);
    if (h[3] == 2)
      hist->Remove(in);
    else if (h[3] == 0)
      hist->AddModified(in, output(h[4], h[5]));
    else
      hist->AddGenerated(in, output(h[4], h[5]));
  }
  return out;
}

} // namespace

bool rust_decline(const char* why) { return decline(why); }

bool rust_boolean(uint8_t op, const std::vector<TopoDS_Shape>& args, std::vector<const OcShape*>& srcs,
                  std::vector<std::unique_ptr<OcShape>>& own, TopoDS_Shape& out, occ::handle<BRepTools_History>& hist)
{
  srcs.resize(args.size(), nullptr);
  for (size_t i = 0; i < args.size(); ++i)
    if (srcs[i] == nullptr || !srcs[i]->shape.IsEqual(args[i]))
    {
      own.push_back(std::make_unique<OcShape>(args[i]));
      srcs[i] = own.back().get();
    }
  std::vector<BrepBody> bodies(args.size());
  double                max_tol = 0;
  for (size_t i = 0; i < args.size(); ++i)
    if (const char* why = to_body(*srcs[i], bodies[i], max_tol))
      return decline(why);
  if (max_tol > kMaxTol)
    return decline("a tolerant input");
  // Arguments sharing sub-shapes: OCCT keeps the shared ones as they are.
  for (size_t i = 1; i < args.size(); ++i)
    for (size_t j = 0; j < i; ++j)
    {
      const ShapeIMap& vi = srcs[i]->map(TopAbs_VERTEX);
      const ShapeIMap& vj = srcs[j]->map(TopAbs_VERTEX);
      for (int k = 1; k <= vi.Extent(); ++k)
        if (vj.Contains(vi(k)))
          return decline("arguments share vertices");
    }
  // Two points are one within both their tolerances (OCCT's rule).
  const double tol = 2 * std::max(max_tol, 1e-7);
  BrepResult   r;
  if (!brep_boolean(op, rust::Slice<const BrepBody>(bodies.data(), bodies.size()), tol, r))
    return decline(nullptr);
  out = build(r, srcs, max_tol > 0 ? max_tol : 1e-7, hist);
  return true;
}

} // namespace noshape
