// noshape's own Boolean (crates/brep, docs/BOOLEAN-SPEEDUP.md phase 3): the arguments as flat bodies, the
// result and its history back as shapes. op_boolean tries it (NOSHAPE_BOOL_RUST=0: never); it declines
// anything but solids bounded by planes and cylinders, and OCCT runs as before.
#include "internal.hpp"

#include <chrono>
#include <functional>
#include <optional>
#include <cstdlib>

#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepLib.hxx>
#include <BRepBndLib.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepTools.hxx>
#include <TopExp_Explorer.hxx>
#include <BRepTools_History.hxx>
#include <ElCLib.hxx>
#include <ElSLib.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_Circle.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom_Conic.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <Geom_Hyperbola.hxx>
#include <Geom_Parabola.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_SurfaceOfLinearExtrusion.hxx>
#include <Geom_SurfaceOfRevolution.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2dConvert.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BSplCLib.hxx>
#include <IntTools_Curve.hxx>
#include <IntTools_FaceFace.hxx>
#include <OSD_Parallel.hxx>
#include <Standard_Failure.hxx>
#include <array>
#include <algorithm>
#include <map>
#include <BOPTools_AlgoTools2D.hxx>
#include <gp_Elips.hxx>
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
// While a caller tries noshape's Boolean one way before another (RustLenient): a decline returns, even in the
// replay's strict mode; why, for the caller.
thread_local int t_lenient = 0;
} // namespace

RustLenient::RustLenient() { ++t_lenient; }
RustLenient::~RustLenient() { --t_lenient; }
std::string rust_last_decline() { return std::string(brep_last_decline()); }

namespace {

// Declines (the replay's strict mode: fails).
bool decline(const char* why)
{
  if (why != nullptr)
    brep_decline(why);
  if (brep_strict() && t_lenient == 0)
    fail_unsupported("noshape's Boolean declined");
  return false;
}

// Inputs with a larger tolerance are tolerant shapes: OCCT's. (Points are one within the largest tolerance of
// all the inputs, where OCCT goes by each point's own: kept small, as 1e-3 merged points 0.2 mm apart that OCCT
// keeps apart.)
constexpr double kMaxTol = 1e-5;
// The Rust Boolean's grey band, in tolerances (noshape_brep::GREY).
constexpr double GREY_TOL = 4.0;

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

// An ellipse or circle as a cubic B-spline through 129 of its points at its own parameters over one turn (within
// ~1e-11 of it; its parameters the conic's), else null: the curve swept into an extruded face.
occ::handle<Geom_BSplineCurve> conic_as_bspline(const occ::handle<Geom_Curve>& c)
{
  if (occ::down_cast<Geom_Ellipse>(c).IsNull() && occ::down_cast<Geom_Circle>(c).IsNull())
    return occ::handle<Geom_BSplineCurve>();
  const int                                n   = 128;
  occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, n + 1);
  occ::handle<NCollection_HArray1<double>> prm = new NCollection_HArray1<double>(1, n + 1);
  for (int k = 0; k <= n; ++k)
  {
    const double t = 2 * M_PI * k / n;
    pts->SetValue(k + 1, c->Value(t));
    prm->SetValue(k + 1, t);
  }
  GeomAPI_Interpolate fit(pts, prm, false, 1e-12);
  fit.Perform();
  return fit.IsDone() ? fit.Curve() : occ::handle<Geom_BSplineCurve>();
}

void push3(rust::Vec<double>& v, const gp_XYZ& p)
{
  v.push_back(p.X());
  v.push_back(p.Y());
  v.push_back(p.Z());
}

// NURBS for the Rust side (`b.nurbs`, back to back; returns where it starts): a curve as 0, its dimension,
// degree, knot count (each knot as often as its multiplicity), pole count, rational, then its knots, poles and
// weights; a surface as 1, its degrees, knot counts, pole counts, rational, knots (u, v), poles (row by row, u
// first), weights. Periodic ones made non-periodic first (the same curve, the same parameters).
double push_nurbs(rust::Vec<double>& out, const occ::handle<Geom_BSplineCurve>& c0, const TopLoc_Location& loc)
{
  occ::handle<Geom_BSplineCurve> c = occ::down_cast<Geom_BSplineCurve>(c0->Copy());
  if (c->IsPeriodic())
    c->SetNotPeriodic();
  if (!loc.IsIdentity())
    c->Transform(loc.Transformation());
  const double at = static_cast<double>(out.size());
  const NCollection_Array1<double>& k = c->KnotSequence();
  out.push_back(0);
  out.push_back(3);
  out.push_back(c->Degree());
  out.push_back(k.Length());
  out.push_back(c->NbPoles());
  out.push_back(c->IsRational() ? 1 : 0);
  for (int i = k.Lower(); i <= k.Upper(); ++i)
    out.push_back(k(i));
  for (int i = 1; i <= c->NbPoles(); ++i)
    push3(out, c->Pole(i).XYZ());
  if (c->IsRational())
    for (int i = 1; i <= c->NbPoles(); ++i)
      out.push_back(c->Weight(i));
  return at;
}

// A curve on a face's parameter plane, its parameters kept: a B-spline or a line (as a degree 1 one over
// `first`..`last`); null for others.
double push_nurbs2(rust::Vec<double>& out, const occ::handle<Geom2d_Curve>& c0, double first, double last)
{
  occ::handle<Geom2d_Curve> c = c0;
  while (const occ::handle<Geom2d_TrimmedCurve> t = occ::down_cast<Geom2d_TrimmedCurve>(c))
    c = t->BasisCurve();
  const double at = static_cast<double>(out.size());
  if (const occ::handle<Geom2d_Line> l = occ::down_cast<Geom2d_Line>(c))
  {
    const gp_Pnt2d a = l->Value(first), z = l->Value(last);
    for (const double x : {0.0, 2.0, 1.0, 4.0, 2.0, 0.0, first, first, last, last, a.X(), a.Y(), 0.0, z.X(), z.Y(), 0.0})
      out.push_back(x);
    return at;
  }
  occ::handle<Geom2d_BSplineCurve> b = occ::down_cast<Geom2d_BSplineCurve>(c);
  if (b.IsNull())
    return -1;
  b = occ::down_cast<Geom2d_BSplineCurve>(b->Copy());
  if (b->IsPeriodic())
    b->SetNotPeriodic();
  const NCollection_Array1<double>& k = b->KnotSequence();
  for (const double x : {0.0, 2.0, (double)b->Degree(), (double)k.Length(), (double)b->NbPoles(), b->IsRational() ? 1.0 : 0.0})
    out.push_back(x);
  for (int i = k.Lower(); i <= k.Upper(); ++i)
    out.push_back(k(i));
  for (int i = 1; i <= b->NbPoles(); ++i)
  {
    out.push_back(b->Pole(i).X());
    out.push_back(b->Pole(i).Y());
    out.push_back(0);
  }
  if (b->IsRational())
    for (int i = 1; i <= b->NbPoles(); ++i)
      out.push_back(b->Weight(i));
  return at;
}

