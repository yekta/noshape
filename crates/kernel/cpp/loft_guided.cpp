// Loft between two profiles along guide curves, for profiles with curved
// edges (Parasolid's guided loft as Onshape shows it, live kern G32 (f)):
//   - each profile is divided at its corners and at the guides' ends; a
//     profile without corners (a circle) is divided evenly by length between
//     the guide ends, as many pieces as the other profile has;
//   - side faces run between consecutive corners (a guide off a corner lies
//     inside its face, a guide at a corner is the seam);
//   - a side face without a guide is ruled between its two profile pieces
//     (straight seams); a face holding a guide is a Gordon surface through
//     its profile pieces, its seams and the guides, interpolated as a cubic
//     B-spline.

#include "internal.hpp"
#include <BRepAdaptor_CompCurve.hxx>
#include <TopExp.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <GeomConvert.hxx>
#include <GeomConvert_CompCurveToBSplineCurve.hxx>
#include <Approx_CurvilinearParameter.hxx>
#include <Geom_Conic.hxx>
#include <Geom_Line.hxx>
#include <BSplCLib.hxx>
#include <GeomAdaptor_Curve.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepCheck_Shell.hxx>
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRep_Builder.hxx>
#include <BRepTools_ReShape.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepFill.hxx>
#include <BRepLib.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepGProp.hxx>
#include <Extrema_ExtCC.hxx>
#include <Extrema_ExtPC.hxx>
#include <GeomConvert_SurfToAnaSurf.hxx>
#include <ShapeFix_Face.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GeomAPI_PointsToBSplineSurface.hxx>
#include <Geom_BSplineSurface.hxx>
#include <NCollection_Array2.hxx>
#include <NCollection_HArray1.hxx>
#include <ShapeFix_Solid.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <algorithm>
#include <cstdlib>
#include <functional>
#include <cmath>
#include <vector>

namespace noshape {

namespace {

// The angle over which a profile joint is a corner (an edge of the loft): live smooths joints up to 3° over (a stadium's
// right joints kinked by 0.0523 rad give one side face, by 0.0524 a side edge up from each; REQUESTS 3023).
constexpr double SMOOTH_JOINT = 0.05235987755982988; // 3°

// A closed profile loop by arc length.
struct Loop
{
  TopoDS_Wire                            wire;
  std::unique_ptr<BRepAdaptor_CompCurve> c;
  double                                 t0 = 0, t1 = 0, len = 0;
  // Arc length at evenly spaced parameters (each step integrated once): param and abscissa look up their step and
  // integrate only within it (GCPnts_AbscissaPoint from the start each time was most of c2-hunting-knife's Loft 1).
  std::vector<double> ts, ss;

  explicit Loop(const TopoDS_Wire& w) : wire(w), c(new BRepAdaptor_CompCurve(w))
  {
    t0             = c->FirstParameter();
    t1             = c->LastParameter();
    const int    n = 256;
    ts.push_back(t0);
    ss.push_back(0);
    for (int i = 1; i <= n; ++i)
    {
      ts.push_back(t0 + (t1 - t0) * i / n);
      ss.push_back(ss.back() + GCPnts_AbscissaPoint::Length(*c, ts[(size_t)i - 1], ts[(size_t)i]));
    }
    len = ss.back();
  }
  // Arc length from the start to parameter t.
  double length_to(double t) const
  {
    t              = std::clamp(t, t0, t1);
    const size_t i = std::min(ts.size() - 2, (size_t)(std::upper_bound(ts.begin(), ts.end(), t) - ts.begin()) - 1);
    return ss[i] + (t > ts[i] ? GCPnts_AbscissaPoint::Length(*c, ts[i], t) : 0.0);
  }
  // Parameter at arc length s (wrapped into [0, len)).
  double param(double s) const
  {
    s = std::fmod(s, len);
    if (s < 0)
      s += len;
    if (s < 1e-15)
      return t0;
    const size_t i = std::min(ts.size() - 2, (size_t)(std::upper_bound(ss.begin(), ss.end(), s) - ss.begin()) - 1);
    GCPnts_AbscissaPoint ap(*c, s - ss[i], ts[i]);
    if (!ap.IsDone())
      fail_op("loft: arc length on a profile failed");
    return ap.Parameter();
  }
  gp_Pnt at(double s) const { return c->Value(param(s)); }
  // Arc length of the point of the loop nearest `p` and its distance.
  double abscissa(const gp_Pnt& p, double& dist) const
  {
    Extrema_ExtPC ext(p, *c);
    dist = 1e300;
    double t = t0;
    if (ext.IsDone())
      for (int i = 1; i <= ext.NbExt(); ++i)
        if (ext.SquareDistance(i) < dist)
        {
          dist = ext.SquareDistance(i);
          t    = ext.Point(i).Parameter();
        }
    // The ends of an open parameter range.
    for (double e : {t0, t1})
      if (p.SquareDistance(c->Value(e)) < dist)
      {
        dist = p.SquareDistance(c->Value(e));
        t    = e;
      }
    dist = std::sqrt(dist);
    const double s = length_to(t);
    return s >= len - 1e-12 ? 0.0 : s;
  }
  // Arc lengths of the corners (tangent breaks between edges).
  std::vector<double> corners() const { return joints(SMOOTH_JOINT); }
  // Arc lengths of the joints between edges turning by more than `over` (every joint for a negative `over`).
  std::vector<double> joints(double over) const
  {
    std::vector<double> out;
    std::vector<TopoDS_Edge> edges;
    for (BRepTools_WireExplorer x(wire); x.More(); x.Next())
      edges.push_back(x.Current());
    const size_t n = edges.size();
    for (size_t i = 0; i < n; ++i)
    {
      const TopoDS_Edge& a = edges[i];
      const TopoDS_Edge& b = edges[(i + 1) % n];
      BRepAdaptor_Curve  ca(a), cb(b);
      const double       ta = a.Orientation() == TopAbs_REVERSED ? ca.FirstParameter() : ca.LastParameter();
      const double       tb = b.Orientation() == TopAbs_REVERSED ? cb.LastParameter() : cb.FirstParameter();
      gp_Pnt             pa, pb;
      gp_Vec             da, db;
      ca.D1(ta, pa, da);
      cb.D1(tb, pb, db);
      if (a.Orientation() == TopAbs_REVERSED)
        da.Reverse();
      if (b.Orientation() == TopAbs_REVERSED)
        db.Reverse();
      if (da.Magnitude() < 1e-12 || db.Magnitude() < 1e-12)
        continue;
      if (da.Angle(db) > over)
      {
        double d;
        out.push_back(abscissa(pa, d));
      }
    }
    std::sort(out.begin(), out.end());
    return out;
  }
};

TopoDS_Wire outer_wire(const TopoDS_Shape& s)
{
  if (s.ShapeType() == TopAbs_WIRE)
    return TopoDS::Wire(s);
  if (s.ShapeType() == TopAbs_FACE)
    return BRepTools::OuterWire(TopoDS::Face(s));
  for (TopExp_Explorer x(s, TopAbs_WIRE); x.More(); x.Next())
    return TopoDS::Wire(x.Current());
  fail_invalid("loft profile has no wire");
}

// Arc length from `a` forward to `b` on a loop of length `len`.
double forward(double a, double b, double len)
{
  double d = std::fmod(b - a, len);
  return d < 0 ? d + len : d;
}

// Where a guide meets a profile curve: the guide parameter and the point,
// the nearest of the curves' extrema and their ends' projections.
double meet(const Adaptor3d_Curve& g, const Adaptor3d_Curve& p, double& tg, gp_Pnt& at)
{
  double best = 1e300;
  auto   take = [&](double d2, double t, const gp_Pnt& q) {
    if (d2 < best)
    {
      best = d2;
      tg   = t;
      at   = q;
    }
  };
  Extrema_ExtCC cc(g, p);
  if (cc.IsDone() && !cc.IsParallel())
    for (int i = 1; i <= cc.NbExt(); ++i)
    {
      Extrema_POnCurv a, b;
      cc.Points(i, a, b);
      take(cc.SquareDistance(i), a.Parameter(), a.Value());
    }
  for (double e : {g.FirstParameter(), g.LastParameter()})
  {
    const gp_Pnt  q = g.Value(e);
    Extrema_ExtPC x(q, p);
    if (x.IsDone())
      for (int i = 1; i <= x.NbExt(); ++i)
        take(x.SquareDistance(i), e, q);
    for (double f : {p.FirstParameter(), p.LastParameter()})
      take(q.SquareDistance(p.Value(f)), e, q);
  }
  for (double f : {p.FirstParameter(), p.LastParameter()})
  {
    const gp_Pnt  q = p.Value(f);
    Extrema_ExtPC x(q, g);
    if (x.IsDone())
      for (int i = 1; i <= x.NbExt(); ++i)
        take(x.SquareDistance(i), x.Point(i).Parameter(), q);
  }
  return std::sqrt(best);
}

// meet() for a guide that ends on the profile (as guides do): the end within `tol` of the profile's wire, checked
// edge by edge; else meet() with the whole profile (OCCT's curve-curve extrema against a composite profile were 9 ms a
// guide, c2-hunting-knife's Loft 1).
double meet(const Adaptor3d_Curve& g, const Adaptor3d_Curve& p, const TopoDS_Wire& w, double tol, double& tg, gp_Pnt& at)
{
  double best = 1e300;
  for (const double e : {g.FirstParameter(), g.LastParameter()})
  {
    const gp_Pnt q = g.Value(e);
    for (TopExp_Explorer x(w, TopAbs_EDGE); x.More(); x.Next())
    {
      if (BRep_Tool::Degenerated(TopoDS::Edge(x.Current())))
        continue;
      BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
      double            d2 = std::min(q.SquareDistance(c.Value(c.FirstParameter())), q.SquareDistance(c.Value(c.LastParameter())));
      Extrema_ExtPC     ext(q, c);
      if (ext.IsDone())
        for (int i = 1; i <= ext.NbExt(); ++i)
          d2 = std::min(d2, ext.SquareDistance(i));
      if (d2 < best)
        best = d2, tg = e, at = q;
    }
  }
  if (std::sqrt(best) <= tol)
    return std::sqrt(best);
  return meet(g, p, tg, at);
}

// A guide as a curve from its start on profile A to its end on profile B,
// by normalized arc length.
// A guide as one curve: its edge, or its edges chained into a wire (a guide of several sketch curves, c2-hunting-knife's
// Loft 3) run as one.
std::unique_ptr<Adaptor3d_Curve> guide_curve(const TopoDS_Shape& gx)
{
  std::vector<TopoDS_Edge> edges;
  for (TopExp_Explorer x(gx, TopAbs_EDGE); x.More(); x.Next())
    edges.push_back(TopoDS::Edge(x.Current()));
  if (edges.empty())
    fail_invalid("a loft guide has no curve");
  if (edges.size() == 1)
    return std::make_unique<BRepAdaptor_Curve>(edges[0]);
  BRepBuilderAPI_MakeWire mk;
  ShapeList               list;
  for (const TopoDS_Edge& e : edges)
    list.Append(e);
  mk.Add(list);
  if (!mk.IsDone())
    fail_op("A loft guide's curves are not connected.");
  return std::make_unique<BRepAdaptor_CompCurve>(mk.Wire());
}

struct Guide
{
  std::unique_ptr<Adaptor3d_Curve> c;
  double                           t0, t1, len;
  bool                               reversed;
  double                             sa, sb; // arc lengths on A and B
  gp_Pnt at(double v) const
  {
    const double s = (reversed ? 1.0 - v : v) * len;
    GCPnts_AbscissaPoint ap(*c, s, t0);
    return c->Value(ap.IsDone() ? ap.Parameter() : t0 + (t1 - t0) * (reversed ? 1.0 - v : v));
  }
};

// The columns' weights at `u` (live kern G32 (f)'s guided face): a cubic
// Hermite tent per column, 1 at its node and 0 at the neighbouring nodes,
// flat at both (3t² − 2t³).
std::vector<double> hermite_tents(const std::vector<double>& nodes, double u)
{
  std::vector<double> w(nodes.size(), 0.0);
  auto                step = [](double t) { return t * t * (3 - 2 * t); };
  for (size_t m = 0; m < nodes.size(); ++m)
  {
    if (m > 0 && u >= nodes[m - 1] && u <= nodes[m])
      w[m] = step((u - nodes[m - 1]) / (nodes[m] - nodes[m - 1]));
    else if (m + 1 < nodes.size() && u >= nodes[m] && u <= nodes[m + 1])
      w[m] = 1 - step((u - nodes[m]) / (nodes[m + 1] - nodes[m]));
  }
  return w;
}

// A cubic B-spline surface through a grid of points (rows along u).
occ::handle<Geom_BSplineSurface> grid_surface(const std::vector<std::vector<gp_Pnt>>& g)
{
  const int                  nu = (int)g.size(), nv = (int)g[0].size();
  NCollection_Array2<gp_Pnt> pts(1, nu, 1, nv);
  for (int i = 0; i < nu; ++i)
    for (int j = 0; j < nv; ++j)
      pts(i + 1, j + 1) = g[i][j];
  GeomAPI_PointsToBSplineSurface fit;
  fit.Interpolate(pts, Approx_IsoParametric);
  if (!fit.IsDone())
    fail_op("loft: guided face interpolation failed");
  return fit.Surface();
}

// A cubic B-spline surface through a grid's rows at `us` (its u), with the u-derivatives `t0` / `t1` (one per column) at
// its first and last rows: pieces with the same derivative where they meet join C1.
occ::handle<Geom_BSplineSurface> grid_surface_with_ends(const std::vector<std::vector<gp_Pnt>>& g, const std::vector<double>& us, const std::vector<gp_Vec>& t0, const std::vector<gp_Vec>& t1)
{
  const int                                nu = (int)g.size(), nv = (int)g[0].size();
  occ::handle<NCollection_HArray1<double>> pu = new NCollection_HArray1<double>(1, nu);
  for (int i = 0; i < nu; ++i)
    pu->SetValue(i + 1, us[i]);
  std::vector<occ::handle<Geom_BSplineCurve>> cols;
  for (int j = 0; j < nv; ++j)
  {
    occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, nu);
    for (int i = 0; i < nu; ++i)
      pts->SetValue(i + 1, g[i][j]);
    GeomAPI_Interpolate ip(pts, pu, false, 1e-12);
    ip.Load(t0[j], t1[j], false);
    ip.Perform();
    if (!ip.IsDone())
      fail_op("loft: guided face interpolation failed");
    cols.push_back(ip.Curve());
  }
  const int                                np = cols[0]->NbPoles();
  occ::handle<NCollection_HArray1<double>> pv = new NCollection_HArray1<double>(1, nv);
  for (int j = 0; j < nv; ++j)
    pv->SetValue(j + 1, (double)j / (nv - 1));
  NCollection_Array2<gp_Pnt>                  poles;
  occ::handle<Geom_BSplineCurve>              across;
  for (int i = 1; i <= np; ++i)
  {
    occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, nv);
    for (int j = 0; j < nv; ++j)
      pts->SetValue(j + 1, cols[j]->Pole(i));
    GeomAPI_Interpolate ip(pts, pv, false, 1e-12);
    ip.Perform();
    if (!ip.IsDone())
      fail_op("loft: guided face interpolation failed");
    across = ip.Curve();
    if (i == 1)
      poles.Resize(1, np, 1, across->NbPoles(), false);
    for (int j = 1; j <= across->NbPoles(); ++j)
      poles(i, j) = across->Pole(j);
  }
  return new Geom_BSplineSurface(poles, cols[0]->Knots(), across->Knots(), cols[0]->Multiplicities(), across->Multiplicities(), cols[0]->Degree(), across->Degree());
}

