#include <BRepTools_ReShape.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <ElSLib.hxx>
#include <TopoDS_Iterator.hxx>
#include <NCollection_DataMap.hxx>
#include <ShapeFix_Face.hxx>
// Construction: vertices, edges and wires from 2D sketch data and 3D data,
// planar faces, primitives.
#include "internal.hpp"

#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
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
#include <BRep_Tool.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_Plane.hxx>
#include <BRepTools.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <gp_Dir2d.hxx>
#include <BSplCLib.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <TopAbs.hxx>
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <Precision.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <Geom_Line.hxx>
#include <TopExp.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomProjLib.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2dConvert.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopoDS_Compound.hxx>
#include <GeomConvert.hxx>
#include <Geom_BSplineSurface.hxx>
#include <GeomLib.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Line.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GeomAdaptor_Surface.hxx>
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

// The same wire (its edges, vertices and orientations) with its edges stored
// in `order` (indices into the wire's edge order): the order sub-shape
// queries list them in.
std::unique_ptr<OcShape> wire_with_edge_order(const OcShape& wire, rust::Slice<const uint32_t> order)
{
  TopoDS_Wire                src = as_wire(wire.shape, "wire");
  std::vector<TopoDS_Shape> edges;
  for (TopExp_Explorer x(src, TopAbs_EDGE); x.More(); x.Next())
    edges.push_back(x.Current());
  if (order.size() != edges.size())
    fail_invalid("edge order must list every edge of the wire once");
  std::vector<bool> used(edges.size(), false);
  BRep_Builder      b;
  TopoDS_Wire       w;
  b.MakeWire(w);
  for (uint32_t i : order)
  {
    if (i >= edges.size() || used[i])
      fail_invalid("edge order must list every edge of the wire once");
    used[i] = true;
    b.Add(w, edges[i]);
  }
  w.Closed(BRep_Tool::IsClosed(src));
  return wrap(w);
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

// A face on `on`'s surface bounded by `wire` (a face pattern's cap where the faces leave a cylinder); the wire is
// copied, so the edges it came from keep their curves.
std::unique_ptr<OcShape> make_face_on(const OcShape& on, const OcShape& wire)
{
  const TopoDS_Face& base = as_face(on.shape, "face");
  const TopoDS_Wire  w    = TopoDS::Wire(BRepBuilderAPI_Copy(as_wire(wire.shape, "wire")).Shape());
  BRepBuilderAPI_MakeFace mk(BRep_Tool::Surface(base), w, true);
  if (!mk.IsDone())
    fail_op("could not build a face on the surface from the wire");
  ShapeFix_Face fix(mk.Face());
  fix.FixOrientation();
  fix.Perform();
  return wrap(fix.Face());
}

// `on`'s face on its surface carried `len` past its edges (a template that must reach past the faces it replaces):
// a B-spline extended in its non-periodic directions (GeomLib, curvature-continuous), another surface by its
// parameters (a plane's by length, a periodic direction whole).
namespace
{
// A template built over a face's own parameters (a displacement map's): the map from the face's (u, v), normalized,
// maybe swapped or flipped, onto the template's parameters. Along a clamped direction the sample at fractional index x
// sits where the knots' index function sum_j N_j(t) j is x (a B-spline repeats data linear in the index exactly: a
// plane's points); along a periodic one, linearly.
struct TemplateAxis
{
  double                           lo, hi, last;
  occ::handle<Geom2d_BSplineCurve> index;  // null: periodic
  double at(double s) const
  {
    if (index.IsNull())
      return lo + s * (hi - lo);
    const double x = std::clamp(s, 0.0, 1.0) * last;
    double       a = lo, b = hi;
    for (int k = 0; k < 64; ++k)
    {
      const double mid = 0.5 * (a + b);
      (index->Value(mid).X() < x ? a : b) = mid;
    }
    return 0.5 * (a + b);
  }
};

struct TemplateFlip
{
  bool swap, flip_u, flip_v;
};

struct TemplateMap
{
  TopoDS_Face                      F;  // forward
  occ::handle<Geom_BSplineSurface> S;
  gp_Trsf                          tt;
  double                           fu0 = 0, fu1 = 0, fv0 = 0, fv1 = 0, su0 = 0, su1 = 0, sv0 = 0, sv1 = 0, size = 1;
  TemplateAxis                     ua, va;
  TemplateFlip                     m{};
  gp_XY norm(const gp_Pnt2d& p) const { return gp_XY((p.X() - fu0) / (fu1 - fu0), (p.Y() - fv0) / (fv1 - fv0)); }
  gp_Pnt2d apply(const TemplateFlip& f, const gp_XY& q0) const
  {
    gp_XY q = f.swap ? gp_XY(q0.Y(), q0.X()) : q0;
    if (f.flip_u)
      q.SetX(1 - q.X());
    if (f.flip_v)
      q.SetY(1 - q.Y());
    return gp_Pnt2d(ua.at(q.X()), va.at(q.Y()));
  }
};

// The map from face `face_in` onto template `T` (each sampled point of the face moved along its normal, the move's
// sideways part small); a message when there is none.
const char* template_map(const TopoDS_Face& face_in, const TopoDS_Face& T, TemplateMap& tm)
{
  tm.F = TopoDS::Face(face_in.Oriented(TopAbs_FORWARD));
  TopLoc_Location tl;
  tm.S = occ::down_cast<Geom_BSplineSurface>(BRep_Tool::Surface(T, tl));
  if (tm.S.IsNull())
    return "the template is not a B-spline surface";
  const occ::handle<Geom_BSplineSurface>& S = tm.S;
  tm.tt = tl.Transformation();
  BRepTools::UVBounds(tm.F, tm.fu0, tm.fu1, tm.fv0, tm.fv1);
  S->Bounds(tm.su0, tm.su1, tm.sv0, tm.sv1);
  if (!(tm.fu1 > tm.fu0 && tm.fv1 > tm.fv0))
    return "the face has no extent";
  auto axis = [&](bool along_u) -> TemplateAxis {
    const int    n        = along_u ? S->NbUPoles() : S->NbVPoles();
    const bool   periodic = along_u ? S->IsUPeriodic() : S->IsVPeriodic();
    TemplateAxis x{along_u ? tm.su0 : tm.sv0, along_u ? tm.su1 : tm.sv1, double(n - 1), {}};
    if (periodic)
      return x;
    NCollection_Array1<gp_Pnt2d> poles(1, n);
    for (int i = 1; i <= n; ++i)
      poles(i) = gp_Pnt2d(i - 1, 0);
    x.index = new Geom2d_BSplineCurve(poles, along_u ? S->UKnots() : S->VKnots(), along_u ? S->UMultiplicities() : S->VMultiplicities(), along_u ? S->UDegree() : S->VDegree());
    return x;
  };
  tm.ua = axis(true);
  tm.va = axis(false);
  {
    Bnd_Box b;
    BRepBndLib::Add(tm.F, b);
    tm.size = b.IsVoid() ? 1.0 : std::sqrt(b.SquareExtent());
  }
  BRepAdaptor_Surface fs(tm.F, false);
  for (int k = 0; k < 8; ++k)
  {
    const TemplateFlip m{(k & 4) != 0, (k & 1) != 0, (k & 2) != 0};
    bool               ok = true;
    for (int i = 0; i <= 6 && ok; ++i)
      for (int j = 0; j <= 6 && ok; ++j)
      {
        const double u = tm.fu0 + (tm.fu1 - tm.fu0) * (0.1 + 0.8 * i / 6.0), v = tm.fv0 + (tm.fv1 - tm.fv0) * (0.1 + 0.8 * j / 6.0);
        gp_Pnt       p;
        gp_Vec       du, dv;
        fs.D1(u, v, p, du, dv);
        const gp_Vec n = du.Crossed(dv);
        if (n.Magnitude() < 1e-12)
          continue;
        const gp_Pnt2d st = tm.apply(m, tm.norm(gp_Pnt2d(u, v)));
        const gp_Vec   d(p, S->Value(st.X(), st.Y()).Transformed(tm.tt));
        const gp_Vec   nn = n.Normalized();
        const double   side = (d - nn * d.Dot(nn)).Magnitude();
        ok = side <= 0.05 * tm.size;
      }
    if (ok)
    {
      tm.m = m;
      return nullptr;
    }
  }
  return "the template is not the face moved along its normals";
}
} // namespace

// Whether template `tmpl` (a single B-spline face built over `face`'s parameters, as replace_face_mapped takes) faces
// the way `face` does, both as they lie in their shapes: the majority of the map's sample points, the face's normal
// against the template's at the point it maps to (no projection onto the dense template). 1 along, -1 against, 0 no
// such map.
int mapped_template_sense(const OcShape& face_in, const OcShape& tmpl_in)
{
  const TopoDS_Face& face = as_face(face_in.shape, "face");
  const TopoDS_Face& T    = as_face(tmpl_in.shape, "template");
  TemplateMap        tm;
  if (template_map(face, T, tm) != nullptr)
    return 0;
  BRepAdaptor_Surface fs(tm.F, false);
  const double        fsign = face.Orientation() == TopAbs_REVERSED ? -1.0 : 1.0;
  const double        tsign = T.Orientation() == TopAbs_REVERSED ? -1.0 : 1.0;
  int                 votes = 0;
  for (int i = 0; i <= 6; ++i)
    for (int j = 0; j <= 6; ++j)
    {
      const double u = tm.fu0 + (tm.fu1 - tm.fu0) * (0.1 + 0.8 * i / 6.0), v = tm.fv0 + (tm.fv1 - tm.fv0) * (0.1 + 0.8 * j / 6.0);
      gp_Pnt       p, q;
      gp_Vec       du, dv, su, sv;
      fs.D1(u, v, p, du, dv);
      const gp_Pnt2d st = tm.apply(tm.m, tm.norm(gp_Pnt2d(u, v)));
      tm.S->D1(st.X(), st.Y(), q, su, sv);
      const double d = fsign * tsign * du.Crossed(dv).Dot(su.Crossed(sv).Transformed(tm.tt));
      votes += d > 0 ? 1 : d < 0 ? -1 : 0;
    }
  return votes > 0 ? 1 : votes < 0 ? -1 : 0;
}

// `face` of `solid` replaced by a template built over its own parameters: a B-spline surface whose points at the
// face's normalized (u, v) are the face's points moved along its normals (a displacement map's, std
// evFaceTangentPlanes then bSplineSurface over the samples). The face's edge curves on it are carried into the
// template's parameters (an affine map, its axes swapped or flipped as the samples' are), so the new face is the
// template within the face's own boundary; the neighbours keep their surfaces and take the new edges (each checked
// to lie on them), and a straight edge along the move at a moved vertex (a wall's seam line) is drawn to the new
// vertex. No intersection with the dense surface: the user's 512 x 512 map. Fails where any of that doesn't hold.
std::unique_ptr<OcShape> replace_face_mapped(const OcShape& solid_in, const OcShape& face_in, const OcShape& tmpl_in)
{
  const KernelTimer   timer("replace_mapped");
  const TopoDS_Shape& solid = solid_in.shape;
  const TopoDS_Face&  T     = as_face(tmpl_in.shape, "template");
  TemplateMap         tm;
  if (const char* why = template_map(as_face(face_in.shape, "face"), T, tm))
    fail_op(why);
  const TopoDS_Face&                      F = tm.F;
  const occ::handle<Geom_BSplineSurface>& S = tm.S;
  const gp_Trsf&                          tt = tm.tt;
  TopLoc_Location                         tl;
  BRep_Tool::Surface(T, tl);
  double &fu0 = tm.fu0, &fu1 = tm.fu1, &fv0 = tm.fv0, &fv1 = tm.fv1;
  const double su0 = tm.su0, su1 = tm.su1, sv0 = tm.sv0, sv1 = tm.sv1, size = tm.size;
  BRepAdaptor_Surface fs(F, false);
  using Map = TemplateFlip;
  auto norm  = [&](const gp_Pnt2d& p) { return tm.norm(p); };
  auto apply = [&](const Map& m, const gp_XY& q) { return tm.apply(m, q); };
  const int best = (tm.m.swap ? 4 : 0) | (tm.m.flip_u ? 1 : 0) | (tm.m.flip_v ? 2 : 0);
  const Map    m{(best & 4) != 0, (best & 1) != 0, (best & 2) != 0};
  const bool   odd = m.swap != (m.flip_u != m.flip_v);
  // The face's box as the samples were taken over it: the template's corners back on the face (OCCT's bounds of
  // a disk's edges came out microns wider, and its rim then missed the wall by as much). Not round a period.
  {
    TopLoc_Location                 floc;
    const occ::handle<Geom_Surface> fsurf = BRep_Tool::Surface(F, floc);
    auto                            back  = [&](const gp_XY& q, double& pu, double& pv) {
      const gp_Pnt2d             st = apply(m, q);
      GeomAPI_ProjectPointOnSurf pr(S->Value(st.X(), st.Y()).Transformed(tt).Transformed(floc.Transformation().Inverted()), fsurf);
      if (!pr.IsDone() || pr.NbPoints() == 0)
        return false;
      pr.LowerDistanceParameters(pu, pv);
      return true;
    };
    double a_u, a_v, b_u, b_v;
    if (back(gp_XY(0, 0), a_u, a_v) && back(gp_XY(1, 1), b_u, b_v))
    {
      const bool near_u = std::abs(a_u - fu0) <= 0.01 * (fu1 - fu0) && std::abs(b_u - fu1) <= 0.01 * (fu1 - fu0);
      const bool near_v = std::abs(a_v - fv0) <= 0.01 * (fv1 - fv0) && std::abs(b_v - fv1) <= 0.01 * (fv1 - fv0);
      if (near_u && !fsurf->IsUPeriodic())
      {
        fu0 = a_u;
        fu1 = b_u;
      }
      if (near_v && !fsurf->IsVPeriodic())
      {
        fv0 = a_v;
        fv1 = b_v;
      }
    }
  }
  const double tol = std::max(BRep_Tool::Tolerance(F), 1e-7);
  BRep_Builder B;
  // An edge's curve on the face, carried into the template's parameters: sampled, mapped and interpolated.
  bool last_iso = false;
  auto carried  = [&](const TopoDS_Edge& e, const TopoDS_Face& on, double& f, double& l) -> occ::handle<Geom2d_BSplineCurve> {
    last_iso = false;
    const occ::handle<Geom2d_Curve> c = BRep_Tool::CurveOnSurface(e, on, f, l);
    if (c.IsNull())
      return {};
    // A line of constant u or v on the face (a rim, a seam): the map keeps each axis apart, so its image is the
    // template's iso line between its ends' images; no sampling.
    {
      occ::handle<Geom2d_Curve> basis = c;
      while (const occ::handle<Geom2d_TrimmedCurve> tc = occ::down_cast<Geom2d_TrimmedCurve>(basis))
        basis = tc->BasisCurve();
      const occ::handle<Geom2d_Line> ln = occ::down_cast<Geom2d_Line>(basis);
      if (!ln.IsNull() && (std::abs(ln->Direction().X()) < 1e-12 || std::abs(ln->Direction().Y()) < 1e-12))
      {
        const gp_Pnt2d p0 = apply(m, norm(c->Value(f))), p1 = apply(m, norm(c->Value(l)));
        const gp_Vec2d d(p0, p1);
        if (d.Magnitude() > 0)
        {
          occ::handle<Geom2d_BSplineCurve> b = Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(new Geom2d_Line(p0, gp_Dir2d(d)), 0, d.Magnitude()));
          f        = b->FirstParameter();
          l        = b->LastParameter();
          last_iso = true;
          return b;
        }
      }
    }
    // (Four points a span of the template: the map bends within its first spans at each clamped side.)
    const int                                   n   = std::clamp(4 * (S->NbUKnots() + S->NbVKnots()) + 1, 129, 40001);
    occ::handle<NCollection_HArray1<gp_Pnt2d>> pts = new NCollection_HArray1<gp_Pnt2d>(1, n);
    // (Each point's map a bisection on the template's knots: ~10 µs, 0.3 s an edge at 704².)
    for (int k = 0; k < n; ++k)
    {
      if ((k & 1023) == 0)
        fail_if_cancelled();
      pts->SetValue(k + 1, apply(m, norm(c->Value(f + (l - f) * k / (n - 1)))));
    }
    // A line of constant u or v (a rim or seam carried onto the template's own boundary): that iso line, exactly.
    const gp_Pnt2d p0 = pts->Value(1), p1 = pts->Value(n);
    bool           iso_u = true, iso_v = true;
    for (int k = 1; k <= n; ++k)
    {
      iso_u = iso_u && std::abs(pts->Value(k).X() - p0.X()) <= 1e-12 * (1 + std::abs(su1 - su0));
      iso_v = iso_v && std::abs(pts->Value(k).Y() - p0.Y()) <= 1e-12 * (1 + std::abs(sv1 - sv0));
    }
    if (iso_u != iso_v)
    {
      const gp_Vec2d d(p0, p1);
      if (d.Magnitude() > 0)
      {
        occ::handle<Geom2d_BSplineCurve> b = Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(new Geom2d_Line(p0, gp_Dir2d(d)), 0, d.Magnitude()));
        f        = b->FirstParameter();
        l        = b->LastParameter();
        last_iso = true;
        return b;
      }
    }
    Geom2dAPI_Interpolate in(pts, false, 1e-12);
    in.Perform();
    if (!in.IsDone())
      return {};
    const occ::handle<Geom2d_BSplineCurve> b = in.Curve();
    f = b->FirstParameter();
    l = b->LastParameter();
    return b;
  };
  // New vertices (by the old ones) and edges.
  NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher> vmap, emap;
  auto vertex_at = [&](const TopoDS_Vertex& old, const gp_Pnt& p) -> TopoDS_Vertex {
    const TopoDS_Shape key = old.Oriented(TopAbs_FORWARD);
    if (const TopoDS_Shape* found = vmap.Seek(key))
      return TopoDS::Vertex(*found);
    TopoDS_Vertex nv;
    B.MakeVertex(nv, p, tol);
    vmap.Bind(key, nv);
    return nv;
  };
  // A curve projected onto a periodic surface shifted by whole periods to pass where an old one does (`want` at `t`):
  // the face's other curves are in that period.
  auto align = [](occ::handle<Geom2d_Curve>& pc, double t, const gp_Pnt2d& want, const occ::handle<Geom_Surface>& on) {
    const gp_Pnt2d got = pc->Value(t);
    const double   du  = on->IsUPeriodic() ? std::round((want.X() - got.X()) / on->UPeriod()) * on->UPeriod() : 0;
    const double   dv  = on->IsVPeriodic() ? std::round((want.Y() - got.Y()) / on->VPeriod()) * on->VPeriod() : 0;
    if (du != 0 || dv != 0)
      pc = occ::down_cast<Geom2d_Curve>(pc->Translated(gp_Vec2d(du, dv)));
  };
  TopoDS_Face G;
  B.MakeFace(G, S, tl, tol);
  std::optional<KernelTimer> step(std::in_place, "replace_mapped.carry");
  for (TopoDS_Iterator wi(F); wi.More(); wi.Next())
  {
    TopoDS_Wire w;
    B.MakeWire(w);
    for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
    {
      const TopoDS_Edge  e   = TopoDS::Edge(ei.Value());
      const TopoDS_Shape key = e.Oriented(TopAbs_FORWARD);
      if (!emap.IsBound(key))
      {
        const TopoDS_Edge ef = TopoDS::Edge(key);
        double            f, l;
        const occ::handle<Geom2d_BSplineCurve> c = carried(ef, F, f, l);
        if (c.IsNull())
          fail_op("an edge of the face has no curve on it");
        const bool iso_edge = last_iso;
        TopoDS_Edge ne;
        B.MakeEdge(ne);
        TopoDS_Vertex v0, v1;
        TopExp::Vertices(ef, v0, v1);
        if (v0.IsNull() || v1.IsNull())
          fail_op("an edge of the face has no vertices");
        const gp_Pnt2d a = c->Value(f), z = c->Value(l);
        const TopoDS_Vertex n0 = vertex_at(v0, S->Value(a.X(), a.Y()).Transformed(tt)), n1 = vertex_at(v1, S->Value(z.X(), z.Y()).Transformed(tt));
        B.Add(ne, n0.Oriented(TopAbs_FORWARD));
        B.Add(ne, n1.Oriented(TopAbs_REVERSED));
        if (BRep_Tool::IsClosed(ef, F))
        {
          double f2, l2;
          const occ::handle<Geom2d_BSplineCurve> c2 = carried(TopoDS::Edge(ef.Reversed()), F, f2, l2);
          if (c2.IsNull())
            fail_op("a seam of the face has no second curve");
          B.UpdateEdge(ne, c, c2, S, tl, tol);
        }
        else
          B.UpdateEdge(ne, c, S, tl, tol);
        B.Range(ne, f, l);
        // Its 3D curve dense on the template (BRepLib's fit of a curve across hundreds of the template's spans
        // strayed microns off the walls it must lie on): points of the template along it, interpolated at its
        // own parameters. An iso line's comes from BRepLib below.
        if (!iso_edge)
        {
          // (Four points a span of the template, as the curve on it: between them the curve strays nanometres.)
          const int n = std::clamp(4 * (S->NbUKnots() + S->NbVKnots()) + 1, 257, 100001);
          occ::handle<NCollection_HArray1<gp_Pnt>> ps = new NCollection_HArray1<gp_Pnt>(1, n);
          occ::handle<NCollection_HArray1<double>> ts = new NCollection_HArray1<double>(1, n);
          for (int k = 0; k < n; ++k)
          {
            if ((k & 4095) == 0)
              fail_if_cancelled();
            const double   t = f + (l - f) * k / (n - 1);
            const gp_Pnt2d w = c->Value(t);
            ps->SetValue(k + 1, S->Value(w.X(), w.Y()));
            ts->SetValue(k + 1, t);
          }
          const KernelTimer t3("replace_mapped.carry.3d");
          GeomAPI_Interpolate in3(ps, ts, false, 1e-12);
          in3.Perform();
          if (!in3.IsDone())
            fail_op("a new edge's curve could not be made");
          B.UpdateEdge(ne, in3.Curve(), tl, tol);
          B.Range(ne, f, l);
        }
        emap.Bind(key, ne);
      }
      // (A map turning the parameters over runs the loops the other way round on the template.)
      const TopAbs_Orientation o = e.Orientation();
      B.Add(w, TopoDS::Edge(emap.Find(key)).Oriented(odd ? TopAbs::Reverse(o) : o));
    }
    w.Closed(BRep_Tool::IsClosed(TopoDS::Wire(wi.Value())));
    B.Add(G, w);
  }
  step.emplace("replace_mapped.curves3d");
  if (!BRepLib::BuildCurves3d(G, tol))
    fail_op("the new face's edges have no 3D curves");
  step.emplace("replace_mapped.neighbours");
  // Standing for the face as its surface faces: the template's normal against the face's at a point.
  bool same;
  {
    const double u = 0.5 * (fu0 + fu1), v = 0.5 * (fv0 + fv1);
    gp_Pnt       p;
    gp_Vec       du, dv, su, sv;
    fs.D1(u, v, p, du, dv);
    const gp_Pnt2d st = apply(m, norm(gp_Pnt2d(u, v)));
    gp_Pnt         q;
    S->D1(st.X(), st.Y(), q, su, sv);
    same = du.Crossed(dv).Dot(su.Crossed(sv).Transformed(tt)) > 0;
  }
  if (same == odd)
    fail_op("the template faces against its parameters' map");
  // The neighbours: each new edge's curve on them, checked to lie on them.
  ShapeAncMap ef;
  TopExp::MapShapesAndAncestors(solid, TopAbs_EDGE, TopAbs_FACE, ef);
  for (NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(emap); it.More(); it.Next())
  {
    const TopoDS_Edge& old = TopoDS::Edge(it.Key());
    const TopoDS_Edge& ne  = TopoDS::Edge(it.Value());
    if (!ef.Contains(old))
      fail_op("an edge of the face is not in the part");
    double                        f, l;
    TopLoc_Location               cl;
    const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(ne, cl, f, l);
    if (c3.IsNull())
      fail_op("a new edge has no 3D curve");
    for (ShapeList::Iterator fi(ef.FindFromKey(old)); fi.More(); fi.Next())
    {
      const TopoDS_Face N = TopoDS::Face(fi.Value());
      if (N.IsSame(F))
        continue;
      TopLoc_Location                 nl;
      const occ::handle<Geom_Surface> ns = BRep_Tool::Surface(N, nl);
      const occ::handle<Geom_Curve>   local = occ::down_cast<Geom_Curve>(c3->Transformed(cl.Transformation().Multiplied(nl.Transformation().Inverted())));
      double                          dmax = 0;
      for (int k = 0; k <= 32; ++k)
      {
        GeomAPI_ProjectPointOnSurf pr(local->Value(f + (l - f) * k / 32.0), ns);
        if (!pr.IsDone() || pr.NbPoints() == 0)
          fail_op("a new edge can't be projected onto a neighbour");
        dmax = std::max(dmax, pr.LowerDistance());
      }
      if (dmax > 1e-6 * size + 1e-7)
        fail_op("a new edge leaves a neighbouring face");
      // Its curve there: its points' parameters on the surface, each from the last (continuous round a period),
      // interpolated at the 3D curve's own parameters, so the two agree point for point.
      occ::handle<Geom2d_Curve> pc;
      double                    dev = 0;
      {
        // (Two points a span of the 3D curve: it carries the image's detail, sampled as finely as the template.)
        const occ::handle<Geom_BSplineCurve> bc = occ::down_cast<Geom_BSplineCurve>(local);
        const int n = std::clamp(bc.IsNull() ? 257 : 2 * bc->NbKnots() * std::max(1, bc->Degree() / 2) + 1, 257, 80001);
        occ::handle<NCollection_HArray1<gp_Pnt2d>> uv = new NCollection_HArray1<gp_Pnt2d>(1, n);
        occ::handle<NCollection_HArray1<double>>   ts = new NCollection_HArray1<double>(1, n);
        ShapeAnalysis_Surface                       sas(ns);
        const GeomAdaptor_Surface                   ga(ns);
        const GeomAbs_SurfaceType                   kind = ga.GetType();
        // (On a plane, cylinder, cone or sphere a point's parameters in closed form.)
        auto elementary = [&](const gp_Pnt& q, gp_Pnt2d& w) {
          double pu = 0, pv = 0;
          switch (kind)
          {
            case GeomAbs_Plane: ElSLib::Parameters(ga.Plane(), q, pu, pv); break;
            case GeomAbs_Cylinder: ElSLib::Parameters(ga.Cylinder(), q, pu, pv); break;
            case GeomAbs_Cone: ElSLib::Parameters(ga.Cone(), q, pu, pv); break;
            case GeomAbs_Sphere: ElSLib::Parameters(ga.Sphere(), q, pu, pv); break;
            default: return false;
          }
          w.SetCoord(pu, pv);
          return true;
        };
        gp_Pnt2d                                    last;
        for (int k = 0; k < n; ++k)
        {
          if ((k & 1023) == 0)
            fail_if_cancelled();
          const double t = f + (l - f) * k / (n - 1);
          const gp_Pnt q = local->Value(t);
          gp_Pnt2d     w;
          if (!elementary(q, w))
            w = k == 0 ? sas.ValueOfUV(q, 1e-9) : sas.NextValueOfUV(last, q, 1e-9);
          // (Unwrapped: each within half a period of the last.)
          if (k > 0 && ns->IsUPeriodic())
            w.SetX(w.X() + std::round((last.X() - w.X()) / ns->UPeriod()) * ns->UPeriod());
          if (k > 0 && ns->IsVPeriodic())
            w.SetY(w.Y() + std::round((last.Y() - w.Y()) / ns->VPeriod()) * ns->VPeriod());
          last = w;
          uv->SetValue(k + 1, last);
          ts->SetValue(k + 1, t);
        }
        const KernelTimer ti("replace_mapped.neighbours.interpolate");
        Geom2dAPI_Interpolate in(uv, ts, false, 1e-12);
        in.Perform();
        if (!in.IsDone())
          fail_op("a new edge has no curve on a neighbour");
        pc = in.Curve();
        for (int k = 0; k < n - 1; ++k)
        {
          const double   t = f + (l - f) * (k + 0.5) / (n - 1);
          const gp_Pnt2d w = pc->Value(t);
          dev              = std::max(dev, ns->Value(w.X(), w.Y()).Distance(local->Value(t)));
        }
        if (dev > 2e-5 * size + 1e-7)
          fail_op("a new edge's curve on a neighbour strays from it");
      }
      double                          of, ol;
      const occ::handle<Geom2d_Curve> was = BRep_Tool::CurveOnSurface(old, N, of, ol);
      if (!was.IsNull())
        align(pc, f, was->Value(of), ns);
      B.UpdateEdge(ne, pc, ns, nl, std::max({tol, dmax * 2, dev * 2}));
    }
  }
  // Edges elsewhere at a moved vertex: a straight one along the move drawn to the new vertex.
  step.emplace("replace_mapped.rest");
  ShapeAncMap ve;
  TopExp::MapShapesAndAncestors(solid, TopAbs_VERTEX, TopAbs_EDGE, ve);
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  rs->Replace(F, same ? G : TopoDS::Face(G.Reversed()));
  for (NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(emap); it.More(); it.Next())
    rs->Replace(it.Key(), it.Value());
  ShapeIMap face_edges;
  TopExp::MapShapes(F, TopAbs_EDGE, face_edges);
  std::vector<TopoDS_Shape> rebuilt;
  for (NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(vmap); it.More(); it.Next())
  {
    const TopoDS_Vertex& ov = TopoDS::Vertex(it.Key());
    const TopoDS_Vertex& nv = TopoDS::Vertex(it.Value());
    const gp_Pnt         po = BRep_Tool::Pnt(ov), pn = BRep_Tool::Pnt(nv);
    if (!ve.Contains(ov))
      continue;
    for (ShapeList::Iterator gi(ve.FindFromKey(ov)); gi.More(); gi.Next())
    {
      const TopoDS_Edge g = TopoDS::Edge(gi.Value().Oriented(TopAbs_FORWARD));
      if (face_edges.Contains(g))
        continue;
      if (po.Distance(pn) <= tol)
        fail_op("an edge at an unmoved vertex");  // (none in the cases this is for)
      TopoDS_Vertex a, z;
      TopExp::Vertices(g, a, z);
      BRepAdaptor_Curve gc(g);
      if (gc.GetType() != GeomAbs_Line || a.IsSame(z))
        fail_op("a curved edge at a moved vertex");
      const bool    at_start = a.IsSame(ov);
      const gp_Pnt  other    = BRep_Tool::Pnt(at_start ? z : a);
      const gp_Vec  along(other, pn);
      if (along.Magnitude() < tol || along.Crossed(gp_Vec(other, po)).Magnitude() > 1e-9 * along.Magnitude() * std::max(gp_Vec(other, po).Magnitude(), tol))
        fail_op("an edge at a moved vertex isn't along the move");
      TopoDS_Edge ng;
      B.MakeEdge(ng, new Geom_Line(at_start ? pn : other, gp_Dir(at_start ? gp_Vec(pn, other) : gp_Vec(other, pn))), tol);
      const TopoDS_Vertex na = at_start ? nv : TopoDS::Vertex(a), nz = at_start ? TopoDS::Vertex(z) : nv;
      B.Add(ng, na.Oriented(TopAbs_FORWARD));
      B.Add(ng, nz.Oriented(TopAbs_REVERSED));
      B.Range(ng, 0, along.Magnitude());
      // Its curves on its faces (seams twice): straight in their parameters, from the old curve's point at the
      // unmoved end to the new vertex's parameters (on a plane, cylinder or cone a ruling runs with its length).
      const double len = along.Magnitude();
      for (ShapeList::Iterator fi(ef.FindFromKey(g)); fi.More(); fi.Next())
      {
        const TopoDS_Face               N = TopoDS::Face(fi.Value());
        TopLoc_Location                 nl;
        const occ::handle<Geom_Surface> ns = BRep_Tool::Surface(N, nl);
        GeomAPI_ProjectPointOnSurf      pr(pn.Transformed(nl.Transformation().Inverted()), ns);
        if (!pr.IsDone() || pr.NbPoints() == 0 || pr.LowerDistance() > 1e-6 * size + 1e-7)
          fail_op("a moved vertex leaves a face along it");
        double mu, mv;
        pr.LowerDistanceParameters(mu, mv);
        auto line = [&](const occ::handle<Geom2d_Curve>& p, double ff, double ll) -> occ::handle<Geom2d_Curve> {
          const gp_Pnt2d keep = p->Value(at_start ? ll : ff);
          gp_Pnt2d       moved(mu, mv);
          // (The moved end on the same ruling: its u as the old curve's where the surface is periodic in u.)
          if (ns->IsUPeriodic())
            moved.SetX(moved.X() + std::round((keep.X() - moved.X()) / ns->UPeriod()) * ns->UPeriod());
          if (std::abs(moved.X() - keep.X()) <= 1e-9 * (1 + std::abs(keep.X())))
            moved.SetX(keep.X());
          const gp_Pnt2d from = at_start ? moved : keep, to = at_start ? keep : moved;
          const gp_Vec2d d(from, to);
          if (d.Magnitude() < 1e-15)
            return {};
          // Parameter t in [0, len] along the 3D line: the 2D line's speed |d| / len.
          return new Geom2d_TrimmedCurve(new Geom2d_Line(from, gp_Dir2d(d)), 0, d.Magnitude());
        };
        double                    ff, ll;
        occ::handle<Geom2d_Curve> p1 = BRep_Tool::CurveOnSurface(g, N, ff, ll);
        if (p1.IsNull())
          fail_op("an edge at a moved vertex has no curve on its face");
        occ::handle<Geom2d_Curve> q1 = line(p1, ff, ll);
        if (q1.IsNull())
          fail_op("an edge at a moved vertex is a point in its face's parameters");
        // (Its parameter scaled to the 3D line's: a 2D B-spline line over [0, len].)
        auto scaled = [&](const occ::handle<Geom2d_Curve>& q) -> occ::handle<Geom2d_Curve> {
          occ::handle<Geom2d_BSplineCurve> b = Geom2dConvert::CurveToBSplineCurve(q);
          NCollection_Array1<double>       k(1, b->NbKnots());
          b->Knots(k);
          BSplCLib::Reparametrize(0, len, k);
          b->SetKnots(k);
          return b;
        };
        if (BRep_Tool::IsClosed(g, N))
        {
          double                          f2, l2;
          const occ::handle<Geom2d_Curve> p2 = BRep_Tool::CurveOnSurface(TopoDS::Edge(g.Reversed()), N, f2, l2);
          const occ::handle<Geom2d_Curve> q2 = p2.IsNull() ? occ::handle<Geom2d_Curve>() : line(p2, f2, l2);
          if (q2.IsNull())
            fail_op("a seam at a moved vertex has no second curve");
          B.UpdateEdge(ng, scaled(q1), scaled(q2), ns, nl, tol);
        }
        else
          B.UpdateEdge(ng, scaled(q1), ns, nl, tol);
      }
      rs->Replace(g, ng);
      rebuilt.push_back(ng);
    }
  }
  // Every new edge's curves agreeing parameter for parameter (a projection's needn't).
  for (NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(emap); it.More(); it.Next())
  {
    fail_if_cancelled();
    BRepLib::SameParameter(TopoDS::Edge(it.Value()), tol);
  }
  for (const TopoDS_Shape& g : rebuilt)
  {
    fail_if_cancelled();
    BRepLib::SameParameter(TopoDS::Edge(g), tol);
  }
  fail_if_cancelled();
  const TopoDS_Shape out = rs->Apply(solid);
  fail_if_cancelled();
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    fail_op("the part with the face replaced isn't valid");
  fail_if_cancelled();
  return wrap(out);
}

