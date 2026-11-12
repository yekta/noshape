// Geometry queries, mass properties, measurement.
#include "internal.hpp"
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <Poly_Triangulation.hxx>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRep_Builder.hxx>
#include <algorithm>
#include <BRepBndLib.hxx>
#include <OSD_Parallel.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <vector>
#include <algorithm>
#include <Extrema_POnSurf.hxx>
#include <Extrema_GenLocateExtPS.hxx>
#include <BRepTopAdaptor_FClass2d.hxx>
#include <BndLib_Add3dCurve.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <Precision.hxx>
#include <gp_Pln.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Face.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepLib_FindSurface.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <GProp_GProps.hxx>
#include <Geom_BSplineCurve.hxx>
#include <GeomConvert.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_BezierCurve.hxx>
#include <Geom_BezierSurface.hxx>
#include <Geom_OffsetSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_SurfaceOfLinearExtrusion.hxx>
#include <Geom_SurfaceOfRevolution.hxx>
#include <TopExp_Explorer.hxx>
#include <Geom2d_Curve.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Elips.hxx>
#include <gp_Lin.hxx>
#include <IntCurvesFace_ShapeIntersector.hxx>
#include <IntCurveSurface_TransitionOnCurve.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepExtrema_ShapeProximity.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <TColStd_PackedMapOfInteger.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS_Compound.hxx>
#include <atomic>
#include <ElSLib.hxx>
#include <optional>
#include <tuple>
#include <thread>
#include <set>

namespace noshape {

static SurfaceInfo empty_surface()
{
  SurfaceInfo i{};
  i.kind = 10;
  return i;
}

// A B-surface of degree 1 with two poles across one direction whose pole
// rows (and weights) are translates of each other: an extrusion of either
// row along `d`.
static bool bspline_extrusion(const occ::handle<Geom_BSplineSurface>& b, gp_Vec& d)
{
  for (int dir = 0; dir < 2; ++dir)
  {
    const bool v = dir == 0;
    if ((v ? b->VDegree() : b->UDegree()) != 1 || (v ? b->NbVPoles() : b->NbUPoles()) != 2)
      continue;
    const int n = v ? b->NbUPoles() : b->NbVPoles();
    auto      P = [&](int k, int j) { return v ? b->Pole(k, j) : b->Pole(j, k); };
    auto      W = [&](int k, int j) { return v ? b->Weight(k, j) : b->Weight(j, k); };
    d           = gp_Vec(P(1, 1), P(1, 2));
    const double len = d.Magnitude();
    if (len < 1e-12)
      continue;
    bool ok = true;
    for (int k = 1; k <= n && ok; ++k)
      ok = (gp_Vec(P(k, 1), P(k, 2)) - d).Magnitude() <= 1e-9 * (1 + len) && std::abs(W(k, 1) - W(k, 2)) <= 1e-12 * std::abs(W(k, 1));
    if (ok)
      return true;
  }
  return false;
}


SurfaceInfo surface_info(const OcShape& fs)
{
  const TopoDS_Face& f = as_face(fs.shape, "shape");
  if (is_mesh_face(f))
  {
    // A mesh face: no surface (Onshape's OTHER).
    SurfaceInfo m = empty_surface();
    m.reversed    = f.Orientation() == TopAbs_REVERSED;
    return m;
  }
  BRepAdaptor_Surface s(f, true);
  SurfaceInfo         i = empty_surface();
  i.reversed            = f.Orientation() == TopAbs_REVERSED;
  i.u_periodic          = s.IsUPeriodic();
  i.v_periodic          = s.IsVPeriodic();
  auto set_ax           = [&](const gp_Ax3& a) {
    i.origin = from_pnt(a.Location());
    i.axis   = from_dir(a.Direction());
    i.xdir   = from_dir(a.XDirection());
  };
  switch (s.GetType())
  {
    case GeomAbs_Plane: {
      i.kind = 0;
      set_ax(s.Plane().Position());
      break;
    }
    case GeomAbs_Cylinder: {
      i.kind = 1;
      set_ax(s.Cylinder().Position());
      i.r1 = s.Cylinder().Radius();
      break;
    }
    case GeomAbs_Cone: {
      i.kind = 2;
      gp_Cone c = s.Cone();
      set_ax(c.Position());
      i.r1    = c.RefRadius();
      i.angle = c.SemiAngle();
      // Store apex in xdir slot? No: keep origin = reference circle center and
      // report apex via r2 = signed distance from origin to apex along axis.
      gp_Pnt apex = c.Apex();
      i.r2        = gp_Vec(c.Location(), apex).Dot(gp_Vec(c.Axis().Direction()));
      break;
    }
    case GeomAbs_Sphere: {
      i.kind = 3;
      set_ax(s.Sphere().Position());
      i.r1 = s.Sphere().Radius();
      break;
    }
    case GeomAbs_Torus: {
      i.kind = 4;
      set_ax(s.Torus().Position());
      i.r1 = s.Torus().MajorRadius();
      i.r2 = s.Torus().MinorRadius();
      break;
    }
    case GeomAbs_BezierSurface: {
      i.kind                             = 5;
      occ::handle<Geom_BezierSurface> b = s.Bezier();
      i.u_degree                         = b->UDegree();
      i.v_degree                         = b->VDegree();
      i.nb_u_poles                       = b->NbUPoles();
      i.nb_v_poles                       = b->NbVPoles();
      i.rational                         = b->IsURational() || b->IsVRational();
      break;
    }
    case GeomAbs_BSplineSurface: {
      i.kind                              = 6;
      occ::handle<Geom_BSplineSurface> b = s.BSpline();
      // Straight in one direction with its rows translates of each other: an
      // extrusion, as Parasolid reports such a B-surface after import (live:
      // g50's F8, EXTRUDED).
      if (gp_Vec d; bspline_extrusion(b, d))
      {
        i.kind = 8;
        i.axis = from_dir(gp_Dir(d));
        break;
      }
      i.u_degree                          = b->UDegree();
      i.v_degree                          = b->VDegree();
      i.nb_u_poles                        = b->NbUPoles();
      i.nb_v_poles                        = b->NbVPoles();
      i.rational                          = b->IsURational() || b->IsVRational();
      break;
    }
    case GeomAbs_SurfaceOfRevolution: {
      i.kind    = 7;
      gp_Ax1 ax = s.AxeOfRevolution();
      i.origin  = from_pnt(ax.Location());
      i.axis    = from_dir(ax.Direction());
      // A circle revolved in a plane through the axis is a torus (a sphere
      // with its centre on the axis), as Parasolid reports it; the part of a
      // spindle torus nearer the axis than the circle's centre (the acorn
      // nut's crown, live) has a negative major radius, Parasolid's lemon.
      if (s.BasisCurve()->GetType() == GeomAbs_Circle)
      {
        const gp_Circ c  = s.BasisCurve()->Circle();
        const gp_Dir  D  = ax.Direction();
        const gp_Vec  N  = gp_Vec(c.Axis().Direction());
        const gp_Vec  pc(ax.Location(), c.Location());
        const double  R  = c.Radius();
        if (std::abs(N.Dot(gp_Vec(D))) < 1e-9 && std::abs(pc.Dot(N)) < 1e-9 * (1 + R))
        {
          const gp_Pnt O = ax.Location().Translated(gp_Vec(D) * pc.Dot(gp_Vec(D)));
          const gp_Vec r(O, c.Location());
          const double a = r.Magnitude();
          if (a < 1e-9 * R)
          {
            i.kind   = 3;
            i.origin = from_pnt(O);
            i.r1     = R;
            break;
          }
          double u0, u1, v0, v1;
          BRepTools::UVBounds(f, u0, u1, v0, v1);
          const gp_Pnt q   = s.Value((u0 + u1) / 2, (v0 + v1) / 2);
          const gp_Vec qo(O, q);
          const double h   = qo.Dot(gp_Vec(D));
          const double rho = (qo - gp_Vec(D) * h).Magnitude();
          const bool lemon = std::abs((rho + a) * (rho + a) + h * h - R * R) < std::abs((rho - a) * (rho - a) + h * h - R * R);
          i.kind   = 4;
          i.origin = from_pnt(O);
          i.xdir   = from_dir(gp_Dir(r));
          i.r1     = lemon ? -a : a;
          i.r2     = R;
        }
      }
      break;
    }
    case GeomAbs_SurfaceOfExtrusion: {
      i.kind = 8;
      i.axis = from_dir(s.Direction());
      break;
    }
    case GeomAbs_OffsetSurface: {
      i.kind = 9;
      i.r1   = s.OffsetValue();
      break;
    }
    default:
      i.kind = 10;
  }
  return i;
}

CurveInfo curve_info(const OcShape& es)
{
  const TopoDS_Edge& e = as_edge(es.shape, "shape");
  CurveInfo          i{};
  i.kind        = 8;
  i.degenerated = BRep_Tool::Degenerated(e);
  TopoDS_Vertex v1, v2;
  TopExp::Vertices(e, v1, v2, true);
  if (!v1.IsNull())
    i.start = from_pnt(BRep_Tool::Pnt(v1));
  if (!v2.IsNull())
    i.end = from_pnt(BRep_Tool::Pnt(v2));
  i.closed = !v1.IsNull() && v1.IsSame(v2);
  if (i.degenerated)
    return i;
  BRepAdaptor_Curve c(e);
  i.first    = c.FirstParameter();
  i.last     = c.LastParameter();
  i.periodic = c.IsPeriodic();
  i.length   = GCPnts_AbscissaPoint::Length(c);
  auto set_ax = [&](const gp_Ax2& a) {
    i.origin = from_pnt(a.Location());
    i.axis   = from_dir(a.Direction());
    i.xdir   = from_dir(a.XDirection());
  };
  switch (c.GetType())
  {
    case GeomAbs_Line: {
      i.kind    = 0;
      gp_Lin l  = c.Line();
      i.origin  = from_pnt(l.Location());
      gp_Dir d  = l.Direction();
      if (e.Orientation() == TopAbs_REVERSED)
        d.Reverse();
      i.axis = from_dir(d);
      break;
    }
    case GeomAbs_Circle: {
      i.kind = 1;
      set_ax(c.Circle().Position());
      i.r1 = c.Circle().Radius();
      break;
    }
    case GeomAbs_Ellipse: {
      i.kind = 2;
      set_ax(c.Ellipse().Position());
      i.r1 = c.Ellipse().MajorRadius();
      i.r2 = c.Ellipse().MinorRadius();
      break;
    }
    case GeomAbs_Hyperbola:
      i.kind = 3;
      break;
    case GeomAbs_Parabola:
      i.kind = 4;
      break;
    case GeomAbs_BezierCurve: {
      i.kind     = 5;
      i.degree   = c.Bezier()->Degree();
      i.nb_poles = c.Bezier()->NbPoles();
      i.rational = c.Bezier()->IsRational();
      break;
    }
    case GeomAbs_BSplineCurve: {
      i.kind     = 6;
      i.degree   = c.BSpline()->Degree();
      i.nb_poles = c.BSpline()->NbPoles();
      i.rational = c.BSpline()->IsRational();
      break;
    }
    case GeomAbs_OffsetCurve:
      i.kind = 7;
      break;
    default:
      i.kind = 8;
  }
  return i;
}

BSplineF edge_bspline(const OcShape& es)
{
  const TopoDS_Edge& e = as_edge(es.shape, "shape");
  if (BRep_Tool::Degenerated(e))
    fail_invalid("degenerated edge has no curve");
  TopLoc_Location           loc;
  double                    f = 0, l = 0;
  occ::handle<Geom_Curve>   c = BRep_Tool::Curve(e, loc, f, l);
  if (c.IsNull())
    fail_invalid("edge has no 3D curve");
  occ::handle<Geom_BSplineCurve> bs = occ::down_cast<Geom_BSplineCurve>(c);
  if (!bs.IsNull())
  {
    bs = occ::down_cast<Geom_BSplineCurve>(bs->Copy());
    // Restrict to the edge's range (keeps the curve exact).
    if (!bs->IsPeriodic() && (f > bs->FirstParameter() + 1e-12 || l < bs->LastParameter() - 1e-12))
      bs->Segment(f, l);
  }
  else
  {
    if (c->IsPeriodic() && l - f >= c->Period() - 1e-12)
      bs = GeomConvert::CurveToBSplineCurve(c);
    else
      bs = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(c, f, l));
  }
  if (bs.IsNull())
    fail_op("could not convert the edge curve to a B-spline");
  if (!loc.IsIdentity())
    bs->Transform(loc.Transformation());
  // Follow the edge direction.
  if (e.Orientation() == TopAbs_REVERSED)
    bs->Reverse();
  BSplineF out;
  out.degree   = bs->Degree();
  out.periodic = bs->IsPeriodic();
  for (int i = 1; i <= bs->NbPoles(); ++i)
  {
    const gp_Pnt& p = bs->Pole(i);
    out.poles.push_back(p.X());
    out.poles.push_back(p.Y());
    out.poles.push_back(p.Z());
    if (bs->IsRational())
      out.weights.push_back(bs->Weight(i));
  }
  for (int i = 1; i <= bs->NbKnots(); ++i)
  {
    out.knots.push_back(bs->Knot(i));
    out.mults.push_back(bs->Multiplicity(i));
  }
  return out;
}

