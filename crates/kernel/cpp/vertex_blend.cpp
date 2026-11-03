// Corner faces of a fillet that runs over smaller corner fillets, made the way
// Parasolid makes them (Onshape G24/G25). A loop of edges filleted with R
// whose corners are cylinders of radius Rc < R leaves, at each corner, a
// face between the two R cylinders, the Rc cylinder and the corner of the
// face the loop bounds. Parasolid's face there is a rolling-ball blend of
// radius Rc between the two R cylinders: the ball's centre runs along the
// intersection of the two cylinders offset inwards by R - Rc (an ellipse),
// each section is a circle of radius Rc normal to it, the face's sides are
// ellipses on the R cylinders and it closes in a point at the corner.
// (Onshape's corner face areas match to 1e-7; the section points of five
// cases to their 1e-4 mm accuracy.) OCCT fills the same corner with a patch
// that sags (3-24 mm³ off on those cases), so after OCCT's fillet the patch is
// replaced: same topology, the face and its two side edges get the exact
// geometry (a rational B-spline: cubic along the spine, the circle arcs as
// exact quadratic sections, like Parasolid's export of the face). Also the
// loop's fillet round a cylinder at an angle to its face (a drafted corner):
// `Rolling`.

#include "internal.hpp"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepLib.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Precision.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <ShapeFix_Edge.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Lin.hxx>

#include <algorithm>
#include <cmath>

namespace noshape {

namespace {

struct Cyl
{
  gp_Ax1      axis;
  double      radius = 0;
  TopoDS_Face face;
};

bool cylinder_of(const TopoDS_Face& f, Cyl& c)
{
  BRepAdaptor_Surface a(f, false);
  if (a.GetType() != GeomAbs_Cylinder)
    return false;
  c.axis   = a.Cylinder().Axis();
  c.radius = a.Cylinder().Radius();
  c.face   = f;
  return true;
}

// The corner: the R cylinders' axes meet at q; the Rc cylinder's axis is
// l0 from q along b (in the axes' plane), n is normal to that plane towards
// the corner's point.
struct Corner
{
  gp_Pnt q;
  gp_Vec b, n;
  double l0 = 0, R = 0, Rc = 0;
  // Where the spine starts: 0 when the Rc cylinder is square to the R axes, else where its slanted axis meets the
  // spine (a drafted block's corners, live 1831).
  double th0 = 0;
  Cyl    c1, c2;