// Surfaces through grids with the same last / first rows and the same columns, one after another in u, as one
// surface (C0 where they meet, C1 where their derivatives agree: one knot fewer there); `at` the u each starts at,
// then where the last ends.
occ::handle<Geom_BSplineSurface> join_u(std::vector<occ::handle<Geom_BSplineSurface>> ps, const std::vector<double>& at)
{
  if (ps.size() == 1)
    return ps[0];
  int du = 0;
  for (const auto& p : ps)
    du = std::max(du, p->UDegree());
  const int dv = ps[0]->VDegree(), nv = ps[0]->NbVPoles();
  for (auto& p : ps)
  {
    if (p->IsURational() || p->IsVRational() || p->VDegree() != dv || p->NbVPoles() != nv || p->NbVKnots() != ps[0]->NbVKnots())
      fail_op("loft: guided face pieces differ");
    if (p->UDegree() < du)
    {
      p = occ::down_cast<Geom_BSplineSurface>(p->Copy());
      p->IncreaseDegree(du, dv);
    }
  }
  std::vector<double>              knots;
  std::vector<int>                 mults;
  std::vector<std::vector<gp_Pnt>> rows;
  for (size_t k = 0; k < ps.size(); ++k)
  {
    const auto&  p  = ps[k];
    const int    nk = p->NbUKnots();
    const double k0 = p->UKnot(1), k1 = p->UKnot(nk);
    auto         map = [&](double x) { return at[k] + (at[k + 1] - at[k]) * (x - k0) / (k1 - k0); };
    for (int i = k == 0 ? 1 : 2; i < nk; ++i)
    {
      knots.push_back(map(p->UKnot(i)));
      mults.push_back(p->UMultiplicity(i));
    }
    // A piece's first row is the last one's last.
    for (int i = k == 0 ? 1 : 2; i <= p->NbUPoles(); ++i)
    {
      std::vector<gp_Pnt> row;
      for (int j = 1; j <= nv; ++j)
        row.push_back(p->Pole(i, j));
      rows.push_back(row);
    }
    if (k + 1 < ps.size())
    {
      knots.push_back(at[k + 1]);
      mults.push_back(du);
    }
  }
  knots.push_back(at.back());
  mults.push_back(du + 1);
  NCollection_Array2<gp_Pnt> poles(1, (int)rows.size(), 1, nv);
  for (size_t i = 0; i < rows.size(); ++i)
    for (int j = 0; j < nv; ++j)
      poles((int)i + 1, j + 1) = rows[i][j];
  NCollection_Array1<double> uk(1, (int)knots.size()), vk(1, ps[0]->NbVKnots());
  NCollection_Array1<int>    um(1, (int)mults.size()), vm(1, ps[0]->NbVKnots());
  for (size_t i = 0; i < knots.size(); ++i)
  {
    uk((int)i + 1) = knots[i];
    um((int)i + 1) = mults[i];
  }
  for (int i = 1; i <= ps[0]->NbVKnots(); ++i)
  {
    vk(i) = ps[0]->VKnot(i);
    vm(i) = ps[0]->VMultiplicity(i);
  }
  occ::handle<Geom_BSplineSurface> out = new Geom_BSplineSurface(poles, uk, vk, um, vm, du, dv);
  double                           x0, x1, y0, y1;
  out->Bounds(x0, x1, y0, y1);
  for (size_t k = 1; k + 1 < at.size(); ++k)
    for (int i = 2; i < out->NbUKnots(); ++i)
      if (std::abs(out->UKnot(i) - at[k]) < 1e-12 && out->UMultiplicity(i) == du)
      {
        Bnd_Box b;
        for (int a = 1; a <= out->NbUPoles(); ++a)
          for (int c = 1; c <= out->NbVPoles(); ++c)
            b.Add(out->Pole(a, c));
        out->RemoveUKnot(i, du - 1, 1e-9 * std::sqrt(b.SquareExtent()));
        break;
      }
  return out;
}

// A cubic B-spline face through a grid of points (rows along u).
TopoDS_Face grid_face(const std::vector<std::vector<gp_Pnt>>& g)
{
  BRepBuilderAPI_MakeFace mf(grid_surface(g), 1e-7);
  if (!mf.IsDone())
    fail_op("loft: guided face failed");
  return mf.Face();
}

// The edge through points of a curve sampled at `n + 1` positions.
TopoDS_Edge sampled_edge(const std::function<gp_Pnt(double)>& f, int n)
{
  occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, n + 1);
  occ::handle<NCollection_HArray1<double>> prm = new NCollection_HArray1<double>(1, n + 1);
  for (int i = 0; i <= n; ++i)
  {
    pts->SetValue(i + 1, f((double)i / n));
    prm->SetValue(i + 1, (double)i / n);
  }
  GeomAPI_Interpolate ip(pts, prm, false, 1e-9);
  ip.Perform();
  if (!ip.IsDone())
    fail_op("loft: profile piece failed");
  return BRepBuilderAPI_MakeEdge(ip.Curve(), 0.0, 1.0).Edge();
}

// The profile with a vertex at each point inside one of its edges (live splits a profile's edge where a guide meets it:
// c2-hunting-knife's Loft 1 has a vertex at each guide end on both profiles).
TopoDS_Shape split_at(const TopoDS_Shape& in, const std::vector<gp_Pnt>& at, double tol)
{
  // A closed edge (a circle) started at a split point: no vertex left where it started (live's circle split by two
  // guides is two edges).
  BRepTools_ReShape rs;
  for (TopExp_Explorer x(in, TopAbs_EDGE); x.More(); x.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(x.Current());
    TopoDS_Vertex      v0, v1;
    TopExp::Vertices(e, v0, v1);
    double                        f, l;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
    if (!v0.IsSame(v1) || c.IsNull() || !c->IsPeriodic() || rs.IsRecorded(e))
      continue;
    for (const gp_Pnt& p : at)
    {
      GeomAPI_ProjectPointOnCurve pp(p, c);
      if (pp.NbPoints() == 0 || pp.LowerDistance() > tol || p.Distance(BRep_Tool::Pnt(v0)) <= tol)
        continue;
      const double t  = pp.LowerDistanceParameter();
      TopoDS_Edge  ne = BRepBuilderAPI_MakeEdge(c, t, t + (l - f)).Edge();
      ne.Orientation(e.Orientation());
      rs.Replace(e.Oriented(TopAbs_FORWARD), ne.Oriented(TopAbs_FORWARD));
      break;
    }
  }
  const TopoDS_Shape profile = rs.Apply(in);
  ShapeIMap vs;
  TopExp::MapShapes(profile, TopAbs_VERTEX, vs);
  ShapeList tools;
  for (const gp_Pnt& p : at)
  {
    bool known = false;
    for (int i = 1; i <= vs.Extent() && !known; ++i)
      known = BRep_Tool::Pnt(TopoDS::Vertex(vs(i))).Distance(p) <= tol;
    if (!known)
      tools.Append(BRepBuilderAPI_MakeVertex(p).Vertex());
  }
  if (tools.IsEmpty())
    return profile;
  ShapeList args;
  args.Append(profile);
  BRepAlgoAPI_Splitter sp;
  sp.SetArguments(args);
  sp.SetTools(tools);
  sp.SetNonDestructive(true);
  sp.Build();
  return sp.HasErrors() ? profile : sp.Shape();
}

// Sewing keeps the longer of two edges it merges, and any spline along a line is longer than the line: the profile's own
// curves back on the sewn edges along them (live's handle keeps its profiles' lines and arcs).
void keep_profile_curves(const TopoDS_Shape& sewn, const TopoDS_Shape& profile, double tol)
{
  auto ends = [](const TopoDS_Edge& e, gp_Pnt& a, gp_Pnt& m, gp_Pnt& b) {
    BRepAdaptor_Curve c(e);
    a = c.Value(c.FirstParameter());
    m = c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
    b = c.Value(c.LastParameter());
  };
  ShapeIMap ps;
  TopExp::MapShapes(profile, TopAbs_EDGE, ps);
  ShapeIMap ss;
  TopExp::MapShapes(sewn, TopAbs_EDGE, ss);
  BRep_Builder bb;
  for (int i = 1; i <= ss.Extent(); ++i)
  {
    const TopoDS_Edge& se = TopoDS::Edge(ss(i));
    TopLoc_Location    ls;
    double             sf, sl;
    const occ::handle<Geom_Curve> sc = BRep_Tool::Curve(se, ls, sf, sl);
    if (sc.IsNull() || !ls.IsIdentity() || sc->IsKind(STANDARD_TYPE(Geom_Line)) || sc->IsKind(STANDARD_TYPE(Geom_Conic)))
      continue;
    gp_Pnt s0, sm, s1;
    ends(se, s0, sm, s1);
    for (int j = 1; j <= ps.Extent(); ++j)
    {
      const TopoDS_Edge&      pe = TopoDS::Edge(ps(j));
      TopLoc_Location         lp;
      double                  pf, pl;
      occ::handle<Geom_Curve> pc = BRep_Tool::Curve(pe, lp, pf, pl);
      if (pc.IsNull() || pc == sc || !lp.IsIdentity())
        continue;
      while (const auto t = occ::down_cast<Geom_TrimmedCurve>(pc))
        pc = t->BasisCurve();
      gp_Pnt p0, pm, p1;
      ends(pe, p0, pm, p1);
      const bool along = s0.Distance(p0) < tol && s1.Distance(p1) < tol, back = s0.Distance(p1) < tol && s1.Distance(p0) < tol;
      if (!along && !back)
        continue;
      // (The profile edge's middle on the sewn curve: their parameters run differently.)
      GeomAPI_ProjectPointOnCurve on(pm, sc, sf, sl);
      if (on.NbPoints() == 0 || on.LowerDistance() > tol)
        continue;
      if (back)
      {
        const double f = pc->ReversedParameter(pl), l = pc->ReversedParameter(pf);
        pc = pc->Reversed();
        pf = f;
        pl = l;
      }
      bb.UpdateEdge(se, pc, BRep_Tool::Tolerance(se));
      bb.Range(se, pf, pl, true);
      bb.SameRange(se, false);
      bb.SameParameter(se, false);
      BRepLib::SameParameter(se, tol);
      break;
    }
  }
}

} // namespace

