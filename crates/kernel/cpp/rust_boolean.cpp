// noshape's own Boolean (crates/brep, docs/BOOLEAN-SPEEDUP.md phase 3): the arguments as flat bodies, the
// result and its history back as shapes. op_boolean tries it (NOSHAPE_BOOL_RUST=0: never); it declines
// anything but solids bounded by planes and cylinders, and OCCT runs as before.
#include "internal.hpp"

#include <chrono>
#include <optional>
#include <cstdlib>

#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepLib.hxx>
#include <BRepTools.hxx>
#include <TopExp_Explorer.hxx>
#include <BRepTools_History.hxx>
#include <ElCLib.hxx>
#include <ElSLib.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_Circle.hxx>
#include <Geom_ConicalSurface.hxx>
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

// Inputs with a larger tolerance are tolerant shapes: OCCT's. (Points are one within the largest tolerance of
// all the inputs, where OCCT goes by each point's own: kept small, as 1e-3 merged points 0.2 mm apart that OCCT
// keeps apart.)
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
  // (Or an edge alone: a line's common with a solid; or faces alone: sheets apart.)
  if ((sm.Extent() == 0 || fm.Extent() == 0) && !(sm.Extent() == 0 && fm.Extent() == 0 && em.Extent() == 1) && !(sm.Extent() == 0 && fm.Extent() > 0))
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
    {
      // A cone's apex.
      TopoDS_Vertex v1, v2;
      TopExp::Vertices(e, v1, v2);
      if (v1.IsNull() || !v1.IsSame(v2))
        return "a degenerate edge";
      b.curves.push_back(2);
      for (int k = 0; k < 10; ++k)
        b.curves.push_back(0);
      b.edges.push_back(static_cast<uint32_t>(vm.FindIndex(v1) - 1));
      b.edges.push_back(static_cast<uint32_t>(vm.FindIndex(v1) - 1));
      continue;
    }
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
    const occ::handle<Geom_ConicalSurface>     con = occ::down_cast<Geom_ConicalSurface>(surf);
    if (pl.IsNull() && cyl.IsNull() && con.IsNull())
      return "a curved face";
    if (f.Orientation() != TopAbs_FORWARD && f.Orientation() != TopAbs_REVERSED)
      return "an internal face";
    max_tol   = std::max(max_tol, BRep_Tool::Tolerance(f));
    bool flip = false;
    if (!pl.IsNull())
    {
      // Its normal X × Y (a left-handed frame's is minus its axis).
      const gp_Pln  p = pl->Pln().Transformed(loc.Transformation());
      const gp_Ax3& a = p.Position();
      b.surfaces.push_back(0);
      push3(b.surfaces, p.Location().XYZ());
      push3(b.surfaces, a.XDirection().Crossed(a.YDirection()).XYZ());
      for (int k = 0; k < 5; ++k)
        b.surfaces.push_back(0);
    }
    else if (!con.IsNull())
    {
      // As a cylinder's below; its half-angle the other way about a left-handed frame's axis.
      const gp_Cone c = con->Cone().Transformed(loc.Transformation());
      const gp_Ax3& a = c.Position();
      b.surfaces.push_back(2);
      push3(b.surfaces, c.Location().XYZ());
      push3(b.surfaces, a.XDirection().Crossed(a.YDirection()).XYZ());
      push3(b.surfaces, a.XDirection().XYZ());
      b.surfaces.push_back(c.RefRadius());
      b.surfaces.push_back(a.Direct() ? c.SemiAngle() : -c.SemiAngle());
      flip = !a.Direct();
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
      b.surfaces.push_back(0);
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
        // A seam (twice in a cylinder's or cone's face) once each way.
        if (seen[k] == i && ((cyl.IsNull() && con.IsNull()) || BRep_Tool::IsClosed(TopoDS::Edge(e), f) == false))
          return "an edge twice in a face";
        seen[k] = i;
        b.uses.push_back(static_cast<uint32_t>(2 * (k - 1) + (e.Orientation() == TopAbs_REVERSED ? 1 : 0)));
      }
      b.loop_uses.push_back(static_cast<uint32_t>(b.uses.size()));
    }
    b.face_loops.push_back(static_cast<uint32_t>(b.loop_uses.size() - 1));
  }
  b.solid_shells.push_back(0);
  b.shell_faces.push_back(0);
  for (int i = 1; i <= sm.Extent(); ++i)
  {
    for (TopoDS_Iterator si(sm(i)); si.More(); si.Next())
    {
      if (si.Value().ShapeType() != TopAbs_SHELL)
        return "a solid of other than shells";
      for (TopoDS_Iterator fi(si.Value()); fi.More(); fi.Next())
      {
        const int k = fm.FindIndex(fi.Value());
        if (k <= 0)
          return "a shell's face not mapped";
        b.faces_of_shells.push_back(static_cast<uint32_t>(k - 1));
      }
      b.shell_faces.push_back(static_cast<uint32_t>(b.faces_of_shells.size()));
    }
    b.solid_shells.push_back(static_cast<uint32_t>(b.shell_faces.size() - 1));
  }
  if (sm.Extent() > 0 && b.faces_of_shells.size() != static_cast<size_t>(fm.Extent()))
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

