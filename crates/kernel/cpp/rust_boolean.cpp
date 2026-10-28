// noshape's own Boolean (crates/brep, docs/BOOLEAN-SPEEDUP.md phase 3): the arguments as flat bodies, the
// result and its history back as shapes. op_boolean runs it first with NOSHAPE_BOOL_RUST=1; it declines
// anything but solids bounded by planes and cylinders, and OCCT runs as before.
#include "internal.hpp"

#include <cstdlib>

#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepTools_History.hxx>
#include <ElCLib.hxx>
#include <ElSLib.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_Circle.hxx>
#include <Geom_CylindricalSurface.hxx>
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

occ::handle<Geom_Curve> basis_of(const occ::handle<Geom_Curve>& c)
{
  occ::handle<Geom_Curve> b = c;
  while (true)
  {
    occ::handle<Geom_TrimmedCurve> t = occ::down_cast<Geom_TrimmedCurve>(b);
    if (t.IsNull())
      return b;
    b = t->BasisCurve();
  }
}

void push3(rust::Vec<double>& v, const gp_XYZ& p)
{
  v.push_back(p.X());
  v.push_back(p.Y());
  v.push_back(p.Z());
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
    if (c.IsNull())
      return "an edge without a curve";
    const occ::handle<Geom_Curve>  bc   = basis_of(c);
    const occ::handle<Geom_Circle> circ = occ::down_cast<Geom_Circle>(bc);
    if (occ::down_cast<Geom_Line>(bc).IsNull() && circ.IsNull())
      return "a curved edge";
    TopoDS_Vertex v1, v2;
    TopExp::Vertices(e, v1, v2);
    if (v1.IsNull() || v2.IsNull())
      return "an edge without vertices";
    if (v1.IsSame(v2) && (circ.IsNull() || std::abs(l - f - 2 * M_PI) > 1e-9))
      return "a closed edge";
    if (circ.IsNull())
      for (int k = 0; k < 11; ++k)
        b.curves.push_back(0);
    else
    {
      const gp_Circ g = circ->Circ().Transformed(loc.Transformation());
      b.curves.push_back(1);
      push3(b.curves, g.Location().XYZ());
      push3(b.curves, g.Axis().Direction().XYZ());
      push3(b.curves, g.XAxis().Direction().XYZ());
      b.curves.push_back(g.Radius());
    }
    max_tol = std::max(max_tol, BRep_Tool::Tolerance(e));
    b.edges.push_back(static_cast<uint32_t>(vm.FindIndex(v1) - 1));
    b.edges.push_back(static_cast<uint32_t>(vm.FindIndex(v2) - 1));
  }
  b.face_loops.push_back(0);
  b.loop_uses.push_back(0);
  std::vector<int> seen(em.Extent() + 1, 0);
  for (int i = 1; i <= fm.Extent(); ++i)
  {
    const TopoDS_Face&                    f = TopoDS::Face(fm(i));
    TopLoc_Location                       loc;
    const occ::handle<Geom_Surface>       surf = BRep_Tool::Surface(f, loc);
    const occ::handle<Geom_Plane>         pl   = occ::down_cast<Geom_Plane>(surf);
    const occ::handle<Geom_CylindricalSurface> cyl = occ::down_cast<Geom_CylindricalSurface>(surf);
    if (pl.IsNull() && cyl.IsNull())
      return "a curved face";
    if (f.Orientation() != TopAbs_FORWARD && f.Orientation() != TopAbs_REVERSED)
      return "an internal face";
    max_tol   = std::max(max_tol, BRep_Tool::Tolerance(f));
    bool flip = false;
    if (!pl.IsNull())
    {
      const gp_Pln p = pl->Pln().Transformed(loc.Transformation());
      b.surfaces.push_back(0);
      push3(b.surfaces, p.Location().XYZ());
      push3(b.surfaces, p.Axis().Direction().XYZ());
      for (int k = 0; k < 4; ++k)
        b.surfaces.push_back(0);
    }
    else
    {
      // A left-handed frame is the right-handed one about the other way of its axis, with the normal
      // inward (the face the other way round).
      const gp_Cylinder c = cyl->Cylinder().Transformed(loc.Transformation());
      const gp_Ax3&     a = c.Position();
      b.surfaces.push_back(1);
      push3(b.surfaces, c.Location().XYZ());
      push3(b.surfaces, a.XDirection().Crossed(a.YDirection()).XYZ());
      push3(b.surfaces, a.XDirection().XYZ());
      b.surfaces.push_back(c.Radius());
      flip = !a.Direct();
    }
    b.reversed.push_back((f.Orientation() == TopAbs_REVERSED) != flip);
    for (TopoDS_Iterator wi(f); wi.More(); wi.Next())
    {
      if (wi.Value().ShapeType() != TopAbs_WIRE)
        return "a vertex in a face";
      for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
      {
        const TopoDS_Shape& e = ei.Value();
        const int           k = em.FindIndex(e);
        if (k <= 0 || (e.Orientation() != TopAbs_FORWARD && e.Orientation() != TopAbs_REVERSED))
          return "an internal edge";
        // A seam (twice in a cylinder's face) once each way.
        if (seen[k] == i && (cyl.IsNull() || BRep_Tool::IsClosed(TopoDS::Edge(e), f) == false))
          return "an edge twice in a face";
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

// `s` as a flat body, from its cache (made here the first time).
const char* cached_body(const OcShape& s, BrepBody& b, double& max_tol)
{
  if (std::getenv("NOSHAPE_BREP_NOCACHE") != nullptr)
    return to_body(s, b, max_tol);
  {
    const std::lock_guard<std::mutex> lock(s.myBrepLock);
    if (s.myBrepWhy != nullptr)
      return s.myBrepWhy;
    if (s.myBrep)
    {
      b       = *s.myBrep;
      max_tol = std::max(max_tol, s.myBrepTol);
      return nullptr;
    }
  }
  double      tol = 0;
  const char* why = to_body(s, b, tol);
  const std::lock_guard<std::mutex> lock(s.myBrepLock);
  if (why != nullptr)
    s.myBrepWhy = why;
  else if (!s.myBrep)
  {
    s.myBrep    = std::make_shared<BrepBody>(b);
    s.myBrepTol = tol;
  }
  max_tol = std::max(max_tol, tol);
  return why;
}

// This thread's last result (its flat body, the shapes built for its entities, the tolerance), for
// rust_boolean_keep.
struct Built
{
  BrepBody                  body;
  std::vector<TopoDS_Shape> v, e, f, s;
  double                    tol = 0;
  bool                      set = false;
};
thread_local Built t_built;

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
  // Edges: the input's own, a piece on its curve, or new on a line or circle.
  std::vector<TopoDS_Edge> es(ne);
  for (size_t i = 0; i < ne; ++i)
  {
    const uint32_t kind = r.edge_origin[3 * i];
    if (kind == 0)
    {
      es[i] = TopoDS::Edge(input(srcs, r.edge_origin[3 * i + 1], TopAbs_EDGE, r.edge_origin[3 * i + 2]).Oriented(TopAbs_FORWARD));
      continue;
    }
    const uint32_t          v0 = b.edges[2 * i], v1 = b.edges[2 * i + 1];
    const gp_Pnt            p0 = point_of(b, v0), p1 = point_of(b, v1);
    const double*           cv = &b.curves[11 * i];
    occ::handle<Geom_Curve> c;
    TopLoc_Location         loc;
    double                  f = 0, l = 0, tol_e = tol;
    if (kind == 1)
    {
      const TopoDS_Edge src = TopoDS::Edge(input(srcs, r.edge_origin[3 * i + 1], TopAbs_EDGE, r.edge_origin[3 * i + 2]));
      c                     = BRep_Tool::Curve(src, loc, f, l);
      tol_e                 = BRep_Tool::Tolerance(src);
    }
    else if (cv[0] == 0)
      c = new Geom_Line(p0, gp_Dir(gp_Vec(p0, p1)));
    else
    {
      const gp_Pnt ctr(cv[1], cv[2], cv[3]);
      const gp_Dir n(cv[4], cv[5], cv[6]);
      // Its parameter 0 at the start.
      gp_Vec x(ctr, p0);
      x -= gp_Vec(n) * x.Dot(gp_Vec(n));
      c = new Geom_Circle(gp_Ax2(ctr, n, gp_Dir(x)), cv[10]);
      f = 0;
    }
    const occ::handle<Geom_Curve> bc = basis_of(c);
    double                        t0 = 0, t1 = 0;
    if (const occ::handle<Geom_Circle> circ = occ::down_cast<Geom_Circle>(bc); !circ.IsNull())
    {
      const gp_Circ g = circ->Circ().Transformed(loc.Transformation());
      t0              = f + std::fmod(std::fmod(ElCLib::Parameter(g, p0) - f, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
      double d        = std::fmod(std::fmod(ElCLib::Parameter(g, p1) - t0, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
      if (v0 == v1 || d < 1e-12)
        d = 2 * M_PI;
      t1 = t0 + d;
    }
    else
    {
      const gp_Lin g = occ::down_cast<Geom_Line>(bc)->Lin().Transformed(loc.Transformation());
      t0             = ElCLib::Parameter(g, p0);
      t1             = ElCLib::Parameter(g, p1);
    }
    if (!(t0 < t1))
      fail_op("brep: an edge against its curve");
    TopoDS_Edge e;
    bb.MakeEdge(e, c, loc, tol_e);
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
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> s   = BRep_Tool::Surface(src, loc);
    const occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(s);
    // The face's way round against its surface's normal (to_body: a left-handed cylinder's is inward).
    const bool rev = b.reversed[i] != (!cs.IsNull() && !cs->Position().Direct());
    if (kind == 0)
    {
      // A cut's tool face is the result's the other way round.
      fs[i] = (src.Orientation() == TopAbs_REVERSED) == rev ? src : TopoDS::Face(src.Reversed());
      continue;
    }
    TopoDS_Face                     f;
    bb.MakeFace(f, s, loc, BRep_Tool::Tolerance(src));
    // Its edges' curves on a cylinder (a plane's are implicit).
    if (!cs.IsNull())
    {
      const gp_Cylinder cyl = cs->Cylinder().Transformed(loc.Transformation());
      const gp_Dir up = cyl.Position().XDirection().Crossed(cyl.Position().YDirection());
      const double      eps = 1e-7;
      std::vector<int>  uses(ne, 0);
      for (uint32_t k = b.loop_uses[b.face_loops[i]]; k < b.loop_uses[b.face_loops[i + 1]]; ++k)
        ++uses[b.uses[k] / 2];
      // Its angles: from where the Boolean's start, by whole turns to where its kept edges' curves are.
      double umin = r.face_u0[i];
      for (size_t k = 0; k < ne; ++k)
      {
        double f0 = 0, f1 = 0;
        occ::handle<Geom2d_Curve> pc;
        // (Not a seam: it lies at either end.)
        if (uses[k] == 1 && r.edge_origin[3 * k] == 0 && !BRep_Tool::IsClosed(es[k], s, loc)
            && !(pc = BRep_Tool::CurveOnSurface(es[k], s, loc, f0, f1)).IsNull())
        {
          const double um = pc->Value(0.5 * (f0 + f1)).X();
          umin += 2 * M_PI * std::floor((um - umin) / (2 * M_PI));
          break;
        }
      }
      const double umax = umin + 2 * M_PI;
      for (size_t k = 0; k < ne; ++k)
      {
        if (uses[k] == 0)
          continue;
        const TopoDS_Edge& e = es[k];
        double             f0 = 0, f1 = 0;
        if (r.edge_origin[3 * k] == 0 && !BRep_Tool::CurveOnSurface(e, s, loc, f0, f1).IsNull())
          continue;
        TopLoc_Location               el;
        const occ::handle<Geom_Curve> c  = BRep_Tool::Curve(e, el, f0, f1);
        const occ::handle<Geom_Curve> bc = basis_of(c);
        // u in the face's range.
        auto shift = [&](double lo, double hi) {
          const double n = std::ceil((umin - eps - lo) / (2 * M_PI));
          if (hi + n * 2 * M_PI > umax + eps)
            fail_op("brep: an edge outside its face's range");
          return n * 2 * M_PI;
        };
        double u = 0, v = 0;
        if (const occ::handle<Geom_Circle> circ = occ::down_cast<Geom_Circle>(bc); !circ.IsNull())
        {
          const gp_Circ g  = circ->Circ().Transformed(el.Transformation());
          const double  sg = g.Axis().Direction().Dot(up) > 0 ? 1.0 : -1.0;
          ElSLib::Parameters(cyl, ElCLib::Value(0, g), u, v);
          const double a = u + sg * f0, z = u + sg * f1;
          u += shift(std::min(a, z), std::max(a, z));
          bb.UpdateEdge(e, new Geom2d_Line(gp_Pnt2d(u, v), gp_Dir2d(sg, 0)), s, loc, BRep_Tool::Tolerance(e));
          continue;
        }
        const gp_Lin g  = occ::down_cast<Geom_Line>(bc)->Lin().Transformed(el.Transformation());
        const double dv = g.Direction().Dot(cyl.Axis().Direction()) > 0 ? 1.0 : -1.0;
        ElSLib::Parameters(cyl, g.Location(), u, v);
        u += shift(u, u);
        // On the seam's line the edge has both of the seam's curves (the faces either side of it each take one).
        const bool on_seam = std::abs(u - umin) < eps || std::abs(u - umax) < eps;
        if (on_seam)
          u = umin;
        if (!on_seam)
        {
          bb.UpdateEdge(e, new Geom2d_Line(gp_Pnt2d(u, v), gp_Dir2d(0, dv)), s, loc, BRep_Tool::Tolerance(e));
          continue;
        }
        // A seam: the way up on the face's right, the way down on its left.
        const double left = u, right = u + 2 * M_PI;
        const occ::handle<Geom2d_Curve> up   = new Geom2d_Line(gp_Pnt2d(dv > 0 ? right : left, v), gp_Dir2d(0, dv));
        const occ::handle<Geom2d_Curve> down = new Geom2d_Line(gp_Pnt2d(dv > 0 ? left : right, v), gp_Dir2d(0, dv));
        bb.UpdateEdge(e, up, down, s, loc, BRep_Tool::Tolerance(e));
      }
    }
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
  t_built.v.assign(vs.begin(), vs.end());
  t_built.e.assign(es.begin(), es.end());
  t_built.f.assign(fs.begin(), fs.end());
  t_built.s.assign(sol.begin(), sol.end());
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
  // Big targets are the local Boolean's (op_boolean), until this one is local too.
  if (srcs[0]->map(TopAbs_FACE).Extent() >= 200 && std::getenv("NOSHAPE_BREP_BIG") == nullptr)
    return decline("a big target (the local Boolean's)");
  std::vector<BrepBody> bodies(args.size());
  double                max_tol = 0;
  {
    const KernelTimer timer("boolean.rust.convert");
    for (size_t i = 0; i < args.size(); ++i)
      if (const char* why = cached_body(*srcs[i], bodies[i], max_tol))
        return decline(why);
  }
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
  t_built.set = false;
  {
    const KernelTimer timer("boolean.rust.brep");
    if (!brep_boolean(op, rust::Slice<const BrepBody>(bodies.data(), bodies.size()), tol, r))
      return decline(nullptr);
  }
  const KernelTimer timer("boolean.rust.build");
  out = build(r, srcs, max_tol > 0 ? max_tol : 1e-7, hist);
  t_built.body = std::move(r.body);
  t_built.tol  = max_tol;
  t_built.set  = true;
  return true;
}

void rust_boolean_keep(const OcShape& out)
{
  if (!t_built.set)
    return;
  t_built.set = false;
  const KernelTimer timer("boolean.rust.keep");
  Built             done;
  std::swap(done, t_built);
  const BrepBody& b = done.body;
  // The body in `out`'s own order (its maps').
  const ShapeIMap& vm = out.map(TopAbs_VERTEX);
  const ShapeIMap& em = out.map(TopAbs_EDGE);
  const ShapeIMap& fm = out.map(TopAbs_FACE);
  const ShapeIMap& sm = out.map(TopAbs_SOLID);
  const size_t nv = b.vertices.size() / 3, ne = b.edges.size() / 2, nf = b.reversed.size(), ns = b.solid_faces.size() - 1;
  if (static_cast<size_t>(vm.Extent()) != nv || static_cast<size_t>(em.Extent()) != ne || static_cast<size_t>(fm.Extent()) != nf
      || static_cast<size_t>(sm.Extent()) != ns)
    return;
  auto order = [](const ShapeIMap& m, const std::vector<TopoDS_Shape>& built, std::vector<uint32_t>& at) {
    at.resize(built.size());
    for (size_t i = 0; i < built.size(); ++i)
    {
      const int k = m.FindIndex(built[i]);
      if (k <= 0)
        return false;
      at[i] = static_cast<uint32_t>(k - 1);
    }
    return true;
  };
  std::vector<uint32_t> av, ae, af, as;
  if (!order(vm, done.v, av) || !order(em, done.e, ae) || !order(fm, done.f, af) || !order(sm, done.s, as))
    return;
  auto c = std::make_shared<BrepBody>();
  std::vector<double> vx(3 * nv), cu(11 * ne), su(11 * nf);
  std::vector<uint32_t> ed(2 * ne);
  for (size_t i = 0; i < nv; ++i)
    for (int k = 0; k < 3; ++k)
      vx[3 * av[i] + k] = b.vertices[3 * i + k];
  for (size_t i = 0; i < ne; ++i)
  {
    ed[2 * ae[i]]     = av[b.edges[2 * i]];
    ed[2 * ae[i] + 1] = av[b.edges[2 * i + 1]];
    for (int k = 0; k < 11; ++k)
      cu[11 * ae[i] + k] = b.curves[11 * i + k];
  }
  std::vector<size_t> face_of(nf);
  for (size_t i = 0; i < nf; ++i)
  {
    face_of[af[i]] = i;
    for (int k = 0; k < 11; ++k)
      su[11 * af[i] + k] = b.surfaces[11 * i + k];
  }
  for (double x : vx)
    c->vertices.push_back(x);
  for (uint32_t x : ed)
    c->edges.push_back(x);
  for (double x : cu)
    c->curves.push_back(x);
  for (double x : su)
    c->surfaces.push_back(x);
  c->face_loops.push_back(0);
  c->loop_uses.push_back(0);
  for (size_t j = 0; j < nf; ++j)
  {
    const size_t i = face_of[j];
    c->reversed.push_back(b.reversed[i]);
    for (uint32_t l = b.face_loops[i]; l < b.face_loops[i + 1]; ++l)
    {
      for (uint32_t k = b.loop_uses[l]; k < b.loop_uses[l + 1]; ++k)
        c->uses.push_back(2 * ae[b.uses[k] / 2] + b.uses[k] % 2);
      c->loop_uses.push_back(static_cast<uint32_t>(c->uses.size()));
    }
    c->face_loops.push_back(static_cast<uint32_t>(c->loop_uses.size() - 1));
  }
  std::vector<size_t> solid_of(ns);
  for (size_t i = 0; i < ns; ++i)
    solid_of[as[i]] = i;
  c->solid_faces.push_back(0);
  for (size_t j = 0; j < ns; ++j)
  {
    const size_t i = solid_of[j];
    for (uint32_t k = b.solid_faces[i]; k < b.solid_faces[i + 1]; ++k)
      c->faces_of_solids.push_back(af[b.faces_of_solids[k]]);
    c->solid_faces.push_back(static_cast<uint32_t>(c->faces_of_solids.size()));
  }
  const std::lock_guard<std::mutex> lock(out.myBrepLock);
  out.myBrep    = c;
  out.myBrepTol = done.tol;
}

} // namespace noshape
