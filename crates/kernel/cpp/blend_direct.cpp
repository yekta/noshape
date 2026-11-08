// Blends built with no cut (BLEND-DIRECT, handoff/blend.md): the part less its blend tools, the tools' touch curves
// put on the part's faces in 2D, only the real intersections computed, each face split and the kept pieces sewn.

#include "internal.hpp"

#include <chrono>
#include <optional>

#include <BOPAlgo_BuilderFace.hxx>
#include <ElCLib.hxx>
#include <gp_Circ.hxx>
#include <BOPTools_AlgoTools3D.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <GeomInt_IntSS.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepCheck_Result.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRepTopAdaptor_FClass2d.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Geom2dAPI_InterCurveCurve.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomLib.hxx>
#include <GeomProjLib.hxx>
#include <Geom_BoundedCurve.hxx>
#include <Geom_Curve.hxx>
#include <IntTools_Context.hxx>
#include <Precision.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Pln.hxx>

namespace noshape
{
namespace
{

using HHist = occ::handle<BRepTools_History>;

bool debug() { return std::getenv("NOSHAPE_BLEND_DIRECT_DEBUG") != nullptr; }

// Whether two blend faces meet only tangentially along a circle both end on (the rounds of a tangent chain, one after
// another): their section is that circle, which their bounds already carry (IntSS takes ~5 ms over two tori there).
bool tangent_along_shared_circle(const TopoDS_Face& a, const TopoDS_Face& b, double tol)
{
  std::vector<gp_Circ> circles;
  for (TopExp_Explorer x(a, TopAbs_EDGE); x.More(); x.Next())
    if (BRepAdaptor_Curve c(TopoDS::Edge(x.Current())); c.GetType() == GeomAbs_Circle)
      circles.push_back(c.Circle());
  for (TopExp_Explorer x(b, TopAbs_EDGE); x.More(); x.Next())
  {
    BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
    if (c.GetType() != GeomAbs_Circle)
      continue;
    const gp_Circ cb = c.Circle();
    for (const gp_Circ& ca : circles)
    {
      if (ca.Location().Distance(cb.Location()) > tol || std::abs(ca.Radius() - cb.Radius()) > tol || !ca.Axis().Direction().IsParallel(cb.Axis().Direction(), 1e-7))
        continue;
      // Tangent there: the faces' normals at a point of the circle agree (up to sign).
      const gp_Pnt p = ElCLib::Value(c.FirstParameter(), cb);
      GeomAPI_ProjectPointOnSurf pa(p, BRep_Tool::Surface(a)), pb(p, BRep_Tool::Surface(b));
      if (pa.NbPoints() == 0 || pb.NbPoints() == 0 || pa.LowerDistance() > tol || pb.LowerDistance() > tol)
        continue;
      double ua, va, ub, vb;
      pa.LowerDistanceParameters(ua, va);
      pb.LowerDistanceParameters(ub, vb);
      gp_Pnt q;
      gp_Vec a1, a2, b1, b2;
      BRep_Tool::Surface(a)->D1(ua, va, q, a1, a2);
      BRep_Tool::Surface(b)->D1(ub, vb, q, b1, b2);
      const gp_Vec na = a1.Crossed(a2), nb = b1.Crossed(b2);
      if (na.Magnitude() > 0 && nb.Magnitude() > 0 && na.IsParallel(nb, 1e-6))
        return true;
    }
  }
  return false;
}

// Whether two faces lie on one plane or one cylinder.
bool same_surface(const TopoDS_Face& a, const TopoDS_Face& b, double tol)
{
  const BRepAdaptor_Surface sa(a, false), sb(b, false);
  if (sa.GetType() != sb.GetType())
    return false;
  if (sa.GetType() == GeomAbs_Plane)
  {
    const gp_Pln pa = sa.Plane(), pb = sb.Plane();
    return pa.Axis().Direction().IsParallel(pb.Axis().Direction(), 1e-9) && pa.Distance(pb.Location()) < tol;
  }
  if (sa.GetType() == GeomAbs_Cylinder)
  {
    const gp_Cylinder ca = sa.Cylinder(), cb = sb.Cylinder();
    return std::abs(ca.Radius() - cb.Radius()) < tol && ca.Axis().Direction().IsParallel(cb.Axis().Direction(), 1e-9) &&
           gp_Lin(ca.Axis()).Distance(cb.Location()) < tol;
  }
  if (sa.GetType() != GeomAbs_Cone && sa.GetType() != GeomAbs_Torus && sa.GetType() != GeomAbs_Sphere)
    return false;
  // (Others: b's points at its parameter box's corners and middle on a's surface, normals parallel.)
  double u0, u1, v0, v1;
  BRepTools::UVBounds(b, u0, u1, v0, v1);
  const occ::handle<Geom_Surface> ga = BRep_Tool::Surface(a), gb = BRep_Tool::Surface(b);
  for (const auto& [u, v] : {std::make_pair(u0, v0), std::make_pair(u1, v0), std::make_pair(u0, v1), std::make_pair(u1, v1),
                             std::make_pair(0.5 * (u0 + u1), 0.5 * (v0 + v1))})
  {
    gp_Pnt q;
    gp_Vec du, dv;
    gb->D1(u, v, q, du, dv);
    GeomAPI_ProjectPointOnSurf pr(q, ga);
    if (pr.NbPoints() == 0 || pr.LowerDistance() > tol)
      return false;
    double ua, va;
    pr.LowerDistanceParameters(ua, va);
    gp_Pnt qa;
    gp_Vec dua, dva;
    ga->D1(ua, va, qa, dua, dva);
    const gp_Vec nb = du.Crossed(dv), na = dua.Crossed(dva);
    if (nb.Magnitude() < 1e-300 || na.Magnitude() < 1e-300 || !gp_Dir(nb).IsParallel(gp_Dir(na), 1e-7))
      return false;
  }
  return true;
}

// A curve to split a face by: its 3D curve and its curve in the face's parameters, over [f, l].
struct Cut
{
  occ::handle<Geom_Curve>   c3;
  occ::handle<Geom2d_Curve> c2;
  double                    f = 0, l = 0;
  // At least this edge tolerance (a blend's touch edge: OCCT's, which later Shells read as its tangency's slack).
  double                    tol = 0;
};

struct Split
{
  double        t;
  TopoDS_Vertex v;
};

// The vertex at `p` among `known` (within tol), or a new one.
TopoDS_Vertex vertex_at(const gp_Pnt& p, double merge, double tol, std::vector<TopoDS_Vertex>& known)
{
  const TopoDS_Vertex* best = nullptr;
  double               d    = merge;
  for (const TopoDS_Vertex& v : known)
    if (const double dv = BRep_Tool::Pnt(v).Distance(p); dv < d)
    {
      d    = dv;
      best = &v;
    }
  if (best != nullptr)
  {
    // (Its tolerance takes in the point: two faces' curves meeting their shared edge a hair apart.)
    if (1.01 * d > BRep_Tool::Tolerance(*best))
      BRep_Builder().UpdateVertex(*best, 1.01 * d);
    return *best;
  }
  TopoDS_Vertex v;
  BRep_Builder().MakeVertex(v, p, tol);
  known.push_back(v);
  return v;
}

// An edge on `face` over [t0, t1] of a cut, between v0 and v1.
TopoDS_Edge cut_edge(const Cut& c, double t0, double t1, const TopoDS_Vertex& v0, const TopoDS_Vertex& v1,
                     const TopoDS_Face& face, double tol)
{
  BRepBuilderAPI_MakeEdge mk(c.c3, v0, v1, t0, t1);
  if (!mk.IsDone())
    return TopoDS_Edge();
  TopoDS_Edge  e = mk.Edge();
  BRep_Builder b;
  b.UpdateEdge(e, c.c2, face, tol);
  b.Range(e, face, t0, t1);
  // (Its curve on the face is the projection's or the intersection's own: made to agree with the 3D curve.)
  b.SameRange(e, false);
  b.SameParameter(e, false);
  BRepLib::SameParameter(e, tol);
  if (!BRep_Tool::SameParameter(e))
    return TopoDS_Edge();
  if (c.tol > BRep_Tool::Tolerance(e))
    b.UpdateEdge(e, c.tol);
  return e;
}

// Sorted split parameters, those closer than `dt` merged.
void tidy(std::vector<Split>& s, double dt)
{
  std::sort(s.begin(), s.end(), [](const Split& a, const Split& b) { return a.t < b.t; });
  std::vector<Split> out;
  for (const Split& x : s)
    if (out.empty() || x.t - out.back().t > dt)
      out.push_back(x);
  s = out;
}

// A face's parameter plane: its boundary's curves, inside tests, crossings.
struct Domain
{
  TopoDS_Face face;
  struct Bound
  {
    TopoDS_Edge               e;
    occ::handle<Geom2d_Curve> c2;
    double                    f, l;
    std::vector<Split>        at;
  };
  std::vector<Bound>                       bounds;
  std::vector<double>                      shifts{0.0};
  double                                   u0 = 0, period = 0, tol2 = 1e-12;
  std::unique_ptr<BRepTopAdaptor_FClass2d> cls;