// One direction of a B-spline surface in FeatureScript's form: the full knot
// vector (repeats written out); periodic, the knots unrolled so that
// knots[degree] starts the period (the inverse of surfacing.cpp's
// periodic_dir), with `degree` overlapping poles added by the caller.
static std::vector<double> fs_knots(const NCollection_Array1<double>& k, const NCollection_Array1<int>& m, int degree, bool periodic)
{
  std::vector<double> one; // one period from its start (open: all knots)
  for (int i = k.Lower(); i <= k.Upper(); ++i)
  {
    const bool last = i == k.Upper();
    if (periodic && last)
      break;
    for (int r = 0; r < m(i); ++r)
      one.push_back(k(i));
  }
  if (!periodic)
    return one;
  const double        period = k(k.Upper()) - k(k.Lower());
  const int           n      = (int)one.size(); // unique poles
  std::vector<double> out;
  for (int i = n - degree; i < n; ++i)
    out.push_back(one[(size_t)i] - period);
  out.insert(out.end(), one.begin(), one.end());
  for (int i = 0; i <= degree; ++i)
    out.push_back(one[(size_t)i] + period);
  return out;
}

BSplineSurfF face_bspline(const OcShape& fs)
{
  const TopoDS_Face&        f = as_face(fs.shape, "shape");
  TopLoc_Location           loc;
  occ::handle<Geom_Surface> s = BRep_Tool::Surface(f, loc);
  if (s.IsNull())
    fail_invalid("face has no surface");
  occ::handle<Geom_BSplineSurface> bs = occ::down_cast<Geom_BSplineSurface>(s);
  bs = bs.IsNull() ? GeomConvert::SurfaceToBSplineSurface(s) : occ::down_cast<Geom_BSplineSurface>(bs->Copy());
  if (bs.IsNull())
    fail_op("could not convert the face surface to a B-spline");
  if (!loc.IsIdentity())
    bs->Transform(loc.Transformation());
  BSplineSurfF out;
  out.u_degree   = bs->UDegree();
  out.v_degree   = bs->VDegree();
  out.u_periodic = bs->IsUPeriodic();
  out.v_periodic = bs->IsVPeriodic();
  const int pu = bs->NbUPoles(), pv = bs->NbVPoles();
  const int nu = pu + (out.u_periodic ? out.u_degree : 0), nv = pv + (out.v_periodic ? out.v_degree : 0);
  out.nu = (uint32_t)nu;
  out.nv = (uint32_t)nv;
  for (int i = 0; i < nu; ++i)
    for (int j = 0; j < nv; ++j)
    {
      const gp_Pnt& p = bs->Pole(i % pu + 1, j % pv + 1);
      out.poles.push_back(p.X());
      out.poles.push_back(p.Y());
      out.poles.push_back(p.Z());
      if (bs->IsURational() || bs->IsVRational())
        out.weights.push_back(bs->Weight(i % pu + 1, j % pv + 1));
    }
  for (double t : fs_knots(bs->UKnots(), bs->UMultiplicities(), out.u_degree, out.u_periodic))
    out.u_knots.push_back(t);
  for (double t : fs_knots(bs->VKnots(), bs->VMultiplicities(), out.v_degree, out.v_periodic))
    out.v_knots.push_back(t);
  return out;
}

P3 face_normal_at_uv(const OcShape& fs, double u, double v)
{
  const TopoDS_Face& f = as_face(fs.shape, "shape");
  BRepGProp_Face     gf(f); // accounts for face orientation
  gp_Pnt             p;
  gp_Vec             n;
  gf.Normal(u, v, p, n);
  if (n.Magnitude() < 1e-15)
  {
    // Singular point: fall back to local properties.
    BRepAdaptor_Surface s(f);
    BRepLProp_SLProps   props(s, u, v, 1, 1e-9);
    if (!props.IsNormalDefined())
      fail_op("normal is undefined at this point");
    gp_Dir d = props.Normal();
    if (f.Orientation() == TopAbs_REVERSED)
      d.Reverse();
    return from_dir(d);
  }
  n.Normalize();
  return from_vec(n);
}

P3 face_point_at_uv(const OcShape& fs, double u, double v)
{
  const TopoDS_Face&  f = as_face(fs.shape, "shape");
  BRepAdaptor_Surface s(f);
  return from_pnt(s.Value(u, v));
}

// The nearest point of a face's interior to `p`: the nearest samples of a
// UV grid inside the face, each refined by a local search. BRepExtrema's
// global search misses interior points on some B-spline faces (ruled ones:
// lofts, curved parting-line drafts) and falls back to the boundary.
// False when no interior point is found.
static bool face_interior_nearest(const TopoDS_Face& f, const gp_Pnt& p, double& u, double& v, gp_Pnt& q)
{
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  if (!(u1 > u0) || !(v1 > v0))
    return false;
  BRepAdaptor_Surface     s(f, true);
  BRepTopAdaptor_FClass2d cl(f, 1e-9);
  struct Sample
  {
    double d, u, v;
  };
  std::vector<Sample> grid;
  const int           N = 24;
  for (int i = 0; i <= N; ++i)
    for (int j = 0; j <= N; ++j)
    {
      const double uu = u0 + (u1 - u0) * (i + 0.5) / (N + 1), vv = v0 + (v1 - v0) * (j + 0.5) / (N + 1);
      grid.push_back({p.SquareDistance(s.Value(uu, vv)), uu, vv});
    }
  // The 4 nearest inside the face: classified nearest first, only until 4 are found (classifying all 625 was most of
  // a point-to-face distance: cf-surface-pattern's 11,700, 0.76 s).
  std::stable_sort(grid.begin(), grid.end(), [](const Sample& a, const Sample& b) { return a.d < b.d; });
  std::vector<Sample> samples;
  for (const Sample& g : grid)
  {
    if (cl.Perform(gp_Pnt2d(g.u, g.v)) != TopAbs_IN)
      continue;
    samples.push_back(g);
    if (samples.size() == 4)
      break;
  }
  if (samples.empty())
    return false;
  const size_t k = samples.size();
  double best = 1e300;
  for (size_t i = 0; i < k; ++i)
  {
    double bu = samples[i].u, bv = samples[i].v, bd = samples[i].d;
    Extrema_GenLocateExtPS loc(s, 1e-12, 1e-12);
    loc.Perform(p, bu, bv, true);
    if (loc.IsDone() && loc.SquareDistance() < bd)
    {
      double lu, lv;
      loc.Point().Parameter(lu, lv);
      const TopAbs_State st = cl.Perform(gp_Pnt2d(lu, lv));
      if (st == TopAbs_IN || st == TopAbs_ON)
      {
        bu = lu;
        bv = lv;
        bd = loc.SquareDistance();
      }
    }
    if (bd < best)
    {
      best = bd;
      u    = bu;
      v    = bv;
    }
  }
  q = s.Value(u, v);
  return true;
}

// The point of a B-spline face of many poles (a displacement map's 512 x 512 template) nearest `P`, or false where it
// doesn't apply: OCCT's projection evaluates a fine grid of the surface on every call (0.9 s each there). Here the
// surface's points at its knots and spans' middles are kept per surface (the faces a split leaves on it share them),
// the nearest few inside the face refined (a local extremum on the surface), and the face's edges measured.
static bool big_bspline_nearest(const TopoDS_Face& f, const gp_Pnt& P, ProjPoint& r)
{
  TopLoc_Location                        loc;
  const occ::handle<Geom_Surface>        gs = BRep_Tool::Surface(f, loc);
  const occ::handle<Geom_BSplineSurface> bs = occ::down_cast<Geom_BSplineSurface>(gs);
  if (bs.IsNull() || bs->NbUPoles() * bs->NbVPoles() < 4096)
    return false;
  struct Grid
  {
    occ::handle<Geom_Surface>          surf;
    TopLoc_Location                    loc;
    std::vector<std::array<double, 5>> pts;  // x, y, z, u, v
  };
  struct Face
  {
    TopoDS_Face                              face;
    std::unique_ptr<BRepAdaptor_Surface>     surf;
    std::unique_ptr<BRepTopAdaptor_FClass2d> cls;
    double                                   u0, u1, v0, v1;
  };
  thread_local std::vector<std::unique_ptr<Grid>> grids;  // the latest first
  thread_local std::vector<std::unique_ptr<Face>> faces;
  Grid*                                           g = nullptr;
  for (size_t i = 0; i < grids.size() && g == nullptr; ++i)
    if (grids[i]->surf == gs && grids[i]->loc.IsEqual(loc))
    {
      std::rotate(grids.begin(), grids.begin() + i, grids.begin() + i + 1);
      g = grids.front().get();
    }
  if (g == nullptr)
  {
    auto n  = std::make_unique<Grid>();
    n->surf = gs;
    n->loc  = loc;
    const gp_Trsf           t   = loc.Transformation();
    // (Each knot and each span's middle.)
    auto params = [](const NCollection_Array1<double>& k) {
      std::vector<double> out;
      for (int i = k.Lower(); i < k.Upper(); ++i)
      {
        out.push_back(k(i));
        out.push_back(0.5 * (k(i) + k(i + 1)));
      }
      out.push_back(k(k.Upper()));
      return out;
    };
    const std::vector<double> us = params(bs->UKnots()), vs = params(bs->VKnots());
    n->pts.reserve(us.size() * vs.size());
    for (double u : us)
    {
      // (2.5 M points at 704²: a cancel stops it between rows.)
      fail_if_cancelled();
      for (double v : vs)
      {
        const gp_Pnt q = bs->Value(u, v).Transformed(t);
        n->pts.push_back({q.X(), q.Y(), q.Z(), u, v});
      }
    }
    grids.insert(grids.begin(), std::move(n));
    if (grids.size() > 2)
      grids.pop_back();
    g = grids.front().get();
  }
  Face* fc = nullptr;
  for (size_t i = 0; i < faces.size() && fc == nullptr; ++i)
    if (faces[i]->face.IsEqual(f))
    {
      std::rotate(faces.begin(), faces.begin() + i, faces.begin() + i + 1);
      fc = faces.front().get();
    }
  if (fc == nullptr)
  {
    auto n  = std::make_unique<Face>();
    n->face = f;
    n->surf = std::make_unique<BRepAdaptor_Surface>(f, true);
    n->cls  = std::make_unique<BRepTopAdaptor_FClass2d>(f, 1e-9);
    BRepTools::UVBounds(f, n->u0, n->u1, n->v0, n->v1);
    faces.insert(faces.begin(), std::move(n));
    if (faces.size() > 8)
      faces.pop_back();
    fc = faces.front().get();
  }
  // The nearest samples within the face's bounds, then a local extremum from each inside it.
  constexpr size_t                         K  = 16;
  const double                             eu = 1e-9 * (1 + std::abs(fc->u1 - fc->u0)), ev = 1e-9 * (1 + std::abs(fc->v1 - fc->v0));
  std::array<std::pair<double, size_t>, K> best;
  best.fill({std::numeric_limits<double>::infinity(), 0});
  for (size_t i = 0; i < g->pts.size(); ++i)
  {
    const auto& q = g->pts[i];
    if (q[3] < fc->u0 - eu || q[3] > fc->u1 + eu || q[4] < fc->v0 - ev || q[4] > fc->v1 + ev)
      continue;
    const double d = (q[0] - P.X()) * (q[0] - P.X()) + (q[1] - P.Y()) * (q[1] - P.Y()) + (q[2] - P.Z()) * (q[2] - P.Z());
    if (d >= best[K - 1].first)
      continue;
    size_t k = K - 1;
    for (; k > 0 && best[k - 1].first > d; --k)
      best[k] = best[k - 1];
    best[k] = {d, i};
  }
  double bd = std::numeric_limits<double>::infinity(), bu = 0, bv = 0;
  for (const auto& [d, i] : best)
  {
    if (!std::isfinite(d))
      break;
    double u = g->pts[i][3], v = g->pts[i][4], dd = fc->cls->Perform(gp_Pnt2d(u, v)) == TopAbs_OUT ? std::numeric_limits<double>::infinity() : d;
    Extrema_GenLocateExtPS lx(*fc->surf, 1e-12, 1e-12);
    lx.Perform(P, u, v, true);
    if (lx.IsDone() && lx.SquareDistance() < dd)
    {
      double lu, lv;
      lx.Point().Parameter(lu, lv);
      if (fc->cls->Perform(gp_Pnt2d(lu, lv)) != TopAbs_OUT)
      {
        u  = lu;
        v  = lv;
        dd = lx.SquareDistance();
      }
    }
    if (dd < bd)
    {
      bd = dd;
      bu = u;
      bv = v;
    }
  }
  // The boundary: the nearest point of its edges (none nearer than a point on the face, nor an edge whose box is
  // farther than the inside's).
  const double        inside = std::isfinite(bd) ? std::sqrt(bd) : std::numeric_limits<double>::infinity();
  const TopoDS_Vertex vx     = BRepBuilderAPI_MakeVertex(P);
  double              ed     = std::numeric_limits<double>::infinity(), et = 0;
  gp_Pnt              eq;
  TopoDS_Edge         ee;
  ShapeIMap           es;
  if (inside > 1e-12)
    TopExp::MapShapes(f, TopAbs_EDGE, es);
  for (int i = 1; i <= es.Extent(); ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(es(i));
    if (BRep_Tool::Degenerated(e))
      continue;
    Bnd_Box b;
    BRepBndLib::Add(e, b);
    if (!b.IsVoid() && b.Distance(Bnd_Box(P, P)) >= std::min(inside, ed))
      continue;
    BRepExtrema_DistShapeShape de(vx, e);
    if (!de.IsDone() || de.NbSolution() < 1 || de.Value() >= ed)
      continue;
    ed = de.Value();
    eq = de.PointOnShape2(1);
    ee = e;
    if (de.SupportTypeShape2(1) == BRepExtrema_IsVertex)
      et = BRep_Tool::Parameter(TopoDS::Vertex(de.SupportOnShape2(1)), e);
    else
      de.ParOnEdgeS2(1, et);
  }
  if (std::isfinite(bd) && (std::sqrt(bd) <= ed + 1e-12 || ee.IsNull()))
  {
    const gp_Pnt bq = fc->surf->Value(bu, bv);
    r.point         = from_pnt(bq);
    r.distance      = bq.Distance(P);
    r.u             = bu;
    r.v             = bv;
    return true;
  }
  if (ee.IsNull())
    return false;
  r.point    = from_pnt(eq);
  r.distance = ed;
  double                          f0, l0;
  const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(ee, f, f0, l0);
  if (pc.IsNull())
    return false;
  const gp_Pnt2d uv = pc->Value(et);
  r.u               = uv.X();
  r.v               = uv.Y();
  return true;
}