std::unique_ptr<OcShape> merge_faces_on(const OcShape& in, const OcShape& on)
{
  const TopoDS_Shape&             s  = in.shape;
  TopLoc_Location                 l0;
  const occ::handle<Geom_Surface> g0 = BRep_Tool::Surface(as_face(on.shape, "face"), l0);
  // The faces on it; every other face, edge and vertex kept as it is.
  int       n = 0;
  ShapeList keep;
  ShapeIMap mine;
  for (TopExp_Explorer fx(s, TopAbs_FACE); fx.More(); fx.Next())
  {
    TopLoc_Location l;
    if (BRep_Tool::Surface(TopoDS::Face(fx.Current()), l) == g0 && l.IsEqual(l0))
    {
      ++n;
      TopExp::MapShapes(fx.Current(), TopAbs_EDGE, mine);
    }
  }
  if (n < 2)
    return wrap(s);
  ShapeIMap all;
  TopExp::MapShapes(s, TopAbs_EDGE, all);
  for (int i = 1; i <= all.Extent(); ++i)
    if (!mine.Contains(all(i)))
      keep.Append(all(i));
  ShapeUpgrade_UnifySameDomain u(s, true, true, false);
  for (ShapeList::Iterator it(keep); it.More(); it.Next())
    u.KeepShape(it.Value());
  u.Build();
  const TopoDS_Shape out = u.Shape();
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    return wrap(s);
  return wrap(out);
}

