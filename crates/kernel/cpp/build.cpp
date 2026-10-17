#include <ShapeFix_Face.hxx>
// Construction: vertices, edges and wires from 2D sketch data and 3D data,
// planar faces, primitives.
#include "internal.hpp"

#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_MakeOffset.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>
#include <BRepPrimAPI_MakeWedge.hxx>
#include <BRep_Builder.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Line.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_HArray1.hxx>
#include <NCollection_HSequence.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <ShapeFix_Wire.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Circ.hxx>
#include <gp_Elips.hxx>
#include <gp_Lin2d.hxx>

namespace noshape {

namespace {

struct PlaneMap
{
  gp_Ax3 ax;
  gp_XYZ o, x, y, n;
  explicit PlaneMap(const PlaneF& p)
      : ax(to_ax3(p))
  {
    o = ax.Location().XYZ();
    x = ax.XDirection().XYZ();
    y = ax.YDirection().XYZ();
    n = ax.Direction().XYZ();
  }
  gp_Pnt pt(double u, double v) const { return gp_Pnt(o + x * u + y * v); }
  gp_Dir dir(double u, double v) const { return gp_Dir(x * u + y * v); }
  gp_Ax2 ax2_at(double u, double v, double rot) const
  {
    gp_Dir xd = dir(std::cos(rot), std::sin(rot));
    return gp_Ax2(pt(u, v), gp_Dir(n), xd);
  }
};

void check_finite(double v, const char* what)
{
  if (!std::isfinite(v))
    fail_invalid(std::string(what) + " is not finite");
}

double normalize_sweep(double a0, double a1)
{
  double a = a1;
  while (a <= a0)
    a += 2.0 * M_PI;
  while (a - a0 > 2.0 * M_PI + 1e-12)
    a -= 2.0 * M_PI;
  return a;
}

TopoDS_Edge edge_from_curve(const occ::handle<Geom_Curve>& c, double f, double l)
{
  BRepBuilderAPI_MakeEdge mk(c, f, l);
  if (!mk.IsDone())
    fail_op("could not build edge from curve");
  return mk.Edge();
}

occ::handle<Geom_BSplineCurve> bspline_from(const std::vector<gp_Pnt>& poles,
                                            const rust::Vec<double>& weights,
                                            const rust::Vec<double>& knots,
                                            const rust::Vec<int32_t>& mults, int degree,
                                            bool periodic)
{
  const int np = static_cast<int>(poles.size());
  if (np < 2)
    fail_invalid("B-spline needs at least 2 poles");
  if (degree < 1 || degree > 25)
    fail_invalid("B-spline degree must be in 1..25");
  if (knots.size() != mults.size() || knots.size() < 2)
    fail_invalid("B-spline knots and multiplicities must have the same length (>= 2)");
  if (!weights.empty() && weights.size() != poles.size())
    fail_invalid("B-spline weights must match the number of poles");
  int sum = 0;
  for (size_t i = 0; i < mults.size(); ++i)
  {
    if (mults[i] < 1 || mults[i] > degree + 1)
      fail_invalid("B-spline multiplicity out of range");
    sum += mults[i];
    if (i > 0 && !(knots[i] > knots[i - 1]))
      fail_invalid("B-spline knots must be strictly increasing");
  }
  if (!periodic && sum != np + degree + 1)
    fail_invalid("B-spline: sum(mults) must equal nb_poles + degree + 1");
  if (periodic && sum - mults[mults.size() - 1] != np)
    fail_invalid("periodic B-spline: sum(mults) - last mult must equal nb_poles");
  NCollection_Array1<gp_Pnt> P(1, np);
  for (int i = 0; i < np; ++i)
    P(i + 1) = poles[i];
  NCollection_Array1<double> K(1, (int)knots.size());
  NCollection_Array1<int>    M(1, (int)mults.size());
  for (size_t i = 0; i < knots.size(); ++i)
  {
    K((int)i + 1) = knots[i];
    M((int)i + 1) = mults[i];
  }
  if (weights.empty())
    return new Geom_BSplineCurve(P, K, M, degree, periodic);
  NCollection_Array1<double> W(1, np);
  for (int i = 0; i < np; ++i)
  {
    if (!(weights[i] > 0))
      fail_invalid("B-spline weights must be positive");
    W(i + 1) = weights[i];
  }
  return new Geom_BSplineCurve(P, W, K, M, degree, periodic);
}

TopoDS_Edge edge_2d(const PlaneMap& pm, const Seg2d& s)
{
  for (double v : s.p)
    check_finite(v, "segment parameter");
  const auto& p = s.p;
  switch (s.kind)
  {
    case 0: { // line
      gp_Pnt a = pm.pt(p[0], p[1]);
      gp_Pnt b = pm.pt(p[2], p[3]);
      if (a.Distance(b) < 1e-9)
        fail_invalid("line segment has zero length");
      BRepBuilderAPI_MakeEdge mk(a, b);
      if (!mk.IsDone())
        fail_op("could not build line edge");
      return mk.Edge();
    }
    case 1: { // arc: cx cy r a0 a1 (CCW from a0 to a1)
      if (!(p[2] > 1e-9))
        fail_invalid("arc radius must be positive");
      occ::handle<Geom_Circle> c = new Geom_Circle(gp_Circ(pm.ax2_at(p[0], p[1], 0.0), p[2]));
      double                   a1 = normalize_sweep(p[3], p[4]);
      return edge_from_curve(c, p[3], a1);
    }
    case 2: { // circle: cx cy r
      if (!(p[2] > 1e-9))
        fail_invalid("circle radius must be positive");
      occ::handle<Geom_Circle> c = new Geom_Circle(gp_Circ(pm.ax2_at(p[0], p[1], 0.0), p[2]));
      BRepBuilderAPI_MakeEdge  mk(c);
      return mk.Edge();
    }
    case 3:   // elliptic arc: cx cy rmaj rmin rot a0 a1
    case 4: { // ellipse: cx cy rmaj rmin rot
      double rmaj = p[2], rmin = p[3], rot = p[4];
      if (!(rmaj > 1e-9) || !(rmin > 1e-9))
        fail_invalid("ellipse radii must be positive");
      double shift = 0.0;
      if (rmin > rmaj)
      {
        std::swap(rmaj, rmin);
        rot += M_PI / 2.0;
        shift = -M_PI / 2.0;
      }
      occ::handle<Geom_Ellipse> e =
        new Geom_Ellipse(gp_Elips(pm.ax2_at(p[0], p[1], rot), rmaj, rmin));
      if (s.kind == 4)
        return BRepBuilderAPI_MakeEdge(e).Edge();
      double a0 = p[5] + shift;
      double a1 = normalize_sweep(a0, p[6] + shift);
      return edge_from_curve(e, a0, a1);
    }
    case 5: { // bspline
      if (s.poles.size() % 2 != 0)
        fail_invalid("2D B-spline poles must be (x, y) pairs");
      std::vector<gp_Pnt> poles;
      for (size_t i = 0; i + 1 < s.poles.size(); i += 2)
        poles.push_back(pm.pt(s.poles[i], s.poles[i + 1]));
      auto c = bspline_from(poles, s.weights, s.knots, s.mults, s.degree, s.periodic);
      return BRepBuilderAPI_MakeEdge(c).Edge();
    }
    case 6: { // arc through 3 points
      GC_MakeArcOfCircle mk(pm.pt(p[0], p[1]), pm.pt(p[2], p[3]), pm.pt(p[4], p[5]));
      if (!mk.IsDone())
        fail_invalid("arc through 3 points failed (collinear points?)");
      return BRepBuilderAPI_MakeEdge(mk.Value()).Edge();
    }
    case 7: { // interpolated spline through points
      if (s.poles.size() % 2 != 0 || s.poles.size() < 4)
        fail_invalid("interpolated spline needs >= 2 (x, y) points");
      const int                               n   = (int)s.poles.size() / 2;
      occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, n);
      for (int i = 0; i < n; ++i)
        pts->SetValue(i + 1, pm.pt(s.poles[2 * i], s.poles[2 * i + 1]));
      GeomAPI_Interpolate ip(pts, s.periodic, 1e-7);
      ip.Perform();
      if (!ip.IsDone())
        fail_op("spline interpolation failed");
      return BRepBuilderAPI_MakeEdge(ip.Curve()).Edge();
    }
    default:
      fail_invalid("unknown 2D segment kind");
  }
}

TopoDS_Wire wire_from_edges(const std::vector<TopoDS_Shape>& edges)
{
  if (edges.empty())
    fail_invalid("cannot build a wire from zero edges");
  BRepBuilderAPI_MakeWire mk;
  ShapeList               l;
  for (const auto& e : edges)
  {
    require_non_null(e, "edge");
    if (e.ShapeType() == TopAbs_EDGE)
      l.Append(e);
    else if (e.ShapeType() == TopAbs_WIRE)
      for (TopExp_Explorer x(e, TopAbs_EDGE); x.More(); x.Next())
        l.Append(x.Current());
    else
      fail_invalid("wire input must be edges or wires");
  }
  mk.Add(l);
  if (mk.IsDone())
    return mk.Wire();
  // Fallback: connect with tolerance.
  occ::handle<NCollection_HSequence<TopoDS_Shape>> seq = new NCollection_HSequence<TopoDS_Shape>();
  for (ShapeList::Iterator it(l); it.More(); it.Next())
    seq->Append(it.Value());
  occ::handle<NCollection_HSequence<TopoDS_Shape>> wires =
    ShapeAnalysis_FreeBounds::ConnectEdgesToWires(seq, 1e-5, false);
  if (wires.IsNull() || wires->Length() != 1)
    fail_invalid("edges do not form a single connected wire");
  return TopoDS::Wire(wires->Value(1));
}

double signed_area(const TopoDS_Face& f)
{
  GProp_GProps g;
  BRepGProp::SurfaceProperties(f, g);
  return g.Mass();
}

} // namespace