std::unique_ptr<OcShape> loft_guided_curved(const OcShape& profile_a, const OcShape& profile_b, const OcShapeList& guides,
                                            bool make_solid)
{
  require_non_null(profile_a.shape, "first profile");
  require_non_null(profile_b.shape, "second profile");
  std::optional<KernelTimer> t_setup(std::in_place, "loft_guided.setup");
  Loop A(outer_wire(profile_a.shape)), B(outer_wire(profile_b.shape));
  const double size = std::max(A.len, B.len);
  const double tol  = 1e-7 * (1 + size);
  // B runs the way A does about the loft's axis.
  auto newell = [](const Loop& L) {
    gp_XYZ n(0, 0, 0), c(0, 0, 0);
    const int N = 64;
    std::vector<gp_Pnt> p;
    for (int i = 0; i < N; ++i)
      p.push_back(L.at(L.len * i / N));
    for (int i = 0; i < N; ++i)
    {
      n += p[i].XYZ().Crossed(p[(i + 1) % N].XYZ());
      c += p[i].XYZ() / N;
    }
    return std::pair<gp_XYZ, gp_XYZ>(n, c);
  };
  const auto [na, ca] = newell(A);
  const auto [nb, cb] = newell(B);
  const bool flip_b = na.Dot(cb - ca) * nb.Dot(cb - ca) < 0;
  // B's arc length in A's direction.
  auto b_at = [&](double s) { return B.at(flip_b ? B.len - s : s); };
  auto b_abs = [&](const gp_Pnt& p, double& d) {
    const double s = B.abscissa(p, d);
    return flip_b ? std::fmod(B.len - s, B.len) : s;
  };
  // The guides, oriented A → B.
  std::vector<Guide> gs;
  for (const TopoDS_Shape& gx : guides.items)
  {
    Guide g;
    g.c = guide_curve(gx);
    // Trimmed to its stretch between the profiles.
    double ta, tb;
    gp_Pnt pa, pb;
    if (meet(*g.c, *A.c, A.wire, tol, ta, pa) > tol || meet(*g.c, *B.c, B.wire, tol, tb, pb) > tol || std::abs(ta - tb) < 1e-12)
      fail_op("All guides should intersect each profile boundary.");
    g.reversed = ta > tb;
    g.t0       = std::min(ta, tb);
    g.t1       = std::max(ta, tb);
    g.len      = GCPnts_AbscissaPoint::Length(*g.c, g.t0, g.t1);
    double d;
    g.sa = A.abscissa(pa, d);
    g.sb = b_abs(pb, d);
    gs.push_back(std::move(g));
  }
  if (gs.empty())
    fail_invalid("loft: no guides");
  std::sort(gs.begin(), gs.end(), [&](const Guide& x, const Guide& y) { return x.sa < y.sa; });
  // Division points: A's corners and the guide starts; B's matching points.
  std::vector<double>       corners_a = A.corners();
  std::vector<double>       cb_raw    = B.corners();
  const std::vector<double> joints_a  = A.joints(-1);
  std::vector<double>       joints_b;
  for (double s : B.joints(-1))
    joints_b.push_back(flip_b ? std::fmod(B.len - s, B.len) : s);
  std::vector<double> corners_b;
  for (double s : cb_raw)
    corners_b.push_back(flip_b ? std::fmod(B.len - s, B.len) : s);
  // Joints lined up across the loft (one of A's lands on one of B's, by arc length between the guides) are corners
  // however smooth (live 3023 / 3053: a stadium lofted to a stadium has a side edge up from each joint; turned 90°,
  // one side face).
  auto on_b = [&](double s) {
    for (size_t i = 0; i < gs.size(); ++i)
    {
      const Guide& g = gs[i];
      const Guide& h = gs[(i + 1) % gs.size()];
      const double la = gs.size() == 1 ? A.len : forward(g.sa, h.sa, A.len), lb = gs.size() == 1 ? B.len : forward(g.sb, h.sb, B.len);
      const double o  = forward(g.sa, s, A.len);
      if (o <= la)
        return std::fmod(g.sb + lb * o / la, B.len);
    }
    return -1.0;
  };
  const double lined = 1e-6 * (1 + size);
  for (double s : joints_a)
  {
    const double t = on_b(s);
    for (double u : joints_b)
      if (t >= 0 && std::min(forward(t, u, B.len), forward(u, t, B.len)) < lined)
      {
        auto same = [&](double c, double x, double len) { return std::min(forward(c, x, len), forward(x, c, len)) < tol; };
        if (std::none_of(corners_a.begin(), corners_a.end(), [&](double c) { return same(c, s, A.len); }))
          corners_a.push_back(s);
        if (std::none_of(corners_b.begin(), corners_b.end(), [&](double c) { return same(c, u, B.len); }))
          corners_b.push_back(u);
      }
  }
  std::sort(corners_a.begin(), corners_a.end());
  std::sort(corners_b.begin(), corners_b.end());
  struct Pt
  {
    double sa, sb;
    int    guide; // index into gs, -1 when none
    bool   corner;
  };
  std::vector<Pt> pts;
  auto close_to = [&](double a, double b, double len) { return std::min(forward(a, b, len), forward(b, a, len)) < tol; };
  for (double s : corners_a)
    pts.push_back({s, 0, -1, true});
  for (size_t i = 0; i < gs.size(); ++i)
  {
    bool merged = false;
    for (Pt& p : pts)
      if (close_to(p.sa, gs[i].sa, A.len))
      {
        p.guide = (int)i;
        merged  = true;
      }
    if (!merged)
      pts.push_back({gs[i].sa, 0, (int)i, false});
  }
  std::sort(pts.begin(), pts.end(), [](const Pt& x, const Pt& y) { return x.sa < y.sa; });
  const size_t n = pts.size();
  if (!corners_b.empty())
  {
    // Both profiles with corners: as many points on each, matched in order
    // from a guide.
    std::vector<double> pb = corners_b;
    for (const Guide& g : gs)
    {
      bool on = false;
      for (double s : pb)
        on |= close_to(s, g.sb, B.len);
      if (!on)
        pb.push_back(g.sb);
    }
    std::sort(pb.begin(), pb.end());
    if (pb.size() != n)
      fail_op("Loft guides need profiles with as many corners yet");
    size_t ia = 0, ib = 0;
    for (size_t i = 0; i < n; ++i)
      if (pts[i].guide >= 0)
      {
        ia = i;
        break;
      }
    for (size_t j = 0; j < n; ++j)
      if (close_to(pb[j], gs[pts[ia].guide].sb, B.len))
        ib = j;
    for (size_t i = 0; i < n; ++i)
    {
      pts[(ia + i) % n].sb = pb[(ib + i) % n];
      if (pts[(ia + i) % n].guide >= 0 && !close_to(pts[(ia + i) % n].sb, gs[pts[(ia + i) % n].guide].sb, B.len))
        fail_op("All guides should intersect each profile boundary.");
    }
    for (size_t i = 0; i < n; ++i)
      for (double s : corners_b)
        if (close_to(pts[i].sb, s, B.len))
          pts[i].corner = true;
  }
  else
  {
    // B without corners: evenly between consecutive guide ends.
    std::vector<size_t> anchors;
    for (size_t i = 0; i < n; ++i)
      if (pts[i].guide >= 0)
        anchors.push_back(i);
    for (size_t k = 0; k < anchors.size(); ++k)
    {
      const size_t i0 = anchors[k], i1 = anchors[(k + 1) % anchors.size()];
      const size_t cnt = anchors.size() == 1 ? n : (i1 + n - i0) % n;
      const double b0 = gs[pts[i0].guide].sb;
      double       span = anchors.size() == 1 ? B.len : forward(b0, gs[pts[i1].guide].sb, B.len);
      for (size_t j = 0; j < cnt; ++j)
        pts[(i0 + j) % n].sb = std::fmod(b0 + span * j / cnt, B.len);
    }
  }
  // Seams: the corners (and guides at corners).
  std::vector<size_t> seams;
  for (size_t i = 0; i < n; ++i)
    if (pts[i].corner)
      seams.push_back(i);
  // Both profiles smooth (a handle's cross-sections): one face round the loop, from the first guide (live: c2-hunting-
  // knife's Loft 1, the handle's side one face).
  if (seams.empty())
    for (size_t i = 0; i < n && seams.empty(); ++i)
      if (pts[i].guide >= 0)
      {
        pts[i].corner = true;
        seams.push_back(i);
      }
  if (seams.empty())
    fail_op("Loft guides need a profile with corners yet");
  // The faces between consecutive seams.
  std::vector<gp_Pnt>       splits_b;
  std::vector<TopoDS_Shape> caps;
  BRepBuilderAPI_Sewing     sew(1e-6 * (1 + size));
  const int             NU = 48, NV = 24;
  t_setup.reset();
  std::optional<KernelTimer> t_faces(std::in_place, "loft_guided.faces");
  for (size_t k = 0; k < seams.size(); ++k)
  {
    const size_t i0 = seams[k], i1 = seams[(k + 1) % seams.size()];
    const double sa0 = pts[i0].sa, la = forward(sa0, pts[i1].sa, A.len) + (seams.size() == 1 ? A.len : 0);
    const double sb0 = pts[i0].sb, lb = forward(sb0, pts[i1].sb, B.len) + (seams.size() == 1 ? B.len : 0);
    // Guides inside the face, by position along A and along B.
    std::vector<double> ua{0.0}, ub{0.0};
    std::vector<int>    cols{pts[i0].guide};
    for (size_t j = (i0 + 1) % n; j != i1; j = (j + 1) % n)
      if (pts[j].guide >= 0)
      {
        ua.push_back(forward(sa0, pts[j].sa, A.len) / la);
        ub.push_back(forward(sb0, pts[j].sb, B.len) / lb);
        cols.push_back(pts[j].guide);
      }
    ua.push_back(1.0);
    ub.push_back(1.0);
    cols.push_back(pts[i1].guide);
    auto bot = [&](double u) { return A.at(sa0 + la * u); };
    // B above A by arc length between the guide ends, and joint to joint where a stretch between two guides has as
    // many joints on both (live 3070: the handle's sections pair its profiles' edges in order, its u knots the means of
    // their arc-length shares).
    std::vector<double> ma{ua[0]}, mb{ub[0]};
    for (size_t g = 0; g + 1 < ua.size(); ++g)
    {
      std::vector<double> ja, jb;
      for (double s : joints_a)
        if (const double u = forward(sa0, s, A.len) / la; u > ua[g] + 1e-9 && u < ua[g + 1] - 1e-9)
          ja.push_back(u);
      for (double s : joints_b)
        if (const double o = forward(sb0, s, B.len) / lb; o > ub[g] + 1e-9 && o < ub[g + 1] - 1e-9)
          jb.push_back(o);
      std::sort(ja.begin(), ja.end());
      std::sort(jb.begin(), jb.end());
      if (ja.size() == jb.size())
      {
        ma.insert(ma.end(), ja.begin(), ja.end());
        mb.insert(mb.end(), jb.begin(), jb.end());
      }
      ma.push_back(ua[g + 1]);
      mb.push_back(ub[g + 1]);
    }
    // B's share at A's share u: over each stretch between paired points a cubic in u, at each paired point running at
    // the geometric mean of the stretches' speeds there (a speed jumping there left the side's edge 19 µm off B, its C1
    // pieces averaging the two).
    const size_t        nm = ma.size();
    std::vector<double> ks(nm - 1), ds(nm);
    for (size_t i = 0; i + 1 < nm; ++i)
      ks[i] = (mb[i + 1] - mb[i]) / (ma[i + 1] - ma[i]);
    const bool round = seams.size() == 1;
    for (size_t i = 0; i < nm; ++i)
    {
      const double before = i > 0 ? ks[i - 1] : ks[round ? nm - 2 : 0], after = i + 1 < nm ? ks[i] : ks[round ? 0 : nm - 2];
      ds[i]               = std::sqrt(before * after);
    }
    auto b_share = [&](double u) {
      size_t s = 0;
      while (s + 2 < nm && u > ma[s + 1])
        ++s;
      const double t = (u - ma[s]) / (ma[s + 1] - ma[s]), t2 = t * t, t3 = t2 * t;
      double       a = ds[s] / ks[s], b = ds[s + 1] / ks[s];
      // (Monotone: a² + b² <= 9.)
      if (a * a + b * b > 9)
      {
        const double k = 3 / std::sqrt(a * a + b * b);
        a *= k;
        b *= k;
      }
      return mb[s] + (mb[s + 1] - mb[s]) * ((3 * t2 - 2 * t3) + a * (t3 - 2 * t2 + t) + b * (t3 - t2));
    };
    auto top = [&](double u) { return b_at(sb0 + lb * b_share(u)); };
    // The columns blended by the mean of A's and B's shares (live 3070's u, its knots those means at the joints).
    auto                mean_share = [&](double u) { return 0.5 * (u + b_share(u)); };
    std::vector<double> mean_nodes;
    for (double u : ua)
      mean_nodes.push_back(mean_share(u));
    // Column m at v (a guide or the straight seam).
    auto column = [&](size_t m, double v) {
      if (cols[m] >= 0)
        return gs[cols[m]].at(v);
      const gp_Pnt p = bot(ua[m]), q = top(ua[m]);
      return gp_Pnt(p.XYZ() * (1 - v) + q.XYZ() * v);
    };
    const bool guided = std::any_of(cols.begin(), cols.end(), [](int c) { return c >= 0; });
    if (!guided)
    {
      TopoDS_Edge e0 = sampled_edge(bot, NU), e1 = sampled_edge(top, NU);
      TopoDS_Face f  = BRepFill::Face(e0, e1);
      if (f.IsNull())
        fail_op("loft: ruled face failed");
      sew.Add(f);
      continue;
    }
    // Gordon: the profiles linearly in v, the columns by Hermite tents in u,
    // less their tensor product.
    // Each column's difference from the profiles' linear blend at each v (the face's own: evaluated once).
    std::vector<std::vector<gp_XYZ>> lift(ua.size(), std::vector<gp_XYZ>(NV + 1));
    for (size_t m = 0; m < ua.size(); ++m)
    {
      const gp_XYZ b = bot(ua[m]).XYZ(), t = top(ua[m]).XYZ();
      for (int j = 0; j <= NV; ++j)
      {
        const double v = (double)j / NV;
        lift[m][j]     = column(m, v).XYZ() - (b * (1 - v) + t * v);
      }
    }
    auto row = [&](double u) {
      std::vector<gp_Pnt>       r(NV + 1);
      const std::vector<double> w = hermite_tents(mean_nodes, mean_share(u));
      const gp_Pnt              pb = bot(u), pt = top(u);
      for (int j = 0; j <= NV; ++j)
      {
        const double v = (double)j / NV;
        gp_XYZ       s = pb.XYZ() * (1 - v) + pt.XYZ() * v;
        for (size_t m = 0; m < ua.size(); ++m)
          s += lift[m][j] * w[m];
        r[j] = gp_Pnt(s);
      }
      return r;
    };
    // Pieces between the columns and every joint of either profile, one surface: one cubic across a joint where
    // the curvature jumps (an arc into a line) leaves its edge tens of µm off the profile (c2-hunting-knife's Loft 1).
    std::vector<double> brk(ua.begin(), ua.end());
    for (double s : joints_a)
    {
      const double u = forward(sa0, s, A.len) / la;
      if (u > 0 && u < 1)
        brk.push_back(u);
    }
    for (double s : joints_b)
    {
      const double o = forward(sb0, s, B.len) / lb;
      if (o <= 0 || o >= 1)
        continue;
      size_t g = 0;
      while (g + 2 < mb.size() && o > mb[g + 1])
        ++g;
      double lo = ma[g], hi = ma[g + 1];
      for (int it = 0; it < 60; ++it)
        (b_share(0.5 * (lo + hi)) < o ? lo : hi) = 0.5 * (lo + hi);
      brk.push_back(0.5 * (lo + hi));
    }
    std::sort(brk.begin(), brk.end());
    std::vector<double> at{0.0};
    for (double u : brk)
      if (u - at.back() > 1e-6 && 1 - u > 1e-6)
        at.push_back(u);
    at.push_back(1.0);
    // Each break's u-derivative, the pieces either side sharing it (C1 there); round the loop at the seam of a face
    // that closes on itself, one-sided at a corner.
    const bool closed = seams.size() == 1;
    // (A step of a quarter of the narrower piece beside it, at most 1e-3: arc length can't place a much smaller one.)
    auto slope = [&](double u, double h) {
      std::vector<gp_Pnt> a, b;
      double              w = 2 * h;
      if (u <= 0 && !closed)
        a = row(0), b = row(h), w = h;
      else if (u >= 1 && !closed)
        a = row(1 - h), b = row(1), w = h;
      else
        a = row(u - h < 0 ? u - h + 1 : u - h), b = row(u + h > 1 ? u + h - 1 : u + h);
      std::vector<gp_Vec> t(a.size());
      for (size_t j = 0; j < a.size(); ++j)
        t[j] = gp_Vec(a[j], b[j]) / w;
      return t;
    };
    std::vector<std::vector<gp_Vec>> slopes;
    for (size_t k2 = 0; k2 < at.size(); ++k2)
    {
      const size_t n2     = at.size() - 1;
      const double before = k2 > 0 ? at[k2] - at[k2 - 1] : at[n2] - at[n2 - 1];
      const double after  = k2 < n2 ? at[k2 + 1] - at[k2] : at[1] - at[0];
      slopes.push_back(slope(at[k2], std::min(1e-3, 0.25 * std::min(before, after))));
    }
    std::vector<occ::handle<Geom_BSplineSurface>> pieces;
    for (size_t k2 = 0; k2 + 1 < at.size(); ++k2)
    {
      // Rows for the wider of A's and B's shares, twice as many until the piece's ends lie on both profiles between
      // them within half the sewing tolerance (an arc of B paired to a short line or spline of A: c2-hunting-knife's
      // handle didn't sew there; to `tol` it took 988 rows of poles, live 66, and every later cut on it slowed).
      occ::handle<Geom_BSplineSurface> piece;
      const KernelTimer                t_rows("loft_guided.faces.rows");
      for (int m = std::max(3, (int)std::ceil(NU * std::max(at[k2 + 1] - at[k2], b_share(at[k2 + 1]) - b_share(at[k2])))); ; m *= 2)
      {
        std::vector<std::vector<gp_Pnt>> grid;
        std::vector<double>              us;
        for (int i = 0; i <= m; ++i)
        {
          us.push_back(at[k2] + (at[k2 + 1] - at[k2]) * i / m);
          grid.push_back(row(us.back()));
        }
        piece = grid_surface_with_ends(grid, us, slopes[k2], slopes[k2 + 1]);
        double off = 0;
        for (int i = 0; i < m; ++i)
        {
          const double u = 0.5 * (us[i] + us[i + 1]);
          off            = std::max({off, piece->Value(u, 0).Distance(bot(u)), piece->Value(u, 1).Distance(top(u))});
        }
        if (off <= 0.5e-6 * (1 + size) || m >= 256)
          break;
      }
      pieces.push_back(piece);
    }
    occ::handle<Geom_BSplineSurface> side = join_u(pieces, at);
    // (Round on itself: OCCT's seam at the first guide, where live's periodic face starts (3070) and its rings' vertices
    // lie: c2-hunting-knife's handle ring after Revolve 1. The guide's vertices stay shown: kept_ends.)
    BRepBuilderAPI_MakeFace mf(side, 1e-7);
    if (!mf.IsDone())
      fail_op("loft: guided face failed");
    sew.Add(mf.Face());
  }
  t_faces.reset();
  // Each guide end a vertex of both profiles.
  std::vector<gp_Pnt> ends_a;
  for (const Guide& g : gs)
  {
    ends_a.push_back(A.at(g.sa));
    splits_b.push_back(b_at(g.sb));
  }
  if (make_solid)
  {
    caps = {split_at(profile_a.shape, ends_a, tol), split_at(profile_b.shape, splits_b, tol)};
    for (const TopoDS_Shape& c : caps)
      sew.Add(c);
  }
  std::optional<KernelTimer> t_sew(std::in_place, "loft_guided.sew");
  sew.Perform();
  TopoDS_Shape r = sew.SewedShape();
  t_sew.reset();
  // One closed shell, or the solid would be open (a piece's end off a profile by more than the tolerance: BRepCheck
  // NotClosed, kept as a solid before): sewn again ten times looser, else refused.
  auto closed = [](const TopoDS_Shape& s) {
    int          n = 0;
    TopoDS_Shell sh;
    for (TopExp_Explorer x(s, TopAbs_SHELL); x.More(); x.Next(), ++n)
      sh = TopoDS::Shell(x.Current());
    return n == 1 && BRepCheck_Shell(sh).Closed() == BRepCheck_NoError;
  };
  if (make_solid && !closed(r))
  {
    BRepBuilderAPI_Sewing again(1e-5 * (1 + size));
    again.Add(r);
    again.Perform();
    r = again.SewedShape();
    if (!closed(r))
      fail_op("Could not create loft with given information. Check profile order, guide/profile intersections or end conditions.");
  }
  for (const TopoDS_Shape& c : caps)
    keep_profile_curves(r, c, 1e-6 * (1 + size));
  // For good: no unify merges the edges either side of a guide end.
  std::vector<gp_Pnt> guide_ends = ends_a;
  guide_ends.insert(guide_ends.end(), splits_b.end() - (long)gs.size(), splits_b.end());
  for (TopExp_Explorer x(r, TopAbs_EDGE); x.More(); x.Next())
    for (TopExp_Explorer v(x.Current(), TopAbs_VERTEX); v.More(); v.Next())
    {
      const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(v.Current()));
      if (std::any_of(guide_ends.begin(), guide_ends.end(), [&](const gp_Pnt& q) { return p.Distance(q) < 1e-6 * (1 + size); }))
        mark_kept_ends(x.Current());
    }
  if (!make_solid)
    return wrap(r);
  TopoDS_Shell shell;
  int          shells = 0;
  for (TopExp_Explorer x(r, TopAbs_SHELL); x.More(); x.Next())
  {
    shell = TopoDS::Shell(x.Current());
    ++shells;
  }
  if (shells != 1)
    fail_op("Could not create loft with given information. Check profile order, guide/profile intersections or end conditions.");
  const KernelTimer        t_solid("loft_guided.solid");
  BRepBuilderAPI_MakeSolid ms(shell);
  if (!ms.IsDone())
    fail_op("loft: solid failed");
  TopoDS_Solid sol = ms.Solid();
  // (Oriented outward, and not flat: no volume integral over the B-spline sides, 20 ms of c2-hunting-knife's Loft 1.)
  Bnd_Box box;
  BRepBndLib::Add(sol, box, false);
  double x0, y0, z0, x1, y1, z1;
  box.Get(x0, y0, z0, x1, y1, z1);
  if (!BRepLib::OrientClosedSolid(sol) || std::min({x1 - x0, y1 - y0, z1 - z0}) <= tol)
    fail_op("Could not create loft with given information. Check profile order, guide/profile intersections or end conditions.");
  return wrap(sol);
}


