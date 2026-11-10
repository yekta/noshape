// noshape's own Boolean (crates/brep, docs/BOOLEAN-SPEEDUP.md phase 3): the arguments as flat bodies, the
// result and its history back as shapes. op_boolean tries it (NOSHAPE_BOOL_RUST=0: never); it declines
// anything but solids bounded by planes and cylinders, and OCCT runs as before.
#include "internal.hpp"

#include <chrono>
#include <condition_variable>
#include <deque>
#include <functional>
#include <mutex>
#include <optional>
#include <thread>
#include <cstdlib>

#include <BRepAlgoAPI_Common.hxx>
#include <NCollection_DataMap.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRepLib.hxx>
#include <BRepBndLib.hxx>
#include <GProp_GProps.hxx>
#include <BRepGProp.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <IntSurf_PntOn2S.hxx>
#include <NCollection_List.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepTools.hxx>
#include <TopExp_Explorer.hxx>
#include <BRepTools_History.hxx>
#include <ElCLib.hxx>
#include <ElSLib.hxx>
#include <Geom2d_Line.hxx>
#include <gp_Lin.hxx>
#include <gp_Ax22d.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <gp_Ax2d.hxx>
#include <IntRes2d_IntersectionPoint.hxx>
#include <Geom2d_Circle.hxx>
#include <Geom2dAPI_InterCurveCurve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom_Conic.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <Geom_Hyperbola.hxx>
#include <Geom_Parabola.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <GeomConvert.hxx>
#include <Geom_BezierSurface.hxx>
#include <Geom_BezierCurve.hxx>
#include <Geom_OffsetSurface.hxx>
#include <GeomConvert_ApproxSurface.hxx>
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
#include <GeomLib_IsPlanarSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <TopExp.hxx>
#include <TopoDS.hxx>
#include <Precision.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Wire.hxx>

// boolean.cpp: this thread's cancel flag, budget deadline or memory watch says stop.
extern "C" int noshape_should_break();

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

occ::handle<Geom_Surface> as_plane(const occ::handle<Geom_Surface>& s)
{
  occ::handle<Geom_Surface> base = s;
  while (const occ::handle<Geom_RectangularTrimmedSurface> tr = occ::down_cast<Geom_RectangularTrimmedSurface>(base))
    base = tr->BasisSurface();
  if (occ::down_cast<Geom_BSplineSurface>(base).IsNull() && occ::down_cast<Geom_BezierSurface>(base).IsNull() && occ::down_cast<Geom_SurfaceOfLinearExtrusion>(base).IsNull())
    return s;
  const GeomLib_IsPlanarSurface planar(base, Precision::Confusion());
  if (!planar.IsPlanar())
    return s;
  // (Its normal the surface's own, so the face stays the same way round.)
  double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
  base->Bounds(u0, u1, v0, v1);
  const auto at = [](double a, double b, double k) { return Precision::IsInfinite(a) || Precision::IsInfinite(b) ? 0.0 : a + k * (b - a); };
  for (const double k : {0.5, 0.25, 0.75})
  {
    gp_Pnt p;
    gp_Vec du, dv;
    base->D1(at(u0, u1, k), at(v0, v1, k), p, du, dv);
    const gp_Vec n = du.Crossed(dv);
    if (n.Magnitude() < 1e-12)
      continue;
    const gp_Ax3  a = planar.Plan().Position();
    const gp_Dir  z = a.XDirection().Crossed(a.YDirection());
    return new Geom_Plane(gp_Ax3(a.Location(), n.Dot(gp_Vec(z)) < 0 ? z.Reversed() : z, a.XDirection()));
  }
  return s;
}

namespace {

// Declines (the replay's strict mode, or NOSHAPE_BOOL_FALLBACK=0: fails at once, the reason in the error).
bool decline(const char* why)
{
  if (why != nullptr)
    brep_decline(why);
  if ((brep_strict() || brep_no_fallback()) && t_lenient == 0)
  {
    // (Also on stderr: a feature's FeatureScript may put its own message on the error.)
    if (brep_no_fallback())
      std::fprintf(stderr, "noshape's Boolean declined: %s\n", rust_last_decline().c_str());
    fail_unsupported("noshape's Boolean declined: " + rust_last_decline());
  }
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

// An ellipse or circle as a cubic B-spline through 129 of its points at its own parameters over one turn from
// `start` (within ~1e-11 of it; its parameters the conic's), else null: the curve swept into an extruded face.
occ::handle<Geom_BSplineCurve> conic_as_bspline(const occ::handle<Geom_Curve>& c, double start)
{
  if (occ::down_cast<Geom_Ellipse>(c).IsNull() && occ::down_cast<Geom_Circle>(c).IsNull())
    return occ::handle<Geom_BSplineCurve>();
  const int                                n   = 128;
  occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, n + 1);
  occ::handle<NCollection_HArray1<double>> prm = new NCollection_HArray1<double>(1, n + 1);
  for (int k = 0; k <= n; ++k)
  {
    const double t = start + 2 * M_PI * k / n;
    pts->SetValue(k + 1, c->Value(t));
    prm->SetValue(k + 1, t);
  }
  GeomAPI_Interpolate fit(pts, prm, false, 1e-12);
  fit.Perform();
  return fit.IsDone() ? fit.Curve() : occ::handle<Geom_BSplineCurve>();
}

// A hyperbola's or parabola's part from `f` to `l` as a cubic B-spline through 129 of its points at its own
// parameters (its parameters the conic's), else null where it misses the conic by over 1e-9 m between them: an
// input's edge on one (a plane's section of a cone), the curve an extruded face sweeps (Shell's slabs of those).
occ::handle<Geom_BSplineCurve> conic_part_as_bspline(const occ::handle<Geom_Curve>& c, double f, double l)
{
  if (occ::down_cast<Geom_Hyperbola>(c).IsNull() && occ::down_cast<Geom_Parabola>(c).IsNull())
    return occ::handle<Geom_BSplineCurve>();
  const int                                n   = 128;
  occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, n + 1);
  occ::handle<NCollection_HArray1<double>> prm = new NCollection_HArray1<double>(1, n + 1);
  for (int k = 0; k <= n; ++k)
  {
    const double t = f + (l - f) * k / n;
    pts->SetValue(k + 1, c->Value(t));
    prm->SetValue(k + 1, t);
  }
  GeomAPI_Interpolate fit(pts, prm, false, 1e-12);
  fit.Perform();
  if (!fit.IsDone())
    return occ::handle<Geom_BSplineCurve>();
  const occ::handle<Geom_BSplineCurve> b = fit.Curve();
  for (int k = 0; k < n; ++k)
  {
    const double t = f + (l - f) * (k + 0.5) / n;
    if (b->Value(t).Distance(c->Value(t)) > 1e-9)
      return occ::handle<Geom_BSplineCurve>();
  }
  return b;
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

// Offset surface `off` (a B-spline's) as a B-spline within 1e-7 of it over face `f`'s parameter range (grown by a
// thousandth, within the surface's), on the same parameters; null where OCCT's approximation doesn't get there.
static occ::handle<Geom_BSplineSurface> offset_as_bspline(const occ::handle<Geom_OffsetSurface>& off, const TopoDS_Face& f)
{
  double u0, u1, v0, v1, a0, a1, b0, b1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  off->Bounds(a0, a1, b0, b1);
  const double du = 1e-3 * (u1 - u0), dv = 1e-3 * (v1 - v0);
  try
  {
    const occ::handle<Geom_RectangularTrimmedSurface> part = new Geom_RectangularTrimmedSurface(off, std::max(a0, u0 - du), std::min(a1, u1 + du), std::max(b0, v0 - dv), std::min(b1, v1 + dv));
    GeomConvert_ApproxSurface fit(part, 1e-7, GeomAbs_C1, GeomAbs_C1, 9, 9, 200, 0);
    if (fit.IsDone() && fit.HasResult() && fit.MaxError() <= 1e-7)
      return fit.Surface();
  }
  catch (const Standard_Failure&)
  {
  }
  return occ::handle<Geom_BSplineSurface>();
}

// `s` as a flat body (its maps' indices are the body's), or why not.
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
    b.vertex_tol.push_back(BRep_Tool::Tolerance(v));
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
      // (One per edge, as `curves`: an ellipse after an apex read another edge's minor radius, 0 there: "degenerate".)
      b.ellipse_minor.push_back(0);
      b.edges.push_back(static_cast<uint32_t>(vm.FindIndex(v1) - 1));
      b.edges.push_back(static_cast<uint32_t>(vm.FindIndex(v1) - 1));
      b.edge_tol.push_back(BRep_Tool::Tolerance(e));
      continue;
    }
    TopLoc_Location               loc;
    double                        f = 0, l = 0;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, loc, f, l);
    if (c.IsNull())
      return "an edge without a curve";
    const occ::handle<Geom_Curve>        bc   = basis_of(c);
    const occ::handle<Geom_Circle>       circ = occ::down_cast<Geom_Circle>(bc);
    // (A Bézier curve (Shell's slabs of B-spline faces have them): as the B-spline it is, the same parameters.)
    const occ::handle<Geom_BezierCurve>  bzc  = occ::down_cast<Geom_BezierCurve>(bc);
    // (A hyperbola or parabola: a B-spline of it, its parameters.)
    occ::handle<Geom_BSplineCurve> bsc = bzc.IsNull() ? occ::down_cast<Geom_BSplineCurve>(bc) : GeomConvert::CurveToBSplineCurve(bzc);
    if (bsc.IsNull())
      bsc = conic_part_as_bspline(bc, f, l);
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
    b.edge_tol.push_back(BRep_Tool::Tolerance(e));
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
    surf = as_plane(surf);
    // (An offset surface (Shell's slabs): the plane, cylinder, cone, ... OCCT keeps as its equivalent; else a
    // B-spline within 1e-7 of it over the face's parameters (the same parameters, so its curves on it stay).)
    if (const occ::handle<Geom_OffsetSurface> off = occ::down_cast<Geom_OffsetSurface>(surf))
    {
      if (const occ::handle<Geom_Surface> eq = off->Surface(); !eq.IsNull())
        surf = eq;
      else if (const occ::handle<Geom_BSplineSurface> fit = offset_as_bspline(off, f); !fit.IsNull())
        surf = fit;
    }
    const occ::handle<Geom_Plane>         pl   = occ::down_cast<Geom_Plane>(surf);
    const occ::handle<Geom_CylindricalSurface> cyl = occ::down_cast<Geom_CylindricalSurface>(surf);
    const occ::handle<Geom_ConicalSurface>     con = occ::down_cast<Geom_ConicalSurface>(surf);
    const occ::handle<Geom_BezierSurface>      bzs = occ::down_cast<Geom_BezierSurface>(surf);
    const occ::handle<Geom_BSplineSurface>     bss = bzs.IsNull() ? occ::down_cast<Geom_BSplineSurface>(surf) : GeomConvert::SurfaceToBSplineSurface(bzs);
    const occ::handle<Geom_ToroidalSurface>    tor = occ::down_cast<Geom_ToroidalSurface>(surf);
    const occ::handle<Geom_SphericalSurface>   sph = occ::down_cast<Geom_SphericalSurface>(surf);
    const occ::handle<Geom_SurfaceOfLinearExtrusion> ext = occ::down_cast<Geom_SurfaceOfLinearExtrusion>(surf);
    const occ::handle<Geom_SurfaceOfRevolution>      rvl = occ::down_cast<Geom_SurfaceOfRevolution>(surf);
    // (A sweep of a B-spline only; an extrusion's ellipse or circle as one.)
    occ::handle<Geom_BSplineCurve> swept = occ::down_cast<Geom_BSplineCurve>(basis_of(!ext.IsNull() ? ext->BasisCurve() : !rvl.IsNull() ? rvl->BasisCurve() : occ::handle<Geom_Curve>()));
    // (The turn the face's parameters are on: a face from pi to 3 pi starts its turn at pi. A revolution's profile
    // is along v: cf-snap-hook's revolved arcs, tori kept as revolutions.)
    if (swept.IsNull() && (!ext.IsNull() || !rvl.IsNull()))
    {
      double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      const double a0 = ext.IsNull() ? v0 : u0, a1 = ext.IsNull() ? v1 : u1;
      const bool   first = a0 > -1e-9 && a1 < 2 * M_PI + 1e-9;
      swept = conic_as_bspline(basis_of(!ext.IsNull() ? ext->BasisCurve() : rvl->BasisCurve()), first || a1 - a0 > 2 * M_PI + 1e-9 ? 0.0 : a0);
    }
    // (A hyperbola's or parabola's over the face's range of it, a little more either side.)
    if (swept.IsNull() && !ext.IsNull())
    {
      double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      const double m = 0.01 * (u1 - u0);
      swept = conic_part_as_bspline(basis_of(ext->BasisCurve()), u0 - m, u1 + m);
    }
    const bool free = !bss.IsNull() || !tor.IsNull() || !sph.IsNull() || ((!ext.IsNull() || !rvl.IsNull()) && !swept.IsNull());
    if (pl.IsNull() && cyl.IsNull() && con.IsNull() && !free)
    {
      const GeomAbs_SurfaceType t = GeomAdaptor_Surface(surf).GetType();
      return t == GeomAbs_Torus ? "a torus face" : t == GeomAbs_Sphere ? "a sphere face" : t == GeomAbs_OffsetSurface ? "an offset face" : t == GeomAbs_SurfaceOfRevolution ? "a face of revolution" : t == GeomAbs_SurfaceOfExtrusion ? "an extruded face" : "a curved face";
    }
    if (f.Orientation() != TopAbs_FORWARD && f.Orientation() != TopAbs_REVERSED)
      return "an internal face";
    max_tol   = std::max(max_tol, BRep_Tool::Tolerance(f));
    b.face_tol.push_back(BRep_Tool::Tolerance(f));
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
      // (A vertex inside it (INTERNAL, on no wire: Shell's slabs of a part with one) left out: OCCT's result keeps
      // none in the faces it splits either (shfall a7bdd15b); any difference shows in the result's counts.)
      if (wi.Value().ShapeType() == TopAbs_VERTEX)
        continue;
      if (wi.Value().ShapeType() != TopAbs_WIRE)
        return "a vertex in a face";
      for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
      {
        const TopoDS_Shape& e = ei.Value();
        const int           k = em.FindIndex(e);
        if (k <= 0)
          return "an internal edge";
        // An edge inside the face (INTERNAL): noshape's Boolean keeps it inside the face's piece it lies in.
        if (e.Orientation() != TopAbs_FORWARD && e.Orientation() != TopAbs_REVERSED)
        {
          if (e.Orientation() == TopAbs_EXTERNAL)
            return "an external edge";
          b.face_inner_edges.push_back(static_cast<uint32_t>(i - 1));
          b.face_inner_edges.push_back(static_cast<uint32_t>(k - 1));
          continue;
        }
        // A seam (twice in a cylinder's or cone's face) once each way.
        if (seen[k] == i && ((cyl.IsNull() && con.IsNull() && !free) || BRep_Tool::IsClosed(TopoDS::Edge(e), f) == false))
          return "an edge twice in a face";
        seen[k] = i;
        b.uses.push_back(static_cast<uint32_t>(2 * (k - 1) + (e.Orientation() == TopAbs_REVERSED ? 1 : 0)));
      }
      // (A wire of edges inside it alone: no loop.)
      if (b.loop_uses.back() != static_cast<uint32_t>(b.uses.size()))
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
  // (Called noexcept from Rust: an exception would terminate the process.)
  try
  {
    double      tol = 0;
    const char* why = to_body(s, out, tol);
    return rust::String(why != nullptr ? why : "");
  }
  catch (...)
  {
    return rust::String("OCCT threw converting it");
  }
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