  bool init(const TopoDS_Face& f, double tol)
  {
    face = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    const BRepAdaptor_Surface sa(face, false);
    double                    u1, v0, v1;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    if (sa.IsUPeriodic())
    {
      period = sa.UPeriod();
      shifts = {-period, 0.0, period};
    }
    tol2 = std::max(std::min(sa.UResolution(tol), sa.VResolution(tol)), 1e-12);
    for (TopExp_Explorer x(face, TopAbs_EDGE); x.More(); x.Next())
    {
      const TopoDS_Edge e = TopoDS::Edge(x.Current());
      double            cf, cl;
      const occ::handle<Geom2d_Curve> c2 = BRep_Tool::CurveOnSurface(e, face, cf, cl);
      if (c2.IsNull())
        return false;
      bounds.push_back({e, c2, cf, cl, {}});
    }
    cls = std::make_unique<BRepTopAdaptor_FClass2d>(face, tol2);
    return true;
  }

  // `c2` moved into the face's period (by its middle).
  occ::handle<Geom2d_Curve> into_period(const occ::handle<Geom2d_Curve>& c2, double f, double l) const
  {
    if (period == 0)
      return c2;
    const double k = std::floor((c2->Value(0.5 * (f + l)).X() - u0) / period);
    return k == 0 ? c2 : occ::down_cast<Geom2d_Curve>(c2->Translated(gp_Vec2d(-k * period, 0)));
  }

  // The shift that puts `uv` inside the face, if one does.
  std::optional<double> inside(const gp_Pnt2d& uv) const
  {
    for (double sh : shifts)
      if (cls->Perform(gp_Pnt2d(uv.X() + sh, uv.Y()), false) == TopAbs_IN)
        return sh;
    return std::nullopt;
  }

