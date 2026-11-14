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
#include <BSplCLib.hxx>
#include <GeomAdaptor_Curve.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
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

  explicit Loop(const TopoDS_Wire& w) : wire(w), c(new BRepAdaptor_CompCurve(w))
  {
    t0  = c->FirstParameter();
    t1  = c->LastParameter();
    len = GCPnts_AbscissaPoint::Length(*c, t0, t1);
  }
  // Parameter at arc length s (wrapped into [0, len)).
  double param(double s) const
  {
    s = std::fmod(s, len);
    if (s < 0)
      s += len;
    if (s < 1e-15)
      return t0;
    GCPnts_AbscissaPoint ap(*c, s, t0);
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
    const double s = GCPnts_AbscissaPoint::Length(*c, t0, t);
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

} // namespace

std::unique_ptr<OcShape> loft_guided_curved(const OcShape& profile_a, const OcShape& profile_b, const OcShapeList& guides,
                                            bool make_solid)
{
  require_non_null(profile_a.shape, "first profile");
  require_non_null(profile_b.shape, "second profile");
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
    if (meet(*g.c, *A.c, ta, pa) > tol || meet(*g.c, *B.c, tb, pb) > tol || std::abs(ta - tb) < 1e-12)
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
  BRepBuilderAPI_Sewing sew(1e-6 * (1 + size));
  const int             NU = 48, NV = 24;
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
    // B by the piecewise-linear map that puts each guide end above its start.
    auto top = [&](double u) {
      size_t s = 0;
      while (s + 2 < ua.size() && u > ua[s + 1])
        ++s;
      const double t = (u - ua[s]) / (ua[s + 1] - ua[s]);
      return b_at(sb0 + lb * (ub[s] + (ub[s + 1] - ub[s]) * t));
    };
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
      const std::vector<double> w  = hermite_tents(ua, u);
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
      while (g + 2 < ub.size() && o > ub[g + 1])
        ++g;
      brk.push_back(ua[g] + (ua[g + 1] - ua[g]) * (o - ub[g]) / (ub[g + 1] - ub[g]));
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
      const int                        m = std::max(3, (int)std::ceil(NU * (at[k2 + 1] - at[k2])));
      std::vector<std::vector<gp_Pnt>> grid;
      std::vector<double>              us;
      for (int i = 0; i <= m; ++i)
      {
        us.push_back(at[k2] + (at[k2 + 1] - at[k2]) * i / m);
        grid.push_back(row(us.back()));
      }
      pieces.push_back(grid_surface_with_ends(grid, us, slopes[k2], slopes[k2 + 1]));
    }
    BRepBuilderAPI_MakeFace mf(join_u(pieces, at), 1e-7);
    if (!mf.IsDone())
      fail_op("loft: guided face failed");
    sew.Add(mf.Face());
  }
  if (make_solid)
  {
    sew.Add(profile_a.shape);
    sew.Add(profile_b.shape);
  }
  sew.Perform();
  TopoDS_Shape r = sew.SewedShape();
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
  BRepBuilderAPI_MakeSolid ms(shell);
  if (!ms.IsDone())
    fail_op("loft: solid failed");
  TopoDS_Solid sol = ms.Solid();
  BRepLib::OrientClosedSolid(sol);
  GProp_GProps gp;
  BRepGProp::VolumeProperties(sol, gp);
  if (gp.Mass() <= 0)
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

  explicit Open(const TopoDS_Wire& w) : c(new BRepAdaptor_CompCurve(w)), wire(w)
  {
    t0  = c->FirstParameter();
    t1  = c->LastParameter();
    len = GCPnts_AbscissaPoint::Length(*c, t0, t1);
  }
  gp_Pnt at(double s) const
  {
    s = std::clamp(s, 0.0, len);
    if (s < 1e-15)
      return c->Value(t0);
    if (s > len - 1e-15)
      return c->Value(t1);
    GCPnts_AbscissaPoint ap(*c, s, t0);
    return c->Value(ap.IsDone() ? ap.Parameter() : t0 + (t1 - t0) * s / len);
  }
  double abscissa_of(double t) const { return GCPnts_AbscissaPoint::Length(*c, t0, std::clamp(t, t0, t1)); }
  double abscissa(const gp_Pnt& p) const
  {
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

// A run of lines on one straight line, between curves running at other speeds (a spline in its own parameter), takes
// their speeds at its ends, eased between as a cubic over its share of the length: the chain C1 where the curves'
// shapes fix their own parameters (live 2291: c2-hunting-knife's guide, a spline, two lines and an arc; the lines'
// speed 1.13 at the spline, 1 at the arc, 0.96 between). Lines where the speed would turn back stay as they are.
void ease_lines(std::vector<occ::handle<Geom_BSplineCurve>>& pieces, std::vector<bool>& lines)
{
  for (size_t i = 0; i < pieces.size();)
  {
    if (!lines[i])
    {
      ++i;
      continue;
    }
    // The run: lines on one straight line.
    const gp_Dir dir(gp_Vec(pieces[i]->StartPoint(), pieces[i]->EndPoint()));
    size_t       j = i + 1;
    while (j < pieces.size() && lines[j] && gp_Dir(gp_Vec(pieces[j]->StartPoint(), pieces[j]->EndPoint())).Angle(dir) < 1e-9)
      ++j;
    const gp_Pnt q0 = pieces[i]->StartPoint(), q1 = pieces[j - 1]->EndPoint();
    const double len = q0.Distance(q1);
    const double a   = i > 0 ? pieces[i - 1]->DN(pieces[i - 1]->LastParameter(), 1).Magnitude() : 1.0;
    const double b   = j < pieces.size() ? pieces[j]->DN(pieces[j]->FirstParameter(), 1).Magnitude() : 1.0;
    if (len > 0 && (std::abs(a - 1) > 1e-9 || std::abs(b - 1) > 1e-9) && a > 0 && b > 0 && a * a + b * b <= 9)
    {
      const gp_Vec               d(q0, q1);
      NCollection_Array1<gp_Pnt> poles(1, 4);
      poles(1) = q0;
      poles(2) = q0.Translated(d * (a / 3));
      poles(3) = q1.Translated(d * (-b / 3));
      poles(4) = q1;
      NCollection_Array1<double> knots(1, 2);
      NCollection_Array1<int>    mults(1, 2);
      knots(1) = 0;
      knots(2) = len;
      mults(1) = mults(2) = 4;
      pieces[i] = new Geom_BSplineCurve(poles, knots, mults, 3);
      pieces.erase(pieces.begin() + (long)i + 1, pieces.begin() + (long)j);
      lines.erase(lines.begin() + (long)i + 1, lines.begin() + (long)j);
      j = i + 1;
    }
    i = j;
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
    ease_lines(pieces, lines);
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

std::unique_ptr<OcShape> loft_guided_open(const OcShape& profile_a, const OcShape& profile_b, const OcShapeList& guides,
                                          bool trim_guides, bool trim_profiles)
{
  require_non_null(profile_a.shape, "first profile");
  require_non_null(profile_b.shape, "second profile");
  Open         A(outer_wire(profile_a.shape)), B(outer_wire(profile_b.shape));
  const double size = std::max(A.len, B.len);
  const double tol  = 1e-7 * (1 + size);
  // B runs the way A does (its ends nearest A's).
  const bool flip_b = A.at(0).Distance(B.at(0)) + A.at(A.len).Distance(B.at(B.len)) > A.at(0).Distance(B.at(B.len)) + A.at(A.len).Distance(B.at(0));
  auto       b_at   = [&](double s) { return B.at(flip_b ? B.len - s : s); };
  // Each guide from A to B: its parameters there, its arc length between
  // them and how far it runs on past each profile.
  struct G
  {
    std::unique_ptr<Adaptor3d_Curve> c;
    double                           ta, tb, len, before, after, sa, sb;
    // Arc lengths from ta to the parameter range's ends.
    double up, down;
    TopoDS_Shape shape; // the guide as selected
    // Arc length along the curve at parameters `knot` (a dense table: GCPnts_AbscissaPoint fails across a composite
    // guide's kink, c2-hunting-knife's Loft 3, and the point went off the guide).
    std::vector<double> knot, arc;
    void tabulate()
    {
      const double lo = c->FirstParameter(), hi = c->LastParameter();
      const int    n  = 4000;
      knot.resize(n + 1);
      arc.resize(n + 1);
      gp_Pnt prev = c->Value(lo);
      for (int i = 0; i <= n; ++i)
      {
        knot[i]       = lo + (hi - lo) * i / n;
        const gp_Pnt p = c->Value(knot[i]);
        arc[i]        = i ? arc[i - 1] + p.Distance(prev) : 0.0;
        prev          = p;
      }
    }
    double arc_at(double t) const
    {
      const size_t i = std::min<size_t>(std::upper_bound(knot.begin(), knot.end(), t) - knot.begin(), knot.size() - 1);
      if (i == 0)
        return 0;
      return arc[i - 1] + (arc[i] - arc[i - 1]) * (t - knot[i - 1]) / std::max(knot[i] - knot[i - 1], 1e-300);
    }
    double param_at_arc(double s) const
    {
      const size_t i = std::min<size_t>(std::max<size_t>(std::lower_bound(arc.begin(), arc.end(), s) - arc.begin(), 1), arc.size() - 1);
      const double f = (s - arc[i - 1]) / std::max(arc[i] - arc[i - 1], 1e-300);
      return knot[i - 1] + (knot[i] - knot[i - 1]) * std::clamp(f, 0.0, 1.0);
    }
    gp_Pnt at(double v) const
    {
      // Signed arc length from ta along increasing parameter.
      const double a     = (tb > ta ? 1.0 : -1.0) * v * len;
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
    gp_Pnt pa, pb;
    if (meet(*g.c, *A.c, g.ta, pa) > tol || meet(*g.c, *B.c, g.tb, pb) > tol || std::abs(g.ta - g.tb) < 1e-12)
      fail_op("All guides should intersect each profile boundary.");
    const double lo = g.c->FirstParameter(), hi = g.c->LastParameter();
    g.len    = GCPnts_AbscissaPoint::Length(*g.c, std::min(g.ta, g.tb), std::max(g.ta, g.tb));
    g.before = GCPnts_AbscissaPoint::Length(*g.c, g.ta < g.tb ? lo : g.ta, g.ta < g.tb ? g.ta : hi);
    g.after  = GCPnts_AbscissaPoint::Length(*g.c, g.ta < g.tb ? g.tb : lo, g.ta < g.tb ? hi : g.tb);
    g.up     = GCPnts_AbscissaPoint::Length(*g.c, g.ta, hi);
    g.down   = GCPnts_AbscissaPoint::Length(*g.c, lo, g.ta);
    g.sa     = A.abscissa(pa);
    g.sb     = flip_b ? B.len - B.abscissa(pb) : B.abscissa(pb);
    // A guide through a profile's end runs along the face's side: its position exactly that end (a projection on a
    // composite curve lands a hair inside).
    if (pa.Distance(A.at(0)) < tol)
      g.sa = 0;
    else if (pa.Distance(A.at(A.len)) < tol)
      g.sa = A.len;
    if (pb.Distance(b_at(0)) < tol)
      g.sb = 0;
    else if (pb.Distance(b_at(B.len)) < tol)
      g.sb = B.len;
    gs.push_back(std::move(g));
  }
  if (gs.empty())
    fail_invalid("loft: no guides");
  std::sort(gs.begin(), gs.end(), [](const G& x, const G& y) { return x.sa < y.sa; });
  // The v range: the profiles, or on to the guides' ends.
  double vlo = 0, vhi = 1;
  if (!trim_guides)
    for (const G& g : gs)
    {
      vlo = std::min(vlo, -g.before / g.len);
      vhi = std::max(vhi, 1 + g.after / g.len);
    }
  // The u range: the whole profiles, or between the outermost guides.
  double a0 = 0, a1 = A.len, b0 = 0, b1 = B.len;
  if (trim_profiles && gs.size() > 1)
  {
    a0 = gs.front().sa;
    a1 = gs.back().sa;
    b0 = std::min(gs.front().sb, gs.back().sb);
    b1 = std::max(gs.front().sb, gs.back().sb);
  }
  // Seams: the ends and the corners, as many on B as on A.
  std::vector<double> sa{a0}, sb{b0};
  std::vector<double> ca = A.corners(), cb = B.corners();
  if (flip_b)
  {
    for (double& x : cb)
      x = B.len - x;
    std::sort(cb.begin(), cb.end());
  }
  ca.erase(std::remove_if(ca.begin(), ca.end(), [&](double x) { return x <= a0 + tol || x >= a1 - tol; }), ca.end());
  cb.erase(std::remove_if(cb.begin(), cb.end(), [&](double x) { return x <= b0 + tol || x >= b1 - tol; }), cb.end());
  if (ca.size() != cb.size())
    fail_op("Loft guides need profiles with as many corners yet");
  sa.insert(sa.end(), ca.begin(), ca.end());
  sb.insert(sb.end(), cb.begin(), cb.end());
  sa.push_back(a1);
  sb.push_back(b1);
  BRepBuilderAPI_Sewing sew(1e-6 * (1 + size));
  const int             NU = 48, NV = 24;
  for (size_t k = 0; k + 1 < sa.size(); ++k)
  {
    const double la = sa[k + 1] - sa[k], lb = sb[k + 1] - sb[k];
    std::vector<double> ua{0.0}, ub{0.0};
    std::vector<int>    cols{-1};
    for (size_t i = 0; i < gs.size(); ++i)
      if (gs[i].sa > sa[k] - tol && gs[i].sa < sa[k + 1] + tol)
      {
        const double u = (gs[i].sa - sa[k]) / la, w = (gs[i].sb - sb[k]) / lb;
        if (u < 1e-9)
          cols[0] = (int)i;
        else if (u > 1 - 1e-9)
          continue;
        else
        {
          ua.push_back(u);
          ub.push_back(w);
          cols.push_back((int)i);
        }
      }
    ua.push_back(1.0);
    ub.push_back(1.0);
    cols.push_back(-1);
    for (size_t i = 0; i < gs.size(); ++i)
      if (std::abs((gs[i].sa - sa[k]) / la - 1) < 1e-9)
        cols.back() = (int)i;
    auto bot = [&](double u) { return A.at(sa[k] + la * u); };
    auto top = [&](double u) {
      size_t s = 0;
      while (s + 2 < ua.size() && u > ua[s + 1])
        ++s;
      const double t = (u - ua[s]) / (ua[s + 1] - ua[s]);
      return b_at(sb[k] + lb * (ub[s] + (ub[s + 1] - ub[s]) * t));
    };
    // One patch between two guides along its sides (c2-hunting-knife's Loft 3, closing a hole): bounded by the
    // profiles' and guides' own edges, as Parasolid's face (it shares them with the faces around the hole).
    if (sa.size() == 2 && cols.front() >= 0 && cols.back() >= 0 && vlo == 0 && vhi == 1 && !trim_profiles)
    {
      const TopoDS_Face exact = face_on_edges({A.wire, gs[cols.back()].shape, B.wire, gs[cols.front()].shape}, tol);
      if (!exact.IsNull())
      {
        sew.Add(as_analytic(exact, 1e-6 * (1 + size)));
        continue;
      }
    }
    std::vector<std::vector<gp_Pnt>> grid(NU + 1, std::vector<gp_Pnt>(NV + 1));
    for (int i = 0; i <= NU; ++i)
    {
      const double              u = (double)i / NU;
      const std::vector<double> w = hermite_tents(ua, u);
      const gp_Pnt              pb = bot(u), pt = top(u);
      for (int j = 0; j <= NV; ++j)
      {
        const double v = vlo + (vhi - vlo) * j / NV;
        gp_XYZ       s = pb.XYZ() * (1 - v) + pt.XYZ() * v;
        for (size_t m = 0; m < ua.size(); ++m)
          if (cols[m] >= 0)
          {
            const gp_XYZ lin = bot(ua[m]).XYZ() * (1 - v) + top(ua[m]).XYZ() * v;
            s += (gs[cols[m]].at(v).XYZ() - lin) * w[m];
          }
        grid[i][j] = gp_Pnt(s);
      }
    }
    sew.Add(simplest_face(grid, 1e-6 * (1 + size)));
  }
  sew.Perform();
  return wrap(sew.SewedShape());
}

} // namespace noshape
