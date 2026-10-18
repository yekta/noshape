// Surfacing helpers: wrap / unwrap faces between developable surfaces
// (plane, cylinder, cone) through their isometric developments, isocline
// curves on faces, and B-spline surface fitting over a height grid.
#include "internal.hpp"

#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepLib.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRepTopAdaptor_FClass2d.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <Geom2dAPI_PointsToBSpline.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomAPI_PointsToBSplineSurface.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_Array2.hxx>
#include <NCollection_HArray1.hxx>
#include <ShapeFix_Face.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Trsf.hxx>

#include <algorithm>
#include <array>
#include <cmath>
#include <map>
#include <vector>

namespace noshape {

namespace {

constexpr double kPi = 3.14159265358979323846;

double wrap_pi(double a)
{
  while (a > kPi)
    a -= 2.0 * kPi;
  while (a <= -kPi)
    a += 2.0 * kPi;
  return a;
}

// A developable surface with its isometric development ("flat" coordinates):
//  plane:    flat = ((P-O).X, (P-O).Y)
//  cylinder: flat = (R*phi, h)            phi around Z from X, h = (P-O).Z
//  cone:     flat = rho*(cos psi, sin psi) rho = slant distance from the apex,
//                                          psi = phi*sin(alpha)
struct Dev
{
  int    kind = 0;
  gp_Pnt o;
  gp_Dir z, x, y;
  double r     = 0.0;
  double alpha = 0.0;
  double ref   = 0.0; // reference angle around the axis (results stay within +-pi of it)

  explicit Dev(const DevSurf& d)
  {
    kind = d.kind;
    if (kind > 2)
      fail_invalid("wrap: unknown surface kind");
    o            = to_pnt(d.origin);
    z            = to_dir(d.axis, "wrap axis");
    gp_Vec xv    = to_vec(d.xdir);
    xv           = xv - gp_Vec(z) * xv.Dot(gp_Vec(z));
    if (xv.Magnitude() < 1e-12)
      fail_invalid("wrap: x direction is parallel to the axis");
    x     = gp_Dir(xv);
    y     = z.Crossed(x);
    r     = d.radius;
    alpha = d.half_angle;
    ref   = d.ref_angle;
    if (kind == 1 && !(r > 0))
      fail_invalid("wrap: cylinder radius must be positive");
    if (kind == 2 && (!(alpha > 1e-9) || !(alpha < kPi / 2 - 1e-9) || r < 0))
      fail_invalid("wrap: cone half angle must be in (0, 90) degrees");
  }

  occ::handle<Geom_Surface> surface() const
  {
    gp_Ax3 ax(o, z, x);
    switch (kind)
    {
      case 0:
        return new Geom_Plane(ax);
      case 1:
        return new Geom_CylindricalSurface(ax, r);
      default:
        return new Geom_ConicalSurface(ax, alpha, r);
    }
  }

  // Angle of a point around the axis, within +-pi of the reference angle.
  double phi_of(const gp_Pnt& p) const
  {
    gp_Vec d(o, p);
    return ref + wrap_pi(std::atan2(d.Dot(gp_Vec(y)), d.Dot(gp_Vec(x))) - ref);
  }

  gp_XY flat(const gp_Pnt& p, double phi) const
  {
    gp_Vec d(o, p);
    switch (kind)
    {
      case 0:
        return gp_XY(d.Dot(gp_Vec(x)), d.Dot(gp_Vec(y)));
      case 1:
        return gp_XY(r * phi, d.Dot(gp_Vec(z)));
      default:
      {
        const double h   = d.Dot(gp_Vec(z));
        const double rr  = r + h * std::tan(alpha);
        const double rho = rr / std::sin(alpha);
        const double psi = phi * std::sin(alpha);
        return gp_XY(rho * std::cos(psi), rho * std::sin(psi));
      }
    }
  }