// On an elementary surface: within `tol` of the surface, the foot inside the face or on its boundary (no distance to
// the boundary for a point off it: the projection's edge search took ~0.5 ms a call, a sheet metal Flange's thousands
// of "on this face?" probes 0.7 s). Free-form faces by the projection below.
bool face_contains_uv(const OcShape& fs, double u, double v, double tol)
{
  const TopoDS_Face&             f = as_face(fs.shape, "shape");
  const BRepClass_FaceClassifier fc(f, gp_Pnt2d(u, v), std::max(tol, BRep_Tool::Tolerance(f)));
  return fc.State() == TopAbs_IN || fc.State() == TopAbs_ON;
}

bool face_contains_point(const OcShape& fs, P3 p, double tol)
{
  const TopoDS_Face&        f    = as_face(fs.shape, "shape");
  const GeomAbs_SurfaceType kind = BRepAdaptor_Surface(f, false).GetType();
  if (kind == GeomAbs_Plane || kind == GeomAbs_Cylinder || kind == GeomAbs_Cone || kind == GeomAbs_Sphere || kind == GeomAbs_Torus)
  {
    const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(f);
    if (surf.IsNull())
      return false;
    GeomAPI_ProjectPointOnSurf pr(to_pnt(p), surf);
    if (!pr.IsDone() || pr.NbPoints() == 0 || pr.LowerDistance() > tol)
      return false;
    double u, v;
    pr.LowerDistanceParameters(u, v);
    BRepClass_FaceClassifier fc(f, gp_Pnt2d(u, v), std::max(tol, BRep_Tool::Tolerance(f)));
    return fc.State() == TopAbs_IN || fc.State() == TopAbs_ON;
  }
  // Free-form: the surface's feet within the face's parameter box (a point on the face has one at 0), each classified.
  {
    const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(f);
    if (surf.IsNull())
      return face_project_point(fs, p).distance <= tol;
    double u0, u1, v0, v1;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    GeomAPI_ProjectPointOnSurf pr(to_pnt(p), surf, u0, u1, v0, v1);
    if (!pr.IsDone())
      return face_project_point(fs, p).distance <= tol;
    for (int k = 1; k <= pr.NbPoints(); ++k)
    {
      if (pr.Distance(k) > tol)
        continue;
      double u, v;
      pr.Parameters(k, u, v);
      BRepClass_FaceClassifier fc(f, gp_Pnt2d(u, v), std::max(tol, BRep_Tool::Tolerance(f)));
      if (fc.State() == TopAbs_IN || fc.State() == TopAbs_ON)
        return true;
    }
    return false;
  }
}

ProjPoint face_project_point(const OcShape& fs, P3 p)
{
  const TopoDS_Face& f = as_face(fs.shape, "shape");
  // On an elementary surface, the foot on it when it is inside the face or on its boundary (an edge's midpoint, as a
  // Draft's tangent check asks): the nearest point there. (BRepExtrema below also measures the point against every
  // edge and vertex: 0.6 ms a call on a plate's face with a Fill pattern's 1,700 holes, ~1 ms on cf-circular-pattern's.
  // Not on free-form ones: their projection can stop at another extremum, PROJ-RULED.)
  const GeomAbs_SurfaceType kind = BRepAdaptor_Surface(f, false).GetType();
  if (kind == GeomAbs_Plane || kind == GeomAbs_Cylinder || kind == GeomAbs_Cone || kind == GeomAbs_Sphere || kind == GeomAbs_Torus)
  {
    const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(f);
    if (!surf.IsNull())
    {
      GeomAPI_ProjectPointOnSurf pr(to_pnt(p), surf);
      if (pr.IsDone() && pr.NbPoints() > 0)
      {
        double u, v;
        pr.LowerDistanceParameters(u, v);
        BRepClass_FaceClassifier fc(f, gp_Pnt2d(u, v), BRep_Tool::Tolerance(f));
        if (fc.State() == TopAbs_IN || fc.State() == TopAbs_ON)
        {
          ProjPoint r{};
          r.point    = from_pnt(pr.NearestPoint());
          r.distance = pr.LowerDistance();
          r.u        = u;
          r.v        = v;
          return r;
        }
      }
    }
  }
  if (kind == GeomAbs_BSplineSurface)
  {
    ProjPoint r{};
    if (big_bspline_nearest(f, to_pnt(p), r))
      return r;
  }
  // A point on a free-form face (a normal asked there: std sheet metal's rolled walls, 0.4 ms a call by the search
  // below): a foot on the surface at no distance, inside the face, is the nearest point (none can be nearer). A point
  // off it goes on below (another extremum could be nearer, PROJ-RULED).
  if (kind != GeomAbs_Plane && kind != GeomAbs_Cylinder && kind != GeomAbs_Cone && kind != GeomAbs_Sphere && kind != GeomAbs_Torus)
  {
    const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(f);
    double                          u0, u1, v0, v1;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    if (!surf.IsNull())
    {
      GeomAPI_ProjectPointOnSurf pr(to_pnt(p), surf, u0, u1, v0, v1);
      const double               tol = BRep_Tool::Tolerance(f);
      if (pr.IsDone() && pr.NbPoints() > 0 && pr.LowerDistance() <= tol)
      {
        double u, v;
        pr.LowerDistanceParameters(u, v);
        BRepClass_FaceClassifier fc(f, gp_Pnt2d(u, v), tol);
        if (fc.State() == TopAbs_IN || fc.State() == TopAbs_ON)
        {
          ProjPoint r{};
          r.point    = from_pnt(pr.NearestPoint());
          r.distance = pr.LowerDistance();
          r.u        = u;
          r.v        = v;
          return r;
        }
      }
    }
  }
  TopoDS_Vertex v = BRepBuilderAPI_MakeVertex(to_pnt(p));
  // Off a face of many edges (a plate's 1,700 holes): the nearest point of its edges, visited by their boxes' distance
  // (BRepExtrema measured every edge, 140 ms a call), unless an interior point is nearer (as below).
  {
    ShapeIMap es;
    TopExp::MapShapes(f, TopAbs_EDGE, es);
    if (es.Extent() >= 64)
    {
      const gp_Pnt                         P = to_pnt(p);
      std::vector<std::pair<double, int>> order;
      for (int i = 1; i <= es.Extent(); ++i)
      {
        if (BRep_Tool::Degenerated(TopoDS::Edge(es(i))))
          continue;
        Bnd_Box b;
        BRepBndLib::Add(es(i), b);
        if (b.IsVoid())
          continue;
        double x0, y0, z0, x1, y1, z1;
        b.Get(x0, y0, z0, x1, y1, z1);
        const double dx = std::max({x0 - P.X(), 0.0, P.X() - x1}), dy = std::max({y0 - P.Y(), 0.0, P.Y() - y1}), dz = std::max({z0 - P.Z(), 0.0, P.Z() - z1});
        order.emplace_back(std::sqrt(dx * dx + dy * dy + dz * dz), i);
      }
      std::sort(order.begin(), order.end());
      double      best = std::numeric_limits<double>::infinity(), bt = 0;
      gp_Pnt      bp;
      TopoDS_Edge be;
      for (const auto& [bd, i] : order)
      {
        if (bd > best)
          break;
        const TopoDS_Edge&         e = TopoDS::Edge(es(i));
        BRepExtrema_DistShapeShape de(v, e);
        if (!de.IsDone() || de.NbSolution() < 1 || de.Value() >= best)
          continue;
        best = de.Value();
        bp   = de.PointOnShape2(1);
        be   = e;
        if (de.SupportTypeShape2(1) == BRepExtrema_IsVertex)
          bt = BRep_Tool::Parameter(TopoDS::Vertex(de.SupportOnShape2(1)), e);
        else
          de.ParOnEdgeS2(1, bt);
      }
      if (!be.IsNull())
      {
        ProjPoint r{};
        double    iu, iv;
        gp_Pnt    iq;
        // (A plane's only interior candidate is the foot found outside above; that search took 220 ms there.)
        if (kind != GeomAbs_Plane && face_interior_nearest(f, P, iu, iv, iq) && iq.Distance(P) < best - 1e-12)
        {
          r.point    = from_pnt(iq);
          r.distance = iq.Distance(P);
          r.u        = iu;
          r.v        = iv;
          return r;
        }
        r.point    = from_pnt(bp);
        r.distance = best;
        double                          f0, l0;
        const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(be, f, f0, l0);
        if (!pc.IsNull())
        {
          const gp_Pnt2d uv = pc->Value(bt);
          r.u               = uv.X();
          r.v               = uv.Y();
        }
        return r;
      }
    }
  }
  BRepExtrema_DistShapeShape d(v, f);
  if (!d.IsDone() || d.NbSolution() < 1)
    fail_op("point projection onto face failed");
  ProjPoint r{};
  r.point    = from_pnt(d.PointOnShape2(1));
  r.distance = d.Value();
  // A boundary answer may hide a nearer interior point (see
  // face_interior_nearest).
  if (d.SupportTypeShape2(1) != BRepExtrema_IsInFace)
  {
    double iu, iv;
    gp_Pnt iq;
    if (face_interior_nearest(f, to_pnt(p), iu, iv, iq) && iq.Distance(to_pnt(p)) < d.Value() - 1e-12)
    {
      r.point    = from_pnt(iq);
      r.distance = iq.Distance(to_pnt(p));
      r.u        = iu;
      r.v        = iv;
      return r;
    }
  }
  if (d.SupportTypeShape2(1) == BRepExtrema_IsInFace)
  {
    d.ParOnFaceS2(1, r.u, r.v);
  }
  else
  {
    // On the boundary: take uv from the supporting edge pcurve / vertex.
    double umin, umax, vmin, vmax;
    BRepTools::UVBounds(f, umin, umax, vmin, vmax);
    r.u              = umin;
    r.v              = vmin;
    TopoDS_Shape sup = d.SupportOnShape2(1);
    if (!sup.IsNull() && sup.ShapeType() == TopAbs_EDGE)
    {
      double t;
      d.ParOnEdgeS2(1, t);
      double                    f0, l0;
      occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(TopoDS::Edge(sup), f, f0, l0);
      if (!pc.IsNull())
      {
        gp_Pnt2d uv = pc->Value(t);
        r.u         = uv.X();
        r.v         = uv.Y();
      }
    }
    else if (!sup.IsNull() && sup.ShapeType() == TopAbs_VERTEX)
    {
      gp_Pnt2d uv = BRep_Tool::Parameters(TopoDS::Vertex(sup), f);
      r.u         = uv.X();
      r.v         = uv.Y();
    }
  }
  return r;
}