std::unique_ptr<OcShape> make_vertex(P3 p)
{
  return wrap(BRepBuilderAPI_MakeVertex(to_pnt(p)).Vertex());
}

std::unique_ptr<OcShape> make_edge_2d(const PlaneF& plane, const Seg2d& seg)
{
  PlaneMap pm(plane);
  return wrap(edge_2d(pm, seg));
}

std::unique_ptr<OcShape> make_wire_2d(const PlaneF& plane, rust::Slice<const Seg2d> segs)
{
  PlaneMap                  pm(plane);
  std::vector<TopoDS_Shape> edges;
  for (const auto& s : segs)
    edges.push_back(edge_2d(pm, s));
  return wrap(wire_from_edges(edges));
}

std::unique_ptr<OcShape> make_wire(const OcShapeList& edges)
{
  return wrap(wire_from_edges(edges.items));
}

std::unique_ptr<OcShape> make_face_on_plane(const PlaneF& plane, const OcShape& outer,
                                            const OcShapeList& holes)
{
  gp_Pln      pln = to_pln(plane);
  TopoDS_Wire ow  = as_wire(outer.shape, "outer wire");
  if (!ow.Closed() && !BRep_Tool::IsClosed(ow))
  {
    TopoDS_Vertex v1, v2;
    TopExp::Vertices(ow, v1, v2);
    if (v1.IsNull() || !v1.IsSame(v2))
      fail_invalid("outer wire is not closed");
  }
  {
    BRepBuilderAPI_MakeFace mk(pln, ow, true);
    if (!mk.IsDone())
      fail_op("could not build face from outer wire");
    if (signed_area(mk.Face()) < 0)
      ow.Reverse();
  }
  BRepBuilderAPI_MakeFace mk(pln, ow, true);
  if (!mk.IsDone())
    fail_op("could not build face from outer wire");
  for (const auto& h : holes.items)
  {
    TopoDS_Wire hw = as_wire(h, "hole wire");
    BRepBuilderAPI_MakeFace hf(pln, hw, true);
    if (!hf.IsDone())
      fail_op("could not build face from hole wire");
    if (signed_area(hf.Face()) > 0)
      hw.Reverse();
    mk.Add(hw);
  }
  if (!mk.IsDone())
    fail_op("could not build face with holes");
  TopoDS_Face f = mk.Face();
  if (!holes.items.empty())
  {
    // Per-hole orientation checks can be fooled by multi-edge loops; let
    // ShapeFix make every inner wire a proper hole (positive face area).
    Handle(ShapeFix_Face) sff = new ShapeFix_Face(f);
    sff->FixOrientationMode() = 1;
    sff->FixWireMode()        = 0;
    sff->FixSplitFaceMode()   = 0;
    sff->FixOrientation();
    f = sff->Face();
    if (signed_area(f) < 0)
      f.Reverse();
  }
  BRepLib::BuildCurves3d(f);
  return wrap(f);
}