  gp_Pnt spine(double th) const { return q.Translated(b * (l0 * std::cos(th)) + n * ((R - Rc) * std::sin(th))); }
  // Unit normal of cylinder c at the ball centre s (from its axis).
  static gp_Vec contact(const Cyl& c, const gp_Pnt& s)
  {
    const gp_Vec d(c.axis.Direction()), as(c.axis.Location(), s);
    return (as - d * as.Dot(d)).Normalized();
  }
};

// The section at spine angle th: its end points on the two cylinders, its
// middle (arc midpoint), the middle control point and the weight (cos of
// half the arc).
struct Section
{
  gp_Pnt a, c, mid, m;
  double w = 1;
};

// The ball's section about centre s, radius r, from its contact along u1 to its contact along u2.
Section section_between(const gp_Pnt& s, const gp_Vec& u1, const gp_Vec& u2, double r)
{
  Section x;
  x.a              = s.Translated(u1 * r);
  x.c              = s.Translated(u2 * r);
  x.w              = std::sqrt(std::max(0.0, (1 + std::max(-1.0, std::min(1.0, u1.Dot(u2)))) / 2));
  const gp_Vec bis = u1 + u2;
  if (bis.Magnitude() < 1e-12 || x.w < 1e-9)
  {
    x.mid = x.m = x.a;
    return x;
  }
  x.mid = s.Translated(bis.Normalized() * r);
  x.m   = s.Translated(bis.Normalized() * (r / x.w));
  return x;
}

Section section_at(const Corner& k, double th)
{
  const gp_Pnt s = k.spine(th);
  return section_between(s, Corner::contact(k.c1, s), Corner::contact(k.c2, s), k.Rc);
}

// The face as a rational B-spline over [t0, t1] (along the spine) x [0, 1] (the section's arc, from its first contact
// to its second) through n + 1 exact sections; null if it strays more than `tol` from the exact sections between them.
template <class At> occ::handle<Geom_BSplineSurface> blend_surface(const At& section, double t0, double t1, int n, double tol)
{
  occ::handle<NCollection_HArray1<gp_Pnt>> p0 = new NCollection_HArray1<gp_Pnt>(1, n + 1), p1 = new NCollection_HArray1<gp_Pnt>(1, n + 1),
                                           p2 = new NCollection_HArray1<gp_Pnt>(1, n + 1), pw = new NCollection_HArray1<gp_Pnt>(1, n + 1);
  occ::handle<NCollection_HArray1<double>> t = new NCollection_HArray1<double>(1, n + 1);
  for (int i = 0; i <= n; ++i)
  {
    const double  th = t0 + (t1 - t0) * i / n;
    const Section x  = section(th);
    t->SetValue(i + 1, th);
    p0->SetValue(i + 1, x.a);
    p1->SetValue(i + 1, gp_Pnt(x.m.XYZ() * x.w)); // homogeneous
    p2->SetValue(i + 1, x.c);
    pw->SetValue(i + 1, gp_Pnt(x.w, 0, 0));
  }
  auto interpolate = [&t](const occ::handle<NCollection_HArray1<gp_Pnt>>& p) {
    GeomAPI_Interpolate in(p, t, false, 1e-15);
    in.Perform();
    return in.IsDone() ? in.Curve() : occ::handle<Geom_BSplineCurve>();
  };
  const auto c0 = interpolate(p0), c1 = interpolate(p1), c2 = interpolate(p2), cw = interpolate(pw);
  if (c0.IsNull() || c1.IsNull() || c2.IsNull() || cw.IsNull() || c0->NbPoles() != cw->NbPoles() || c1->NbPoles() != cw->NbPoles() ||
      c2->NbPoles() != cw->NbPoles())
    return {};
  const int                  nu = c0->NbPoles();
  NCollection_Array2<gp_Pnt> poles(1, nu, 1, 3);
  NCollection_Array2<double> weights(1, nu, 1, 3);
  for (int i = 1; i <= nu; ++i)
  {
    const double w = cw->Pole(i).X();
    if (!(w > 1e-6))
      return {};
    poles(i, 1)   = c0->Pole(i);
    poles(i, 2)   = gp_Pnt(c1->Pole(i).XYZ() / w);
    poles(i, 3)   = c2->Pole(i);
    weights(i, 1) = weights(i, 3) = 1;
    weights(i, 2) = w;
  }
  NCollection_Array1<double> vk(1, 2);
  NCollection_Array1<int>    vm(1, 2);
  vk(1) = 0, vk(2) = 1, vm(1) = vm(2) = 3;
  occ::handle<Geom_BSplineSurface> surf = new Geom_BSplineSurface(poles, weights, c0->Knots(), vk, c0->Multiplicities(), vm, 3, 2);
  for (int i = 0; i < n; ++i)
  {
    const double  th = t0 + (t1 - t0) * (i + 0.5) / n;
    const Section x  = section(th);
    if (surf->Value(th, 0).Distance(x.a) > tol || surf->Value(th, 1).Distance(x.c) > tol || surf->Value(th, 0.5).Distance(x.mid) > tol)
      return {};
  }
  return surf;
}

// F's corner, if F is such a corner face: three edges, on two cylinders of one
// radius R whose axes meet and on a cylinder of radius Rc < R normal to
// their plane, with the vertices where the rolling ball puts them.
bool corner_of(const TopoDS_Face& F, const ShapeAncMap& e2f, double tol, Corner& k)
{
  {
    BRepAdaptor_Surface a(F, false);
    if (a.GetType() == GeomAbs_Plane || a.GetType() == GeomAbs_Cylinder || a.GetType() == GeomAbs_Cone || a.GetType() == GeomAbs_Sphere ||
        a.GetType() == GeomAbs_Torus)
      return false;
  }
  std::vector<Cyl> cyls;
  int              edges = 0;
  for (TopExp_Explorer ee(F, TopAbs_EDGE); ee.More(); ee.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(ee.Current());
    if (BRep_Tool::Degenerated(e))
      continue;
    ++edges;
    for (const TopoDS_Shape& g : e2f.FindFromKey(e))
    {
      Cyl c;
      if (!g.IsSame(F) && cylinder_of(TopoDS::Face(g), c))
        cyls.push_back(c);
    }
  }
  if (edges != 3 || cyls.size() != 3)
    return false;
  // The two of equal radius, and the smaller one.
  int small = -1;
  for (int i = 0; i < 3; ++i)
  {
    const Cyl &x = cyls[(i + 1) % 3], &y = cyls[(i + 2) % 3];
    if (std::abs(x.radius - y.radius) < tol && cyls[i].radius < x.radius - tol)
      small = i;
  }
  if (small < 0)
    return false;
  k.c1 = cyls[(small + 1) % 3];
  k.c2 = cyls[(small + 2) % 3];
  k.R  = k.c1.radius;
  k.Rc = cyls[small].radius;
  const gp_Vec d1(k.c1.axis.Direction()), d2(k.c2.axis.Direction());
  gp_Vec       n = d1.Crossed(d2);
  if (n.Magnitude() < 1e-6)
    return false;
  n.Normalize();
  // Where the axes meet.
  const gp_Vec w0(k.c2.axis.Location(), k.c1.axis.Location());
  const double bb = d1.Dot(d2), dd = d1.Dot(w0), ee = d2.Dot(w0), den = 1 - bb * bb;
  k.q             = k.c1.axis.Location().Translated(d1 * ((bb * ee - dd) / den));
  if (gp_Lin(k.c2.axis).Distance(k.q) > tol)
    return false;
  const Cyl& cc = cyls[small];
  // b: from q towards the Rc axis, in the axes' plane; the spine's half-width there l0 (R - Rc from both R axes).
  const gp_Vec dl(cc.axis.Direction());
  gp_Vec       ql(k.q, cc.axis.Location());
  ql -= dl * ql.Dot(dl);
  const gp_Vec inplane = ql - n * ql.Dot(n);
  if (inplane.Magnitude() < tol)
    return false;
  k.b               = inplane.Normalized();
  const double sin1 = k.b.Crossed(d1).Magnitude(), sin2 = k.b.Crossed(d2).Magnitude();
  if (sin1 < 1e-6 || std::abs(sin1 - sin2) > 1e-7)
    return false;
  k.l0 = (k.R - k.Rc) / sin1;
  // The Rc axis lies in the plane of b and n (the bisector); where it meets the spine (u / l0)² + (v / (R - Rc))² = 1.
  const gp_Vec side = k.b.Crossed(n);
  const gp_Vec qp(k.q, cc.axis.Location());
  if (std::abs(dl.Dot(side)) > 1e-7 || std::abs(qp.Dot(side)) > tol)
    return false;
  // (In units of R - Rc.)
  const double h = k.R - k.Rc, L = k.l0 / h, u0 = qp.Dot(k.b) / h, v0 = qp.Dot(n) / h, du = dl.Dot(k.b), dv = dl.Dot(n);
  const double qa = du * du / (L * L) + dv * dv, qb = 2 * (u0 * du / (L * L) + v0 * dv), qc = u0 * u0 / (L * L) + v0 * v0 - 1,
               disc = qb * qb - 4 * qa * qc;
  // (Square to the axes the line touches the spine at 0: a double root, rounding may make it slightly negative.)
  if (disc < -1e-4 || qa < 1e-12)
    return false;
  std::vector<double> starts;
  for (double sgn : {-1.0, 1.0})
  {
    const double t = (-qb + sgn * std::sqrt(std::max(0.0, disc))) / (2 * qa);
    starts.push_back(std::atan2(v0 + t * dv, (u0 + t * du) / L));
  }
  // n towards the face's point; F's vertices where the ball puts them.
  std::vector<gp_Pnt> vs;
  for (TopExp_Explorer ev(F, TopAbs_VERTEX); ev.More(); ev.Next())
  {
    const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(ev.Current()));
    bool         dup = false;
    for (const gp_Pnt& v : vs)
      dup = dup || v.Distance(p) < tol;
    if (!dup)
      vs.push_back(p);
  }
  if (vs.size() != 3)
    return false;
  for (int sign : {1, -1})
    for (double th : starts)
    {
      // (The other side of the plane: the spine angle mirrored.)
      k.n             = n * sign;
      k.th0           = sign * th;
      const Section b = section_at(k, k.th0), t = section_at(k, M_PI / 2);
      int           found = 0;
      for (const gp_Pnt& want : {b.a, b.c, t.a})
        for (const gp_Pnt& v : vs)
          found += v.Distance(want) < tol * 10;
      if (found == 3 && k.th0 < M_PI / 2 - 1e-6 && k.th0 > -M_PI / 4)
        return true;
    }
  return false;
}

// Endpoints of an edge, first to last.
std::pair<gp_Pnt, gp_Pnt> ends(const TopoDS_Edge& e)
{
  return {BRep_Tool::Pnt(TopExp::FirstVertex(e)), BRep_Tool::Pnt(TopExp::LastVertex(e))};
}

// A fillet R along a planar face P's loop where the loop runs round a cylinder S at an angle to P (a drafted block's
// slanted corner round under its top loop, live 1686 (c)): Parasolid's face there is the R ball rolling on P and S,
// its centre on the ellipse where their offsets meet; four-sided: the end arcs of the loop's cylinders on either side,
// an ellipse on P, a curve on S. OCCT leaves two patches meeting in a degenerated edge whose vertex lies off P (0.9 mm
// on 1686 (c)); they become that one face.
struct Rolling
{
  gp_Pnt o;     // on S's axis
  gp_Vec x, y;  // square to the axis, x towards the first end's centre
  gp_Vec d, np; // S's axis, P's outward normal
  double rs = 0, R = 0, D = 0, sweep = 0;
  double sp = 1, ss = 1; // +1: the ball inside the part (a convex edge), -1 in the air; S's normal from its axis or to it