namespace {

// An open profile by arc length (no wrapping).
struct Open
{
  std::unique_ptr<BRepAdaptor_CompCurve> c;
  TopoDS_Wire                            wire;
  double                                 t0 = 0, t1 = 0, len = 0;

  // A point profile (a loft to a vertex): no curve, every abscissa the point.
  bool   point = false;
  gp_Pnt apex;
  // Abscissae proportional to the curve's parameter, not its arc length (through more than two profiles, as live).
  bool by_param = false;

  explicit Open(const TopoDS_Wire& w) : c(new BRepAdaptor_CompCurve(w)), wire(w)
  {
    t0  = c->FirstParameter();
    t1  = c->LastParameter();
    len = GCPnts_AbscissaPoint::Length(*c, t0, t1);
  }
  explicit Open(const gp_Pnt& p) : point(true), apex(p) {}
  // `s` as a profile: a vertex is a point profile.
  static Open of(const TopoDS_Shape& s)
  {
    if (s.ShapeType() == TopAbs_VERTEX)
      return Open(BRep_Tool::Pnt(TopoDS::Vertex(s)));
    return Open(outer_wire(s));
  }
  gp_Pnt at(double s) const
  {
    if (point)
      return apex;
    s = std::clamp(s, 0.0, len);
    if (s < 1e-15)
      return c->Value(t0);
    if (s > len - 1e-15)
      return c->Value(t1);
    if (by_param)
      return c->Value(t0 + (t1 - t0) * s / len);
    GCPnts_AbscissaPoint ap(*c, s, t0);
    return c->Value(ap.IsDone() ? ap.Parameter() : t0 + (t1 - t0) * s / len);
  }
  double abscissa_of(double t) const
  {
    if (by_param)
      return len * (std::clamp(t, t0, t1) - t0) / (t1 - t0);
    return GCPnts_AbscissaPoint::Length(*c, t0, std::clamp(t, t0, t1));
  }
  double abscissa(const gp_Pnt& p) const
  {
    if (point)
      return 0;
    Extrema_ExtPC ext(p, *c);
    double        best = 1e300, t = t0;
    if (ext.IsDone())
      for (int i = 1; i <= ext.NbExt(); ++i)
        if (ext.SquareDistance(i) < best)
        {
          best = ext.SquareDistance(i);
          t    = ext.Point(i).Parameter();
        }
    for (double e : {t0, t1})
      if (p.SquareDistance(c->Value(e)) < best)
      {
        best = p.SquareDistance(c->Value(e));
        t    = e;
      }
    return abscissa_of(t);
  }
  // Arc lengths of the interior corners (tangent breaks between edges).
  std::vector<double> corners() const
  {
    if (point)
      return {};
    std::vector<double>      out;
    std::vector<TopoDS_Edge> edges;
    for (BRepTools_WireExplorer x(wire); x.More(); x.Next())
      edges.push_back(x.Current());
    for (size_t i = 0; i + 1 < edges.size(); ++i)
    {
      BRepAdaptor_Curve ca(edges[i]), cb(edges[i + 1]);
      const bool        ra = edges[i].Orientation() == TopAbs_REVERSED, rb = edges[i + 1].Orientation() == TopAbs_REVERSED;
      gp_Pnt            pa, pb;
      gp_Vec            da, db;
      ca.D1(ra ? ca.FirstParameter() : ca.LastParameter(), pa, da);
      cb.D1(rb ? cb.LastParameter() : cb.FirstParameter(), pb, db);
      if (ra)
        da.Reverse();
      if (rb)
        db.Reverse();
      if (da.Magnitude() > 1e-12 && db.Magnitude() > 1e-12 && da.Angle(db) > SMOOTH_JOINT)
        out.push_back(abscissa(pa));
    }
    return out;
  }
};

// The face through a grid, as a plane or cylinder (Parasolid's simplest
// surface) when the grid lies on one.
// The face on the plane, cylinder, ... its surface lies on within `tol`, bounded by its edges; else as it is.
TopoDS_Face as_analytic(const TopoDS_Face& f, double tol)
{
  occ::handle<Geom_Surface> s(BRep_Tool::Surface(f));
  GeomConvert_SurfToAnaSurf conv(s);
  occ::handle<Geom_Surface> ana(conv.ConvertToAnalytical(tol));
  if (ana.IsNull() || conv.Gap() > tol)
    return f;
  BRepLib::BuildCurves3d(f);
  GProp_GProps a;
  BRepGProp::SurfaceProperties(f, a);
  // The wire both ways round: the one bounding the face (a surface facing the other way runs it reversed).
  for (const TopoDS_Wire& w : {BRepTools::OuterWire(f), TopoDS::Wire(BRepTools::OuterWire(f).Reversed())})
  {
    BRepBuilderAPI_MakeFace mf(ana, w, true);
    if (!mf.IsDone())
      continue;
    ShapeFix_Face fix(mf.Face());
    fix.Perform();
    const TopoDS_Face out = fix.Face();
    GProp_GProps      b;
    BRepGProp::SurfaceProperties(out, b);
    if (std::abs(a.Mass() - b.Mass()) <= 1e-6 * std::abs(a.Mass()))
      return out;
  }
  return f;
}

TopoDS_Face simplest_face(const std::vector<std::vector<gp_Pnt>>& g, double tol)
{
  return as_analytic(grid_face(g), tol);
}

// `c` on [0, len] run through at speed `a` times its own at its start, `b` at its end, eased between by a cubic map of
// its parameter, as C1 cubic Hermite spans (halved until within `tol`; a straight piece is one span, exact).
occ::handle<Geom_BSplineCurve> eased(const occ::handle<Geom_BSplineCurve>& c, double a, double b, double tol)
{
  const double t0 = c->FirstParameter(), t1 = c->LastParameter(), len = t1 - t0;
  // The map w in [0, 1] to the curve's parameter: Hermite, slopes a and b, its derivative.
  auto phi  = [&](double w) { return t0 + len * ((-2 * w + 3) * w * w + (w * w - 2 * w + 1) * w * a + (w * w - w) * w * b); };
  auto dphi = [&](double w) { return len * ((-6 * w + 6) * w + (3 * w * w - 4 * w + 1) * a + (3 * w * w - 2 * w) * b); };
  for (int n = 1; n <= 64; n *= 2)
  {
    NCollection_Array1<double> knots(1, n + 1);
    std::vector<gp_Pnt>        at(n + 1);
    std::vector<gp_Vec>        d(n + 1);
    for (int k = 0; k <= n; ++k)
    {
      const double w = (double)k / n;
      gp_Pnt       p;
      gp_Vec       v;
      c->D1(phi(w), p, v);
      at[k] = p;
      d[k]  = v * (dphi(w) / n);
      knots(k + 1) = t0 + len * w;
    }
    // Each span's Bézier poles from its ends' points and derivatives (C0 form: interior knots 3 times).
    NCollection_Array1<gp_Pnt> all(1, 3 * n + 1);
    for (int k = 0; k < n; ++k)
    {
      all(3 * k + 1) = at[k];
      all(3 * k + 2) = at[k].Translated(d[k] / 3);
      all(3 * k + 3) = at[k + 1].Translated(-d[k + 1] / 3);
    }
    all(3 * n + 1) = at[n];
    NCollection_Array1<int> m2(1, n + 1);
    for (int k = 1; k <= n + 1; ++k)
      m2(k) = k == 1 || k == n + 1 ? 4 : 3;
    occ::handle<Geom_BSplineCurve> out = new Geom_BSplineCurve(all, knots, m2, 3);
    double worst = 0;
    for (int k = 0; k < n && worst <= tol; ++k)
      for (const double f : {0.25, 0.5, 0.75})
      {
        const double w = (k + f) / n;
        worst          = std::max(worst, out->Value(t0 + len * w).Distance(c->Value(phi(w))));
      }
    if (worst <= tol || n == 64)
    {
      // C1 at the joints: knots of multiplicity 2 where the spans' derivatives agree.
      for (int k = 2; k <= n; ++k)
        out->RemoveKnot(k, 2, tol);
      return out;
    }
  }
  return c;
}

// A chain's pieces (each over its share of the chain's length) C1 at their joints as live's loft guides are: runs of
// lines on one straight line one piece; at each joint both pieces run at the geometric mean of their speeds there, the
// chain's ends at their own, each piece's parameter eased between by a cubic map (live 2620 / 3027: c2-hunting-knife's
// Loft 3 guide, a cubic, a line and an arc: 134.68 at the cubic's end where it runs at 152.13 and the line at 119.24;
// live 2291: the line's 1.13 there). A piece whose map would turn back stays as it is.
void ease_chain(std::vector<occ::handle<Geom_BSplineCurve>>& pieces, std::vector<bool>& lines, double tol)
{
  // Runs of lines on one straight line: one straight piece over their length.
  for (size_t i = 0; i < pieces.size(); ++i)
  {
    if (!lines[i])
      continue;
    const gp_Dir dir(gp_Vec(pieces[i]->StartPoint(), pieces[i]->EndPoint()));
    size_t       j = i + 1;
    while (j < pieces.size() && lines[j] && gp_Dir(gp_Vec(pieces[j]->StartPoint(), pieces[j]->EndPoint())).Angle(dir) < 1e-9)
      ++j;
    if (j == i + 1)
      continue;
    const gp_Pnt               q0 = pieces[i]->StartPoint(), q1 = pieces[j - 1]->EndPoint();
    NCollection_Array1<gp_Pnt> poles(1, 2);
    NCollection_Array1<double> knots(1, 2);
    NCollection_Array1<int>    mults(1, 2);
    poles(1) = q0;
    poles(2) = q1;
    knots(1) = 0;
    knots(2) = q0.Distance(q1);
    mults(1) = mults(2) = 2;
    pieces[i] = new Geom_BSplineCurve(poles, knots, mults, 1);
    pieces.erase(pieces.begin() + (long)i + 1, pieces.begin() + (long)j);
    lines.erase(lines.begin() + (long)i + 1, lines.begin() + (long)j);
  }
  const size_t        n = pieces.size();
  std::vector<double> s0(n), s1(n);
  for (size_t i = 0; i < n; ++i)
  {
    s0[i] = pieces[i]->DN(pieces[i]->FirstParameter(), 1).Magnitude();
    s1[i] = pieces[i]->DN(pieces[i]->LastParameter(), 1).Magnitude();
  }
  for (size_t i = 0; i < n; ++i)
  {
    const double a = i > 0 && s1[i - 1] > 0 && s0[i] > 0 ? std::sqrt(s1[i - 1] * s0[i]) / s0[i] : 1.0;
    const double b = i + 1 < n && s1[i] > 0 && s0[i + 1] > 0 ? std::sqrt(s1[i] * s0[i + 1]) / s1[i] : 1.0;
    if ((std::abs(a - 1) <= 1e-9 && std::abs(b - 1) <= 1e-9) || a * a + b * b > 9)
      continue;
    pieces[i] = eased(pieces[i], a, b, tol);
  }
}

// A chain of edges as one B-spline, from `from` (an end of the chain): C0 at its joints.
occ::handle<Geom_BSplineCurve> chain_bspline(const TopoDS_Shape& chain, const gp_Pnt& from, double tol, bool by_arc)
{
  ShapeList edges;
  for (TopExp_Explorer x(chain, TopAbs_EDGE); x.More(); x.Next())
    edges.Append(x.Current());
  BRepBuilderAPI_MakeWire mw;
  mw.Add(edges);
  if (!mw.IsDone())
    return nullptr;
  std::vector<occ::handle<Geom_BSplineCurve>> pieces;
  std::vector<bool>                           lines;
  for (BRepTools_WireExplorer x(mw.Wire()); x.More(); x.Next())
  {
    double                        f, l;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(x.Current(), f, l);
    if (c.IsNull())
      return nullptr;
    const occ::handle<Geom_Curve> trimmed(new Geom_TrimmedCurve(c, f, l));
    occ::handle<Geom_BSplineCurve> piece = GeomConvert::CurveToBSplineCurve(trimmed);
    // A polynomial spline keeps its own parameter, stretched over its length (live 2380 (14): a fit spline profile
    // stays one cubic, paired with the line by its parameter); arcs and other rational curves by arc length (live
    // 2319 (3): an arc profile paired by angle).
    const bool polynomial = !piece->IsRational() && !c->IsKind(STANDARD_TYPE(Geom_Line)) && !c->IsKind(STANDARD_TYPE(Geom_Conic))
                            && !(c->IsKind(STANDARD_TYPE(Geom_TrimmedCurve)) && occ::down_cast<Geom_TrimmedCurve>(c)->BasisCurve()->IsKind(STANDARD_TYPE(Geom_Conic)));
    if (by_arc && !polynomial && !c->IsKind(STANDARD_TYPE(Geom_Line)))
    {
      const occ::handle<Adaptor3d_Curve> ad(new GeomAdaptor_Curve(trimmed));
      Approx_CurvilinearParameter        arc(ad, tol, GeomAbs_C2, 14, 100);
      if (!arc.IsDone() || !arc.HasResult())
        return nullptr;
      piece = arc.Curve3d();
    }
    // Its share of the chain: the range its length (Approx_CurvilinearParameter's is [0, 1]).
    if (by_arc && !c->IsKind(STANDARD_TYPE(Geom_Line)))
    {
      NCollection_Array1<double> knots = piece->Knots();
      BSplCLib::Reparametrize(0.0, GCPnts_AbscissaPoint::Length(GeomAdaptor_Curve(trimmed)), knots);
      piece->SetKnots(knots);
    }
    if (x.Current().Orientation() == TopAbs_REVERSED)
      piece->Reverse();
    pieces.push_back(piece);
    lines.push_back(c->IsKind(STANDARD_TYPE(Geom_Line)));
  }
  if (by_arc)
    ease_chain(pieces, lines, tol);
  // Each piece keeps its share of the chain's length (OCCT's ratio would rescale the next piece to the speed at the
  // joint: c2-hunting-knife's guide, live 2291 has its line and arc at their length shares after the spline).
  GeomConvert_CompCurveToBSplineCurve join;
  for (const occ::handle<Geom_BSplineCurve>& piece : pieces)
    if (!join.Add(piece, tol, false, false))
      return nullptr;
  occ::handle<Geom_BSplineCurve> out = join.BSplineCurve();
  if (out.IsNull())
    return nullptr;
  if (out->StartPoint().Distance(from) > out->EndPoint().Distance(from))
    out->Reverse();
  return out;
}

// `c` on [0, 1] with at least degree `d` and every knot of `with` (as many times).
void match_curve(const occ::handle<Geom_BSplineCurve>& c, int d, const occ::handle<Geom_BSplineCurve>& with)
{
  if (c->Degree() < d)
    c->IncreaseDegree(d);
  if (!with.IsNull())
    c->InsertKnots(with->Knots(), with->Multiplicities(), 1e-12, false);
}

// `x` and `y` on one degree (at least `d`) and one knot vector.
void match_curves(const occ::handle<Geom_BSplineCurve>& x, const occ::handle<Geom_BSplineCurve>& y, int d)
{
  d = std::max({d, x->Degree(), y->Degree()});
  match_curve(x, d, nullptr);
  match_curve(y, d, nullptr);
  match_curve(x, d, y);
  match_curve(y, d, x);
}

occ::handle<Geom_BSplineCurve> unit_range(const occ::handle<Geom_BSplineCurve>& c, bool reversed)
{
  occ::handle<Geom_BSplineCurve> out = occ::down_cast<Geom_BSplineCurve>(c->Copy());
  if (reversed)
    out->Reverse();
  NCollection_Array1<double> knots = out->Knots();
  BSplCLib::Reparametrize(0.0, 1.0, knots);
  out->SetKnots(knots);
  return out;
}

// The coefficients of a polynomial on [0, 1] (its Bezier coefficients `bezier`) in `basis`'s B-spline basis.
std::vector<double> in_basis(const std::vector<double>& bezier, const occ::handle<Geom_BSplineCurve>& basis)
{
  const int                  n = (int)bezier.size();
  NCollection_Array1<gp_Pnt> poles(1, n);
  NCollection_Array1<double> knots(1, 2);
  NCollection_Array1<int>    mults(1, 2);
  for (int i = 0; i < n; ++i)
    poles(i + 1) = gp_Pnt(0, 0, bezier[i]);
  knots(1) = 0;
  knots(2) = 1;
  mults(1) = mults(2) = n;
  occ::handle<Geom_BSplineCurve> f(new Geom_BSplineCurve(poles, knots, mults, n - 1));
  match_curve(f, basis->Degree(), basis);
  std::vector<double> out;
  for (int i = 1; i <= f->NbPoles(); ++i)
    out.push_back(f->Pole(i).Z());
  return out;
}

// Parasolid's patch between two profiles `a`, `b` (u) and two guides `g0`, `g1` along its sides (v), all running from
// the u = 0 / v = 0 corner: the profiles blended linearly in v, each guide's difference from that blend faded across
// by the cubic Hermite (1 - 3u² + 2u³ from g0): live 2291's face to 0.01 µm. Exact as a B-spline; null for rational
// curves.
occ::handle<Geom_BSplineSurface> hermite_coons(occ::handle<Geom_BSplineCurve> a, occ::handle<Geom_BSplineCurve> b,
                                               occ::handle<Geom_BSplineCurve> g0, occ::handle<Geom_BSplineCurve> g1)
{
  if (a->IsRational() || b->IsRational() || g0->IsRational() || g1->IsRational())
    return nullptr;
  match_curves(a, b, 3);
  match_curves(g0, g1, 1);
  const int du = a->Degree(), dv = g0->Degree();
  if (a->NbPoles() != b->NbPoles() || g0->NbPoles() != g1->NbPoles())
    return nullptr;
  const std::vector<double> h0 = in_basis({1, 1, 0, 0}, a), h1 = in_basis({0, 0, 1, 1}, a), lv = in_basis({0, 1}, g0);
  const int                 nu = a->NbPoles(), nv = g0->NbPoles();
  if ((int)h0.size() != nu || (int)lv.size() != nv)
    return nullptr;
  const gp_XYZ a0 = a->StartPoint().XYZ(), a1 = a->EndPoint().XYZ(), b0 = b->StartPoint().XYZ(), b1 = b->EndPoint().XYZ();
  NCollection_Array2<gp_Pnt> poles(1, nu, 1, nv);
  for (int i = 1; i <= nu; ++i)
    for (int j = 1; j <= nv; ++j)
    {
      const double w  = lv[j - 1];
      const gp_XYZ d0 = g0->Pole(j).XYZ() - a0 * (1 - w) - b0 * w, d1 = g1->Pole(j).XYZ() - a1 * (1 - w) - b1 * w;
      poles(i, j)     = gp_Pnt(a->Pole(i).XYZ() * (1 - w) + b->Pole(i).XYZ() * w + d0 * h0[i - 1] + d1 * h1[i - 1]);
    }
  return new Geom_BSplineSurface(poles, a->Knots(), g0->Knots(), a->Multiplicities(), g0->Multiplicities(), du, dv);
}

// A face filling the loop of `bounds` (four chains of edges in order round it: the profiles' and guides' own edges)
// with a Coons patch, which runs through them exactly, bounded by those edges (Parasolid's face shares them with the
// faces round a hole, c2-hunting-knife's Loft 3). Null where they don't close or the face isn't valid.
TopoDS_Face face_on_edges(const std::vector<TopoDS_Shape>& bounds, double tol)
{
  ShapeList edges;
  for (const TopoDS_Shape& b : bounds)
    for (TopExp_Explorer x(b, TopAbs_EDGE); x.More(); x.Next())
      edges.Append(x.Current());
  BRepBuilderAPI_MakeWire mw;
  mw.Add(edges);
  if (bounds.size() != 4 || !mw.IsDone() || !mw.Wire().Closed())
    return TopoDS_Face();
  // Round the loop: each chain from where the one before ended.
  std::vector<occ::handle<Geom_BSplineCurve>> sides;
  TopoDS_Vertex first, last;
  TopExp::Vertices(TopoDS::Wire(BRepBuilderAPI_MakeWire(TopoDS::Edge(TopExp_Explorer(bounds[0], TopAbs_EDGE).Current())).Wire()), first, last);
  gp_Pnt at = BRep_Tool::Pnt(first);
  for (const TopoDS_Shape& b : bounds)
  {
    occ::handle<Geom_BSplineCurve> c = chain_bspline(b, at, tol, !std::getenv("NOSHAPE_LOFT_OWN"));
    if (c.IsNull())
      return TopoDS_Face();
    sides.push_back(c);
    at = c->EndPoint();
  }
  // Round the loop: profile a, guide g1, profile b back, guide g0 back.
  const occ::handle<Geom_BSplineSurface> surface = hermite_coons(unit_range(sides[0], false), unit_range(sides[2], true),
                                                                 unit_range(sides[3], true), unit_range(sides[1], false));
  if (surface.IsNull())
    return TopoDS_Face();
  // The loop round the surface's normal (the other way it bounds the rest of the plane).
  auto on = [&](const TopoDS_Wire& w) {
    BRepBuilderAPI_MakeFace mf(occ::handle<Geom_Surface>(surface), w, true);
    if (!mf.IsDone())
      return TopoDS_Face();
    ShapeFix_Face fix(mf.Face());
    fix.Perform();
    TopoDS_Face f = fix.Face();
    BRepLib::SameParameter(f, tol, true);
    return f;
  };
  TopoDS_Face out = on(mw.Wire());
  if (out.IsNull())
    return out;
  GProp_GProps first_area;
  BRepGProp::SurfaceProperties(out, first_area);
  if (first_area.Mass() < 0)
    out = on(TopoDS::Wire(mw.Wire().Reversed()));
  if (out.IsNull())
    return out;
  double worst = 0;
  for (TopExp_Explorer x(out, TopAbs_EDGE); x.More(); x.Next())
    worst = std::max(worst, BRep_Tool::Tolerance(TopoDS::Edge(x.Current())));
  if (worst > 1e3 * tol || !BRepCheck_Analyzer(out).IsValid())
    return TopoDS_Face();
  return out;
}

} // namespace

