// Geometry queries, mass properties, measurement.
#include "internal.hpp"
#include <Poly_Triangulation.hxx>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <algorithm>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <vector>
#include <algorithm>
#include <Extrema_POnSurf.hxx>
#include <Extrema_GenLocateExtPS.hxx>
#include <BRepTopAdaptor_FClass2d.hxx>
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
  std::vector<Sample> samples;
  const int           N = 24;
  for (int i = 0; i <= N; ++i)
    for (int j = 0; j <= N; ++j)
    {
      const double uu = u0 + (u1 - u0) * (i + 0.5) / (N + 1), vv = v0 + (v1 - v0) * (j + 0.5) / (N + 1);
      if (cl.Perform(gp_Pnt2d(uu, vv)) != TopAbs_IN)
        continue;
      samples.push_back({p.SquareDistance(s.Value(uu, vv)), uu, vv});
    }
  if (samples.empty())
    return false;
  const size_t k = std::min<size_t>(4, samples.size());
  std::partial_sort(samples.begin(), samples.begin() + k, samples.end(), [](const Sample& a, const Sample& b) { return a.d < b.d; });
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

ProjPoint face_project_point(const OcShape& fs, P3 p)
{
  const TopoDS_Face&         f = as_face(fs.shape, "shape");
  TopoDS_Vertex              v = BRepBuilderAPI_MakeVertex(to_pnt(p));
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

// OCCT's optimal box of a free-form face reaches Precision::Confusion past
// short edges round it (BndLib_Add3dCurve's generic curve box: the corner
// patches of an imported filleted block, g47-1part: z 10.0001 mm for 10;
// qa5-routing BBOX-TOL). Each side is pulled in to the shape's exact extreme
// there, the distance to a plane just outside it, by at most a few
// Precision::Confusion (never out, never further).
void tighten_box(const TopoDS_Shape& s, double box[6])
{
  const double gap = 1e3 * Precision::Confusion(), most = 10 * Precision::Confusion();
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
      BRepExtrema_DistShapeShape d(s, plane);
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
  Bnd_Box b;
  if (optimal)
    BRepBndLib::AddOptimal(s.shape, b, false, false);
  else
    BRepBndLib::Add(s.shape, b, false);
  // Mesh faces have no surface: their triangulation's nodes.
  for (TopExp_Explorer e(s.shape, TopAbs_FACE); e.More(); e.Next())
    if (is_mesh_face(TopoDS::Face(e.Current())))
    {
      TopLoc_Location                       l;
      const occ::handle<Poly_Triangulation> t = BRep_Tool::Triangulation(TopoDS::Face(e.Current()), l);
      for (int i = 1; i <= t->NbNodes(); ++i)
        b.Add(t->Node(i).Transformed(l.Transformation()));
    }
  if (b.IsVoid())
    fail_invalid("shape has no geometry (void bounding box)");
  double box[6];
  b.Get(box[0], box[1], box[2], box[3], box[4], box[5]);
  if (optimal && !has_mesh_faces(s.shape) && has_free_form_face(s.shape))
    tighten_box(s.shape, box);
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
      gp_Ax3 ax = a.Plane().Position();
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

// Direction of a planar face normal or linear edge.
static bool linear_direction(const TopoDS_Shape& s, gp_Dir& d, bool& is_plane)
{
  if (s.ShapeType() == TopAbs_FACE)
  {
    BRepAdaptor_Surface a(TopoDS::Face(s), true);
    if (a.GetType() != GeomAbs_Plane)
      return false;
    d = a.Plane().Axis().Direction();
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
    BRepBndLib::Add(faces(i), b[(size_t)i - 1], false);
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

TopAbs_State RayClassifier::Perform(const gp_Pnt& p)
{
  const gp_Dir dirs[] = {gp_Dir(0.5773, 0.5774, 0.5775), gp_Dir(-0.31, 0.83, 0.46), gp_Dir(0.71, -0.24, -0.66), gp_Dir(-0.12, -0.55, 0.83)};
  for (const gp_Dir& d : dirs)
  {
    const gp_Lin ray(p, d);
    int          n     = 0;
    bool         clean = true;
    for (int i = 1; i <= myFaces.Extent() && clean; ++i)
    {
      if (myBoxes[(size_t)i - 1].IsOut(ray))
        continue;
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
  std::lock_guard<std::mutex> g(solid.myClassifyLock);
  const double t = tol > 0 ? tol : 1e-7;
  if (solid.myRays.first != t && rays_for(solid.shape))
    solid.myRays = {t, std::make_shared<RayClassifier>(solid.map(TopAbs_FACE), solid.face_boxes(), t, 100 * t)};
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