  // Where two curves cross (both ways round a periodic face); false on an overlap.
  bool cross(const occ::handle<Geom2d_Curve>& a, double af, double al, const occ::handle<Geom2d_Curve>& b, double bf,
             double bl, const std::function<void(double, double)>& hit) const
  {
    for (double sh : shifts)
    {
      const occ::handle<Geom2d_Curve> bb = sh == 0 ? b : occ::down_cast<Geom2d_Curve>(b->Translated(gp_Vec2d(sh, 0)));
      const occ::handle<Geom2d_TrimmedCurve> ta = new Geom2d_TrimmedCurve(a, af, al), tb = new Geom2d_TrimmedCurve(bb, bf, bl);
      Geom2dAPI_InterCurveCurve ix(ta, tb, tol2);
      if (ix.NbSegments() > 0)
        return false;
      for (int k = 1; k <= ix.NbPoints(); ++k)
      {
        const IntRes2d_IntersectionPoint& ip = ix.Intersector().Point(k);
        hit(ip.ParamOnFirst(), ip.ParamOnSecond());
      }
    }
    return true;
  }
};

// `face` split by `cuts` (curves on its surface, ends on its boundary or inside it): the pieces, or none when it
// can't. A cut's end within `snap` of a boundary edge splits that edge there.
std::vector<TopoDS_Face> split_face(const TopoDS_Face& face_in, const std::vector<Cut>& cuts_in, double tol, double snap,
                                    std::vector<TopoDS_Vertex>& vertices)
{
  Domain d;
  if (!d.init(face_in, tol))
    return {};
  const TopoDS_Face&               face   = d.face;
  std::vector<Domain::Bound>&      bounds = d.bounds;
  std::vector<Cut>                 cuts   = cuts_in;
  std::vector<std::vector<Split>>  cut_at(cuts.size());
  for (size_t i = 0; i < cuts.size(); ++i)
  {
    cuts[i].c2 = d.into_period(cuts[i].c2, cuts[i].f, cuts[i].l);
    cut_at[i].push_back({cuts[i].f, vertex_at(cuts[i].c3->Value(cuts[i].f), snap, tol, vertices)});
    cut_at[i].push_back({cuts[i].l, vertex_at(cuts[i].c3->Value(cuts[i].l), snap, tol, vertices)});
  }
  // A crossing at a cut's end is that end (a run-out meets the boundary tangentially: its end is the better point).
  auto near_end = [&](size_t i, const gp_Pnt& p) -> const Split* {
    for (const Split& e : {cut_at[i][0], cut_at[i][1]})
      if (BRep_Tool::Pnt(e.v).Distance(p) < snap)
        return &cut_at[i][&e == &cut_at[i][0] ? 0 : 1];
    return nullptr;
  };
  // A cut along the face's own edge, or along another cut (two tools' end caps on one line): left out.
  {
    std::vector<bool> drop(cuts.size(), false);
    for (size_t i = 0; i < cuts.size(); ++i)
    {
      for (const Domain::Bound& bd : bounds)
        drop[i] = drop[i] || !d.cross(cuts[i].c2, cuts[i].f, cuts[i].l, bd.c2, bd.f, bd.l, [](double, double) {});
      for (size_t j = 0; j < i && !drop[i]; ++j)
        drop[i] = !drop[j] && !d.cross(cuts[i].c2, cuts[i].f, cuts[i].l, cuts[j].c2, cuts[j].f, cuts[j].l, [](double, double) {});
    }
    if (debug())
      for (size_t i = 0; i < cuts.size(); ++i)
      {
        const gp_Pnt a = cuts[i].c3->Value(cuts[i].f), z = cuts[i].c3->Value(cuts[i].l);
        std::fprintf(stderr, "    cut %zu (%.3f %.3f %.3f) -> (%.3f %.3f %.3f) dropped %d\n", i, a.X() * 1e3, a.Y() * 1e3, a.Z() * 1e3, z.X() * 1e3, z.Y() * 1e3, z.Z() * 1e3, (int)drop[i]);
      }
    std::vector<Cut>                kept;
    std::vector<std::vector<Split>> kept_at;
    for (size_t i = 0; i < cuts.size(); ++i)
      if (!drop[i])
      {
        kept.push_back(cuts[i]);
        kept_at.push_back(cut_at[i]);
      }
    cuts   = kept;
    cut_at = kept_at;
  }
  for (size_t i = 0; i < cuts.size(); ++i)
  {
    // Its ends on the boundary.
    for (int end = 0; end < 2; ++end)
    {
      const gp_Pnt p = BRep_Tool::Pnt(cut_at[i][(size_t)end].v);
      for (Domain::Bound& bd : bounds)
      {
        double                        f, l;
        const occ::handle<Geom_Curve> c = BRep_Tool::Curve(bd.e, f, l);
        if (c.IsNull())
          continue;
        GeomAPI_ProjectPointOnCurve pr(p, c, f, l);
        if (pr.NbPoints() > 0 && pr.LowerDistance() < snap)
          bd.at.push_back({pr.LowerDistanceParameter(), cut_at[i][(size_t)end].v});
      }
    }
    std::vector<Split> extra;
    for (Domain::Bound& bd : bounds)
      if (!d.cross(cuts[i].c2, cuts[i].f, cuts[i].l, bd.c2, bd.f, bd.l, [&](double t, double s) {
            const gp_Pnt p = cuts[i].c3->Value(t);
            if (near_end(i, p) != nullptr)
              return;
            const TopoDS_Vertex v = vertex_at(p, snap, tol, vertices);
            extra.push_back({t, v});
            bd.at.push_back({s, v});
          }))
        return {};
    for (size_t j = i + 1; j < cuts.size(); ++j)
      if (!d.cross(cuts[i].c2, cuts[i].f, cuts[i].l, cuts[j].c2, cuts[j].f, cuts[j].l, [&](double t, double s) {
            const TopoDS_Vertex v = vertex_at(cuts[i].c3->Value(t), snap, tol, vertices);
            extra.push_back({t, v});
            cut_at[j].push_back({s, v});
          }))
        return {};
    cut_at[i].insert(cut_at[i].end(), extra.begin(), extra.end());
  }
  BRep_Builder         b;
  ShapeList            edges;
  // The cuts' pieces inside the face, both ways round.
  for (size_t i = 0; i < cuts.size(); ++i)
  {
    tidy(cut_at[i], 1e-9 * (1 + std::abs(cuts[i].l - cuts[i].f)));
    for (size_t k = 0; k + 1 < cut_at[i].size(); ++k)
    {
      const double t0 = cut_at[i][k].t, t1 = cut_at[i][k + 1].t;
      if (cut_at[i][k].v.IsSame(cut_at[i][k + 1].v) && (t1 - t0) < 1e-6 * (cuts[i].l - cuts[i].f))
        continue;
      Cut                         c  = cuts[i];
      const std::optional<double> sh = d.inside(c.c2->Value(0.5 * (t0 + t1)));
      if (!sh)
        continue;
      if (*sh != 0)
        c.c2 = occ::down_cast<Geom2d_Curve>(c.c2->Translated(gp_Vec2d(*sh, 0)));
      const TopoDS_Edge e = cut_edge(c, t0, t1, cut_at[i][k].v, cut_at[i][k + 1].v, face, tol);
      if (e.IsNull())
        return {};
      edges.Append(e.Oriented(TopAbs_FORWARD));
      edges.Append(e.Oriented(TopAbs_REVERSED));
    }
  }
  if (edges.IsEmpty())
    return {face_in};
  // The boundary's edges, split where cuts end on them (a seam once, both ways round).
  std::vector<bool> done(bounds.size(), false);
  for (size_t m = 0; m < bounds.size(); ++m)
  {
    if (done[m])
      continue;
    std::vector<size_t> uses;
    std::vector<Split>  at;
    for (size_t o = m; o < bounds.size(); ++o)
      if (bounds[o].e.IsSame(bounds[m].e))
      {
        done[o] = true;
        uses.push_back(o);
        at.insert(at.end(), bounds[o].at.begin(), bounds[o].at.end());
      }
    const Domain::Bound& bd = bounds[m];
    if (at.empty())
    {
      for (size_t o : uses)
        edges.Append(bounds[o].e);
      continue;
    }
    TopoDS_Vertex vf, vl;
    TopExp::Vertices(TopoDS::Edge(bd.e.Oriented(TopAbs_FORWARD)), vf, vl);
    at.push_back({bd.f, vf});
    at.push_back({bd.l, vl});
    tidy(at, 1e-9 * (1 + std::abs(bd.l - bd.f)));
    // (The ends stay the edge's own.)
    at.front().v = vf;
    at.back().v  = vl;
    for (size_t k = 0; k + 1 < at.size(); ++k)
    {
      TopoDS_Edge piece = TopoDS::Edge(bd.e.Oriented(TopAbs_FORWARD).EmptyCopied());
      piece.Orientation(TopAbs_FORWARD);
      b.Add(piece, at[k].v.Oriented(TopAbs_FORWARD));
      b.Add(piece, at[k + 1].v.Oriented(TopAbs_REVERSED));
      b.Range(piece, at[k].t, at[k + 1].t);
      b.UpdateVertex(at[k].v, at[k].t, piece, BRep_Tool::Tolerance(at[k].v));
      b.UpdateVertex(at[k + 1].v, at[k + 1].t, piece, BRep_Tool::Tolerance(at[k + 1].v));
      for (size_t o : uses)
        edges.Append(bounds[o].e.Orientation() == TopAbs_REVERSED ? piece.Reversed() : piece);
    }
  }
  BOPAlgo_BuilderFace bf;
  bf.SetFace(face);
  bf.SetShapes(edges);
  bf.SetContext(new IntTools_Context());
  bf.Perform();
  if (bf.HasErrors())
    return {};
  std::vector<TopoDS_Face> out;
  for (ShapeList::Iterator it(bf.Areas()); it.More(); it.Next())
  {
    TopoDS_Face f = TopoDS::Face(it.Value());
    out.push_back(face_in.Orientation() == TopAbs_REVERSED ? TopoDS::Face(f.Reversed()) : f);
  }
  return out;
}

// A point inside `f` and its outward normal there.
bool point_in(const TopoDS_Face& f, gp_Pnt& p, gp_Vec& n, const occ::handle<IntTools_Context>& ctx)
{
  gp_Pnt2d uv;
  if (BOPTools_AlgoTools3D::PointInFace(f, p, uv, ctx) != 0)
  {
    // (A cusp-shaped piece, two edges tangent at its corners: a step in from an edge's middle.)
    const TopoDS_Face        fw = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    BRepTopAdaptor_FClass2d  cls(fw, 1e-12);
    bool                     found = false;
    for (TopExp_Explorer x(fw, TopAbs_EDGE); x.More() && !found; x.Next())
    {
      const TopoDS_Edge e = TopoDS::Edge(x.Current());
      double            cf, cl;
      const occ::handle<Geom2d_Curve> c2 = BRep_Tool::CurveOnSurface(e, fw, cf, cl);
      if (c2.IsNull())
        continue;
      gp_Pnt2d m;
      gp_Vec2d t;
      c2->D1(0.5 * (cf + cl), m, t);
      if (t.Magnitude() < 1e-300)
        continue;
      if (e.Orientation() == TopAbs_REVERSED)
        t.Reverse();
      const gp_Vec2d left(-t.Y(), t.X());
      const double   len = c2->Value(cf).Distance(c2->Value(cl)) + 1e-300;
      for (double step : {1e-3, 1e-4, 1e-5, 1e-6})
      {
        const gp_Pnt2d q = m.Translated(left.Normalized() * (step * len));
        if (cls.Perform(q) == TopAbs_IN)
        {
          uv    = q;
          found = true;
          break;
        }
      }
    }
    if (!found)
      return false;
  }
  const BRepAdaptor_Surface s(f, false);
  gp_Pnt q;
  gp_Vec du, dv;
  s.D1(uv.X(), uv.Y(), q, du, dv);
  p = q;
  n = du.Crossed(dv);
  if (n.Magnitude() < 1e-300)
    return false;
  n.Normalize();
  if (f.Orientation() == TopAbs_REVERSED)
    n.Reverse();
  return true;
}

// `e`'s 3D curve and its curve on `face` (projected when it has none there).
bool cut_of(const TopoDS_Edge& e, const TopoDS_Face& face, Cut& c)
{
  double f, l;
  c.c3 = BRep_Tool::Curve(e, f, l);
  if (c.c3.IsNull())
    return false;
  c.f = f;
  c.l = l;
  double f2, l2;
  c.c2 = BRep_Tool::CurveOnSurface(e, face, f2, l2);
  if (!c.c2.IsNull() && std::abs(f2 - f) < 1e-9 && std::abs(l2 - l) < 1e-9)
    return true;
  c.c2 = GeomProjLib::Curve2d(c.c3, f, l, BRep_Tool::Surface(face));
  return !c.c2.IsNull();
}

// Each vertex's tolerance raised to what it needs: its distance to its edges' ends (3D curves and curves on faces).
void fit_vertex_tolerances(const TopoDS_Shape& s)
{
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  BRep_Builder b;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(ef.FindKey(i));
    if (BRep_Tool::Degenerated(e))
      continue;
    for (TopExp_Explorer x(e, TopAbs_VERTEX); x.More(); x.Next())
    {
      const TopoDS_Vertex v = TopoDS::Vertex(x.Current());
      const gp_Pnt        p = BRep_Tool::Pnt(v);
      const double        t = BRep_Tool::Parameter(v, e);
      double              need = BRep_Tool::Tolerance(v);
      double              f, l;
      if (const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l); !c.IsNull())
        need = std::max(need, 1.01 * c->Value(t).Distance(p));
      for (ShapeList::Iterator it(ef(i)); it.More(); it.Next())
      {
        const TopoDS_Face& face = TopoDS::Face(it.Value());
        for (const TopoDS_Edge& side : {e, TopoDS::Edge(e.Reversed())})
        {
          const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(side, face, f, l);
          if (pc.IsNull())
            continue;
          const gp_Pnt2d uv = pc->Value(BRep_Tool::Parameter(v, side, face));
          need              = std::max(need, 1.01 * BRep_Tool::Surface(face)->Value(uv.X(), uv.Y()).Distance(p));
        }
      }
      if (need > BRep_Tool::Tolerance(v))
        b.UpdateVertex(v, need);
    }
  }
}