  // Parameters on surface() of a flat point.
  gp_Pnt2d uv(const gp_XY& f) const
  {
    switch (kind)
    {
      case 0:
        return gp_Pnt2d(f.X(), f.Y());
      case 1:
        return gp_Pnt2d(f.X() / r, f.Y());
      default:
      {
        const double sa     = std::sin(alpha);
        const double rho    = std::hypot(f.X(), f.Y());
        const double psiref = ref * sa;
        const double psi    = psiref + wrap_pi(std::atan2(f.Y(), f.X()) - psiref);
        return gp_Pnt2d(psi / sa, rho - r / sa);
      }
    }
  }
};

// Source face parameterization -> angle around the source axis, continuous
// across the face (follows the face's own periodic parameter when the face
// lies on an analytic cylinder/cone; otherwise per point).
struct SourceAngle
{
  bool   from_u = false;
  double s      = 1.0;
  double c      = 0.0;

  SourceAngle(const Dev& src, const BRepAdaptor_Surface& S, const TopoDS_Face& F)
  {
    if (src.kind == 0)
      return;
    const GeomAbs_SurfaceType t = S.GetType();
    if (t != GeomAbs_Cylinder && t != GeomAbs_Cone)
      return;
    double u0, u1, v0, v1;
    BRepTools::UVBounds(F, u0, u1, v0, v1);
    const double uc = 0.5 * (u0 + u1), vc = 0.5 * (v0 + v1);
    const double a0 = src.phi_of(S.Value(uc, vc));
    gp_Vec       d(src.o, S.Value(uc + 1e-3, vc));
    const double a1 = a0 + wrap_pi(std::atan2(d.Dot(gp_Vec(src.y)), d.Dot(gp_Vec(src.x))) - a0);
    s               = (a1 - a0) >= 0 ? 1.0 : -1.0;
    c               = a0 - s * uc;
    from_u          = true;
  }