// Shapes freed on a thread of their own (one, for the process's life: its queue is never destroyed).
void free_later(std::vector<TopoDS_Shape>&& shapes)
{
  struct Queue
  {
    std::mutex                             m;
    std::condition_variable                cv;
    std::deque<std::vector<TopoDS_Shape>> items;
  };
  static Queue* const q = [] {
    Queue* made = new Queue();
    std::thread([made] {
      for (;;)
      {
        std::vector<TopoDS_Shape> next;
        {
          std::unique_lock<std::mutex> lock(made->m);
          made->cv.wait(lock, [made] { return !made->items.empty(); });
          next = std::move(made->items.front());
          made->items.pop_front();
        }
        next.clear();
      }
    }).detach();
    return made;
  }();
  {
    const std::lock_guard<std::mutex> lock(q->m);
    q->items.push_back(std::move(shapes));
  }
  q->cv.notify_one();
}

// Set once the process exits: Windows runs the exiting thread's thread_local destructors after the atexit handlers,
// inside its shutdown, where no thread can be started (the freer's first start threw out of ~Built: the CLI's exit
// code 0xC0000409 after a user's E2 printed its result).
std::atomic<bool> g_exiting{false};

void watch_exit()
{
  static std::once_flag once;
  std::call_once(once, [] { std::atexit([] { g_exiting.store(true); }); });
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
  Built() { watch_exit(); }
  Built(const Built&)            = delete;
  Built& operator=(const Built&) = delete;
  Built(Built&&)                 = default;
  Built& operator=(Built&&)      = default;
  // (The thread's exit frees them otherwise, and a regeneration waits for that: the cancelled 45 / 45 knurl's
  // FeatureScript thread spent 60-180 ms freeing its last Boolean's faces, CANCEL-LATENCY.)
  ~Built()
  {
    std::vector<TopoDS_Shape> all;
    for (std::vector<TopoDS_Shape>* x : {&v, &e, &f, &s})
    {
      all.insert(all.end(), std::make_move_iterator(x->begin()), std::make_move_iterator(x->end()));
      x->clear();
    }
    if (all.empty())
      return;
    // (At the exit, left to it: nothing to gain freeing them, and no thread to free them on.)
    if (g_exiting.load())
    {
      static_cast<void>(new std::vector<TopoDS_Shape>(std::move(all)));
      return;
    }
    try
    {
      free_later(std::move(all));
    }
    catch (...)
    {
      // (No freer thread: freed here as `all` goes.)
    }
  }
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
  // OCCT's ellipse round the other way (its axis against ours): the edge runs from its end to its start.
  bool reversed = false;
  // Made straight between its vertices instead (a few tolerances long): its curves on the faces projected.
  bool straight = false;
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
  // OCCT's sections made ahead (brep_occt_ahead) or when asked for, by pair (the earlier argument's face first).
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

// A circle about a surface of revolution's own axis lying on it (a round's edge on a boss's revolved face): its curve
// on face `f`, the line along u at the circle's v (OCCT's projection of it took 30 ms each); null otherwise.
static occ::handle<Geom2d_Curve> parallel_pcurve(const TopoDS_Edge& e, const TopoDS_Face& f, double tol)
{
  double                  a = 0, z = 0;
  TopLoc_Location         el, fl;
  occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, el, a, z);
  while (const occ::handle<Geom_TrimmedCurve> tc = occ::down_cast<Geom_TrimmedCurve>(c))
    c = tc->BasisCurve();
  const occ::handle<Geom_Circle>  ci = occ::down_cast<Geom_Circle>(c);
  const occ::handle<Geom_Surface> s  = BRep_Tool::Surface(f, fl);
  occ::handle<Geom_Surface>       base = s;
  while (const occ::handle<Geom_RectangularTrimmedSurface> tr = occ::down_cast<Geom_RectangularTrimmedSurface>(base))
    base = tr->BasisSurface();
  const occ::handle<Geom_SurfaceOfRevolution> rev = occ::down_cast<Geom_SurfaceOfRevolution>(base);
  if (ci.IsNull() || rev.IsNull())
    return occ::handle<Geom2d_Curve>();
  // (The circle in the surface's own frame; its angle and u turn the same way round or opposite ways.)
  const gp_Circ k  = ci->Circ().Transformed((fl.Inverted() * el).Transformation());
  const gp_Ax1  ax = rev->Axis();
  if (!k.Axis().IsParallel(ax, 1e-9) || gp_Lin(ax).Distance(k.Location()) > tol)
    return occ::handle<Geom2d_Curve>();
  const double   sgn = k.Axis().Direction().Dot(ax.Direction()) > 0 ? 1.0 : -1.0;
  const gp_Pnt2d uv  = ShapeAnalysis_Surface(s).ValueOfUV(ElCLib::Value(a, k), tol);
  double         umin = 0, umax = 0, vmin = 0, vmax = 0;
  BRepTools::UVBounds(f, umin, umax, vmin, vmax);
  // (Its angles within the face's: a whole turn's from one side of the seam to the other.)
  const double eps = 1e-9 * (1 + std::abs(umin) + std::abs(umax));
  for (const double turn : {0.0, 2 * M_PI, -2 * M_PI})
  {
    const double u0 = uv.X() + turn, u1 = u0 + sgn * (z - a);
    if (std::min(u0, u1) < umin - eps || std::max(u0, u1) > umax + eps)
      continue;
    bool on = true;
    for (int j = 0; j <= 8 && on; ++j)
    {
      const double t = a + (z - a) * j / 8;
      on = s->Value(u0 + sgn * (t - a), uv.Y()).Distance(ElCLib::Value(t, k)) <= tol;
    }
    if (!on)
      return occ::handle<Geom2d_Curve>();
    return new Geom2d_Line(gp_Pnt2d(u0 - sgn * a, uv.Y()), gp_Dir2d(sgn, 0));
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
    // (OCCT's round the other way (its axis against ours): the same curve, its part from our end to our start.)
    const bool back = g.Axis().Direction().Dot(n) < 0;
    if (!same(g.Location(), back ? g.Axis().Direction().Reversed() : g.Axis().Direction(), g.MajorRadius()))
      continue;
    const gp_Pnt& e0 = back ? p1 : p0;
    const gp_Pnt& e1 = back ? p0 : p1;
    const double first = c->FirstParameter(), last = c->LastParameter();
    // Parameters within the tolerance along it (its speed is at least its minor radius): a start just short of a
    // turn past OCCT's first is at its first (a closed one starting where OCCT's does), ends just outside OCCT's
    // range at its ends.
    const double eps = std::max(1e-9, tol / g.MinorRadius());
    double       t0  = first + turns(ElCLib::Parameter(g, e0) - first);
    if (t0 > 2 * M_PI + first - eps || (t0 > last + eps && t0 - 2 * M_PI >= first - eps))
      t0 -= 2 * M_PI;
    double d = turns(ElCLib::Parameter(g, e1) - t0);
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
    out.reversed  = back;
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
  // (Made straight: the section's curves on the faces go by its parameters, not the line's.)
  if (x.straight)
  {
    BOPTools_AlgoTools2D::BuildPCurveForEdgeOnFace(e, TopoDS::Face(src.Oriented(TopAbs_FORWARD)));
    double a = 0, z = 0;
    return BRep_Tool::CurveOnSurface(e, s, loc, a, z).IsNull() ? "a short section's curve on a face" : nullptr;
  }
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
  // (On a third face, a piece a few tolerances long merged with that face's (three knurl faces' curves into one
  // vertex): its curve there projected, as for a new edge.)
  if (!x.curve.IsNull() && BRep_Tool::Pnt(TopExp::FirstVertex(e)).Distance(BRep_Tool::Pnt(TopExp::LastVertex(e))) <= 100 * BRep_Tool::Tolerance(e) + 1e-5)
  {
    try
    {
      BOPTools_AlgoTools2D::BuildPCurveForEdgeOnFace(e, TopoDS::Face(src.Oriented(TopAbs_FORWARD)));
      double a, z;
      if (!BRep_Tool::CurveOnSurface(e, s, loc, a, z).IsNull())
        return nullptr;
    }
    catch (const Standard_Failure&)
    {
    }
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
      bb.MakeVertex(vs[i], point_of(b, static_cast<uint32_t>(i)), i < b.vertex_tol.size() ? std::max(tol, b.vertex_tol[i]) : tol);
  }
  // Edges: the input's own, a piece on its curve, or new on a line, circle or ellipse.
  std::vector<TopoDS_Edge> es(ne);
  // (Edges made on OCCT's curve round the other way: from our end to our start, every use the other way.)
  std::vector<char> flipped(ne, 0);
  std::vector<EllEdge>     ells(ne);
  Sections                 sections;
  // (Each loop checks the cancel every 256: the 45 / 45 knurl's result took over a second to build after it.)
  for (size_t i = 0; i < ne; ++i)
  {
    if ((i & 0xFF) == 0xFF)
      fail_if_cancelled();
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
        if (const occ::handle<Geom_Hyperbola> hy = occ::down_cast<Geom_Hyperbola>(cn))
          return ElCLib::Parameter(hy->Hypr(), p);
        if (const occ::handle<Geom_Parabola> pa = occ::down_cast<Geom_Parabola>(cn))
          return ElCLib::Parameter(pa->Parab(), p);
        return cv[2];
      };
      // (A hyperbola's or parabola's parameter goes on, not round.)
      const bool open = !occ::down_cast<Geom_Hyperbola>(cn).IsNull() || !occ::down_cast<Geom_Parabola>(cn).IsNull();
      t0       = open ? par(p0) : cv[2] + std::remainder(par(p0) - cv[2], 2 * M_PI);
      t1       = open ? par(p1) : cv[3] + std::remainder(par(p1) - cv[3], 2 * M_PI);
      if (v0 == v1 && !open)
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
      t0               = at0 ? *at0 : f + std::fmod(std::fmod(ElCLib::Parameter(g, p0) - f, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
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
      // (At its edge's start: from there (its angle a rounding below the start went a turn on).)
      t0              = at0 ? *at0 : f + std::fmod(std::fmod(ElCLib::Parameter(g, p0) - f, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
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
    // (A piece a few tolerances long whose ends' parameters on its curve came out the wrong way (a knurl section's
    // 0.3 um piece by three faces' vertices): straight between its vertices, as the Boolean put it in the faces.)
    if (!(t0 < t1) && p0.Distance(p1) > 1e-12 && p0.Distance(p1) <= 100 * tol)
    {
      c                = new Geom_Line(p0, gp_Dir(gp_Vec(p0, p1)));
      loc              = TopLoc_Location();
      t0               = 0;
      t1               = p0.Distance(p1);
      ells[i].straight = true;
    }
    if (!(t0 < t1))
      return "an edge against its curve";
    TopoDS_Edge e;
    bb.MakeEdge(e, c, loc, tol_e);
    flipped[i] = !ells[i].curve.IsNull() && ells[i].reversed && c == ells[i].curve;
    bb.Add(e, vs[flipped[i] ? v1 : v0].Oriented(TopAbs_FORWARD));
    bb.Add(e, vs[flipped[i] ? v0 : v1].Oriented(TopAbs_REVERSED));
    bb.Range(e, t0, t1);
    es[i] = e;
  }
  std::vector<TopoDS_Face> fs(nf);
  for (size_t i = 0; i < nf; ++i)
  {
    if ((i & 0xFF) == 0xFF)
      fail_if_cancelled();
    const uint32_t kind = r.face_origin[3 * i];
    const TopoDS_Face& src = TopoDS::Face(input(srcs, r.face_origin[3 * i + 1], TopAbs_FACE, r.face_origin[3 * i + 2]));
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> s0  = BRep_Tool::Surface(src, loc);
    // On its input face's surface: the input's edges have their curves there already. (Never change an input
    // shape: other threads may be reading it.)
    const occ::handle<Geom_Surface>& s = s0;
    // (A B-spline that is a plane, read as one (to_body): new edges' curves on it projected, as OCCT keeps its
    // surface.)
    const bool as_flat = as_plane(s0) != s0;
    const occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(s);
    const occ::handle<Geom_ConicalSurface>     cn = occ::down_cast<Geom_ConicalSurface>(s);
    const bool                                 round = !cs.IsNull() || !cn.IsNull();
    const gp_Ax3 frame = !cs.IsNull() ? cs->Position().Transformed(loc.Transformation()) : !cn.IsNull() ? cn->Position().Transformed(loc.Transformation()) : gp_Ax3();
    // The face's way round against its surface's normal (to_body: a left-handed cylinder's or cone's is
    // inward), by its surface's own kind under a trim as to_body reads it (a sheet metal flange's bend on a
    // trimmed left-handed cylinder came out the other way round: RUST-SM-CUT-VOLUME).
    occ::handle<Geom_Surface> base = s;
    while (const occ::handle<Geom_RectangularTrimmedSurface> tr = occ::down_cast<Geom_RectangularTrimmedSurface>(base))
      base = tr->BasisSurface();
    const occ::handle<Geom_CylindricalSurface> bcs = occ::down_cast<Geom_CylindricalSurface>(base);
    const occ::handle<Geom_ConicalSurface>     bcn = occ::down_cast<Geom_ConicalSurface>(base);
    const bool left = !bcs.IsNull() ? !bcs->Position().Transformed(loc.Transformation()).Direct() : !bcn.IsNull() && !bcn->Position().Transformed(loc.Transformation()).Direct();
    const bool rev  = b.reversed[i] != left;
    const bool inner_loop = i < r.face_inner.size() && r.face_inner[i] != 0;
    if (kind == 0)
    {
      if (inner_loop)
        return "an edge inside an unchanged face";
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
        {
          if (!as_flat)
            return "a new edge on a free-form face";
          BOPTools_AlgoTools2D::BuildPCurveForEdgeOnFace(es[k], TopoDS::Face(src.Oriented(TopAbs_FORWARD)));
          continue;
        }
        const TopoDS_Edge se = TopoDS::Edge(input(srcs, r.edge_origin[3 * k + 1], TopAbs_EDGE, r.edge_origin[3 * k + 2]).Oriented(TopAbs_FORWARD));
        const TopoDS_Face sf = TopoDS::Face(src.Oriented(TopAbs_FORWARD));
        double            a = 0, z = 0;
        occ::handle<Geom2d_Curve> pf = BRep_Tool::CurveOnSurface(se, sf, a, z);
        // (Another argument's edge coincident with one of this face's: that one's curve on it.)
        if (pf.IsNull() && !BRep_Tool::IsClosed(se, sf))
          pf = coincident_pcurve(es[k], sf, 1e-7 + BRep_Tool::Tolerance(es[k]));
        if (pf.IsNull() && !BRep_Tool::IsClosed(se, sf))
          pf = parallel_pcurve(es[k], sf, 1e-7 + BRep_Tool::Tolerance(es[k]));
        // (An edge without its curve on it (another argument's edge inside or along it, an INTERNAL one): projected,
        // as OCCT's builder makes it.)
        if (pf.IsNull())
        {
          BOPTools_AlgoTools2D::BuildPCurveForEdgeOnFace(es[k], sf);
          continue;
        }
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
      // (The edges inside it: INTERNAL, as OCCT's builder leaves an edge ending inside a face.)
      if (inner_loop && l + 1 == b.face_loops[i + 1])
      {
        for (uint32_t k = u0; k < u1; ++k)
          bb.Add(w, es[b.uses[k] / 2].Oriented(TopAbs_INTERNAL));
        bb.Add(f, w);
        continue;
      }
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
          starts.push_back(TopExp::FirstVertex(TopoDS::Edge(e.Oriented((b.uses[k] % 2 == 1) != (flipped[b.uses[k] / 2] != 0) ? TopAbs_REVERSED : TopAbs_FORWARD)), true));
          seen.push_back(b.uses[k] / 2);
        }
        std::sort(seen.begin(), seen.end());
        const bool twice = std::adjacent_find(seen.begin(), seen.end()) != seen.end();
        for (size_t x = 0; x < starts.size() && !twice && !pinched; ++x)
          for (size_t y = x + 1; y < starts.size() && !pinched; ++y)
            pinched = starts[x].IsSame(starts[y]);
      }
      std::vector<TopoDS_Edge> seq;
      for (uint32_t k = 0; k < u1 - u0; ++k)
      {
        const uint32_t u    = b.uses[mirrored || pinched ? u1 - 1 - k : u0 + k];
        const bool     back = ((u % 2 == 1) != rev) != (flipped[u / 2] != 0);
        seq.push_back(TopoDS::Edge(es[u / 2].Oriented(back ? TopAbs_REVERSED : TopAbs_FORWARD)));
      }
      // A loop through one vertex twice, no edge twice (a hole touching the boundary at a point): a wire each side
      // of that vertex, as OCCT's builder makes them (one wire through it twice is a bad orientation to BRepCheck,
      // and OCCT's operations on such a result crashed: fuzz seed 1144).
      std::vector<std::vector<TopoDS_Edge>> wires;
      {
        std::vector<uint32_t> ids;
        for (uint32_t k = u0; k < u1; ++k)
          ids.push_back(b.uses[k] / 2);
        std::sort(ids.begin(), ids.end());
        const bool twice = std::adjacent_find(ids.begin(), ids.end()) != ids.end();
        std::vector<TopoDS_Edge>   st;
        std::vector<TopoDS_Vertex> sv;
        for (const TopoDS_Edge& e : seq)
        {
          const TopoDS_Vertex v = TopExp::FirstVertex(e, true);
          if (!twice && seq.size() <= 256 && !v.IsSame(TopExp::LastVertex(e, true)))
            for (size_t j = 0; j < sv.size(); ++j)
              if (sv[j].IsSame(v))
              {
                wires.emplace_back(st.begin() + j, st.end());
                st.resize(j);
                sv.resize(j);
                break;
              }
          st.push_back(e);
          // (A closed edge (a pole's degenerate one, a whole circle) starts and ends at one vertex: not a pinch.)
          sv.push_back(v.IsSame(TopExp::LastVertex(e, true)) ? TopoDS_Vertex() : v);
        }
        wires.push_back(st);
      }
      for (const std::vector<TopoDS_Edge>& es_w : wires)
      {
        if (es_w.empty())
          continue;
        TopoDS_Wire ww;
        bb.MakeWire(ww);
        for (const TopoDS_Edge& e : es_w)
          bb.Add(ww, e);
        if (round)
          if (const char* why = apex_pcurves(ww, s, loc, bb))
            return why;
        ww.Closed(true);
        bb.Add(f, ww);
      }
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
  // A new edge on a made plane whose ends lie off it by more than its tolerance (a tolerant vertex the section runs
  // through, off a slightly tilted plane): declined. OCCT runs its section on the planes' exact intersection and
  // its result there differs (fuzz seed 1144's Shell common b997c4c3: ours, its edge's tolerance raised to cover it,
  // was valid but 0.036 % in volume off, and a later sheet metal extrude crashed on it; on OCCT's result it doesn't).
  {
    ShapeIMap made;
    for (size_t k = 0; k < ne; ++k)
      if (r.edge_origin[3 * k] != 0)
        made.Add(es[k]);
    for (size_t i = 0; i < nf; ++i)
    {
      if (r.face_origin[3 * i] == 0)
        continue;
      TopLoc_Location               loc;
      const occ::handle<Geom_Plane> pl = occ::down_cast<Geom_Plane>(BRep_Tool::Surface(fs[i], loc));
      if (pl.IsNull())
        continue;
      const gp_Pln p = pl->Pln().Transformed(loc.Transformation());
      for (TopExp_Explorer x(fs[i], TopAbs_EDGE); x.More(); x.Next())
      {
        const TopoDS_Edge& e = TopoDS::Edge(x.Current());
        if (!made.Contains(e) || BRep_Tool::Degenerated(e))
          continue;
        TopoDS_Vertex a, z;
        TopExp::Vertices(e, a, z);
        for (const TopoDS_Vertex& v : {a, z})
          if (!v.IsNull() && p.Distance(BRep_Tool::Pnt(v)) > BRep_Tool::Tolerance(e))
            return "an edge off a plane past its tolerance";
      }
    }
  }
  // Made faces of cylinders and cones, all their curves set (a later face on the same surface may have set one
  // of an edge they share): one across its surface's seam angle (its angles over a multiple of 2π, though no
  // seam bounds it) declines, OCCT's unify having gone wrong on it after a mirror (halfmir2's half pocket: a
  // reversed face); so does one whose curves span more than a turn (an invalid face).
  for (size_t i = 0; i < nf; ++i)
  {
    if ((i & 0xFF) == 0xFF)
      fail_if_cancelled();
    if (r.face_origin[3 * i] == 0)
      continue;
    TopLoc_Location loc;
    const occ::handle<Geom_Surface> sf = BRep_Tool::Surface(fs[i], loc);
    // Every edge of a made face off a plane with its curve on it: OCCT projects a missing one when it meets it, and
    // its projection onto a cone's or sphere's pole crashed inside its threaded builder (CRASH-1092, a Shell's slab
    // Boolean after ours). Declined instead.
    if (occ::down_cast<Geom_Plane>(sf).IsNull())
      for (TopExp_Explorer x(fs[i], TopAbs_EDGE); x.More(); x.Next())
      {
        double f0 = 0, f1 = 0;
        if (BRep_Tool::CurveOnSurface(TopoDS::Edge(x.Current()), fs[i], f0, f1).IsNull())
          return "an edge without its curve on a curved face";
      }
    if (occ::down_cast<Geom_CylindricalSurface>(sf).IsNull() && occ::down_cast<Geom_ConicalSurface>(sf).IsNull())
      continue;
    double a = 0, z = 0, v1 = 0, v2 = 0;
    BRepTools::UVBounds(fs[i], a, z, v1, v2);
    // (An end a rounding past a whole turn, 1e-7 radians or the tolerance round the axis: not across it (a thread's
    // helical face going round between rulings 3e-7 rad apart).)
    const occ::handle<Geom_CylindricalSurface> scyl = occ::down_cast<Geom_CylindricalSurface>(sf);
    const double radius = !scyl.IsNull() ? scyl->Radius() : occ::down_cast<Geom_ConicalSurface>(sf)->RefRadius();
    const double round_off = radius > 1e-9 ? std::max(1e-7, tol / radius) : 1e-7;
    // (Within its input face's own angles across it: as that face was (a gear's flank face over u 3.4 .. 9.1).)
    if (z - a < 2 * M_PI - 1e-6 && std::floor((a + round_off) / (2 * M_PI)) != std::floor((z - round_off) / (2 * M_PI)))
    {
      double sa = 0, sz = 0, sv1 = 0, sv2 = 0;
      BRepTools::UVBounds(TopoDS::Face(input(srcs, r.face_origin[3 * i + 1], TopAbs_FACE, r.face_origin[3 * i + 2])), sa, sz, sv1, sv2);
      if (a < sa - round_off || z > sz + round_off)
        return "a piece across its surface's seam angle";
    }
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

static rust::Vec<uint32_t> occt_moved(rust::Slice<const uint32_t> ids, uint32_t a_arg, uint32_t a_face, uint32_t b_arg, uint32_t b_face, rust::Slice<const double> moved)
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

namespace
{
// A face on an extrusion of a B-spline: the profile, the direction, the profile's range and the distances along the
// direction the face spans; the face's own parameters at c(u) + d w: (u, at + w per), the two swapped where `swap`
// (a B-spline face straight along its u). A surface of linear extrusion's are (u, w).
struct Extruded
{
  occ::handle<Geom_BSplineCurve> c;
  gp_Dir                         d;
  double                         u0 = 0, u1 = 0, v0 = 0, v1 = 0;
  double                         at = 0, per = 1;
  bool                           swap = false;

  gp_Pnt2d uv(double u, double w) const
  {
    const double s = at + w * per;
    return swap ? gp_Pnt2d(s, u) : gp_Pnt2d(u, s);
  }
  bool plain() const { return !swap && at == 0 && per == 1; }
};

// The curve on an extrusion's face of its ruling at `u` from distance `w0` on, `sign` the way the 3D curve runs along
// the direction, the 3D curve's parameters `t0` to `t1`.
occ::handle<Geom2d_Curve> on_ruling(const Extruded& e, double u, double w0, double sign, double t0, double t1)
{
  if (e.plain())
    return new Geom2d_Line(gp_Pnt2d(u, w0), gp_Dir2d(0, sign));
  NCollection_Array1<gp_Pnt2d> poles(1, 2);
  poles(1) = e.uv(u, w0 + sign * t0);
  poles(2) = e.uv(u, w0 + sign * t1);
  NCollection_Array1<double> knots(1, 2);
  knots(1) = t0;
  knots(2) = t1;
  NCollection_Array1<int> mults(1, 2);
  mults.Init(2);
  return new Geom2d_BSplineCurve(poles, knots, mults, 1);
}

// The curve on an extrusion's face of its profile moved `w` along the direction (the profile's parameters).
occ::handle<Geom2d_Curve> on_profile(const Extruded& e, double w)
{
  return new Geom2d_Line(e.uv(0, w), e.swap ? gp_Dir2d(0, 1) : gp_Dir2d(1, 0));
}

// A B-spline face straight along one of its directions: each line of poles that way the first one moved by t_j D, the
// same t_j for all and the weights the same along it, t_j linear in that way's Greville points (the face's parameter
// there linear in the distance): an extrusion of the first line of poles across along D (an imported wall).
std::optional<Extruded> bspline_extruded(const TopoDS_Face& f, const occ::handle<Geom_BSplineSurface>& s)
{
  for (const bool along_u : {false, true})
  {
    const int ni = along_u ? s->NbVPoles() : s->NbUPoles(), nj = along_u ? s->NbUPoles() : s->NbVPoles();
    if (nj < 2 || ni < 2 || (along_u ? s->IsUPeriodic() || s->IsVPeriodic() : s->IsVPeriodic() || s->IsUPeriodic()))
      continue;
    auto P = [&](int i, int j) { return along_u ? s->Pole(j, i) : s->Pole(i, j); };
    auto W = [&](int i, int j) { return along_u ? s->Weight(j, i) : s->Weight(i, j); };
    const gp_Vec D(P(1, 1), P(1, nj));
    double       size = 0;
    for (int i = 1; i <= ni; ++i)
      size = std::max(size, gp_Vec(P(1, 1), P(i, 1)).Magnitude());
    size = std::max(size, D.Magnitude());
    const double tol = 1e-12 * std::max(size, 1.0);
    if (D.Magnitude() <= 1e3 * tol)
      continue;
    std::vector<double> t(static_cast<size_t>(nj) + 1);
    bool                ok = true;
    for (int j = 1; j <= nj && ok; ++j)
    {
      t[(size_t)j] = gp_Vec(P(1, 1), P(1, j)).Dot(D) / D.SquareMagnitude();
      for (int i = 1; i <= ni && ok; ++i)
        ok = (gp_Vec(P(i, 1), P(i, j)) - D * t[(size_t)j]).Magnitude() <= tol && (!s->IsURational() && !s->IsVRational() || std::abs(W(i, j) - W(i, 1)) <= 1e-12 * W(i, 1));
    }
    if (!ok)
      continue;
    // Greville points of the straight way's knots: t_j = a + b xi_j.
    const int                 q = along_u ? s->UDegree() : s->VDegree();
    NCollection_Array1<double> flat(1, nj + q + 1);
    along_u ? s->UKnotSequence(flat) : s->VKnotSequence(flat);
    std::vector<double> xi(static_cast<size_t>(nj) + 1);
    for (int j = 1; j <= nj; ++j)
    {
      double sum = 0;
      for (int k = 1; k <= q; ++k)
        sum += flat(j + k);
      xi[(size_t)j] = sum / q;
    }
    if (std::abs(xi[(size_t)nj] - xi[1]) < 1e-15)
      continue;
    const double b = (t[(size_t)nj] - t[1]) / (xi[(size_t)nj] - xi[1]), a = t[1] - b * xi[1];
    for (int j = 1; j <= nj && ok; ++j)
      ok = std::abs(t[(size_t)j] - (a + b * xi[(size_t)j])) <= 1e-12;
    if (!ok || std::abs(b) < 1e-15)
      continue;
    // The profile: the first line of poles across, with that way's knots.
    NCollection_Array1<gp_Pnt> poles(1, ni);
    NCollection_Array1<double> weights(1, ni);
    for (int i = 1; i <= ni; ++i)
    {
      poles(i)   = P(i, 1);
      weights(i) = W(i, 1);
    }
    const bool rational = along_u ? s->IsVRational() : s->IsURational();
    occ::handle<Geom_BSplineCurve> c = rational ? new Geom_BSplineCurve(poles, weights, along_u ? s->VKnots() : s->UKnots(), along_u ? s->VMultiplicities() : s->UMultiplicities(), along_u ? s->VDegree() : s->UDegree())
                                                : new Geom_BSplineCurve(poles, along_u ? s->VKnots() : s->UKnots(), along_u ? s->VMultiplicities() : s->UMultiplicities(), along_u ? s->VDegree() : s->UDegree());
    // S = c(across) + (a + b straight) D: the distance w = |D| (a + b straight).
    Extruded x{c, gp_Dir(D)};
    x.swap = along_u;
    x.per  = 1 / (b * D.Magnitude());
    x.at   = -a / b;
    double fu0, fu1, fv0, fv1;
    BRepTools::UVBounds(f, fu0, fu1, fv0, fv1);
    const double c0 = along_u ? fv0 : fu0, c1 = along_u ? fv1 : fu1, s0 = along_u ? fu0 : fv0, s1 = along_u ? fu1 : fv1;
    x.u0 = std::max(c0, c->FirstParameter());
    x.u1 = std::min(c1, c->LastParameter());
    const double w0 = (s0 - x.at) / x.per, w1 = (s1 - x.at) / x.per;
    x.v0 = std::min(w0, w1);
    x.v1 = std::max(w0, w1);
    if (!(x.u0 < x.u1) || !(x.v0 < x.v1))
      return std::nullopt;
    return x;
  }
  return std::nullopt;
}

std::optional<Extruded> extruded(const TopoDS_Face& f)
{
  const occ::handle<Geom_Surface> sf = BRep_Tool::Surface(f);
  if (const occ::handle<Geom_BSplineSurface> bs = occ::down_cast<Geom_BSplineSurface>(sf))
    return bspline_extruded(f, bs);
  const occ::handle<Geom_SurfaceOfLinearExtrusion> e = occ::down_cast<Geom_SurfaceOfLinearExtrusion>(sf);
  if (e.IsNull())
    return std::nullopt;
  const occ::handle<Geom_BSplineCurve> c = occ::down_cast<Geom_BSplineCurve>(basis_of(e->BasisCurve()));
  if (c.IsNull() || c->IsPeriodic())
    return std::nullopt;
  Extruded x{c, e->Direction()};
  BRepTools::UVBounds(f, x.u0, x.u1, x.v0, x.v1);
  x.u0 = std::max(x.u0, c->FirstParameter());
  x.u1 = std::min(x.u1, c->LastParameter());
  if (!(x.u0 < x.u1) || !(x.v0 < x.v1))
    return std::nullopt;
  return x;
}

// `c` seen along `d` (its poles' coordinates on the frame across it): the same B-spline in that plane.
occ::handle<Geom2d_BSplineCurve> flat(const occ::handle<Geom_BSplineCurve>& c, const gp_Ax2& frame)
{
  NCollection_Array1<gp_Pnt2d> poles(1, c->NbPoles());
  for (int i = 1; i <= c->NbPoles(); ++i)
  {
    const gp_Vec w(frame.Location(), c->Pole(i));
    poles(i) = gp_Pnt2d(w.Dot(gp_Vec(frame.XDirection())), w.Dot(gp_Vec(frame.YDirection())));
  }
  if (c->IsRational())
    return new Geom2d_BSplineCurve(poles, *c->Weights(), c->Knots(), c->Multiplicities(), c->Degree());
  return new Geom2d_BSplineCurve(poles, c->Knots(), c->Multiplicities(), c->Degree());
}

// Where two curves in a plane cross (the parameters on each), none where they overlap; false where they do.
bool crossings2d(const occ::handle<Geom2d_Curve>& a, const occ::handle<Geom2d_Curve>& b, std::vector<std::pair<double, double>>& out)
{
  Geom2dAPI_InterCurveCurve x(a, b, 1e-9);
  if (x.NbSegments() > 0)
    return false;
  for (int i = 1; i <= x.NbPoints(); ++i)
  {
    const IntRes2d_IntersectionPoint& p = x.Intersector().Point(i);
    out.emplace_back(p.ParamOnFirst(), p.ParamOnSecond());
  }
  return true;
}

// A ruling of extrusion `e` at profile parameter `u` over its face's heights, its curve on the extrusion.
IntTools_Curve ruling(const Extruded& e, double u, const occ::handle<Geom2d_Curve>& on_other, bool extrusion_first)
{
  const occ::handle<Geom_Curve>   line = new Geom_TrimmedCurve(new Geom_Line(e.c->Value(u), e.d), e.v0, e.v1);
  const occ::handle<Geom2d_Curve> on_e = on_ruling(e, u, 0, 1, e.v0, e.v1);
  IntTools_Curve                  ic(line, extrusion_first ? on_e : on_other, extrusion_first ? on_other : on_e, Precision::Confusion());
  return ic;
}
} // namespace

// A section given point by point (`at`: its parameter -> the point and its parameters on the two faces) from `t0` to
// `t1`: cubics through n + 1 of them at their own parameters, its curves on the faces likewise (angles carried on by
// turns), n doubled from `n0` until they are within 1e-8 of the points between (up to 1024); false where they
// aren't.
bool interpolated(const std::function<bool(double, gp_Pnt&, gp_Pnt2d&, gp_Pnt2d&)>& at, double t0, double t1, int n0, IntTools_Curve& out)
{
  auto near_turn = [](double x, double to) { return x + 2 * M_PI * std::round((to - x) / (2 * M_PI)); };
  for (int n = n0; n <= 1024; n *= 2)
  {
    occ::handle<NCollection_HArray1<gp_Pnt>>   p3 = new NCollection_HArray1<gp_Pnt>(1, n + 1);
    occ::handle<NCollection_HArray1<gp_Pnt2d>> pa = new NCollection_HArray1<gp_Pnt2d>(1, n + 1);
    occ::handle<NCollection_HArray1<gp_Pnt2d>> pb = new NCollection_HArray1<gp_Pnt2d>(1, n + 1);
    occ::handle<NCollection_HArray1<double>>   pr = new NCollection_HArray1<double>(1, n + 1);
    for (int k = 0; k <= n; ++k)
    {
      const double t = t0 + (t1 - t0) * k / n;
      gp_Pnt       x;
      gp_Pnt2d     a, b;
      if (!at(t, x, a, b))
        return false;
      if (k > 0)
      {
        a.SetX(near_turn(a.X(), pa->Value(k).X()));
        b.SetX(near_turn(b.X(), pb->Value(k).X()));
      }
      p3->SetValue(k + 1, x);
      pa->SetValue(k + 1, a);
      pb->SetValue(k + 1, b);
      pr->SetValue(k + 1, t);
    }
    GeomAPI_Interpolate   f3(p3, pr, false, 1e-12);
    Geom2dAPI_Interpolate fa(pa, pr, false, 1e-12), fb(pb, pr, false, 1e-12);
    f3.Perform();
    fa.Perform();
    fb.Perform();
    if (!f3.IsDone() || !fa.IsDone() || !fb.IsDone())
      return false;
    double err = 0;
    for (int k = 0; k < n; ++k)
    {
      const double t = t0 + (t1 - t0) * (k + 0.5) / n;
      gp_Pnt       x;
      gp_Pnt2d     a, b;
      if (!at(t, x, a, b))
        return false;
      const gp_Pnt2d ca = fa.Curve()->Value(t), cb = fb.Curve()->Value(t);
      err = std::max({err, x.Distance(f3.Curve()->Value(t)), gp_Pnt2d(near_turn(a.X(), ca.X()), a.Y()).Distance(ca), gp_Pnt2d(near_turn(b.X(), cb.X()), b.Y()).Distance(cb)});
    }
    if (err <= 1e-8)
    {
      out = IntTools_Curve(f3.Curve(), fa.Curve(), fb.Curve(), std::max(Precision::Confusion(), err));
      return true;
    }
  }
  return false;
}

// Where a surface generated along a cone's axis (an extrusion along it, a cylinder beside it) meets the cone: each
// generator at the height where the cone's radius is its distance from the axis (`gen`: the parameter -> the
// generator's point at height 0 of its own surface, its direction, its parameters there; `on_gen`: its curve on the
// surface by the height along it), the generating face's parameters from `t0` to `t1`.
bool lifted(const gp_Cone& K, const std::function<gp_Pnt(double)>& gen, const gp_Dir& d, const std::function<gp_Pnt2d(double, double)>& on_gen,
            double t0, double t1, int n0, bool gen_first, IntTools_Curve& out)
{
  const gp_Dir A    = K.Axis().Direction();
  const double tana = std::tan(K.SemiAngle()), up = d.Dot(A);
  if (std::abs(tana) < 1e-9 || std::abs(std::abs(up) - 1) > 1e-12)
    return false;
  auto at = [&](double t, gp_Pnt& x, gp_Pnt2d& g, gp_Pnt2d& c) {
    const gp_Pnt P = gen(t);
    const gp_Vec w(K.Location(), P);
    const double z0 = w.Dot(gp_Vec(A)), rho = (w - gp_Vec(A) * z0).Magnitude();
    const double s  = ((rho - K.RefRadius()) / tana - z0) / up;
    x               = P.Translated(gp_Vec(d) * s);
    double u = 0, v = 0;
    ElSLib::Parameters(K, x, u, v);
    const gp_Pnt2d cg = on_gen(t, s), cc(u, v);
    g = gen_first ? cg : cc;
    c = gen_first ? cc : cg;
    return true;
  };
  return interpolated(at, t0, t1, n0, out);
}

// A surface of revolution about some axis seen in its meridian half-plane (distance from the axis, height along it):
// a plane across the axis (z = h), a cylinder (r = R), a cone (r = R + z k), a sphere or torus (a circle about (c, h),
// radius m; a sphere's c 0). `axis` is the surface's own where it has one (a plane's and a sphere's are free).
struct Meridian
{
  enum Kind { Across, Along, Slope, Round } kind;
  double h = 0, r = 0, k = 0, c = 0, m = 0;
};

struct Revolved
{
  occ::handle<Geom_Surface> s;
  std::optional<gp_Ax1>     axis;   // its own axis (none: a plane's)
  std::optional<gp_Dir>     normal; // a plane's
};

std::optional<Revolved> revolved(const TopoDS_Face& f)
{
  occ::handle<Geom_Surface> s = BRep_Tool::Surface(f);
  while (const occ::handle<Geom_RectangularTrimmedSurface> t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
    s = t->BasisSurface();
  if (const occ::handle<Geom_Plane> p = occ::down_cast<Geom_Plane>(s))
    return Revolved{s, std::nullopt, p->Pln().Axis().Direction()};
  if (const occ::handle<Geom_SphericalSurface> p = occ::down_cast<Geom_SphericalSurface>(s))
    return Revolved{s, p->Position().Axis(), std::nullopt};
  if (const occ::handle<Geom_CylindricalSurface> p = occ::down_cast<Geom_CylindricalSurface>(s))
    return Revolved{s, p->Axis(), std::nullopt};
  if (const occ::handle<Geom_ConicalSurface> p = occ::down_cast<Geom_ConicalSurface>(s))
    return Revolved{s, p->Axis(), std::nullopt};
  if (const occ::handle<Geom_ToroidalSurface> p = occ::down_cast<Geom_ToroidalSurface>(s))
    return Revolved{s, p->Axis(), std::nullopt};
  return std::nullopt;
}

// `x`'s meridian about the axis `ax` (heights from its location along its direction).
Meridian meridian_of(const Revolved& x, const gp_Ax1& ax)
{
  const gp_Pnt o = ax.Location();
  const gp_Vec a(ax.Direction());
  auto height = [&](const gp_Pnt& p) { return gp_Vec(o, p).Dot(a); };
  if (const occ::handle<Geom_Plane> p = occ::down_cast<Geom_Plane>(x.s))
    return Meridian{Meridian::Across, height(p->Location())};
  if (const occ::handle<Geom_SphericalSurface> p = occ::down_cast<Geom_SphericalSurface>(x.s))
    return Meridian{Meridian::Round, height(p->Location()), 0, 0, 0, p->Radius()};
  if (const occ::handle<Geom_CylindricalSurface> p = occ::down_cast<Geom_CylindricalSurface>(x.s))
    return Meridian{Meridian::Along, 0, p->Radius()};
  if (const occ::handle<Geom_ConicalSurface> p = occ::down_cast<Geom_ConicalSurface>(x.s))
  {
    // (Its radius grows along its own axis direction; along `ax`, the same way or the other.)
    const double k = std::tan(p->SemiAngle()) * (p->Axis().Direction().Dot(ax.Direction()) > 0 ? 1 : -1);
    const double z0 = height(p->Location());
    return Meridian{Meridian::Slope, 0, p->RefRadius() - k * z0, k};
  }
  const occ::handle<Geom_ToroidalSurface> t = occ::down_cast<Geom_ToroidalSurface>(x.s);
  return Meridian{Meridian::Round, height(t->Location()), 0, 0, t->MajorRadius(), t->MinorRadius()};
}

// Where two meridians meet in the half-plane: (r, z) with r > 0, one where they touch; none where they are one (OCCT's
// then).
std::optional<std::vector<std::pair<double, double>>> meridians_meet(const Meridian& p, const Meridian& q, double tol, bool* touched = nullptr)
{
  std::vector<std::pair<double, double>> out;
  auto as_line = [](const Meridian& m, gp_Pnt2d& at, gp_Dir2d& d) {
    if (m.kind == Meridian::Across)
      at = gp_Pnt2d(0, m.h), d = gp_Dir2d(1, 0);
    else if (m.kind == Meridian::Along)
      at = gp_Pnt2d(m.r, 0), d = gp_Dir2d(0, 1);
    else
      at = gp_Pnt2d(m.r, 0), d = gp_Dir2d(m.k, 1);
  };
  const bool pr = p.kind == Meridian::Round, qr = q.kind == Meridian::Round;
  if (!pr && !qr)
  {
    gp_Pnt2d a, b;
    gp_Dir2d da, db;
    as_line(p, a, da);
    as_line(q, b, db);
    const double den = da.Crossed(db);
    if (std::abs(den) < 1e-12)
      return std::abs(gp_Vec2d(a, b).Crossed(gp_Vec2d(da))) <= tol ? std::nullopt : std::make_optional(out); // (one line: coincident)
    const double t = gp_Vec2d(a, b).Crossed(gp_Vec2d(db)) / den;
    out.emplace_back(a.X() + t * da.X(), a.Y() + t * da.Y());
  }
  else if (pr != qr)
  {
    const Meridian& l = pr ? q : p;
    const Meridian& c = pr ? p : q;
    gp_Pnt2d a;
    gp_Dir2d d;
    as_line(l, a, d);
    const gp_Vec2d w(gp_Pnt2d(c.c, c.h), a);
    const double   b = w.Dot(gp_Vec2d(d)), cc = w.SquareMagnitude() - c.m * c.m, disc = b * b - cc;
    if (disc < -2 * tol * c.m)
      return out;
    // (Tangent: the one circle they touch along, where OCCT's marching took seconds and broke it into pieces (a
    // user's 0.4 mm rib rounds inside their cylinder: 4.9 s a pair on Windows, as OCCT's own one curve on Linux).)
    if (disc <= 2 * tol * c.m)
    {
      out.emplace_back(a.X() - b * d.X(), a.Y() - b * d.Y());
      if (touched != nullptr)
        *touched = true;
    }
    else
      for (const double sg : {-1.0, 1.0})
      {
        const double t = -b + sg * std::sqrt(disc);
        out.emplace_back(a.X() + t * d.X(), a.Y() + t * d.Y());
      }
  }
  else
  {
    const gp_Pnt2d a(p.c, p.h), b(q.c, q.h);
    const double   dd = a.Distance(b);
    if (dd < 1e-12)
      return std::abs(p.m - q.m) <= tol ? std::nullopt : std::make_optional(out);
    if (dd > p.m + q.m + tol || dd < std::abs(p.m - q.m) - tol)
      return out;
    const double   x = (dd * dd + p.m * p.m - q.m * q.m) / (2 * dd), y2 = p.m * p.m - x * x;
    const gp_Vec2d u = gp_Vec2d(a, b) / dd, n(-u.Y(), u.X());
    const gp_Pnt2d mid(a.X() + u.X() * x, a.Y() + u.Y() * x);
    if (y2 <= 2 * tol * std::max(p.m, q.m))
    {
      out.push_back({mid.X(), mid.Y()}); // (tangent: the one circle, as above)
      if (touched != nullptr)
        *touched = true;
    }
    else
      for (const double sg : {-1.0, 1.0})
        out.emplace_back(mid.X() + n.X() * sg * std::sqrt(y2), mid.Y() + n.Y() * sg * std::sqrt(y2));
  }
  std::vector<std::pair<double, double>> pos;
  for (const auto& rz : out)
    if (rz.first > tol)
      pos.push_back(rz);
  return pos;
}

// Point `q`'s parameters on the cylinder, cone, sphere or torus `s`.
void revolved_params(const occ::handle<Geom_Surface>& s, const gp_Pnt& q, double& u, double& v)
{
  if (const auto cy = occ::down_cast<Geom_CylindricalSurface>(s))
    ElSLib::Parameters(cy->Cylinder(), q, u, v);
  else if (const auto cn = occ::down_cast<Geom_ConicalSurface>(s))
    ElSLib::Parameters(cn->Cone(), q, u, v);
  else if (const auto sp = occ::down_cast<Geom_SphericalSurface>(s))
    ElSLib::Parameters(sp->Sphere(), q, u, v);
  else if (const auto to = occ::down_cast<Geom_ToroidalSurface>(s))
    ElSLib::Parameters(to->Torus(), q, u, v);
}

// A circle's curve on face surface `s` (a plane: a circle in its coordinates; else a line across its angles at one
// height, the angle's way by the surface's own parameters, a turn on where angle `lo` is the face's least at `t`).
occ::handle<Geom2d_Curve> circle_on(const occ::handle<Geom_Surface>& s, const occ::handle<Geom_Circle>& c, double lo, double t)
{
  if (const occ::handle<Geom_Plane> p = occ::down_cast<Geom_Plane>(s))
  {
    const gp_Ax3 pos = p->Pln().Position();
    auto         to2 = [&](const gp_Vec& v) { return gp_XY(v.Dot(gp_Vec(pos.XDirection())), v.Dot(gp_Vec(pos.YDirection()))); };
    const gp_XY  ctr = to2(gp_Vec(pos.Location(), c->Circ().Location()));
    const gp_XY  x = to2(gp_Vec(c->Circ().XAxis().Direction())), y = to2(gp_Vec(c->Circ().YAxis().Direction()));
    return new Geom2d_Circle(gp_Ax22d(gp_Pnt2d(ctr), gp_Dir2d(x), gp_Dir2d(y)), c->Radius());
  }
  double u0 = 0, v0 = 0, u1 = 0, v1 = 0;
  revolved_params(s, c->Value(0), u0, v0);
  revolved_params(s, c->Value(0.1), u1, v1);
  const double way = std::remainder(u1 - u0, 2 * M_PI) > 0 ? 1 : -1;
  u0 -= 2 * M_PI * std::floor((u0 + way * t - lo) / (2 * M_PI));
  return new Geom2d_Line(gp_Pnt2d(u0, v0), gp_Dir2d(way, 0));
}

// Two surfaces of revolution about one axis (planes across it, cylinders, cones, spheres, tori) meet in circles
// where their meridians cross: those, exact. False where they aren't (another axis, coincident surfaces).
bool coaxial_sections(const TopoDS_Face& fa, const TopoDS_Face& fb, std::vector<IntTools_Curve>& out, bool* touched = nullptr)
{
  const std::optional<Revolved> a = revolved(fa), b = revolved(fb);
  if (!a || !b)
    return false;
  // (Not two planes or two analytic faces the Boolean makes itself: a torus or sphere among them.)
  auto free = [](const Revolved& x) { return !occ::down_cast<Geom_ToroidalSurface>(x.s).IsNull() || !occ::down_cast<Geom_SphericalSurface>(x.s).IsNull(); };
  if (!free(*a) && !free(*b))
    return false;
  // (The axis: either's own, a sphere's the one its angles go round, where its circles are lines across its parameters.)
  const double tol = Precision::Confusion();
  const std::optional<gp_Ax1> ax = a->axis ? a->axis : b->axis;
  auto on_axis = [&](const gp_Pnt& p) { return gp_Vec(ax->Location(), p).Crossed(gp_Vec(ax->Direction())).Magnitude() <= 1e-9; };
  for (const Revolved* x : {&*a, &*b})
  {
    if (x->axis && (!x->axis->Direction().IsParallel(ax->Direction(), 1e-12) || !on_axis(x->axis->Location())))
      return false;
    if (x->normal && !x->normal->IsParallel(ax->Direction(), 1e-12))
      return false;
  }
  const Meridian ma = meridian_of(*a, *ax), mb = meridian_of(*b, *ax);
  const std::optional<std::vector<std::pair<double, double>>> rz = meridians_meet(ma, mb, tol, touched);
  if (!rz)
    return false;
  // (Each circle starts where the sphere's or torus's angles do, its seam, as OCCT's; else anywhere.)
  gp_Dir x0 = gp_Ax3(ax->Location(), ax->Direction()).XDirection();
  for (const Revolved* x : {&*a, &*b})
  {
    gp_Ax3 pos;
    if (const auto t = occ::down_cast<Geom_ToroidalSurface>(x->s))
      pos = t->Position();
    else if (const auto sp = occ::down_cast<Geom_SphericalSurface>(x->s))
      pos = sp->Position();
    else
      continue;
    const gp_Vec v = gp_Vec(pos.XDirection()) - gp_Vec(ax->Direction()) * gp_Vec(pos.XDirection()).Dot(gp_Vec(ax->Direction()));
    if (v.Magnitude() > 1e-9)
    {
      x0 = gp_Dir(v);
      break;
    }
  }
  // (Only circles within both faces' heights, the range of the parameter across their angles, as OCCT's.)
  auto within = [&](const TopoDS_Face& f, const Revolved& x, const gp_Pnt& q) {
    if (x.normal)
      return true;
    double u0 = 0, u1 = 0, v0 = 0, v1 = 0, u = 0, v = 0;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    revolved_params(x.s, q, u, v);
    if (!occ::down_cast<Geom_ToroidalSurface>(x.s).IsNull())
      v = v0 + std::fmod(std::fmod(v - v0 + 1e-9, 2 * M_PI) + 2 * M_PI, 2 * M_PI) - 1e-9;
    return v >= v0 - 1e-9 && v <= v1 + 1e-9;
  };
  for (const auto& [r, z] : *rz)
  {
    const gp_Pnt                   centre = ax->Location().Translated(gp_Vec(ax->Direction()) * z);
    const occ::handle<Geom_Circle> c      = new Geom_Circle(gp_Ax2(centre, ax->Direction(), x0), r);
    if (!within(fa, *a, c->Value(0)) || !within(fb, *b, c->Value(0)))
      continue;
    // (The arcs within both faces' angles, as OCCT's: each face's (start, length) along the circle.)
    double lo[2] = {0, 0};
    std::vector<std::pair<double, double>> arcs = {{0, 2 * M_PI}};
    for (int i = 0; i < 2; ++i)
    {
      const Revolved& x = i == 0 ? *a : *b;
      if (x.normal)
        continue;
      double u0 = 0, u1 = 0, v0 = 0, v1 = 0, us = 0, v = 0, ut = 0;
      BRepTools::UVBounds(i == 0 ? fa : fb, u0, u1, v0, v1);
      lo[i] = u0;
      if (u1 - u0 >= 2 * M_PI - 1e-9)
        continue;
      revolved_params(x.s, c->Value(0), us, v);
      revolved_params(x.s, c->Value(0.1), ut, v);
      const bool   up    = std::remainder(ut - us, 2 * M_PI) > 0;
      const double start = std::fmod(std::fmod(up ? u0 - us : us - u1, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
      std::vector<std::pair<double, double>> kept;
      for (const auto& [s0, len] : arcs)
        for (const double k : {-2 * M_PI, 0.0, 2 * M_PI})
        {
          const double from = std::max(s0, start + k), to = std::min(s0 + len, start + k + (u1 - u0));
          if (to - from > 1e-9)
            kept.emplace_back(from, to - from);
        }
      arcs = kept;
    }
    for (const auto& [from, len] : arcs)
    {
      const double mid = from + len / 2;
      const occ::handle<Geom_Curve> arc = len >= 2 * M_PI - 1e-9 ? occ::handle<Geom_Curve>(c) : occ::handle<Geom_Curve>(new Geom_TrimmedCurve(c, from, from + len));
      out.emplace_back(arc, circle_on(a->s, c, lo[0], mid), circle_on(b->s, c, lo[1], mid), tol);
    }
  }
  return true;
}

// noshape's own sections of faces `fa` and `fb`, as OCCT's IntTools_Curve gives them (the curve, its curves on `fa` and
// `fb` in their own parameters (the same parameter), its tolerance): an extrusion against a plane across its
// direction (the profile moved there), a cylinder along it or another extrusion along it (rulings where the profiles
// cross, seen along it). Exact, where OCCT's approximates; false where it isn't one of those (OCCT's then).
bool own_sections(const TopoDS_Face& fa, const TopoDS_Face& fb, std::vector<IntTools_Curve>& out)
{
  static const bool off = std::getenv("NOSHAPE_OWN_SECTIONS") != nullptr && std::string(std::getenv("NOSHAPE_OWN_SECTIONS")) == "0";
  if (off)
    return false;
  if (coaxial_sections(fa, fb, out))
    return true;
  const std::optional<Extruded> xa = extruded(fa), xb = extruded(fb);
  // A cone and a surface generated along its axis: that surface lifted onto it.
  const occ::handle<Geom_ConicalSurface> ka = occ::down_cast<Geom_ConicalSurface>(BRep_Tool::Surface(fa)), kb = occ::down_cast<Geom_ConicalSurface>(BRep_Tool::Surface(fb));
  if (!ka.IsNull() || !kb.IsNull())
  {
    const gp_Cone      K       = (!ka.IsNull() ? ka : kb)->Cone();
    const bool         g_first = ka.IsNull();
    const TopoDS_Face& g       = g_first ? fa : fb;
    IntTools_Curve     ic;
    if (const std::optional<Extruded>& x = g_first ? xa : xb)
    {
      if (!lifted(K, [&](double u) { return x->c->Value(u); }, x->d, [&](double u, double s) { return x->uv(u, s); }, x->u0, x->u1, std::max(32, 8 * (x->c->NbKnots() - 1)), g_first, ic))
        return false;
      out.push_back(ic);
      return true;
    }
    if (const occ::handle<Geom_CylindricalSurface> cy = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(g)))
    {
      const gp_Cylinder Y = cy->Cylinder();
      double            a0 = 0, a1 = 0, h0 = 0, h1 = 0;
      BRepTools::UVBounds(g, a0, a1, h0, h1);
      if (!lifted(K, [&](double a) { return ElSLib::Value(a, 0, Y); }, Y.Axis().Direction(), [](double a, double s) { return gp_Pnt2d(a, s); }, a0, a1, 64, g_first, ic))
        return false;
      out.push_back(ic);
      return true;
    }
    return false;
  }
  if (!xa && !xb)
    return false;
  const bool                      e_first = xa.has_value();
  const Extruded&                 e       = e_first ? *xa : *xb;
  const TopoDS_Face&              other   = e_first ? fb : fa;
  const occ::handle<Geom_Surface> so      = BRep_Tool::Surface(other);
  const gp_Ax2                    frame(gp::Origin(), e.d);
  const double                    along   = 1e-12;
  // A plane across it: the profile, if it lies across the direction too, moved to the plane's height.
  if (const occ::handle<Geom_Plane> pl = occ::down_cast<Geom_Plane>(so))
  {
    const gp_Pln P = pl->Pln();
    const gp_Dir n = P.Axis().Direction();
    // Along the plane (the direction in it): rulings where the profile, seen along the direction, crosses the plane's
    // trace (exact lines, as Parasolid's, where OCCT's approximates them: an imported wall's window cut 0.6 um bowed).
    if (std::abs(n.Dot(e.d)) <= along)
    {
      const gp_Vec   w(frame.Location(), P.Location());
      const gp_Dir   across = n.Crossed(e.d);
      const gp_Pnt2d o(w.Dot(gp_Vec(frame.XDirection())), w.Dot(gp_Vec(frame.YDirection())));
      std::vector<std::pair<double, double>> at;
      if (!crossings2d(new Geom2d_Line(o, gp_Dir2d(across.Dot(frame.XDirection()), across.Dot(frame.YDirection()))), flat(e.c, frame), at))
        return false;
      const gp_Dir2d up(e.d.Dot(P.XAxis().Direction()), e.d.Dot(P.YAxis().Direction()));
      for (const auto& [_, u] : at)
      {
        if (u < e.u0 - 1e-9 || u > e.u1 + 1e-9)
          continue;
        double pu = 0, pv = 0;
        ElSLib::Parameters(P, e.c->Value(u), pu, pv);
        out.push_back(ruling(e, u, new Geom2d_Line(gp_Pnt2d(pu, pv), up), e_first));
      }
      return true;
    }
    if (std::abs(std::abs(n.Dot(e.d)) - 1) > along)
      return false;
    const double h = gp_Vec(e.c->Pole(1).XYZ()).Dot(gp_Vec(e.d));
    for (int i = 2; i <= e.c->NbPoles(); ++i)
      if (std::abs(gp_Vec(e.c->Pole(i).XYZ()).Dot(gp_Vec(e.d)) - h) > 1e-10)
        return false;
    double v = gp_Vec(P.Location().XYZ()).Dot(gp_Vec(e.d)) - h;
    if (v < e.v0 - Precision::Confusion() || v > e.v1 + Precision::Confusion())
      return true;
    v = std::clamp(v, e.v0, e.v1);
    occ::handle<Geom_BSplineCurve> moved = occ::down_cast<Geom_BSplineCurve>(e.c->Copy());
    moved->Translate(gp_Vec(e.d) * v);
    const occ::handle<Geom_Curve>   curve = new Geom_TrimmedCurve(moved, e.u0, e.u1);
    const occ::handle<Geom2d_Curve> on_e  = on_profile(e, v);
    const occ::handle<Geom2d_Curve> on_p  = flat(moved, P.Position().Ax2());
    out.emplace_back(curve, e_first ? on_e : on_p, e_first ? on_p : on_e, Precision::Confusion());
    return true;
  }
  // A cylinder along it: rulings where the profile crosses its circle.
  if (const occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(so))
  {
    const gp_Cylinder C = cs->Cylinder();
    const double      up = C.Axis().Direction().Dot(e.d);
    if (std::abs(std::abs(up) - 1) > along)
      return false;
    const gp_Vec   w(frame.Location(), C.Location());
    const gp_Pnt2d centre(w.Dot(gp_Vec(frame.XDirection())), w.Dot(gp_Vec(frame.YDirection())));
    std::vector<std::pair<double, double>> at;
    if (!crossings2d(new Geom2d_Circle(gp_Ax2d(centre, gp_Dir2d(1, 0)), C.Radius()), flat(e.c, frame), at))
      return false;
    for (const auto& [_, u] : at)
    {
      if (u < e.u0 - 1e-9 || u > e.u1 + 1e-9)
        continue;
      double a = 0, h0 = 0;
      ElSLib::Parameters(C, e.c->Value(u), a, h0);
      out.push_back(ruling(e, u, new Geom2d_Line(gp_Pnt2d(a, h0), gp_Dir2d(0, up > 0 ? 1 : -1)), e_first));
    }
    return true;
  }
  // Another extrusion along it: rulings where the profiles cross.
  if (xa && xb)
  {
    const double same = xa->d.Dot(xb->d);
    if (std::abs(std::abs(same) - 1) > along)
      return false;
    std::vector<std::pair<double, double>> at;
    if (!crossings2d(flat(xa->c, frame), flat(xb->c, frame), at))
      return false;
    for (const auto& [ua, ub] : at)
    {
      if (ua < xa->u0 - 1e-9 || ua > xa->u1 + 1e-9 || ub < xb->u0 - 1e-9 || ub > xb->u1 + 1e-9)
        continue;
      const double w0 = gp_Vec(xb->c->Value(ub), xa->c->Value(ua)).Dot(gp_Vec(xb->d));
      out.push_back(ruling(*xa, ua, on_ruling(*xb, ub, w0, same > 0 ? 1 : -1, xa->v0, xa->v1), true));
    }
    return true;
  }
  return false;
}

// OCCT's section of two faces, one of them free-form, brought onto both surfaces where its approximation is coarse
// (its tolerance past 1e-5 m: a sphere's section with a B-spline face, cf-surface-pattern's, 2.4e-4 m off them,
// our grey band about a face's boundary then millimetres wide): 129 of its points (from its own parameters, its
// curves on the faces as the first guesses) projected onto both surfaces in turn until within 1e-9 of each, the
// curve and its curves on the faces fitted through them by chord length; where the fit strays from the surfaces
// between two points past 1e-7, their middle brought onto both too and the fit made again (up to five rounds: a
// section OCCT gave 0.26 off, fitted through 129 points, wandered 3 mm between them); its tolerance what the fit is
// off them between its points. Else (a point not brought onto both, the fit no tighter) the section as OCCT has it.
static IntTools_Curve refined_section(const IntTools_Curve& ic, const TopoDS_Face& fa, const TopoDS_Face& fb)
{
  auto analytic = [](const TopoDS_Face& f) {
    const GeomAbs_SurfaceType t = GeomAdaptor_Surface(BRep_Tool::Surface(f)).GetType();
    return t == GeomAbs_Plane || t == GeomAbs_Cylinder || t == GeomAbs_Cone || t == GeomAbs_Sphere || t == GeomAbs_Torus;
  };
  const occ::handle<Geom_Curve>   c  = ic.Curve();
  const occ::handle<Geom2d_Curve> c1 = ic.FirstCurve2d(), c2 = ic.SecondCurve2d();
  if (ic.Tolerance() <= 1e-5 || c.IsNull() || c1.IsNull() || c2.IsNull() || (analytic(fa) && analytic(fb)))
    return ic;
  const occ::handle<Geom_Surface> sa = BRep_Tool::Surface(fa), sb = BRep_Tool::Surface(fb);
  const occ::handle<ShapeAnalysis_Surface> qa = new ShapeAnalysis_Surface(sa), qb = new ShapeAnalysis_Surface(sb);
  struct OnBoth
  {
    gp_Pnt   p;
    gp_Pnt2d a, b;
  };
  // (A point and its places on the faces, from guesses: on both within 1e-9, else false.)
  auto onto = [&](gp_Pnt m, gp_Pnt2d pa, gp_Pnt2d pb, OnBoth& out) {
    pa         = qa->NextValueOfUV(pa, m, 1e-10);
    pb         = qb->NextValueOfUV(pb, m, 1e-10);
    double gap = 1;
    for (int it = 0; it < 50 && gap > 1e-9; ++it)
    {
      const gp_Pnt xa = sa->Value(pa.X(), pa.Y()), xb = sb->Value(pb.X(), pb.Y());
      gap = xa.Distance(xb);
      m   = gp_Pnt((xa.XYZ() + xb.XYZ()) / 2);
      pa  = qa->NextValueOfUV(pa, m, 1e-10);
      pb  = qb->NextValueOfUV(pb, m, 1e-10);
    }
    out = OnBoth{m, pa, pb};
    return gap <= 1e-8;
  };
  const int           n = 128;
  const double        t0 = c->FirstParameter(), t1 = c->LastParameter();
  std::vector<OnBoth> ps;
  for (int k = 0; k <= n; ++k)
  {
    const double t = t0 + (t1 - t0) * k / n;
    OnBoth       x;
    if (!onto(c->Value(t), c1->Value(t), c2->Value(t), x))
      return ic;
    ps.push_back(x);
  }
  occ::handle<Geom_Curve>   nc;
  occ::handle<Geom2d_Curve> na, nb;
  double                    tol = 0;
  for (int round = 0;; ++round)
  {
    const int m = static_cast<int>(ps.size());
    occ::handle<NCollection_HArray1<gp_Pnt>>   pts = new NCollection_HArray1<gp_Pnt>(1, m);
    occ::handle<NCollection_HArray1<gp_Pnt2d>> ua  = new NCollection_HArray1<gp_Pnt2d>(1, m), ub = new NCollection_HArray1<gp_Pnt2d>(1, m);
    // (By chord length: OCCT's parameters can bunch, the cubic through them waving between.)
    occ::handle<NCollection_HArray1<double>> prm = new NCollection_HArray1<double>(1, m);
    for (int k = 0; k < m; ++k)
    {
      pts->SetValue(k + 1, ps[k].p);
      ua->SetValue(k + 1, ps[k].a);
      ub->SetValue(k + 1, ps[k].b);
      prm->SetValue(k + 1, k == 0 ? 0.0 : prm->Value(k) + ps[k].p.Distance(ps[k - 1].p));
      if (k > 0 && !(prm->Value(k + 1) > prm->Value(k)))
        return ic;
    }
    GeomAPI_Interpolate   f3(pts, prm, false, 1e-12);
    Geom2dAPI_Interpolate fa2(ua, prm, false, 1e-12), fb2(ub, prm, false, 1e-12);
    f3.Perform();
    fa2.Perform();
    fb2.Perform();
    if (!f3.IsDone() || !fa2.IsDone() || !fb2.IsDone())
      return ic;
    nc = f3.Curve();
    na = fa2.Curve();
    nb = fb2.Curve();
    tol = 0;
    std::vector<double> off(static_cast<size_t>(m - 1));
    for (int k = 0; k + 1 < m; ++k)
    {
      const double   t   = 0.5 * (prm->Value(k + 1) + prm->Value(k + 2));
      const gp_Pnt   p   = nc->Value(t);
      const gp_Pnt2d qa2 = na->Value(t), qb2 = nb->Value(t);
      off[(size_t)k]     = std::max(sa->Value(qa2.X(), qa2.Y()).Distance(p), sb->Value(qb2.X(), qb2.Y()).Distance(p));
      tol                = std::max(tol, off[(size_t)k]);
    }
    if (tol <= 1e-7 || round == 5)
      break;
    // (A middle not brought onto both: the fit as it is.)
    std::vector<OnBoth> more;
    bool                all = true;
    for (int k = 0; k + 1 < m && all; ++k)
    {
      more.push_back(ps[(size_t)k]);
      if (off[(size_t)k] <= 1e-7)
        continue;
      const double t = 0.5 * (prm->Value(k + 1) + prm->Value(k + 2));
      OnBoth       x;
      all = onto(gp_Pnt((ps[(size_t)k].p.XYZ() + ps[(size_t)k + 1].p.XYZ()) / 2), na->Value(t), nb->Value(t), x);
      more.push_back(x);
    }
    if (!all)
      break;
    more.push_back(ps.back());
    ps = std::move(more);
  }
  tol = std::max(tol, 1e-7);
  if (!(tol < 0.5 * ic.Tolerance()))
    return ic;
  return IntTools_Curve(nc, na, nb, tol, ic.TangentialTolerance());
}

static void occt_ahead(rust::Slice<const uint32_t> pairs)
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
    // (None started once the caller's budget or cancel says stop.)
    if (noshape_should_break() != 0)
      return;
    try
    {
      if (own_sections(jobs[k].fa, jobs[k].fb, jobs[k].lines))
      {
        jobs[k].done = true;
        return;
      }
      jobs[k].lines.clear();
      IntTools_FaceFace ff;
      ff.SetParameters(true, true, true, 1e-7);
      ff.Perform(jobs[k].fa, jobs[k].fb);
      if (!ff.IsDone())
        return;
      for (const IntTools_Curve& ic : ff.Lines())
        jobs[k].lines.push_back(refined_section(ic, jobs[k].fa, jobs[k].fb));
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

// OCCT's face/face section of `fa` and `fb` walked from start points between `ends` (where the faces' boundaries
// meet): each two's middle brought onto both surfaces (alternately projected until on both within 1e-7; else
// none from there). A curve whose middle lies on one found before is dropped.
static std::vector<IntTools_Curve> seeded_sections(const TopoDS_Face& fa, const TopoDS_Face& fb, rust::Slice<const double> ends)
{
  std::vector<IntTools_Curve> out;
  const occ::handle<Geom_Surface> sa = BRep_Tool::Surface(fa), sb = BRep_Tool::Surface(fb);
  // (The first projection a search over each surface, then steps from the parameters before (a fresh search each
  // step was most of a small knurl's Boolean).)
  const occ::handle<ShapeAnalysis_Surface> qa = new ShapeAnalysis_Surface(sa), qb = new ShapeAnalysis_Surface(sb);
  NCollection_List<IntSurf_PntOn2S> seeds;
  const size_t                      n = ends.size() / 3;
  for (size_t i = 0; i < n; ++i)
    for (size_t j = i + 1; j < n; ++j)
    {
      gp_Pnt   m((ends[3 * i] + ends[3 * j]) / 2, (ends[3 * i + 1] + ends[3 * j + 1]) / 2, (ends[3 * i + 2] + ends[3 * j + 2]) / 2);
      gp_Pnt2d ua = qa->ValueOfUV(m, 1e-9), ub = qb->ValueOfUV(sa->Value(ua.X(), ua.Y()), 1e-9);
      double   gap = 1;
      for (int k = 0; k < 50 && gap > 1e-9; ++k)
      {
        const gp_Pnt pa = sa->Value(ua.X(), ua.Y()), pb = sb->Value(ub.X(), ub.Y());
        gap = pa.Distance(pb);
        m   = gp_Pnt((pa.XYZ() + pb.XYZ()) / 2);
        ua  = qa->NextValueOfUV(ua, m, 1e-10);
        ub  = qb->NextValueOfUV(ub, m, 1e-10);
      }
      if (gap > 1e-7)
        continue;
      IntSurf_PntOn2S p;
      p.SetValue(m, ua.X(), ua.Y(), ub.X(), ub.Y());
      seeds.Append(p);
    }
  if (seeds.IsEmpty())
    return out;
  IntTools_FaceFace ff;
  ff.SetParameters(true, true, true, 1e-7);
  ff.SetList(seeds);
  ff.Perform(fa, fb);
  if (!ff.IsDone())
    return out;
  for (const IntTools_Curve& ic : ff.Lines())
  {
    const occ::handle<Geom_Curve> c = ic.Curve();
    if (c.IsNull())
      continue;
    const gp_Pnt mid = c->Value((c->FirstParameter() + c->LastParameter()) / 2);
    const bool   seen = std::any_of(out.begin(), out.end(), [&](const IntTools_Curve& o) {
      GeomAPI_ProjectPointOnCurve pc(mid, o.Curve());
      return pc.NbPoints() > 0 && pc.LowerDistance() <= 1e-7;
    });
    if (!seen)
      out.push_back(refined_section(ic, fa, fb));
  }
  return out;
}

static rust::String occt_sections(uint32_t a_arg, uint32_t a_face, uint32_t b_arg, uint32_t b_face, rust::Slice<const double> ends, BrepSections& out)
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
  if (ends.size() >= 6)
  {
    try
    {
      lines = seeded_sections(fa, fb, ends);
    }
    catch (const Standard_Failure&)
    {
      return rust::String("OCCT's section failed");
    }
  }
  // (Kept for the Boolean's later rounds (boolean_with: again with vertices merged), whose input faces are these.)
  else if (made != t_free->ahead.end())
    lines = made->second;
  else if (own_sections(fa, fb, lines))
    t_free->ahead.emplace(swap ? std::array<uint32_t, 4>{b_arg, b_face, a_arg, a_face} : std::array<uint32_t, 4>{a_arg, a_face, b_arg, b_face}, lines);
  else
  {
    lines.clear();
    IntTools_FaceFace ff;
    ff.SetParameters(true, true, true, 1e-7);
    // (Called from Rust: an OCCT exception, a cancelled run's break hook's, must not get out.)
    try
    {
      ff.Perform(fa, fb);
    }
    catch (const Standard_Failure&)
    {
      return rust::String("OCCT's section failed");
    }
    if (!ff.IsDone())
      return rust::String("OCCT's section failed");
    for (const IntTools_Curve& ic : ff.Lines())
      lines.push_back(refined_section(ic, fa, fb));
    t_free->ahead.emplace(swap ? std::array<uint32_t, 4>{b_arg, b_face, a_arg, a_face} : std::array<uint32_t, 4>{a_arg, a_face, b_arg, b_face}, lines);
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
    for (const double v : {id, first, last, at3, at0, at1, section_tol(ic, fa, fb), ic.Tolerance()})
      out.curves.push_back(v);
  }
  return rust::String("");
}

// The callbacks noshape's Boolean makes (cxx calls them noexcept: an exception out of one terminated the process, the
// sheet metal louver's tools' fuse, SM-LOUVER-KERNEL-ABORT): one thrown in OCCT is a failed section, so a decline.
rust::String brep_occt_sections(uint32_t a_arg, uint32_t a_face, uint32_t b_arg, uint32_t b_face, rust::Slice<const double> ends, BrepSections& out)
{
  try
  {
    return occt_sections(a_arg, a_face, b_arg, b_face, ends, out);
  }
  catch (...)
  {
    return rust::String("OCCT's section threw");
  }
}

rust::Vec<uint32_t> brep_occt_moved(rust::Slice<const uint32_t> ids, uint32_t a_arg, uint32_t a_face, uint32_t b_arg, uint32_t b_face, rust::Slice<const double> moved)
{
  try
  {
    return occt_moved(ids, a_arg, a_face, b_arg, b_face, moved);
  }
  catch (...)
  {
    return rust::Vec<uint32_t>();
  }
}

void brep_occt_ahead(rust::Slice<const uint32_t> pairs)
{
  try
  {
    occt_ahead(pairs);
  }
  catch (...)
  {
  }
}

// Whether a solid is inside out (its faces' normals pointing in): a planar face whose plane holds the solid's
// vertices at one extreme says it cheaply, its normal pointing away from them, or at them with its box there too;
// none such: its volume's sign.
static bool solid_inside_out(const TopoDS_Shape& solid)
{
  ShapeIMap vertices;
  TopExp::MapShapes(solid, TopAbs_VERTEX, vertices);
  std::vector<gp_Pnt> points;
  points.reserve(vertices.Extent());
  double tol = Precision::Confusion();
  for (int i = 1; i <= vertices.Extent(); ++i)
  {
    const TopoDS_Vertex& v = TopoDS::Vertex(vertices(i));
    points.push_back(BRep_Tool::Pnt(v));
    tol = std::max(tol, BRep_Tool::Tolerance(v));
  }
  Bnd_Box box;
  for (TopExp_Explorer e(solid, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face&      f     = TopoDS::Face(e.Current());
    occ::handle<Geom_Plane> plane = occ::down_cast<Geom_Plane>(BRep_Tool::Surface(f));
    if (plane.IsNull())
      continue;
    const gp_Ax3& ax = plane->Position();
    gp_Dir        n  = ax.XDirection().Crossed(ax.YDirection());
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    const double d = gp_Vec(plane->Location().XYZ()).Dot(n);
    double lo = d, hi = d;
    for (const gp_Pnt& p : points)
    {
      const double x = gp_Vec(p.XYZ()).Dot(n);
      lo = std::min(lo, x);
      hi = std::max(hi, x);
    }
    if (hi - lo <= 2 * tol)
      continue;
    if (d >= hi - tol)
      return false;
    if (d > lo + tol)
      continue;
    // At the vertices' extreme, unless a curved face bulges past it (its box says).
    if (box.IsVoid())
    {
      BRepBndLib::Add(solid, box, false);
      for (TopExp_Explorer x(solid, TopAbs_EDGE); x.More(); x.Next())
        tol = std::max(tol, BRep_Tool::Tolerance(TopoDS::Edge(x.Current())));
      for (TopExp_Explorer x(solid, TopAbs_FACE); x.More(); x.Next())
        tol = std::max(tol, BRep_Tool::Tolerance(TopoDS::Face(x.Current())));
    }
    double x0, y0, z0, x1, y1, z1;
    box.Get(x0, y0, z0, x1, y1, z1);
    double blo = d;
    for (int c = 0; c < 8; ++c)
      blo = std::min(blo, gp_Vec(c & 1 ? x1 : x0, c & 2 ? y1 : y0, c & 4 ? z1 : z0).Dot(n));
    if (d <= blo + 3 * tol)
      return true;
  }
  GProp_GProps g;
  BRepGProp::VolumeProperties(solid, g, true);
  return g.Mass() < 0;
}

bool rust_boolean(uint8_t op, const std::vector<TopoDS_Shape>& args, std::vector<const OcShape*>& srcs,
                  std::vector<std::unique_ptr<OcShape>>& own, TopoDS_Shape& out, occ::handle<BRepTools_History>& hist,
                  double fuzzy, bool touching_only, bool scaled_inputs)
{
  // A stale regeneration's Boolean stops (as OCCT's does through its progress indicator).
  const std::atomic<bool>* cancel = thread_cancel_flag();
  auto stop = [&] {
    if (cancel != nullptr && cancel->load(std::memory_order_relaxed))
      fail_op("cancelled");
  };
  stop();
  // A sheet cut by 64 sheets or more: ours says its untouched edges and vertices Modified, as OCCT's path does where it
  // cuts scaled copies (op_boolean's ScaledInputs); where it doesn't scale, OCCT's.
  if (op == 1 && args.size() > 64 && (working_scale() == 1.0 || std::getenv("NOSHAPE_BOOL_NO_SCALE") != nullptr)
      && std::none_of(args.begin(), args.end(), [](const TopoDS_Shape& a) { return TopExp_Explorer(a, TopAbs_SOLID).More(); }))
    return decline("a sheet cut by many sheets, unscaled");
  srcs.resize(args.size(), nullptr);
  for (size_t i = 0; i < args.size(); ++i)
    if (srcs[i] == nullptr || !srcs[i]->shape.IsEqual(args[i]))
    {
      own.push_back(std::make_unique<OcShape>(args[i]));
      srcs[i] = own.back().get();
    }
  // A tool that's inside out (a mirror's image not turned back: c2-sm-louver's Mirror 2 cutter, its volume below
  // zero): ours classifies every point against its faces' normals the wrong way round and declines late ("no clean
  // ray", 0.35 s; OCCT takes it as it is). Declined first; found once a shape. (An object inside out is taken alike:
  // a fuse of one, OCCT's result.)
  for (size_t i = 1; i < args.size(); ++i)
  {
    int inside_out = srcs[i]->myInsideOut.load(std::memory_order_relaxed);
    if (inside_out < 0)
    {
      inside_out = 0;
      for (TopExp_Explorer e(args[i], TopAbs_SOLID); e.More() && inside_out == 0; e.Next())
        inside_out = solid_inside_out(e.Current()) ? 1 : 0;
      srcs[i]->myInsideOut.store(inside_out, std::memory_order_relaxed);
    }
    if (inside_out == 1)
      return decline("an inside-out tool");
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
    // (Tolerant vertices and edges alone: noshape's Boolean goes by each one's own tolerance, OCCT's rule, the
    // Boolean's by the rest.)
    // (Tolerant faces: each with nothing of another argument in its grey band, the vertices and edges by their own
    // rule as above; a Shell slab fuse's part has hundreds of tolerant vertices and edges and a face or two of
    // 1e-5 (OCCT's nominal; their real 1e-7).)
    // (A tolerant face up to half a millimetre goes by its own tolerance too (noshape's Boolean has each face's:
    // Shell's slab faces of 7.5e-5 with another slab's vertex 6.7e-5 off them).)
    tolerant.erase(std::remove_if(tolerant.begin(), tolerant.end(), [&](const auto& x) { return x.second.ShapeType() != TopAbs_FACE || tol_of(x.second) <= 5e-4; }), tolerant.end());
    bool reaches = tolerant.size() > 64;
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
            // (The other's own tolerance up to the ordinary: a tolerant vertex or edge goes by its rule.)
            reaches = !dist.IsDone() || (d > exact && d <= (std::getenv("NOSHAPE_X_FT") ? 1.5 : GREY_TOL) * (tx + std::min(tol_of(m(q)), kMaxTol)));
          }
        }
      }
    }
    if (!reaches)
      max_tol = rest;
  }
  if (max_tol > kMaxTol)
    return decline("a tolerant input");
  // Arguments sharing sub-shapes (a joint's tools made from the part's faces): each shared vertex, edge and face of a
  // later argument named with the first one's it is (kind 0 vertex, 1 edge, 2 face; argument, index; the first's
  // argument, index), so ours keeps it as it is (OCCT's Kept). (One map of every argument's so far: a pattern's
  // thousand instances against each other took seconds pair by pair.)
  std::vector<uint32_t> shared;
  {
    const TopAbs_ShapeEnum kinds[] = {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_FACE};
    for (uint32_t kind = 0; kind < 3; ++kind)
    {
      NCollection_DataMap<TopoDS_Shape, std::pair<uint32_t, uint32_t>, TopTools_ShapeMapHasher> first;
      for (size_t i = 0; i < args.size(); ++i)
      {
        const ShapeIMap& m = srcs[i]->map(kinds[kind]);
        for (int k = 1; k <= m.Extent(); ++k)
        {
          if (const std::pair<uint32_t, uint32_t>* at = first.Seek(m(k)))
          {
            if (at->first != i)
              shared.insert(shared.end(), {kind, static_cast<uint32_t>(i), static_cast<uint32_t>(k - 1), at->first, at->second});
          }
          else
            first.Bind(m(k), {static_cast<uint32_t>(i), static_cast<uint32_t>(k - 1)});
        }
      }
    }
  }
  // Kept as they are where everything at them is shared (an edge whose faces are all shared, in every argument, its
  // vertices with only such edges; a face with only such edges): OCCT keeps those. Where two other arguments' faces
  // on different planes meet one, their section reaches it and OCCT remakes it (Modified, at the same place: it never
  // changes an input's tolerance), with the edges and faces on it: ours remakes it too. Where only faces on one plane
  // meet it, OCCT keeps or remakes it as its rounding falls (corpus2 4006f865 kept, 1b2319a3 one corner remade, its
  // mirror image kept), and other surfaces aren't read here: declined.
  if (!shared.empty())
  {
    NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> in_args;
    for (size_t i = 0; i < args.size(); ++i)
      for (const TopAbs_ShapeEnum t : {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_FACE})
      {
        const ShapeIMap& m = srcs[i]->map(t);
        for (int k = 1; k <= m.Extent(); ++k)
          if (int* n = in_args.ChangeSeek(m(k)))
            ++*n;
          else
            in_args.Bind(m(k), 1);
      }
    auto is_shared = [&](const TopoDS_Shape& x) {
      const int* n = in_args.Seek(x);
      return n != nullptr && *n > 1;
    };
    std::vector<TopTools_IndexedDataMapOfShapeListOfShape> ef(args.size()), vf(args.size()), ve(args.size());
    for (size_t i = 0; i < args.size(); ++i)
    {
      TopExp::MapShapesAndAncestors(args[i], TopAbs_EDGE, TopAbs_FACE, ef[i]);
      TopExp::MapShapesAndAncestors(args[i], TopAbs_VERTEX, TopAbs_FACE, vf[i]);
      TopExp::MapShapesAndAncestors(args[i], TopAbs_VERTEX, TopAbs_EDGE, ve[i]);
    }
    auto all_of = [&](const std::vector<TopTools_IndexedDataMapOfShapeListOfShape>& anc, const TopoDS_Shape& x, const auto& ok) {
      for (const TopTools_IndexedDataMapOfShapeListOfShape& m : anc)
        if (const NCollection_List<TopoDS_Shape>* l = m.Seek(x))
          for (const TopoDS_Shape& y : *l)
            if (!ok(y))
              return false;
      return true;
    };
    auto faces_shared = [&](const TopoDS_Shape& e) { return is_shared(e) && all_of(ef, e, is_shared); };
    auto vertex_kept  = [&](const TopoDS_Shape& v) { return is_shared(v) && all_of(ve, v, faces_shared); };
    auto edge_kept    = [&](const TopoDS_Shape& e) {
      TopoDS_Vertex v0, v1;
      TopExp::Vertices(TopoDS::Edge(e), v0, v1);
      return faces_shared(e) && (v0.IsNull() || vertex_kept(v0)) && (v1.IsNull() || vertex_kept(v1));
    };
    auto face_kept = [&](const TopoDS_Shape& f) {
      for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
        if (!edge_kept(x.Current()))
          return false;
      return true;
    };
    // 1: on one plane; 0: not; -1: other surfaces.
    auto one_plane = [](const TopoDS_Face& a, const TopoDS_Face& b) {
      const BRepAdaptor_Surface sa(a, false), sb(b, false);
      if (sa.GetType() != GeomAbs_Plane || sb.GetType() != GeomAbs_Plane)
        return -1;
      const gp_Pln pa = sa.Plane(), pb = sb.Plane();
      return pa.Axis().Direction().IsParallel(pb.Axis().Direction(), 1e-9) && pa.Distance(pb.Location()) <= 1e-7 ? 1 : 0;
    };
    // 1: no section reaches it; 0: one may; -1: unknown.
    auto quiet = [&](const TopoDS_Shape& x, bool vertex) {
      std::vector<std::pair<size_t, TopoDS_Face>> own;
      for (size_t i = 0; i < args.size(); ++i)
        if (const NCollection_List<TopoDS_Shape>* l = (vertex ? vf[i] : ef[i]).Seek(x))
          for (const TopoDS_Shape& f : *l)
            if (!is_shared(f))
              own.emplace_back(i, TopoDS::Face(f));
      int r = 1;
      for (size_t a = 0; a < own.size(); ++a)
        for (size_t b = a + 1; b < own.size(); ++b)
          if (own[a].first != own[b].first)
            r = std::min(r, one_plane(own[a].second, own[b].second));
      return r;
    };
    const TopAbs_ShapeEnum kinds[] = {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_FACE};
    std::vector<uint32_t> steady;
    for (size_t k = 0; k + 4 < shared.size(); k += 5)
    {
      const TopoDS_Shape& x = input(srcs, shared[k + 1], kinds[shared[k]], shared[k + 2]);
      if (shared[k] == 0 ? vertex_kept(x) : shared[k] == 1 ? edge_kept(x) : face_kept(x))
        steady.insert(steady.end(), shared.begin() + static_cast<std::ptrdiff_t>(k), shared.begin() + static_cast<std::ptrdiff_t>(k + 5));
      else if (shared[k] < 2 && quiet(x, shared[k] == 0) != 0)
        return decline("a shared shape OCCT may keep or remake");
    }
    shared = std::move(steady);
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
    const KernelTimer     timer("boolean.rust.brep");
    const RustMemoryWatch watch;
    if (!brep_boolean(op, rust::Slice<const BrepBody>(bodies.data(), bodies.size()), rust::Slice<const uint32_t>(shared.data(), shared.size()), tol, fuzzy, static_cast<uint8_t>((touching_only ? 1 : 0) | (scaled_inputs ? 2 : 0)), r))
    {
      // (Declined "cancelled": stopped, not handed to OCCT; so past the memory budget, with OCCT's error.)
      stop();
      if (RustMemoryWatch::over())
        fail_op("operation stopped: it needed more than " + std::to_string(RustMemoryWatch::budget_mb()) + " MB of memory");
      return decline(nullptr);
    }
  }
  const KernelTimer timer("boolean.rust.build");
  stop();
  if (const char* why = build(r, srcs, max_tol > 0 ? max_tol : 1e-7, out, hist))
    return decline(why);
  // (Stopped while building (the cancel flag, a budget's deadline): OCCT's path stops at once with its error.)
  stop();
  if (noshape_should_break() != 0)
    return decline(nullptr);
  // The result as OCCT's checker sees it, up to 2000 faces and 256 wires a face (a bigger one's check costs seconds:
  // the knurl's 2 s; its wires' check goes by pairs: a plate's face with 1739 holes 0.4 s): an invalid one declines
  // and OCCT builds the Boolean (invalid results of ours, fed into later OCCT operations, crashed them: fuzz seed
  // 1144's mirror fuse and Shell common).
  size_t most_wires = 0;
  for (const TopoDS_Shape& f : t_built.f)
  {
    size_t n = 0;
    for (TopoDS_Iterator it(f); it.More(); it.Next())
      ++n;
    most_wires = std::max(most_wires, n);
  }
  const bool full_check = r.face_origin.size() / 3 <= 2000 && most_wires <= 256;
  // (An edge's range a rounding past its curve on a face (a hyperbola's piece whose end went to its vertex, 1e-6 on,
  // its curve on the cone OCCT's B-spline to the old end): narrowed to what its curves on its faces cover, as OCCT's
  // (an invalid range otherwise). Where the result is checked.)
  if (full_check)
  {
    TopTools_IndexedDataMapOfShapeListOfShape ef;
    TopExp::MapShapesAndAncestors(out, TopAbs_EDGE, TopAbs_FACE, ef);
    BRep_Builder fix;
    for (int k = 1; k <= ef.Extent(); ++k)
    {
      const TopoDS_Edge e = TopoDS::Edge(ef.FindKey(k));
      if (BRep_Tool::Degenerated(e))
        continue;
      double a = 0, z = 0;
      BRep_Tool::Range(e, a, z);
      double lo = a, hi = z;
      for (const TopoDS_Shape& fc : ef.FindFromIndex(k))
      {
        double c0 = 0, c1 = 0;
        const occ::handle<Geom2d_Curve> c = BRep_Tool::CurveOnSurface(e, TopoDS::Face(fc), c0, c1);
        if (c.IsNull() || c->IsPeriodic())
          continue;
        lo = std::max(lo, c->FirstParameter());
        hi = std::min(hi, c->LastParameter());
      }
      const double slack = 1e-5 * (1 + std::abs(a) + std::abs(z));
      if ((lo > a || hi < z) && lo - a <= slack && z - hi <= slack && lo < hi)
        fix.Range(e, lo, hi);
    }
  }
  // (A big one's on all threads: a 300-face gear fuse's took 0.2 s on one, half the Boolean's time.)
  if (full_check && !BRepCheck_Analyzer(out, true, t_built.f.size() > 32).IsValid())
    return decline("an invalid result");
  stop();
  if (noshape_should_break() != 0)
    return decline(nullptr);
  // (Past that: each made face on a cylinder or cone its wires closed on its surface (a curve's piece on the wrong
  // part of its curve on the face left a Ø25 x 25 knurl's cylinder pieces open there: an invalid result, 0.6 % of
  // its volume off).)
  if (!full_check)
    for (size_t i = 0; i < t_built.f.size(); ++i)
    {
      if (r.face_origin[3 * i] == 0)
        continue;
      const TopoDS_Face&               f = TopoDS::Face(t_built.f[i]);
      TopLoc_Location                  loc;
      const occ::handle<Geom_Surface>& sf = BRep_Tool::Surface(f, loc);
      const occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(sf);
      const occ::handle<Geom_ConicalSurface>     cn = occ::down_cast<Geom_ConicalSurface>(sf);
      if (cs.IsNull() && cn.IsNull())
        continue;
      // (Angles to lengths at its largest radius.)
      double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      const auto   cone_r = [&](double v) { return std::abs(cn->RefRadius() + v * std::sin(cn->SemiAngle())); };
      const double radius = !cs.IsNull() ? cs->Radius() : std::max(cone_r(v0), cone_r(v1));
      for (TopExp_Explorer wx(f, TopAbs_WIRE); wx.More(); wx.Next())
      {
        gp_Pnt2d first, last;
        bool     started = false;
        for (BRepTools_WireExplorer we(TopoDS::Wire(wx.Current()), f); we.More(); we.Next())
        {
          const TopoDS_Edge& e = we.Current();
          double a = 0, z = 0;
          const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, f, a, z);
          if (pc.IsNull())
            return decline("an edge without its curve on a curved face");
          const bool     fwd = e.Orientation() != TopAbs_REVERSED;
          const gp_Pnt2d p0 = pc->Value(fwd ? a : z), p1 = pc->Value(fwd ? z : a);
          const double   reach = 10 * (BRep_Tool::Tolerance(e) + tol);
          if (started && (std::abs(p0.X() - last.X()) * radius > reach || std::abs(p0.Y() - last.Y()) > reach))
            return decline("a face's curves don't close");
          if (!started)
            first = p0;
          started = true;
          last    = p1;
        }
        if (started && (std::abs(first.X() - last.X()) * radius > 10 * tol + 1e-6 || std::abs(first.Y() - last.Y()) > 10 * tol + 1e-6))
          return decline("a face's curves don't close");
      }
    }
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
    else if (std::getenv("NOSHAPE_SHELL_DEBUG"))
      std::fprintf(stderr, "quick_boolean: op %d of %d shapes: noshape's declined: %s\n", (int)op, (int)all.size(), rust_last_decline().c_str());
    // (NOSHAPE_BOOL_FALLBACK=0: a failed Boolean, as OCCT's failing; its caller goes on as it does then.)
    if (!done && brep_no_fallback())
      return false;
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

// OCCT patch 0028's hook (IntTools_FaceFace::Perform, every face / face section OCCT makes, its pave filler's too):
// a round touching a coaxial cylinder, cone, sphere or torus along one circle, that exact circle, where OCCT's marching
// near the tangency took seconds and broke it into pieces (a user's rib rounds, 4.9 s a pair on Windows); a plane
// containing an extrusion's direction, its rulings (below). Any other pair: 0, OCCT's own section as before.
extern "C" int noshape_face_face(const TopoDS_Face& theF1, const TopoDS_Face& theF2, NCollection_Sequence<IntTools_Curve>& theCurves)
{
  // (NOSHAPE_BOOL_OWN_FACE_FACE=0: OCCT's own everywhere, to compare.)
  static const bool off = std::getenv("NOSHAPE_BOOL_OWN_FACE_FACE") != nullptr && std::string(std::getenv("NOSHAPE_BOOL_OWN_FACE_FACE")) == "0";
  if (off)
    return 0;
  try
  {
    std::vector<IntTools_Curve> lines;
    bool                        touched = false;
    bool                        ours    = noshape::coaxial_sections(theF1, theF2, lines, &touched) && touched && !lines.empty();
    // A plane containing an extrusion's direction (a surface of linear extrusion, or a B-spline face straight along a
    // direction: an imported wall): the rulings there, exact lines as Parasolid's, where OCCT's marching bowed them (a
    // user's window cut through a Mac mini's corner wall, 10.0012 mm for 10). (Not a plane across one: from our curves
    // there (a gear's end face on its extruded teeth) OCCT's pave filler built broken results, cf-spur-gear's cuts.)
    if (!ours)
    {
      lines.clear();
      const occ::handle<Geom_Plane>          q1 = occ::down_cast<Geom_Plane>(BRep_Tool::Surface(theF1)), q2 = occ::down_cast<Geom_Plane>(BRep_Tool::Surface(theF2));
      const std::optional<noshape::Extruded> x  = q1.IsNull() == q2.IsNull() ? std::nullopt : q1.IsNull() ? noshape::extruded(theF1) : noshape::extruded(theF2);
      ours = x && std::abs((q1.IsNull() ? q2 : q1)->Pln().Axis().Direction().Dot(x->d)) <= 1e-12 && noshape::own_sections(theF1, theF2, lines);
    }
    if (!ours)
      return 0;
    const noshape::KernelTimer timer("occt.own_face_face");
    // (Bounded, as OCCT's own: its pave filler drops a curve without bounds, so a whole circle is a trimmed one; an
    // unbounded one left a full round's torus whole where live splits it along the bore.)
    for (IntTools_Curve& c : lines)
    {
      if (!c.HasBounds() && !c.Curve().IsNull() && !Precision::IsInfinite(c.Curve()->FirstParameter()) && !Precision::IsInfinite(c.Curve()->LastParameter()))
        c.SetCurves(new Geom_TrimmedCurve(c.Curve(), c.Curve()->FirstParameter(), c.Curve()->LastParameter()), c.FirstCurve2d(), c.SecondCurve2d());
      theCurves.Append(c);
    }
    return 1;
  }
  catch (...)
  {
    return 0;
  }
}