// What BRepCheck finds wrong in `s` (debug output).
void print_check(const TopoDS_Shape& s)
{
  const BRepCheck_Analyzer an(s);
  for (TopAbs_ShapeEnum k : {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_WIRE, TopAbs_FACE, TopAbs_SHELL, TopAbs_SOLID})
    for (TopExp_Explorer x(s, k); x.More(); x.Next())
    {
      const occ::handle<BRepCheck_Result> res = an.Result(x.Current());
      if (res.IsNull())
        continue;
      for (res->InitContextIterator(); res->MoreShapeInContext(); res->NextShapeInContext())
        for (const BRepCheck_Status st : res->StatusOnShape())
          if (st != BRepCheck_NoError)
          {
            std::ostringstream os;
            BRepCheck::Print(st, os);
            const TopoDS_Shape& ctx = res->ContextualShape();
            Bnd_Box bb;
            BRepBndLib::Add(x.Current(), bb);
            double a, b2, c, d, e, f;
            bb.Get(a, b2, c, d, e, f);
            std::fprintf(stderr, "  check: %s in %s [%.3f %.3f %.3f .. %.3f %.3f %.3f]: %s", TopAbs::ShapeTypeToString(k), TopAbs::ShapeTypeToString(ctx.ShapeType()), a * 1e3, b2 * 1e3, c * 1e3, d * 1e3, e * 1e3, f * 1e3, os.str().c_str());
            if (k == TopAbs_WIRE && ctx.ShapeType() == TopAbs_FACE)
              for (BRepTools_WireExplorer we(TopoDS::Wire(x.Current()), TopoDS::Face(ctx)); we.More(); we.Next())
              {
                double                          cf, cl;
                const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(we.Current(), TopoDS::Face(ctx), cf, cl);
                if (pc.IsNull())
                  continue;
                gp_Pnt2d p0 = pc->Value(cf), p1 = pc->Value(cl);
                if (we.Current().Orientation() == TopAbs_REVERSED)
                  std::swap(p0, p1);
                std::fprintf(stderr, "    uv (%.6f %.6f) -> (%.6f %.6f) tol %.2e\n", p0.X(), p0.Y(), p1.X(), p1.Y(), BRep_Tool::Tolerance(we.CurrentVertex()));
              }
          }
    }
}

} // namespace