std::unique_ptr<OcShape> make_face_from_wire(const OcShape& wire, bool only_plane)
{
  TopoDS_Wire             w = as_wire(wire.shape, "wire");
  BRepBuilderAPI_MakeFace mk(w, only_plane);
  if (!mk.IsDone())
    fail_op("could not build a face from the wire (not planar or no surface found)");
  TopoDS_Face f = mk.Face();
  if (signed_area(f) < 0)
    f.Reverse();
  return wrap(f);
}

std::unique_ptr<OcShape> make_plane_face(const PlaneF& plane, double umin, double umax,
                                         double vmin, double vmax)
{
  if (!(umax > umin) || !(vmax > vmin))
    fail_invalid("plane face bounds are empty");
  BRepBuilderAPI_MakeFace mk(to_pln(plane), umin, umax, vmin, vmax);
  if (!mk.IsDone())
    fail_op("could not build plane face");
  return wrap(mk.Face());
}

static std::vector<gp_Pnt> points_of(rust::Slice<const double> pts, const char* what)
{
  if (pts.size() % 3 != 0)
    fail_invalid(std::string(what) + " must be (x, y, z) triples");
  std::vector<gp_Pnt> r;
  for (size_t i = 0; i + 2 < pts.size(); i += 3)
  {
    check_finite(pts[i], what);
    check_finite(pts[i + 1], what);
    check_finite(pts[i + 2], what);
    r.emplace_back(pts[i], pts[i + 1], pts[i + 2]);
  }
  return r;
}