// The weights at each of `vs` of a cubic interpolation through values at `nodes` (OCCT's interpolation of each unit
// vector in x: the interpolant is linear in the values; y the node, as OCCT refuses coincident points).
// Each profile's weight at each v of a natural cubic spline through the profiles at `nodes` (its second derivative zero at
// the first and last): as Parasolid's surface across them (live2 q-pipe-loft-1119: c2-surf-workshop-pipe's Loft, sections
// deeper in its long span than an interpolating cubic's).
std::vector<std::vector<double>> through_weights(const std::vector<double>& nodes, const std::vector<double>& vs)
{
  const int                        n = (int)nodes.size();
  std::vector<std::vector<double>> w(vs.size(), std::vector<double>(n, 0.0));
  for (int q = 0; q < n; ++q)
  {
    // The spline through 1 at node q, 0 elsewhere: its second derivatives M by the tridiagonal system.
    std::vector<double> y(n, 0.0), M(n, 0.0);
    y[q] = 1.0;
    if (n > 2)
    {
      std::vector<double> a(n, 0.0), b(n, 0.0), c(n, 0.0), d(n, 0.0);
      for (int i = 1; i + 1 < n; ++i)
      {
        const double h0 = nodes[i] - nodes[i - 1], h1 = nodes[i + 1] - nodes[i];
        a[i] = h0 / 6, b[i] = (h0 + h1) / 3, c[i] = h1 / 6;
        d[i] = (y[i + 1] - y[i]) / h1 - (y[i] - y[i - 1]) / h0;
      }
      for (int i = 2; i + 1 < n; ++i)
      {
        const double m = a[i] / b[i - 1];
        b[i] -= m * c[i - 1];
        d[i] -= m * d[i - 1];
      }
      for (int i = n - 2; i >= 1; --i)
        M[i] = (d[i] - (i + 2 < n ? c[i] * M[i + 1] : 0.0)) / b[i];
    }
    for (size_t j = 0; j < vs.size(); ++j)
    {
      const double v = vs[j];
      int          i = 0;
      while (i + 2 < n && v > nodes[i + 1])
        ++i;
      const double h = nodes[i + 1] - nodes[i], A = nodes[i + 1] - v, B = v - nodes[i];
      w[j][q] = M[i] * A * A * A / (6 * h) + M[i + 1] * B * B * B / (6 * h) + (y[i] / h - M[i] * h / 6) * A + (y[i + 1] / h - M[i + 1] * h / 6) * B;
    }
  }
  return w;
}