// The result's shapes and history: `out`, `hist`; or why not.
const char* build(const BrepResult& r, const std::vector<const OcShape*>& srcs, double tol, TopoDS_Shape& result, occ::handle<BRepTools_History>& hist)
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
    // A piece's ends at its edge's own: its parameters there (as OCCT keeps them).
    std::optional<double> at0, at1;
    if (kind == 1)
    {
      const TopoDS_Edge src = TopoDS::Edge(input(srcs, r.edge_origin[3 * i + 1], TopAbs_EDGE, r.edge_origin[3 * i + 2]).Oriented(TopAbs_FORWARD));
      c                     = BRep_Tool::Curve(src, loc, f, l);
      tol_e                 = BRep_Tool::Tolerance(src);
      TopoDS_Vertex a, z;
      TopExp::Vertices(src, a, z);
      const bool closed = a.IsSame(z);
      if (!a.IsNull() && BRep_Tool::Pnt(a).Distance(p0) < 1e-12)
        at0 = f;
      if (!z.IsNull() && BRep_Tool::Pnt(z).Distance(p1) < 1e-12 && !(closed && v0 != v1))
        at1 = l;
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
    if (at0)
      t0 = *at0;
    if (at1)
      t1 = *at1;
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
    const occ::handle<Geom_Surface> s0  = BRep_Tool::Surface(src, loc);
    // On its input face's surface: the input's edges have their curves there already. (Never change an input
    // shape: other threads may be reading it.)
    const occ::handle<Geom_Surface>& s = s0;
    const occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(s);
    const occ::handle<Geom_ConicalSurface>     cn = occ::down_cast<Geom_ConicalSurface>(s);
    const bool                                 round = !cs.IsNull() || !cn.IsNull();
    const gp_Ax3 frame = !cs.IsNull() ? cs->Position().Transformed(loc.Transformation()) : !cn.IsNull() ? cn->Position().Transformed(loc.Transformation()) : gp_Ax3();
    // The face's way round against its surface's normal (to_body: a left-handed cylinder's or cone's is
    // inward).
    const bool rev = b.reversed[i] != (round && !frame.Direct());
    if (kind == 0)
    {
      // A cut's tool face is the result's the other way round.
      fs[i] = (src.Orientation() == TopAbs_REVERSED) == rev ? src : TopoDS::Face(src.Reversed());
      continue;
    }
    TopoDS_Face                     f;
    bb.MakeFace(f, s, loc, BRep_Tool::Tolerance(src));
    // Its edges' curves on a cylinder or cone (a plane's are made below).
    if (round)
    {
      const gp_Cylinder cyl = cs.IsNull() ? gp_Cylinder() : cs->Cylinder().Transformed(loc.Transformation());
      const gp_Cone     con = cn.IsNull() ? gp_Cone() : cn->Cone().Transformed(loc.Transformation());
      auto params = [&](const gp_Pnt& p, double& u, double& v) {
        if (!cs.IsNull())
          ElSLib::Parameters(cyl, p, u, v);
        else
          ElSLib::Parameters(con, p, u, v);
      };
      const gp_Dir      up  = frame.XDirection().Crossed(frame.YDirection());
      const double      eps = 1e-7;
      std::vector<int>  uses(ne, 0);
      for (uint32_t k = b.loop_uses[b.face_loops[i]]; k < b.loop_uses[b.face_loops[i + 1]]; ++k)
        ++uses[b.uses[k] / 2];
      // The input's own edges: their curves on the surface (one without any: declined).
      double umin = r.face_u0[i];
      bool   placed = false;
      for (size_t k = 0; k < ne; ++k)
      {
        double f0 = 0, f1 = 0;
        occ::handle<Geom2d_Curve> pc;
        if (uses[k] == 0 || r.edge_origin[3 * k] != 0)
          continue;
        if ((pc = BRep_Tool::CurveOnSurface(es[k], s0, loc, f0, f1)).IsNull())
          return "an input edge without its curve on a face";
        if (BRep_Tool::IsClosed(es[k], s0, loc))
          continue;
        // Its angles: from where the Boolean's start, by whole turns to where the input's edges' curves are
        // (not a seam's: it lies at either end; not an apex's: it spans them).
        if (!placed && !BRep_Tool::Degenerated(es[k]))
        {
          const double um = pc->Value(0.5 * (f0 + f1)).X();
          umin += 2 * M_PI * std::floor((um - umin) / (2 * M_PI));
          placed = true;
        }
      }
      // A piece of an edge with two curves on this surface that this face uses once (between two faces on it,
      // once a seam): both, as OCCT's piece keeps them (the model's merging of faces goes by them).
      auto two_curves = [&](size_t k) -> std::optional<TopoDS_Edge> {
        // (Not this face's own seam, which it uses both ways: below.)
        if (r.edge_origin[3 * k] != 1 || uses[k] != 1)
          return std::nullopt;
        const TopoDS_Edge se = TopoDS::Edge(input(srcs, r.edge_origin[3 * k + 1], TopAbs_EDGE, r.edge_origin[3 * k + 2]).Oriented(TopAbs_FORWARD));
        if (BRep_Tool::Degenerated(se) || !BRep_Tool::IsClosed(se, s0, loc))
          return std::nullopt;
        return se;
      };
      // With no input edge of its own to go by: by the angles of its input face, which holds it (and whose
      // curves those pieces keep).
      if (!placed)
      {
        double u1 = 0, u2 = 0, v1 = 0, v2 = 0;
        BRepTools::UVBounds(src, u1, u2, v1, v2);
        umin += 2 * M_PI * std::floor((0.5 * (u1 + u2) - umin) / (2 * M_PI));
      }
      const double umax = umin + 2 * M_PI;
      for (size_t k = 0; k < ne; ++k)
      {
        if (uses[k] == 0 || r.edge_origin[3 * k] == 0)
          continue;
        const TopoDS_Edge& e = es[k];
        if (const std::optional<TopoDS_Edge> se = two_curves(k))
        {
          double                          a0 = 0, a1 = 0;
          const occ::handle<Geom2d_Curve> pf = BRep_Tool::CurveOnSurface(*se, s0, loc, a0, a1);
          const occ::handle<Geom2d_Curve> pr = BRep_Tool::CurveOnSurface(TopoDS::Edge(se->Reversed()), s0, loc, a0, a1);
          bb.UpdateEdge(e, pf, pr, s, loc, BRep_Tool::Tolerance(e));
          continue;
        }
        double             f0 = 0, f1 = 0;
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
          params(ElCLib::Value(0, g), u, v);
          const double a = u + sg * f0, z = u + sg * f1;
          u += shift(std::min(a, z), std::max(a, z));
          bb.UpdateEdge(e, new Geom2d_Line(gp_Pnt2d(u, v), gp_Dir2d(sg, 0)), s, loc, BRep_Tool::Tolerance(e));
          continue;
        }
        const gp_Lin g = occ::down_cast<Geom_Line>(bc)->Lin().Transformed(el.Transformation());
        // Along the generator: v changes as the line's parameter does, one way or the other.
        // (Its angle from its middle: a cone's generator may start at the apex.)
        double u1 = 0, v1 = 0, um = 0, vm = 0;
        params(g.Location(), u, v);
        params(g.Location().Translated(gp_Vec(g.Direction())), u1, v1);
        params(ElCLib::Value(0.5 * (f0 + f1), g), um, vm);
        u = um;
        const double dv = v1 > v ? 1.0 : -1.0;
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
        const occ::handle<Geom2d_Curve> c_up   = new Geom2d_Line(gp_Pnt2d(dv > 0 ? right : left, v), gp_Dir2d(0, dv));
        const occ::handle<Geom2d_Curve> c_down = new Geom2d_Line(gp_Pnt2d(dv > 0 ? left : right, v), gp_Dir2d(0, dv));
        bb.UpdateEdge(e, c_up, c_down, s, loc, BRep_Tool::Tolerance(e));
      }
    }
    for (uint32_t l = b.face_loops[i]; l < b.face_loops[i + 1]; ++l)
    {
      TopoDS_Wire w;
      bb.MakeWire(w);
      const uint32_t u0 = b.loop_uses[l], u1 = b.loop_uses[l + 1];
      // Built on the surface's side: a reversed face's edges the other way round, in the same order (as
      // OCCT's: unify_same_domain goes by the order of a seam's uses).
      for (uint32_t k = 0; k < u1 - u0; ++k)
      {
        const uint32_t u    = b.uses[u0 + k];
        const bool     back = (u % 2 == 1) != rev;
        bb.Add(w, es[u / 2].Oriented(back ? TopAbs_REVERSED : TopAbs_FORWARD));
      }
      w.Closed(true);
      bb.Add(f, w);
    }
    // On a plane, the curves of the edges it made stored, as OCCT's pieces have them (mass properties
    // integrate along them; a big face's would cost more than they're worth).
    if (!round && b.loop_uses[b.face_loops[i + 1]] - b.loop_uses[b.face_loops[i]] <= 64)
      for (uint32_t k = b.loop_uses[b.face_loops[i]]; k < b.loop_uses[b.face_loops[i + 1]]; ++k)
        if (r.edge_origin[3 * (b.uses[k] / 2)] != 0)
          BRepLib::BuildPCurveForEdgeOnPlane(es[b.uses[k] / 2], f);
    fs[i] = rev ? TopoDS::Face(f.Reversed()) : f;
  }
  TopoDS_Compound out;
  bb.MakeCompound(out);
  std::vector<TopoDS_Solid> sol(b.solid_shells.size() - 1);
  for (size_t s = 0; s + 1 < b.solid_shells.size(); ++s)
  {
    if (r.solid_origin[3 * s] == 0)
    {
      sol[s] = TopoDS::Solid(input(srcs, r.solid_origin[3 * s + 1], TopAbs_SOLID, r.solid_origin[3 * s + 2]));
      bb.Add(out, sol[s]);
      continue;
    }
    bb.MakeSolid(sol[s]);
    for (uint32_t h = b.solid_shells[s]; h < b.solid_shells[s + 1]; ++h)
    {
      TopoDS_Shell sh;
      bb.MakeShell(sh);
      for (uint32_t k = b.shell_faces[h]; k < b.shell_faces[h + 1]; ++k)
        bb.Add(sh, fs[b.faces_of_shells[k]]);
      sh.Closed(true);
      bb.Add(sol[s], sh);
    }
    bb.Add(out, sol[s]);
  }
  // A result of edges alone (a line's common with a solid).
  if (nf == 0)
    for (const TopoDS_Edge& e : es)
      bb.Add(out, e);
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
  result = out;
  return nullptr;
}

} // namespace