std::unique_ptr<OcShape> face_patches(const OcShape& in, uint32_t spans)
{
  const TopoDS_Face&                     f = as_face(in.shape, "face");
  TopLoc_Location                        loc;
  const occ::handle<Geom_Surface>        s  = BRep_Tool::Surface(f, loc);
  const occ::handle<Geom_BSplineSurface> bs = occ::down_cast<Geom_BSplineSurface>(s);
  if (bs.IsNull() || spans == 0)
    return wrap(f);
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  // The cuts: every `spans`-th knot inside the bounds.
  auto cuts = [spans](const NCollection_Array1<double>& k, double a, double b) {
    std::vector<double> out{a};
    int                 n = 0;
    for (int i = k.Lower(); i <= k.Upper(); ++i)
      if (k(i) > a + 1e-12 * (b - a) && k(i) < b - 1e-12 * (b - a) && ++n % (int)spans == 0)
        out.push_back(k(i));
    out.push_back(b);
    return out;
  };
  const std::vector<double> us = cuts(bs->UKnots(), u0, u1), vs = cuts(bs->VKnots(), v0, v1);
  if (us.size() == 2 && vs.size() == 2)
    return wrap(f);
  // Sewn into one sheet: as a split's tool, faces that only lie side by side don't cut a solid apart.
  const double            tol = BRep_Tool::Tolerance(f);
  BRepBuilderAPI_Sewing   sew(std::max(tol, 1e-7));
  for (size_t i = 0; i + 1 < us.size(); ++i)
    for (size_t j = 0; j + 1 < vs.size(); ++j)
    {
      BRepBuilderAPI_MakeFace mf(s, us[i], us[i + 1], vs[j], vs[j + 1], tol);
      if (!mf.IsDone())
        fail_op("a patch of the face could not be made");
      sew.Add(mf.Face().Located(loc).Oriented(f.Orientation()));
    }
  sew.Perform();
  const TopoDS_Shape out = sew.SewedShape();
  if (out.IsNull() || out.ShapeType() != TopAbs_SHELL)
    fail_op("the face's patches could not be sewn into one sheet");
  return wrap(out);
}

