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
// exact quadratic sections, like Parasolid's export of the face).

#include "internal.hpp"

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

Section section_at(const Corner& k, double th)
{
  Section      r;
  const gp_Pnt s  = k.spine(th);
  const gp_Vec u1 = Corner::contact(k.c1, s), u2 = Corner::contact(k.c2, s);
  r.a             = s.Translated(u1 * k.Rc);
  r.c             = s.Translated(u2 * k.Rc);
  r.w             = std::sqrt(std::max(0.0, (1 + std::max(-1.0, std::min(1.0, u1.Dot(u2)))) / 2));
  const gp_Vec bis = u1 + u2;
  if (bis.Magnitude() < 1e-12 || r.w < 1e-9)
  {
    r.mid = r.m = r.a;
    return r;
  }
  r.mid = s.Translated(bis.Normalized() * k.Rc);
  r.m   = s.Translated(bis.Normalized() * (k.Rc / r.w));
  return r;
}

// The face as a rational B-spline over [0, π/2] (spine angle) x [0, 1] (arc
// from cylinder 1 to cylinder 2) through n + 1 exact sections; null if it
// strays more than `tol` from the exact sections between them.
occ::handle<Geom_BSplineSurface> blend_surface(const Corner& k, int n, double tol)
{
  occ::handle<NCollection_HArray1<gp_Pnt>> p0 = new NCollection_HArray1<gp_Pnt>(1, n + 1), p1 = new NCollection_HArray1<gp_Pnt>(1, n + 1),
                                           p2 = new NCollection_HArray1<gp_Pnt>(1, n + 1), pw = new NCollection_HArray1<gp_Pnt>(1, n + 1);
  occ::handle<NCollection_HArray1<double>> t = new NCollection_HArray1<double>(1, n + 1);
  for (int i = 0; i <= n; ++i)
  {
    const double  th = M_PI / 2 * i / n;
    const Section x  = section_at(k, th);
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
    const double  th = M_PI / 2 * (i + 0.5) / n;
    const Section x  = section_at(k, th);
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
  if (gp_Vec(cc.axis.Direction()).Crossed(n).Magnitude() > 1e-7)
    return false;
  const gp_Vec qa(k.q, cc.axis.Location());
  const gp_Vec inplane = qa - n * qa.Dot(n);
  k.l0                 = inplane.Magnitude();
  if (k.l0 < tol)
    return false;
  k.b = inplane / k.l0;
  // The corner axis is R - Rc from both R axes.
  const gp_Pnt s0 = k.q.Translated(k.b * k.l0);
  if (std::abs(gp_Lin(k.c1.axis).Distance(s0) - (k.R - k.Rc)) > tol || std::abs(gp_Lin(k.c2.axis).Distance(s0) - (k.R - k.Rc)) > tol)
    return false;
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
  {
    k.n             = n * sign;
    const Section b = section_at(k, 0), t = section_at(k, M_PI / 2);
    int           found = 0;
    for (const gp_Pnt& want : {b.a, b.c, t.a})
      for (const gp_Pnt& v : vs)
        found += v.Distance(want) < tol * 10;
    if (found == 3)
      return true;
  }
  return false;
}

// Endpoints of an edge, first to last.
std::pair<gp_Pnt, gp_Pnt> ends(const TopoDS_Edge& e)
{
  return {BRep_Tool::Pnt(TopExp::FirstVertex(e)), BRep_Tool::Pnt(TopExp::LastVertex(e))};
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
        surf = blend_surface(k, n, tol * 0.25);
      if (surf.IsNull())
        continue;
      const double            etol = std::max(tol, Precision::Confusion()); // edge / vertex tolerances
      BRepBuilderAPI_MakeFace mf(surf, 0, M_PI / 2, 0, 1, etol);
      if (!mf.IsDone())
        continue;
      TopoDS_Face nf = mf.Face();
      {
        // Outward: away from the ball's centre.
        BRepAdaptor_Surface an(nf, false);
        gp_Pnt              p;
        gp_Vec              du, dv;
        an.D1(M_PI / 4, 0.5, p, du, dv);
        gp_Vec nn = du.Crossed(dv);
        if (nf.Orientation() == TopAbs_REVERSED)
          nn.Reverse();
        if (nn.Dot(gp_Vec(k.spine(M_PI / 4), p)) < 0)
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