// Point, first u derivative and outward normal (face orientation) at each (u, v) of `uvs`: 9 values a parameter, one
// adaptor for all (a Displacement map asks 262,144 tangent planes of one face).
rust::Vec<double> face_frames_at_uvs(const OcShape& fs, rust::Slice<const double> uvs)
{
  const TopoDS_Face&  f = as_face(fs.shape, "shape");
  BRepAdaptor_Surface s(f);
  const bool          reversed = f.Orientation() == TopAbs_REVERSED;
  rust::Vec<double>   r;
  r.reserve(uvs.size() / 2 * 9);
  for (size_t k = 0; k + 1 < uvs.size(); k += 2)
  {
    if ((k & 16383) == 0)
      fail_if_cancelled();
    const double u = uvs[k], v = uvs[k + 1];
    gp_Pnt       p;
    gp_Vec       du, dv;
    s.D1(u, v, p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (n.Magnitude() < 1e-15)
    {
      BRepLProp_SLProps props(s, u, v, 1, 1e-9);
      if (!props.IsNormalDefined())
        fail_op("normal is undefined at this point");
      n = gp_Vec(props.Normal());
    }
    else
      n.Normalize();
    if (reversed)
      n.Reverse();
    for (double x : {p.X(), p.Y(), p.Z(), du.X(), du.Y(), du.Z(), n.X(), n.Y(), n.Z()})
      r.push_back(x);
  }
  return r;
}

// First and second derivatives and the outward normal (face orientation) at each (u, v) of `uvs`: Su, Sv, Suu, Suv,
// Svv, N, 18 values a parameter (curvatures and their derivatives).
rust::Vec<double> face_d2_at_uvs(const OcShape& fs, rust::Slice<const double> uvs)
{
  const TopoDS_Face&  f = as_face(fs.shape, "shape");
  BRepAdaptor_Surface s(f);
  const bool          reversed = f.Orientation() == TopAbs_REVERSED;
  rust::Vec<double>   r;
  r.reserve(uvs.size() / 2 * 18);
  for (size_t k = 0; k + 1 < uvs.size(); k += 2)
  {
    if ((k & 16383) == 0)
      fail_if_cancelled();
    const double u = uvs[k], v = uvs[k + 1];
    gp_Pnt       p;
    gp_Vec       du, dv, duu, dvv, duv;
    s.D2(u, v, p, du, dv, duu, dvv, duv);
    gp_Vec n = du.Crossed(dv);
    if (n.Magnitude() < 1e-15)
    {
      BRepLProp_SLProps props(s, u, v, 1, 1e-9);
      if (!props.IsNormalDefined())
        fail_op("normal is undefined at this point");
      n = gp_Vec(props.Normal());
    }
    else
      n.Normalize();
    if (reversed)
      n.Reverse();
    for (const gp_Vec& w : {du, dv, duu, duv, dvv, n})
    {
      r.push_back(w.X());
      r.push_back(w.Y());
      r.push_back(w.Z());
    }
  }
  return r;
}

rust::Vec<double> face_uv_bounds(const OcShape& fs)
{
  const TopoDS_Face& f = as_face(fs.shape, "shape");
  double             umin, umax, vmin, vmax;
  BRepTools::UVBounds(f, umin, umax, vmin, vmax);
  rust::Vec<double> r;
  r.push_back(umin);
  r.push_back(umax);
  r.push_back(vmin);
  r.push_back(vmax);
  return r;
}

P3 edge_point_at(const OcShape& es, double t)
{
  const TopoDS_Edge& e = as_edge(es.shape, "shape");
  if (BRep_Tool::Degenerated(e))
    fail_invalid("edge is degenerated");
  BRepAdaptor_Curve c(e);
  return from_pnt(c.Value(t));
}

P3 edge_tangent_at(const OcShape& es, double t)
{
  const TopoDS_Edge& e = as_edge(es.shape, "shape");
  if (BRep_Tool::Degenerated(e))
    fail_invalid("edge is degenerated");
  BRepAdaptor_Curve c(e);
  gp_Pnt            p;
  gp_Vec            v;
  c.D1(t, p, v);
  if (v.Magnitude() < 1e-15)
    fail_op("tangent is undefined at this parameter");
  v.Normalize();
  if (e.Orientation() == TopAbs_REVERSED)
    v.Reverse();
  return from_vec(v);
}

rust::Vec<double> edge_derivatives_at(const OcShape& es, double t)
{
  const TopoDS_Edge& e = as_edge(es.shape, "shape");
  if (BRep_Tool::Degenerated(e))
    fail_invalid("edge is degenerated");
  BRepAdaptor_Curve c(e);
  gp_Pnt            p;
  gp_Vec            d1, d2, d3;
  c.D3(t, p, d1, d2, d3);
  rust::Vec<double> r;
  for (const gp_XYZ& x : {p.XYZ(), d1.XYZ(), d2.XYZ(), d3.XYZ()})
  {
    r.push_back(x.X());
    r.push_back(x.Y());
    r.push_back(x.Z());
  }
  return r;
}

P3 vertex_point(const OcShape& vs)
{
  const TopoDS_Vertex& v = as_vertex(vs.shape, "shape");
  return from_pnt(BRep_Tool::Pnt(v));
}

namespace
{
// Whether any face of `s` lies on a surface OCCT's optimal box isn't exact
// for (B-splines, sweeps, offsets).
bool has_free_form_face(const TopoDS_Shape& s)
{
  for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
    switch (BRepAdaptor_Surface(TopoDS::Face(e.Current()), false).GetType())
    {
      case GeomAbs_Plane:
      case GeomAbs_Cylinder:
      case GeomAbs_Cone:
      case GeomAbs_Sphere:
      case GeomAbs_Torus:
        break;
      default:
        return true;
    }
  return false;
}

// A face's box from what it holds, not its surface's whole parameter rectangle (OCCT's optimal box takes the
// rectangle: a wall run on past its edge and trimmed back, c2-impeller-loft's Shell inlet, reached 6.2 mm above the
// part). A free-form face: its edges exactly, never past OCCT's box, and on each side where OCCT's box passes the
// edges' box its own extreme there: the best of a grid of its points inside the face, climbed to the top within it.
static void face_box(const TopoDS_Face& f, Bnd_Box& out, NCollection_DataMap<TopoDS_Shape, Bnd_Box, TopTools_ShapeMapHasher>& edge_boxes)
{
  // (Planes, cylinders, cones, spheres and tori: OCCT's box as before; the extended surfaces are free-form.)
  switch (BRepAdaptor_Surface(f, false).GetType())
  {
    case GeomAbs_Plane:
    case GeomAbs_Cylinder:
    case GeomAbs_Cone:
    case GeomAbs_Sphere:
    case GeomAbs_Torus:
      BRepBndLib::AddOptimal(f, out, false, false);
      return;
    default:
      break;
  }
  // (Only past the edges' quick box (their poles'): the exact work where the surface reaches well out.)
  Bnd_Box whole;
  BRepBndLib::AddOptimal(f, whole, false, false);
  {
    Bnd_Box quick;
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
      if (!BRep_Tool::Degenerated(TopoDS::Edge(e.Current())))
        BndLib_Add3dCurve::Add(BRepAdaptor_Curve(TopoDS::Edge(e.Current())), 0.0, quick);
    if (quick.IsVoid() || whole.IsVoid())
    {
      out.Add(whole);
      return;
    }
    double q[6], w[6];
    quick.Get(q[0], q[1], q[2], q[3], q[4], q[5]);
    whole.Get(w[0], w[1], w[2], w[3], w[4], w[5]);
    const double t = 1e-4 * std::sqrt(whole.SquareExtent());
    bool         past = false;
    for (int i = 0; i < 3; ++i)
      past = past || w[i] < q[i] - t || w[i + 3] > q[i + 3] + t;
    if (!past)
    {
      out.Add(whole);
      return;
    }
  }
  Bnd_Box edges;
  for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
  {
    const TopoDS_Edge& ed = TopoDS::Edge(e.Current());
    if (BRep_Tool::Degenerated(ed))
      continue;
    // (An edge bounds two faces: boxed once.)
    if (!edge_boxes.IsBound(ed))
    {
      Bnd_Box eb;
      BndLib_Add3dCurve::AddOptimal(BRepAdaptor_Curve(ed), 0.0, eb);
      edge_boxes.Bind(ed, eb);
    }
    edges.Add(edge_boxes(ed));
  }
  BRepAdaptor_Surface s(f, false);
  if (edges.IsVoid() || whole.IsVoid())
  {
    out.Add(whole);
    out.Add(edges);
    return;
  }
  double a[6], b[6];
  edges.Get(a[0], a[1], a[2], a[3], a[4], a[5]);
  whole.Get(b[0], b[1], b[2], b[3], b[4], b[5]);
  // (Only a face reaching well past its edges: OCCT's small margins on rational faces are tighten_box's.)
  const double tol = 1e-4 * std::sqrt(whole.SquareExtent());
  // (OCCT's box holds the face: no side past it, the edges' curves lying off a face by their tolerance included.)
  double box[6];
  for (int i = 0; i < 3; ++i)
  {
    box[i]     = std::max(a[i], b[i]);
    box[i + 3] = std::min(a[i + 3], b[i + 3]);
  }
  std::optional<BRepTopAdaptor_FClass2d> inside;
  double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
  constexpr int n = 8;
  std::vector<std::pair<gp_Pnt2d, gp_Pnt>> grid;
  for (int side = 0; side < 6; ++side)
  {
    const int    axis = side % 3;
    const double sign = side < 3 ? -1.0 : 1.0;
    if (sign * (b[side] - a[side]) <= tol)
      continue;
    if (!inside)
    {
      inside.emplace(f, 1e-9);
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      for (int i = 0; i <= n; ++i)
        for (int j = 0; j <= n; ++j)
        {
          const gp_Pnt2d uv(u0 + (u1 - u0) * i / n, v0 + (v1 - v0) * j / n);
          if (inside->Perform(uv) == TopAbs_IN)
            grid.emplace_back(uv, s.Value(uv.X(), uv.Y()));
        }
    }
    // The best grid point along the side's direction, then up its slope while it stays inside.
    auto along = [&](const gp_Pnt& p) { return sign * p.Coord(axis + 1); };
    const std::pair<gp_Pnt2d, gp_Pnt>* best = nullptr;
    for (const auto& g : grid)
      if (!best || along(g.second) > along(best->second))
        best = &g;
    if (!best)
      continue;
    gp_Pnt2d uv   = best->first;
    double   top  = along(best->second);
    double   step = 0.5 * std::max((u1 - u0), (v1 - v0)) / n;
    for (int it = 0; it < 40 && step > 1e-6 * std::max(u1 - u0, v1 - v0); ++it)
    {
      gp_Pnt p;
      gp_Vec du, dv;
      s.D1(uv.X(), uv.Y(), p, du, dv);
      gp_Vec2d g(sign * du.Coord(axis + 1) * (u1 - u0), sign * dv.Coord(axis + 1) * (v1 - v0));
      if (g.Magnitude() < 1e-300)
        break;
      g.Normalize();
      const gp_Pnt2d next(uv.X() + g.X() * step * (u1 - u0) / std::max(u1 - u0, v1 - v0), uv.Y() + g.Y() * step * (v1 - v0) / std::max(u1 - u0, v1 - v0));
      if (inside->Perform(next) == TopAbs_IN && along(s.Value(next.X(), next.Y())) > top)
      {
        uv  = next;
        top = along(s.Value(next.X(), next.Y()));
      }
      else
        step *= 0.5;
    }
    box[side] = sign > 0 ? std::min(b[side], std::max(box[side], top)) : std::max(b[side], std::min(box[side], -top));
  }
  out.Update(box[0], box[1], box[2], box[3], box[4], box[5]);
}

// OCCT's optimal box of a free-form face reaches Precision::Confusion past
// short edges round it (BndLib_Add3dCurve's generic curve box: the corner
// patches of an imported filleted block, g47-1part: z 10.0001 mm for 10;
// qa5-routing BBOX-TOL). Each side is pulled in to the shape's exact extreme
// there, the distance to a plane just outside it, by at most a few
// Precision::Confusion (never out, never further).
void tighten_box(const TopoDS_Shape& s, double box[6])
{
  const double gap = 1e3 * Precision::Confusion(), most = 10 * Precision::Confusion();
  // The overshoot is past edges: their extremes, not the faces' (a long wavy strip's B-spline faces took 2.2 s a box
  // here, cf-wave-spring; a face's own extreme, OCCT's, is never pulled further than `most` by this).
  TopoDS_Compound rims;
  BRep_Builder    rb;
  rb.MakeCompound(rims);
  for (TopExp_Explorer e(s, TopAbs_EDGE); e.More(); e.Next())
    if (!BRep_Tool::Degenerated(TopoDS::Edge(e.Current())))
      rb.Add(rims, e.Current());
  for (TopExp_Explorer v(s, TopAbs_VERTEX, TopAbs_EDGE); v.More(); v.Next())
    rb.Add(rims, v.Current());
  if (rims.NbChildren() == 0)
    return;
  const double cx = (box[0] + box[3]) / 2, cy = (box[1] + box[4]) / 2, cz = (box[2] + box[5]) / 2;
  const double half = std::max({box[3] - box[0], box[4] - box[1], box[5] - box[2]}) + 2 * gap;
  for (int axis = 0; axis < 3; ++axis)
    for (int side = 0; side < 2; ++side)
    {
      double&      v = box[side * 3 + axis];
      const double at = side ? v + gap : v - gap;
      gp_Dir       n(axis == 0 ? 1 : 0, axis == 1 ? 1 : 0, axis == 2 ? 1 : 0);
      gp_Pnt       o(axis == 0 ? at : cx, axis == 1 ? at : cy, axis == 2 ? at : cz);
      const TopoDS_Face plane = BRepBuilderAPI_MakeFace(gp_Pln(o, n), -half, half, -half, half).Face();
      BRepExtrema_DistShapeShape d(rims, plane);
      if (!d.IsDone() || d.NbSolution() == 0)
        continue;
      const double exact = side ? at - d.Value() : at + d.Value();
      const double pull  = side ? v - exact : exact - v;
      if (pull > 0 && pull <= most)
        v = exact;
    }
}
} // namespace

rust::Vec<double> bounding_box(const OcShape& s, bool optimal)
{
  require_non_null(s.shape, "shape");
  {
    const LockedSection locked_section_685;
    std::lock_guard<std::mutex> g(s.myBoxLock);
    if (s.myBoxMade[optimal ? 1 : 0])
    {
      rust::Vec<double> r;
      for (double v : s.myBox[optimal ? 1 : 0])
        r.push_back(v);
      return r;
    }
  }
  Bnd_Box          b;
  const ShapeIMap& faces = s.map(TopAbs_FACE);
  if (optimal)
  {
    // Each face by what it holds (face_box); edges outside faces and vertices outside edges as OCCT's.
    NCollection_DataMap<TopoDS_Shape, Bnd_Box, TopTools_ShapeMapHasher> edge_boxes;
    for (int i = 1; i <= faces.Extent(); ++i)
      if (!is_mesh_face(TopoDS::Face(faces(i))))
        face_box(TopoDS::Face(faces(i)), b, edge_boxes);
    for (TopExp_Explorer e(s.shape, TopAbs_EDGE, TopAbs_FACE); e.More(); e.Next())
      BRepBndLib::AddOptimal(e.Current(), b, false, false);
    for (TopExp_Explorer v(s.shape, TopAbs_VERTEX, TopAbs_EDGE); v.More(); v.Next())
      BRepBndLib::AddOptimal(v.Current(), b, false, false);
  }
  else if (faces.Extent() >= 256)
  {
    // A big shape's faces boxed in runs on OCCT's pool, then joined (a box's gap joins as the largest, so this is
    // BRepBndLib::Add's box: 14 ms → ~3 on the 400-fin plate's 3206 faces, the display's first call on a new
    // part, psui); its edges outside faces and vertices outside edges as Add does them.
    constexpr int        run = 64;
    const int            n   = faces.Extent();
    std::vector<Bnd_Box> boxes((size_t)((n + run - 1) / run));
    {
      const KernelTimer timer("box.faces");
      OSD_Parallel::For(0, (int)boxes.size(), [&](int c) {
        for (int i = c * run + 1; i <= std::min(n, (c + 1) * run); ++i)
          BRepBndLib::Add(faces(i), boxes[(size_t)c], false);
      });
    }
    for (const Bnd_Box& x : boxes)
      b.Add(x);
    // A solid's (or solids') edges all bound its faces, and its vertices its edges: nothing outside to walk to.
    const KernelTimer  timer("box.rest");
    const TopAbs_ShapeEnum t      = s.shape.ShapeType();
    bool                   solids = t == TopAbs_SOLID || t == TopAbs_COMPSOLID;
    if (t == TopAbs_COMPOUND)
    {
      solids = true;
      for (TopoDS_Iterator it(s.shape); it.More() && solids; it.Next())
        solids = it.Value().ShapeType() == TopAbs_SOLID;
    }
    if (!solids)
    {
      for (TopExp_Explorer e(s.shape, TopAbs_EDGE, TopAbs_FACE); e.More(); e.Next())
        BRepBndLib::Add(e.Current(), b, false);
      for (TopExp_Explorer v(s.shape, TopAbs_VERTEX, TopAbs_EDGE); v.More(); v.Next())
        BRepBndLib::Add(v.Current(), b, false);
    }
  }
  else
    BRepBndLib::Add(s.shape, b, false);
  // Mesh faces have no surface: their triangulation's nodes.
  for (int i = 1; i <= faces.Extent(); ++i)
    if (is_mesh_face(TopoDS::Face(faces(i))))
    {
      TopLoc_Location                       l;
      const occ::handle<Poly_Triangulation> t = BRep_Tool::Triangulation(TopoDS::Face(faces(i)), l);
      for (int k = 1; k <= t->NbNodes(); ++k)
        b.Add(t->Node(k).Transformed(l.Transformation()));
    }
  if (b.IsVoid())
    fail_invalid("shape has no geometry (void bounding box)");
  double box[6];
  b.Get(box[0], box[1], box[2], box[3], box[4], box[5]);
  // (Wires too: a lone B-spline edge's box has the same margin.)
  if (optimal && !has_mesh_faces(s.shape) && (has_free_form_face(s.shape) || faces.Extent() == 0))
  {
    const KernelTimer timer("box.tighten");
    tighten_box(s.shape, box);
  }
  {
    const LockedSection locked_section_715;
    std::lock_guard<std::mutex> g(s.myBoxLock);
    for (int i = 0; i < 6; ++i)
      s.myBox[optimal ? 1 : 0][(size_t)i] = box[i];
    s.myBoxMade[optimal ? 1 : 0] = true;
  }
  rust::Vec<double> r;
  for (double v : box)
    r.push_back(v);
  return r;
}

PlaneQ find_plane(const OcShape& s, double tolerance)
{
  require_non_null(s.shape, "shape");
  PlaneQ q{};
  q.found = false;
  if (s.shape.ShapeType() == TopAbs_FACE)
  {
    BRepAdaptor_Surface a(TopoDS::Face(s.shape), true);
    if (a.GetType() == GeomAbs_Plane)
    {
      // A left-handed frame (a mirror's image) has its surface normal X × Y
      // along -Z.
      gp_Ax3 ax = a.Plane().Position();
      if (!ax.Direct())
        ax.ZReverse();
      if (s.shape.Orientation() == TopAbs_REVERSED)
        ax.ZReverse();
      q.found = true;
      q.plane = from_ax3(ax);
      return q;
    }
  }
  BRepLib_FindSurface fs(s.shape, tolerance > 0 ? tolerance : 1e-7, true);
  if (!fs.Found())
    return q;
  occ::handle<Geom_Plane> pl = occ::down_cast<Geom_Plane>(fs.Surface());
  if (pl.IsNull())
    return q;
  gp_Ax3 ax = pl->Position();
  if (!fs.Location().IsIdentity())
    ax.Transform(fs.Location().Transformation());
  // A planar face on another surface (a B-spline after a non-uniform scale):
  // the plane's normal along the face's outward normal, as for a Geom_Plane.
  if (s.shape.ShapeType() == TopAbs_FACE)
  {
    const TopoDS_Face& F = TopoDS::Face(s.shape);
    double         u0, u1, v0, v1;
    BRepTools::UVBounds(F, u0, u1, v0, v1);
    BRepGProp_Face gf(F);
    gp_Pnt         p;
    gp_Vec         n;
    gf.Normal(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, n);
    if (n.Magnitude() > 1e-12 && n.Dot(gp_Vec(ax.Direction())) < 0)
      ax.ZReverse();
  }
  q.found = true;
  q.plane = from_ax3(ax);
  return q;
}

static PropsF to_props(const GProp_GProps& g)
{
  PropsF p{};
  p.mass         = g.Mass();
  p.center       = from_pnt(g.CentreOfMass());
  gp_Mat m       = g.MatrixOfInertia();
  for (int r = 0; r < 3; ++r)
    for (int c = 0; c < 3; ++c)
      p.inertia[r * 3 + c] = m.Value(r + 1, c + 1);
  return p;
}

PropsF props_volume(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  return to_props(s.volume_props([&] { return volume_props(s.shape); }));
}

PropsF props_volume_precise(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  return to_props(volume_props_precise(s.shape));
}

PropsF props_surface(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  return to_props(s.surface_props([&] { return surface_props(s.shape); }));
}

PropsF props_linear(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  GProp_GProps g;
  BRepGProp::LinearProperties(s.shape, g, true);
  return to_props(g);
}

extern thread_local bool t_props_coarse;

// Bounds of the mass properties of `kind` (0 the volume's as shown (precise), 1 the surface's, 2 the length's): each
// value's difference from a second, coarser integration (segments twice as long, OCCT's adaptive tolerance 1000
// times looser; lengths: against GCPnts' adaptive length to 1e-6, the centre and inertia by the same share), at
// least 1e-12 of the value's scale. The integrands are smooth, so the values shown are well within.
PropsF props_bounds(const OcShape& s, uint8_t kind)
{
  require_non_null(s.shape, "shape");
  Bnd_Box box;
  BRepBndLib::Add(s.shape, box);
  const double size = box.IsVoid() ? 1.0 : std::sqrt(box.SquareExtent());
  PropsF       fine, coarse;
  if (kind == 2)
  {
    GProp_GProps g;
    BRepGProp::LinearProperties(s.shape, g, true);
    fine       = to_props(g);
    double len = 0;
    for (TopExp_Explorer x(s.shape, TopAbs_EDGE); x.More(); x.Next())
      if (!BRep_Tool::Degenerated(TopoDS::Edge(x.Current())))
      {
        BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
        len += GCPnts_AbscissaPoint::Length(c, 1e-6);
      }
    const double share = fine.mass > 0 ? std::abs(len - fine.mass) / fine.mass : 0;
    coarse             = fine;
    coarse.mass        = len;
    coarse.center.x += share * size;
    coarse.center.y += share * size;
    coarse.center.z += share * size;
    for (double& v : coarse.inertia)
      v *= 1 + share;
  }
  else
  {
    auto run = [&] { return to_props(kind == 0 ? volume_props_precise(s.shape) : surface_props(s.shape)); };
    fine           = run();
    t_props_coarse = true;
    try
    {
      coarse = run();
    }
    catch (...)
    {
      t_props_coarse = false;
      throw;
    }
    t_props_coarse = false;
  }
  PropsF b{};
  b.mass     = std::max(std::abs(fine.mass - coarse.mass), 1e-12 * std::abs(fine.mass));
  b.center.x = std::max(std::abs(fine.center.x - coarse.center.x), 1e-12 * size);
  b.center.y = std::max(std::abs(fine.center.y - coarse.center.y), 1e-12 * size);
  b.center.z = std::max(std::abs(fine.center.z - coarse.center.z), 1e-12 * size);
  double imax = 0;
  for (double v : fine.inertia)
    imax = std::max(imax, std::abs(v));
  for (int i = 0; i < 9; ++i)
    b.inertia[i] = std::max(std::abs(fine.inertia[i] - coarse.inertia[i]), 1e-12 * imax);
  return b;
}

DistanceF min_distance(const OcShape& a, const OcShape& b)
{
  require_non_null(a.shape, "first shape");
  require_non_null(b.shape, "second shape");
  BRepExtrema_DistShapeShape d(a.shape, b.shape);
  if (!d.IsDone() || d.NbSolution() < 1)
    fail_op("minimum distance computation failed");
  DistanceF r{};
  r.distance = d.Value();
  r.p1       = from_pnt(d.PointOnShape1(1));
  r.p2       = from_pnt(d.PointOnShape2(1));
  r.count    = d.NbSolution();
  // A point against faces: BRepExtrema can miss interior points of some
  // B-spline faces (face_interior_nearest); check the faces it may have.
  for (int side = 0; side < 2; ++side)
  {
    const TopoDS_Shape& pt  = side == 0 ? a.shape : b.shape;
    const TopoDS_Shape& oth = side == 0 ? b.shape : a.shape;
    if (pt.ShapeType() != TopAbs_VERTEX)
      continue;
    const gp_Pnt P = BRep_Tool::Pnt(TopoDS::Vertex(pt));
    for (TopExp_Explorer x(oth, TopAbs_FACE); x.More(); x.Next())
    {
      const TopoDS_Face f = TopoDS::Face(x.Current());
      if (BRepAdaptor_Surface(f).GetType() != GeomAbs_BSplineSurface)
        continue;
      Bnd_Box box;
      BRepBndLib::Add(f, box);
      if (box.Distance(Bnd_Box(P, P)) >= r.distance)
        continue;
      double u, v;
      gp_Pnt q;
      if (face_interior_nearest(f, P, u, v, q) && q.Distance(P) < r.distance - 1e-12)
      {
        r.distance = q.Distance(P);
        r.p1       = from_pnt(side == 0 ? P : q);
        r.p2       = from_pnt(side == 0 ? q : P);
        r.count    = 1;
      }
    }
  }
  return r;
}

// The minimum distance between two parts (Measure's Min dist): BRepExtrema's, without measuring every pair of their
// faces (two imported bodies of many B-spline faces that touch: 51 s). Their faces (and free edges and vertices) by
// their boxes; points on them (coarse meshes' nodes of copies, the edges' samples) give a distance the result can't
// exceed; of the pairs whose boxes come that close, those whose meshes do too are measured exactly, the nearest by
// their points first, on all threads in waves, until no box can come closer or two touch.
static DistanceF part_distance_faces(const OcShape& a, const OcShape& b)
{
  require_non_null(a.shape, "first shape");
  require_non_null(b.shape, "second shape");
  if (a.shape.ShapeType() == TopAbs_VERTEX || b.shape.ShapeType() == TopAbs_VERTEX)
    return min_distance(a, b);
  Bnd_Box all;
  BRepBndLib::Add(a.shape, all, false);
  BRepBndLib::Add(b.shape, all, false);
  if (all.IsVoid())
    return min_distance(a, b);
  const double defl = std::max(2e-3 * std::sqrt(all.SquareExtent()), 1e-7);
  struct Piece
  {
    TopoDS_Shape        s;
    Bnd_Box             box;
    std::vector<gp_Pnt> pts;
    bool                meshed = false;
  };
  auto pieces = [defl](const TopoDS_Shape& shape) {
    std::vector<Piece> v;
    const TopoDS_Shape c = BRepBuilderAPI_Copy(shape, false).Shape();
    auto add = [&](const TopoDS_Shape& x) {
      Piece p{x, {}, {}, false};
      BRepBndLib::Add(x, p.box, false);
      for (TopExp_Explorer e(x, TopAbs_VERTEX); e.More(); e.Next())
        p.pts.push_back(BRep_Tool::Pnt(TopoDS::Vertex(e.Current())));
      if (x.ShapeType() == TopAbs_EDGE && !BRep_Tool::Degenerated(TopoDS::Edge(x)))
      {
        const BRepAdaptor_Curve ac(TopoDS::Edge(x));
        for (int k = 1; k < 8; ++k)
          p.pts.push_back(ac.Value(ac.FirstParameter() + (ac.LastParameter() - ac.FirstParameter()) * k / 8.0));
      }
      v.push_back(std::move(p));
    };
    for (TopExp_Explorer x(c, TopAbs_FACE); x.More(); x.Next())
      add(x.Current());
    const size_t nf = v.size();
    for (TopExp_Explorer x(c, TopAbs_EDGE, TopAbs_FACE); x.More(); x.Next())
      add(x.Current());
    for (TopExp_Explorer x(c, TopAbs_VERTEX, TopAbs_EDGE); x.More(); x.Next())
      add(x.Current());
    // Meshed: faces of up to 32 wires (a plate's face with 1735 holes took 20 s to mesh); the others by their edges'
    // samples (on the face: still an upper bound), measured exactly.
    if (nf > 0)
    {
      BRep_Builder    bb;
      TopoDS_Compound some;
      bb.MakeCompound(some);
      bool any = false;
      for (size_t i = 0; i < nf; ++i)
      {
        int wires = 0;
        for (TopoDS_Iterator w(v[i].s); w.More() && wires <= 32; w.Next())
          ++wires;
        if (wires <= 32)
        {
          bb.Add(some, v[i].s);
          any = true;
        }
        else
          for (TopExp_Explorer e(v[i].s, TopAbs_EDGE); e.More(); e.Next())
          {
            const TopoDS_Edge& ed = TopoDS::Edge(e.Current());
            if (BRep_Tool::Degenerated(ed))
              continue;
            const BRepAdaptor_Curve ac(ed);
            for (int k = 0; k <= 4; ++k)
              v[i].pts.push_back(ac.Value(ac.FirstParameter() + (ac.LastParameter() - ac.FirstParameter()) * k / 4.0));
          }
      }
      if (any)
        BRepMesh_IncrementalMesh(some, defl, false, 0.5, true);
    }
    for (size_t i = 0; i < nf; ++i)
    {
      TopLoc_Location                       loc;
      const occ::handle<Poly_Triangulation> t = BRep_Tool::Triangulation(TopoDS::Face(v[i].s), loc);
      if (t.IsNull())
        continue;
      v[i].meshed = true;
      for (int k = 1; k <= t->NbNodes(); ++k)
        v[i].pts.push_back(t->Node(k).Transformed(loc.Transformation()));
    }
    return v;
  };
  const std::vector<Piece> pa = pieces(a.shape), pb = pieces(b.shape);
  if (pa.size() * pb.size() <= 400)
    return min_distance(a, b);
  // (Some of each one's points: an upper bound.)
  auto near_pts = [](const Piece& p, const Piece& q) {
    const size_t sp = std::max<size_t>(1, p.pts.size() / 96), sq = std::max<size_t>(1, q.pts.size() / 96);
    double       d2 = std::numeric_limits<double>::infinity();
    for (size_t i = 0; i < p.pts.size(); i += sp)
      for (size_t j = 0; j < q.pts.size(); j += sq)
        d2 = std::min(d2, p.pts[i].SquareDistance(q.pts[j]));
    return std::sqrt(d2);
  };
  struct Pair
  {
    double   bd, est;
    uint32_t i, j;
  };
  std::vector<Pair> pairs;
  pairs.reserve(pa.size() * pb.size());
  for (uint32_t i = 0; i < pa.size(); ++i)
    for (uint32_t j = 0; j < pb.size(); ++j)
      pairs.push_back({pa[i].box.Distance(pb[j].box), std::numeric_limits<double>::infinity(), i, j});
  // A planar face's plane is a lower bound too: the other piece's box wholly on one side of it, no nearer than its
  // nearest corner (two parallel plates: every pair with a plate's face ruled out).
  auto plane_of = [](const Piece& p) -> std::optional<gp_Pln> {
    if (p.s.ShapeType() != TopAbs_FACE)
      return std::nullopt;
    const BRepAdaptor_Surface sf(TopoDS::Face(p.s), false);
    if (sf.GetType() != GeomAbs_Plane)
      return std::nullopt;
    return sf.Plane();
  };
  auto side_gap = [](const gp_Pln& pl, const Bnd_Box& b) {
    if (b.IsVoid())
      return 0.0;
    double x0, y0, z0, x1, y1, z1;
    b.Get(x0, y0, z0, x1, y1, z1);
    double lo = std::numeric_limits<double>::infinity(), hi = -lo;
    const double a[2] = {x0, x1}, bb[2] = {y0, y1}, c[2] = {z0, z1};
    double A, B, C, D;
    pl.Coefficients(A, B, C, D);
    for (double x : a)
      for (double y : bb)
        for (double z : c)
        {
          const double d = A * x + B * y + C * z + D;
          lo = std::min(lo, d);
          hi = std::max(hi, d);
        }
    return lo > 0 ? lo : hi < 0 ? -hi : 0.0;
  };
  {
    std::vector<std::optional<gp_Pln>> qa(pa.size()), qb(pb.size());
    for (size_t i = 0; i < pa.size(); ++i)
      qa[i] = plane_of(pa[i]);
    for (size_t j = 0; j < pb.size(); ++j)
      qb[j] = plane_of(pb[j]);
    for (Pair& x : pairs)
    {
      if (qa[x.i])
        x.bd = std::max(x.bd, side_gap(*qa[x.i], pb[x.j].box));
      if (qb[x.j])
        x.bd = std::max(x.bd, side_gap(*qb[x.j], pa[x.i].box));
    }
  }
  std::sort(pairs.begin(), pairs.end(), [](const Pair& x, const Pair& y) { return std::tie(x.bd, x.i, x.j) < std::tie(y.bd, y.i, y.j); });
  double upper = std::numeric_limits<double>::infinity();
  size_t seen  = 0;
  for (; seen < pairs.size() && pairs[seen].bd <= upper; ++seen)
  {
    pairs[seen].est = near_pts(pa[pairs[seen].i], pb[pairs[seen].j]);
    upper           = std::min(upper, pairs[seen].est);
  }
  std::vector<Pair> cand;
  for (size_t k = 0; k < seen; ++k)
    if (pairs[k].bd <= upper)
      cand.push_back(pairs[k]);
  // Of two meshed faces, only those whose meshes come within `reach` (and the meshes' error): the meshes as a lower
  // bound.
  auto near_meshes = [&](std::vector<Pair>& list, double reach) {
    std::vector<int> ia(pa.size(), -1), ib(pb.size(), -1);
    BRep_Builder     bb;
    TopoDS_Compound  ca, cb;
    bb.MakeCompound(ca);
    bb.MakeCompound(cb);
    std::vector<uint32_t> fa, fb;
    for (const Pair& x : list)
    {
      if (pa[x.i].meshed && ia[x.i] < 0)
      {
        ia[x.i] = static_cast<int>(fa.size());
        fa.push_back(x.i);
        bb.Add(ca, pa[x.i].s);
      }
      if (pb[x.j].meshed && ib[x.j] < 0)
      {
        ib[x.j] = static_cast<int>(fb.size());
        fb.push_back(x.j);
        bb.Add(cb, pb[x.j].s);
      }
    }
    // (Past 2000 pairs the meshes' overlap costs more than it saves: two plates of 1735 holes 0.3 mm apart, 376 s.)
    if (fa.empty() || fb.empty() || list.size() > 2000)
      return;
    BRepExtrema_ShapeProximity prox(ca, cb, reach + 10 * defl);
    prox.Perform(); // not "done" when no faces overlap
    std::set<std::pair<uint32_t, uint32_t>> close;
    for (NCollection_DataMap<int, TColStd_PackedMapOfInteger>::Iterator x(prox.OverlapSubShapes1()); x.More(); x.Next())
      for (TColStd_PackedMapOfInteger::Iterator y(x.Value()); y.More(); y.Next())
        close.emplace(fa[static_cast<size_t>(x.Key())], fb[static_cast<size_t>(y.Key())]);
    std::vector<Pair> kept;
    for (const Pair& x : list)
      if (!pa[x.i].meshed || !pb[x.j].meshed || close.count({x.i, x.j}) != 0)
        kept.push_back(x);
    list = std::move(kept);
  };
  near_meshes(cand, upper);
  // Order: pairs whose meshes touch first (a contact ends it), of them those of planes, cylinders, cones, spheres
  // and tori first (BRepExtrema's quick ones), then by their points' distance.
  std::vector<Pair> touching = cand;
  if (cand.size() <= 2000)
    near_meshes(touching, 0.0);
  else
    touching.clear();
  std::set<std::pair<uint32_t, uint32_t>> touch;
  for (const Pair& x : touching)
    touch.emplace(x.i, x.j);
  auto slow = [](const Piece& p) {
    if (p.s.ShapeType() != TopAbs_FACE)
      return 0;
    const GeomAbs_SurfaceType t = BRepAdaptor_Surface(TopoDS::Face(p.s), false).GetType();
    return t == GeomAbs_Plane || t == GeomAbs_Cylinder || t == GeomAbs_Cone || t == GeomAbs_Sphere || t == GeomAbs_Torus ? 0 : 1;
  };
  auto rank = [&](const Pair& x) { return std::make_tuple(touch.count({x.i, x.j}) == 0, slow(pa[x.i]) + slow(pb[x.j]), x.est, x.bd, x.i, x.j); };
  std::sort(cand.begin(), cand.end(), [&](const Pair& x, const Pair& y) { return rank(x) < rank(y); });
  struct Found
  {
    bool   done = false;
    double d    = std::numeric_limits<double>::infinity();
    gp_Pnt p1, p2;
  };
  Found      best;
  const auto threads = static_cast<size_t>(std::max(1u, std::thread::hardware_concurrency()));
  // Two parallel planar faces: no nearer than their planes' gap, and exactly that where a point of one (a mesh node or
  // an edge's sample) projects inside the other (two plates of 1735 holes 0.3 mm apart: BRepExtrema took minutes).
  auto parallel_planes = [](const Piece& p, const Piece& q, Found& out) {
    if (p.s.ShapeType() != TopAbs_FACE || q.s.ShapeType() != TopAbs_FACE)
      return false;
    const BRepAdaptor_Surface sp(TopoDS::Face(p.s), false), sq(TopoDS::Face(q.s), false);
    if (sp.GetType() != GeomAbs_Plane || sq.GetType() != GeomAbs_Plane)
      return false;
    const gp_Pln P = sp.Plane(), Q = sq.Plane();
    if (!P.Axis().Direction().IsParallel(Q.Axis().Direction(), 1e-12))
      return false;
    for (int side = 0; side < 2; ++side)
    {
      const Piece&       from = side == 0 ? p : q;
      const gp_Pln&      onto = side == 0 ? Q : P;
      const TopoDS_Face& f    = TopoDS::Face(side == 0 ? q.s : p.s);
      std::optional<BRepTopAdaptor_FClass2d> cls;
      const size_t step = std::max<size_t>(1, from.pts.size() / 512);
      for (size_t n = 0; n < from.pts.size(); n += step)
      {
        const gp_Pnt& x = from.pts[n];
        double        u, v;
        ElSLib::Parameters(onto, x, u, v);
        if (!cls)
          cls.emplace(f, 1e-9);
        const TopAbs_State st = cls->Perform(gp_Pnt2d(u, v));
        if (st != TopAbs_IN && st != TopAbs_ON)
          continue;
        const gp_Pnt y = ElSLib::Value(u, v, onto);
        out            = Found{true, x.Distance(y), side == 0 ? x : y, side == 0 ? y : x};
        return true;
      }
    }
    return false;
  };
  // A contact first, cheaply: a mesh node of one face (a point on it) on the other's surface and inside it. (Two
  // planes touching measured through their hundreds of edges: 2.7 s.)
  {
    std::vector<Pair> list;
    for (const Pair& x : cand)
      if (touch.count({x.i, x.j}) != 0 && pa[x.i].s.ShapeType() == TopAbs_FACE && pb[x.j].s.ShapeType() == TopAbs_FACE)
        list.push_back(x);
    // A point of `from` on face `to` (within 1e-9 of its surface, inside it), if any.
    auto lands = [](const Piece& from, const Piece& to, gp_Pnt& at) {
      const TopoDS_Face&  f = TopoDS::Face(to.s);
      BRepAdaptor_Surface sf(f, false);
      Bnd_Box             near_box = to.box;
      near_box.Enlarge(1e-9);
      std::optional<GeomAPI_ProjectPointOnSurf> pr;
      std::optional<BRepTopAdaptor_FClass2d>    cls;
      const size_t step = std::max<size_t>(1, from.pts.size() / 256);
      for (size_t n = 0; n < from.pts.size(); n += step)
      {
        const gp_Pnt& p = from.pts[n];
        if (near_box.IsOut(p))
          continue;
        double u, v;
        if (sf.GetType() == GeomAbs_Plane)
        {
          const gp_Pln pl = sf.Plane();
          if (pl.Distance(p) > 1e-9)
            continue;
          ElSLib::Parameters(pl, p, u, v);
        }
        else
        {
          if (!pr)
          {
            pr.emplace();
            pr->Init(BRep_Tool::Surface(f), sf.FirstUParameter(), sf.LastUParameter(), sf.FirstVParameter(), sf.LastVParameter());
          }
          pr->Perform(p);
          if (!pr->IsDone() || pr->NbPoints() == 0 || pr->LowerDistance() > 1e-9)
            continue;
          pr->LowerDistanceParameters(u, v);
        }
        if (!cls)
          cls.emplace(f, 1e-9);
        const TopAbs_State st = cls->Perform(gp_Pnt2d(u, v));
        if (st == TopAbs_IN || st == TopAbs_ON)
        {
          at = p;
          return true;
        }
      }
      return false;
    };
    for (size_t from = 0, wave = 1; from < list.size(); from += wave, wave = threads)
    {
      const size_t        n = std::min(wave, list.size() - from);
      std::vector<int>    hit(n, 0);
      std::vector<gp_Pnt> at(n);
      OSD_Parallel::For(0, static_cast<int>(n), [&](int k) {
        const Pair& x = list[from + (size_t)k];
        hit[(size_t)k] = lands(pa[x.i], pb[x.j], at[(size_t)k]) || lands(pb[x.j], pa[x.i], at[(size_t)k]);
      });
      for (size_t k = 0; k < n; ++k)
        if (hit[k] != 0)
        {
          DistanceF r{};
          r.distance = 0.0;
          r.p1       = from_pnt(at[k]);
          r.p2       = from_pnt(at[k]);
          r.count    = 1;
          return r;
        }
    }
  }
  // (The likeliest pair alone first: a contact found there ends it in its time, not the slowest of a wave's.)
  size_t wave_size = 1;
  while (!cand.empty() && best.d > 1e-9)
  {
    const size_t       n = std::min(wave_size, cand.size());
    std::vector<Found> got(n);
    OSD_Parallel::For(0, static_cast<int>(n), [&](int k) {
      if (parallel_planes(pa[cand[(size_t)k].i], pb[cand[(size_t)k].j], got[(size_t)k]))
        return;
      BRepExtrema_DistShapeShape d;
      d.SetFlag(Extrema_ExtFlag_MIN);
      d.LoadS1(pa[cand[(size_t)k].i].s);
      d.LoadS2(pb[cand[(size_t)k].j].s);
      if (d.Perform() && d.NbSolution() > 0)
        got[(size_t)k] = Found{true, d.Value(), d.PointOnShape1(1), d.PointOnShape2(1)};
    });
    const double before = best.d;
    for (size_t k = 0; k < n; ++k)
    {
      // (A pair BRepExtrema couldn't do, that might have been nearer: the whole of them as before.)
      if (!got[k].done)
        return min_distance(a, b);
      if (got[k].d < best.d)
        best = got[k];
    }
    cand.erase(cand.begin(), cand.begin() + static_cast<std::ptrdiff_t>(n));
    // The rest that can still come nearer: by their boxes, then (once nearer) their meshes.
    // (Within the boxes' own margin of the best, the shapes' tolerance they were grown by: what such a pair could
    // still win is below that.)
    cand.erase(std::remove_if(cand.begin(), cand.end(), [&](const Pair& x) { return x.bd >= best.d - pa[x.i].box.GetGap() - pb[x.j].box.GetGap() - 1e-9; }), cand.end());
    if (best.d < before && cand.size() > threads)
      near_meshes(cand, best.d);
    wave_size = threads;
  }
  if (!best.done)
    return min_distance(a, b);
  DistanceF r{};
  r.distance = best.d;
  r.p1       = from_pnt(best.p1);
  r.p2       = from_pnt(best.p2);
  r.count    = 1;
  return r;
}

DistanceF part_distance(const OcShape& a, const OcShape& b)
{
  require_non_null(a.shape, "first shape");
  require_non_null(b.shape, "second shape");
  DistanceF r{};
  try
  {
    r = part_distance_faces(a, b);
  }
  catch (const Standard_Failure&)
  {
    // (A face without a surface and the like: the whole of them as before.)
    return min_distance(a, b);
  }
  // One solid inside the other: 0, at a vertex of the inner one (BRepExtrema's inner solution), where their
  // boundaries are apart.
  if (r.distance > 1e-9)
    for (int side = 0; side < 2; ++side)
    {
      const TopoDS_Shape& in  = side == 0 ? a.shape : b.shape;
      const TopoDS_Shape& out = side == 0 ? b.shape : a.shape;
      if (!TopExp_Explorer(out, TopAbs_SOLID).More())
        continue;
      TopExp_Explorer v(in, TopAbs_VERTEX);
      if (!v.More())
        continue;
      const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(v.Current()));
      for (TopExp_Explorer so(out, TopAbs_SOLID); so.More(); so.Next())
      {
        BRepClass3d_SolidClassifier cls(so.Current(), p, 1e-7);
        if (cls.State() == TopAbs_IN)
        {
          r.distance = 0;
          r.p1 = r.p2 = from_pnt(p);
          r.count    = 1;
          return r;
        }
      }
    }
  return r;
}