std::unique_ptr<OcShape> reverse_edge_curves(const OcShape& in, const OcShapeList& edges)
{
  const TopoDS_Shape& s = in.shape;
  ShapeAncMap         ef;
  TopExp::MapShapesAndAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  BRep_Builder                   B;
  bool                           any = false;
  for (const TopoDS_Shape& x : edges.items)
  {
    const TopoDS_Edge e = TopoDS::Edge(x.Oriented(TopAbs_FORWARD));
    if (!ef.Contains(e) || BRep_Tool::Degenerated(e))
      continue;
    double                        f, l;
    TopLoc_Location               cl;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, cl, f, l);
    if (c.IsNull())
      continue;
    bool seam = false;
    for (ShapeList::Iterator fi(ef.FindFromKey(e)); fi.More(); fi.Next())
      seam = seam || BRep_Tool::IsClosed(e, TopoDS::Face(fi.Value()));
    if (seam)
      continue;
    const occ::handle<Geom_Curve> rc = c->Reversed();
    const double                  rf = c->ReversedParameter(l), rl = c->ReversedParameter(f);
    const double                  tol = BRep_Tool::Tolerance(e);
    TopoDS_Edge                   ne;
    B.MakeEdge(ne, rc, cl, tol);
    TopoDS_Vertex v0, v1;
    TopExp::Vertices(e, v0, v1);
    if (v0.IsNull() || v1.IsNull())
      continue;
    B.Add(ne, v1.Oriented(TopAbs_FORWARD));
    B.Add(ne, v0.Oriented(TopAbs_REVERSED));
    B.Range(ne, rf, rl);
    for (ShapeList::Iterator fi(ef.FindFromKey(e)); fi.More(); fi.Next())
    {
      const TopoDS_Face&              F = TopoDS::Face(fi.Value());
      double                          pf, pl;
      const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, F, pf, pl);
      if (pc.IsNull())
        continue;
      TopLoc_Location                 sl;
      const occ::handle<Geom_Surface> su = BRep_Tool::Surface(F, sl);
      B.UpdateEdge(ne, pc->Reversed(), su, sl, tol);
      B.Range(ne, su, sl, pc->ReversedParameter(pl), pc->ReversedParameter(pf));
    }
    B.SameRange(ne, BRep_Tool::SameRange(e));
    B.SameParameter(ne, BRep_Tool::SameParameter(e));
    rs->Replace(e, ne.Reversed());
    any = true;
  }
  if (!any)
    return wrap(s);
  const TopoDS_Shape out = rs->Apply(s);
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    return wrap(s);
  return wrap(out);
}

