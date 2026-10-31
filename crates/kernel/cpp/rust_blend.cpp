// noshape's own fillets and chamfers (crates/brep blend.rs): the shape as a flat body (its cached one), the
// blend's result back as shapes, and the history OCCT's fillet builder gives (BRepTools_History of
// BRepFilletAPI_MakeFillet / MakeChamfer: the faces it trims Modified, each blended edge Generated its blend
// face, the edges it shortens Modified; every other edge and vertex of the input reported deleted). op_fillet and
// op_chamfer try it first (NOSHAPE_BLEND_RUST=0: they don't); it declines anything it doesn't build exactly, and OCCT's
// blend runs.

#include "internal.hpp"

#include <BRepCheck.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepCheck_Result.hxx>
#include <TopAbs.hxx>
#include <sstream>
#include <BRepLib.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <ElCLib.hxx>
#include <ElSLib.hxx>
#include <Geom2d_Line.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <algorithm>
#include <chrono>
#include <map>
#include <Geom2d_Curve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <NCollection_HArray1.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Geom_Plane.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>

namespace noshape
{

namespace
{

gp_Pnt pnt(const BrepBody& b, uint32_t v) { return gp_Pnt(b.vertices[3 * v], b.vertices[3 * v + 1], b.vertices[3 * v + 2]); }

gp_Dir dir3(const double* p) { return gp_Dir(p[0], p[1], p[2]); }

// A new face's surface (a direct frame: its normal the body's surface normal).
occ::handle<Geom_Surface> new_surface(const BrepBody& b, size_t f)
{
  const double* s = &b.surfaces[12 * f];
  const gp_Pnt  o(s[1], s[2], s[3]);
  switch (static_cast<int>(s[0]))
  {
    case 0:
    {
      const gp_Dir n = dir3(s + 4);
      return new Geom_Plane(gp_Ax3(o, n));
    }
    case 1:
      return new Geom_CylindricalSurface(gp_Ax3(o, dir3(s + 4), dir3(s + 7)), s[10]);
    case 2:
      if (std::abs(s[11]) < 1e-12 || std::abs(s[11]) >= M_PI_2 - 1e-12)
        return nullptr;
      return new Geom_ConicalSurface(gp_Ax3(o, dir3(s + 4), dir3(s + 7)), s[11], s[10]);
    case 4:
    {
      // Origin, x, y (its axis x × y; a negative size a left-handed frame: not made here).
      if (s[10] < 0)
        return nullptr;
      const gp_Dir x = dir3(s + 4), y = dir3(s + 7);
      return new Geom_ToroidalSurface(gp_Ax3(o, x.Crossed(y), x), s[10], s[11]);
    }
    case 5:
    {
      if (s[10] < 0)
        return nullptr;
      const gp_Dir x = dir3(s + 4), y = dir3(s + 7);
      return new Geom_SphericalSurface(gp_Ax3(o, x.Crossed(y), x), s[10]);
    }
    default:
      return nullptr;
  }
}


// The parameters of `p` (world) on elementary surface `S` (located by `loc`).
bool uv_of(const GeomAdaptor_Surface& a, const TopLoc_Location& loc, const gp_Pnt& p0, gp_Pnt2d& uv)
{
  const gp_Pnt p = loc.IsIdentity() ? p0 : p0.Transformed(loc.Transformation().Inverted());
  double       u = 0, v = 0;
  switch (a.GetType())
  {
    case GeomAbs_Cylinder:
      ElSLib::Parameters(a.Cylinder(), p, u, v);
      break;
    case GeomAbs_Cone:
      ElSLib::Parameters(a.Cone(), p, u, v);
      break;
    case GeomAbs_Torus:
      ElSLib::Parameters(a.Torus(), p, u, v);
      break;
    case GeomAbs_Sphere:
      ElSLib::Parameters(a.Sphere(), p, u, v);
      break;
    default:
      return false;
  }
  uv.SetCoord(u, v);
  return true;
}

// Face `i`'s new edges' curves on its (elementary, not planar) surface: each an iso-line at unit speed in the
// edge's own parameter (checked at samples), unwrapped, and shifted by whole periods so that every loop closes
// against its neighbours' (input edges keep their own curves). A seam gets one curve per use.
const char* iso_pcurves(const BrepBody& o, size_t i, bool flip, std::vector<TopoDS_Edge>& eo, const std::vector<bool>& made,
                        const TopoDS_Face& fwd, const occ::handle<Geom_Surface>& S, const TopLoc_Location& loc, double tol)
{
  const GeomAdaptor_Surface a(S);
  const double up = a.IsUPeriodic() ? a.UPeriod() : 0, vp = a.IsVPeriodic() ? a.VPeriod() : 0;
  struct UseCurve
  {
    uint32_t edge;
    bool     rev;      // the use in the face's own (TShape) wire
    bool     fixed;    // an input edge's curve, not moved
    gp_Pnt2d p0, p1;   // where the use starts and ends
    occ::handle<Geom2d_Curve> pc; // a new edge's curve on the face (in its own parameter), before shifting
    gp_Vec2d shift;               // the whole periods it moved by
    double   t0, t1;
    uint32_t v0, v1;   // the use's start and end vertices
  };
  BRep_Builder bb;
  std::vector<std::vector<UseCurve>> loops;
  for (uint32_t l = o.face_loops[i]; l < o.face_loops[i + 1]; ++l)
  {
    std::vector<UseCurve> lp;
    for (uint32_t k = o.loop_uses[l]; k < o.loop_uses[l + 1]; ++k)
    {
      UseCurve c;
      c.shift = gp_Vec2d(0, 0);
      c.edge  = o.uses[k] / 2;
      c.rev  = (o.uses[k] % 2 == 1) != flip;
      c.v0   = o.edges[2 * c.edge + (c.rev ? 1 : 0)];
      c.v1   = o.edges[2 * c.edge + (c.rev ? 0 : 1)];
      const TopoDS_Edge E = TopoDS::Edge(eo[c.edge].Oriented(c.rev ? TopAbs_REVERSED : TopAbs_FORWARD));
      BRep_Tool::Range(eo[c.edge], c.t0, c.t1);
      if (!made[c.edge])
      {
        double f, l2;
        const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(E, fwd, f, l2);
        if (pc.IsNull())
          return "an input edge without its curve on a face the blend trims";
        c.fixed = true;
        c.p0    = pc->Value(c.rev ? l2 : f);
        c.p1    = pc->Value(c.rev ? f : l2);
      }
      else
      {
        // Samples of the edge, their parameters unwrapped.
        BRepAdaptor_Curve ca(eo[c.edge]);
        const int         n = 16;
        std::vector<gp_Pnt2d> uv(n + 1);
        for (int s = 0; s <= n; ++s)
        {
          if (!uv_of(a, loc, ca.Value(c.t0 + (c.t1 - c.t0) * s / n), uv[s]))
            return "a blend face not elementary";
          if (s > 0)
          {
            double du = uv[s].X() - uv[s - 1].X(), dv = uv[s].Y() - uv[s - 1].Y();
            if (up > 0)
              du = std::remainder(du, up);
            if (vp > 0)
              dv = std::remainder(dv, vp);
            uv[s].SetCoord(uv[s - 1].X() + du, uv[s - 1].Y() + dv);
          }
        }
        gp_Vec2d d((uv[n].X() - uv[0].X()) / (c.t1 - c.t0), (uv[n].Y() - uv[0].Y()) / (c.t1 - c.t0));
        bool iso = std::abs(d.Magnitude() - 1) <= 1e-9;
        // Exactly along u or v when it is to rounding (a frame 1e-15 off its axes left a cone's ruling 1e-14
        // off a constant u, and Shell's slabs take only exact iso-lines).
        if (iso && std::abs(d.X()) <= 1e-9)
          d = gp_Vec2d(0.0, d.Y() > 0 ? 1.0 : -1.0);
        else if (iso && std::abs(d.Y()) <= 1e-9)
          d = gp_Vec2d(d.X() > 0 ? 1.0 : -1.0, 0.0);
        for (int s2 = 0; s2 <= n && iso; ++s2)
        {
          const gp_Pnt2d q(uv[0].X() + d.X() * (c.t1 - c.t0) * s2 / n, uv[0].Y() + d.Y() * (c.t1 - c.t0) * s2 / n);
          iso = q.Distance(uv[s2]) <= 1e-9 * std::max(1.0, std::abs(c.t1 - c.t0));
        }
        if (iso)
          c.pc = new Geom2d_Line(gp_Pnt2d(uv[0].X() - d.X() * c.t0, uv[0].Y() - d.Y() * c.t0), gp_Dir2d(d));
        else
        {
          // Not an iso-line (a fillet corner's ellipse on its cylinders): a cubic through its parameters at
          // 64 steps (unwrapped as the samples above), checked half-way between them against the edge.
          const int m = 64;
          occ::handle<NCollection_HArray1<gp_Pnt2d>> pts = new NCollection_HArray1<gp_Pnt2d>(1, m + 1);
          occ::handle<NCollection_HArray1<double>>   prm = new NCollection_HArray1<double>(1, m + 1);
          gp_Pnt2d prev;
          for (int s2 = 0; s2 <= m; ++s2)
          {
            const double t = c.t0 + (c.t1 - c.t0) * s2 / m;
            gp_Pnt2d     q;
            if (!uv_of(a, loc, ca.Value(t), q))
              return "a blend face not elementary";
            if (s2 > 0)
            {
              double du = q.X() - prev.X(), dv = q.Y() - prev.Y();
              if (up > 0)
                du = std::remainder(du, up);
              if (vp > 0)
                dv = std::remainder(dv, vp);
              q.SetCoord(prev.X() + du, prev.Y() + dv);
            }
            pts->SetValue(s2 + 1, q);
            prm->SetValue(s2 + 1, t);
            prev = q;
          }
          Geom2dAPI_Interpolate ip(pts, prm, false, 1e-12);
          ip.Perform();
          if (!ip.IsDone())
            return "an edge's curve on a blend face not made";
          c.pc = ip.Curve();
          for (int s2 = 0; s2 < m; ++s2)
          {
            const double   t = c.t0 + (c.t1 - c.t0) * (s2 + 0.5) / m;
            const gp_Pnt2d q = c.pc->Value(t);
            if (S->Value(q.X(), q.Y()).Distance(ca.Value(t)) > 0.1 * tol)
              return "an edge's curve on a blend face off it";
          }
        }
        c.fixed = false;
        c.p0    = c.pc->Value(c.rev ? c.t1 : c.t0);
        c.p1    = c.pc->Value(c.rev ? c.t0 : c.t1);
      }
      lp.push_back(c);
    }
    loops.push_back(lp);
  }
  // Each loop in its order round (the body's loops may list their uses in any order), the new curves shifted
  // by whole periods onto the end of the one before.
  auto gap = [&](const UseCurve& c, const gp_Pnt2d& at) {
    double du = c.p0.X() - at.X(), dv = c.p0.Y() - at.Y();
    if (!c.fixed)
    {
      if (up > 0)
        du = std::remainder(du, up);
      if (vp > 0)
        dv = std::remainder(dv, vp);
    }
    return std::hypot(du, dv);
  };
  for (auto& lp : loops)
  {
    const size_t n = lp.size();
    size_t first = 0;
    while (first < n && !lp[first].fixed)
      ++first;
    if (first == n)
      first = 0;
    std::vector<UseCurve> order{lp[first]};
    order[0].fixed = true;
    std::vector<bool> used(n, false);
    used[first] = true;
    while (order.size() < n)
    {
      const UseCurve& cur = order.back();
      size_t best = n;
      double bg   = 0;
      bool   bnew = false;
      for (size_t k = 0; k < n; ++k)
      {
        if (used[k] || lp[k].v0 != cur.v1)
          continue;
        // Prefer an edge not placed yet (a seam's second use last).
        const bool fresh = std::none_of(order.begin(), order.end(), [&](const UseCurve& x) { return x.edge == lp[k].edge; });
        const double g   = gap(lp[k], cur.p1);
        if (best == n || (fresh && !bnew) || (fresh == bnew && g < bg))
        {
          best = k;
          bg   = g;
          bnew = fresh;
        }
      }
      if (best == n || bg > 1e-7)
        return "a blend face's loop not closed in its parameters";
      UseCurve c = lp[best];
      used[best] = true;
      if (!c.fixed)
      {
        const gp_Vec2d sh(c.p0, cur.p1);
        c.p0.Translate(sh);
        c.p1.Translate(sh);
        c.shift += sh;
        c.fixed = true;
      }
      order.push_back(c);
    }
    if (order.back().p1.Distance(order.front().p0) > 1e-7)
    {
      if (std::getenv("NOSHAPE_BLEND_DEBUG"))
      {
        std::fprintf(stderr, "blend: face %zu (surface %d) loop not closed:\n", i, (int)a.GetType());
        for (const auto& c : order)
          std::fprintf(stderr, "  edge %u rev %d %s (%.6f %.6f) -> (%.6f %.6f)\n", c.edge, (int)c.rev, made[c.edge] ? "new" : "input", c.p0.X(), c.p0.Y(), c.p1.X(), c.p1.Y());
      }
      return "a blend face's loop not closed in its parameters";
    }
    lp = order;
  }
  // A new face (no input edge to go by) as OCCT's: its angles from 0 up (not from -2pi).
  if (std::all_of(loops.begin(), loops.end(), [&](const std::vector<UseCurve>& lp) {
        return std::all_of(lp.begin(), lp.end(), [&](const UseCurve& c) { return made[c.edge]; });
      }))
  {
    double umin = HUGE_VAL, vmin = HUGE_VAL;
    for (const auto& lp : loops)
      for (const auto& c : lp)
      {
        umin = std::min({umin, c.p0.X(), c.p1.X()});
        vmin = std::min({vmin, c.p0.Y(), c.p1.Y()});
      }
    const double su = up > 0 ? -up * std::floor(umin / up + 1e-9) : 0;
    const double sv = vp > 0 ? -vp * std::floor(vmin / vp + 1e-9) : 0;
    for (auto& lp : loops)
      for (auto& c : lp)
      {
        c.p0.Translate(gp_Vec2d(su, sv));
        c.p1.Translate(gp_Vec2d(su, sv));
        c.shift += gp_Vec2d(su, sv);
      }
  }
  // The curves: per made edge, its use(s) here.
  std::map<uint32_t, std::vector<const UseCurve*>> per_edge;
  for (const auto& lp : loops)
    for (const auto& c : lp)
      if (made[c.edge])
        per_edge[c.edge].push_back(&c);
  for (const auto& [e, cs] : per_edge)
  {
    auto curve = [](const UseCurve& c) -> occ::handle<Geom2d_Curve> {
      return occ::down_cast<Geom2d_Curve>(c.pc->Translated(c.shift));
    };
    if (cs.size() == 1)
      bb.UpdateEdge(eo[e], curve(*cs[0]), S, loc, tol);
    else if (cs.size() == 2 && cs[0]->rev != cs[1]->rev)
    {
      const UseCurve& fwd_use = cs[0]->rev ? *cs[1] : *cs[0];
      const UseCurve& rev_use = cs[0]->rev ? *cs[0] : *cs[1];
      bb.UpdateEdge(eo[e], curve(fwd_use), curve(rev_use), S, loc, tol);
    }
    else
      return "an edge used oddly by a blend face";
  }
  return nullptr;
}

} // namespace

bool rust_blend(const OcShape& s, const std::vector<TopoDS_Edge>& es, const std::vector<std::array<double, 3>>& spec,
                const std::vector<TopoDS_Shape>& refs, TopoDS_Shape& result, occ::handle<BRepTools_History>& hist)
{
  if (!brep_blend_enabled())
    return false;
  const KernelTimer timer("blend.rust");
  const bool        dbg = std::getenv("NOSHAPE_BLEND_DEBUG") != nullptr;
  auto              t_last = std::chrono::steady_clock::now();
  auto              lap = [&](const char* what) {
    if (!dbg)
      return;
    const auto now = std::chrono::steady_clock::now();
    std::fprintf(stderr, "blend: %s %.2f ms\n", what, std::chrono::duration<double, std::milli>(now - t_last).count());
    t_last = now;
  };
  BrepBody     b;
  double       tol = 0;
  if (const char* why = brep_body_cached(s, b, tol))
  {
    brep_decline(why);
    return false;
  }
  tol = std::max(tol, 1e-7);
  const ShapeIMap& fm = s.map(TopAbs_FACE);
  const ShapeIMap& em = s.map(TopAbs_EDGE);
  const ShapeIMap& vm = s.map(TopAbs_VERTEX);
  // Per edge: edge, kind, d1, d2, reference face.
  rust::Vec<double> sp;
  for (size_t i = 0; i < es.size(); ++i)
  {
    const int k = em.FindIndex(es[i]);
    if (k <= 0)
      return false;
    int r = 0;
    if (i < refs.size() && !refs[i].IsNull())
      r = fm.FindIndex(refs[i]);
    sp.push_back(k - 1);
    sp.push_back(spec[i][0]);
    sp.push_back(spec[i][1]);
    sp.push_back(spec[i][2]);
    sp.push_back(r > 0 ? r - 1 : -1);
  }
  lap("body");
  BrepResult out;
  if (!brep_blend(b, rust::Slice<const double>(sp.data(), sp.size()), tol, out))
    return false;
  lap("rust");
  const BrepBody& o = out.body;
  BRep_Builder    bb;
  const size_t    nv = o.vertices.size() / 3, ne = o.edges.size() / 2, nf = o.reversed.size();
  std::vector<TopoDS_Vertex> vs(nv);
  for (size_t i = 0; i < nv; ++i)
  {
    if (out.vertex_origin[3 * i] == 0)
      vs[i] = TopoDS::Vertex(vm(static_cast<int>(out.vertex_origin[3 * i + 2]) + 1).Oriented(TopAbs_FORWARD));
    else
      bb.MakeVertex(vs[i], pnt(o, static_cast<uint32_t>(i)), tol);
  }
  // Edges: the input's, a piece of an input line, or a new line or arc.
  std::vector<TopoDS_Edge> eo(ne);
  std::vector<bool>        made(ne, false);
  for (size_t i = 0; i < ne; ++i)
  {
    const uint32_t kind = out.edge_origin[3 * i];
    if (kind == 0)
    {
      eo[i] = TopoDS::Edge(em(static_cast<int>(out.edge_origin[3 * i + 2]) + 1).Oriented(TopAbs_FORWARD));
      continue;
    }
    made[i]           = true;
    const uint32_t v0 = o.edges[2 * i], v1 = o.edges[2 * i + 1];
    const gp_Pnt   p0 = pnt(o, v0), p1 = pnt(o, v1);
    TopoDS_Edge    e;
    if (kind == 1)
    {
      // A piece of an input line: its curve, its curves on its faces, the new range.
      const TopoDS_Edge src = TopoDS::Edge(em(static_cast<int>(out.edge_origin[3 * i + 2]) + 1).Oriented(TopAbs_FORWARD));
      TopLoc_Location   loc;
      double            f, l;
      const occ::handle<Geom_Curve> c = BRep_Tool::Curve(src, loc, f, l);
      occ::handle<Geom_Curve> basis = c;
      while (const occ::handle<Geom_TrimmedCurve> tc = occ::down_cast<Geom_TrimmedCurve>(basis))
        basis = tc->BasisCurve();
      const occ::handle<Geom_Line> ln = occ::down_cast<Geom_Line>(basis);
      if (ln.IsNull())
      {
        brep_decline("a piece of an edge not a line");
        return false;
      }
      const gp_Lin g  = ln->Lin().Transformed(loc.Transformation());
      const double t0 = ElCLib::Parameter(g, p0), t1 = ElCLib::Parameter(g, p1);
      bb.MakeEdge(e, c, loc, BRep_Tool::Tolerance(src));
      // (Its curves on the faces it bounds: theirs, set as those faces are made below.)
      bb.Add(e, vs[v0].Oriented(TopAbs_FORWARD));
      bb.Add(e, vs[v1].Oriented(TopAbs_REVERSED));
      bb.Range(e, t0, t1);
      made[i] = false; // (its curves on its faces are set)
    }
    else
    {
      const double*           cv = &o.curves[11 * i];
      occ::handle<Geom_Curve> c;
      double                  t0 = 0, t1 = 0;
      if (cv[0] == 0)
      {
        if (p0.Distance(p1) < tol)
        {
          brep_decline("a new line of no length");
          return false;
        }
        c  = new Geom_Line(p0, gp_Dir(gp_Vec(p0, p1)));
        t1 = p0.Distance(p1);
      }
      else if (cv[0] == 1)
      {
        const gp_Pnt ctr(cv[1], cv[2], cv[3]);
        const gp_Dir n = dir3(cv + 4);
        gp_Vec       x(cv[7], cv[8], cv[9]);
        x -= gp_Vec(n) * x.Dot(gp_Vec(n));
        if (x.Magnitude() < 1e-9)
          x = gp_Vec(ctr, p0);
        const gp_Circ g(gp_Ax2(ctr, n, gp_Dir(x)), cv[10]);
        c        = new Geom_Circle(g);
        t0       = ElCLib::Parameter(g, p0);
        double d = std::fmod(std::fmod(ElCLib::Parameter(g, p1) - t0, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
        if (v0 == v1 || d < 1e-12)
          d = 2 * M_PI;
        t1 = t0 + d;
      }
      else if (cv[0] == 3)
      {
        const gp_Pnt ctr(cv[1], cv[2], cv[3]);
        const gp_Elips g(gp_Ax2(ctr, dir3(cv + 4), dir3(cv + 7)), cv[10], out.ellipse_minor[i]);
        c        = new Geom_Ellipse(g);
        t0       = ElCLib::Parameter(g, p0);
        double d = std::fmod(std::fmod(ElCLib::Parameter(g, p1) - t0, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
        if (v0 == v1 || d < 1e-12)
          d = 2 * M_PI;
        t1 = t0 + d;
      }
      else
      {
        brep_decline("a new edge not a line, an arc or an ellipse");
        return false;
      }
      bb.MakeEdge(e, c, tol);
      bb.Add(e, vs[v0].Oriented(TopAbs_FORWARD));
      bb.Add(e, vs[v1].Oriented(TopAbs_REVERSED));
      bb.Range(e, t0, t1);
    }
    eo[i] = e;
  }
  // Faces: the input's, one on an input face's surface with new loops, or a new blend face.
  std::vector<TopoDS_Face> fo(nf);
  for (size_t i = 0; i < nf; ++i)
  {
    const uint32_t kind = out.face_origin[3 * i];
    if (kind == 0)
    {
      fo[i] = TopoDS::Face(fm(static_cast<int>(out.face_origin[3 * i + 2]) + 1));
      continue;
    }
    occ::handle<Geom_Surface> S;
    TopLoc_Location           loc;
    TopAbs_Orientation        ori;
    double                    ftol = tol;
    if (kind == 1)
    {
      const TopoDS_Face& src = TopoDS::Face(fm(static_cast<int>(out.face_origin[3 * i + 2]) + 1));
      S                      = BRep_Tool::Surface(src, loc);
      ori                    = src.Orientation();
      if (std::getenv("NOSHAPE_BLEND_DEBUG") && occ::down_cast<Geom_Plane>(S).IsNull())
        for (TopExp_Explorer we(src, TopAbs_EDGE); we.More(); we.Next())
        {
          double a, z;
          const TopoDS_Edge& ed = TopoDS::Edge(we.Current());
          const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(ed, src, a, z);
          gp_Pnt2d p0 = pc->Value(a), p1 = pc->Value(z);
          if (ed.Orientation() == TopAbs_REVERSED)
            std::swap(p0, p1);
          std::fprintf(stderr, "  input edge ori %d (face ori %d): (%.6f %.6f) -> (%.6f %.6f)\n", (int)ed.Orientation(), (int)src.Orientation(), p0.X(), p0.Y(), p1.X(), p1.Y());
        }
      ftol                   = BRep_Tool::Tolerance(src);
    }
    else
    {
      S   = new_surface(o, i);
      ori = o.reversed[i] ? TopAbs_REVERSED : TopAbs_FORWARD;
      if (S.IsNull())
      {
        brep_decline("a blend surface not made");
        return false;
      }
    }
    TopoDS_Face f;
    bb.MakeFace(f, S, loc, ftol);
    const bool flip   = ori == TopAbs_REVERSED;
    const bool planar = !occ::down_cast<Geom_Plane>(S).IsNull();
    for (uint32_t l = o.face_loops[i]; l < o.face_loops[i + 1]; ++l)
    {
      TopoDS_Wire w;
      bb.MakeWire(w);
      std::vector<std::pair<uint32_t, bool>> loop;
      for (uint32_t k = o.loop_uses[l]; k < o.loop_uses[l + 1]; ++k)
        loop.emplace_back(o.uses[k] / 2, (o.uses[k] % 2 == 1) != flip);
      for (const auto& [k, rev] : loop)
        bb.Add(w, eo[k].Oriented(rev ? TopAbs_REVERSED : TopAbs_FORWARD));
      bb.Add(f, w);
    }
    // Pieces of input edges: the input edge's curves on its face (the same parameters on the same line).
    if (kind == 1)
    {
      const TopoDS_Face src = TopoDS::Face(fm(static_cast<int>(out.face_origin[3 * i + 2]) + 1).Oriented(TopAbs_FORWARD));
      std::vector<uint32_t> done;
      for (uint32_t l = o.face_loops[i]; l < o.face_loops[i + 1]; ++l)
        for (uint32_t k = o.loop_uses[l]; k < o.loop_uses[l + 1]; ++k)
        {
          const uint32_t e = o.uses[k] / 2;
          if (out.edge_origin[3 * e] != 1 || std::find(done.begin(), done.end(), e) != done.end())
            continue;
          done.push_back(e);
          const TopoDS_Edge in = TopoDS::Edge(em(static_cast<int>(out.edge_origin[3 * e + 2]) + 1).Oriented(TopAbs_FORWARD));
          double a, z;
          const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(in, src, a, z);
          if (pc.IsNull())
          {
            brep_decline("an input edge without its curve on its face");
            return false;
          }
          if (BRep_Tool::IsClosed(in, src))
            bb.UpdateEdge(eo[e], pc, BRep_Tool::CurveOnSurface(TopoDS::Edge(in.Reversed()), src, a, z), S, loc, BRep_Tool::Tolerance(in));
          else
            bb.UpdateEdge(eo[e], pc, S, loc, BRep_Tool::Tolerance(in));
        }
    }
    // The new edges' curves on it: a plane's by OCCT; on the others iso-lines (unit speed), shifted by whole
    // periods so that each loop closes against the curves its input edges have.
    TopoDS_Face fwd = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    if (planar)
    {
      for (uint32_t l = o.face_loops[i]; l < o.face_loops[i + 1]; ++l)
        for (uint32_t k = o.loop_uses[l]; k < o.loop_uses[l + 1]; ++k)
          if (made[o.uses[k] / 2])
            BRepLib::BuildPCurveForEdgeOnPlane(eo[o.uses[k] / 2], fwd);
    }
    else if (const char* why = iso_pcurves(o, i, flip, eo, made, fwd, S, loc, tol))
    {
      brep_decline(why);
      return false;
    }
    fo[i] = TopoDS::Face(f.Oriented(ori));
  }
  lap("shapes");
  // The solid.
  TopoDS_Solid sol;
  bb.MakeSolid(sol);
  for (uint32_t h = o.solid_shells[0]; h < o.solid_shells[1]; ++h)
  {
    TopoDS_Shell sh;
    bb.MakeShell(sh);
    for (uint32_t k = o.shell_faces[h]; k < o.shell_faces[h + 1]; ++k)
      bb.Add(sh, fo[o.faces_of_shells[k]]);
    sh.Closed(true);
    bb.Add(sol, sh);
  }
  // Its curves were checked as they were made (each new curve on a blend face against its edge at samples, each
  // loop closed in its face's parameters; a plane's are its edges projected), and every edge is used twice by
  // construction; BRepCheck of the whole (NOSHAPE_BLEND_CHECK=1, the blend corpus's replay does it per case)
  // would cost more than the blend (100 ms of a plate's 100 hole rims, most in its top face's loops).
  const bool valid = std::getenv("NOSHAPE_BLEND_CHECK") == nullptr || BRepCheck_Analyzer(sol).IsValid();
  if (!valid)
  {
    const BRepCheck_Analyzer an(sol);
    if (std::getenv("NOSHAPE_BLEND_DEBUG"))
      for (TopAbs_ShapeEnum k : {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_WIRE, TopAbs_FACE, TopAbs_SHELL, TopAbs_SOLID})
        for (TopExp_Explorer x(sol, k); x.More(); x.Next())
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
                const int st_kind = ctx.ShapeType() == TopAbs_FACE ? (int)BRepAdaptor_Surface(TopoDS::Face(ctx)).GetType() : -1;
                std::fprintf(stderr, "blend: invalid %s (in %s, surface %d): %s", TopAbs::ShapeTypeToString(k), TopAbs::ShapeTypeToString(ctx.ShapeType()), st_kind, os.str().c_str());
                if (ctx.ShapeType() == TopAbs_FACE && k == TopAbs_WIRE)
                  for (TopExp_Explorer we(x.Current(), TopAbs_EDGE); we.More(); we.Next())
                  {
                    double a, z;
                    const TopoDS_Edge& ed = TopoDS::Edge(we.Current());
                    const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(ed, TopoDS::Face(ctx), a, z);
                    if (pc.IsNull())
                    {
                      std::fprintf(stderr, "  edge ori %d: no curve\n", (int)ed.Orientation());
                      continue;
                    }
                    gp_Pnt2d p0 = pc->Value(a), p1 = pc->Value(z);
                    if (ed.Orientation() == TopAbs_REVERSED)
                      std::swap(p0, p1);
                    std::fprintf(stderr, "  edge ori %d closed %d: (%.6f %.6f) -> (%.6f %.6f)\n", (int)ed.Orientation(), (int)BRep_Tool::IsClosed(ed, TopoDS::Face(ctx)), p0.X(), p0.Y(), p1.X(), p1.Y());
                  }
              }
        }
    brep_decline("the blend's result is not valid");
    return false;
  }
  lap("check");
  // OCCT's fillet history: as the result's, then every input edge and vertex not modified reported deleted.
  hist = new BRepTools_History();
  ShapeIMap modified;
  for (size_t k = 0; k + 6 <= out.history.size(); k += 6)
  {
    const uint32_t* h   = &out.history[k];
    const uint32_t kind = h[0], index = h[2], rel = h[3], okind = h[4], oidx = h[5];
    const TopoDS_Shape* in = nullptr;
    if (kind == 1)
      in = &fm(static_cast<int>(index) + 1);
    else if (kind == 2)
      in = &em(static_cast<int>(index) + 1);
    else if (kind == 3)
      in = &vm(static_cast<int>(index) + 1);
    else
      continue;
    const TopoDS_Shape* to = nullptr;
    if (okind == 1 && oidx < fo.size())
      to = &fo[oidx];
    else if (okind == 2 && oidx < eo.size())
      to = &eo[oidx];
    if (rel == 0 && to != nullptr)
    {
      hist->AddModified(*in, *to);
      modified.Add(*in);
    }
    else if (rel == 1 && to != nullptr)
      hist->AddGenerated(*in, *to);
  }
  {
    const ShapeIMap& sm = s.map(TopAbs_SOLID);
    if (sm.Extent() == 1)
      hist->AddModified(sm(1), sol);
  }
  for (int i = 1; i <= em.Extent(); ++i)
    if (!modified.Contains(em(i)))
      hist->Remove(em(i));
  for (int i = 1; i <= vm.Extent(); ++i)
    if (!modified.Contains(vm(i)))
      hist->Remove(vm(i));
  lap("history");
  result = sol;
  return true;
}

} // namespace noshape