bool rust_decline(const char* why) { return decline(why); }

bool rust_boolean(uint8_t op, const std::vector<TopoDS_Shape>& args, std::vector<const OcShape*>& srcs,
                  std::vector<std::unique_ptr<OcShape>>& own, TopoDS_Shape& out, occ::handle<BRepTools_History>& hist,
                  double fuzzy, bool touching_only)
{
  // A stale regeneration's Boolean stops (as OCCT's does through its progress indicator).
  const std::atomic<bool>* cancel = thread_cancel_flag();
  auto stop = [&] {
    if (cancel != nullptr && cancel->load(std::memory_order_relaxed))
      fail_op("cancelled");
  };
  stop();
  srcs.resize(args.size(), nullptr);
  for (size_t i = 0; i < args.size(); ++i)
    if (srcs[i] == nullptr || !srcs[i]->shape.IsEqual(args[i]))
    {
      own.push_back(std::make_unique<OcShape>(args[i]));
      srcs[i] = own.back().get();
    }
  std::vector<BrepBody> bodies(args.size());
  double                max_tol = 0;
  {
    const KernelTimer timer("boolean.rust.convert");
    for (size_t i = 0; i < args.size(); ++i)
      if (const char* why = cached_body(*srcs[i], bodies[i], max_tol))
        return decline(why);
  }
  // A line's common with a solid whose vertices alone are tolerant (model-core's rays at bodies with a few):
  // those vertices are judged by their own tolerances here (far from the line, else declined), the rest by the
  // edges' and faces'.
  if (max_tol > kMaxTol && args.size() == 2 && srcs[0]->map(TopAbs_FACE).Extent() == 0 && srcs[0]->map(TopAbs_EDGE).Extent() == 1)
  {
    double ef = 0;
    for (const OcShape* x : srcs)
    {
      const ShapeIMap& em = x->map(TopAbs_EDGE);
      for (int k = 1; k <= em.Extent(); ++k)
        ef = std::max(ef, BRep_Tool::Tolerance(TopoDS::Edge(em(k))));
      const ShapeIMap& fm = x->map(TopAbs_FACE);
      for (int k = 1; k <= fm.Extent(); ++k)
        ef = std::max(ef, BRep_Tool::Tolerance(TopoDS::Face(fm(k))));
    }
    const ShapeIMap& lv = srcs[0]->map(TopAbs_VERTEX);
    for (int k = 1; k <= lv.Extent(); ++k)
      ef = std::max(ef, BRep_Tool::Tolerance(TopoDS::Vertex(lv(k))));
    if (ef <= kMaxTol && lv.Extent() == 2)
    {
      const gp_Pnt a = BRep_Tool::Pnt(TopoDS::Vertex(lv(1))), z = BRep_Tool::Pnt(TopoDS::Vertex(lv(2)));
      const gp_Vec d(a, z);
      const ShapeIMap& sv = srcs[1]->map(TopAbs_VERTEX);
      for (int k = 1; k <= sv.Extent(); ++k)
      {
        const TopoDS_Vertex& v  = TopoDS::Vertex(sv(k));
        const double         vt = BRep_Tool::Tolerance(v);
        if (vt <= kMaxTol)
          continue;
        const gp_Pnt p = BRep_Tool::Pnt(v);
        const double t = std::clamp(gp_Vec(a, p).Dot(d) / d.SquareMagnitude(), 0.0, 1.0);
        // (Within ten times the two tolerances: the Rust side's grey zone.)
        if (p.Distance(a.Translated(d * t)) <= 10 * (vt + std::max(ef, 1e-7)))
          return decline("a tolerant vertex near the line");
      }
      max_tol = ef;
    }
  }
  if (max_tol > kMaxTol)
    return decline("a tolerant input");
  // Arguments sharing sub-shapes: OCCT keeps the shared ones as they are. (One map of every argument's vertices
  // so far: a pattern's thousand instances against each other took seconds.)
  {
    ShapeIMap seen;
    for (size_t i = 0; i < args.size(); ++i)
    {
      const ShapeIMap& vi = srcs[i]->map(TopAbs_VERTEX);
      for (int k = 1; k <= vi.Extent(); ++k)
        if (seen.Contains(vi(k)))
          return decline("arguments share vertices");
      for (int k = 1; k <= vi.Extent(); ++k)
        seen.Add(vi(k));
    }
  }
  // Two points are one within both their tolerances (OCCT's rule; a fuzzy Boolean adds its value to each).
  const double tol = 2 * (std::max(max_tol, 1e-7) + fuzzy);
  BrepResult   r;
  t_built.set = false;
  {
    const KernelTimer timer("boolean.rust.brep");
    if (!brep_boolean(op, rust::Slice<const BrepBody>(bodies.data(), bodies.size()), tol, touching_only, r))
      return decline(nullptr);
  }
  const KernelTimer timer("boolean.rust.build");
  stop();
  if (const char* why = build(r, srcs, max_tol > 0 ? max_tol : 1e-7, out, hist))
    return decline(why);
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
  const size_t nv = b.vertices.size() / 3, ne = b.edges.size() / 2, nf = b.reversed.size(), ns = b.solid_shells.size() - 1;
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
  std::vector<double> vx(3 * nv), cu(11 * ne), su(12 * nf);
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
    for (int k = 0; k < 12; ++k)
      su[12 * af[i] + k] = b.surfaces[12 * i + k];
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
  c->solid_shells.push_back(0);
  c->shell_faces.push_back(0);
  for (size_t j = 0; j < ns; ++j)
  {
    const size_t i = solid_of[j];
    for (uint32_t h = b.solid_shells[i]; h < b.solid_shells[i + 1]; ++h)
    {
      for (uint32_t k = b.shell_faces[h]; k < b.shell_faces[h + 1]; ++k)
        c->faces_of_shells.push_back(af[b.faces_of_shells[k]]);
      c->shell_faces.push_back(static_cast<uint32_t>(c->faces_of_shells.size()));
    }
    c->solid_shells.push_back(static_cast<uint32_t>(c->shell_faces.size() - 1));
  }
  const std::lock_guard<std::mutex> lock(out.myBrepLock);
  out.myBrep    = c;
  out.myBrepTol = done.tol;
}

} // namespace noshape

