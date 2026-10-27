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
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
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
  std::vector<double> corners() const
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
      if (da.Angle(db) > 1e-6)
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
struct Guide
{
  std::unique_ptr<BRepAdaptor_Curve> c;
  double                             t0, t1, len;
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

// A cubic B-spline face through a grid of points (rows along u).
TopoDS_Face grid_face(const std::vector<std::vector<gp_Pnt>>& g)
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
  BRepBuilderAPI_MakeFace mf(fit.Surface(), 1e-7);
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
    std::vector<TopoDS_Edge> edges;
    for (TopExp_Explorer x(gx, TopAbs_EDGE); x.More(); x.Next())
      edges.push_back(TopoDS::Edge(x.Current()));
    if (edges.size() != 1)
      fail_op("Loft guides of several curves are not supported yet");
    Guide g;
    g.c.reset(new BRepAdaptor_Curve(edges[0]));
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
  const std::vector<double> corners_a = A.corners();
  std::vector<double>       cb_raw    = B.corners();
  std::vector<double>       corners_b;
  for (double s : cb_raw)
    corners_b.push_back(flip_b ? std::fmod(B.len - s, B.len) : s);
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
    std::vector<std::vector<gp_Pnt>> grid(NU + 1, std::vector<gp_Pnt>(NV + 1));
    for (int i = 0; i <= NU; ++i)
    {
      const double              u  = (double)i / NU;
      const std::vector<double> w  = hermite_tents(ua, u);
      const gp_Pnt              pb = bot(u), pt = top(u);
      for (int j = 0; j <= NV; ++j)
      {
        const double v = (double)j / NV;
        gp_XYZ       s = pb.XYZ() * (1 - v) + pt.XYZ() * v;
        for (size_t m = 0; m < ua.size(); ++m)
        {
          const gp_XYZ lin = bot(ua[m]).XYZ() * (1 - v) + top(ua[m]).XYZ() * v;
          s += (column(m, v).XYZ() - lin) * w[m];
        }
        grid[i][j] = gp_Pnt(s);
      }
    }
    sew.Add(grid_face(grid));
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
      if (da.Magnitude() > 1e-12 && db.Magnitude() > 1e-12 && da.Angle(db) > 1e-6)
        out.push_back(abscissa(pa));
    }
    return out;
  }
};

// The face through a grid, as a plane or cylinder (Parasolid's simplest
// surface) when the grid lies on one.
TopoDS_Face simplest_face(const std::vector<std::vector<gp_Pnt>>& g, double tol)
{
  TopoDS_Face           f = grid_face(g);
  occ::handle<Geom_Surface> s(BRep_Tool::Surface(f));
  GeomConvert_SurfToAnaSurf conv(s);
  occ::handle<Geom_Surface> ana(conv.ConvertToAnalytical(tol));
  if (ana.IsNull() || conv.Gap() > tol)
    return f;
  BRepLib::BuildCurves3d(f);
  BRepBuilderAPI_MakeFace mf(ana, BRepTools::OuterWire(f), true);
  if (!mf.IsDone())
    return f;
  ShapeFix_Face fix(mf.Face());
  fix.Perform();
  TopoDS_Face out = fix.Face();
  GProp_GProps a, b;
  BRepGProp::SurfaceProperties(f, a);
  BRepGProp::SurfaceProperties(out, b);
  return std::abs(a.Mass() - b.Mass()) <= 1e-6 * a.Mass() ? out : f;
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
    std::unique_ptr<BRepAdaptor_Curve> c;
    double                             ta, tb, len, before, after, sa, sb;
    // Arc lengths from ta to the parameter range's ends.
    double up, down;
    gp_Pnt at(double v) const
    {
      // Signed arc length from ta along increasing parameter.
      const double a     = (tb > ta ? 1.0 : -1.0) * v * len;
      const double avail = a >= 0 ? up : down;
      if (std::abs(a) <= avail + 1e-15)
      {
        GCPnts_AbscissaPoint ap(*c, a, ta);
        if (ap.IsDone())
          return c->Value(ap.Parameter());
      }
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
    std::vector<TopoDS_Edge> edges;
    for (TopExp_Explorer x(gx, TopAbs_EDGE); x.More(); x.Next())
      edges.push_back(TopoDS::Edge(x.Current()));
    if (edges.size() != 1)
      fail_op("Loft guides of several curves are not supported yet");
    G g;
    g.c.reset(new BRepAdaptor_Curve(edges[0]));
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