// Whether two shapes come within `tol` of each other, measured until the first pair that does: their pieces (faces,
// free edges, free vertices) whose boxes meet; of two faces, only those whose coarse meshes (of copies) come within
// the meshes' error. Parts far apart in most places then cost the meshing, not an exact distance per face pair.
bool shapes_touch(const OcShape& a, const OcShape& b, double tol)
{
  require_non_null(a.shape, "first shape");
  require_non_null(b.shape, "second shape");
  struct Piece
  {
    TopoDS_Shape s;
    Bnd_Box      box;
  };
  auto pieces = [tol](const TopoDS_Shape& s) {
    std::vector<Piece> v;
    auto add = [&](TopExp_Explorer x) {
      for (; x.More(); x.Next())
      {
        Piece p{x.Current(), {}};
        BRepBndLib::Add(p.s, p.box);
        p.box.Enlarge(tol);
        v.push_back(p);
      }
    };
    add(TopExp_Explorer(s, TopAbs_FACE));
    add(TopExp_Explorer(s, TopAbs_EDGE, TopAbs_FACE));
    add(TopExp_Explorer(s, TopAbs_VERTEX, TopAbs_EDGE));
    return v;
  };
  const std::vector<Piece> pa = pieces(a.shape), pb = pieces(b.shape);
  std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> exact;
  std::vector<char> face_a(pa.size(), 0), face_b(pb.size(), 0);
  Bnd_Box reach;
  for (size_t i = 0; i < pa.size(); ++i)
    for (size_t j = 0; j < pb.size(); ++j)
    {
      if (pa[i].box.IsOut(pb[j].box))
        continue;
      if (pa[i].s.ShapeType() != TopAbs_FACE || pb[j].s.ShapeType() != TopAbs_FACE)
      {
        exact.emplace_back(pa[i].s, pb[j].s);
        continue;
      }
      face_a[i] = face_b[j] = 1;
      reach.Add(pa[i].box);
      reach.Add(pb[j].box);
    }
  if (!reach.IsVoid())
  {
    // Copies (sharing the geometry) take the meshes; a face that doesn't mesh is measured against each it may touch.
    const double defl = std::max(1e-3 * std::sqrt(reach.SquareExtent()), 10 * tol);
    auto meshed = [defl](const std::vector<Piece>& p, const std::vector<char>& used, TopoDS_Compound& with, std::vector<TopoDS_Shape>& without, std::vector<size_t>& without_at) {
      BRep_Builder bb;
      TopoDS_Compound c;
      bb.MakeCompound(c);
      std::vector<size_t> at;
      for (size_t i = 0; i < p.size(); ++i)
        if (used[i])
        {
          bb.Add(c, p[i].s);
          at.push_back(i);
        }
      BRepBuilderAPI_Copy copy(c, false);
      BRepMesh_IncrementalMesh(copy.Shape(), defl, false, 0.5, true);
      bb.MakeCompound(with);
      size_t k = 0;
      for (TopoDS_Iterator x(copy.Shape()); x.More(); x.Next(), ++k)
      {
        TopLoc_Location loc;
        if (BRep_Tool::Triangulation(TopoDS::Face(x.Value()), loc).IsNull())
        {
          without.push_back(x.Value());
          without_at.push_back(at[k]);
        }
        else
          bb.Add(with, x.Value());
      }
    };
    TopoDS_Compound ma, mb;
    std::vector<TopoDS_Shape> ua, ub;
    std::vector<size_t> ua_at, ub_at;
    meshed(pa, face_a, ma, ua, ua_at);
    meshed(pb, face_b, mb, ub, ub_at);
    BRepExtrema_ShapeProximity prox(ma, mb, tol + 10 * defl);
    prox.Perform(); // not "done" when no faces overlap
    for (NCollection_DataMap<int, TColStd_PackedMapOfInteger>::Iterator x(prox.OverlapSubShapes1()); x.More(); x.Next())
      for (TColStd_PackedMapOfInteger::Iterator y(x.Value()); y.More(); y.Next())
        exact.emplace_back(prox.GetSubShape1(x.Key()), prox.GetSubShape2(y.Key()));
    for (size_t k = 0; k < ua.size(); ++k)
      for (size_t j = 0; j < pb.size(); ++j)
        if (face_b[j] && !pa[ua_at[k]].box.IsOut(pb[j].box))
          exact.emplace_back(ua[k], pb[j].s);
    for (size_t k = 0; k < ub.size(); ++k)
      for (size_t i = 0; i < pa.size(); ++i)
        if (face_a[i] && !pb[ub_at[k]].box.IsOut(pa[i].box))
          exact.emplace_back(pa[i].s, ub[k]);
  }
  std::atomic<bool> found{false};
  OSD_Parallel::For(0, (int)exact.size(), [&](int k) {
    if (found.load(std::memory_order_relaxed))
      return;
    BRepExtrema_DistShapeShape d;
    d.SetFlag(Extrema_ExtFlag_MIN);
    d.LoadS1(exact[(size_t)k].first);
    d.LoadS2(exact[(size_t)k].second);
    if (d.Perform() && d.Value() <= tol)
      found = true;
  });
  return found;
}