namespace noshape {

bool quick_boolean(uint8_t op, const ShapeList& args, const ShapeList& tools, bool parallel, TopoDS_Shape& out,
                   occ::handle<BRepTools_History>& hist)
{
  const auto                t0 = std::chrono::steady_clock::now();
  std::vector<TopoDS_Shape> all;
  for (const TopoDS_Shape& x : args)
    all.push_back(x);
  for (const TopoDS_Shape& x : tools)
    all.push_back(x);
  const char* path = "direct";
  bool        done = false;
  // (Not in the replay's strict mode: that is for op_boolean's.)
  if (args.Extent() == 1 && rust_boolean_on() && !brep_strict())
  {
    std::vector<const OcShape*>           srcs;
    std::vector<std::unique_ptr<OcShape>> own;
    const KernelTimer                     timer("boolean.rust.direct");
    done = rust_boolean(op, all, srcs, own, out, hist);
    if (done)
      path = "direct_rust";
  }
  if (!done)
  {
    std::unique_ptr<BRepAlgoAPI_BooleanOperation> b;
    if (op == 0)
      b = std::make_unique<BRepAlgoAPI_Fuse>();
    else if (op == 1)
      b = std::make_unique<BRepAlgoAPI_Cut>();
    else
      b = std::make_unique<BRepAlgoAPI_Common>();
    b->SetArguments(args);
    b->SetTools(tools);
    b->SetRunParallel(parallel);
    b->Build();
    if (b->HasErrors() || !b->IsDone())
      return false;
    out  = b->Shape();
    hist = b->History();
  }
  if (brep_recording())
  {
    const double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();
    OcShapeList  a, t;
    a.items.assign(all.begin(), all.begin() + args.Extent());
    t.items.assign(all.begin() + args.Extent(), all.end());
    brep_record_direct(op, a, t, make_result(all, out, hist.get()), parallel, ms, path);
  }
  return true;
}

} // namespace noshape