std::unique_ptr<OcShape> loft_guided_open(const OcShapeList& profiles, const OcShapeList& guides, bool trim_guides,
                                          bool trim_profiles)
{
  std::vector<TopoDS_Shape> ps(profiles.items.begin(), profiles.items.end());
  if (ps.size() < 2)
    fail_op("Select at least two profiles, a closed guide curve or a closed path.");
  for (const TopoDS_Shape& s : ps)
    if (s.IsNull())
      fail_invalid("loft: a profile is null");
  // (A point profile last: the loft runs from the curves to it.)
  if (ps.front().ShapeType() == TopAbs_VERTEX)
    std::reverse(ps.begin(), ps.end());
  std::vector<Open> P;
  for (const TopoDS_Shape& s : ps)
    P.push_back(Open::of(s));
  const size_t n = P.size();
  for (Open& o : P)
    o.by_param = n > 2 && !o.point;
  if (P[0].point)
    fail_op("Select at least two profiles, a closed guide curve or a closed path.");
  for (size_t k = 1; k + 1 < n; ++k)
    if (P[k].point)
      fail_op("Point profiles can only exist as first or last profiles.");
  const Open& A    = P[0];
  double      size = 0;
  for (const Open& o : P)
    size = std::max(size, o.len);
  const double tol = 1e-7 * (1 + size);
  // Each profile runs the way the one before it does (its ends nearest that one's).
  std::vector<bool> flip(n, false);
  auto              p_at = [&](size_t k, double s) { return P[k].at(flip[k] ? P[k].len - s : s); };
  for (size_t k = 1; k < n; ++k)
  {
    const Open&  Q = P[k];
    const gp_Pnt e0 = p_at(k - 1, 0), e1 = p_at(k - 1, P[k - 1].len);
    flip[k] = e0.Distance(Q.at(0)) + e1.Distance(Q.at(Q.len)) > e0.Distance(Q.at(Q.len)) + e1.Distance(Q.at(0));
  }
  // Each guide from the first profile to the last: its parameters on each, its arc length between the ends and how
  // far it runs on past each.
  struct G
  {
    std::unique_ptr<Adaptor3d_Curve> c;
    double                           ta, tb, len, before, after;
    std::vector<double>              t, s, cum; // per profile: guide parameter, abscissa on the profile, arc from ta
    // Arc lengths from ta to the parameter range's ends.
    double up, down;
    TopoDS_Shape shape; // the guide as selected
    // The common v of each profile (set once all guides are known).
    const std::vector<double>* vs = nullptr;
    // Arc length along the curve at parameters `knot` (a dense table: GCPnts_AbscissaPoint fails across a composite
    // guide's kink, c2-hunting-knife's Loft 3, and the point went off the guide).
    std::vector<double> knot, arc;
    void tabulate()
    {
      const double lo = c->FirstParameter(), hi = c->LastParameter();
      const int    m  = 4000;
      knot.resize(m + 1);
      arc.resize(m + 1);
      gp_Pnt prev = c->Value(lo);
      for (int i = 0; i <= m; ++i)
      {
        knot[i]       = lo + (hi - lo) * i / m;
        const gp_Pnt p = c->Value(knot[i]);
        arc[i]        = i ? arc[i - 1] + p.Distance(prev) : 0.0;
        prev          = p;
      }
    }
    double arc_at(double x) const
    {
      const size_t i = std::min<size_t>(std::upper_bound(knot.begin(), knot.end(), x) - knot.begin(), knot.size() - 1);
      if (i == 0)
        return 0;
      return arc[i - 1] + (arc[i] - arc[i - 1]) * (x - knot[i - 1]) / std::max(knot[i] - knot[i - 1], 1e-300);
    }
    double param_at_arc(double a) const
    {
      const size_t i = std::min<size_t>(std::max<size_t>(std::lower_bound(arc.begin(), arc.end(), a) - arc.begin(), 1), arc.size() - 1);
      const double f = (a - arc[i - 1]) / std::max(arc[i] - arc[i - 1], 1e-300);
      return knot[i - 1] + (knot[i] - knot[i - 1]) * std::clamp(f, 0.0, 1.0);
    }
    // Arc length from ta at v: the profiles at their common v, evenly between them.
    double arc_of(double v) const
    {
      if (!vs || vs->size() == 2 || v <= 0 || v >= 1)
        return v * len;
      size_t k = 0;
      while (k + 2 < vs->size() && v > (*vs)[k + 1])
        ++k;
      return cum[k] + (cum[k + 1] - cum[k]) * (v - (*vs)[k]) / ((*vs)[k + 1] - (*vs)[k]);
    }
    gp_Pnt at(double v) const
    {
      // Signed arc length from ta along increasing parameter.
      const double a     = (tb > ta ? 1.0 : -1.0) * arc_of(v);
      const double avail = a >= 0 ? up : down;
      if (std::abs(a) <= avail + 1e-15)
        return c->Value(param_at_arc(arc_at(ta) + a));
      // Past the guide's end: on along its end tangent.
      gp_Pnt p;
      gp_Vec d;
      c->D1(a >= 0 ? c->LastParameter() : c->FirstParameter(), p, d);
      return p.Translated(d.Normalized() * ((a >= 0 ? 1.0 : -1.0) * (std::abs(a) - avail)));
    }
  };
  std::vector<G> gs;
  for (const TopoDS_Shape& gx : guides.items)
  {
    G g;
    g.c     = guide_curve(gx);
    g.shape = gx;
    g.tabulate();
    g.t.assign(n, 0.0);
    g.s.assign(n, 0.0);
    g.cum.assign(n, 0.0);
    for (size_t k = 0; k < n; ++k)
    {
      const Open& Q = P[k];
      gp_Pnt      at;
      double      d = 1e300;
      if (!Q.point)
        d = meet(*g.c, *Q.c, Q.wire, tol, g.t[k], at);
      else
      {
        // (To a point profile: where the guide passes through it.)
        for (const double e : {g.c->FirstParameter(), g.c->LastParameter()})
          if (g.c->Value(e).Distance(Q.apex) < d)
            d = g.c->Value(e).Distance(Q.apex), g.t[k] = e;
        Extrema_ExtPC ext(Q.apex, *g.c);
        if (ext.IsDone())
          for (int i = 1; i <= ext.NbExt(); ++i)
            if (std::sqrt(ext.SquareDistance(i)) < d)
              d = std::sqrt(ext.SquareDistance(i)), g.t[k] = ext.Point(i).Parameter();
        at = Q.apex;
      }
      if (d > tol)
        fail_op("All guides should intersect each profile boundary.");
      g.s[k] = flip[k] ? Q.len - Q.abscissa(at) : Q.abscissa(at);
      // A guide through a profile's end runs along the face's side: its position exactly that end (a projection on
      // a composite curve lands a hair inside).
      if (at.Distance(p_at(k, 0)) < tol)
        g.s[k] = 0;
      else if (at.Distance(p_at(k, Q.len)) < tol)
        g.s[k] = Q.len;
    }
    g.ta = g.t.front();
    g.tb = g.t.back();
    if (std::abs(g.ta - g.tb) < 1e-12)
      fail_op("All guides should intersect each profile boundary.");
    // The profiles in order along the guide.
    for (size_t k = 1; k < n; ++k)
      if ((g.t[k] - g.t[k - 1]) * (g.tb - g.ta) <= 0)
        fail_op("Could not create loft with given information. Check profile order, guide/profile intersections or end conditions.");
    const double lo = g.c->FirstParameter(), hi = g.c->LastParameter();
    g.len    = GCPnts_AbscissaPoint::Length(*g.c, std::min(g.ta, g.tb), std::max(g.ta, g.tb));
    g.before = GCPnts_AbscissaPoint::Length(*g.c, g.ta < g.tb ? lo : g.ta, g.ta < g.tb ? g.ta : hi);
    g.after  = GCPnts_AbscissaPoint::Length(*g.c, g.ta < g.tb ? g.tb : lo, g.ta < g.tb ? hi : g.tb);
    g.up     = GCPnts_AbscissaPoint::Length(*g.c, g.ta, hi);
    g.down   = GCPnts_AbscissaPoint::Length(*g.c, lo, g.ta);
    for (size_t k = 1; k + 1 < n; ++k)
      g.cum[k] = GCPnts_AbscissaPoint::Length(*g.c, std::min(g.ta, g.t[k]), std::max(g.ta, g.t[k]));
    g.cum[n - 1] = g.len;
    gs.push_back(std::move(g));
  }
  if (gs.empty())
    fail_invalid("loft: no guides");
  // Each profile's v: its mean share of the guides' lengths.
  std::vector<double> vs(n, 0.0);
  vs[n - 1] = 1;
  for (size_t k = 1; k + 1 < n; ++k)
  {
    for (const G& g : gs)
      vs[k] += g.cum[k] / g.len;
    vs[k] /= (double)gs.size();
  }
  for (G& g : gs)
    g.vs = &vs;
  std::sort(gs.begin(), gs.end(), [](const G& x, const G& y) { return x.s[0] < y.s[0]; });
  // The v range: the profiles, or on to the guides' ends.
  double vlo = 0, vhi = 1;
  if (!trim_guides)
    for (const G& g : gs)
    {
      vlo = std::min(vlo, -g.before / g.len);
      vhi = std::max(vhi, 1 + g.after / g.len);
    }
  // The u range: the whole profiles, or between the outermost guides. Seams: the ends and the corners, as many on
  // each profile as on the first.
  std::vector<std::vector<double>> seams(n);
  for (size_t k = 0; k < n; ++k)
  {
    double lo = 0, hi = P[k].len;
    if (trim_profiles && gs.size() > 1)
    {
      lo = std::min(gs.front().s[k], gs.back().s[k]);
      hi = std::max(gs.front().s[k], gs.back().s[k]);
      if (k == 0)
        lo = gs.front().s[0], hi = gs.back().s[0];
    }
    std::vector<double> c = P[k].corners();
    if (flip[k])
    {
      for (double& x : c)
        x = P[k].len - x;
      std::sort(c.begin(), c.end());
    }
    c.erase(std::remove_if(c.begin(), c.end(), [&](double x) { return x <= lo + tol || x >= hi - tol; }), c.end());
    if (!P[k].point && k > 0 && c.size() != seams[0].size() - 2)
      fail_op("Loft guides need profiles with as many corners yet");
    seams[k].push_back(lo);
    seams[k].insert(seams[k].end(), c.begin(), c.end());
    seams[k].push_back(hi);
  }
  const size_t spans = seams[0].size() - 1;
  for (size_t k = 1; k < n; ++k)
    if (P[k].point)
      seams[k].assign(spans + 1, 0.0);
  BRepBuilderAPI_Sewing sew(1e-6 * (1 + size));
  const int             NU = 48, NV = 24;
  std::vector<double>   grid_v(NV + 1);
  for (int j = 0; j <= NV; ++j)
    grid_v[j] = vlo + (vhi - vlo) * j / NV;
  const std::vector<std::vector<double>> wv = n > 2 ? through_weights(vs, grid_v) : std::vector<std::vector<double>>();
  for (size_t k = 0; k < spans; ++k)
  {
    std::vector<double> lens(n);
    for (size_t q = 0; q < n; ++q)
      lens[q] = seams[q][k + 1] - seams[q][k];
    const double la = lens[0];
    // The guides' columns: u on the first profile, and each profile's share of its span there.
    std::vector<double>              ua{0.0};
    std::vector<std::vector<double>> uq{std::vector<double>(n, 0.0)};
    std::vector<int>                 cols{-1};
    auto                             shares = [&](const G& g) {
      std::vector<double> w(n);
      for (size_t q = 0; q < n; ++q)
        w[q] = lens[q] > 0 ? (g.s[q] - seams[q][k]) / lens[q] : 0.0;
      return w;
    };
    for (size_t i = 0; i < gs.size(); ++i)
      if (gs[i].s[0] > seams[0][k] - tol && gs[i].s[0] < seams[0][k + 1] + tol)
      {
        const double u = (gs[i].s[0] - seams[0][k]) / la;
        if (u < 1e-9)
          cols[0] = (int)i;
        else if (u > 1 - 1e-9)
          continue;
        else
        {
          ua.push_back(u);
          uq.push_back(shares(gs[i]));
          cols.push_back((int)i);
        }
      }
    ua.push_back(1.0);
    uq.push_back(std::vector<double>(n, 1.0));
    cols.push_back(-1);
    for (size_t i = 0; i < gs.size(); ++i)
      if (std::abs((gs[i].s[0] - seams[0][k]) / la - 1) < 1e-9)
        cols.back() = (int)i;
    // Profile q at u: the first profile by its arc length, the others piecewise between the guides.
    auto prof = [&](size_t q, double u) {
      if (q == 0)
        return A.at(seams[0][k] + la * u);
      size_t s = 0;
      while (s + 2 < ua.size() && u > ua[s + 1])
        ++s;
      const double t = (u - ua[s]) / (ua[s + 1] - ua[s]);
      return p_at(q, seams[q][k] + lens[q] * (uq[s][q] + (uq[s + 1][q] - uq[s][q]) * t));
    };
    // Through the profiles at u, at grid row j: straight between two, a cubic through more.
    auto across = [&](const std::vector<gp_Pnt>& at, int j) {
      if (n == 2)
      {
        const double v = grid_v[j];
        return at[0].XYZ() * (1 - v) + at[1].XYZ() * v;
      }
      gp_XYZ s(0, 0, 0);
      for (size_t q = 0; q < n; ++q)
        s += at[q].XYZ() * wv[j][q];
      return s;
    };
    auto profiles_at = [&](double u) {
      std::vector<gp_Pnt> at(n);
      for (size_t q = 0; q < n; ++q)
        at[q] = prof(q, u);
      return at;
    };
    // One patch between two guides along its sides (c2-hunting-knife's Loft 3, closing a hole): bounded by the
    // profiles' and guides' own edges, as Parasolid's face (it shares them with the faces around the hole).
    if (n == 2 && spans == 1 && cols.front() >= 0 && cols.back() >= 0 && vlo == 0 && vhi == 1 && !trim_profiles && !P[1].point)
    {
      const TopoDS_Face exact = face_on_edges({A.wire, gs[cols.back()].shape, P[1].wire, gs[cols.front()].shape}, tol);
      if (!exact.IsNull())
      {
        sew.Add(as_analytic(exact, 1e-6 * (1 + size)));
        continue;
      }
    }
    std::vector<std::vector<gp_Pnt>> col_at(ua.size());
    for (size_t m = 0; m < ua.size(); ++m)
      if (cols[m] >= 0)
        col_at[m] = profiles_at(ua[m]);
    std::vector<std::vector<gp_Pnt>> grid(NU + 1, std::vector<gp_Pnt>(NV + 1));
    for (int i = 0; i <= NU; ++i)
    {
      const double              u = (double)i / NU;
      const std::vector<double> w = hermite_tents(ua, u);
      const std::vector<gp_Pnt> at = profiles_at(u);
      for (int j = 0; j <= NV; ++j)
      {
        const double v = grid_v[j];
        gp_XYZ       s = across(at, j);
        for (size_t m = 0; m < ua.size(); ++m)
          if (cols[m] >= 0)
            s += (gs[cols[m]].at(v).XYZ() - across(col_at[m], j)) * w[m];
        grid[i][j] = gp_Pnt(s);
      }
    }
    sew.Add(simplest_face(grid, 1e-6 * (1 + size)));
  }
  sew.Perform();
  return wrap(sew.SewedShape());
}