// Direction of a planar face normal or linear edge.
static bool linear_direction(const TopoDS_Shape& s, gp_Dir& d, bool& is_plane)
{
  if (s.ShapeType() == TopAbs_FACE)
  {
    BRepAdaptor_Surface a(TopoDS::Face(s), true);
    if (a.GetType() != GeomAbs_Plane)
      return false;
    d = a.Plane().Axis().Direction();
    if (!a.Plane().Position().Direct())
      d.Reverse();
    if (s.Orientation() == TopAbs_REVERSED)
      d.Reverse();
    is_plane = true;
    return true;
  }
  if (s.ShapeType() == TopAbs_EDGE)
  {
    BRepAdaptor_Curve c(TopoDS::Edge(s));
    if (c.GetType() != GeomAbs_Line)
      return false;
    d = c.Line().Direction();
    if (s.Orientation() == TopAbs_REVERSED)
      d.Reverse();
    is_plane = false;
    return true;
  }
  return false;
}

double angle_between(const OcShape& a, const OcShape& b)
{
  require_non_null(a.shape, "first shape");
  require_non_null(b.shape, "second shape");
  gp_Dir da, db;
  bool   pa = false, pb = false;
  if (!linear_direction(a.shape, da, pa) || !linear_direction(b.shape, db, pb))
    fail_invalid("angle requires planar faces or linear edges");
  double ang = da.Angle(db); // 0..pi
  if (pa != pb)
  {
    // plane vs line: angle between line and plane = 90deg - angle(normal, line)
    ang = std::abs(M_PI / 2.0 - ang);
  }
  return ang;
}