double push_nurbs_surface(rust::Vec<double>& out, const occ::handle<Geom_BSplineSurface>& s0, const TopLoc_Location& loc)
{
  occ::handle<Geom_BSplineSurface> s = occ::down_cast<Geom_BSplineSurface>(s0->Copy());
  if (s->IsUPeriodic())
    s->SetUNotPeriodic();
  if (s->IsVPeriodic())
    s->SetVNotPeriodic();
  if (!loc.IsIdentity())
    s->Transform(loc.Transformation());
  const double at = static_cast<double>(out.size());
  const NCollection_Array1<double>& ku = s->UKnotSequence();
  const NCollection_Array1<double>& kv = s->VKnotSequence();
  const bool                        rational = s->IsURational() || s->IsVRational();
  for (const double x : {1.0, (double)s->UDegree(), (double)s->VDegree(), (double)ku.Length(), (double)kv.Length(), (double)s->NbUPoles(), (double)s->NbVPoles(), rational ? 1.0 : 0.0})
    out.push_back(x);
  for (int i = ku.Lower(); i <= ku.Upper(); ++i)
    out.push_back(ku(i));
  for (int i = kv.Lower(); i <= kv.Upper(); ++i)
    out.push_back(kv(i));
  for (int i = 1; i <= s->NbUPoles(); ++i)
    for (int j = 1; j <= s->NbVPoles(); ++j)
      push3(out, s->Pole(i, j).XYZ());
  if (rational)
    for (int i = 1; i <= s->NbUPoles(); ++i)
      for (int j = 1; j <= s->NbVPoles(); ++j)
        out.push_back(s->Weight(i, j));
  return at;
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
    const occ::handle<Geom_Curve>        bc   = basis_of(c);
    const occ::handle<Geom_Circle>       circ = occ::down_cast<Geom_Circle>(bc);
    const occ::handle<Geom_BSplineCurve> bsc  = occ::down_cast<Geom_BSplineCurve>(bc);
    const occ::handle<Geom_Ellipse>      ell  = occ::down_cast<Geom_Ellipse>(bc);
    if (occ::down_cast<Geom_Line>(bc).IsNull() && circ.IsNull() && bsc.IsNull() && ell.IsNull())
      return !occ::down_cast<Geom_Ellipse>(bc).IsNull() ? "an ellipse edge" : !occ::down_cast<Geom_Conic>(bc).IsNull() ? "a conic edge" : "a curved edge";
    TopoDS_Vertex v1, v2;
    TopExp::Vertices(e, v1, v2);
    if (v1.IsNull() || v2.IsNull())
      return "an edge without vertices";
    // (A closed B-spline: the whole of a closed curve, one vertex where it starts.)
    const bool closed_bs = !bsc.IsNull() && bsc->IsClosed() && std::abs(f - bsc->FirstParameter()) < 1e-12 && std::abs(l - bsc->LastParameter()) < 1e-12;
    if (v1.IsSame(v2) && !closed_bs && ((circ.IsNull() && ell.IsNull()) || std::abs(l - f - 2 * M_PI) > 1e-9))
      return "a closed edge";
    b.ellipse_minor.push_back(ell.IsNull() ? 0 : ell->MinorRadius());
    if (!bsc.IsNull())
    {
      // A B-spline: kind 5, where its NURBS starts, its parameters from the edge's start to its end.
      b.curves.push_back(5);
      b.curves.push_back(push_nurbs(b.nurbs, bsc, loc));
      b.curves.push_back(f);
      b.curves.push_back(l);
      for (int k = 0; k < 7; ++k)
        b.curves.push_back(0);
    }
    else if (!ell.IsNull())
    {
      // An ellipse: kind 3, its frame (its parameter OCCT's), major radius (the minor in `ellipse_minor`).
      const gp_Elips g = ell->Elips().Transformed(loc.Transformation());
      b.curves.push_back(3);
      push3(b.curves, g.Location().XYZ());
      push3(b.curves, g.Axis().Direction().XYZ());
      push3(b.curves, g.XAxis().Direction().XYZ());
      b.curves.push_back(g.MajorRadius());
    }
    else if (circ.IsNull())
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
    occ::handle<Geom_Surface>             surf = BRep_Tool::Surface(f, loc);
    // (A trimmed surface's own: the same parameters.)
    while (const occ::handle<Geom_RectangularTrimmedSurface> tr = occ::down_cast<Geom_RectangularTrimmedSurface>(surf))
      surf = tr->BasisSurface();
    const occ::handle<Geom_Plane>         pl   = occ::down_cast<Geom_Plane>(surf);
    const occ::handle<Geom_CylindricalSurface> cyl = occ::down_cast<Geom_CylindricalSurface>(surf);
    const occ::handle<Geom_ConicalSurface>     con = occ::down_cast<Geom_ConicalSurface>(surf);
    const occ::handle<Geom_BSplineSurface>     bss = occ::down_cast<Geom_BSplineSurface>(surf);
    const occ::handle<Geom_ToroidalSurface>    tor = occ::down_cast<Geom_ToroidalSurface>(surf);
    const occ::handle<Geom_SphericalSurface>   sph = occ::down_cast<Geom_SphericalSurface>(surf);
    const occ::handle<Geom_SurfaceOfLinearExtrusion> ext = occ::down_cast<Geom_SurfaceOfLinearExtrusion>(surf);
    const occ::handle<Geom_SurfaceOfRevolution>      rvl = occ::down_cast<Geom_SurfaceOfRevolution>(surf);
    // (A sweep of a B-spline only; an extrusion's ellipse or circle as one.)
    occ::handle<Geom_BSplineCurve> swept = occ::down_cast<Geom_BSplineCurve>(basis_of(!ext.IsNull() ? ext->BasisCurve() : !rvl.IsNull() ? rvl->BasisCurve() : occ::handle<Geom_Curve>()));
    if (swept.IsNull() && !ext.IsNull())
      swept = conic_as_bspline(basis_of(ext->BasisCurve()));
    const bool free = !bss.IsNull() || !tor.IsNull() || !sph.IsNull() || ((!ext.IsNull() || !rvl.IsNull()) && !swept.IsNull());
    if (pl.IsNull() && cyl.IsNull() && con.IsNull() && !free)
    {
      const GeomAbs_SurfaceType t = GeomAdaptor_Surface(surf).GetType();
      return t == GeomAbs_Torus ? "a torus face" : t == GeomAbs_Sphere ? "a sphere face" : t == GeomAbs_OffsetSurface ? "an offset face" : t == GeomAbs_SurfaceOfRevolution ? "a face of revolution" : t == GeomAbs_SurfaceOfExtrusion ? "an extruded face" : "a curved face";
    }
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
    else if (!bss.IsNull())
    {
      // A B-spline: kind 3, where its NURBS starts; its edges' curves on it below.
      b.surfaces.push_back(3);
      b.surfaces.push_back(push_nurbs_surface(b.nurbs, bss, loc));
      for (int k = 0; k < 10; ++k)
        b.surfaces.push_back(0);
    }
    else if (!tor.IsNull() || !sph.IsNull())
    {
      // A torus (4) or sphere (5): origin, x and y directions, size (negative: a left-handed frame), a torus's
      // minor radius; its parameters OCCT's.
      const gp_Ax3 a = !tor.IsNull() ? tor->Torus().Transformed(loc.Transformation()).Position() : sph->Sphere().Transformed(loc.Transformation()).Position();
      const double h = a.Direct() ? 1.0 : -1.0;
      b.surfaces.push_back(!tor.IsNull() ? 4 : 5);
      push3(b.surfaces, a.Location().XYZ());
      push3(b.surfaces, a.XDirection().XYZ());
      push3(b.surfaces, a.YDirection().XYZ());
      b.surfaces.push_back(h * (!tor.IsNull() ? tor->MajorRadius() : sph->Radius()));
      b.surfaces.push_back(!tor.IsNull() ? tor->MinorRadius() : 0);
    }
    else if (!ext.IsNull())
    {
      // A B-spline swept along a direction (6): where its NURBS starts, the direction.
      b.surfaces.push_back(6);
      b.surfaces.push_back(push_nurbs(b.nurbs, swept, loc));
      push3(b.surfaces, ext->Direction().Transformed(loc.Transformation()).XYZ());
      for (int k = 0; k < 7; ++k)
        b.surfaces.push_back(0);
    }
    else if (!rvl.IsNull())
    {
      // A B-spline revolved (7): where its NURBS starts, the axis.
      const gp_Ax1 ax = rvl->Axis().Transformed(loc.Transformation());
      b.surfaces.push_back(7);
      b.surfaces.push_back(push_nurbs(b.nurbs, swept, loc));
      push3(b.surfaces, ax.Location().XYZ());
      push3(b.surfaces, ax.Direction().XYZ());
      for (int k = 0; k < 4; ++k)
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
        if (seen[k] == i && ((cyl.IsNull() && con.IsNull() && !free) || BRep_Tool::IsClosed(TopoDS::Edge(e), f) == false))
          return "an edge twice in a face";
        seen[k] = i;
        b.uses.push_back(static_cast<uint32_t>(2 * (k - 1) + (e.Orientation() == TopAbs_REVERSED ? 1 : 0)));
      }
      b.loop_uses.push_back(static_cast<uint32_t>(b.uses.size()));
    }
    b.face_loops.push_back(static_cast<uint32_t>(b.loop_uses.size() - 1));
    if (free)
    {
      // Its edges' curves on it (face, edge, a seam's other one, where its NURBS starts, the edge's parameters).
      const TopoDS_Face ff = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
      ShapeIMap         done;
      for (TopExp_Explorer x(ff, TopAbs_EDGE); x.More(); x.Next())
      {
        const TopoDS_Edge e = TopoDS::Edge(x.Current().Oriented(TopAbs_FORWARD));
        if (done.Contains(e))
          continue;
        done.Add(e);
        const bool closed = BRep_Tool::IsClosed(e, ff);
        for (int second = 0; second < (closed ? 2 : 1); ++second)
        {
          double                          a = 0, z = 0;
          const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(second ? TopoDS::Edge(e.Reversed()) : e, ff, a, z);
          if (pc.IsNull())
            return "an edge without its curve on a face";
          const double at = push_nurbs2(b.nurbs, pc, a, z);
          if (at < 0)
            return "a curve on a free-form face other than a line or B-spline";
          for (const double v : {(double)(i - 1), (double)(em.FindIndex(e) - 1), (double)second, at, a, z})
            b.pcurves.push_back(v);
        }
      }
    }
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

} // namespace

rust::String brep_body_of(const OcShape& s, BrepBody& out)
{
  double      tol = 0;
  const char* why = to_body(s, out, tol);
  return rust::String(why != nullptr ? why : "");
}