std::pair<TopoDS_Shape, occ::handle<BRepTools_History>> blend_direct(const TopoDS_Shape& P, const std::vector<TopoDS_Shape>& tools,
                                                                     const std::vector<TopoDS_Shape>& rim)
{
  const KernelTimer timer("blend.direct");
  const bool        dbg = debug();
  auto              no  = [&](const char* why) {
    if (dbg)
      std::fprintf(stderr, "blend direct: %s\n", why);
    fail_op(std::string("direct blend: ") + why);
  };
  if (tools.empty() || tools.size() != rim.size())
    no("one rim edge a tool");
  Bnd_Box pb;
  BRepBndLib::Add(P, pb);
  const double size = std::sqrt(pb.SquareExtent());
  const double tol  = 1e-7 * size;
  // Where curves meet the same point (OCCT's blend faces are ~1e-6 m off their supports).
  const double snap = (std::getenv("BD_SNAP") ? std::atof(std::getenv("BD_SNAP")) : 1e-4) * size;
  ShapeIMap    pfaces;
  TopExp::MapShapes(P, TopAbs_FACE, pfaces);
  std::vector<Bnd_Box> pboxes(pfaces.Extent() + 1);
  for (int i = 1; i <= pfaces.Extent(); ++i)
    BRepBndLib::Add(pfaces(i), pboxes[i]);
  // Each tool's blend faces, and the curves it puts on the part's faces.
  std::optional<KernelTimer> t_tools;
  t_tools.emplace("blend.direct.tools");
  std::vector<std::vector<TopoDS_Face>> blends(tools.size()), strip_faces(tools.size());
  std::map<int, std::vector<Cut>>       on_part;
  std::vector<std::vector<Cut>>         on_blend;
  std::vector<std::vector<int>>         coincident(tools.size());
  for (size_t i = 0; i < tools.size(); ++i)
  {
    ShapeIMap strips;
    for (TopExp_Explorer x(tools[i], TopAbs_FACE); x.More(); x.Next())
    {
      const TopoDS_Face t = TopoDS::Face(x.Current());
      bool              on = false;
      for (int j = 1; j <= pfaces.Extent(); ++j)
        if (same_surface(t, TopoDS::Face(pfaces(j)), 10 * tol))
        {
          on = true;
          if (std::find(coincident[i].begin(), coincident[i].end(), j) == coincident[i].end())
            coincident[i].push_back(j);
        }
      if (on)
        strips.Add(t);
      else
        blends[i].push_back(t);
    }
    if (blends[i].empty() || strips.IsEmpty())
      no("a tool with no blend face or none on the part");
    // A strip's edges where the tool's other face there is off its surface (a blend, an end cap): onto every part face
    // of that surface.
    ShapeAncMap tef;
    TopExp::MapShapesAndUniqueAncestors(tools[i], TopAbs_EDGE, TopAbs_FACE, tef);
    for (int s = 1; s <= strips.Extent(); ++s)
    {
      strip_faces[i].push_back(TopoDS::Face(strips(s)));
      for (TopExp_Explorer x(strips(s), TopAbs_EDGE); x.More(); x.Next())
      {
        const TopoDS_Edge e = TopoDS::Edge(x.Current());
        if (BRep_Tool::Degenerated(e))
          continue;
        bool off = false;
        if (const ShapeList* fl = tef.Seek(e))
          for (ShapeList::Iterator it(*fl); it.More(); it.Next())
            off = off || (!it.Value().IsSame(strips(s)) && !same_surface(TopoDS::Face(it.Value()), TopoDS::Face(strips(s)), 10 * tol));
        if (!off)
          continue;
        for (int j : coincident[i])
        {
          const TopoDS_Face g = TopoDS::Face(pfaces(j));
          if (!same_surface(TopoDS::Face(strips(s)), g, 10 * tol))
            continue;
          Bnd_Box eb;
          BRepBndLib::Add(e, eb);
          if (eb.IsOut(pboxes[j]))
            continue;
          Cut c;
          if (!cut_of(e, g, c))
            no("a touch curve with no curve on its face");
          c.tol = BRep_Tool::Tolerance(e);
          on_part[j].push_back(c);
        }
      }
    }
  }
  t_tools.reset();
  const occ::handle<IntTools_Context> ctx = new IntTools_Context();
  BRepClass3d_SolidClassifier in_part(P);
  const double                eps = 1e-5 * size;
  auto in_p = [&](const gp_Pnt& q) {
    const KernelTimer t1("blend.direct.classify");
    in_part.Perform(q, tol);
    return in_part.State() == TopAbs_IN;
  };
  // Each tool taken away (it lies in the part: a convex edge's round) or added (outside it: a concave edge's): by a
  // point just inside it beside a face it shares with the part.
  std::vector<bool> added(tools.size(), false);
  for (size_t i = 0; i < tools.size(); ++i)
  {
    // (Where the strip lies on the part: inside it on one side and out on the other; past the part's face both are out.)
    bool known = false;
    for (TopExp_Explorer x(tools[i], TopAbs_FACE); x.More() && !known; x.Next())
    {
      const TopoDS_Face t = TopoDS::Face(x.Current());
      if (std::find(blends[i].begin(), blends[i].end(), t) != blends[i].end())
        continue;
      const TopoDS_Face        fw = TopoDS::Face(t.Oriented(TopAbs_FORWARD));
      double                   u0, u1, v0, v1;
      BRepTools::UVBounds(fw, u0, u1, v0, v1);
      BRepTopAdaptor_FClass2d  cls(fw, 1e-9);
      const BRepAdaptor_Surface sf(fw, false);
      for (int k = 0; k < 25 && !known; ++k)
      {
        const gp_Pnt2d uv(u0 + (u1 - u0) * ((k % 5) + 0.5) / 5, v0 + (v1 - v0) * ((k / 5) + 0.5) / 5);
        if (cls.Perform(uv, false) != TopAbs_IN)
          continue;
        gp_Pnt q;
        gp_Vec du, dv;
        sf.D1(uv.X(), uv.Y(), q, du, dv);
        gp_Vec n = du.Crossed(dv);
        if (n.Magnitude() < 1e-300)
          continue;
        n.Normalize();
        if (t.Orientation() == TopAbs_REVERSED)
          n.Reverse();
        const bool in_inner = in_p(q.Translated(n * -eps)), in_outer = in_p(q.Translated(n * eps));
        if (in_inner == in_outer)
          continue;
        added[i] = in_outer;
        known    = true;
      }
    }
    if (!known)
      no("a tool with no point beside the part");
  }
  // A strip off the part's faces where the tool's own material side is the result's (a removed tool's end cap inside the
  // material, on a part face's plane past that face): one of its faces too.
  for (size_t i = 0; i < tools.size(); ++i)
    for (const TopoDS_Face& t : strip_faces[i])
    {
      gp_Pnt p;
      gp_Vec n;
      if (point_in(t, p, n, ctx) && in_p(p.Translated(n * eps)) != added[i])
        blends[i].push_back(t);
    }
  // The blends against the part's other faces and each other: real intersections.
  std::vector<TopoDS_Face> all_blends;
  std::vector<size_t>      blend_tool;
  for (size_t i = 0; i < tools.size(); ++i)
    for (const TopoDS_Face& f : blends[i])
    {
      all_blends.push_back(f);
      blend_tool.push_back(i);
    }
  on_blend.resize(all_blends.size());
  // Each face's parameter plane, made once.
  std::map<const TopoDS_TShape*, std::unique_ptr<Domain>> domains;
  auto domain_of = [&](const TopoDS_Face& f) -> const Domain* {
    std::unique_ptr<Domain>& d = domains[f.TShape().get()];
    if (!d)
    {
      d = std::make_unique<Domain>();
      if (!d->init(f, tol))
        d->face.Nullify();
    }
    return d->face.IsNull() ? nullptr : d.get();
  };
  auto section = [&](const TopoDS_Face& a, const TopoDS_Face& bface, std::vector<Cut>& on_a, std::vector<Cut>& on_b,
                     const std::vector<TopoDS_Vertex>* run_to) {
    const KernelTimer t1("blend.direct.section");
    // The surfaces' intersection within the faces' parameter boxes (each curve then trimmed to both faces).
    auto bounded = [](const TopoDS_Face& f) {
      double u0, u1, v0, v1;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      const double du = 1e-3 * (u1 - u0), dv = 1e-3 * (v1 - v0);
      return occ::handle<GeomAdaptor_Surface>(new GeomAdaptor_Surface(BRep_Tool::Surface(f), u0 - du, u1 + du, v0 - dv, v1 + dv));
    };
    GeomInt_IntSS ss;
    const auto    t_ss = std::chrono::steady_clock::now();
    ss.Perform(bounded(a), bounded(bface), tol, true, true, true);
    if (!ss.IsDone())
    {
      if (dbg)
        std::fprintf(stderr, "  IntSS not done\n");
      return false;
    }
    if (dbg)
      std::fprintf(stderr, "  section (%d x %d): %d lines, %.1f ms\n", (int)BRepAdaptor_Surface(a).GetType(), (int)BRepAdaptor_Surface(bface).GetType(), ss.NbLines(), std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t_ss).count());
    const Domain* pa = domain_of(a);
    const Domain* pb = domain_of(bface);
    if (pa == nullptr || pb == nullptr)
      return false;
    const Domain &da = *pa, &db = *pb;
    // (A section edge as loose as the blend's own edges, as OCCT's Boolean leaves it.)
    double slack = 0;
    for (const Domain::Bound& bd : db.bounds)
      slack = std::max(slack, BRep_Tool::Tolerance(bd.e));
    for (int n = 1; n <= ss.NbLines(); ++n)
    {
      occ::handle<Geom_Curve> c3 = ss.Line(n);
      if (c3.IsNull() || !ss.HasLineOnS1(n) || !ss.HasLineOnS2(n))
      {
        if (dbg)
          std::fprintf(stderr, "  line %d with no curve on a face\n", n);
        return false;
      }
      occ::handle<Geom2d_Curve> la = ss.LineOnS1(n), lb = ss.LineOnS2(n);
      // Two blends tangent to each other where they meet a face they both touch: their section stops short of the
      // crossing of their touch curves there; it runs on to it.
      if (run_to != nullptr)
      {
        bool longer = false;
        for (const bool after : {false, true})
        {
          const gp_Pnt end = c3->Value(after ? c3->LastParameter() : c3->FirstParameter());
          const TopoDS_Vertex* best = nullptr;
          double               d    = 1e-3 * size;
          for (const TopoDS_Vertex& v : *run_to)
            if (const double dv = BRep_Tool::Pnt(v).Distance(end); dv < d)
            {
              d    = dv;
              best = &v;
            }
          occ::handle<Geom_BoundedCurve> bc = occ::down_cast<Geom_BoundedCurve>(c3);
          if (best == nullptr || d < snap || bc.IsNull())
            continue;
          bc = occ::down_cast<Geom_BoundedCurve>(bc->Copy());
          GeomLib::ExtendCurveToPoint(bc, BRep_Tool::Pnt(*best), 1, after);
          c3     = bc;
          longer = true;
        }
        if (longer)
        {
          la = GeomProjLib::Curve2d(c3, c3->FirstParameter(), c3->LastParameter(), BRep_Tool::Surface(a));
          lb = GeomProjLib::Curve2d(c3, c3->FirstParameter(), c3->LastParameter(), BRep_Tool::Surface(bface));
          if (la.IsNull() || lb.IsNull())
            return false;
        }
      }
      const double f = c3->FirstParameter(), l = c3->LastParameter();
      const occ::handle<Geom2d_Curve> ca = da.into_period(la, f, l), cb = db.into_period(lb, f, l);
      // Its pieces inside both faces: split where it crosses either's boundary (the second's first: a blend's boundary
      // is crossed square, the part's face's may be touched).
      std::vector<std::pair<double, int>> at{{f, 2}, {l, 2}};
      bool                                overlap = false;
      for (const Domain::Bound& bd : db.bounds)
        if (!db.cross(cb, f, l, bd.c2, bd.f, bd.l, [&](double t, double) { at.push_back({t, 0}); }))
          overlap = true;
      for (const Domain::Bound& bd : da.bounds)
        if (!da.cross(ca, f, l, bd.c2, bd.f, bd.l, [&](double t, double) { at.push_back({t, 1}); }))
          overlap = true;
      // (A line along a face's own edge (a tool's end cap meeting a face its edge lies on): nothing to put on either.)
      if (overlap)
        continue;
      std::sort(at.begin(), at.end(), [](const auto& x, const auto& y) { return x.first < y.first; });
      std::vector<std::pair<double, int>> kept;
      for (const auto& x : at)
      {
        if (!kept.empty() && c3->Value(x.first).Distance(c3->Value(kept.back().first)) < snap)
        {
          if (x.second < kept.back().second)
            kept.back() = x;
          continue;
        }
        kept.push_back(x);
      }
      for (size_t k = 0; k + 1 < kept.size(); ++k)
      {
        const double t0 = kept[k].first, t1 = kept[k + 1].first, tm = 0.5 * (t0 + t1);
        const std::optional<double> sa = da.inside(ca->Value(tm)), sb = db.inside(cb->Value(tm));
        if (!sa || !sb)
          continue;
        const occ::handle<Geom2d_Curve> pa = *sa == 0 ? ca : occ::down_cast<Geom2d_Curve>(ca->Translated(gp_Vec2d(*sa, 0)));
        const occ::handle<Geom2d_Curve> pb = *sb == 0 ? cb : occ::down_cast<Geom2d_Curve>(cb->Translated(gp_Vec2d(*sb, 0)));
        on_a.push_back(Cut{c3, pa, t0, t1, slack});
        on_b.push_back(Cut{c3, pb, t0, t1, slack});
      }
    }
    return true;
  };
  // The part's faces a tool's faces can cross: those beside its supports (sharing an edge with one). (A run-out onto a
  // face further off is missed: the faces then don't sew and the caller's Boolean takes over.)
  std::vector<ShapeIMap> near_support(tools.size());
  {
    ShapeAncMap e2f;
    TopExp::MapShapesAndUniqueAncestors(P, TopAbs_EDGE, TopAbs_FACE, e2f);
    for (size_t i = 0; i < tools.size(); ++i)
      for (int j : coincident[i])
        for (TopExp_Explorer x(pfaces(j), TopAbs_EDGE); x.More(); x.Next())
          if (const ShapeList* fl = e2f.Seek(x.Current()))
            for (ShapeList::Iterator it(*fl); it.More(); it.Next())
              near_support[i].Add(it.Value());
  }
  for (size_t k = 0; k < all_blends.size(); ++k)
  {
    Bnd_Box bb;
    BRepBndLib::Add(all_blends[k], bb);
    const size_t i = blend_tool[k];
    for (int j = 1; j <= pfaces.Extent(); ++j)
    {
      if (bb.IsOut(pboxes[j]) || std::find(coincident[i].begin(), coincident[i].end(), j) != coincident[i].end() || !near_support[i].Contains(pfaces(j)))
        continue;
      if (!section(TopoDS::Face(pfaces(j)), all_blends[k], on_part[j], on_blend[k], nullptr))
      {
        if (dbg)
          std::fprintf(stderr, "blend direct: section of part face %d (kind %d) with blend %zu (kind %d) failed\n", j, (int)BRepAdaptor_Surface(TopoDS::Face(pfaces(j))).GetType(), k, (int)BRepAdaptor_Surface(all_blends[k]).GetType());
        no("a blend's section with the part");
      }
    }
  }
  // Inside a tool / the part.
  std::vector<std::unique_ptr<BRepClass3d_SolidClassifier>> in_tool;
  for (const TopoDS_Shape& t : tools)
    in_tool.push_back(std::make_unique<BRepClass3d_SolidClassifier>(t));
  std::vector<Bnd_Box> tool_boxes(tools.size());
  for (size_t i = 0; i < tools.size(); ++i)
  {
    BRepBndLib::Add(tools[i], tool_boxes[i]);
    tool_boxes[i].Enlarge(snap);
  }
  // Inside another tool of the same kind (taken away / added).
  auto inside_kind = [&](const gp_Pnt& p, size_t skip, bool add) {
    for (size_t i = 0; i < tools.size(); ++i)
      if (i != skip && added[i] == add && !tool_boxes[i].IsOut(p))
      {
        const KernelTimer t1("blend.direct.classify");
        in_tool[i]->Perform(p, tol);
        if (in_tool[i]->State() == TopAbs_IN)
          return true;
      }
    return false;
  };
  std::vector<TopoDS_Vertex>   vertices;
  HHist                        h = new BRepTools_History();
  BRepBuilderAPI_Sewing        sew(10 * tol);
  std::vector<std::pair<TopoDS_Shape, TopoDS_Face>> made; // (from, piece)
  // The part's faces: split where curves fall on them, the pieces kept outside every tool.
  Bnd_Box tb;
  for (const TopoDS_Shape& t : tools)
    BRepBndLib::Add(t, tb);
  for (int j = 1; j <= pfaces.Extent(); ++j)
  {
    const TopoDS_Face g = TopoDS::Face(pfaces(j));
    if (tb.IsOut(pboxes[j]))
    {
      sew.Add(g);
      continue;
    }
    const auto it = on_part.find(j);
    std::vector<TopoDS_Face> pieces;
    {
      const KernelTimer t1("blend.direct.split");
      pieces = it == on_part.end() ? std::vector<TopoDS_Face>{g} : split_face(g, it->second, tol, snap, vertices);
    }
    if (dbg)
      std::fprintf(stderr, "blend direct: part face %d (%d): %zu curves, %zu pieces\n", j, (int)BRepAdaptor_Surface(g).GetType(), it == on_part.end() ? 0 : it->second.size(), pieces.size());
    if (pieces.empty())
      no("a part face that wouldn't split");
    int kept = 0;
    for (const TopoDS_Face& piece : pieces)
    {
      gp_Pnt p;
      gp_Vec n;
      if (dbg)
      {
        GProp_GProps gp;
        BRepGProp::SurfaceProperties(piece, gp);
        std::fprintf(stderr, "  piece area %.6f mm2, valid %d\n", gp.Mass() * 1e6, (int)BRepCheck_Analyzer(piece).IsValid());
      }
      if (!point_in(piece, p, n, ctx))
        no("a piece with no point inside");
      // (A piece on a taken tool's face goes with it (just inside the material); one an added tool covers, too (just
      // outside).)
      if (inside_kind(p.Translated(n * -eps), tools.size(), false) || inside_kind(p.Translated(n * eps), tools.size(), true))
        continue;
      ++kept;
      sew.Add(piece);
      if (!piece.IsSame(g))
        made.push_back({g, piece});
    }
    if (kept == 0)
      h->Remove(g);
  }
  // Blends against each other (after the part's faces: their touch curves' crossings are known).
  for (size_t k = 0; k < all_blends.size(); ++k)
  {
    Bnd_Box bb;
    BRepBndLib::Add(all_blends[k], bb);
    for (size_t m = k + 1; m < all_blends.size(); ++m)
    {
      if (blend_tool[m] == blend_tool[k])
        continue;
      Bnd_Box mb;
      BRepBndLib::Add(all_blends[m], mb);
      if (bb.IsOut(mb))
        continue;
      const KernelTimer t_bb("blend.direct.section.blends");
      if (tangent_along_shared_circle(all_blends[k], all_blends[m], tol))
      {
        if (dbg)
          std::fprintf(stderr, "  section of blends %zu and %zu: tangent along their shared circle\n", k, m);
        continue;
      }
      if (!section(all_blends[k], all_blends[m], on_blend[k], on_blend[m], &vertices))
        no("two blends' section");
    }
  }
  for (size_t k = 0; k < all_blends.size(); ++k)
  {
    std::vector<TopoDS_Face> pieces;
    {
      const KernelTimer t1("blend.direct.split");
      pieces = on_blend[k].empty() ? std::vector<TopoDS_Face>{all_blends[k]} : split_face(all_blends[k], on_blend[k], tol, snap, vertices);
    }
    if (dbg)
      std::fprintf(stderr, "blend direct: blend %zu of tool %zu: %zu curves, %zu pieces\n", k, blend_tool[k], on_blend[k].size(), pieces.size());
    if (pieces.empty())
      no("a blend face that wouldn't split");
    for (const TopoDS_Face& piece : pieces)
    {
      gp_Pnt p;
      gp_Vec n;
      if (!point_in(piece, p, n, ctx))
        no("a blend piece with no point inside");
      // A taken tool's face is kept where the material is beside it, an added one's where air is (just outside the tool,
      // and not inside another of its kind: two tools' faces on one plane go).
      const size_t i = blend_tool[k];
      const gp_Pnt q = p.Translated(n * eps);
      if (in_p(q) == added[i] || inside_kind(q, i, added[i]))
        continue;
      // (A taken tool's faces face out of it, into what is left: turned round.)
      const TopoDS_Face f = added[i] ? piece : TopoDS::Face(piece.Reversed());
      sew.Add(f);
      made.push_back({rim[blend_tool[k]], f});
    }
  }
  {
    const KernelTimer t2("blend.direct.sew");
    sew.Perform();
  }
  if (dbg)
    for (int i = 1; i <= sew.NbFreeEdges(); ++i)
    {
      Bnd_Box bb;
      BRepBndLib::Add(sew.FreeEdge(i), bb);
      double a, b2, c, d, e, f;
      bb.Get(a, b2, c, d, e, f);
      std::fprintf(stderr, "  free edge [%.4f %.4f %.4f .. %.4f %.4f %.4f]\n", a * 1e3, b2 * 1e3, c * 1e3, d * 1e3, e * 1e3, f * 1e3);
    }
  TopoDS_Shell shell;
  int          shells = 0;
  for (TopExp_Explorer x(sew.SewedShape(), TopAbs_SHELL); x.More(); x.Next())
  {
    shell = TopoDS::Shell(x.Current());
    ++shells;
  }
  if (shells != 1)
    no("the faces didn't sew into one shell");
  BRepBuilderAPI_MakeSolid ms(shell);
  if (!ms.IsDone())
    no("no solid");
  TopoDS_Solid solid = ms.Solid();
  BRepLib::OrientClosedSolid(solid);
  fit_vertex_tolerances(solid);
  BRepLib::UpdateTolerances(solid);
  // (Smooth edges marked so: a blend's touch edges are G1, as OCCT's blends and Booleans mark them for later Shells.)
  BRepLib::EncodeRegularity(solid, 1e-4);
  bool valid = false;
  {
    const KernelTimer t1("blend.direct.check");
    valid = BRepCheck_Analyzer(solid).IsValid();
  }
  if (!valid)
  {
    if (dbg)
      print_check(solid);
    no("an invalid solid");
  }
  // History: the part's faces' pieces Modified, the blends Generated from their rim edges, through the sewing.
  for (const auto& [from, piece] : made)
  {
    const TopoDS_Shape s = sew.IsModified(piece) ? sew.Modified(piece) : piece;
    if (from.ShapeType() == TopAbs_EDGE)
      h->AddGenerated(from, s);
    else
      h->AddModified(from, s);
  }
  for (int j = 1; j <= pfaces.Extent(); ++j)
    if (sew.IsModified(pfaces(j)))
      h->AddModified(pfaces(j), sew.Modified(pfaces(j)));
  if (dbg)
  {
    GProp_GProps g;
    BRepGProp::VolumeProperties(solid, g);
    double tv = 0, te = 0;
    for (TopExp_Explorer x(solid, TopAbs_VERTEX); x.More(); x.Next())
      tv = std::max(tv, BRep_Tool::Tolerance(TopoDS::Vertex(x.Current())));
    for (TopExp_Explorer x(solid, TopAbs_EDGE); x.More(); x.Next())
      te = std::max(te, BRep_Tool::Tolerance(TopoDS::Edge(x.Current())));
    std::fprintf(stderr, "blend direct: largest tolerances: vertex %.3e, edge %.3e\n", tv, te);
    std::fprintf(stderr, "blend direct: %d faces, volume %.6f\n", [&] { ShapeIMap m; TopExp::MapShapes(solid, TopAbs_FACE, m); return m.Extent(); }(), g.Mass() * 1e9);
  }
  return {solid, h};
}

OpOut op_blend_direct(const OcShape& part, const OcShapeList& tools_in, const OcShapeList& rims)
{
  require_non_null(part.shape, "part");
  // (On a copy: the pieces' vertices get parameters on new edges. The same sub-shapes in the same order.)
  BRepBuilderAPI_Copy             copy(part.shape, true, false);
  const TopoDS_Shape              P     = copy.Shape();
  std::vector<TopoDS_Shape>       rim   = to_vec(rims);
  for (TopoDS_Shape& e : rim)
    e = copy.ModifiedShape(e);
  const auto [solid, h] = blend_direct(P, to_vec(tools_in), rim);
  return make_result({P}, solid, h.get());
}

} // namespace noshape