std::unique_ptr<OcShape> drop_open_wires(const OcShape& in)
{
  const TopoDS_Shape&            s = in.shape;
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  bool                           any = false;
  for (TopExp_Explorer fx(s, TopAbs_FACE); fx.More(); fx.Next())
  {
    const TopoDS_Face         f = TopoDS::Face(fx.Current().Oriented(TopAbs_FORWARD));
    std::vector<TopoDS_Shape> keep;
    bool                      open = false;
    for (TopoDS_Iterator w(f); w.More(); w.Next())
    {
      const TopoDS_Shape& x = w.Value();
      // (A wire whose ends don't meet: its vertices aren't each used twice.)
      bool closed = x.ShapeType() == TopAbs_WIRE;
      if (closed)
      {
        NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> uses;
        for (TopoDS_Iterator e(x); e.More(); e.Next())
          for (TopoDS_Iterator v(e.Value()); v.More(); v.Next())
          {
            int* n = uses.ChangeSeek(v.Value().Oriented(TopAbs_FORWARD));
            if (n == nullptr)
              uses.Bind(v.Value().Oriented(TopAbs_FORWARD), 1);
            else
              ++*n;
          }
        for (NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher>::Iterator it(uses); it.More() && closed; it.Next())
          closed = it.Value() % 2 == 0;
        closed = closed && !uses.IsEmpty();
      }
      if (closed)
        keep.push_back(x);
      else
        open = true;
    }
    if (!open || keep.empty())
      continue;
    TopoDS_Face  g = TopoDS::Face(f.EmptyCopied());
    BRep_Builder B;
    for (const TopoDS_Shape& x : keep)
      B.Add(g, x);
    if (!BRepCheck_Analyzer(g).IsValid())
      continue;
    rs->Replace(f, g);
    any = true;
  }
  return wrap(any ? rs->Apply(s) : s);
}