namespace
{

// `s` as a flat body, from its cache (made here the first time).
const char* cached_body(const OcShape& s, BrepBody& b, double& max_tol)
{
  if (std::getenv("NOSHAPE_BREP_NOCACHE") != nullptr)
    return to_body(s, b, max_tol);
  {
    const LockedSection locked_section_253;
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
  // A result of this Boolean's, its body not remapped yet (rust_boolean_keep).
  std::function<std::shared_ptr<const BrepBody>(const OcShape&)> lazy;
  {
    const LockedSection locked_section_266;
    const std::lock_guard<std::mutex> lock(s.myBrepLock);
    std::swap(lazy, s.myBrepLazy);
  }
  if (lazy)
    if (std::shared_ptr<const BrepBody> kept = lazy(s))
    {
      const LockedSection locked_section_272;
      const std::lock_guard<std::mutex> lock(s.myBrepLock);
      if (!s.myBrep)
        s.myBrep = kept;
      b       = *s.myBrep;
      max_tol = std::max(max_tol, s.myBrepTol);
      return nullptr;
    }
  double      tol = 0;
  const char* why = to_body(s, b, tol);
  const LockedSection locked_section_281;
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
  // Per face: the target's own (argument 0's), as it was.
  std::vector<char>         target_face;
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

// An ellipse's edge as OCCT makes it (BOPAlgo_PaveFiller::MakeBlocks): on its section's curve, its parameters
// within the part of it the edge is in, its curves on the two faces (made over that part).
struct EllEdge
{
  occ::handle<Geom_Curve>   curve;
  TopoDS_Face               face[2];
  occ::handle<Geom2d_Curve> pcurve[2];
  double                    t0 = 0, t1 = 0, tol = 0;
};

// OCCT's sections of pairs of the arguments' faces (arg, face, arg, face; the earlier argument's first).
using Sections = std::map<std::array<uint32_t, 4>, std::vector<IntTools_Curve>>;

// During one of noshape's own Booleans (rust_boolean): its arguments, and the sections of free-form faces it
// asked for (brep_occt_sections), by id: the curve and its two faces (in the order of its curves on them).
struct FreeSections
{
  const std::vector<const OcShape*>*                     srcs = nullptr;
  std::vector<std::pair<std::array<TopoDS_Face, 2>, IntTools_Curve>> curves;
  // Sections moved from others (brep_occt_moved), by id (the same as `curves`'): built when an edge takes one.
  struct Moved
  {
    uint32_t from;
    gp_Trsf  move;
    // Whether the faces' order is the other way round from the section's moved from's (arguments ordered), and
    // which faces are that section's own (another argument's face both pairs share: its curves move too).
    bool swap;
    bool same[2];
  };
  std::vector<std::optional<Moved>> moved;
  // The section `id`, built if it is a moved one.
  const std::pair<std::array<TopoDS_Face, 2>, IntTools_Curve>& curve(uint32_t id);
  // OCCT's sections made ahead (brep_occt_ahead), by pair (the earlier argument's face first) until asked for.
  std::map<std::array<uint32_t, 4>, std::vector<IntTools_Curve>> ahead;
};

// Edge `e`'s curve on face `f` (over its own 3D curve's parameters) from the face's own edge lying along it (an
// edge of another argument coincident with it, which the Boolean made one): that edge's, its parameters mapped
// onto `e`'s (they correspond affinely, checked along it); null if none does.
occ::handle<Geom2d_Curve> coincident_pcurve(const TopoDS_Edge& e, const TopoDS_Face& f, double tol)
{
  double                        ea = 0, eb = 0;
  const occ::handle<Geom_Curve> ec = BRep_Tool::Curve(e, ea, eb);
  if (ec.IsNull())
    return occ::handle<Geom2d_Curve>();
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    const TopoDS_Edge             o = TopoDS::Edge(x.Current());
    double                        oa = 0, ob = 0, ca = 0, cb = 0;
    const occ::handle<Geom_Curve> oc = BRep_Tool::Curve(o, oa, ob);
    if (oc.IsNull() || BRep_Tool::IsClosed(o, f))
      continue;
    const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(o, f, ca, cb);
    if (pc.IsNull())
      continue;
    // Where e's ends are on o's curve, then e's points between against o's there.
    double p[2];
    bool   ok = true;
    for (int k = 0; k < 2 && ok; ++k)
    {
      GeomAPI_ProjectPointOnCurve pr(ec->Value(k == 0 ? ea : eb), oc, oa, ob);
      ok = pr.NbPoints() > 0 && pr.LowerDistance() <= tol;
      if (ok)
        p[k] = pr.LowerDistanceParameter();
    }
    if (!ok || std::abs(p[1] - p[0]) < 1e-12)
      continue;
    const double al = (p[1] - p[0]) / (eb - ea);
    for (int k = 1; k < 8 && ok; ++k)
    {
      const double t = ea + (eb - ea) * k / 8;
      ok = ec->Value(t).Distance(oc->Value(p[0] + al * (t - ea))) <= tol;
    }
    if (!ok)
      continue;
    occ::handle<Geom2d_BSplineCurve> bs = Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(pc, std::min(p[0], p[1]), std::max(p[0], p[1])));
    if (bs.IsNull())
      continue;
    if (al < 0)
      bs->Reverse();
    NCollection_Array1<double> knots = bs->Knots();
    BSplCLib::Reparametrize(ea, eb, knots);
    bs->SetKnots(knots);
    return bs;
  }
  return occ::handle<Geom2d_Curve>();
}

thread_local FreeSections* t_free = nullptr;

// A section's tolerance as the pave filler gives it: its own, at least the faces' (ToleranceFF), at least 5e-6
// unless both faces are planes, cylinders, cones, spheres or tori.
double section_tol(const IntTools_Curve& ic, const TopoDS_Face& fa, const TopoDS_Face& fb)
{
  auto analytic = [](const TopoDS_Face& f) {
    const GeomAbs_SurfaceType t = GeomAdaptor_Surface(BRep_Tool::Surface(f)).GetType();
    return t == GeomAbs_Plane || t == GeomAbs_Cylinder || t == GeomAbs_Cone || t == GeomAbs_Sphere || t == GeomAbs_Torus;
  };
  const double ff = std::max(BRep_Tool::Tolerance(fa), BRep_Tool::Tolerance(fb));
  return std::max(ic.Tolerance(), analytic(fa) && analytic(fb) ? ff : std::max(ff, 5e-6));
}

double turns(double x)
{
  return std::fmod(std::fmod(x, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
}

const char* ellipse_edge(const BrepResult& r, const std::vector<const OcShape*>& srcs, size_t i, const gp_Pnt& p0,
                         const gp_Pnt& p1, bool closed, double tol, Sections& sections, EllEdge& out)
{
  const uint32_t*         ef = &r.ellipse_faces[4 * i];
  std::array<uint32_t, 4> key{ef[0], ef[1], ef[2], ef[3]};
  if (std::make_pair(key[2], key[3]) < std::make_pair(key[0], key[1]))
    key = {ef[2], ef[3], ef[0], ef[1]};
  const TopoDS_Face fa = TopoDS::Face(input(srcs, key[0], TopAbs_FACE, key[1]));
  const TopoDS_Face fb = TopoDS::Face(input(srcs, key[2], TopAbs_FACE, key[3]));
  auto              it = sections.find(key);
  if (it == sections.end())
  {
    // As the pave filler asks (approximated curves on both faces, its approximation tolerance).
    IntTools_FaceFace ff;
    ff.SetParameters(true, true, true, 1e-7);
    ff.Perform(fa, fb);
    if (!ff.IsDone())
      return "an ellipse OCCT doesn't make";
    std::vector<IntTools_Curve> lines;
    for (const IntTools_Curve& c : ff.Lines())
      lines.push_back(c);
    it = sections.emplace(key, std::move(lines)).first;
  }
  const double* cv = &r.body.curves[11 * i];
  const gp_Pnt  ctr(cv[1], cv[2], cv[3]);
  const gp_Dir  n(cv[4], cv[5], cv[6]);
  // OCCT's conic is ours within the model's tolerance (centre, and the axis over the conic's size; compilers round
  // OCCT's and ours differently by some ulps, MSVC's more than GCC's), the parameters within that tolerance along it.
  auto same = [&](const gp_Pnt& c, const gp_Dir& a, double size) {
    return c.Distance(ctr) <= tol && a.Dot(n) > 0 && a.XYZ().Subtracted(n.XYZ()).Modulus() * size <= tol;
  };
  if (cv[0] == 4)
  {
    // A hyperbola's branch (not closed): the piece of OCCT's that holds it.
    for (const IntTools_Curve& ic : it->second)
    {
      const occ::handle<Geom_Curve>     c  = ic.Curve();
      const occ::handle<Geom_Hyperbola> hy = occ::down_cast<Geom_Hyperbola>(basis_of(c));
      if (hy.IsNull())
        continue;
      const gp_Hypr g = hy->Hypr();
      if (!same(g.Location(), g.Axis().Direction(), g.MajorRadius()))
        continue;
      const double first = c->FirstParameter(), last = c->LastParameter();
      const double t0 = ElCLib::Parameter(g, p0), t1 = ElCLib::Parameter(g, p1);
      // (Its speed is at least its minor radius.)
      const double eps = std::max(1e-9, tol / g.MinorRadius());
      if (!(t1 > t0) || t0 < first - eps || t1 > last + eps)
        continue;
      out.curve     = hy;
      out.face[0]   = fa;
      out.face[1]   = fb;
      out.pcurve[0] = ic.FirstCurve2d();
      out.pcurve[1] = ic.SecondCurve2d();
      out.t0        = t0;
      out.t1        = t1;
      out.tol       = std::max({ic.Tolerance(), BRep_Tool::Tolerance(fa), BRep_Tool::Tolerance(fb)});
      return nullptr;
    }
    return "a hyperbola unlike OCCT's sections";
  }
  if (cv[0] == 6)
  {
    // A parabola (not closed): the piece of OCCT's that holds it.
    for (const IntTools_Curve& ic : it->second)
    {
      const occ::handle<Geom_Curve>    c  = ic.Curve();
      const occ::handle<Geom_Parabola> pa = occ::down_cast<Geom_Parabola>(basis_of(c));
      if (pa.IsNull())
        continue;
      const gp_Parab g = pa->Parab();
      if (!same(g.Location(), g.Axis().Direction(), std::max(p0.Distance(ctr), p1.Distance(ctr))))
        continue;
      const double first = c->FirstParameter(), last = c->LastParameter();
      double       t0 = ElCLib::Parameter(g, p0), t1 = ElCLib::Parameter(g, p1);
      // (Its speed is at least 1.)
      const double eps = std::max(1e-9, tol);
      if (!(t1 > t0) || t0 < first - eps || t1 > last + eps)
        continue;
      t0            = std::max(t0, first);
      t1            = std::min(t1, last);
      out.curve     = pa;
      out.face[0]   = fa;
      out.face[1]   = fb;
      out.pcurve[0] = ic.FirstCurve2d();
      out.pcurve[1] = ic.SecondCurve2d();
      out.t0        = t0;
      out.t1        = t1;
      out.tol       = std::max({ic.Tolerance(), BRep_Tool::Tolerance(fa), BRep_Tool::Tolerance(fb)});
      return nullptr;
    }
    return "a parabola unlike OCCT's sections";
  }
  for (const IntTools_Curve& ic : it->second)
  {
    const occ::handle<Geom_Curve>   c  = ic.Curve();
    const occ::handle<Geom_Ellipse> el = occ::down_cast<Geom_Ellipse>(basis_of(c));
    if (el.IsNull())
      continue;
    const gp_Elips g = el->Elips();
    if (!same(g.Location(), g.Axis().Direction(), g.MajorRadius()))
      continue;
    const double first = c->FirstParameter(), last = c->LastParameter();
    // Parameters within the tolerance along it (its speed is at least its minor radius): a start just short of a
    // turn past OCCT's first is at its first (a closed one starting where OCCT's does), ends just outside OCCT's
    // range at its ends.
    const double eps = std::max(1e-9, tol / g.MinorRadius());
    double       t0  = first + turns(ElCLib::Parameter(g, p0) - first);
    if (t0 > 2 * M_PI + first - eps || (t0 > last + eps && t0 - 2 * M_PI >= first - eps))
      t0 -= 2 * M_PI;
    double d = turns(ElCLib::Parameter(g, p1) - t0);
    if (closed || d < 1e-12)
      d = 2 * M_PI;
    if (t0 < first - eps || t0 + d > last + eps)
      continue;
    if (t0 < first)
      t0 = first;
    if (t0 + d > last)
      d = last - t0;
    // An end just short of an axis' end (an edge tangent to the ellipse there, its crossing found to 1e-6 by the
    // Boolean where OCCT's is exact): declined.
    for (const double t : {t0, t0 + d})
    {
      const double off = std::abs(t - M_PI_2 * std::round(t / M_PI_2));
      if (off > 1e-9 && off < 1e-5)
        return "an ellipse ending nearly at an axis";
    }
    out.curve     = el;
    out.face[0]   = fa;
    out.face[1]   = fb;
    out.pcurve[0] = ic.FirstCurve2d();
    out.pcurve[1] = ic.SecondCurve2d();
    out.t0        = t0;
    out.t1        = t0 + d;
    // The pave filler's section tolerance: the curve's and the faces' (PerformFF; MakeBlocks adds the tangential
    // one, which CorrectToleranceOfSE takes off again).
    out.tol = std::max({ic.Tolerance(), BRep_Tool::Tolerance(fa), BRep_Tool::Tolerance(fb)});
    return nullptr;
  }
  if (std::getenv("NOSHAPE_BOOL_DEBUG"))
  {
    std::fprintf(stderr, "ellipse: ours c (%g %g %g) n (%g %g %g)\n", ctr.X(), ctr.Y(), ctr.Z(), n.X(), n.Y(), n.Z());
    for (const IntTools_Curve& ic : it->second)
    {
      const occ::handle<Geom_Ellipse> el = occ::down_cast<Geom_Ellipse>(basis_of(ic.Curve()));
      if (el.IsNull())
        continue;
      const gp_Elips g = el->Elips();
      std::fprintf(stderr, "  occt c (%g %g %g) n (%g %g %g) x (%g %g %g) [%g, %g]\n", g.Location().X(), g.Location().Y(), g.Location().Z(), g.Axis().Direction().X(), g.Axis().Direction().Y(), g.Axis().Direction().Z(), g.XAxis().Direction().X(), g.XAxis().Direction().Y(), g.XAxis().Direction().Z(), ic.Curve()->FirstParameter(), ic.Curve()->LastParameter());
      const double t = ic.Curve()->FirstParameter();
      const gp_Pnt q = ic.Curve()->Value(t);
      const gp_Pnt2d u1 = ic.FirstCurve2d().IsNull() ? gp_Pnt2d() : ic.FirstCurve2d()->Value(t), u2 = ic.SecondCurve2d().IsNull() ? gp_Pnt2d() : ic.SecondCurve2d()->Value(t);
      std::fprintf(stderr, "    start (%g %g %g) uv1 (%g %g) uv2 (%g %g)\n", q.X(), q.Y(), q.Z(), u1.X(), u1.Y(), u2.X(), u2.Y());
    }
  }
  return "an ellipse unlike OCCT's sections";
}

// A piece `e` of the argument's edge (r.edge_origin[k]) on the input's face `src`: the input edge's curves on that
// face (one, or a seam's two), set on `e` (the same parameters).
// The curves on a cone of wire `w`'s new degenerate edges (its apex made anew, or a piece of it where the cone is
// split through its apex): along the apex from where the edge into its vertex ends to where the one out starts,
// the way the wire runs it (its parameter the surface's angle, as OCCT's: OCCT's are lines along u).
const char* apex_pcurves(const TopoDS_Wire& w, const occ::handle<Geom_Surface>& s, const TopLoc_Location& loc, BRep_Builder& bb)
{
  std::vector<TopoDS_Edge> es;
  for (TopoDS_Iterator it(w, false, false); it.More(); it.Next())
    es.push_back(TopoDS::Edge(it.Value()));
  auto end2d = [&](const TopoDS_Edge& e, bool at_start, gp_Pnt2d& p) {
    double                          f = 0, l = 0;
    const occ::handle<Geom2d_Curve> c = BRep_Tool::CurveOnSurface(e, s, loc, f, l);
    if (c.IsNull())
      return false;
    p = c->Value((e.Orientation() == TopAbs_REVERSED) != at_start ? f : l);
    return true;
  };
  for (size_t j = 0; j < es.size(); ++j)
  {
    const TopoDS_Edge& e = es[j];
    double             f = 0, l = 0;
    if (!BRep_Tool::Degenerated(e) || !BRep_Tool::CurveOnSurface(e, s, loc, f, l).IsNull())
      continue;
    // Its neighbours: the edge whose run ends at its vertex, the one whose run starts there (a reversed face's
    // wire lists its edges the other way round).
    const TopoDS_Vertex v = TopExp::FirstVertex(e);
    const TopoDS_Edge*  into = nullptr;
    const TopoDS_Edge*  from = nullptr;
    int                 n_into = 0, n_from = 0;
    for (const TopoDS_Edge& x : es)
    {
      if (BRep_Tool::Degenerated(x))
        continue;
      if (TopExp::LastVertex(x, true).IsSame(v))
        into = &x, ++n_into;
      if (TopExp::FirstVertex(x, true).IsSame(v))
        from = &x, ++n_from;
    }
    gp_Pnt2d a, z;
    if (n_into != 1 || n_from != 1 || !end2d(*into, false, a) || !end2d(*from, true, z))
      return "a degenerate edge without its neighbours' curves";
    if (std::abs(a.Y() - z.Y()) > 1e-9 || std::abs(a.X() - z.X()) < 1e-12)
      return "a degenerate edge between its neighbours' curves";
    // Its own way: from `a` to `z` run forwards, from `z` to `a` reversed.
    const bool     fwd = e.Orientation() != TopAbs_REVERSED;
    const gp_Pnt2d p0 = fwd ? a : z, p1 = fwd ? z : a;
    const double   sg = p1.X() > p0.X() ? 1.0 : -1.0;
    bb.UpdateEdge(e, new Geom2d_Line(gp_Pnt2d(0, p0.Y()), gp_Dir2d(sg, 0)), s, loc, BRep_Tool::Tolerance(e));
    bb.Range(e, s, loc, sg * p0.X(), sg * p1.X());
  }
  return nullptr;
}

const char* piece_pcurve(const BrepResult& r, const std::vector<const OcShape*>& srcs, size_t k, const TopoDS_Face& src, const TopoDS_Edge& e,
                         const occ::handle<Geom_Surface>& s, const TopLoc_Location& loc, BRep_Builder& bb)
{
  if (r.edge_origin[3 * k] != 1)
    return "a new edge without its curve on a face";
  const TopoDS_Edge se = TopoDS::Edge(input(srcs, r.edge_origin[3 * k + 1], TopAbs_EDGE, r.edge_origin[3 * k + 2]).Oriented(TopAbs_FORWARD));
  const TopoDS_Face sf = TopoDS::Face(src.Oriented(TopAbs_FORWARD));
  double            a = 0, z = 0;
  const occ::handle<Geom2d_Curve> pf = BRep_Tool::CurveOnSurface(se, sf, a, z);
  if (pf.IsNull())
  {
    // A piece of another argument's edge lying on the face: its curve there projected, as the pave filler's
    // (MakePCurves on the input face; the piece's tolerance to the projection's).
    BOPTools_AlgoTools2D::BuildPCurveForEdgeOnFace(e, sf);
    double b0 = 0, b1 = 0;
    if (BRep_Tool::CurveOnSurface(e, sf, b0, b1).IsNull())
      return "an input edge without its curve on a face";
    return nullptr;
  }
  if (BRep_Tool::IsClosed(se, sf))
    bb.UpdateEdge(e, pf, BRep_Tool::CurveOnSurface(TopoDS::Edge(se.Reversed()), sf, a, z), s, loc, BRep_Tool::Tolerance(e));
  else
    bb.UpdateEdge(e, pf, s, loc, BRep_Tool::Tolerance(e));
  // (A new degenerate edge has no curve of its own: its range is its curve's on the face.)
  if (BRep_Tool::Degenerated(e))
    bb.Range(e, s, loc, a, z);
  return nullptr;
}

// An ellipse's curve on its face `src` (one of the two it is the section of), set on `e` on surface `s`, as
// BOPTools_AlgoTools::MakePCurve: moved by turns to the face.
const char* ellipse_pcurve(const EllEdge& x, const TopoDS_Face& src, const TopoDS_Edge& e, const occ::handle<Geom_Surface>& s,
                           const TopLoc_Location& loc, BRep_Builder& bb)
{
  for (int j = 0; j < 2; ++j)
    if (!x.curve.IsNull() && x.face[j].IsSame(src))
    {
      if (x.pcurve[j].IsNull())
        return "an ellipse without its curve on a face";
      occ::handle<Geom2d_Curve> adjusted;
      BOPTools_AlgoTools2D::AdjustPCurveOnFace(TopoDS::Face(x.face[j].Oriented(TopAbs_FORWARD)), x.t0, x.t1, x.pcurve[j], adjusted);
      bb.UpdateEdge(e, adjusted, s, loc, BRep_Tool::Tolerance(e));
      return nullptr;
    }
  return "an ellipse on another face";
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
  // Edges: the input's own, a piece on its curve, or new on a line, circle or ellipse.
  std::vector<TopoDS_Edge> es(ne);
  std::vector<EllEdge>     ells(ne);
  Sections                 sections;
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
      if (BRep_Tool::Degenerated(src))
      {
        // A cone's apex OCCT makes anew (another argument through it): a new degenerate edge at the new vertex,
        // its curve on the cone the input's (piece_pcurve).
        if (v0 != v1)
          return "a degenerate edge between two vertices";
        TopoDS_Edge e;
        bb.MakeEdge(e);
        bb.Degenerated(e, true);
        bb.Add(e, vs[v0].Oriented(TopAbs_FORWARD));
        bb.Add(e, vs[v1].Oriented(TopAbs_REVERSED));
        es[i] = e;
        continue;
      }
      c = BRep_Tool::Curve(src, loc, f, l);
      tol_e                 = BRep_Tool::Tolerance(src);
      TopoDS_Vertex a, z;
      TopExp::Vertices(src, a, z);
      const bool closed = a.IsSame(z);
      // (Within its tolerance: a vertex merged with another argument's keeps the edge's parameter, as OCCT's
      // pave does; an ellipse's piece got 2π - 3e-9 by projection.)
      const double merged = std::max(1e-12, std::min(BRep_Tool::Tolerance(a.IsNull() ? z : a), 1e-6));
      if (!a.IsNull() && BRep_Tool::Pnt(a).Distance(p0) < merged)
        at0 = f;
      if (!z.IsNull() && BRep_Tool::Pnt(z).Distance(p1) < merged && !(closed && v0 != v1))
        at1 = l;
    }
    else if (cv[0] == 0)
      c = new Geom_Line(p0, gp_Dir(gp_Vec(p0, p1)));
    else if (cv[0] == 5)
    {
      // A piece of OCCT's section of a free-form face (as an ellipse's: its curve, its curves on the faces).
      const uint32_t id = r.edge_section[i];
      if (t_free == nullptr || id >= t_free->curves.size())
        return "a section not OCCT's";
      const auto& [faces, ic] = t_free->curve(id);
      EllEdge& x  = ells[i];
      x.curve     = ic.Curve();
      x.face[0]   = faces[0];
      x.face[1]   = faces[1];
      x.pcurve[0] = ic.FirstCurve2d();
      x.pcurve[1] = ic.SecondCurve2d();
      x.t0        = cv[2];
      x.t1        = cv[3];
      x.tol       = section_tol(ic, faces[0], faces[1]);
      c           = x.curve;
      tol_e       = x.tol;
      // At its own ends OCCT's paves are its bounds exactly (PutBoundPaveOnCurve; the Boolean's ends there were
      // found to ~1e-8).
      const double cf = c->FirstParameter(), cl = c->LastParameter();
      for (double* t : {&x.t0, &x.t1})
      {
        const double bound = *t - cf < cl - *t ? cf : cl;
        if (c->Value(*t).Distance(c->Value(bound)) <= tol_e)
          *t = bound;
      }
      const double need = tol_e + 1e-12;
      for (const uint32_t v : {v0, v1})
        if (BRep_Tool::Tolerance(vs[v]) < need)
        {
          if (r.vertex_origin[3 * v] == 0)
            return "an argument's vertex on a section";
          bb.UpdateVertex(vs[v], need);
        }
    }
    else if (cv[0] == 3 || cv[0] == 4 || cv[0] == 6)
    {
      // OCCT's own section of the two faces: its ellipse and the part of it this piece is in (OCCT's curves on
      // the faces are made over the part), the curves on the faces.
      const char* why = ellipse_edge(r, srcs, i, p0, p1, v0 == v1, tol, sections, ells[i]);
      if (why != nullptr)
        return why;
      c     = ells[i].curve;
      tol_e = ells[i].tol;
      // Its vertices' tolerances to the section's (BOPTools_AlgoTools::MakeEdge; CorrectToleranceOfSE may take
      // 1e-12 off again; an argument's vertex OCCT would replace by a new one).
      const double need = tol_e + 1e-12;
      for (const uint32_t v : {v0, v1})
        if (BRep_Tool::Tolerance(vs[v]) < need)
        {
          if (r.vertex_origin[3 * v] == 0)
            return "an argument's vertex on an ellipse";
          bb.UpdateVertex(vs[v], need);
        }
    }
    else
    {
      const gp_Pnt ctr(cv[1], cv[2], cv[3]);
      const gp_Dir n(cv[4], cv[5], cv[6]);
      // In the frame of the surface it is a section of (its parameter the cylinder's or cone's angle), as
      // OCCT's section circles: every piece of a circle on one frame. (Each from 0 at its start made
      // unify_same_domain merge a cylinder's arcs into a face more than a turn round: qa's RUST-MIRROR-NOTCH,
      // RUST-HALF-POCKET-3.) Else its parameter 0 at the start.
      gp_Vec x(ctr, p0);
      x -= gp_Vec(n) * x.Dot(gp_Vec(n));
      const gp_Vec own(cv[7], cv[8], cv[9]);
      if (own.Magnitude() > 0.5 && std::abs(own.Dot(gp_Vec(n))) < 1e-9)
        x = own;
      c = new Geom_Circle(gp_Ax2(ctr, n, gp_Dir(x)), cv[10]);
      f = 0;
    }
    const occ::handle<Geom_Curve> bc = basis_of(c);
    double                        t0 = 0, t1 = 0;
    if (cv[0] == 5 && !occ::down_cast<Geom_Conic>(bc).IsNull())
    {
      // A piece of OCCT's conic section (the Boolean went by a cubic through it): at its vertices, within the
      // turn from its start.
      const occ::handle<Geom_Conic> cn = occ::down_cast<Geom_Conic>(bc);
      auto par = [&](const gp_Pnt& p) {
        if (const occ::handle<Geom_Circle> ci = occ::down_cast<Geom_Circle>(cn))
          return ElCLib::Parameter(ci->Circ(), p);
        if (const occ::handle<Geom_Ellipse> el = occ::down_cast<Geom_Ellipse>(cn))
          return ElCLib::Parameter(el->Elips(), p);
        return cv[2];
      };
      t0       = cv[2] + std::remainder(par(p0) - cv[2], 2 * M_PI);
      t1       = cv[3] + std::remainder(par(p1) - cv[3], 2 * M_PI);
      if (v0 == v1)
        t1 = t0 + 2 * M_PI;
      ells[i].t0 = t0;
      ells[i].t1 = t1;
    }
    else if (cv[0] == 5)
    {
      // (The Boolean's own parameters: a B-spline's, or OCCT's section's.)
      t0 = ells[i].curve.IsNull() ? cv[2] : ells[i].t0;
      t1 = ells[i].curve.IsNull() ? cv[3] : ells[i].t1;
    }
    else if (const occ::handle<Geom_Ellipse> el = occ::down_cast<Geom_Ellipse>(bc); !el.IsNull() && kind == 1)
    {
      // A piece of an argument's ellipse: as a circle's, from its start through the turn to its end.
      const gp_Elips g = el->Elips().Transformed(loc.Transformation());
      t0               = f + std::fmod(std::fmod(ElCLib::Parameter(g, p0) - f, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
      double d         = std::fmod(std::fmod(ElCLib::Parameter(g, p1) - t0, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
      if (v0 == v1 || d < 1e-12)
        d = 2 * M_PI;
      t1 = t0 + d;
      // An end just short of an axis' end (an edge tangent to the ellipse there, its crossing found to 1e-6 by
      // the Boolean where OCCT's is exact): declined.
      for (const double t : {at0 ? -1.0 : t0, at1 ? -1.0 : t1})
      {
        const double off = std::abs(t - M_PI_2 * std::round(t / M_PI_2));
        if (t >= 0 && off > 1e-9 && off < 1e-5)
          return "a piece of an ellipse ending nearly at an axis";
      }
    }
    else if (!occ::down_cast<Geom_Ellipse>(bc).IsNull() || !occ::down_cast<Geom_Hyperbola>(bc).IsNull() || !occ::down_cast<Geom_Parabola>(bc).IsNull())
    {
      if (kind != 2)
        return "a piece of an ellipse";
      t0 = ells[i].t0;
      t1 = ells[i].t1;
    }
    else if (const occ::handle<Geom_Circle> circ = occ::down_cast<Geom_Circle>(bc); !circ.IsNull())
    {
      const gp_Circ g = circ->Circ().Transformed(loc.Transformation());
      t0              = f + std::fmod(std::fmod(ElCLib::Parameter(g, p0) - f, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
      double d        = std::fmod(std::fmod(ElCLib::Parameter(g, p1) - t0, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
      if (v0 == v1 || d < 1e-12)
        d = 2 * M_PI;
      t1 = t0 + d;
    }
    else if (const occ::handle<Geom_Line> ln = occ::down_cast<Geom_Line>(bc); !ln.IsNull())
    {
      const gp_Lin g = ln->Lin().Transformed(loc.Transformation());
      t0             = ElCLib::Parameter(g, p0);
      t1             = ElCLib::Parameter(g, p1);
    }
    else
    {
      if (std::getenv("NOSHAPE_BOOL_DEBUG"))
        std::fprintf(stderr, "build: edge %zu kind %u curve kind %g on a %s\n", i, kind, cv[0], bc.IsNull() ? "null" : bc->DynamicType()->Name());
      return "an edge on a curve of another kind";
    }
    if (at0)
      t0 = *at0;
    if (at1)
      t1 = *at1;
    if (!(t0 < t1))
      return "an edge against its curve";
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
    const bool free_form = occ::down_cast<Geom_Plane>(s).IsNull() && occ::down_cast<Geom_CylindricalSurface>(s).IsNull() && occ::down_cast<Geom_ConicalSurface>(s).IsNull();
    if (free_form)
    {
      // Its edges' curves on it: OCCT's for its sections, its input edges' own for their pieces (the same
      // parameters).
      std::vector<int> uses(ne, 0);
      for (uint32_t k = b.loop_uses[b.face_loops[i]]; k < b.loop_uses[b.face_loops[i + 1]]; ++k)
        ++uses[b.uses[k] / 2];
      for (size_t k = 0; k < ne; ++k)
      {
        if (uses[k] == 0 || r.edge_origin[3 * k] == 0)
          continue;
        if (b.curves[11 * k] == 5 && r.edge_section[k] != UINT32_MAX)
        {
          if (const char* why = ellipse_pcurve(ells[k], src, es[k], s, loc, bb))
            return why;
          continue;
        }
        if (r.edge_origin[3 * k] != 1)
          return "a new edge on a free-form face";
        const TopoDS_Edge se = TopoDS::Edge(input(srcs, r.edge_origin[3 * k + 1], TopAbs_EDGE, r.edge_origin[3 * k + 2]).Oriented(TopAbs_FORWARD));
        const TopoDS_Face sf = TopoDS::Face(src.Oriented(TopAbs_FORWARD));
        double            a = 0, z = 0;
        occ::handle<Geom2d_Curve> pf = BRep_Tool::CurveOnSurface(se, sf, a, z);
        // (Another argument's edge coincident with one of this face's: that one's curve on it.)
        if (pf.IsNull() && !BRep_Tool::IsClosed(se, sf))
          pf = coincident_pcurve(es[k], sf, 1e-7 + BRep_Tool::Tolerance(es[k]));
        if (pf.IsNull())
          return "an input edge without its curve on a free-form face";
        if (BRep_Tool::IsClosed(se, sf))
          bb.UpdateEdge(es[k], pf, BRep_Tool::CurveOnSurface(TopoDS::Edge(se.Reversed()), sf, a, z), s, loc, BRep_Tool::Tolerance(es[k]));
        else
          bb.UpdateEdge(es[k], pf, s, loc, BRep_Tool::Tolerance(es[k]));
        // (A new degenerate edge has no curve of its own: its range is its curve's on the face.)
        if (BRep_Tool::Degenerated(es[k]))
          bb.Range(es[k], s, loc, a, z);
      }
    }
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
      // A rounding off a whole turn (an angle from a seam's point a rounding off its axis): the turn, where OCCT's
      // seams are (a seam's curve at u = -3e-16 failed the PVZ writer's trimming, PVZ-RUST-BOOL).
      if (const double turn = 2 * M_PI * std::round(umin / (2 * M_PI)); std::abs(umin - turn) < 1e-12)
        umin = turn;
      const double umax = umin + 2 * M_PI;
      for (size_t k = 0; k < ne; ++k)
      {
        if (uses[k] == 0 || r.edge_origin[3 * k] == 0)
          continue;
        const TopoDS_Edge& e = es[k];
        // (A new degenerate edge: once its wire is made, apex_pcurves.)
        if (BRep_Tool::Degenerated(e))
          continue;
        if (const std::optional<TopoDS_Edge> se = two_curves(k))
        {
          double                          a0 = 0, a1 = 0;
          const occ::handle<Geom2d_Curve> pf = BRep_Tool::CurveOnSurface(*se, s0, loc, a0, a1);
          const occ::handle<Geom2d_Curve> pr = BRep_Tool::CurveOnSurface(TopoDS::Edge(se->Reversed()), s0, loc, a0, a1);
          // The one this face uses (by the way round it uses the edge) must be in its angles.
          bool back = false;
          for (uint32_t q = b.loop_uses[b.face_loops[i]]; q < b.loop_uses[b.face_loops[i + 1]]; ++q)
            if (b.uses[q] / 2 == k)
              back = (b.uses[q] % 2 == 1) != rev;
          const double um = (back ? pr : pf)->Value(0.5 * (a0 + a1)).X();
          if (um < umin - eps || um > umax + eps)
            return "a piece of a seam on the other side of its face";
          bb.UpdateEdge(e, pf, pr, s, loc, BRep_Tool::Tolerance(e));
          continue;
        }
        double             f0 = 0, f1 = 0;
        TopLoc_Location               el;
        const occ::handle<Geom_Curve> c  = BRep_Tool::Curve(e, el, f0, f1);
        const occ::handle<Geom_Curve> bc = basis_of(c);
        // u in the face's range (none: an edge outside it).
        auto shift = [&](double lo, double hi) -> std::optional<double> {
          const double n = std::ceil((umin - eps - lo) / (2 * M_PI));
          if (hi + n * 2 * M_PI > umax + eps)
            return std::nullopt;
          return n * 2 * M_PI;
        };
        double u = 0, v = 0;
        if (b.curves[11 * k] == 5 && r.edge_section[k] != UINT32_MAX)
        {
          if (const char* why = ellipse_pcurve(ells[k], src, e, s, loc, bb))
            return why;
          continue;
        }
        if ((!occ::down_cast<Geom_Ellipse>(bc).IsNull() || !occ::down_cast<Geom_Hyperbola>(bc).IsNull() || !occ::down_cast<Geom_Parabola>(bc).IsNull() || b.curves[11 * k] == 5) && r.edge_origin[3 * k] == 1)
        {
          if (const char* why = piece_pcurve(r, srcs, k, src, e, s, loc, bb))
            return why;
          continue;
        }
        if (!occ::down_cast<Geom_Ellipse>(bc).IsNull() || !occ::down_cast<Geom_Hyperbola>(bc).IsNull() || !occ::down_cast<Geom_Parabola>(bc).IsNull())
        {
          if (const char* why = ellipse_pcurve(ells[k], src, e, s, loc, bb))
            return why;
          continue;
        }
        if (const occ::handle<Geom_Circle> circ = occ::down_cast<Geom_Circle>(bc); !circ.IsNull())
        {
          const gp_Circ g  = circ->Circ().Transformed(el.Transformation());
          const double  sg = g.Axis().Direction().Dot(up) > 0 ? 1.0 : -1.0;
          params(ElCLib::Value(0, g), u, v);
          const double a = u + sg * f0, z = u + sg * f1;
          const std::optional<double> sh = shift(std::min(a, z), std::max(a, z));
          if (!sh)
            return "an edge outside its face's range";
          u += *sh;
          bb.UpdateEdge(e, new Geom2d_Line(gp_Pnt2d(u, v), gp_Dir2d(sg, 0)), s, loc, BRep_Tool::Tolerance(e));
          continue;
        }
        const occ::handle<Geom_Line> gl = occ::down_cast<Geom_Line>(bc);
        if (gl.IsNull())
          return "an edge of another kind on a cylinder or cone";
        const gp_Lin g = gl->Lin().Transformed(el.Transformation());
        // Along the generator: v changes as the line's parameter does, one way or the other.
        // (Its angle from its middle: a cone's generator may start at the apex.)
        double u1 = 0, v1 = 0, um = 0, vm = 0;
        params(g.Location(), u, v);
        params(g.Location().Translated(gp_Vec(g.Direction())), u1, v1);
        params(ElCLib::Value(0.5 * (f0 + f1), g), um, vm);
        u = um;
        const double dv = v1 > v ? 1.0 : -1.0;
        const std::optional<double> sh = shift(u, u);
        if (!sh)
          return "an edge outside its face's range";
        u += *sh;
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
    // Built on the surface's side: a reversed face's edges the other way round, in the same order; a face
    // going round (using its seam twice) a left-handed cylinder or cone (a mirrored one: its angles run the
    // other way from ours) in the opposite order. As OCCT's: unify_same_domain goes by the order of a seam's
    // uses (Shell's slab fuses; a part mirrored onto itself kept its hole wall in two, RUST-MIRROR-EXTRA-FACE).
    // A part of a mirrored one keeps ours (the other way, halfmir's half walls merged into a reversed face).
    std::vector<uint32_t> used;
    for (uint32_t q = b.loop_uses[b.face_loops[i]]; q < b.loop_uses[b.face_loops[i + 1]]; ++q)
      used.push_back(b.uses[q] / 2);
    std::sort(used.begin(), used.end());
    const bool mirrored = round && !frame.Direct() && std::adjacent_find(used.begin(), used.end()) != used.end();
    for (uint32_t l = b.face_loops[i]; l < b.face_loops[i + 1]; ++l)
    {
      TopoDS_Wire w;
      bb.MakeWire(w);
      const uint32_t u0 = b.loop_uses[l], u1 = b.loop_uses[l + 1];
      // (A loop through one vertex twice, no edge twice (a hole touching the boundary at a point): on a reversed
      // face its edges in the other order too, so that each starts where the one before ends, as OCCT's; a
      // simple loop either way.)
      bool pinched = false;
      if (rev && u1 - u0 <= 256)
      {
        std::vector<TopoDS_Vertex> starts;
        std::vector<uint32_t>      seen;
        for (uint32_t k = u0; k < u1; ++k)
        {
          const TopoDS_Edge& e = es[b.uses[k] / 2];
          starts.push_back(TopExp::FirstVertex(TopoDS::Edge(e.Oriented(b.uses[k] % 2 == 1 ? TopAbs_REVERSED : TopAbs_FORWARD)), true));
          seen.push_back(b.uses[k] / 2);
        }
        std::sort(seen.begin(), seen.end());
        const bool twice = std::adjacent_find(seen.begin(), seen.end()) != seen.end();
        for (size_t x = 0; x < starts.size() && !twice && !pinched; ++x)
          for (size_t y = x + 1; y < starts.size() && !pinched; ++y)
            pinched = starts[x].IsSame(starts[y]);
      }
      for (uint32_t k = 0; k < u1 - u0; ++k)
      {
        const uint32_t u    = b.uses[mirrored || pinched ? u1 - 1 - k : u0 + k];
        const bool     back = (u % 2 == 1) != rev;
        bb.Add(w, es[u / 2].Oriented(back ? TopAbs_REVERSED : TopAbs_FORWARD));
      }
      if (round)
        if (const char* why = apex_pcurves(w, s, loc, bb))
          return why;
      w.Closed(true);
      bb.Add(f, w);
    }
    // On a plane, the curves of the edges it made stored, as OCCT's pieces have them (mass properties
    // integrate along them; a big face's would cost more than they're worth).
    // (An ellipse's and a free-form section's always: its tolerance goes by them, below.)
    if (!round && !free_form)
    {
      const bool small = b.loop_uses[b.face_loops[i + 1]] - b.loop_uses[b.face_loops[i]] <= 64;
      for (uint32_t k = b.loop_uses[b.face_loops[i]]; k < b.loop_uses[b.face_loops[i + 1]]; ++k)
      {
        const uint32_t ek = b.uses[k] / 2;
        if (r.edge_origin[3 * ek] == 1 && (b.curves[11 * ek] == 3 || b.curves[11 * ek] == 5))
        {
          if (const char* why = piece_pcurve(r, srcs, ek, src, es[ek], s, loc, bb))
            return why;
        }
        else if (r.edge_origin[3 * ek] != 0 && (b.curves[11 * ek] == 3 || b.curves[11 * ek] == 4 || b.curves[11 * ek] == 6 || (b.curves[11 * ek] == 5 && r.edge_section[ek] != UINT32_MAX)))
        {
          if (const char* why = ellipse_pcurve(ells[ek], src, es[ek], s, loc, bb))
            return why;
        }
        else if (r.edge_origin[3 * ek] != 0 && small)
          BRepLib::BuildPCurveForEdgeOnPlane(es[ek], f);
      }
    }
    fs[i] = rev ? TopoDS::Face(f.Reversed()) : f;
  }
  // Made faces of cylinders and cones, all their curves set (a later face on the same surface may have set one
  // of an edge they share): one across its surface's seam angle (its angles over a multiple of 2π, though no
  // seam bounds it) declines, OCCT's unify having gone wrong on it after a mirror (halfmir2's half pocket: a
  // reversed face); so does one whose curves span more than a turn (an invalid face).
  for (size_t i = 0; i < nf; ++i)
  {
    if (r.face_origin[3 * i] == 0)
      continue;
    TopLoc_Location loc;
    const occ::handle<Geom_Surface> sf = BRep_Tool::Surface(fs[i], loc);
    if (occ::down_cast<Geom_CylindricalSurface>(sf).IsNull() && occ::down_cast<Geom_ConicalSurface>(sf).IsNull())
      continue;
    double a = 0, z = 0, v1 = 0, v2 = 0;
    BRepTools::UVBounds(fs[i], a, z, v1, v2);
    // (An end a rounding past a whole turn, 1e-7 radians: not across it.)
    if (z - a < 2 * M_PI - 1e-6 && std::floor((a + 1e-7) / (2 * M_PI)) != std::floor((z - 1e-7) / (2 * M_PI)))
      return "a piece across its surface's seam angle";
    if (z - a > 2 * M_PI + 1e-6)
      return "a face's curves over more than a turn";
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
  // A sheet's result: its faces alone, in the compound (as OCCT's).
  if (b.solid_shells.size() <= 1)
    for (size_t i = 0; i < nf; ++i)
      bb.Add(out, fs[i]);
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

namespace {
// A motion keeping face `f`'s plane, cylinder or cone, in the face's parameters: a rotation and a translation of
// them (checked at three points); none for other surfaces or where it isn't one.
std::optional<gp_Trsf2d> params_moved(const TopoDS_Face& f, const gp_Trsf& move)
{
  const occ::handle<Geom_Surface> sf = BRep_Tool::Surface(f);
  const GeomAdaptor_Surface       ad(sf);
  const gp_Pnt2d                  uv[3] = {gp_Pnt2d(0, 0), gp_Pnt2d(1e-3, 0), gp_Pnt2d(0, 1e-3)};
  gp_Pnt2d                        to[3];
  for (int i = 0; i < 3; ++i)
  {
    const gp_Pnt q = sf->Value(uv[i].X(), uv[i].Y()).Transformed(move);
    double       u = 0, v = 0;
    if (ad.GetType() == GeomAbs_Plane)
      ElSLib::Parameters(ad.Plane(), q, u, v);
    else if (ad.GetType() == GeomAbs_Cylinder)
      ElSLib::Parameters(ad.Cylinder(), q, u, v);
    else if (ad.GetType() == GeomAbs_Cone)
      ElSLib::Parameters(ad.Cone(), q, u, v);
    else
      return std::nullopt;
    // (Angles: the first's turn kept for the others.)
    if (i > 0 && ad.GetType() != GeomAbs_Plane)
      u = to[0].X() + std::remainder(u - to[0].X(), 2 * M_PI);
    to[i] = gp_Pnt2d(u, v);
  }
  const gp_Vec2d d(to[0], to[1]);
  gp_Trsf2d      t;
  t.SetRotation(gp::Origin2d(), std::atan2(d.Y(), d.X()));
  t.SetTranslationPart(gp_Vec2d(to[0].XY()));
  for (int i = 0; i < 3; ++i)
    if (uv[i].Transformed(t).Distance(to[i]) > 1e-9)
      return std::nullopt;
  return t;
}
} // namespace

const std::pair<std::array<TopoDS_Face, 2>, IntTools_Curve>& FreeSections::curve(uint32_t id)
{
  if (id < moved.size() && moved[id])
  {
    const Moved m = *moved[id];
    moved[id].reset();
    const IntTools_Curve&           ic   = curve(m.from).second;
    const occ::handle<Geom_Curve>   c    = ic.Curve().IsNull() ? occ::handle<Geom_Curve>() : occ::down_cast<Geom_Curve>(ic.Curve()->Transformed(m.move));
    occ::handle<Geom2d_Curve>       c2[2] = {ic.FirstCurve2d(), ic.SecondCurve2d()};
    if (m.swap)
      std::swap(c2[0], c2[1]);
    for (int k = 0; k < 2; ++k)
      if (m.same[k] && !c2[k].IsNull())
      {
        // (Its curve on another argument's face both pairs share: moved in its parameters; else left out, the
        // build makes it.)
        const std::optional<gp_Trsf2d> t = params_moved(curves[id].first[k], m.move);
        c2[k] = t ? occ::down_cast<Geom2d_Curve>(c2[k]->Transformed(*t)) : occ::handle<Geom2d_Curve>();
        // (On a cylinder or cone, its angles by whole turns into the surface's [0, 2π), as OCCT's own curves; the
        // ones moved here don't cross the seam.)
        const GeomAbs_SurfaceType st = GeomAdaptor_Surface(BRep_Tool::Surface(curves[id].first[k])).GetType();
        if (!c2[k].IsNull() && (st == GeomAbs_Cylinder || st == GeomAbs_Cone))
        {
          const double um = c2[k]->Value(0.5 * (c2[k]->FirstParameter() + c2[k]->LastParameter())).X();
          const double turn = 2 * M_PI * std::floor(um / (2 * M_PI));
          if (turn != 0)
          {
            gp_Trsf2d sh;
            sh.SetTranslation(gp_Vec2d(-turn, 0));
            c2[k] = occ::down_cast<Geom2d_Curve>(c2[k]->Transformed(sh));
          }
        }
      }
    curves[id].second = IntTools_Curve(c, c2[0], c2[1], ic.Tolerance(), ic.TangentialTolerance());
  }
  return curves[id];
}

rust::Vec<uint32_t> brep_occt_moved(rust::Slice<const uint32_t> ids, uint32_t a_arg, uint32_t a_face, uint32_t b_arg, uint32_t b_face, rust::Slice<const double> moved)
{
  rust::Vec<uint32_t> out;
  if (t_free == nullptr || t_free->srcs == nullptr || moved.size() != 16)
    return out;
  const std::pair<uint32_t, uint32_t> a{a_arg, a_face}, b{b_arg, b_face};
  const std::pair<uint32_t, uint32_t> ca{static_cast<uint32_t>(moved[0]), static_cast<uint32_t>(moved[1])}, cb{static_cast<uint32_t>(moved[2]), static_cast<uint32_t>(moved[3])};
  const bool        swap = b < a, cswap = cb < ca;
  const TopoDS_Face fa   = TopoDS::Face(input(*t_free->srcs, swap ? b_arg : a_arg, TopAbs_FACE, swap ? b_face : a_face));
  const TopoDS_Face fb   = TopoDS::Face(input(*t_free->srcs, swap ? a_arg : b_arg, TopAbs_FACE, swap ? a_face : b_face));
  gp_Trsf           move;
  move.SetValues(moved[4], moved[5], moved[6], moved[13], moved[7], moved[8], moved[9], moved[14], moved[10], moved[11], moved[12], moved[15]);
  // (The faces in argument order, each with the role it has: a face the moved-from pair has too is its own.)
  const std::pair<uint32_t, uint32_t> first = swap ? b : a, second = swap ? a : b;
  const std::pair<uint32_t, uint32_t> cfirst = swap ? cb : ca, csecond = swap ? ca : cb;
  for (const uint32_t id : ids)
  {
    if (id >= t_free->curves.size())
      return rust::Vec<uint32_t>();
    out.push_back(static_cast<uint32_t>(t_free->curves.size()));
    t_free->curves.push_back({{fa, fb}, IntTools_Curve()});
    t_free->moved.push_back(FreeSections::Moved{id, move, swap != cswap, {first == cfirst, second == csecond}});
  }
  return out;
}

void brep_occt_ahead(rust::Slice<const uint32_t> pairs)
{
  if (t_free == nullptr || t_free->srcs == nullptr)
    return;
  const KernelTimer timer("boolean.rust.sections_ahead");
  // The faces found here (the arguments' maps made as they are first asked for); each pair's section on its own
  // thread, kept where it is done (else made again when asked for, failing or throwing there as before).
  struct Job
  {
    std::array<uint32_t, 4>     key;
    TopoDS_Face                 fa, fb;
    bool                        done = false;
    std::vector<IntTools_Curve> lines;
  };
  std::vector<Job> jobs;
  for (size_t i = 0; i + 3 < pairs.size(); i += 4)
  {
    std::pair<uint32_t, uint32_t> a{pairs[i], pairs[i + 1]}, b{pairs[i + 2], pairs[i + 3]};
    if (b < a)
      std::swap(a, b);
    const std::array<uint32_t, 4> key{a.first, a.second, b.first, b.second};
    if (t_free->ahead.count(key) != 0)
      continue;
    jobs.push_back(Job{key, TopoDS::Face(input(*t_free->srcs, a.first, TopAbs_FACE, a.second)), TopoDS::Face(input(*t_free->srcs, b.first, TopAbs_FACE, b.second)), false, {}});
  }
  OSD_Parallel::For(0, static_cast<int>(jobs.size()), [&](int k) {
    try
    {
      IntTools_FaceFace ff;
      ff.SetParameters(true, true, true, 1e-7);
      ff.Perform(jobs[k].fa, jobs[k].fb);
      if (!ff.IsDone())
        return;
      for (const IntTools_Curve& ic : ff.Lines())
        jobs[k].lines.push_back(ic);
      jobs[k].done = true;
    }
    catch (const Standard_Failure&)
    {
      jobs[k].lines.clear();
    }
  });
  for (Job& j : jobs)
    if (j.done)
      t_free->ahead.emplace(j.key, std::move(j.lines));
}

rust::String brep_occt_sections(uint32_t a_arg, uint32_t a_face, uint32_t b_arg, uint32_t b_face, BrepSections& out)
{
  if (t_free == nullptr || t_free->srcs == nullptr)
    return rust::String("no Boolean running");
  // As the pave filler asks (approximated curves on both faces, its approximation tolerance), the earlier
  // argument's face first.
  const bool        swap = std::make_pair(b_arg, b_face) < std::make_pair(a_arg, a_face);
  const TopoDS_Face fa   = TopoDS::Face(input(*t_free->srcs, swap ? b_arg : a_arg, TopAbs_FACE, swap ? b_face : a_face));
  const TopoDS_Face fb   = TopoDS::Face(input(*t_free->srcs, swap ? a_arg : b_arg, TopAbs_FACE, swap ? a_face : b_face));
  const KernelTimer timer("boolean.rust.sections");
  std::vector<IntTools_Curve> lines;
  const auto made = t_free->ahead.find(swap ? std::array<uint32_t, 4>{b_arg, b_face, a_arg, a_face} : std::array<uint32_t, 4>{a_arg, a_face, b_arg, b_face});
  if (made != t_free->ahead.end())
  {
    lines = std::move(made->second);
    t_free->ahead.erase(made);
  }
  else
  {
    IntTools_FaceFace ff;
    ff.SetParameters(true, true, true, 1e-7);
    ff.Perform(fa, fb);
    if (!ff.IsDone())
      return rust::String("OCCT's section failed");
    for (const IntTools_Curve& ic : ff.Lines())
      lines.push_back(ic);
  }
  for (const IntTools_Curve& ic : lines)
  {
    const occ::handle<Geom_Curve> c = ic.Curve();
    if (c.IsNull())
      continue;
    const double first = c->FirstParameter(), last = c->LastParameter();
    occ::handle<Geom_BSplineCurve> bs = occ::down_cast<Geom_BSplineCurve>(basis_of(c));
    if (bs.IsNull() && !occ::down_cast<Geom_Conic>(basis_of(c)).IsNull())
    {
      // A circle or ellipse (a torus's section by a plane): for the Boolean's geometry a cubic through 65 of its
      // points at its own parameters (within ~1e-10 of it); the edge is built on the conic itself.
      const int                         n = 64;
      occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, n + 1);
      occ::handle<NCollection_HArray1<double>> prm = new NCollection_HArray1<double>(1, n + 1);
      for (int k = 0; k <= n; ++k)
      {
        const double t = first + (last - first) * k / n;
        pts->SetValue(k + 1, c->Value(t));
        prm->SetValue(k + 1, t);
      }
      GeomAPI_Interpolate fit(pts, prm, false, 1e-12);
      fit.Perform();
      if (!fit.IsDone())
        return rust::String("a section OCCT makes as a conic");
      bs = fit.Curve();
    }
    if (bs.IsNull())
    {
      // A line as a degree 1 B-spline over its range (the same parameters).
      const occ::handle<Geom_Line> l = occ::down_cast<Geom_Line>(basis_of(c));
      if (l.IsNull())
        return rust::String("a section OCCT makes as a conic");
      NCollection_Array1<gp_Pnt> poles(1, 2);
      poles(1) = l->Value(first);
      poles(2) = l->Value(last);
      NCollection_Array1<double> knots(1, 2);
      knots(1) = first;
      knots(2) = last;
      NCollection_Array1<int> mults(1, 2);
      mults(1) = 2;
      mults(2) = 2;
      bs       = new Geom_BSplineCurve(poles, knots, mults, 1);
    }
    const occ::handle<Geom2d_Curve> p0 = swap ? ic.SecondCurve2d() : ic.FirstCurve2d();
    const occ::handle<Geom2d_Curve> p1 = swap ? ic.FirstCurve2d() : ic.SecondCurve2d();
    if (p0.IsNull() || p1.IsNull())
      return rust::String("a section without its curves on the faces");
    const double at3 = push_nurbs(out.nurbs, bs, TopLoc_Location());
    // (On a plane, cylinder or cone the Boolean maps the 3D curve itself: a curve it can't take there is moot.)
    auto analytic = [](const TopoDS_Face& f) {
      const occ::handle<Geom_Surface> g = BRep_Tool::Surface(f);
      return !occ::down_cast<Geom_Plane>(g).IsNull() || !occ::down_cast<Geom_CylindricalSurface>(g).IsNull() || !occ::down_cast<Geom_ConicalSurface>(g).IsNull();
    };
    const occ::handle<Geom2d_Curve> dummy = new Geom2d_Line(gp_Pnt2d(0, 0), gp_Dir2d(1, 0));
    double at0 = push_nurbs2(out.nurbs, p0, first, last);
    if (at0 < 0 && analytic(swap ? fb : fa))
      at0 = push_nurbs2(out.nurbs, dummy, first, last);
    double at1 = push_nurbs2(out.nurbs, p1, first, last);
    if (at1 < 0 && analytic(swap ? fa : fb))
      at1 = push_nurbs2(out.nurbs, dummy, first, last);
    if (at0 < 0 || at1 < 0)
      return rust::String("a section without its curves on the faces");
    const double id = static_cast<double>(t_free->curves.size());
    t_free->curves.push_back({{fa, fb}, ic});
    t_free->moved.emplace_back();
    for (const double v : {id, first, last, at3, at0, at1, section_tol(ic, fa, fb)})
      out.curves.push_back(v);
  }
  return rust::String("");
}

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
  // A sheet of shells (not faces): OCCT's result keeps them shells, the Rust Boolean's sheets are faces alone (not
  // yet).
  for (size_t i = 0; i < args.size(); ++i)
    if (srcs[i]->map(TopAbs_SOLID).Extent() == 0 && srcs[i]->map(TopAbs_SHELL).Extent() > 0 && srcs[i]->map(TopAbs_FACE).Extent() > 0)
      return decline("a sheet of shells");
  // Tolerant sub-shapes that meet no other argument where their tolerance would matter: each sub-shape of another
  // argument near one (their boxes, grown by the grey band of the tolerances, meeting) is on it exactly (within the
  // rest's tolerance: OCCT and the Boolean put them together alike) or clearly apart (past the grey band of the two
  // tolerances, which OCCT merges within); then the Boolean goes by the rest's tolerance. Else declined.
  if (max_tol > kMaxTol && args.size() >= 2)
  {
    double rest = 0;
    auto   tol_of = [](const TopoDS_Shape& x) {
      return x.ShapeType() == TopAbs_VERTEX ? BRep_Tool::Tolerance(TopoDS::Vertex(x)) : x.ShapeType() == TopAbs_EDGE ? BRep_Tool::Tolerance(TopoDS::Edge(x)) : BRep_Tool::Tolerance(TopoDS::Face(x));
    };
    std::vector<std::pair<size_t, TopoDS_Shape>> tolerant;
    for (size_t i = 0; i < args.size(); ++i)
      for (const TopAbs_ShapeEnum t : {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_FACE})
      {
        const ShapeIMap& m = srcs[i]->map(t);
        for (int k = 1; k <= m.Extent(); ++k)
        {
          const double tx = tol_of(m(k));
          if (tx <= kMaxTol)
            rest = std::max(rest, tx);
          else
            tolerant.emplace_back(i, m(k));
        }
      }
    const double exact   = 2 * std::max(rest, 1e-7);
    bool         reaches = tolerant.size() > 64;
    std::vector<Bnd_Box> whole(args.size());
    for (size_t i = 0; i < args.size() && !reaches; ++i)
      BRepBndLib::Add(srcs[i]->shape, whole[i]);
    for (const auto& [i, x] : tolerant)
    {
      if (reaches)
        break;
      const double tx = tol_of(x);
      Bnd_Box      b;
      BRepBndLib::Add(x, b);
      b.Enlarge(GREY_TOL * (tx + kMaxTol));
      for (size_t j = 0; j < args.size() && !reaches; ++j)
      {
        if (j == i || b.IsOut(whole[j]))
          continue;
        for (const TopAbs_ShapeEnum t : {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_FACE})
        {
          const ShapeIMap& m = srcs[j]->map(t);
          for (int q = 1; q <= m.Extent() && !reaches; ++q)
          {
            Bnd_Box yb;
            BRepBndLib::Add(m(q), yb);
            if (b.IsOut(yb))
              continue;
            BRepExtrema_DistShapeShape dist(x, m(q));
            const double d = dist.IsDone() ? dist.Value() : 0.0;
            reaches = !dist.IsDone() || (d > exact && d <= GREY_TOL * (tx + tol_of(m(q))));
          }
        }
      }
    }
    if (!reaches)
      max_tol = rest;
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
  FreeSections free;
  free.srcs = &srcs;
  t_free    = &free;
  struct Reset
  {
    ~Reset() { t_free = nullptr; }
  } reset;
  {
    const KernelTimer timer("boolean.rust.brep");
    if (!brep_boolean(op, rust::Slice<const BrepBody>(bodies.data(), bodies.size()), tol, touching_only, r))
      return decline(nullptr);
  }
  const KernelTimer timer("boolean.rust.build");
  stop();
  if (const char* why = build(r, srcs, max_tol > 0 ? max_tol : 1e-7, out, hist))
    return decline(why);
  // (A result with free-form faces isn't kept as a body: the next Boolean converts it, curves on faces and all;
  // nor one with ellipses, hyperbolas or parabolas: converted, they are OCCT's curves, frame and parameters.)
  for (size_t i = 0; i < r.body.surfaces.size(); i += 12)
    if (r.body.surfaces[i] >= 3)
      return true;
  for (size_t i = 0; i < r.body.curves.size(); i += 11)
    if (r.body.curves[i] == 3 || r.body.curves[i] == 4 || r.body.curves[i] == 6)
      return true;
  t_built.target_face.assign(r.face_origin.size() / 3, 0);
  for (size_t i = 0; i < t_built.target_face.size(); ++i)
    t_built.target_face[i] = r.face_origin[3 * i] == 0 && r.face_origin[3 * i + 1] == 0;
  t_built.body = std::move(r.body);
  t_built.tol  = max_tol;
  t_built.set  = true;
  return true;
}

namespace
{
// `done`'s body in `out`'s own order (its maps'), or none when they don't match.
std::shared_ptr<const BrepBody> kept_body(const OcShape& out, const Built& done)
{
  const KernelTimer timer("boolean.rust.keep");
  const BrepBody&   b = done.body;
  // The body in `out`'s own order (its maps').
  const ShapeIMap& vm = out.map(TopAbs_VERTEX);
  const ShapeIMap& em = out.map(TopAbs_EDGE);
  const ShapeIMap& fm = out.map(TopAbs_FACE);
  const ShapeIMap& sm = out.map(TopAbs_SOLID);
  const size_t nv = b.vertices.size() / 3, ne = b.edges.size() / 2, nf = b.reversed.size(), ns = b.solid_shells.size() - 1;
  if (static_cast<size_t>(vm.Extent()) != nv || static_cast<size_t>(em.Extent()) != ne || static_cast<size_t>(fm.Extent()) != nf
      || static_cast<size_t>(sm.Extent()) != ns)
    return nullptr;
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
    return nullptr;
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
  // (The B-spline edges' curves: the curves above index into it as they are.)
  c->nurbs = b.nurbs;
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
  return c;
}
} // namespace

std::shared_ptr<const UnifyScope> rust_unify_scope(const OcShape& target)
{
  if (!t_built.set)
    return nullptr;
  const KernelTimer timer("boolean.rust.unify_scope");
  auto              sc = std::make_shared<UnifyScope>();
  ShapeIMap         out_v, out_e, changed, changed_edges;
  for (const TopoDS_Shape& v : t_built.v)
    out_v.Add(v);
  for (const TopoDS_Shape& e : t_built.e)
    out_e.Add(e);
  // The faces the target didn't have as they are (pieces, the tools', new): their vertices are seeds, and
  // so are those of the target's edges the result lost.
  for (size_t i = 0; i < t_built.f.size(); ++i)
    if (!t_built.target_face[i])
      changed.Add(t_built.f[i]);
  for (int i = 1; i <= changed.Extent(); ++i)
  {
    for (TopExp_Explorer x(changed(i), TopAbs_VERTEX); x.More(); x.Next())
      sc->seeds.Add(x.Current());
    for (TopExp_Explorer x(changed(i), TopAbs_EDGE); x.More(); x.Next())
      changed_edges.Add(x.Current());
  }
  const ShapeIMap& te = target.map(TopAbs_EDGE);
  for (int k = 1; k <= te.Extent(); ++k)
    if (!out_e.Contains(te(k)))
      for (TopExp_Explorer x(te(k), TopAbs_VERTEX); x.More(); x.Next())
        if (out_v.Contains(x.Current()))
          sc->seeds.Add(x.Current());
  // Where to look: those faces, and every face at a seed or across an edge of one of them.
  BRep_Builder bb;
  bb.MakeCompound(sc->faces);
  for (const TopoDS_Shape& f : t_built.f)
  {
    bool in = changed.Contains(f);
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More() && !in; x.Next())
      in = changed_edges.Contains(x.Current());
    for (TopExp_Explorer x(f, TopAbs_VERTEX); x.More() && !in; x.Next())
      in = sc->seeds.Contains(x.Current());
    if (in)
      bb.Add(sc->faces, f);
  }
  return sc;
}

void rust_boolean_keep(const OcShape& out)
{
  if (!t_built.set)
    return;
  t_built.set = false;
  // Remapped on first use (cached_body): the model's unify usually makes a new shape of it first.
  auto done = std::make_shared<Built>();
  std::swap(*done, t_built);
  const LockedSection locked_section_918;
  const std::lock_guard<std::mutex> lock(out.myBrepLock);
  out.myBrepTol  = done->tol;
  out.myBrepLazy = [done](const OcShape& o) { return kept_body(o, *done); };
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
    // OCCT at the working scale (ScaledInputs: a fuzz shell's slab common
    // took 30 s in metres, 0.8 s in mm; SHELL-SLOW).
    std::unique_ptr<ScaledInputs> sc;
    ShapeList                     sargs = args, stools = tools;
    if (working_scale() != 1.0 && std::getenv("NOSHAPE_BOOL_NO_SCALE") == nullptr)
    {
      sc = std::make_unique<ScaledInputs>(working_scale());
      sargs.Clear();
      stools.Clear();
      for (const TopoDS_Shape& x : args)
        sargs.Append(sc->scale(x));
      for (const TopoDS_Shape& x : tools)
        stools.Append(sc->scale(x));
    }
    std::unique_ptr<BRepAlgoAPI_BooleanOperation> b;
    if (op == 0)
      b = std::make_unique<BRepAlgoAPI_Fuse>();
    else if (op == 1)
      b = std::make_unique<BRepAlgoAPI_Cut>();
    else
      b = std::make_unique<BRepAlgoAPI_Common>();
    b->SetArguments(sargs);
    b->SetTools(stools);
    b->SetRunParallel(parallel);
    b->Build(new_cancel_indicator()->Start());
    if (b->HasErrors() || !b->IsDone())
      return false;
    out  = b->Shape();
    hist = b->History();
    if (sc)
    {
      out                                    = sc->unscale(out, hist);
      occ::handle<BRepTools_History> whole = sc->hist;
      whole->Merge(hist);
      hist = whole;
    }
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

const char* brep_body_cached(const OcShape& s, BrepBody& b, double& max_tol) { return cached_body(s, b, max_tol); }

} // namespace noshape