  double operator()(const Dev& src, double u, const gp_Pnt& p) const
  {
    return from_u ? s * u + c : src.phi_of(p);
  }
};

struct Affine2
{
  double a, b, c, d, e, f;
  gp_XY  apply(const gp_XY& p) const { return gp_XY(a * p.X() + b * p.Y() + e, c * p.X() + d * p.Y() + f); }
};

// Fit a 2D B-spline through mapped samples of a parameter interval, refining
// until the 3D deviation at the midpoints is within `tol`.
occ::handle<Geom2d_Curve> fit_mapped(const std::function<gp_Pnt2d(double)>& map, double t0, double t1,
                                     const occ::handle<Geom_Surface>& surf, double tol)
{
  occ::handle<Geom2d_Curve> best;
  for (int n = 17; n <= 1025; n = 2 * n - 1)
  {
    NCollection_Array1<gp_Pnt2d> pts(1, n);
    NCollection_Array1<double>   par(1, n);
    for (int i = 0; i < n; ++i)
    {
      const double t = t0 + (t1 - t0) * i / (n - 1);
      pts(i + 1)     = map(t);
      par(i + 1)     = t;
    }
    occ::handle<Geom2d_Curve> c;
    try
    {
      Geom2dAPI_PointsToBSpline fit(pts, par, 3, 8, GeomAbs_C2, tol * 0.1);
      if (fit.IsDone())
        c = fit.Curve();
    }
    catch (const Standard_Failure&)
    {
    }
    if (c.IsNull())
      continue;
    best       = c;
    double err = 0.0;
    for (int i = 0; i + 1 < n; ++i)
    {
      const double t  = t0 + (t1 - t0) * (i + 0.5) / (n - 1);
      const gp_Pnt a  = surf->Value(map(t).X(), map(t).Y());
      const gp_Pnt2d q = c->Value(t);
      const gp_Pnt b  = surf->Value(q.X(), q.Y());
      err             = std::max(err, a.Distance(b));
    }
    if (err <= tol)
      break;
  }
  if (best.IsNull())
    fail_op("wrap: could not approximate a mapped edge");
  return best;
}

} // namespace

OpOut op_wrap_faces(const OcShapeList& faces, const DevSurf& src_d, const DevSurf& dst_d,
                    rust::Slice<const double> xf, double tol)
{
  if (faces.items.empty())
    fail_invalid("wrap needs at least one face");
  if (xf.size() != 6)
    fail_invalid("wrap: placement must have 6 values");
  const Dev     src(src_d), dst(dst_d);
  const Affine2 T{xf[0], xf[1], xf[2], xf[3], xf[4], xf[5]};
  if (!(tol > 0))
    tol = 1e-7;
  occ::handle<Geom_Surface>      dsurf = dst.surface();
  occ::handle<BRepTools_History> hist  = new BRepTools_History();
  std::vector<TopoDS_Shape>      outs;
  BRep_Builder                   B;
  const double                   q = 1e-9;
  for (const TopoDS_Shape& fs : faces.items)
  {
    const TopoDS_Face&        F = as_face(fs, "wrap face");
    BRepAdaptor_Surface       S(F, false);
    const SourceAngle         ang(src, S, F);
    const bool analytic_src = (S.GetType() == GeomAbs_Plane && src.kind == 0) || (S.GetType() == GeomAbs_Cylinder && src.kind == 1);
    const bool affine       = analytic_src && dst.kind <= 1;
    // Point map: source face parameters -> destination surface parameters.
    auto map_uv = [&](double u, double v) -> gp_Pnt2d {
      const gp_Pnt p = S.Value(u, v);
      return dst.uv(T.apply(src.flat(p, ang(src, u, p))));
    };
    auto map_flat = [&](double u, double v) -> gp_XY {
      const gp_Pnt p = S.Value(u, v);
      return T.apply(src.flat(p, ang(src, u, p)));
    };
    // Exact affine map of parameters (plane/cylinder to plane/cylinder).
    double A[6] = {1, 0, 0, 1, 0, 0};
    if (affine)
    {
      const gp_Pnt2d p0 = map_uv(0, 0), pu = map_uv(1, 0), pv = map_uv(0, 1);
      A[0] = pu.X() - p0.X();
      A[1] = pv.X() - p0.X();
      A[2] = pu.Y() - p0.Y();
      A[3] = pv.Y() - p0.Y();
      A[4] = p0.X();
      A[5] = p0.Y();
    }
    std::map<std::pair<long long, long long>, TopoDS_Vertex> vcache;
    auto vertex_at = [&](const gp_XY& f) -> TopoDS_Vertex {
      const std::pair<long long, long long> key{std::llround(f.X() / q), std::llround(f.Y() / q)};
      auto it = vcache.find(key);
      if (it != vcache.end())
        return it->second;
      const gp_Pnt2d uv = dst.uv(f);
      TopoDS_Vertex  v  = BRepBuilderAPI_MakeVertex(dsurf->Value(uv.X(), uv.Y()));
      vcache.emplace(key, v);
      return v;
    };
    double umin = 1e300, umax = -1e300;
    const TopoDS_Wire        outer = BRepTools::OuterWire(F);
    std::vector<TopoDS_Wire> wires;
    std::vector<double>      wire_area;
    int                      outer_index = -1;
    for (TopExp_Explorer we(F, TopAbs_WIRE); we.More(); we.Next())
    {
      const TopoDS_Wire& W = TopoDS::Wire(we.Current());
      TopoDS_Wire        nw;
      B.MakeWire(nw);
      int                   n_edges = 0;
      std::vector<gp_Pnt2d> loop; // uv samples in traversal order (signed area)
      for (BRepTools_WireExplorer ex(W, F); ex.More(); ex.Next())
      {
        const TopoDS_Edge& E = ex.Current();
        if (BRep_Tool::Degenerated(E))
          continue;
        double                     f, l;
        occ::handle<Geom2d_Curve>  c2 = BRep_Tool::CurveOnSurface(E, F, f, l);
        if (c2.IsNull())
          fail_op("wrap: an edge has no curve on its face");
        occ::handle<Geom2d_Curve> out;
        if (affine)
        {
          occ::handle<Geom2d_TrimmedCurve> tc = new Geom2d_TrimmedCurve(c2, f, l);
          occ::handle<Geom2d_BSplineCurve> bs = Geom2dConvert::CurveToBSplineCurve(tc);
          for (int i = 1; i <= bs->NbPoles(); ++i)
          {
            const gp_Pnt2d p = bs->Pole(i);
            bs->SetPole(i, gp_Pnt2d(A[0] * p.X() + A[1] * p.Y() + A[4], A[2] * p.X() + A[3] * p.Y() + A[5]));
          }
          out = bs;
        }
        else
        {
          out = fit_mapped(
            [&](double t) {
              const gp_Pnt2d p = c2->Value(t);
              return map_uv(p.X(), p.Y());
            },
            f, l, dsurf, tol);
        }
        const double t0 = out->FirstParameter(), t1 = out->LastParameter();
        const bool   back = E.Orientation() == TopAbs_REVERSED;
        for (int i = 0; i <= 16; ++i)
        {
          const double   k = back ? 16 - i : i;
          const gp_Pnt2d q2 = out->Value(t0 + (t1 - t0) * k / 16.0);
          umin             = std::min(umin, q2.X());
          umax             = std::max(umax, q2.X());
          if (i < 16)
            loop.push_back(q2);
        }
        const gp_Pnt2d pf = c2->Value(f), pl = c2->Value(l);
        TopoDS_Vertex  v1 = vertex_at(map_flat(pf.X(), pf.Y()));
        TopoDS_Vertex  v2 = vertex_at(map_flat(pl.X(), pl.Y()));
        BRepBuilderAPI_MakeEdge me(out, dsurf, v1, v2, t0, t1);
        if (!me.IsDone())
          fail_op("wrap: could not build a mapped edge");
        TopoDS_Edge ne = me.Edge();
        B.Add(nw, ne.Oriented(E.Orientation()));
        hist->AddGenerated(E, ne);
        ++n_edges;
      }
      if (n_edges == 0)
        continue;
      nw.Closed(true);
      if (W.IsSame(outer))
        outer_index = (int)wires.size();
      double area = 0.0;
      for (size_t i = 0; i < loop.size(); ++i)
      {
        const gp_Pnt2d& a = loop[i];
        const gp_Pnt2d& b = loop[(i + 1) % loop.size()];
        area += a.X() * b.Y() - b.X() * a.Y();
      }
      wires.push_back(nw);
      wire_area.push_back(0.5 * area);
    }
    if (wires.empty())
      fail_op("wrap: a face has no boundary");
    if (dst.kind != 0 && umax - umin > 2.0 * kPi + 1e-9)
      fail_op("wrap: the region goes more than once around the target");
    if (outer_index < 0)
      outer_index = 0;
    // Outer boundary counter-clockwise in the target's parameters, holes clockwise.
    for (size_t i = 0; i < wires.size(); ++i)
    {
      const bool is_outer = (int)i == outer_index;
      if ((is_outer && wire_area[i] < 0) || (!is_outer && wire_area[i] > 0))
        wires[i].Reverse();
    }
    BRepBuilderAPI_MakeFace mf(dsurf, wires[outer_index], true);
    if (!mf.IsDone())
      fail_op("wrap: could not build a mapped face");
    for (size_t i = 0; i < wires.size(); ++i)
      if ((int)i != outer_index)
        mf.Add(wires[i]);
    TopoDS_Face nf = mf.Face();
    BRepLib::BuildCurves3d(nf);
    ShapeFix_Face fix(nf);
    fix.FixOrientationMode() = 0;
    fix.Perform();
    nf = fix.Face();
    hist->AddGenerated(F, nf);
    outs.push_back(nf);
  }
  std::vector<TopoDS_Shape> args{compound_of(faces.items)};
  return make_result(args, outs.size() == 1 ? outs[0] : compound_of(outs), hist.get());
}

// ---------------------------------------------------------------- isocline

std::unique_ptr<OcShapeList> isocline_edges(const OcShape& face, P3 dir, double angle, bool extend)
{
  const TopoDS_Face&  F = as_face(face.shape, "isocline face");
  BRepAdaptor_Surface S(F, true);
  const gp_Dir        d   = to_dir(dir, "isocline direction");
  const bool          rev = F.Orientation() == TopAbs_REVERSED;
  const double        target = std::sin(angle);
  double              u0, u1, v0, v1;
  BRepTools::UVBounds(F, u0, u1, v0, v1);
  if (!(u1 > u0) || !(v1 > v0))
    fail_invalid("isocline: face has an empty parameter range");
  auto fval = [&](double u, double v) -> double {
    gp_Pnt p;
    gp_Vec du, dv;
    S.D1(u, v, p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (n.Magnitude() < 1e-14)
      return std::nan("");
    n.Normalize();
    if (rev)
      n.Reverse();
    return n.Dot(gp_Vec(d)) - target;
  };
  const int nu = 96, nv = 96;
  const double du = (u1 - u0) / nu, dv = (v1 - v0) / nv;
  std::vector<double> g((nu + 1) * (nv + 1));
  auto G = [&](int i, int j) -> double& { return g[i * (nv + 1) + j]; };
  for (int i = 0; i <= nu; ++i)
    for (int j = 0; j <= nv; ++j)
      G(i, j) = fval(u0 + du * i, v0 + dv * j);
  // Crossing point on a grid edge, keyed by (horizontal?, i, j).
  std::map<std::array<int, 3>, int> point_id;
  std::vector<gp_Pnt2d>             points;
  auto crossing = [&](int i, int j, bool along_u) -> int {
    const std::array<int, 3> key{along_u ? 1 : 0, i, j};
    auto it = point_id.find(key);
    if (it != point_id.end())
      return it->second;
    gp_Pnt2d a(u0 + du * i, v0 + dv * j);
    gp_Pnt2d b = along_u ? gp_Pnt2d(a.X() + du, a.Y()) : gp_Pnt2d(a.X(), a.Y() + dv);
    double   fa = G(i, j);
    double   lo = 0, hi = 1;
    for (int k = 0; k < 40; ++k)
    {
      const double m  = 0.5 * (lo + hi);
      const double fm = fval(a.X() + (b.X() - a.X()) * m, a.Y() + (b.Y() - a.Y()) * m);
      if (std::isnan(fm))
        break;
      if ((fm < 0) == (fa < 0))
      {
        lo = m;
        fa = fm;
      }
      else
      {
        hi = m;
      }
    }
    const double t = 0.5 * (lo + hi);
    points.emplace_back(a.X() + (b.X() - a.X()) * t, a.Y() + (b.Y() - a.Y()) * t);
    point_id.emplace(key, (int)points.size() - 1);
    return (int)points.size() - 1;
  };
  std::vector<std::pair<int, int>> segs;
  for (int i = 0; i < nu; ++i)
    for (int j = 0; j < nv; ++j)
    {
      const double c[4] = {G(i, j), G(i + 1, j), G(i + 1, j + 1), G(i, j + 1)};
      if (std::isnan(c[0]) || std::isnan(c[1]) || std::isnan(c[2]) || std::isnan(c[3]))
        continue;
      // Edges: 0 bottom (i,j)-(i+1,j), 1 right (i+1,j)-(i+1,j+1), 2 top (i,j+1)-(i+1,j+1), 3 left (i,j)-(i,j+1)
      int  e[4];
      bool has[4];
      has[0] = (c[0] < 0) != (c[1] < 0);
      has[1] = (c[1] < 0) != (c[2] < 0);
      has[2] = (c[3] < 0) != (c[2] < 0);
      has[3] = (c[0] < 0) != (c[3] < 0);
      if (has[0])
        e[0] = crossing(i, j, true);
      if (has[1])
        e[1] = crossing(i + 1, j, false);
      if (has[2])
        e[2] = crossing(i, j + 1, true);
      if (has[3])
        e[3] = crossing(i, j, false);
      std::vector<int> ids;
      for (int k = 0; k < 4; ++k)
        if (has[k])
          ids.push_back(k);
      if (ids.size() == 2)
        segs.emplace_back(e[ids[0]], e[ids[1]]);
      else if (ids.size() == 4)
      {
        const double center = fval(u0 + du * (i + 0.5), v0 + dv * (j + 0.5));
        if ((center < 0) == (c[0] < 0))
        {
          segs.emplace_back(e[0], e[1]);
          segs.emplace_back(e[2], e[3]);
        }
        else
        {
          segs.emplace_back(e[0], e[3]);
          segs.emplace_back(e[1], e[2]);
        }
      }
    }
  // Chain segments into polylines.
  std::vector<std::vector<int>> adj(points.size());
  for (size_t s = 0; s < segs.size(); ++s)
  {
    adj[segs[s].first].push_back((int)s);
    adj[segs[s].second].push_back((int)s);
  }
  std::vector<bool>             used(segs.size(), false);
  std::vector<std::vector<int>> chains;
  auto walk = [&](int start_pt, int seg) {
    std::vector<int> chain{start_pt};
    int              cur = start_pt;
    while (seg >= 0 && !used[seg])
    {
      used[seg]  = true;
      const int nx = segs[seg].first == cur ? segs[seg].second : segs[seg].first;
      chain.push_back(nx);
      cur = nx;
      seg = -1;
      for (int s2 : adj[cur])
        if (!used[s2])
        {
          seg = s2;
          break;
        }
    }
    return chain;
  };
  for (size_t p = 0; p < points.size(); ++p)
    if (adj[p].size() == 1 && !used[adj[p][0]])
      chains.push_back(walk((int)p, adj[p][0]));
  for (size_t s = 0; s < segs.size(); ++s)
    if (!used[s])
      chains.push_back(walk(segs[s].first, (int)s));
  // Clip to the face and build edges on its surface.
  BRepTopAdaptor_FClass2d    cls(F, 1e-9);
  auto inside = [&](const gp_Pnt2d& p) { return cls.Perform(p) != TopAbs_OUT; };
  auto boundary = [&](gp_Pnt2d a, gp_Pnt2d b) {
    // a inside, b outside
    for (int k = 0; k < 50; ++k)
    {
      gp_Pnt2d m(0.5 * (a.X() + b.X()), 0.5 * (a.Y() + b.Y()));
      if (inside(m))
        a = m;
      else
        b = m;
    }
    return extend ? b : a;
  };
  occ::handle<Geom_Surface> surf = BRep_Tool::Surface(F);
  auto                      out  = std::make_unique<OcShapeList>();
  const double              ext  = std::max(u1 - u0, v1 - v0);
  auto emit = [&](std::vector<gp_Pnt2d> pts, bool closed) {
    std::vector<gp_Pnt2d> clean;
    for (const auto& p : pts)
      if (clean.empty() || clean.back().Distance(p) > ext * 1e-9)
        clean.push_back(p);
    if (closed && clean.size() > 2 && clean.front().Distance(clean.back()) <= ext * 1e-9)
      clean.pop_back();
    if (clean.size() < 2)
      return;
    occ::handle<Geom2d_Curve> c2;
    try
    {
      if (clean.size() == 2)
      {
        gp_Vec2d dir2(clean[0], clean[1]);
        c2 = new Geom2d_TrimmedCurve(new Geom2d_Line(clean[0], gp_Dir2d(dir2)), 0.0, dir2.Magnitude());
      }
      else if (closed && clean.size() >= 3)
      {
        occ::handle<NCollection_HArray1<gp_Pnt2d>> hp = new NCollection_HArray1<gp_Pnt2d>(1, (int)clean.size());
        for (size_t i = 0; i < clean.size(); ++i)
          hp->SetValue((int)i + 1, clean[i]);
        Geom2dAPI_Interpolate ip(hp, true, ext * 1e-9);
        ip.Perform();
        if (ip.IsDone())
          c2 = ip.Curve();
      }
      else
      {
        NCollection_Array1<gp_Pnt2d> arr(1, (int)clean.size());
        for (size_t i = 0; i < clean.size(); ++i)
          arr((int)i + 1) = clean[i];
        Geom2dAPI_PointsToBSpline fit(arr, 3, 8, GeomAbs_C2, ext * 1e-7);
        if (fit.IsDone())
          c2 = fit.Curve();
      }
    }
    catch (const Standard_Failure&)
    {
      c2.Nullify();
    }
    if (c2.IsNull())
      return;
    BRepBuilderAPI_MakeEdge me(c2, surf, c2->FirstParameter(), c2->LastParameter());
    if (!me.IsDone())
      return;
    TopoDS_Edge e = me.Edge();
    BRepLib::BuildCurves3d(e);
    out->items.push_back(e);
  };
  for (const auto& ch : chains)
  {
    const bool closed = ch.size() > 2 && ch.front() == ch.back();
    std::vector<gp_Pnt2d> run;
    bool                  all_in = true;
    for (int id : ch)
      if (!inside(points[id]))
        all_in = false;
    if (all_in)
    {
      std::vector<gp_Pnt2d> pts;
      for (int id : ch)
        pts.push_back(points[id]);
      emit(pts, closed);
      continue;
    }
    for (size_t k = 0; k < ch.size(); ++k)
    {
      const gp_Pnt2d& p  = points[ch[k]];
      const bool      in = inside(p);
      if (in)
      {
        if (run.empty() && k > 0)
          run.push_back(boundary(p, points[ch[k - 1]]));
        run.push_back(p);
      }
      else if (!run.empty())
      {
        run.push_back(boundary(run.back(), p));
        emit(run, false);
        run.clear();
      }
    }
    if (!run.empty())
      emit(run, false);
  }
  return out;
}

// ---------------------------------------------------------------- surface fitting

std::unique_ptr<OcShape> bspline_grid_face(const PlaneF& frame, double x0, double dx, double y0, double dy,
                                           rust::Slice<const double> z, uint32_t nx, uint32_t ny, int32_t deg_max,
                                           double tol, rust::Slice<const double> bounds)
{
  if (nx < 2 || ny < 2 || z.size() != (size_t)nx * ny)
    fail_invalid("surface fit: grid needs at least 2 x 2 heights");
  if (!(dx > 0) || !(dy > 0))
    fail_invalid("surface fit: grid spacing must be positive");
  NCollection_Array2<double> Z(1, (int)nx, 1, (int)ny);
  for (uint32_t i = 0; i < nx; ++i)
    for (uint32_t j = 0; j < ny; ++j)
    {
      const double h = z[(size_t)i * ny + j];
      if (!std::isfinite(h))
        fail_invalid("surface fit: non-finite height");
      Z((int)i + 1, (int)j + 1) = h;
    }
  // Cubic C2 interpolation of the grid (stable; the least-squares
  // approximation of the Z-grid variant can oscillate between nodes).
  (void)deg_max;
  (void)tol;
  GeomAPI_PointsToBSplineSurface fit;
  fit.Interpolate(Z, x0, dx, y0, dy);
  if (!fit.IsDone())
    fail_op("surface fit: B-spline interpolation failed");
  occ::handle<Geom_BSplineSurface> s = fit.Surface();
  gp_Trsf                          T;
  T.SetDisplacement(gp_Ax3(), to_ax3(frame));
  s->Transform(T);
  double fb[4] = {x0, x0 + dx * (nx - 1), y0, y0 + dy * (ny - 1)};
  if (bounds.size() == 4)
    for (int i = 0; i < 4; ++i)
      fb[i] = bounds[i];
  BRepBuilderAPI_MakeFace mf(s, fb[0], fb[1], fb[2], fb[3], 1e-7);
  if (!mf.IsDone())
    fail_op("surface fit: could not build the face");
  return wrap(mf.Face());
}

double face_max_deviation(const OcShape& face, rust::Slice<const double> pts)
{
  const TopoDS_Face& F = as_face(face.shape, "face");
  if (pts.size() % 3 != 0)
    fail_invalid("points must be xyz triples");
  occ::handle<Geom_Surface> s = BRep_Tool::Surface(F);
  double                    u0, u1, v0, v1;
  BRepTools::UVBounds(F, u0, u1, v0, v1);
  GeomAPI_ProjectPointOnSurf proj;
  proj.Init(s, u0, u1, v0, v1);
  double worst = 0.0;
  for (size_t i = 0; i + 2 < pts.size(); i += 3)
  {
    gp_Pnt p(pts[i], pts[i + 1], pts[i + 2]);
    proj.Perform(p);
    if (proj.NbPoints() > 0)
      worst = std::max(worst, proj.LowerDistance());
  }
  return worst;
}

} // namespace noshape
