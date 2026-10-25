// Geometry queries, mass properties, measurement.
#include "internal.hpp"
#include <Poly_Triangulation.hxx>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
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
  double x0, y0, z0, x1, y1, z1;
  b.Get(x0, y0, z0, x1, y1, z1);
  rust::Vec<double> r;
  for (double v : {x0, y0, z0, x1, y1, z1})
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
  return to_props(volume_props(s.shape));
}

PropsF props_surface(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  return to_props(surface_props(s.shape));
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
  IntCurvesFace_ShapeIntersector it;
  it.Load(s.shape, Precision::Confusion());
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

int8_t classify_point(const OcShape& solid, P3 p, double tol)
{
  require_non_null(solid.shape, "solid");
  BRepClass3d_SolidClassifier c(solid.shape, to_pnt(p), tol > 0 ? tol : 1e-7);
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