std::unique_ptr<OcShape> make_polyline(rust::Slice<const double> pts, bool closed)
{
  auto p = points_of(pts, "polyline points");
  if (p.size() < 2)
    fail_invalid("polyline needs at least 2 points");
  BRepBuilderAPI_MakePolygon mk;
  for (const auto& q : p)
    mk.Add(q);
  if (closed)
    mk.Close();
  if (!mk.IsDone())
    fail_op("could not build polyline (coincident points?)");
  return wrap(mk.Wire());
}

std::unique_ptr<OcShape> make_line(P3 a, P3 b)
{
  if (to_pnt(a).Distance(to_pnt(b)) < 1e-9)
    fail_invalid("line has zero length");
  BRepBuilderAPI_MakeEdge mk(to_pnt(a), to_pnt(b));
  if (!mk.IsDone())
    fail_op("could not build line");
  return wrap(mk.Edge());
}

std::unique_ptr<OcShape> make_circle(P3 c, P3 axis, P3 xdir, double r, double a0, double a1,
                                     bool full)
{
  if (!(r > 1e-9))
    fail_invalid("circle radius must be positive");
  PlaneF pf{c, xdir, axis};
  gp_Ax3 ax = to_ax3(pf);
  occ::handle<Geom_Circle> circ =
    new Geom_Circle(gp_Circ(gp_Ax2(ax.Location(), ax.Direction(), ax.XDirection()), r));
  if (full)
    return wrap(BRepBuilderAPI_MakeEdge(circ).Edge());
  return wrap(edge_from_curve(circ, a0, normalize_sweep(a0, a1)));
}

std::unique_ptr<OcShape> make_arc_3pts(P3 a, P3 m, P3 b)
{
  GC_MakeArcOfCircle mk(to_pnt(a), to_pnt(m), to_pnt(b));
  if (!mk.IsDone())
    fail_invalid("arc through 3 points failed (collinear points?)");
  return wrap(BRepBuilderAPI_MakeEdge(mk.Value()).Edge());
}