// The line p + t dir (t in [0, len]) against the faces of s: per crossing
// t, the transition (1 into the material, -1 out of it, 0 tangent) and
// whether it is on a face's boundary (1) or inside the face (0).
rust::Vec<double> line_crossings(const OcShape& s, P3 p, P3 dir, double len)
{
  require_non_null(s.shape, "shape");
  const gp_Vec d(dir.x, dir.y, dir.z);
  if (d.Magnitude() < 1e-12 || !(len > 0))
    fail_invalid("line needs a direction and a length");
  const LockedSection locked_section_902;
  std::lock_guard<std::mutex> g(s.myLineLock);
  if (!s.myLineHits)
  {
    auto loaded = std::make_shared<IntCurvesFace_ShapeIntersector>();
    loaded->Load(s.shape, Precision::Confusion());
    s.myLineHits = loaded;
  }
  IntCurvesFace_ShapeIntersector& it = *s.myLineHits;
  it.Perform(gp_Lin(to_pnt(p), gp_Dir(d)), 0.0, len);
  if (!it.IsDone())
    fail_op("line intersection failed");
  rust::Vec<double> out;
  for (int i = 1; i <= it.NbPnt(); ++i)
  {
    out.push_back(it.WParameter(i));
    const IntCurveSurface_TransitionOnCurve t = it.Transition(i);
    out.push_back(t == IntCurveSurface_In ? 1.0 : t == IntCurveSurface_Out ? -1.0 : 0.0);
    out.push_back(it.State(i) == TopAbs_ON ? 1.0 : 0.0);
  }
  return out;
}