  gp_Pnt centre(double phi) const
  {
    const gp_Vec radial = x * std::cos(phi) + y * std::sin(phi);
    const gp_Pnt q      = o.Translated(radial * rs);
    return q.Translated(d * ((D - np.Dot(gp_Vec(q.XYZ()))) / np.Dot(d)));
  }
  // At t in [0, 1] along the spine: from the contact on P to the contact on S.
  Section section(double t) const
  {
    const double phi    = sweep * t;
    const gp_Vec radial = x * std::cos(phi) + y * std::sin(phi);
    return section_between(centre(phi), np * sp, radial * (sp * ss), R);
  }
};

// A face's outward normal at the middle of its parameter range.
gp_Vec outward(const TopoDS_Face& f)
{
  BRepAdaptor_Surface a(f);
  gp_Pnt              p;
  gp_Vec              du, dv;
  a.D1(0.5 * (a.FirstUParameter() + a.LastUParameter()), 0.5 * (a.FirstVParameter() + a.LastVParameter()), p, du, dv);
  gp_Vec n = du.Crossed(dv);
  if (n.Magnitude() < 1e-300)
    return n;
  n.Normalize();
  return f.Orientation() == TopAbs_REVERSED ? -n : n;
}

// The two patches about the degenerated edge `dg` and what bounds them: f1's and f2's edges with the end cylinder
// beside it (c), with P (p) and with S (s).
struct RollPair
{
  Rolling     k;
  TopoDS_Face f1, f2, p, s, c1, c2;
  TopoDS_Edge e1c, e2c, e1p, e2p, e1s, e2s;
  TopoDS_Vertex mid;
};

bool rolling_pair(const TopoDS_Edge& dg, const ShapeAncMap& e2f, double tol, RollPair& r)
{
  auto no = [](const char* why) {
    if (std::getenv("NOSHAPE_VB_DEBUG"))
      std::fprintf(stderr, "rolling corner: %s\n", why);
    return false;
  };
  std::vector<TopoDS_Face> fs;
  for (const TopoDS_Shape& g : e2f.FindFromKey(dg))
    if (std::none_of(fs.begin(), fs.end(), [&](const TopoDS_Face& f) { return f.IsSame(g); }))
      fs.push_back(TopoDS::Face(g));
  if (fs.size() != 2)
    return no("not two faces");
  r.f1  = fs[0];
  r.f2  = fs[1];
  r.mid = TopExp::FirstVertex(dg);
  auto elementary = [](const TopoDS_Face& f) {
    const GeomAbs_SurfaceType t = BRepAdaptor_Surface(f, false).GetType();
    return t == GeomAbs_Plane || t == GeomAbs_Cylinder || t == GeomAbs_Cone || t == GeomAbs_Sphere || t == GeomAbs_Torus;
  };
  if (elementary(r.f1) || elementary(r.f2))
    return no("an elementary face");
  // Each patch's edges and the faces across them.
  auto across = [&](const TopoDS_Face& f) {
    std::vector<std::pair<TopoDS_Edge, TopoDS_Face>> out;
    for (TopExp_Explorer ee(f, TopAbs_EDGE); ee.More(); ee.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(ee.Current());
      if (BRep_Tool::Degenerated(e) || std::any_of(out.begin(), out.end(), [&](const auto& x) { return x.first.IsSame(e); }))
        continue;
      TopoDS_Face nb;
      for (const TopoDS_Shape& g : e2f.FindFromKey(e))
        if (!g.IsSame(f))
          nb = TopoDS::Face(g);
      out.emplace_back(e, nb);
    }
    return out;
  };
  const auto a1 = across(r.f1), a2 = across(r.f2);
  if (a1.size() != 3 || a2.size() != 3)
    return no("not three edges each");
  auto in = [](const std::vector<std::pair<TopoDS_Edge, TopoDS_Face>>& a, const TopoDS_Face& f) {
    return std::any_of(a.begin(), a.end(), [&](const auto& x) { return !x.second.IsNull() && x.second.IsSame(f); });
  };
  auto sort = [&](const std::vector<std::pair<TopoDS_Edge, TopoDS_Face>>& a, const std::vector<std::pair<TopoDS_Edge, TopoDS_Face>>& other,
                  TopoDS_Edge& ec, TopoDS_Edge& ep, TopoDS_Edge& es, TopoDS_Face& c) {
    for (const auto& [e, nb] : a)
    {
      if (nb.IsNull())
        return false;
      const GeomAbs_SurfaceType t = BRepAdaptor_Surface(nb, false).GetType();
      if (t == GeomAbs_Plane && in(other, nb))
        ep = e, r.p = nb;
      else if (t == GeomAbs_Cylinder && in(other, nb))
        es = e, r.s = nb;
      else if (t == GeomAbs_Cylinder)
        ec = e, c = nb;
    }
    return !ec.IsNull() && !ep.IsNull() && !es.IsNull();
  };
  if (!sort(a1, a2, r.e1c, r.e1p, r.e1s, r.c1) || !sort(a2, a1, r.e2c, r.e2p, r.e2s, r.c2))
    return no("not a plane, a shared cylinder and an end cylinder");
  Cyl c1, c2, s;
  if (!cylinder_of(r.c1, c1) || !cylinder_of(r.c2, c2) || !cylinder_of(r.s, s) || std::abs(c1.radius - c2.radius) > tol)
    return no("end radii");
  Rolling& k = r.k;
  k.R        = c1.radius;
  k.np       = outward(r.p);
  if (k.np.Magnitude() < 0.5)
    return false;
  const double p0 = k.np.Dot(gp_Vec(BRepAdaptor_Surface(r.p, false).Plane().Location().XYZ()));
  k.d             = gp_Vec(s.axis.Direction());
  k.o = s.axis.Location();
  if (std::abs(k.np.Dot(k.d)) < 1e-3)
    return no("S along P");
  auto radial = [&](const gp_Pnt& q) {
    const gp_Vec oq(k.o, q);
    return oq - k.d * oq.Dot(k.d);
  };
  {
    BRepAdaptor_Surface sa(r.s);
    const gp_Pnt        q = sa.Value(0.5 * (sa.FirstUParameter() + sa.LastUParameter()), 0.5 * (sa.FirstVParameter() + sa.LastVParameter()));
    k.ss                  = outward(r.s).Dot(radial(q)) > 0 ? 1 : -1;
  }
  // The ball's centres at the ends: on the end cylinders' axes, under their end arcs.
  auto centre_of = [](const TopoDS_Edge& arc, const Cyl& c) {
    BRepAdaptor_Curve ac(arc);
    const gp_Pnt      m = ac.Value(0.5 * (ac.FirstParameter() + ac.LastParameter()));
    const gp_Vec      d(c.axis.Direction()), am(c.axis.Location(), m);
    return c.axis.Location().Translated(d * am.Dot(d));
  };
  const gp_Pnt q0 = centre_of(r.e1c, c1), q1 = centre_of(r.e2c, c2);
  k.sp            = k.np.Dot(gp_Vec(q0.XYZ())) < p0 ? 1 : -1;
  k.D             = p0 - k.sp * k.R;
  k.rs            = s.radius - k.sp * k.ss * k.R;
  if (!(k.rs > 0))
    return no("S's offset");
  const gp_Vec r0 = radial(q0), r1 = radial(q1);
  if (std::abs(r0.Magnitude() - k.rs) > tol || std::abs(r1.Magnitude() - k.rs) > tol)
  {
    if (std::getenv("NOSHAPE_VB_DEBUG"))
      std::fprintf(stderr, "rolling corner: ends %.9g %.9g, spine radius %.9g\n", r0.Magnitude(), r1.Magnitude(), k.rs);
    return no("the ends off the spine's cylinder");
  }
  // The exact ends: where the wall beside each end cylinder is tangent to S, the radial square to S's axis and to
  // that cylinder's (OCCT's end arcs are fitted).
  auto tangent = [&](const Cyl& c, const gp_Vec& like) {
    gp_Vec n = k.d.Crossed(gp_Vec(c.axis.Direction()));
    if (n.Magnitude() < 1e-9)
      return like.Normalized();
    n.Normalize();
    return n.Dot(like) < 0 ? -n : n;
  };
  const gp_Vec n0 = tangent(c1, r0), n1 = tangent(c2, r1);
  k.x             = n0;
  k.y             = k.d.Crossed(k.x);
  k.sweep         = std::atan2(n1.Dot(k.y), n1.Dot(k.x));
  if (std::getenv("NOSHAPE_VB_DEBUG"))
    std::fprintf(stderr, "rolling corner: OCCT's ends %.3g / %.3g off (tol %.3g)\n", k.centre(0).Distance(q0), k.centre(k.sweep).Distance(q1), tol);
  if (std::abs(k.sweep) < 1e-6 || k.centre(0).Distance(q0) > tol || k.centre(k.sweep).Distance(q1) > tol)
  {
    if (std::getenv("NOSHAPE_VB_DEBUG"))
      std::fprintf(stderr, "rolling corner: sweep %.9g, ends %.3g %.3g off\n", k.sweep, k.centre(0).Distance(q0), k.centre(k.sweep).Distance(q1));
    return no("the ends off the spine");
  }
  // The patches' own corners where the ball puts them.
  const Section b = k.section(0), e = k.section(1);
  auto          at = [&](const TopoDS_Edge& x, const gp_Pnt& p, const gp_Pnt& q) {
    const auto [u, v] = ends(x);
    return (u.Distance(p) < tol && v.Distance(q) < tol) || (u.Distance(q) < tol && v.Distance(p) < tol);
  };
  return at(r.e1c, b.a, b.c) && at(r.e2c, e.a, e.c) ? true : no("the end arcs' ends");
}

} // namespace