// A surface loft from an open profile to a point between two guides from the profile's ends to it (live core
// q-singularity-loft-1119, c2-surf-workshop-singularities' Problem loft). Live's face is the Coons sum of the profile,
// the point and the guides, each curve by its own parameter: across the guides the smoothstep W(u) = 3u^2 - 2u^3,
// toward the point straight. A start Normal to profile adds phi(v) T(u), phi = v (1 - v)(1 - v / 2), T turning the
// start derivative onto the normal at the guides' own (blended by W); a guide with a condition then takes that term's
// turn off its side: psi(v) h(u) P'(end), h the cubic Hermite of that side, psi = phi with C1 caps to rest over
// v in [0, 1/32] and [7/8, 1]. (Live's three variants within 0.2 mm; live adds a small slope along the normal of a rule
// not known.)
std::unique_ptr<OcShape> loft_to_point(const OcShape& profile, P3 apex, const OcShapeList& guides, P3 start_normal,
                                       rust::Slice<const bool> guide_conditions)
{
  if (guides.items.size() != 2 || guide_conditions.size() != 2)
    fail_invalid("loft to a point: two guides");
  struct Par
  {
    std::unique_ptr<BRepAdaptor_CompCurve> c;
    bool                                   rev = false;
    double                                 a = 0, b = 1;
    explicit Par(const TopoDS_Wire& w) : c(std::make_unique<BRepAdaptor_CompCurve>(w)), a(c->FirstParameter()), b(c->LastParameter()) {}
    double t_of(double t) const { return rev ? b - (b - a) * t : a + (b - a) * t; }
    gp_Pnt at(double t) const { return c->Value(t_of(t)); }
    gp_Vec d1(double t) const
    {
      gp_Pnt p;
      gp_Vec v;
      c->D1(t_of(t), p, v);
      return v * (rev ? a - b : b - a);
    }
  };
  const Par    P(as_wire(profile.shape, "loft profile"));
  const gp_Pnt C(apex.x, apex.y, apex.z);
  const gp_Pnt P0 = P.at(0), P1 = P.at(1);
  Bnd_Box      box;
  BRepBndLib::Add(profile.shape, box);
  const double tol = 1e-7 * (1 + std::sqrt(box.SquareExtent()) + P0.Distance(C));
  // Each guide from the profile to the point, the one from the profile's start first.
  std::vector<Par> G;
  bool             cond[2] = {false, false};
  for (size_t i = 0; i < 2; ++i)
  {
    Par g(as_wire(guides.items[i], "loft guide"));
    g.rev = g.at(0).Distance(C) < g.at(1).Distance(C);
    if (g.at(1).Distance(C) > tol)
      fail_op("loft to a point: a guide doesn't reach the point");
    G.push_back(std::move(g));
    cond[i] = guide_conditions[i];
  }
  if (G[0].at(0).Distance(P0) > G[1].at(0).Distance(P0))
  {
    std::swap(G[0], G[1]);
    std::swap(cond[0], cond[1]);
  }
  if (G[0].at(0).Distance(P0) > tol || G[1].at(0).Distance(P1) > tol)
    fail_op("loft to a point: the guides don't leave from the profile's ends");
  const gp_Vec n = to_vec(start_normal);
  const bool   normal = n.Magnitude() > 1e-12;
  const gp_Dir nd = normal ? gp_Dir(n) : gp_Dir(0, 0, 1);
  const gp_Vec g0 = G[0].d1(0), g1 = G[1].d1(0), dp0 = P.d1(0), dp1 = P.d1(1);
  auto W   = [](double u) { return u * u * (3 - 2 * u); };
  auto phi = [](double v) { return v * (1 - v) * (1 - v / 2); };
  auto psi = [&](double v) {
    auto cap = [&](double x, double from, double to) {
      const double L = to - from, t = (x - from) / L, e = 1e-7;
      const double dphi = (phi(from + e) - phi(from - e)) / (2 * e);
      return (2 * t * t * t - 3 * t * t + 1) * phi(from) + (t * t * t - 2 * t * t + t) * L * dphi;
    };
    if (v < 1.0 / 32)
      return cap(v, 1.0 / 32, 0.0);
    if (v > 7.0 / 8)
      return cap(v, 7.0 / 8, 1.0);
    return phi(v);
  };
  const int NU = 48, NV = 64;
  std::vector<std::vector<gp_Pnt>> grid(NU + 1, std::vector<gp_Pnt>(NV + 1));
  for (int i = 0; i <= NU; ++i)
  {
    const double u = (double)i / NU, w = W(u);
    const gp_XYZ d = P.at(u).XYZ() - P0.XYZ() * (1 - w) - P1.XYZ() * w;
    // The start term: the start derivative onto the normal at the guides' own there.
    gp_XYZ T(0, 0, 0);
    if (normal)
    {
      const gp_Vec guides_d = g0 * (1 - w) + g1 * w;
      T = (gp_Vec(nd) * guides_d.Dot(gp_Vec(nd))).XYZ() - guides_d.XYZ() + d;
    }
    gp_XYZ side(0, 0, 0);
    if (normal && cond[0])
      side += dp0.XYZ() * (u * (1 - u) * (1 - u));
    if (normal && cond[1])
      side += dp1.XYZ() * (u * u * (u - 1));
    for (int j = 0; j <= NV; ++j)
    {
      const double v = (double)j / NV;
      const gp_XYZ s = G[0].at(v).XYZ() * (1 - w) + G[1].at(v).XYZ() * w + d * (1 - v) + T * phi(v) - side * psi(v);
      grid[i][j] = gp_Pnt(s);
    }
  }
  return wrap(simplest_face(grid, 1e-6 * (1 + std::sqrt(box.SquareExtent()))));
}

namespace {
// v -> arc length along a guide through its meets (v_i, s_i), increasing: monotone cubic Hermite (Fritsch-Carlson),
// so the guide runs smoothly through every station.
struct Pchip
{
  std::vector<double> x, y, m;
  Pchip(std::vector<double> xs, std::vector<double> ys) : x(std::move(xs)), y(std::move(ys)), m(x.size(), 0.0)
  {
    const size_t        n = x.size();
    std::vector<double> d(n - 1);
    for (size_t i = 0; i + 1 < n; ++i)
      d[i] = (y[i + 1] - y[i]) / (x[i + 1] - x[i]);
    m[0]     = d[0];
    m[n - 1] = d[n - 2];
    for (size_t i = 1; i + 1 < n; ++i)
    {
      const double h0 = x[i] - x[i - 1], h1 = x[i + 1] - x[i], w0 = 2 * h1 + h0, w1 = h1 + 2 * h0;
      m[i]            = d[i - 1] * d[i] <= 0 ? 0.0 : (w0 + w1) / (w0 / d[i - 1] + w1 / d[i]);
    }
  }
  double operator()(double t) const
  {
    const size_t k = (size_t)(std::upper_bound(x.begin(), x.end(), t) - x.begin());
    const size_t i = std::min(x.size() - 2, k == 0 ? 0 : k - 1);
    const double h = x[i + 1] - x[i], s = (t - x[i]) / h;
    const double h00 = 2 * s * s * s - 3 * s * s + 1, h10 = s * s * s - 2 * s * s + s, h01 = -2 * s * s * s + 3 * s * s, h11 = s * s * s - s * s;
    return h00 * y[i] + h10 * h * m[i] + h01 * y[i + 1] + h11 * h * m[i + 1];
  }
};
} // namespace