std::unique_ptr<OcShape> make_interpolated(rust::Slice<const double> pts,
                                           rust::Slice<const double> tangents, bool periodic)
{
  auto p = points_of(pts, "spline points");
  if (p.size() < 2)
    fail_invalid("interpolated spline needs at least 2 points");
  const int n = (int)p.size();
  occ::handle<NCollection_HArray1<gp_Pnt>> hp = new NCollection_HArray1<gp_Pnt>(1, n);
  for (int i = 0; i < n; ++i)
    hp->SetValue(i + 1, p[i]);
  GeomAPI_Interpolate ip(hp, periodic, 1e-7);
  if (!tangents.empty())
  {
    auto t = points_of(tangents, "spline tangents");
    if (t.size() == 2 && n != 2)
    {
      ip.Load(gp_Vec(t[0].XYZ()), gp_Vec(t[1].XYZ()), true);
    }
    else if ((int)t.size() == n)
    {
      NCollection_Array1<gp_Vec>                tv(1, n);
      occ::handle<NCollection_HArray1<bool>> flags = new NCollection_HArray1<bool>(1, n);
      for (int i = 0; i < n; ++i)
      {
        tv(i + 1) = gp_Vec(t[i].XYZ());
        flags->SetValue(i + 1, tv(i + 1).Magnitude() > 1e-12);
      }
      ip.Load(tv, flags, true);
    }
    else
      fail_invalid("tangents must be either 2 (start, end) or one per point");
  }
  ip.Perform();
  if (!ip.IsDone())
    fail_op("spline interpolation failed (coincident points?)");
  return wrap(BRepBuilderAPI_MakeEdge(ip.Curve()).Edge());
}

std::unique_ptr<OcShape> make_bspline_3d(rust::Slice<const double> poles,
                                         rust::Slice<const double> weights,
                                         rust::Slice<const double> knots,
                                         rust::Slice<const int32_t> mults, int32_t degree,
                                         bool periodic)
{
  auto              p = points_of(poles, "poles");
  rust::Vec<double> w, k;
  rust::Vec<int32_t> m;
  for (double v : weights)
    w.push_back(v);
  for (double v : knots)
    k.push_back(v);
  for (int32_t v : mults)
    m.push_back(v);
  auto c = bspline_from(p, w, k, m, degree, periodic);
  return wrap(BRepBuilderAPI_MakeEdge(c).Edge());
}

std::unique_ptr<OcShape> make_helix(const PlaneF& frame, double radius, double pitch,
                                    double height, bool left_handed, double taper_angle)
{
  if (!(radius > 1e-9))
    fail_invalid("helix radius must be positive");
  if (!(std::abs(pitch) > 1e-9))
    fail_invalid("helix pitch must be non-zero");
  if (!(height > 1e-9))
    fail_invalid("helix height must be positive");
  if (!(std::abs(taper_angle) < M_PI / 2.0 - 1e-6))
    fail_invalid("helix taper angle must be within (-90, 90) degrees");
  gp_Ax3                    ax    = to_ax3(frame);
  const double              turns = height / std::abs(pitch);
  occ::handle<Geom_Surface> surf;
  double                    dv; // v advance per turn
  if (std::abs(taper_angle) < 1e-12)
  {
    surf = new Geom_CylindricalSurface(ax, radius);
    dv   = std::abs(pitch);
  }
  else
  {
    surf = new Geom_ConicalSurface(ax, taper_angle, radius);
    dv   = std::abs(pitch) / std::cos(taper_angle);
  }
  const double         du  = left_handed ? -2.0 * M_PI : 2.0 * M_PI;
  gp_Lin2d             l2d(gp_Pnt2d(0.0, 0.0), gp_Dir2d(du, dv));
  const double         len = turns * std::sqrt(du * du + dv * dv);
  occ::handle<Geom2d_Line> gl = new Geom2d_Line(l2d);
  BRepBuilderAPI_MakeEdge  mk(gl, surf, 0.0, len);
  if (!mk.IsDone())
    fail_op("could not build helix edge");
  TopoDS_Edge e     = mk.Edge();
  int         nseg  = std::max(16, (int)std::ceil(turns * 8.0));
  if (!BRepLib::BuildCurves3d(e, 1e-6, GeomAbs_C2, 8, nseg))
    fail_op("helix 3D curve approximation failed");
  BRep_Builder b;
  TopoDS_Wire  w;
  b.MakeWire(w);
  b.Add(w, e);
  return wrap(w);
}