TopoDS_Shape exact_vertex_blends(const TopoDS_Shape& s, occ::handle<BRepTools_History>& history)
{
  history.Nullify();
  ShapeAncMap e2f;
  TopExp::MapShapesAndAncestors(s, TopAbs_EDGE, TopAbs_FACE, e2f);
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  BRep_Builder                   bb;
  int                            replaced = 0;
  for (TopExp_Explorer ef(s, TopAbs_FACE); ef.More(); ef.Next())
  {
    const TopoDS_Face& F = TopoDS::Face(ef.Current());
    Corner             k;
    try
    {
      // Tolerances relative to the largest adjacent cylinder.
      double R = 0;
      for (TopExp_Explorer ee(F, TopAbs_EDGE); ee.More() && R == 0; ee.Next())
        if (e2f.Contains(ee.Current()))
          for (const TopoDS_Shape& g : e2f.FindFromKey(ee.Current()))
          {
            Cyl c;
            if (!g.IsSame(F) && cylinder_of(TopoDS::Face(g), c))
              R = std::max(R, c.radius);
          }
      if (R <= 0)
        continue;
      // Recognising the corner: OCCT's geometry is good to its tolerances
      // (absolute, so coarse on parts in metres); the new geometry to 1e-7 R.
      double vtol = 0;
      for (TopExp_Explorer ev(F, TopAbs_VERTEX); ev.More(); ev.Next())
        vtol = std::max(vtol, BRep_Tool::Tolerance(TopoDS::Vertex(ev.Current())));
      const double dtol = std::max(R * 1e-5, vtol);
      const double tol  = R * 1e-7;
      if (!corner_of(F, e2f, dtol, k))
        continue;
      occ::handle<Geom_BSplineSurface> surf;
      for (int n = 64; n <= 256 && surf.IsNull(); n *= 2)
        surf = blend_surface([&k](double th) { return section_at(k, th); }, k.th0, M_PI / 2, n, tol * 0.25);
      if (surf.IsNull())
        continue;
      const double            etol = std::max(tol, Precision::Confusion()); // edge / vertex tolerances
      BRepBuilderAPI_MakeFace mf(surf, k.th0, M_PI / 2, 0, 1, etol);
      if (!mf.IsDone())
        continue;
      TopoDS_Face nf = mf.Face();
      {
        // Outward: away from the ball's centre.
        BRepAdaptor_Surface an(nf, false);
        gp_Pnt              p;
        gp_Vec              du, dv;
        const double mid = 0.5 * (k.th0 + M_PI / 2);
        an.D1(mid, 0.5, p, du, dv);
        gp_Vec nn = du.Crossed(dv);
        if (nf.Orientation() == TopAbs_REVERSED)
          nn.Reverse();
        if (nn.Dot(gp_Vec(k.spine(mid), p)) < 0)
          nf.Reverse();
      }
      // Each new edge replaces the old edge with the same ends; it gets a
      // pcurve on the old edge's other face. Recorded only if the whole
      // corner works.
      std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> subs;
      auto recorded = [&](const TopoDS_Shape& x) {
        if (rs->IsRecorded(x))
          return true;
        // A new vertex keeps at least the old one's tolerance: the untouched
      // edges at it end within that.
      // Vertices are replaced as FORWARD shapes (TopExp::LastVertex gives a
      // REVERSED one; ReShape composes orientations).
      for (auto& [o, n] : subs)
        if (o.ShapeType() == TopAbs_VERTEX)
          o.Orientation(TopAbs_FORWARD), n.Orientation(TopAbs_FORWARD);
      for (const auto& [o, n] : subs)
        if (o.ShapeType() == TopAbs_VERTEX)
          bb.UpdateVertex(TopoDS::Vertex(n), std::max(BRep_Tool::Tolerance(TopoDS::Vertex(n)), BRep_Tool::Tolerance(TopoDS::Vertex(o))));
      for (const auto& [o, n] : subs)
          if (o.IsSame(x))
            return true;
        return false;
      };
      bool ok = true;
      for (TopExp_Explorer ne(nf, TopAbs_EDGE); ne.More() && ok; ne.Next())
      {
        TopoDS_Edge e = TopoDS::Edge(ne.Current());
        if (BRep_Tool::Degenerated(e))
        {
          const TopoDS_Vertex nv = TopExp::FirstVertex(e);
          for (TopExp_Explorer ov(F, TopAbs_VERTEX); ov.More(); ov.Next())
            if (BRep_Tool::Pnt(TopoDS::Vertex(ov.Current())).Distance(BRep_Tool::Pnt(nv)) < dtol * 10 && !recorded(ov.Current()))
              subs.emplace_back(ov.Current(), nv);
          continue;
        }
        e.Orientation(TopAbs_FORWARD);
        const auto  ne_ends = ends(e);
        TopoDS_Edge old;
        bool        same = true;
        for (TopExp_Explorer oe(F, TopAbs_EDGE); oe.More(); oe.Next())
        {
          const TopoDS_Edge& o = TopoDS::Edge(oe.Current());
          if (BRep_Tool::Degenerated(o))
            continue;
          const auto oe_ends = ends(o);
          if (oe_ends.first.Distance(ne_ends.first) < dtol * 10 && oe_ends.second.Distance(ne_ends.second) < dtol * 10)
            old = o, same = true;
          else if (oe_ends.first.Distance(ne_ends.second) < dtol * 10 && oe_ends.second.Distance(ne_ends.first) < dtol * 10)
            old = o, same = false;
        }
        if (old.IsNull())
        {
          ok = false;
          break;
        }
        TopoDS_Face nb;
        for (const TopoDS_Shape& g : e2f.FindFromKey(old))
          if (!g.IsSame(F))
            nb = TopoDS::Face(g);
        if (nb.IsNull())
        {
          ok = false;
          break;
        }
        ShapeFix_Edge sfe;
        sfe.FixAddPCurve(e, nb, false, etol);
        double f, l;
        if (BRep_Tool::CurveOnSurface(e, nb, f, l).IsNull())
        {
          ok = false;
          break;
        }
        bb.SameParameter(e, false);
        BRepLib::SameParameter(e, etol);
        old.Orientation(TopAbs_FORWARD);
        subs.emplace_back(old, same ? e : TopoDS::Edge(e.Reversed()));
        for (int end = 0; end < 2; ++end)
        {
          const TopoDS_Vertex ov = end ? TopExp::LastVertex(old) : TopExp::FirstVertex(old);
          const TopoDS_Vertex nv = (end == 1) == same ? TopExp::LastVertex(e) : TopExp::FirstVertex(e);
          if (!ov.IsSame(nv) && !recorded(ov))
            subs.emplace_back(ov, nv);
        }
      }
      if (!ok)
        continue;
      for (const auto& [o, n] : subs)
        rs->Replace(o, n);
      rs->Replace(F, nf);
      ++replaced;
    }
    catch (const Standard_Failure&)
    {
    }
  }
  for (TopExp_Explorer ed(s, TopAbs_EDGE); ed.More(); ed.Next())
  {
    const TopoDS_Edge& dg = TopoDS::Edge(ed.Current());
    if (!BRep_Tool::Degenerated(dg) || !e2f.Contains(dg) || rs->IsRecorded(dg))
      continue;
    try
    {
      RollPair pr;
      // OCCT's end arcs are good to their vertices' tolerances (not the degenerated edge's: that is what's wrong).
      double R = 0, vtol = 0;
      {
        RollPair probe;
        if (!rolling_pair(dg, e2f, 1e300, probe))
          continue;
        R = probe.k.R;
        for (const TopoDS_Edge& e : {probe.e1c, probe.e2c})
          for (TopExp_Explorer ev(e, TopAbs_VERTEX); ev.More(); ev.Next())
            vtol = std::max(vtol, BRep_Tool::Tolerance(TopoDS::Vertex(ev.Current())));
      }
      const double dtol = std::max(R * 1e-5, vtol), tol = R * 1e-7;
      if (!rolling_pair(dg, e2f, dtol, pr) || rs->IsRecorded(pr.f1) || rs->IsRecorded(pr.f2))
        continue;
      if (std::getenv("NOSHAPE_VB_DEBUG"))
        std::fprintf(stderr, "rolling corner: R %g, sweep %g\n", pr.k.R, pr.k.sweep);
      const Rolling&                   k = pr.k;
      occ::handle<Geom_BSplineSurface> surf;
      for (int n = 16; n <= 256 && surf.IsNull(); n *= 2)
        surf = blend_surface([&k](double t) { return k.section(t); }, 0, 1, n, tol * 0.25);
      if (surf.IsNull())
        continue;
      const double            etol = std::max(tol, Precision::Confusion());
      BRepBuilderAPI_MakeFace mf(surf, 0, 1, 0, 1, etol);
      if (!mf.IsDone())
        continue;
      TopoDS_Face nf = mf.Face();
      {
        BRepAdaptor_Surface an(nf, false);
        gp_Pnt              p;
        gp_Vec              du, dv;
        an.D1(0.5, 0.5, p, du, dv);
        gp_Vec nn = du.Crossed(dv);
        if (nf.Orientation() == TopAbs_REVERSED)
          nn.Reverse();
        // Outward: away from the ball's centre, or towards it where the ball is in the air.
        if (k.sp * nn.Dot(gp_Vec(k.centre(k.sweep / 2), p)) < 0)
          nf.Reverse();
      }
      // The end arcs replace OCCT's; the edge on P replaces its two there (and the edge on S S's two), the second
      // of each pair and the degenerated edge going with the patches.
      auto outer = [&](const TopoDS_Edge& e, int& end) {
        const TopoDS_Vertex a = TopExp::FirstVertex(e), b = TopExp::LastVertex(e);
        end                   = a.IsSame(pr.mid) ? 1 : 0;
        return end ? b : a;
      };
      std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> subs;
      std::vector<TopoDS_Shape>                          gone{pr.f2};
      bool                                               ok = true;
      for (TopExp_Explorer ne(nf, TopAbs_EDGE); ne.More() && ok; ne.Next())
      {
        TopoDS_Edge e = TopoDS::Edge(ne.Current());
        e.Orientation(TopAbs_FORWARD);
        const auto [na, nb] = ends(e);
        auto       at_point     = [&](const gp_Pnt& p, const gp_Pnt& q) { return p.Distance(q) < dtol * 10; };
        TopoDS_Edge old, second;
        TopoDS_Face on;
        bool        same = true;
        for (const auto& [o, f] : {std::pair{pr.e1c, pr.c1}, std::pair{pr.e2c, pr.c2}})
        {
          TopoDS_Edge of = TopoDS::Edge(o.Oriented(TopAbs_FORWARD));
          const auto [oa, ob] = ends(of);
          if ((at_point(oa, na) && at_point(ob, nb)) || (at_point(oa, nb) && at_point(ob, na)))
            old = of, on = f, same = at_point(oa, na);
        }
        int end1 = 0, end2 = 0;
        for (const auto& [o1, o2, f] : {std::tuple{pr.e1p, pr.e2p, pr.p}, std::tuple{pr.e1s, pr.e2s, pr.s}})
        {
          if (!old.IsNull())
            break;
          TopoDS_Edge   of = TopoDS::Edge(o1.Oriented(TopAbs_FORWARD)), os = TopoDS::Edge(o2.Oriented(TopAbs_FORWARD));
          const gp_Pnt  p1 = BRep_Tool::Pnt(outer(of, end1)), p2 = BRep_Tool::Pnt(outer(os, end2));
          if ((at_point(p1, na) && at_point(p2, nb)) || (at_point(p1, nb) && at_point(p2, na)))
          {
            old = of, second = os, on = f;
            // The new edge runs from old's outer vertex the way old does.
            same = end1 == 0 ? at_point(p1, na) : at_point(p1, nb);
          }
        }
        if (old.IsNull() || on.IsNull())
        {
          ok = false;
          break;
        }
        ShapeFix_Edge sfe;
        sfe.FixAddPCurve(e, on, false, etol);
        double f0, f1;
        if (BRep_Tool::CurveOnSurface(e, on, f0, f1).IsNull())
        {
          ok = false;
          break;
        }
        bb.SameParameter(e, false);
        BRepLib::SameParameter(e, etol);
        const TopoDS_Edge en = same ? e : TopoDS::Edge(e.Reversed());
        subs.emplace_back(old, en);
        // Each old outer vertex -> the new one at its place.
        std::vector<TopoDS_Vertex> olds;
        if (second.IsNull())
          olds = {TopExp::FirstVertex(old), TopExp::LastVertex(old)};
        else
        {
          olds = {outer(old, end1), outer(second, end2)};
          gone.push_back(second);
        }
        for (const TopoDS_Vertex& ov : olds)
        {
          const gp_Pnt  op = BRep_Tool::Pnt(ov);
          TopoDS_Vertex nv = at_point(op, na) ? TopExp::FirstVertex(e) : TopExp::LastVertex(e);
          if (ov.IsSame(nv) || std::any_of(subs.begin(), subs.end(), [&](const auto& x) { return x.first.IsSame(ov); }))
            continue;
          subs.emplace_back(ov, nv);
        }
      }
      if (!ok)
        continue;
      for (auto& [o, n] : subs)
        if (o.ShapeType() == TopAbs_VERTEX)
        {
          o.Orientation(TopAbs_FORWARD), n.Orientation(TopAbs_FORWARD);
          bb.UpdateVertex(TopoDS::Vertex(n), std::max(BRep_Tool::Tolerance(TopoDS::Vertex(n)), BRep_Tool::Tolerance(TopoDS::Vertex(o))));
        }
      for (const auto& [o, n] : subs)
        rs->Replace(o, n);
      for (const TopoDS_Shape& g : gone)
        rs->Remove(g);
      rs->Replace(pr.f1, nf);
      ++replaced;
    }
    catch (const Standard_Failure&)
    {
    }
  }
  if (replaced == 0)
    return s;
  try
  {
    const TopoDS_Shape out = rs->Apply(s);
    if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
      return s;
    history = rs->History();
    return out;
  }
  catch (const Standard_Failure&)
  {
    return s;
  }
}

} // namespace noshape
