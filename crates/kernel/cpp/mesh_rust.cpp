// The display mesh of analytic faces by noshape's own mesher (crates/brep `mesh`, through `brep_mesh`): the
// faces it takes get their triangulation and their edges' polygons here, as BRepMesh would leave them, and
// BRepMesh meshes only the rest (mesh.cpp). NOSHAPE_MESH_RUST=0: off.
#include "internal.hpp"

#include <BRepMesh_ShapeTool.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_BezierCurve.hxx>
#include <Geom_SurfaceOfLinearExtrusion.hxx>
#include <Geom_SurfaceOfRevolution.hxx>
#include <Precision.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <Geom_Ellipse.hxx>
#include <GeomConvert.hxx>
#include <BRepTools.hxx>
#include <NCollection_Array2.hxx>
#include <Geom_Circle.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2dConvert.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <OSD_Parallel.hxx>
#include <Poly_Triangulation.hxx>
#include <TopoDS_Iterator.hxx>

#include <cstdlib>
#include <string>
#include <algorithm>
#include <tuple>
#include <unordered_map>

namespace noshape {

namespace {

occ::handle<Geom_Curve> basis(occ::handle<Geom_Curve> c)
{
  while (true)
  {
    const occ::handle<Geom_TrimmedCurve> t = occ::down_cast<Geom_TrimmedCurve>(c);
    if (t.IsNull())
      return c;
    c = t->BasisCurve();
  }
}

// The input of `brep_mesh` (its slices).
struct MeshIn
{
  std::vector<double>   vertices, curves, surfaces, nurbs, pcurves;
  std::vector<uint32_t> edges, face_loops, loop_uses, uses;
  double                linear = 0, angular = 0;
};

void push3(std::vector<double>& v, const gp_XYZ& p)
{
  v.push_back(p.X());
  v.push_back(p.Y());
  v.push_back(p.Z());
}

void put3(double* v, const gp_XYZ& p)
{
  v[0] = p.X();
  v[1] = p.Y();
  v[2] = p.Z();
}

// f(i) for i in 1..=n, in runs of 64 on OCCT's pool when `parallel` and there are many (a small part's few
// hundred stay here: waking the pool cost more, up to 6 ms on a loaded machine for live-30's 150 edges).
template <class F>
void in_chunks(int n, bool parallel, const F& f)
{
  constexpr int run = 64;
  OSD_Parallel::For(
    0, (n + run - 1) / run,
    [&](int c) {
      for (int i = c * run + 1; i <= std::min(n, (c + 1) * run); ++i)
        f(i);
    },
    !parallel || n < 1024);
}

// B-splines for `brep_mesh` (`in.nurbs`, back to back; returns where it starts), in the layout of rust_boolean.cpp's
// push_nurbs / push_nurbs_surface (brep_bridge.rs nurbs_at reads it): a curve as 0, 3, degree, knot count (each
// knot as often as its multiplicity), pole count, rational, knots, poles, weights; a surface as 1, degrees, knot
// counts, pole counts, rational, knots (u, v), poles (row by row, u first), weights. Periodic ones made
// non-periodic (the same curve, the same parameters), moved by `loc`.
double push_bspline(std::vector<double>& out, const occ::handle<Geom_BSplineCurve>& c0, const TopLoc_Location& loc)
{
  occ::handle<Geom_BSplineCurve> c = occ::down_cast<Geom_BSplineCurve>(c0->Copy());
  if (c->IsPeriodic())
    c->SetNotPeriodic();
  if (!loc.IsIdentity())
    c->Transform(loc.Transformation());
  const double                      at = static_cast<double>(out.size());
  const NCollection_Array1<double>& k  = c->KnotSequence();
  for (const double x : {0.0, 3.0, (double)c->Degree(), (double)k.Length(), (double)c->NbPoles(), c->IsRational() ? 1.0 : 0.0})
    out.push_back(x);
  for (int i = k.Lower(); i <= k.Upper(); ++i)
    out.push_back(k(i));
  for (int i = 1; i <= c->NbPoles(); ++i)
    push3(out, c->Pole(i).XYZ());
  if (c->IsRational())
    for (int i = 1; i <= c->NbPoles(); ++i)
      out.push_back(c->Weight(i));
  return at;
}

// An edge's curve on a B-spline face, at the edge's own parameters (its 3D curve's: same parameter and range), as a
// B-spline; a B-spline's or a line's only (a circle's conversion would change its parameters). Null when none.
occ::handle<Geom2d_BSplineCurve> pcurve_of(const TopoDS_Edge& e, const TopoDS_Face& f)
{
  if (!BRep_Tool::SameParameter(e) || !BRep_Tool::SameRange(e))
    return {};
  double                          a, b;
  const occ::handle<Geom2d_Curve> c = BRep_Tool::CurveOnSurface(e, f, a, b);
  if (c.IsNull() || !(b > a))
    return {};
  occ::handle<Geom2d_Curve> basis = c;
  while (const occ::handle<Geom2d_TrimmedCurve> t = occ::down_cast<Geom2d_TrimmedCurve>(basis))
    basis = t->BasisCurve();
  if (occ::down_cast<Geom2d_BSplineCurve>(basis).IsNull() && occ::down_cast<Geom2d_Line>(basis).IsNull())
    return {};
  try
  {
    return Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(c, a, b));
  }
  catch (const Standard_Failure&)
  {
    return {};
  }
}

// A curve on a face's parameters as a 3D B-spline at z 0 (the mesher's curve layout), where in `out`.
double push_pcurve(std::vector<double>& out, const occ::handle<Geom2d_BSplineCurve>& c)
{
  NCollection_Array1<gp_Pnt> poles(1, c->NbPoles());
  NCollection_Array1<double> weights(1, c->NbPoles()), knots(1, c->NbKnots());
  NCollection_Array1<int>    mults(1, c->NbKnots());
  for (int i = 1; i <= c->NbPoles(); ++i)
  {
    poles(i)   = gp_Pnt(c->Pole(i).X(), c->Pole(i).Y(), 0);
    weights(i) = c->Weight(i);
  }
  for (int i = 1; i <= c->NbKnots(); ++i)
  {
    knots(i) = c->Knot(i);
    mults(i) = c->Multiplicity(i);
  }
  const occ::handle<Geom_BSplineCurve> c3 = c->IsRational() ? new Geom_BSplineCurve(poles, weights, knots, mults, c->Degree(), c->IsPeriodic())
                                                            : new Geom_BSplineCurve(poles, knots, mults, c->Degree(), c->IsPeriodic());
  return push_bspline(out, c3, TopLoc_Location());
}

double push_bspline_surface(std::vector<double>& out, const occ::handle<Geom_BSplineSurface>& s0, const TopLoc_Location& loc)
{
  occ::handle<Geom_BSplineSurface> s = occ::down_cast<Geom_BSplineSurface>(s0->Copy());
  if (s->IsUPeriodic())
    s->SetUNotPeriodic();
  if (s->IsVPeriodic())
    s->SetVNotPeriodic();
  if (!loc.IsIdentity())
    s->Transform(loc.Transformation());
  const double                      at       = static_cast<double>(out.size());
  const NCollection_Array1<double>& ku       = s->UKnotSequence();
  const NCollection_Array1<double>& kv       = s->VKnotSequence();
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

// A face's B-spline surface (under a rectangular trim too), or null.
// A face's surface as a B-spline with its own parameters, where it is one: a B-spline, or a B-spline or Bezier curve
// extruded along a line (exactly: the curve's poles at the face's two v bounds, degree 1 across, the curve's weights
// in both rows; S(u, v) = C(u) + v D as OCCT's). A gear's tooth flanks are such (cf-drive-train-generator's 1928, and
// with them the gear's plates and cylinders their edges are shared with, 10.8 s of BRepMesh; core 11-06).
occ::handle<Geom_BSplineSurface> bspline_of(occ::handle<Geom_Surface> s, const TopoDS_Face& f)
{
  while (const occ::handle<Geom_RectangularTrimmedSurface> t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
    s = t->BasisSurface();
  if (const occ::handle<Geom_BSplineSurface> b = occ::down_cast<Geom_BSplineSurface>(s))
    return b;
  const occ::handle<Geom_SurfaceOfLinearExtrusion> ex = occ::down_cast<Geom_SurfaceOfLinearExtrusion>(s);
  if (ex.IsNull())
    return {};
  occ::handle<Geom_Curve> c = ex->BasisCurve();
  while (const occ::handle<Geom_TrimmedCurve> t = occ::down_cast<Geom_TrimmedCurve>(c))
    c = t->BasisCurve();
  occ::handle<Geom_BSplineCurve> bc = occ::down_cast<Geom_BSplineCurve>(c);
  if (bc.IsNull())
    if (const occ::handle<Geom_BezierCurve> bz = occ::down_cast<Geom_BezierCurve>(c))
      bc = GeomConvert::CurveToBSplineCurve(bz);
  if (bc.IsNull() || bc->IsPeriodic())
    return {};
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  if (!(v1 > v0))
    return {};
  const double margin = 0.01 * (v1 - v0);
  v0 -= margin;
  v1 += margin;
  const gp_Vec d(ex->Direction());
  const int    n = bc->NbPoles();
  NCollection_Array2<gp_Pnt> poles(1, n, 1, 2);
  NCollection_Array2<double> weights(1, n, 1, 2);
  for (int i = 1; i <= n; ++i)
  {
    poles(i, 1)   = bc->Pole(i).Translated(d * v0);
    poles(i, 2)   = bc->Pole(i).Translated(d * v1);
    weights(i, 1) = weights(i, 2) = bc->Weight(i);
  }
  NCollection_Array1<double> vk(1, 2);
  NCollection_Array1<int>    vm(1, 2);
  vk(1) = v0;
  vk(2) = v1;
  vm(1) = vm(2) = 2;
  try
  {
    return bc->IsRational() ? new Geom_BSplineSurface(poles, weights, bc->Knots(), vk, bc->Multiplicities(), vm, bc->Degree(), 1)
                            : new Geom_BSplineSurface(poles, bc->Knots(), vk, bc->Multiplicities(), vm, bc->Degree(), 1);
  }
  catch (const Standard_Failure&)
  {
    return {};
  }
}

// A surface of revolution's curve as a B-spline (a B-spline's or Bezier's, open) and its axis, when the curve lies in
// a plane through the axis (noshape's mesher finds a point's parameters so: its angle, then the curve's nearest).
// A circle turned round an axis in its plane and clear of it: a torus (cf-o-ring-generator's rings, whose surfaces
// of revolution close both ways and so weren't meshed here). Its frame left-handed where the revolution's own
// normal points into the tube, so the face keeps its side.
bool revolution_torus(const occ::handle<Geom_Surface>& s, gp_Ax3& frame, double& major, double& minor)
{
  const occ::handle<Geom_SurfaceOfRevolution> rev = occ::down_cast<Geom_SurfaceOfRevolution>(s);
  if (rev.IsNull())
    return false;
  occ::handle<Geom_Curve> c = rev->BasisCurve();
  while (const occ::handle<Geom_TrimmedCurve> t = occ::down_cast<Geom_TrimmedCurve>(c))
    c = t->BasisCurve();
  gp_Circ circ;
  if (const occ::handle<Geom_Circle> ci = occ::down_cast<Geom_Circle>(c); !ci.IsNull())
    circ = ci->Circ();
  else if (const occ::handle<Geom_Ellipse> el = occ::down_cast<Geom_Ellipse>(c); !el.IsNull() && std::abs(el->MajorRadius() - el->MinorRadius()) <= 1e-12 * el->MajorRadius())
    circ = gp_Circ(el->Position(), el->MajorRadius());
  else
    return false;
  const gp_Ax1 axis = rev->Axis();
  const gp_Vec d(axis.Direction());
  const gp_Vec oc(axis.Location(), circ.Location());
  const gp_Vec along = d * oc.Dot(d);
  const gp_Vec off   = oc - along;
  const gp_Vec cn(circ.Axis().Direction());
  if (off.Magnitude() <= circ.Radius() * (1.0 + 1e-9) || std::abs(cn.Dot(d)) > 1e-9 || std::abs(cn.Dot(off.Normalized())) > 1e-9)
    return false;
  const gp_Pnt origin = axis.Location().Translated(along);
  major              = off.Magnitude();
  minor              = circ.Radius();
  // The revolution's normal at a point, against the way out of the tube there.
  double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
  rev->Bounds(u0, u1, v0, v1);
  const double v = Precision::IsInfinite(v0) || Precision::IsInfinite(v1) ? 0.3 : v0 + 0.37 * (v1 - v0);
  gp_Pnt       p;
  gp_Vec       du, dv;
  rev->D1(0.3, v, p, du, dv);
  const gp_Vec op(origin, p);
  const gp_Vec radial = op - d * op.Dot(d);
  if (radial.Magnitude() < 1e-12)
    return false;
  const gp_Pnt centre  = origin.Translated(radial.Normalized() * major);
  const bool   outward = du.Crossed(dv).Dot(gp_Vec(centre, p)) > 0.0;
  frame                = gp_Ax3(origin, axis.Direction(), gp_Dir(off));
  if (!outward)
    frame.YReverse();
  return true;
}

occ::handle<Geom_BSplineCurve> revolved_curve(const occ::handle<Geom_Surface>& s, gp_Ax1& axis)
{
  const occ::handle<Geom_SurfaceOfRevolution> rv = occ::down_cast<Geom_SurfaceOfRevolution>(s);
  if (rv.IsNull())
    return {};
  const occ::handle<Geom_Curve> c0 = rv->BasisCurve();
  occ::handle<Geom_Curve>       c  = c0;
  while (const occ::handle<Geom_TrimmedCurve> t = occ::down_cast<Geom_TrimmedCurve>(c))
    c = t->BasisCurve();
  occ::handle<Geom_BSplineCurve> bc = occ::down_cast<Geom_BSplineCurve>(c);
  try
  {
    if (bc.IsNull() && !occ::down_cast<Geom_BezierCurve>(c).IsNull())
      bc = GeomConvert::CurveToBSplineCurve(c);
    // A circle or ellipse exactly (rational), on its own parameters: the faces' curves on it then don't fit them
    // and the mesher finds its nodes' parameters on it instead.
    else if (bc.IsNull() && (!occ::down_cast<Geom_Circle>(c).IsNull() || !occ::down_cast<Geom_Ellipse>(c).IsNull()))
      bc = GeomConvert::CurveToBSplineCurve(c0);
    if (!bc.IsNull() && bc->IsPeriodic())
    {
      bc = occ::down_cast<Geom_BSplineCurve>(bc->Copy());
      bc->SetNotPeriodic();
    }
  }
  catch (const Standard_Failure&)
  {
    return {};
  }
  if (bc.IsNull())
    return {};
  axis                = rv->Axis();
  const gp_XYZ o      = axis.Location().XYZ(), k = axis.Direction().XYZ();
  gp_XYZ       e0     = gp_XYZ(0, 0, 0);
  double       size   = 0;
  for (int i = 1; i <= bc->NbPoles(); ++i)
  {
    const gp_XYZ d   = bc->Pole(i).XYZ() - o;
    const gp_XYZ off = d - k * d.Dot(k);
    size             = std::max(size, d.Modulus());
    if (off.Modulus() > e0.Modulus())
      e0 = off;
  }
  if (e0.Modulus() <= 1e-12 * std::max(size, 1e-9))
    return {};
  const gp_XYZ n = k.Crossed(e0.Normalized());
  for (int i = 1; i <= bc->NbPoles(); ++i)
    if (std::abs((bc->Pole(i).XYZ() - o).Dot(n)) > 1e-9 * std::max(size, 1e-9))
      return {};
  return bc;
}

} // namespace

// B-spline faces in noshape's mesher (NOSHAPE_MESH_BSPLINE=0: BRepMesh's, as before).
static bool bspline_faces_on()
{
  static const bool on = [] {
    const char* v = std::getenv("NOSHAPE_MESH_BSPLINE");
    return v == nullptr || std::string(v) != "0";
  }();
  return on;
}

bool rust_mesh_on()
{
  static const bool on = [] {
    const char* v = std::getenv("NOSHAPE_MESH_RUST");
    return v == nullptr || std::string(v) != "0";
  }();
  return on && brep_mesh_on();
}

int rust_mesh_faces(const ShapeIMap& faces, const ShapeIMap& edges, const ShapeIMap& vertices, const std::vector<char>& fresh, double lin, double ang,
                    bool parallel, std::vector<char>& taken, std::vector<RustFace>& meshes, std::vector<std::vector<std::array<float, 3>>>& polys,
                    const std::function<void(bool)>& unlocked)
{
  const KernelTimer timer("mesh.rust");
  auto              lap = std::make_unique<KernelTimer>("mesh.rust.in");
  MeshIn            in;
  in.linear  = lin;
  in.angular = ang;
  auto              sub = std::make_unique<KernelTimer>("mesh.rust.in.vertices");
  in.vertices.reserve(3 * (size_t)vertices.Extent());
  for (int i = 1; i <= vertices.Extent(); ++i)
    push3(in.vertices, BRep_Tool::Pnt(TopoDS::Vertex(vertices(i))).XYZ());
  // Edges: kind (0 line, 1 circle, 3 another curve or none), centre, normal, x direction, radius, parameter
  // range (13 numbers); 6 an ellipse: centre, major and minor axis vectors, parameter range; start and end vertex. Each edge's own slots, read in parallel (OCCT's pool, as
  // BRepMesh's InParallel under the same lock).
  sub = std::make_unique<KernelTimer>("mesh.rust.in.edges");
  const int ne = edges.Extent();
  in.edges.assign(2 * (size_t)ne, 0);
  in.curves.assign(13 * (size_t)ne, 0.0);
  in_chunks(ne, parallel, [&](int i) {
    uint32_t*         ev = &in.edges[2 * (size_t)(i - 1)];
    double*           cv = &in.curves[13 * (size_t)(i - 1)];
    const TopoDS_Edge e  = TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     v1, v2;
    TopExp::Vertices(e, v1, v2);
    const int a = v1.IsNull() ? 0 : vertices.FindIndex(v1), b = v2.IsNull() ? 0 : vertices.FindIndex(v2);
    ev[0]       = static_cast<uint32_t>(a > 0 ? a - 1 : 0);
    ev[1]       = static_cast<uint32_t>(b > 0 ? b - 1 : 0);
    TopLoc_Location               loc;
    double                        f = 0, l = 0;
    const bool                    degenerated = BRep_Tool::Degenerated(e);
    const occ::handle<Geom_Curve> c  = degenerated || a <= 0 || b <= 0 ? occ::handle<Geom_Curve>() : BRep_Tool::Curve(e, loc, f, l);
    const occ::handle<Geom_Curve> bc = c.IsNull() ? c : basis(c);
    if (degenerated && a > 0 && a == b)
    {
      // An apex or pole: its parameters are its face's u.
      BRep_Tool::Range(e, f, l);
      cv[0] = 2;
    }
    else if (!bc.IsNull() && !occ::down_cast<Geom_Line>(bc).IsNull())
      cv[0] = 0;
    else if (const occ::handle<Geom_Circle> circ = bc.IsNull() ? occ::handle<Geom_Circle>() : occ::down_cast<Geom_Circle>(bc); !circ.IsNull())
    {
      const gp_Circ g = circ->Circ().Transformed(loc.Transformation());
      cv[0]           = 1;
      put3(cv + 1, g.Location().XYZ());
      put3(cv + 4, g.Axis().Direction().XYZ());
      put3(cv + 7, g.XAxis().Direction().XYZ());
      cv[10] = g.Radius();
    }
    else if (const occ::handle<Geom_Ellipse> el = bc.IsNull() ? occ::handle<Geom_Ellipse>() : occ::down_cast<Geom_Ellipse>(bc); !el.IsNull())
    {
      const gp_Elips g = el->Elips().Transformed(loc.Transformation());
      cv[0]            = 6;
      put3(cv + 1, g.Location().XYZ());
      put3(cv + 4, g.XAxis().Direction().XYZ() * g.MajorRadius());
      put3(cv + 7, g.YAxis().Direction().XYZ() * g.MinorRadius());
    }
    else if (!bc.IsNull() && !occ::down_cast<Geom_BSplineCurve>(bc).IsNull())
      cv[0] = 5; // a B-spline: flattened below, in turn
    else
      cv[0] = 3;
    cv[11] = f;
    cv[12] = l;
  });
  // The B-spline edges' curves (cv[1]: where in in.nurbs).
  for (int i = 1; i <= ne; ++i)
  {
    double* cv = &in.curves[13 * (size_t)(i - 1)];
    if (cv[0] != 5)
      continue;
    TopLoc_Location loc;
    double          f = 0, l = 0;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD)), loc, f, l);
    cv[1] = push_bspline(in.nurbs, occ::down_cast<Geom_BSplineCurve>(basis(c)), loc);
  }
  // Faces: kind (0 plane, 1 cylinder, 2 cone, 3 torus, 4 sphere, 255 not meshed here) and 12 numbers of geometry (below);
  // their wires' edge uses. One face per TFace: the others (a pattern's instances) take its triangulation.
  sub = std::make_unique<KernelTimer>("mesh.rust.in.faces");
  std::vector<int>                     first(faces.Extent() + 1, 0);
  std::unordered_map<const void*, int> by_tface;
  std::vector<int>                     face_of; // input face -> faces index
  std::vector<int>                     input_of(faces.Extent() + 1, -1); // faces index -> input face
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    if (!fresh[(size_t)i])
      continue;
    const auto [it, added] = by_tface.emplace(faces(i).TShape().get(), i);
    first[(size_t)i]       = it->second;
    if (added)
    {
      input_of[(size_t)i] = (int)face_of.size();
      face_of.push_back(i);
    }
  }
  // Each first face read on its own (in parallel), then joined in order.
  struct FaceIn
  {
    std::array<double, 13> surface{};
    std::vector<uint32_t>  uses, loop_ends;
    // A B-spline face's: each use's edge on it (BRepMesh's own fallback has its own).
    std::vector<occ::handle<Geom2d_BSplineCurve>> pcurves;
    occ::handle<Geom_BSplineSurface>              nurbs;
    occ::handle<Geom_BSplineCurve>                revolved; // a surface of revolution's curve
  };
  std::vector<FaceIn> read(face_of.size());
  in_chunks((int)face_of.size(), parallel, [&](int j1) {
    FaceIn&                         r = read[(size_t)j1 - 1];
    const TopoDS_Face&              f = TopoDS::Face(faces(face_of[(size_t)j1 - 1]));
    TopLoc_Location                 loc;
    occ::handle<Geom_Surface>       s  = BRep_Tool::Surface(f, loc);
    // A rectangular trim keeps its basis's parametrisation.
    while (const occ::handle<Geom_RectangularTrimmedSurface> t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
      s = t->BasisSurface();
    const bool                      mf = s.IsNull() || is_mesh_face(f);
    const occ::handle<Geom_Plane>   pl = mf ? occ::handle<Geom_Plane>() : occ::down_cast<Geom_Plane>(s);
    const occ::handle<Geom_CylindricalSurface> cyl = mf ? occ::handle<Geom_CylindricalSurface>() : occ::down_cast<Geom_CylindricalSurface>(s);
    const occ::handle<Geom_ConicalSurface>     con = mf ? occ::handle<Geom_ConicalSurface>() : occ::down_cast<Geom_ConicalSurface>(s);
    const occ::handle<Geom_ToroidalSurface>    tor = mf ? occ::handle<Geom_ToroidalSurface>() : occ::down_cast<Geom_ToroidalSurface>(s);
    const occ::handle<Geom_SphericalSurface>   sph = mf ? occ::handle<Geom_SphericalSurface>() : occ::down_cast<Geom_SphericalSurface>(s);
    const occ::handle<Geom_BSplineSurface>     bss = mf ? occ::handle<Geom_BSplineSurface>() : bspline_of(s, f);
    r.nurbs                                        = bss;
    gp_Ax1                                     rev_axis;
    gp_Ax3                                     rtor_frame;
    double                                     rtor_major = 0, rtor_minor = 0;
    const bool rtor = !mf && tor.IsNull() && bss.IsNull() && revolution_torus(s, rtor_frame, rtor_major, rtor_minor);
    if (!mf && bss.IsNull() && !rtor)
      r.revolved = revolved_curve(s, rev_axis);
    bool ok = (!pl.IsNull() || !cyl.IsNull() || !con.IsNull() || !tor.IsNull() || rtor || !sph.IsNull() || ((!bss.IsNull() || !r.revolved.IsNull()) && bspline_faces_on())) && (f.Orientation() == TopAbs_FORWARD || f.Orientation() == TopAbs_REVERSED);
    // Its loops, meshed here or not (a face BRepMesh meshes tells which edges it splits its own way).
    for (TopoDS_Iterator wi(f); wi.More(); wi.Next())
    {
      if (wi.Value().ShapeType() != TopAbs_WIRE)
      {
        ok = false;
        continue;
      }
      for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
      {
        const int k = edges.FindIndex(ei.Value());
        if (k <= 0 || (ei.Value().Orientation() != TopAbs_FORWARD && ei.Value().Orientation() != TopAbs_REVERSED))
        {
          ok = false;
          continue;
        }
        r.uses.push_back(static_cast<uint32_t>(2 * (k - 1) + (ei.Value().Orientation() == TopAbs_REVERSED ? 1 : 0)));
        r.pcurves.push_back(bss.IsNull() && r.revolved.IsNull() ? occ::handle<Geom2d_BSplineCurve>() : pcurve_of(TopoDS::Edge(ei.Value()), f));
      }
      r.loop_ends.push_back(static_cast<uint32_t>(r.uses.size()));
    }
    double* sv = r.surface.data();
    if (ok && !pl.IsNull())
    {
      const gp_Ax3 a = pl->Position().Transformed(loc.Transformation());
      sv[0]          = 0;
      put3(sv + 1, a.Location().XYZ());
      put3(sv + 4, a.XDirection().Crossed(a.YDirection()).XYZ());
      put3(sv + 7, a.XDirection().XYZ());
    }
    else if (ok && !sph.IsNull())
    {
      // Kind, origin, axis, x direction, radius, (spare), whether the frame is right-handed.
      const gp_Ax3 a = sph->Position().Transformed(loc.Transformation());
      sv[0]          = 4;
      put3(sv + 1, a.Location().XYZ());
      put3(sv + 4, a.Direction().XYZ());
      put3(sv + 7, a.XDirection().XYZ());
      sv[10] = sph->Radius();
      sv[12] = a.Direct() ? 1.0 : 0.0;
    }
    else if (ok && (!tor.IsNull() || rtor))
    {
      // Kind, origin, axis, x direction, major and minor radius, whether the frame is right-handed.
      const gp_Ax3 a = (rtor ? rtor_frame : tor->Position()).Transformed(loc.Transformation());
      sv[0]          = 3;
      put3(sv + 1, a.Location().XYZ());
      put3(sv + 4, a.Direction().XYZ());
      put3(sv + 7, a.XDirection().XYZ());
      sv[10] = rtor ? rtor_major : tor->MajorRadius();
      sv[11] = rtor ? rtor_minor : tor->MinorRadius();
      sv[12] = a.Direct() ? 1.0 : 0.0;
    }
    else if (ok && !bss.IsNull())
      sv[0] = 5; // a B-spline: flattened below, in turn (sv[1]: where in in.nurbs)
    else if (ok && !r.revolved.IsNull())
    {
      // A surface of revolution: its curve flattened below (sv[1]), the axis' origin and direction.
      const gp_Ax1 a = rev_axis.Transformed(loc.Transformation());
      sv[0]          = 6;
      put3(sv + 2, a.Location().XYZ());
      put3(sv + 5, a.Direction().XYZ());
    }
    else if (ok)
    {
      // Kind, origin, axis, x direction, radius, half-angle, whether the frame is right-handed.
      const gp_Ax3 a = (cyl.IsNull() ? con->Position() : cyl->Position()).Transformed(loc.Transformation());
      sv[0]          = cyl.IsNull() ? 2 : 1;
      put3(sv + 1, a.Location().XYZ());
      put3(sv + 4, a.Direction().XYZ());
      put3(sv + 7, a.XDirection().XYZ());
      sv[10] = cyl.IsNull() ? con->RefRadius() : cyl->Radius();
      sv[11] = cyl.IsNull() ? con->SemiAngle() : 0.0;
      sv[12] = a.Direct() ? 1.0 : 0.0;
    }
    else
      sv[0] = 255; // meshed by BRepMesh
  });
  for (size_t j = 0; j < read.size(); ++j)
    if (read[j].surface[0] == 5)
    {
      TopLoc_Location loc;
      BRep_Tool::Surface(TopoDS::Face(faces(face_of[j])), loc);
      read[j].surface[1] = push_bspline_surface(in.nurbs, read[j].nurbs, loc);
    }
    else if (read[j].surface[0] == 6)
    {
      TopLoc_Location loc;
      BRep_Tool::Surface(TopoDS::Face(faces(face_of[j])), loc);
      read[j].surface[1] = push_bspline(in.nurbs, read[j].revolved, loc);
    }
  in.face_loops.push_back(0);
  in.loop_uses.push_back(0);
  for (const FaceIn& r : read)
  {
    const uint32_t u0 = (uint32_t)in.uses.size();
    in.uses.insert(in.uses.end(), r.uses.begin(), r.uses.end());
    for (size_t k = 0; k < r.uses.size(); ++k)
      in.pcurves.push_back((r.surface[0] == 5 || r.surface[0] == 6) && k < r.pcurves.size() && !r.pcurves[k].IsNull() ? push_pcurve(in.nurbs, r.pcurves[k]) : -1.0);
    for (const uint32_t e : r.loop_ends)
      in.loop_uses.push_back(u0 + e);
    in.surfaces.insert(in.surfaces.end(), r.surface.begin(), r.surface.end());
    in.face_loops.push_back(static_cast<uint32_t>(in.loop_uses.size() - 1));
  }
  if (face_of.empty())
    return 0;
  auto out = std::make_shared<MeshOut>();
  sub.reset();
  lap = std::make_unique<KernelTimer>("mesh.rust.mesh");
  auto f64s = [](const std::vector<double>& v) { return rust::Slice<const double>(v.data(), v.size()); };
  auto u32s = [](const std::vector<uint32_t>& v) { return rust::Slice<const uint32_t>(v.data(), v.size()); };
  if (unlocked)
    unlocked(true);
  const bool meshed = brep_mesh(f64s(in.vertices), u32s(in.edges), f64s(in.curves), f64s(in.surfaces), f64s(in.nurbs), u32s(in.face_loops), u32s(in.loop_uses), u32s(in.uses), f64s(in.pcurves), in.linear, in.angular, *out);
  if (unlocked)
    unlocked(false);
  // (Cancelled: its faces declined, not left to BRepMesh.)
  fail_if_cancelled();
  if (!meshed)
    return 0;
  // The output through plain pointers (a rust::Vec's [] asks Rust for its data each time).
  const uint8_t*  face_ok    = out->face_ok.data();
  const uint32_t* face_nodes = out->face_nodes.data();
  const uint32_t* face_tris  = out->face_tris.data();
  const double*   nodes      = out->nodes.data();
  const uint32_t* use_nodes  = out->use_nodes.data();
  lap = std::make_unique<KernelTimer>("mesh.rust.out");
  // Per meshed face: its ranges of the output; per edge of one: its polyline (in the map's FORWARD direction,
  // as mesh.cpp's edges loop wants it).
  std::vector<size_t> use_at(faces.Extent() + 1, 0); // its first entry in out.use_nodes
  int                 n  = 0;
  size_t              un = 0;
  for (size_t j = 0; j < face_of.size(); ++j)
  {
    const int i = face_of[j];
    if (!face_ok[j])
      continue; // no use_nodes for a declined face
    RustFace& r = meshes[(size_t)i];
    r.out       = out;
    r.n0        = face_nodes[j];
    r.n1        = face_nodes[j + 1];
    r.t0        = face_tris[j];
    r.t1        = face_tris[j + 1];
    use_at[(size_t)i] = un;
    for (uint32_t l = in.face_loops[j]; l < in.face_loops[j + 1]; ++l)
      for (uint32_t u = in.loop_uses[l]; u < in.loop_uses[l + 1]; ++u)
        un += 1 + use_nodes[un];
  }
  // Edge polylines of the meshed faces' edges (a pattern's instances: the first's, moved), and the instances'
  // meshes (the first's, moved). Each edge's polyline comes from its first meshed face's use: found here in
  // turn, then made in parallel (made in turn, 1.9 ms of the 400-fin plate's 15.5 on the perf server).
  struct PolyFrom
  {
    int      face = 0; // 0: none
    size_t   at   = 0; // its count in out.use_nodes
    uint32_t base = 0; // the first face's first node
  };
  std::vector<PolyFrom> from((size_t)edges.Extent());
  std::vector<int>      ks;
  // The moved instances' uses: (face, edge, its count's place in out.use_nodes).
  std::vector<std::tuple<int, int, size_t>> instance_uses;
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    if (!fresh[(size_t)i] || first[(size_t)i] == 0 || !meshes[(size_t)first[(size_t)i]].out)
      continue;
    const int          i0 = first[(size_t)i];
    const TopoDS_Face& f  = TopoDS::Face(faces(i));
    taken[(size_t)i]      = 1;
    ++n;
    // From the first face's nodes (global) to this face's place.
    const bool moved = i != i0 && !f.Location().IsEqual(faces(i0).Location());
    if (i != i0)
    {
      meshes[(size_t)i]       = meshes[(size_t)i0];
      meshes[(size_t)i].moved = moved;
      meshes[(size_t)i].to    = moved ? f.Location().Transformation() * faces(i0).Location().Transformation().Inverted() : gp_Trsf();
    }
    // The first face's j: its nodes start at face_nodes[j].
    const size_t   j0    = (size_t)input_of[(size_t)i0];
    const uint32_t nbase = face_nodes[j0];
    size_t         at    = use_at[(size_t)i0];
    // Its uses' edges: the first face's read above, an instance's found in the map.
    ks.clear();
    if (i == i0)
      for (uint32_t u = in.loop_uses[in.face_loops[j0]]; u < in.loop_uses[in.face_loops[j0 + 1]]; ++u)
        ks.push_back((int)(in.uses[u] >> 1) + 1);
    else
      for (TopoDS_Iterator wi(f); wi.More(); wi.Next())
        for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
          ks.push_back(edges.FindIndex(ei.Value()));
    for (const int k : ks)
    {
      // (A face meshed in place before a moved instance: its nodes are the edge's own.)
      if (k > 0 && (from[(size_t)k - 1].face == 0 || (meshes[(size_t)from[(size_t)k - 1].face].moved && !moved)))
        from[(size_t)k - 1] = {i, at, nbase};
      if (moved && k > 0)
        instance_uses.emplace_back(i, k, at);
      at += 1 + use_nodes[at];
    }
  }
  // A moved instance's nodes on an edge another face's polyline gives: that face's points (moved as it is).
  for (const auto& [i, k, at] : instance_uses)
  {
    const PolyFrom& pf = from[(size_t)k - 1];
    if (pf.face == 0 || pf.face == i || use_nodes[pf.at] != use_nodes[at])
      continue;
    RustFace&       r   = meshes[(size_t)i];
    const RustFace& src = meshes[(size_t)pf.face];
    if (!r.pins)
      r.pins = std::make_shared<std::vector<std::pair<uint32_t, gp_XYZ>>>();
    for (uint32_t c = 0; c < use_nodes[at]; ++c)
    {
      const uint32_t node = pf.base + use_nodes[pf.at + 1 + c];
      gp_XYZ         p(nodes[3 * node], nodes[3 * node + 1], nodes[3 * node + 2]);
      if (src.moved)
        src.to.Transforms(p);
      r.pins->emplace_back(use_nodes[at + 1 + c], p);
    }
  }
  in_chunks(edges.Extent(), parallel, [&](int k) {
    const PolyFrom& pf = from[(size_t)k - 1];
    if (pf.face == 0)
      return;
    const RustFace& r     = meshes[(size_t)pf.face];
    const uint32_t  count = use_nodes[pf.at];
    auto&           poly  = polys[(size_t)k - 1];
    poly.reserve(count);
    for (uint32_t c = 0; c < count; ++c)
    {
      const uint32_t node = pf.base + use_nodes[pf.at + 1 + c];
      gp_Pnt         p(nodes[3 * node], nodes[3 * node + 1], nodes[3 * node + 2]);
      if (r.moved)
        p.Transform(r.to);
      poly.push_back({(float)p.X(), (float)p.Y(), (float)p.Z()});
    }
  });
  return n;
}

} // namespace noshape