RayClassifier::RayClassifier(const ShapeIMap& faces, std::vector<Bnd_Box> boxes, double tol, double band)
    : myFaces(faces),
      myBoxes(std::move(boxes)),
      myRays((size_t)faces.Extent()),
      myKinds((size_t)faces.Extent(), -1),
      myPlanes((size_t)faces.Extent()),
      myTol(tol),
      myBand(band)
{
  for (Bnd_Box& b : myBoxes)
    b.Enlarge(tol);
}

static ShapeIMap faces_of(const TopoDS_Shape& s)
{
  ShapeIMap f;
  TopExp::MapShapes(s, TopAbs_FACE, f);
  return f;
}

static std::vector<Bnd_Box> boxes_of(const ShapeIMap& faces)
{
  std::vector<Bnd_Box> b((size_t)faces.Extent());
  for (int i = 1; i <= faces.Extent(); ++i)
    face_box(faces(i), b[(size_t)i - 1]);
  return b;
}

RayClassifier::RayClassifier(const TopoDS_Shape& solid, double tol, double band)
    : RayClassifier(faces_of(solid), {}, tol, band)
{
  myBoxes = boxes_of(myFaces);
  for (Bnd_Box& b : myBoxes)
    b.Enlarge(tol);
}

RayClassifier::~RayClassifier() = default;

// Whether the half-line p + t d, t >= t0, misses the box (slabs).
static bool ray_misses(const Bnd_Box& b, const gp_Pnt& p, const gp_Dir& d, double t0)
{
  if (b.IsVoid())
    return true;
  if (b.IsOpen())
    return false;
  double lo[3], hi[3];
  b.Get(lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
  const double o[3] = {p.X(), p.Y(), p.Z()}, v[3] = {d.X(), d.Y(), d.Z()};
  double       tmin = t0, tmax = RealLast();
  for (int k = 0; k < 3; ++k)
  {
    if (std::abs(v[k]) < 1e-300)
    {
      if (o[k] < lo[k] || o[k] > hi[k])
        return true;
      continue;
    }
    double a = (lo[k] - o[k]) / v[k], c = (hi[k] - o[k]) / v[k];
    if (a > c)
      std::swap(a, c);
    tmin = std::max(tmin, a);
    tmax = std::min(tmax, c);
    if (tmin > tmax)
      return true;
  }
  return false;
}

TopAbs_State RayClassifier::Perform(const gp_Pnt& p)
{
  // Slanted, and near the axes (a point under a plate leaves it downwards
  // past nothing, sideways past every fin): the rays that pass the fewest
  // faces' boxes first. Any clean ray gives the same parity.
  static const gp_Dir all[] = {gp_Dir(0.5773, 0.5774, 0.5775), gp_Dir(-0.31, 0.83, 0.46), gp_Dir(0.71, -0.24, -0.66), gp_Dir(-0.12, -0.55, 0.83),
                               gp_Dir(0.0123, 0.0071, 0.9999), gp_Dir(-0.0071, 0.0123, -0.9999), gp_Dir(0.9999, 0.0123, 0.0071),
                               gp_Dir(-0.9999, -0.0071, 0.0123), gp_Dir(0.0071, 0.9999, -0.0123), gp_Dir(0.0123, -0.9999, -0.0071)};
  const int        nd = (int)(sizeof(all) / sizeof(all[0]));
  std::vector<int> passed((size_t)nd, 0), order((size_t)nd);
  for (int k = 0; k < nd; ++k)
  {
    order[(size_t)k] = k;
    for (const Bnd_Box& b : myBoxes)
      passed[(size_t)k] += ray_misses(b, p, all[k], -myBand) ? 0 : 1;
  }
  std::stable_sort(order.begin(), order.end(), [&](int a, int b) { return passed[(size_t)a] < passed[(size_t)b]; });
  for (int k : order)
  {
    const gp_Dir& d = all[k];
    const gp_Lin  ray(p, d);
    int           n     = 0;
    bool          clean = true;
    for (int i = 1; i <= myFaces.Extent() && clean; ++i)
    {
      if (ray_misses(myBoxes[(size_t)i - 1], p, d, -myBand))
        continue;
      // A planar face: its plane met directly, the point classified in the
      // face (OCCT's intersector approximated the whole boundary first: ~10
      // ms for the 400-fin plate's 800-edge front face).
      const TopoDS_Face& face = TopoDS::Face(myFaces(i));
      int8_t&            kind = myKinds[(size_t)i - 1];
      if (kind < 0)
      {
        TopLoc_Location                l;
        const occ::handle<Geom_Plane> pl = occ::down_cast<Geom_Plane>(BRep_Tool::Surface(face, l));
        kind                             = pl.IsNull() ? 0 : 1;
        if (kind == 1)
          myPlanes[(size_t)i - 1] = pl->Pln().Transformed(l.Transformation());
      }
      if (kind == 1)
      {
        const gp_Pln& pl = myPlanes[(size_t)i - 1];
        const gp_Vec  nv(pl.Axis().Direction());
        const double  dn = gp_Vec(d).Dot(nv), h = gp_Vec(p, pl.Location()).Dot(nv);
        if (std::abs(dn) < 1e-9)
        {
          // Along the plane: in it, nothing to count on.
          if (std::abs(h) <= myTol)
            clean = false;
          continue;
        }
        const double w = h / dn;
        if (w < -myBand)
          continue;
        BRepClass_FaceClassifier fc(face, p.Translated(gp_Vec(d) * w), myTol);
        const TopAbs_State       st = fc.State();
        if (st != TopAbs_IN && st != TopAbs_ON)
          continue;
        if (std::abs(w) <= myTol)
          return TopAbs_ON;
        if (std::abs(w) <= myBand || st == TopAbs_ON)
          clean = false;
        else
          ++n;
        continue;
      }
      std::unique_ptr<IntCurvesFace_Intersector>& fi = myRays[(size_t)i - 1];
      if (!fi)
        fi = std::make_unique<IntCurvesFace_Intersector>(TopoDS::Face(myFaces(i)), myTol);
      fi->Perform(ray, -myBand, RealLast());
      if (!fi->IsDone())
        return TopAbs_UNKNOWN;
      for (int k = 1; k <= fi->NbPnt(); ++k)
      {
        const double w = std::abs(fi->WParameter(k));
        if (w <= myTol)
          return TopAbs_ON;
        if (w <= myBand || fi->State(k) == TopAbs_ON || fi->Transition(k) == IntCurveSurface_Tangent)
        {
          clean = false;
          break;
        }
        ++n;
      }
    }
    if (clean)
      return n % 2 == 1 ? TopAbs_IN : TopAbs_OUT;
  }
  return TopAbs_UNKNOWN;
}

// Rays for big solids only (NOSHAPE_CLASSIFY_RAYS=0: never): OCCT's
// classifier is quick on small ones.
static bool rays_for(const TopoDS_Shape& s)
{
  static const bool off = [] {
    const char* v = std::getenv("NOSHAPE_CLASSIFY_RAYS");
    return v != nullptr && std::string(v) == "0";
  }();
  if (off)
    return false;
  TopoDS_Shape one = s;
  if (one.ShapeType() == TopAbs_COMPOUND && one.NbChildren() == 1)
    one = TopoDS_Iterator(one).Value();
  if (one.ShapeType() != TopAbs_SOLID)
    return false;
  int n = 0;
  for (TopExp_Explorer f(one, TopAbs_FACE); f.More() && n < 200; f.Next())
    ++n;
  return n >= 200;
}

int8_t classify_point(const OcShape& solid, P3 p, double tol)
{
  require_non_null(solid.shape, "solid");
  const LockedSection locked_section_1108;
  std::lock_guard<std::mutex> g(solid.myClassifyLock);
  const double t = tol > 0 ? tol : 1e-7;
  if (solid.myRays.first != t && rays_for(solid.shape))
  {
    const KernelTimer timer("classify.setup");
    solid.myRays = {t, std::make_shared<RayClassifier>(solid.map(TopAbs_FACE), solid.face_boxes(), t, 100 * t)};
  }
  std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("classify.rays");
  if (solid.myRays.second)
    switch (solid.myRays.second->Perform(to_pnt(p)))
    {
      case TopAbs_IN:
        return 1;
      case TopAbs_OUT:
        return -1;
      case TopAbs_ON:
        return 0;
      default:
        break;
    }
  if (!solid.myClassifier)
  {
    auto loaded = std::make_shared<BRepClass3d_SolidClassifier>();
    loaded->Load(solid.shape);
    solid.myClassifier = loaded;
  }
  BRepClass3d_SolidClassifier& c = *solid.myClassifier;
  c.Perform(to_pnt(p), t);
  switch (c.State())
  {
    case TopAbs_IN:
      return 1;
    case TopAbs_OUT:
      return -1;
    case TopAbs_ON:
      return 0;
    default:
      return 2;
  }
}

} // namespace noshape