std::unique_ptr<OcShape> extend_face(const OcShape& on, double len)
{
  const TopoDS_Face& f = as_face(on.shape, "face");
  if (!(len > 0))
    fail_invalid("the extension must be positive");
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  occ::handle<Geom_Surface> s = BRep_Tool::Surface(f);
  if (const auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(s); !t.IsNull())
    s = t->BasisSurface();
  TopoDS_Face out;
  if (occ::handle<Geom_BSplineSurface> bs = occ::down_cast<Geom_BSplineSurface>(s->Copy()); !bs.IsNull())
  {
    occ::handle<Geom_BoundedSurface> b = bs;
    for (const bool in_u : {true, false})
    {
      // (Not round a closed direction: a loft through closed curves would overlap itself.)
      if (in_u ? (bs->IsUPeriodic() || bs->IsUClosed()) : (bs->IsVPeriodic() || bs->IsVClosed()))
        continue;
      for (const bool after : {false, true})
        GeomLib::ExtendSurfByLength(b, len, 1, in_u, after);
    }
    BRepBuilderAPI_MakeFace mk(b, 1e-7);
    if (!mk.IsDone())
      fail_op("could not build the extended face");
    out = mk.Face();
  }
  else
  {
    // (Plane: lengths; others: a periodic direction whole, else by the length over the surface's scale there.)
    const bool   plane = !occ::down_cast<Geom_Plane>(s).IsNull();
    const double su = plane ? len : (s->IsUPeriodic() ? 0 : len), sv = plane ? len : (s->IsVPeriodic() ? 0 : len);
    double       a0 = u0 - su, a1 = u1 + su, b0 = v0 - sv, b1 = v1 + sv;
    if (s->IsUPeriodic())
      a0 = u0, a1 = u0 + s->UPeriod();
    if (s->IsVPeriodic())
      b0 = v0, b1 = v0 + s->VPeriod();
    BRepBuilderAPI_MakeFace mk(s, a0, a1, b0, b1, 1e-7);
    if (!mk.IsDone())
      fail_op("could not build the extended face");
    out = mk.Face();
  }
  if (f.Orientation() == TopAbs_REVERSED)
    out.Reverse();
  return wrap(out);
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
  // Tangents are derivative vectors with respect to the chord-length
  // parameterization GeomAPI_Interpolate uses (unit vector = natural speed),
  // so they are loaded unscaled: their magnitudes shape the curve.
  GeomAPI_Interpolate ip(hp, periodic, 1e-7);
  if (!tangents.empty())
  {
    auto t = points_of(tangents, "spline tangents");
    if (t.size() == 2 && n != 2)
    {
      ip.Load(gp_Vec(t[0].XYZ()), gp_Vec(t[1].XYZ()), false);
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
      ip.Load(tv, flags, false);
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
  // Relative to the radius: an absolute 1e-6 is 1 um on a metre-scale model,
  // enough for a thread's tooth (whose edges lie on the core cylinder) to
  // wobble across the cylinder and break the boolean that adds it.
  if (!BRepLib::BuildCurves3d(e, std::max(1e-12, radius * 1e-8), GeomAbs_C2, 8, nseg))
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

namespace noshape {

// A face pattern of a pocket (`faces`, the seed's walls) whose copies land on the faces holding the seed's rims, each
// of which the instance transforms (`m`, 12 values each: rows of 3 x 4) map onto itself (a tube's coaxial cylinders
// under a rotation about their axis, a plate's plane under a shift in it): each such face with every copy's rim loops
// as more holes (their curves on it the seed's, shifted), the walls copied (Parasolid's face pattern copies the faces;
// no Boolean). A cylinder band whose seam a loop would cross gets its seam moved to the widest angle clear of them
// first. `codes`, per face of the result: its index in `solid`, or -(1 + instance * walls + wall). An error where it
// doesn't apply (a copy leaving its face, near another loop or any other face): then the general cut.
std::unique_ptr<OcShape> stitch_face_copies(const OcShape& solid_in, const OcShapeList& faces_in, rust::Slice<const double> m,
                                            rust::Vec<int64_t>& codes)
{
  const bool dbg = std::getenv("NOSHAPE_STITCH_DEBUG") != nullptr;
  if (faces_in.items.empty() || m.empty() || m.size() % 12 != 0)
    fail_op("stitch: nothing to pattern");
  const int            n = (int)(m.size() / 12);
  std::vector<gp_Trsf> ts((size_t)n);
  for (int i = 0; i < n; ++i)
  {
    const double* r = m.data() + 12 * i;
    ts[(size_t)i].SetValues(r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8], r[9], r[10], r[11]);
    if (ts[(size_t)i].IsNegative() || std::abs(ts[(size_t)i].ScaleFactor() - 1) > 1e-12)
      fail_op("stitch: not a rigid motion");
  }
  ShapeIMap sel;
  for (const TopoDS_Shape& f : faces_in.items)
    sel.Add(f);
  TopoDS_Shape S = solid_in.shape;
  occ::handle<BRepTools_History> moved; // the seams moved
  BRep_Builder B;
  for (int attempt = 0;; ++attempt)
  {
    TopoDS_Shell shell;
    int          nshells = 0, nsolids = 0;
    for (TopExp_Explorer x(S, TopAbs_SHELL); x.More(); x.Next(), ++nshells)
      shell = TopoDS::Shell(x.Current());
    for (TopExp_Explorer x(S, TopAbs_SOLID); x.More(); x.Next())
      ++nsolids;
    if (nsolids != 1 || nshells != 1)
      fail_op("stitch: not one solid of one shell");
    ShapeAncMap ef;
    TopExp::MapShapesAndUniqueAncestors(S, TopAbs_EDGE, TopAbs_FACE, ef);
    // The faces holding the seed's rims, each with its rim edges.
    std::vector<TopoDS_Face> hosts;
    std::vector<ShapeIMap>   rims;
    for (int i = 1; i <= sel.Extent(); ++i)
      for (TopExp_Explorer x(sel(i), TopAbs_EDGE); x.More(); x.Next())
      {
        const ShapeList* fs = ef.Seek(x.Current());
        if (fs == nullptr)
          fail_op("stitch: a wall off the part");
        for (ShapeList::Iterator it(*fs); it.More(); it.Next())
        {
          if (sel.Contains(it.Value()))
            continue;
          size_t h = 0;
          while (h < hosts.size() && !hosts[h].IsSame(it.Value()))
            ++h;
          if (h == hosts.size())
          {
            hosts.push_back(TopoDS::Face(it.Value()));
            rims.emplace_back();
          }
          rims[h].Add(x.Current());
        }
      }
    if (hosts.empty())
      fail_op("stitch: no rims");
    // Each host's rims: whole holes of it (the seed's there), none on a seam.
    std::vector<std::vector<TopoDS_Wire>> seed_wires(hosts.size());
    for (size_t h = 0; h < hosts.size(); ++h)
    {
      const TopoDS_Face hf      = TopoDS::Face(hosts[h].Oriented(TopAbs_FORWARD));
      const TopoDS_Wire outer   = BRepTools::OuterWire(hf);
      int               covered = 0;
      for (TopoDS_Iterator w(hf); w.More(); w.Next())
      {
        int in = 0, k = 0;
        for (TopoDS_Iterator e(w.Value()); e.More(); e.Next(), ++k)
          in += rims[h].Contains(e.Value()) ? 1 : 0;
        if (in == 0)
          continue;
        if (in != k || w.Value().IsSame(outer))
          fail_op("stitch: a rim isn't a hole of its face");
        seed_wires[h].push_back(TopoDS::Wire(w.Value()));
        covered += k;
      }
      for (int i = 1; i <= rims[h].Extent(); ++i)
        if (BRep_Tool::IsClosed(TopoDS::Edge(rims[h](i)), hf))
          fail_op("stitch: a rim on a seam");
      if (covered != rims[h].Extent())
        fail_op("stitch: rims left over");
    }
    // Each host mapped onto itself by each transform, its parameters shifted by (du, dv): checked over its (u, v) box.
    std::vector<std::vector<gp_Vec2d>> shift(hosts.size(), std::vector<gp_Vec2d>((size_t)n));
    for (size_t h = 0; h < hosts.size(); ++h)
    {
      BRepAdaptor_Surface       s(hosts[h]);
      const GeomAbs_SurfaceType k = s.GetType();
      auto params = [&](const gp_Pnt& p, double& u, double& v) {
        switch (k)
        {
          case GeomAbs_Plane: ElSLib::Parameters(s.Plane(), p, u, v); return true;
          case GeomAbs_Cylinder: ElSLib::Parameters(s.Cylinder(), p, u, v); return true;
          case GeomAbs_Cone: ElSLib::Parameters(s.Cone(), p, u, v); return true;
          case GeomAbs_Sphere: ElSLib::Parameters(s.Sphere(), p, u, v); return true;
          case GeomAbs_Torus: ElSLib::Parameters(s.Torus(), p, u, v); return true;
          default: return false;
        }
      };
      double u0, u1, v0, v1;
      BRepTools::UVBounds(hosts[h], u0, u1, v0, v1);
      const double tol = 10 * BRep_Tool::Tolerance(hosts[h]);
      for (int i = 0; i < n; ++i)
      {
        const double um = 0.5 * (u0 + u1), vm = 0.5 * (v0 + v1);
        double       u, v;
        if (!params(s.Value(um, vm).Transformed(ts[(size_t)i]), u, v))
          fail_op("stitch: a host that isn't elementary");
        gp_Vec2d d(u - um, v - vm);
        if (s.IsUPeriodic())
          d.SetX(std::remainder(d.X(), s.UPeriod()));
        for (int a = 0; a <= 2; ++a)
          for (int b = 0; b <= 2; ++b)
          {
            const double uu = u0 + (u1 - u0) * a / 2, vv = v0 + (v1 - v0) * b / 2;
            if (s.Value(uu + d.X(), vv + d.Y()).Distance(s.Value(uu, vv).Transformed(ts[(size_t)i])) > tol)
              fail_op("stitch: a host the transform doesn't keep");
          }
        shift[h][(size_t)i] = d;
      }
    }
    // Each loop's curves' u on a periodic host (a copy's shifted, by whole turns so its first edge's middle is in the
    // face's range): one crossing the seam has the seam moved into the widest gap of every loop's and edge's, the band
    // then made again.
    auto loop_us = [&](size_t h, const TopoDS_Wire& w, double du, double period, double u0, double& turns) {
      std::vector<double> us;
      const TopoDS_Face   hf    = TopoDS::Face(hosts[h].Oriented(TopAbs_FORWARD));
      bool                first = true;
      for (TopoDS_Iterator e(w); e.More(); e.Next())
      {
        double                          f, l;
        const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(TopoDS::Edge(e.Value().Oriented(TopAbs_FORWARD)), hf, f, l);
        if (pc.IsNull())
          fail_op("stitch: a rim without its curve on the face");
        if (first && period > 0)
          turns = std::floor((pc->Value(0.5 * (f + l)).X() + du - u0) / period) * period;
        first = false;
        for (int k = 0; k <= 16; ++k)
          us.push_back(pc->Value(f + (l - f) * k / 16).X() + du - turns);
      }
      return us;
    };
    bool restart = false;
    for (size_t h = 0; h < hosts.size() && !restart; ++h)
    {
      BRepAdaptor_Surface s(hosts[h]);
      if (s.GetType() != GeomAbs_Cylinder || !s.IsUPeriodic())
        continue;
      const double period = s.UPeriod();
      double       u0, u1, v0, v1;
      BRepTools::UVBounds(hosts[h], u0, u1, v0, v1);
      if (u1 - u0 < period - 1e-9)
        continue;
      std::vector<double> all;
      bool                crosses = false;
      for (int i = 0; i < n; ++i)
        for (const TopoDS_Wire& w : seed_wires[h])
        {
          double turns = 0;
          for (const double u : loop_us(h, w, shift[h][(size_t)i].X(), period, u0, turns))
          {
            crosses = crosses || u < u0 - 1e-9 || u > u0 + period + 1e-9;
            all.push_back(u);
          }
        }
      if (!crosses)
        continue;
      if (attempt >= 4)
        fail_op("stitch: the seams keep crossing the copies");
      // The seed's loops, and the host's other edges but its end circles and seams.
      for (const TopoDS_Wire& w : seed_wires[h])
      {
        double turns = 0;
        for (const double u : loop_us(h, w, 0, period, u0, turns))
          all.push_back(u);
      }
      const TopoDS_Face hf = TopoDS::Face(hosts[h].Oriented(TopAbs_FORWARD));
      for (TopExp_Explorer x(hf, TopAbs_EDGE); x.More(); x.Next())
      {
        const TopoDS_Edge& e = TopoDS::Edge(x.Current());
        TopoDS_Vertex      a0, a1;
        TopExp::Vertices(e, a0, a1);
        if (BRep_Tool::IsClosed(e, hf) || (!a0.IsNull() && a0.IsSame(a1)))
          continue;
        double                          f, l;
        const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, hf, f, l);
        if (pc.IsNull())
          fail_op("stitch: an edge without its curve on the face");
        for (int k = 0; k <= 16; ++k)
          all.push_back(pc->Value(f + (l - f) * k / 16).X());
      }
      for (double& u : all)
        u = u0 + std::fmod(std::fmod(u - u0, period) + period, period);
      std::sort(all.begin(), all.end());
      double gap = all.front() + period - all.back(), alpha = all.back() + gap / 2;
      for (size_t k = 0; k + 1 < all.size(); ++k)
        if (all[k + 1] - all[k] > gap)
        {
          gap   = all[k + 1] - all[k];
          alpha = all[k] + gap / 2;
        }
      if (gap < 1e-3)
        fail_op("stitch: no room for the seam");
      const TopoDS_Shape out = band_seam_at(S, hosts[h], alpha, moved);
      if (out.IsNull())
        fail_op("stitch: the seam couldn't move");
      if (dbg)
        std::fprintf(stderr, "stitch: host %zu seam moved to %.4f\n", h, alpha);
      S       = out;
      restart = true;
    }
    if (restart)
      continue;
    // The walls copied, each instance's own.
    TopoDS_Compound walls;
    B.MakeCompound(walls);
    for (int i = 1; i <= sel.Extent(); ++i)
      B.Add(walls, sel(i));
    std::vector<std::unique_ptr<BRepBuilderAPI_Transform>> copies;
    for (int i = 0; i < n; ++i)
    {
      copies.push_back(std::make_unique<BRepBuilderAPI_Transform>(walls, ts[(size_t)i], true));
      if (!copies.back()->IsDone())
        fail_op("stitch: a copy failed");
      fail_if_cancelled();
    }
    // No copy's walls near any other face (but its hosts) or another copy's walls (the seed's among them).
    {
      auto box_of = [](const TopoDS_Shape& x) {
        Bnd_Box b;
        BRepBndLib::Add(x, b, false);
        b.Enlarge(1e-7);
        return b;
      };
      std::vector<Bnd_Box> each{box_of(walls)};
      for (int i = 0; i < n; ++i)
        each.push_back(box_of(copies[(size_t)i]->Shape()));
      for (size_t i = 0; i < each.size(); ++i)
        for (size_t j = i + 1; j < each.size(); ++j)
          if (!each[i].IsOut(each[j]))
            fail_op("stitch: copies near each other");
      for (TopExp_Explorer x(S, TopAbs_FACE); x.More(); x.Next())
      {
        if (sel.Contains(x.Current()) || std::any_of(hosts.begin(), hosts.end(), [&](const TopoDS_Face& h) { return h.IsSame(x.Current()); }))
          continue;
        const Bnd_Box fb = box_of(x.Current());
        for (size_t i = 1; i < each.size(); ++i)
          if (!each[i].IsOut(fb))
            fail_op("stitch: a copy near another face");
      }
    }
    // The hosts with the copies' rims as more holes.
    std::vector<TopoDS_Face> new_hosts;
    for (size_t h = 0; h < hosts.size(); ++h)
    {
      const TopoDS_Face               hf = TopoDS::Face(hosts[h].Oriented(TopAbs_FORWARD));
      TopLoc_Location                 hl;
      const occ::handle<Geom_Surface> hs = BRep_Tool::Surface(hf, hl);
      double                          u0, u1, v0, v1;
      BRepTools::UVBounds(hf, u0, u1, v0, v1);
      BRepAdaptor_Surface             ha(hf);
      const double                    period = ha.IsUPeriodic() ? ha.UPeriod() : 0;
      TopoDS_Face                     nf;
      B.MakeFace(nf, hs, hl, BRep_Tool::Tolerance(hf));
      for (TopoDS_Iterator w(hf); w.More(); w.Next())
        B.Add(nf, w.Value());
      BRepClass_FaceClassifier inside;
      for (int i = 0; i < n; ++i)
        for (const TopoDS_Wire& sw : seed_wires[h])
        {
          double turns = 0;
          (void)loop_us(h, sw, shift[h][(size_t)i].X(), period, u0, turns);
          TopoDS_Wire nw;
          B.MakeWire(nw);
          for (TopoDS_Iterator e(sw); e.More(); e.Next())
          {
            const TopoDS_Edge               se = TopoDS::Edge(e.Value());
            double                          f, l;
            const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(TopoDS::Edge(se.Oriented(TopAbs_FORWARD)), hf, f, l);
            const gp_Vec2d                  d(shift[h][(size_t)i].X() - turns, shift[h][(size_t)i].Y());
            const occ::handle<Geom2d_Curve> npc = occ::down_cast<Geom2d_Curve>(pc->Translated(d));
            const TopoDS_Edge ce = TopoDS::Edge(copies[(size_t)i]->ModifiedShape(se).Oriented(TopAbs_FORWARD));
            B.UpdateEdge(ce, npc, nf, BRep_Tool::Tolerance(ce));
            B.Range(ce, nf, f, l);
            B.Add(nw, ce.Oriented(se.Orientation()));
            // Inside the face as it was, off its own edges.
            inside.Perform(hf, npc->Value(0.5 * (f + l)), BRep_Tool::Tolerance(hf));
            if (inside.State() != TopAbs_IN)
              fail_op("stitch: a copy off its face");
          }
          nw.Closed(true);
          B.Add(nf, nw.Oriented(sw.Orientation()));
        }
      fail_if_cancelled();
      if (BRepCheck_Analyzer an(nf); !an.IsValid())
      {
        if (dbg)
          for (const TopAbs_ShapeEnum kk : {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_WIRE, TopAbs_FACE})
            for (TopExp_Explorer x(nf, kk); x.More(); x.Next())
              if (const occ::handle<BRepCheck_Result> r = an.Result(x.Current()); !r.IsNull())
                for (const BRepCheck_Status st : r->Status())
                  if (st != BRepCheck_NoError)
                    std::fprintf(stderr, "stitch: host %zu kind %d status %d\n", h, (int)kk, (int)st);
        fail_op("stitch: a host with the copies isn't valid");
      }
      new_hosts.push_back(TopoDS::Face(nf.Oriented(hosts[h].Orientation())));
    }
    // Each face of S by its index in `solid` (through the seams moved).
    ShapeIMap orig;
    TopExp::MapShapes(solid_in.shape, TopAbs_FACE, orig);
    NCollection_DataMap<TopoDS_Shape, int64_t, TopTools_ShapeMapHasher> orig_of;
    for (int i = 1; i <= orig.Extent(); ++i)
    {
      orig_of.Bind(orig(i), i - 1);
      if (!moved.IsNull())
        for (ShapeList::Iterator it(moved->Modified(orig(i))); it.More(); it.Next())
          orig_of.Bind(it.Value(), i - 1);
    }
    // The shell: the part's faces (the hosts replaced), then each copy's walls.
    NCollection_DataMap<TopoDS_Shape, int64_t, TopTools_ShapeMapHasher> code_of;
    TopoDS_Shell                                                          ns;
    B.MakeShell(ns);
    for (TopoDS_Iterator x(shell); x.More(); x.Next())
    {
      size_t h = 0;
      while (h < hosts.size() && !hosts[h].IsSame(x.Value()))
        ++h;
      const TopoDS_Shape f = h < hosts.size() ? TopoDS_Shape(new_hosts[h].Oriented(x.Value().Orientation())) : x.Value();
      const int64_t*     o = orig_of.Seek(x.Value());
      if (o == nullptr)
        fail_op("stitch: a face without its source");
      B.Add(ns, f);
      code_of.Bind(f, *o);
    }
    const int64_t nw = sel.Extent();
    for (int i = 0; i < n; ++i)
      for (TopoDS_Iterator x(shell); x.More(); x.Next())
      {
        const int j = sel.FindIndex(x.Value());
        if (j == 0)
          continue;
        const TopoDS_Shape c = copies[(size_t)i]->ModifiedShape(x.Value()).Oriented(x.Value().Orientation());
        B.Add(ns, c);
        code_of.Bind(c, -(1 + i * nw + (j - 1)));
      }
    ns.Closed(true);
    TopoDS_Solid so;
    B.MakeSolid(so);
    B.Add(so, ns.Oriented(shell.Orientation()));
    ShapeIMap out_faces;
    TopExp::MapShapes(so, TopAbs_FACE, out_faces);
    codes.clear();
    for (int i = 1; i <= out_faces.Extent(); ++i)
    {
      const int64_t* c = code_of.Seek(out_faces(i));
      if (c == nullptr)
        fail_op("stitch: a face without its source");
      codes.push_back(*c);
    }
    return wrap(so);
  }
}

} // namespace noshape