// A surface loft through closed profiles in order (a point may end it at either side) along two guides or more that
// meet every one (c2-surf-workshop-knifehandle's Loft 1: P1..P6 to the tip between two side curves; live one face
// through every profile and guide to 1 µm). Each profile runs from guide 0 by arc length between the guides' meets,
// the guides' nodes at their mean share of the loop; the skin interpolates the profiles cubically across stations (the
// guides' mean share of their length between the end profiles); each guide's difference from the skin along its node
// fades across u by periodic cubic Hermite tents, zero on every profile.
std::unique_ptr<OcShape> loft_guided_multi(const OcShapeList& profiles, const OcShapeList& guides_in)
{
  const KernelTimer timer("loft_guided_multi");
  const size_t      n = profiles.items.size(), m = guides_in.items.size();
  if (n < 2 || m < 2)
    fail_invalid("loft_guided_multi: needs two profiles and two guides at least");
  Bnd_Box box;
  for (const TopoDS_Shape& p : profiles.items)
    BRepBndLib::Add(p, box);
  const double tol = 1e-5 * (1 + std::sqrt(box.SquareExtent()));
  // Stations: a loop or a point.
  std::vector<std::unique_ptr<Loop>> loops(n);
  std::vector<gp_Pnt>                points(n);
  for (size_t i = 0; i < n; ++i)
  {
    const TopoDS_Shape& p = profiles.items[i];
    if (p.ShapeType() == TopAbs_VERTEX)
    {
      if (i != 0 && i + 1 != n)
        fail_invalid("loft: a point profile must be first or last");
      points[i] = BRep_Tool::Pnt(TopoDS::Vertex(p));
      continue;
    }
    loops[i] = std::make_unique<Loop>(outer_wire(p));
    if (!loops[i]->c->IsClosed() && loops[i]->c->Value(loops[i]->t0).Distance(loops[i]->c->Value(loops[i]->t1)) > tol)
      fail_invalid("loft_guided_multi: profiles must be closed");
  }
  // Each guide's meets: arc length along it and the point.
  std::vector<std::unique_ptr<Adaptor3d_Curve>> gc;
  std::vector<std::vector<double>>              gs(m, std::vector<double>(n));
  std::vector<std::vector<gp_Pnt>>              gp(m, std::vector<gp_Pnt>(n));
  std::vector<double>                           glen(m);
  for (size_t j = 0; j < m; ++j)
  {
    gc.push_back(guide_curve(guides_in.items[j]));
    const Adaptor3d_Curve& g = *gc.back();
    glen[j]                  = GCPnts_AbscissaPoint::Length(g);
    for (size_t i = 0; i < n; ++i)
    {
      double t = g.FirstParameter();
      gp_Pnt at;
      if (loops[i])
      {
        if (meet(g, *loops[i]->c, loops[i]->wire, tol, t, at) > tol)
          fail_op("All guides should intersect each profile boundary.");
      }
      else
      {
        Extrema_ExtPC x(points[i], g);
        double        best = 1e300;
        if (x.IsDone())
          for (int k = 1; k <= x.NbExt(); ++k)
            if (x.SquareDistance(k) < best)
              best = x.SquareDistance(k), t = x.Point(k).Parameter();
        for (const double e : {g.FirstParameter(), g.LastParameter()})
          if (points[i].SquareDistance(g.Value(e)) < best)
            best = points[i].SquareDistance(g.Value(e)), t = e;
        if (std::sqrt(best) > tol)
          fail_op("All guides should intersect each profile boundary.");
        at = points[i];
      }
      gs[j][i] = GCPnts_AbscissaPoint::Length(g, g.FirstParameter(), t);
      gp[j][i] = at;
    }
    // Along the guide in the profiles' order, either way.
    const bool up = gs[j][n - 1] > gs[j][0];
    for (size_t i = 0; i + 1 < n; ++i)
      if ((gs[j][i + 1] - gs[j][i]) * (up ? 1 : -1) <= tol)
        fail_op("loft_guided_multi: a guide doesn't run through the profiles in order");
  }
  // Stations' v: chord length between the profiles' centres (live's knifehandle area within 0.5 %; the guides' share
  // of their runs 0.7 % off).
  std::vector<gp_XYZ> centre(n);
  for (size_t i = 0; i < n; ++i)
  {
    if (!loops[i])
    {
      centre[i] = points[i].XYZ();
      continue;
    }
    gp_XYZ sum(0, 0, 0);
    for (int q = 0; q < 64; ++q)
      sum += loops[i]->at(loops[i]->len * q / 64).XYZ();
    centre[i] = sum / 64;
  }
  std::vector<double> v(n, 0.0);
  for (size_t i = 1; i < n; ++i)
    v[i] = v[i - 1] + (centre[i] - centre[i - 1]).Modulus();
  if (!(v[n - 1] > tol))
    fail_op("loft_guided_multi: the profiles lie on one another");
  for (size_t i = 0; i < n; ++i)
    v[i] /= v[n - 1];
  std::vector<Pchip> gmap;
  for (size_t j = 0; j < m; ++j)
    gmap.emplace_back(v, gs[j]);
  auto guide_at = [&](size_t j, double t) {
    const Adaptor3d_Curve& g = *gc[j];
    GCPnts_AbscissaPoint   ap(g, gmap[j](t), g.FirstParameter());
    if (!ap.IsDone())
      fail_op("loft: arc length on a guide failed");
    return g.Value(ap.Parameter());
  };
  // Loops' direction: each loop's turning vector (its area normal) the first loop's way.
  auto turning = [](const Loop& l) {
    gp_XYZ       a(0, 0, 0);
    const int    k = 128;
    const gp_Pnt c0 = l.at(0);
    for (int q = 1; q + 1 <= k; ++q)
      a += (l.at(l.len * q / k).XYZ() - c0.XYZ()).Crossed(l.at(l.len * (q + 1) / k).XYZ() - c0.XYZ());
    return a;
  };
  size_t first = n;
  for (size_t i = 0; i < n && first == n; ++i)
    if (loops[i])
      first = i;
  const gp_XYZ        ref = turning(*loops[first]);
  std::vector<int>    dir(n, 1);
  std::vector<std::vector<double>> abs_(n, std::vector<double>(m, 0.0));
  for (size_t i = 0; i < n; ++i)
  {
    if (!loops[i])
      continue;
    dir[i] = turning(*loops[i]).Dot(ref) < 0 ? -1 : 1;
    for (size_t j = 0; j < m; ++j)
    {
      double d;
      abs_[i][j] = loops[i]->abscissa(gp[j][i], d);
    }
  }
  // Guides by their order round the first loop from guide 0; every loop must have them in that order.
  auto share = [&](size_t i, size_t j) {
    const Loop&  l = *loops[i];
    const double f = dir[i] > 0 ? forward(abs_[i][0], abs_[i][j], l.len) : forward(abs_[i][j], abs_[i][0], l.len);
    return f / l.len;
  };
  std::vector<size_t> order(m);
  for (size_t j = 0; j < m; ++j)
    order[j] = j;
  std::sort(order.begin() + 1, order.end(), [&](size_t a, size_t b) { return share(first, a) < share(first, b); });
  std::vector<double> nodes(m + 1, 0.0);
  for (size_t k = 1; k < m; ++k)
  {
    double sum = 0;
    int    cnt = 0;
    for (size_t i = 0; i < n; ++i)
      if (loops[i])
      {
        if (k > 1 && share(i, order[k]) <= share(i, order[k - 1]))
          fail_op("loft_guided_multi: the guides cross between profiles");
        sum += share(i, order[k]);
        ++cnt;
      }
    nodes[k] = sum / cnt;
  }
  nodes[m] = 1.0;
  // A loop's point at u: between the guides' meets by arc length.
  auto loop_at = [&](size_t i, double u) {
    if (!loops[i])
      return points[i];
    const Loop& l = *loops[i];
    size_t      k = 0;
    while (k + 1 < m && u >= nodes[k + 1])
      ++k;
    const double sa = share(i, order[k]), sb = k + 1 < m ? share(i, order[k + 1]) : 1.0;
    const double f  = sa + (sb - sa) * (u - nodes[k]) / (nodes[k + 1] - nodes[k]);
    return l.at(abs_[i][0] + dir[i] * f * l.len);
  };
  // The skin's column at u: a cubic through the stations.
  auto column = [&](double u) {
    occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, (int)n);
    occ::handle<NCollection_HArray1<double>> par = new NCollection_HArray1<double>(1, (int)n);
    for (size_t i = 0; i < n; ++i)
    {
      pts->SetValue((int)i + 1, loop_at(i, u));
      par->SetValue((int)i + 1, v[i]);
    }
    GeomAPI_Interpolate in(pts, par, false, 1e-12);
    in.Perform();
    if (!in.IsDone())
      fail_op("loft: interpolation through the profiles failed");
    return in.Curve();
  };
  std::vector<occ::handle<Geom_BSplineCurve>> at_nodes;
  for (size_t k = 0; k < m; ++k)
    at_nodes.push_back(column(nodes[k]));
  // A u-periodic surface (Parasolid's) through the grid: rows round the profiles, the first not repeated.
  // Within 1e-3 of the profiles and guides; more points only slow what intersects it (knifehandle's Mutual trim 1).
  const int                  nu = 12 * (int)m, nv = std::max(24, 6 * (int)n);
  NCollection_Array2<gp_Pnt> grid(1, nu, 1, nv + 1);
  for (int a = 0; a < nu; ++a)
  {
    const double                         u  = (double)a / nu;
    const occ::handle<Geom_BSplineCurve> cu = column(u);
    std::vector<double>                  w  = hermite_tents(nodes, u);
    w[0] += w[m];
    for (int b = 0; b <= nv; ++b)
    {
      const double t = (double)b / nv;
      gp_XYZ       p = cu->Value(t).XYZ();
      for (size_t k = 0; k < m; ++k)
        if (w[k] != 0)
          p += w[k] * (guide_at(order[k], t).XYZ() - at_nodes[k]->Value(t).XYZ());
      grid(a + 1, b + 1) = gp_Pnt(p);
    }
  }
  GeomAPI_PointsToBSplineSurface fit;
  fit.Interpolate(grid, Approx_IsoParametric, true);
  if (!fit.IsDone())
    fail_op("loft: guided face interpolation failed");
  BRepBuilderAPI_MakeFace mf(fit.Surface(), 1e-7);
  if (!mf.IsDone())
    fail_op("loft: guided face failed");
  // Normals outward from the profiles' centres (live's knifehandle Handle: +z on top, +y at the side, whichever way
  // each profile's loop runs).
  TopoDS_Face face = mf.Face();
  {
    double u0, u1, v0, v1;
    fit.Surface()->Bounds(u0, u1, v0, v1);
    const double        vm = 0.5 * (v0 + v1);
    std::vector<gp_Pnt> ring(16);
    gp_XYZ              mid(0, 0, 0);
    for (int i = 0; i < 16; ++i)
    {
      ring[i] = fit.Surface()->Value(u0 + (u1 - u0) * (i + 0.5) / 16, vm);
      mid += ring[i].XYZ() / 16;
    }
    double out = 0;
    for (int i = 0; i < 16; ++i)
    {
      gp_Pnt p;
      gp_Vec du, dv;
      fit.Surface()->D1(u0 + (u1 - u0) * (i + 0.5) / 16, vm, p, du, dv);
      out += du.Crossed(dv).XYZ().Dot(p.XYZ() - mid);
    }
    if (out < 0)
      face.Reverse();
  }
  // The end profiles' boundaries split where the other guides meet them (live: P1's two edges between its guide
  // points); the seam runs through guide 0's.
  ShapeList         tools;
  for (const size_t i : {(size_t)0, n - 1})
    if (loops[i])
      for (size_t k = 1; k < m; ++k)
      {
        double best = 1e100;
        gp_Pnt on;
        for (TopExp_Explorer x(face, TopAbs_EDGE); x.More(); x.Next())
        {
          const TopoDS_Edge& e = TopoDS::Edge(x.Current());
          if (BRep_Tool::Degenerated(e) || BRep_Tool::IsClosed(e, face))
            continue;
          double                        f, l;
          const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
          if (c.IsNull())
            continue;
          GeomAPI_ProjectPointOnCurve pp(gp[order[k]][i], c, f, l);
          if (pp.NbPoints() > 0 && pp.LowerDistance() < best)
          {
            best = pp.LowerDistance();
            on   = pp.NearestPoint();
          }
        }
        if (best < tol * 100)
          tools.Append(BRepBuilderAPI_MakeVertex(on).Vertex());
      }
  if (tools.IsEmpty())
    return wrap(face);
  ShapeList args;
  args.Append(face);
  BRepAlgoAPI_Splitter sp;
  sp.SetArguments(args);
  sp.SetTools(tools);
  sp.Build();
  if (!sp.HasErrors())
    for (TopExp_Explorer x(sp.Shape(), TopAbs_FACE); x.More(); x.Next())
      return wrap(x.Current());
  return wrap(face);
}

} // namespace noshape