std::unique_ptr<OcShape> offset_wire_2d(const OcShape& wire_or_face, double distance,
                                        uint8_t join, bool open_result)
{
  require_non_null(wire_or_face.shape, "wire");
  GeomAbs_JoinType jt = join == 1 ? GeomAbs_Intersection : GeomAbs_Arc;
  BRepOffsetAPI_MakeOffset mk;
  if (wire_or_face.shape.ShapeType() == TopAbs_FACE)
  {
    mk.Init(TopoDS::Face(wire_or_face.shape), jt, open_result);
  }
  else
  {
    TopoDS_Wire w = as_wire(wire_or_face.shape, "wire");
    mk.Init(jt, open_result);
    mk.AddWire(w);
  }
  mk.Perform(distance);
  if (!mk.IsDone())
    fail_op("wire offset failed (offset too large or non-planar wire?)");
  return wrap(simplify_compound(mk.Shape()));
}

static void check_pos(double v, const char* what)
{
  if (!(v > 1e-9) || !std::isfinite(v))
    fail_invalid(std::string(what) + " must be positive");
}

static double check_angle(double a)
{
  if (!(a > 1e-9))
    fail_invalid("angle must be positive");
  return std::min(a, 2.0 * M_PI);
}

std::unique_ptr<OcShape> make_box(const PlaneF& frame, double dx, double dy, double dz)
{
  check_pos(dx, "box dx");
  check_pos(dy, "box dy");
  check_pos(dz, "box dz");
  BRepPrimAPI_MakeBox mk(to_ax2(frame), dx, dy, dz);
  return wrap(mk.Solid());
}

std::unique_ptr<OcShape> make_cylinder(const PlaneF& frame, double r, double h, double angle)
{
  check_pos(r, "cylinder radius");
  check_pos(h, "cylinder height");
  BRepPrimAPI_MakeCylinder mk(to_ax2(frame), r, h, check_angle(angle));
  return wrap(mk.Solid());
}

std::unique_ptr<OcShape> make_cone(const PlaneF& frame, double r1, double r2, double h,
                                   double angle)
{
  if (r1 < 0 || r2 < 0 || (r1 < 1e-9 && r2 < 1e-9) || std::abs(r1 - r2) < 1e-12)
    fail_invalid("cone radii must be non-negative, not both zero and not equal");
  check_pos(h, "cone height");
  BRepPrimAPI_MakeCone mk(to_ax2(frame), r1, r2, h, check_angle(angle));
  return wrap(mk.Solid());
}

std::unique_ptr<OcShape> make_sphere(const PlaneF& frame, double r, double angle)
{
  check_pos(r, "sphere radius");
  BRepPrimAPI_MakeSphere mk(to_ax2(frame), r, check_angle(angle));
  return wrap(mk.Solid());
}

std::unique_ptr<OcShape> make_torus(const PlaneF& frame, double r1, double r2, double angle)
{
  check_pos(r1, "torus major radius");
  check_pos(r2, "torus minor radius");
  BRepPrimAPI_MakeTorus mk(to_ax2(frame), r1, r2, check_angle(angle));
  return wrap(mk.Solid());
}

std::unique_ptr<OcShape> make_wedge(const PlaneF& frame, double dx, double dy, double dz,
                                    double ltx)
{
  check_pos(dx, "wedge dx");
  check_pos(dy, "wedge dy");
  check_pos(dz, "wedge dz");
  if (ltx < 0)
    fail_invalid("wedge ltx must be >= 0");
  BRepPrimAPI_MakeWedge mk(to_ax2(frame), dx, dy, dz, ltx);
  return wrap(mk.Solid());
}

std::unique_ptr<OcShapeList> edges_to_wires(const OcShape& s, double tol)
{
  require_non_null(s.shape, "shape");
  occ::handle<NCollection_HSequence<TopoDS_Shape>> seq = new NCollection_HSequence<TopoDS_Shape>();
  for (TopExp_Explorer e(s.shape, TopAbs_EDGE); e.More(); e.Next())
    seq->Append(e.Current());
  auto out = std::make_unique<OcShapeList>();
  if (seq->IsEmpty())
    return out;
  occ::handle<NCollection_HSequence<TopoDS_Shape>> wires =
    ShapeAnalysis_FreeBounds::ConnectEdgesToWires(seq, tol > 0 ? tol : 1e-6, false);
  if (!wires.IsNull())
    for (int i = 1; i <= wires->Length(); ++i)
      out->items.push_back(wires->Value(i));
  return out;
}

} // namespace noshape
