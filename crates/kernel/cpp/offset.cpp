// A solid with each face moved along its outward normal by its own distance (0: it stays on its surface), its
// neighbours' offsets trimmed or run on to meet it: offset_solid. Faces of any kind, free-form ones as their exact
// offsets (Geom_OffsetSurface), on Shell's machinery (offset_quad.hpp). Only offsets that keep the solid's topology
// (no face vanishing, no edges crossing); anything else fails with its reason, the caller keeps its fallback.
#include "internal.hpp"
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepCheck_Result.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GC_MakeCircle.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_IntSS.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <GeomAdaptor_Curve.hxx>
#include <GeomLProp_SLProps.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_OffsetSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <GeomConvert.hxx>
#include <ShapeFix_Edge.hxx>
#include <TColgp_HArray1OfPnt.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Vec2d.hxx>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <set>
#include <sstream>

extern "C" int noshape_should_break();

#include "offset_quad.hpp"

namespace noshape {

thread_local std::string t_offset_report;
rust::String             last_offset_report() { return rust::String(t_offset_report); }

namespace {

using HHist = occ::handle<BRepTools_History>;

struct Failed
{
  std::string why;
};

[[noreturn]] void failed(const std::string& why) { throw Failed{why}; }

std::string face_name(int i) { return "face " + std::to_string(i); }

// Edge `e`'s curve on face `f` (forward), for the way `e` runs in `f`: two faces on one surface (a cylinder's strips)
// share it, and OCCT keeps such an edge's two curves there as a seam's.
occ::handle<Geom2d_Curve> curve_on(const TopoDS_Edge& e, const TopoDS_Face& f, double& p0, double& p1)
{
  const TopoDS_Face ff = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
  for (TopExp_Explorer x(ff, TopAbs_EDGE); x.More(); x.Next())
    if (x.Current().IsSame(e))
      return BRep_Tool::CurveOnSurface(TopoDS::Edge(x.Current()), ff, p0, p1);
  return BRep_Tool::CurveOnSurface(e, ff, p0, p1);
}

// The outward normal of face `f` at its point nearest `p` (its orientation applied).
gp_XYZ outward_normal(const TopoDS_Face& f, const occ::handle<Geom_Surface>& s, const gp_XYZ& p)
{
  GeomAPI_ProjectPointOnSurf pr(gp_Pnt(p), s);
  if (pr.NbPoints() == 0)
    return gp_XYZ();
  double u, v;
  pr.LowerDistanceParameters(u, v);
  GeomLProp_SLProps lp(s, u, v, 1, 1e-12);
  if (!lp.IsNormalDefined())
    return gp_XYZ();
  gp_XYZ n = lp.Normal().XYZ();
  return f.Orientation() == TopAbs_REVERSED ? n.Reversed() : n;
}

// Face `f`'s surface moved `D` along its outward normal, as Shell's quad::Surf (its own copies of the face's geometry:
// workers never share a surface).
quad::Surf offset_surf(const TopoDS_Face& f, double D, double dmax, double size, double tol, int index)
{
  const double        d = f.Orientation() == TopAbs_REVERSED ? -D : D; // along the natural normal
  BRepAdaptor_Surface s(f);
  quad::Surf          q;
  q.kind = s.GetType();
  if (q.kind == GeomAbs_Plane)
  {
    const gp_Ax3 ax = s.Plane().Position();
    gp_XYZ       n  = ax.XDirection().XYZ().Crossed(ax.YDirection().XYZ());
    q.off           = new Geom_Plane(gp_Pln(ax).Translated(gp_Vec(n * d)));
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    q.n = n;
    q.c = n.Dot(ax.Location().XYZ()) + D;
    return q;
  }
  const occ::handle<Geom_Surface> own = occ::down_cast<Geom_Surface>(BRep_Tool::Surface(f)->Copy());
  if (q.kind == GeomAbs_Sphere || q.kind == GeomAbs_Torus)
  {
    q.ax            = q.kind == GeomAbs_Sphere ? s.Sphere().Position() : s.Torus().Position();
    const double r0 = q.kind == GeomAbs_Sphere ? s.Sphere().Radius() : s.Torus().MinorRadius();
    q.ang           = q.kind == GeomAbs_Torus ? s.Torus().MajorRadius() : 0.0;
    q.r             = r0;
    if (d == 0)
    {
      q.off = own;
      return q;
    }
    // (The natural normal's way against the radius: a point moved along it, nearer the centre or farther.)
    gp_Pnt P;
    gp_Vec Du, Dv;
    s.D1(0.3, 0.3, P, Du, Dv);
    const gp_Vec N = Du.Crossed(Dv);
    if (N.Magnitude() < 1e-12)
      failed(face_name(index) + ": normal undefined");
    const double way = quad::eval(q, P.XYZ() + N.Normalized().XYZ() * (1e-3 * r0)) > 0 ? 1.0 : -1.0;
    q.r              = r0 + way * d;
    if (q.r <= 10 * tol)
      failed(face_name(index) + ": its offset collapses");
    if (q.kind == GeomAbs_Sphere)
      q.off = new Geom_SphericalSurface(q.ax, q.r);
    else
      q.off = new Geom_ToroidalSurface(q.ax, q.ang, q.r);
    return q;
  }
  if (q.kind == GeomAbs_Cylinder || q.kind == GeomAbs_Cone)
  {
    q.ax  = q.kind == GeomAbs_Cylinder ? s.Cylinder().Position() : s.Cone().Position();
    q.ang = q.kind == GeomAbs_Cone ? s.Cone().SemiAngle() : 0.0;
    q.r   = q.kind == GeomAbs_Cylinder ? s.Cylinder().Radius() : s.Cone().RefRadius();
    if (d == 0)
    {
      q.off = own;
      return q;
    }
    // (The point at (0, 0) moved along the natural normal; the offset's reference radius where its generator crosses
    // the frame's plane.)
    gp_Pnt P;
    gp_Vec Du, Dv;
    s.D1(0, 0, P, Du, Dv);
    const gp_Vec N = Du.Crossed(Dv);
    if (N.Magnitude() < 1e-12)
      failed(face_name(index) + ": normal undefined at its frame");
    const gp_XYZ O = q.ax.Location().XYZ(), Z = q.ax.Direction().XYZ(), X = q.ax.XDirection().XYZ();
    const gp_XYZ P1 = P.XYZ() + N.Normalized().XYZ() * d, G = Dv.Normalized().XYZ();
    const double gz = G.Dot(Z);
    if (std::abs(gz) < 1e-9)
      failed(face_name(index) + ": a cone's generator square to its axis");
    q.r = (P1 - G * ((P1 - O).Dot(Z) / gz) - O).Dot(X);
    if (q.r <= 10 * tol)
      failed(face_name(index) + ": its offset collapses");
    if (q.kind == GeomAbs_Cylinder)
      q.off = new Geom_CylindricalSurface(q.ax, q.r);
    else
      q.off = new Geom_ConicalSurface(q.ax, q.ang, q.r);
    return q;
  }
  // Free-form: the exact offset of the face's own surface, run on past its bounds (its own parameters kept) where a
  // neighbour's offset meets it there.
  q.general = true;
  q.base    = own;
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  q.own[0] = u0, q.own[1] = u1, q.own[2] = v0, q.own[3] = v1;
  // (An extruded curve or a Bezier patch as the B-spline it is, where that keeps its parameters: it can be run on.)
  occ::handle<Geom_BSplineSurface> as_bs = occ::down_cast<Geom_BSplineSurface>(own);
  if (as_bs.IsNull() && dmax > 0 && (q.kind == GeomAbs_SurfaceOfExtrusion || q.kind == GeomAbs_BezierSurface))
  {
    try
    {
      double U0, U1, V0, V1;
      own->Bounds(U0, U1, V0, V1);
      const double                    m = 0.5 * (v1 - v0) + 1e-3 * size;
      const occ::handle<Geom_Surface> t = q.kind == GeomAbs_SurfaceOfExtrusion
                                            ? occ::handle<Geom_Surface>(new Geom_RectangularTrimmedSurface(own, std::max(U0, u0 - (u1 - u0)), std::min(U1, u1 + (u1 - u0)), v0 - m, v1 + m))
                                            : own;
      occ::handle<Geom_BSplineSurface> c = GeomConvert::SurfaceToBSplineSurface(t);
      bool                             same = !c.IsNull();
      for (int a = 0; a <= 4 && same; ++a)
        for (int b = 0; b <= 4 && same; ++b)
        {
          const double pu = u0 + (u1 - u0) * a / 4, pv = v0 + (v1 - v0) * b / 4;
          same            = c->Value(pu, pv).Distance(own->Value(pu, pv)) <= tol;
        }
      if (same)
        as_bs = c;
    }
    catch (const Standard_Failure&)
    {
    }
  }
  if (auto bs = as_bs; !bs.IsNull() && !bs->IsUPeriodic() && !bs->IsVPeriodic() && dmax > 0)
  {
    const double len = std::min(std::max(100 * dmax, 1e-3 * size), 0.5 * size); // (as far as a vertex may move)
    try
    {
      occ::handle<Geom_BSplineSurface> ext = quad::continued(bs, len);
      if (ext.IsNull())
        ext = quad::linear_extension(bs, len);
      bool same = !ext.IsNull();
      for (int a = 0; a <= 4 && same; ++a)
        for (int b = 0; b <= 4 && same; ++b)
        {
          const double pu = u0 + (u1 - u0) * a / 4, pv = v0 + (v1 - v0) * b / 4;
          same            = ext->Value(pu, pv).Distance(bs->Value(pu, pv)) <= tol;
        }
      if (same)
      {
        // (Its own patch now the whole run-on surface: the march and the feet search it all.)
        q.base = ext;
        ext->Bounds(q.own[0], q.own[1], q.own[2], q.own[3]);
      }
    }
    catch (const Standard_Failure&)
    {
    }
  }
  q.sas = new ShapeAnalysis_Surface(q.base);
  q.d   = d;
  if (d == 0)
  {
    q.off = q.base;
    return q;
  }
  if (!own->IsCNu(1) || !own->IsCNv(1))
    failed(face_name(index) + ": a free-form face not smooth");
  // (It folds where offset towards a centre of curvature as far as it.)
  const int n = 12;
  for (int a = 0; a <= n; ++a)
    for (int b = 0; b <= n; ++b)
    {
      GeomLProp_SLProps lp(own, u0 + (u1 - u0) * a / n, v0 + (v1 - v0) * b / n, 2, 1e-12);
      if (!lp.IsNormalDefined())
        failed(face_name(index) + ": normal undefined");
      if (lp.IsCurvatureDefined() && std::max(lp.MaxCurvature() * d, lp.MinCurvature() * d) > 0.9)
        failed(face_name(index) + ": its offset folds");
    }
  q.off = new Geom_OffsetSurface(q.base, d);
  return q;
}

// Newton on curve `c` for the parameter of the point nearest `p`, from `s`.
bool param_on(const occ::handle<Geom_Curve>& c, const gp_Pnt& p, double& s)
{
  for (int it = 0; it < 40; ++it)
  {
    gp_Pnt C;
    gp_Vec d1, d2;
    c->D2(s, C, d1, d2);
    const gp_Vec w(C, p);
    const double g = w.Dot(d1), dg = w.Dot(d2) - d1.SquareMagnitude();
    if (std::abs(dg) < 1e-300)
      return false;
    const double ds = -g / dg;
    s += ds;
    if (std::abs(ds) <= 1e-14 * std::max(1.0, std::abs(s)))
      return true;
  }
  return false;
}

// Whether faces `a` and `b` are tangent along edge `e` (their outward normals within 1e-6 rad at nine points).
bool tangent_along(const TopoDS_Edge& e, const TopoDS_Face& a, const TopoDS_Face& b)
{
  double                          a0, a1, b0, b1;
  const occ::handle<Geom2d_Curve> pa = curve_on(e, a, a0, a1), pb = curve_on(e, b, b0, b1);
  if (pa.IsNull() || pb.IsNull())
    return false;
  const occ::handle<Geom_Surface> sa = BRep_Tool::Surface(a), sb = BRep_Tool::Surface(b);
  for (int w = 0; w <= 8; ++w)
  {
    const gp_Pnt2d qa = pa->Value(a0 + (a1 - a0) * w / 8), qb = pb->Value(b0 + (b1 - b0) * w / 8);
    gp_Pnt         P;
    gp_Vec         ua, va, ub, vb;
    sa->D1(qa.X(), qa.Y(), P, ua, va);
    sb->D1(qb.X(), qb.Y(), P, ub, vb);
    gp_Vec na = ua.Crossed(va), nb = ub.Crossed(vb);
    if (na.Magnitude() < 1e-300 || nb.Magnitude() < 1e-300)
      return false;
    if (a.Orientation() == TopAbs_REVERSED)
      na.Reverse();
    if (b.Orientation() == TopAbs_REVERSED)
      nb.Reverse();
    if (na.Angle(nb) > 1e-6)
      return false;
  }
  return true;
}

struct NewEdge
{
  TopoDS_Edge                                   e;
  std::map<int, occ::handle<Geom2d_Curve>>      pc;      // its curve on a face's new surface, by face, where known
  std::map<int, std::pair<occ::handle<Geom2d_Curve>, occ::handle<Geom2d_Curve>>> seam_pc; // a seam's two
  double                                        s0 = 0, s1 = 0;
};

TopoDS_Shape offset_solid_impl(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& moved, const std::vector<double>& dist,
                               int face_budget, HHist& hist, std::ostringstream& report)
{
  TopoDS_Shape so;
  int          nso = 0;
  for (TopExp_Explorer x(solid, TopAbs_SOLID); x.More(); x.Next(), ++nso)
    so = x.Current();
  if (nso != 1)
    failed("not one solid");
  ShapeIMap faces, edges, verts;
  TopExp::MapShapes(so, TopAbs_FACE, faces);
  TopExp::MapShapes(so, TopAbs_EDGE, edges);
  TopExp::MapShapes(so, TopAbs_VERTEX, verts);
  const int           nf = faces.Extent(), ne = edges.Extent(), nv = verts.Extent();
  std::vector<double> D((size_t)nf + 1, 0.0);
  std::vector<char>   removed((size_t)nf + 1);
  bool                any_removed = false;
  double              dmax        = 0;
  for (size_t k = 0; k < moved.size(); ++k)
  {
    const int i = faces.FindIndex(moved[k]);
    if (i == 0)
      failed("a face not on the solid");
    const double d = dist.size() == 1 ? dist[0] : dist[k];
    // (Removed (NaN): it stays where it is for its neighbours' offsets to end on, and isn't in the result.)
    if (std::isnan(d))
    {
      removed[(size_t)i] = 1;
      any_removed        = true;
      continue;
    }
    D[(size_t)i] = d;
    dmax         = std::max(dmax, std::abs(d));
  }
  Bnd_Box bb;
  BRepBndLib::Add(so, bb, false);
  const double size = std::sqrt(bb.SquareExtent());
  const double tol  = 1e-9 * size + 1e-12;
  ShapeAncMap  ef, vf, ve;
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_EDGE, TopAbs_FACE, ef);
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_VERTEX, TopAbs_FACE, vf);
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_VERTEX, TopAbs_EDGE, ve);
  auto t_lap = std::chrono::steady_clock::now();
  auto lap   = [&](const char* what) {
    const auto now = std::chrono::steady_clock::now();
    report << what << " " << std::chrono::duration<double, std::milli>(now - t_lap).count() << " ms\n";
    t_lap = now;
  };
  std::vector<quad::Surf>                sf((size_t)nf + 1);
  std::vector<occ::handle<Geom_Surface>> world((size_t)nf + 1); // each face's own surface, its location applied
  for (int i = 1; i <= nf; ++i)
  {
    const TopoDS_Face& f = TopoDS::Face(faces(i));
    sf[(size_t)i]        = offset_surf(f, D[(size_t)i], dmax, size, tol, i);
    world[(size_t)i]     = BRep_Tool::Surface(f);
  }
  // (A free-form face that stays but whose corners move: its run-on surface, its new edges can lie past the old.)
  std::vector<char> corners_moved((size_t)nf + 1);
  auto new_surface = [&](int i) { return D[(size_t)i] != 0 || (sf[(size_t)i].general && corners_moved[(size_t)i]) ? sf[(size_t)i].off : world[(size_t)i]; };
  std::vector<std::vector<int>> efi((size_t)ne + 1);
  for (int i = 1; i <= ne; ++i)
  {
    for (ShapeList::Iterator it(ef.FindFromKey(edges(i))); it.More(); it.Next())
      efi[(size_t)i].push_back(faces.FindIndex(it.Value()));
    if (efi[(size_t)i].empty() || efi[(size_t)i].size() > 2)
      failed("edge " + std::to_string(i) + ": not between one or two faces");
  }
  std::vector<char> degen((size_t)ne + 1), tangent((size_t)ne + 1);
  for (int i = 1; i <= ne; ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(edges(i));
    degen[(size_t)i]     = BRep_Tool::Degenerated(e);
    if (!degen[(size_t)i] && efi[(size_t)i].size() == 2)
      tangent[(size_t)i] = tangent_along(e, TopoDS::Face(faces(efi[(size_t)i][0])), TopoDS::Face(faces(efi[(size_t)i][1])));
    if (tangent[(size_t)i] && D[(size_t)efi[(size_t)i][0]] != D[(size_t)efi[(size_t)i][1]])
      failed("edge " + std::to_string(i) + ": tangent faces moved apart");
    if (degen[(size_t)i] && D[(size_t)efi[(size_t)i][0]] != 0)
      failed("edge " + std::to_string(i) + ": a pole on a moved face");
  }
  lap("surfaces");
  // The vertices' new places: on all their faces' offsets.
  std::vector<gp_XYZ> pts((size_t)nv + 1);
  for (int i = 1; i <= nv; ++i)
  {
    if (noshape_should_break() != 0)
      failed("stopped");
    const TopoDS_Vertex& v = TopoDS::Vertex(verts(i));
    const gp_XYZ         V = BRep_Tool::Pnt(v).XYZ();
    std::vector<int>     fs;
    bool                 any = false;
    for (ShapeList::Iterator it(vf.FindFromKey(v)); it.More(); it.Next())
    {
      fs.push_back(faces.FindIndex(it.Value()));
      any = any || D[(size_t)fs.back()] != 0;
    }
    gp_XYZ x = V;
    pts[(size_t)i] = x;
    if (!any)
      continue;
    // (A free-form face's feet from the vertex's own place on it: a helical flank has a sheet a turn away.)
    for (const int f : fs)
      if (sf[(size_t)f].general)
      {
        sf[(size_t)f].hint     = BRep_Tool::Parameters(v, TopoDS::Face(faces(f)));
        sf[(size_t)f].has_hint = true;
      }
    std::vector<int> es;
    for (ShapeList::Iterator it(ve.FindFromKey(v)); it.More(); it.Next())
      es.push_back(edges.FindIndex(it.Value()));
    // Two of its faces tangent along an edge through it: along that edge's offset (the vertex moved along their
    // common normal, then along the edge's tangent) to the third face's offset.
    bool done = false;
    for (const int k : es)
    {
      if (!tangent[(size_t)k] || done)
        continue;
      const int a = efi[(size_t)k][0], b = efi[(size_t)k][1];
      for (const int c : fs)
      {
        if (c == a || c == b)
          continue;
        const gp_XYZ n = outward_normal(TopoDS::Face(faces(a)), world[(size_t)a], V);
        BRepAdaptor_Curve cv(TopoDS::Edge(edges(k)));
        const double      s = BRep_Tool::Parameter(v, TopoDS::Edge(edges(k)));
        gp_Pnt            P;
        gp_Vec            T;
        cv.D1(s, P, T);
        if (n.Modulus() < 0.5 || T.Magnitude() < 1e-300)
          continue;
        gp_XYZ y = V + n * D[(size_t)a];
        if (quad::solve_line(sf[(size_t)c], y, T.Normalized().XYZ(), y, size))
        {
          x    = y;
          done = true;
          break;
        }
      }
    }
    if (!done && fs.size() >= 3)
    {
      std::vector<const quad::Surf*> all;
      for (const int f : fs)
        all.push_back(&sf[(size_t)f]);
      done = all.size() == 3 ? quad::solve3(all.data(), x, size) : quad::solve_all(all, x, size);
      if (!done && all.size() == 3)
      {
        x    = V;
        done = quad::solve3_damped(all.data(), x, size);
      }
    }
    if (!done && fs.size() == 2)
    {
      // A closed face's seam and one other face: along the seam's line on the closed face's offset (its parameters
      // kept) to the other's.
      for (const int k : es)
      {
        if (done)
          break;
        const TopoDS_Edge& e = TopoDS::Edge(edges(k));
        if (efi[(size_t)k].size() != 1)
          continue;
        const int                       a = efi[(size_t)k][0], c = fs[0] == a ? fs[1] : fs[0];
        double                          p0, p1;
        const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, TopoDS::Face(faces(a)), p0, p1);
        if (pc.IsNull())
          continue;
        const occ::handle<Geom_Surface>& g = sf[(size_t)a].off;
        double                           s = BRep_Tool::Parameter(v, e);
        auto                             at = [&](double t) {
          const gp_Pnt2d w = pc->Value(t);
          return g->Value(w.X(), w.Y()).XYZ();
        };
        const double h = 1e-7 * std::max(1.0, std::abs(p1 - p0));
        for (int it = 0; it < 60; ++it)
        {
          const double f0 = quad::eval(sf[(size_t)c], at(s));
          if (std::abs(f0) < 1e-13 * size)
          {
            x    = at(s);
            done = true;
            break;
          }
          const double df = (quad::eval(sf[(size_t)c], at(s + h)) - quad::eval(sf[(size_t)c], at(s - h))) / (2 * h);
          if (std::abs(df) < 1e-300)
            break;
          s -= f0 / df;
        }
      }
    }
    if (!done)
    {
      std::string what;
      for (const int f : fs)
        what += " " + std::to_string(f) + " (kind " + std::to_string((int)sf[(size_t)f].kind) + ", " + std::to_string(D[(size_t)f] * 1e3) + " mm)";
      for (const int k : es)
        what += " edge " + std::to_string(k) + (tangent[(size_t)k] ? " tangent" : "");
      failed("vertex " + std::to_string(i) + ": no point on its faces' offsets:" + what);
    }
    if ((x - V).Modulus() > 100 * dmax + 1e3 * tol)
      failed("vertex " + std::to_string(i) + ": its offset ran " + std::to_string((x - V).Modulus() * 1e3) + " mm off");
    pts[(size_t)i] = x;
  }
  lap("vertices");
  // Three-sided faces whose offset vanishes (an edge of them turned round: a sliver past a seam, its corners passing
  // each other): gone, their three neighbours meeting at one new vertex where their offsets do.
  std::vector<char> gone_face((size_t)nf + 1), gone_edge((size_t)ne + 1);
  std::vector<int>  vmap((size_t)nv + 1);
  for (int i = 1; i <= nv; ++i)
    vmap[(size_t)i] = i;
  auto ends = [&](int k) {
    TopoDS_Vertex a, b;
    TopExp::Vertices(TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), a, b);
    return std::make_pair(verts.FindIndex(a), verts.FindIndex(b));
  };
  for (int i = 1; i <= nf; ++i)
  {
    std::vector<int> fe;
    int              wires = 0;
    for (TopoDS_Iterator w(faces(i), false); w.More(); w.Next(), ++wires)
      for (TopoDS_Iterator x(w.Value(), false); x.More(); x.Next())
        fe.push_back(edges.FindIndex(x.Value()));
    if (wires != 1 || fe.size() != 3)
      continue;
    bool turned = false, plain = true;
    for (const int k : fe)
    {
      plain = plain && efi[(size_t)k].size() == 2 && !degen[(size_t)k] && !gone_edge[(size_t)k];
      if (!plain)
        break;
      const auto [a, b] = ends(k);
      const gp_XYZ was  = BRep_Tool::Pnt(TopoDS::Vertex(verts(b))).XYZ() - BRep_Tool::Pnt(TopoDS::Vertex(verts(a))).XYZ();
      turned            = turned || (pts[(size_t)b] - pts[(size_t)a]).Dot(was) < 0;
    }
    if (!plain || !turned)
      continue;
    std::vector<int> across;
    std::set<int>    vs;
    for (const int k : fe)
    {
      across.push_back(efi[(size_t)k][0] == i ? efi[(size_t)k][1] : efi[(size_t)k][0]);
      vs.insert(ends(k).first), vs.insert(ends(k).second);
    }
    if (std::set<int>(across.begin(), across.end()).size() != 3 || vs.size() != 3)
      failed(face_name(i) + ": its offset vanishes");
    gp_XYZ x(0, 0, 0);
    for (const int v : vs)
      x += pts[(size_t)v] / 3.0;
    const gp_XYZ start = x;
    for (const int f : across)
      if (sf[(size_t)f].general)
        for (const int v : vs)
          if (const TopoDS_Vertex& vv = TopoDS::Vertex(verts(v)); vf.FindFromKey(vv).Contains(faces(f)))
          {
            sf[(size_t)f].hint     = BRep_Tool::Parameters(vv, TopoDS::Face(faces(f)));
            sf[(size_t)f].has_hint = true;
            break;
          }
    const quad::Surf* s3[3] = {&sf[(size_t)across[0]], &sf[(size_t)across[1]], &sf[(size_t)across[2]]};
    if (!quad::solve3(s3, x, size) && !quad::solve3_damped(s3, (x = start), size))
      failed(face_name(i) + ": its offset vanishes, its neighbours' offsets don't meet");
    if ((x - start).Modulus() > 100 * dmax)
      failed(face_name(i) + ": its offset vanishes, its neighbours' offsets meet far off");
    gone_face[(size_t)i] = 1;
    for (const int k : fe)
      gone_edge[(size_t)k] = 1;
    const int to = *vs.begin();
    for (const int v : vs)
      vmap[(size_t)v] = to;
    pts[(size_t)to] = x;
    report << face_name(i) << " vanishes\n";
  }
  for (int i = 1; i <= nv; ++i)
    if ((pts[(size_t)vmap[(size_t)i]] - BRep_Tool::Pnt(TopoDS::Vertex(verts(i))).XYZ()).Modulus() > tol)
      for (ShapeList::Iterator it(vf.FindFromKey(verts(i))); it.More(); it.Next())
        corners_moved[(size_t)faces.FindIndex(it.Value())] = 1;
  // Removed faces go, and the edges between them.
  for (int i = 1; i <= nf; ++i)
    gone_face[(size_t)i] = gone_face[(size_t)i] || removed[(size_t)i];
  for (int k = 1; k <= ne; ++k)
    gone_edge[(size_t)k] = gone_edge[(size_t)k] || std::all_of(efi[(size_t)k].begin(), efi[(size_t)k].end(), [&](int f) { return gone_face[(size_t)f] != 0; });
  BRep_Builder               bld;
  std::vector<TopoDS_Vertex> vx((size_t)nv + 1);
  for (int i = 1; i <= nv; ++i)
    if (vmap[(size_t)i] == i)
      bld.MakeVertex(vx[(size_t)i], gp_Pnt(pts[(size_t)i]), 1e3 * tol);
  for (int i = 1; i <= nv; ++i)
    vx[(size_t)i] = vx[(size_t)vmap[(size_t)i]];
  lap("collapses");
  // The edges' offsets.
  std::vector<NewEdge> ex((size_t)ne + 1);
  std::vector<int>     work((size_t)nf + 1, 0);
  const double         fit_tol = 100 * tol;
  for (int i = 1; i <= ne; ++i)
  {
    if (noshape_should_break() != 0)
      failed("stopped");
    if (gone_edge[(size_t)i])
      continue;
    const TopoDS_Edge e = TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     va, vb;
    TopExp::Vertices(e, va, vb);
    const int            ia = vmap[(size_t)verts.FindIndex(va)], ib = vmap[(size_t)verts.FindIndex(vb)];
    const TopoDS_Vertex &A = vx[(size_t)ia], &B = vx[(size_t)ib];
    const gp_Pnt         pA(pts[(size_t)ia]), pB(pts[(size_t)ib]);
    NewEdge&             n  = ex[(size_t)i];
    const auto&          fi = efi[(size_t)i];
    const std::string    en = "edge " + std::to_string(i);
    if (degen[(size_t)i])
    {
      bld.MakeEdge(n.e);
      bld.Degenerated(n.e, true);
      bld.Add(n.e, A.Oriented(TopAbs_FORWARD));
      bld.Add(n.e, A.Oriented(TopAbs_REVERSED));
      double                          c0, c1;
      const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, TopoDS::Face(faces(fi[0])), c0, c1);
      if (pc.IsNull())
        failed(en + ": a pole without its curve");
      n.pc[fi[0]] = occ::down_cast<Geom2d_Curve>(pc->Copy());
      n.s0 = c0, n.s1 = c1;
      continue;
    }
    double                         c0, c1;
    const occ::handle<Geom_Curve>  oc = BRep_Tool::Curve(e, c0, c1);
    if (oc.IsNull())
      failed(en + ": no 3D curve");
    gp_Pnt o0;
    gp_Vec t0;
    oc->D1(c0, o0, t0);
    const bool closed = va.IsSame(vb);
    if (fi.size() == 1)
    {
      // A seam: its two curves on the closed face kept (the offset keeps the face's parameters), its 3D curve made from
      // them between its ends' new places.
      const int                 a  = fi[0];
      const TopoDS_Face         ff = TopoDS::Face(faces(a).Oriented(TopAbs_FORWARD));
      double                    p0, p1;
      occ::handle<Geom2d_Curve> c1f = BRep_Tool::CurveOnSurface(e, ff, p0, p1);
      occ::handle<Geom2d_Curve> c2r = BRep_Tool::CurveOnSurface(TopoDS::Edge(e.Reversed()), ff, p0, p1);
      if (c1f.IsNull() || c2r.IsNull())
        failed(en + ": a seam without its curves");
      const occ::handle<Geom_Surface> g = new_surface(a);
      double                          s0 = p0, s1 = p1;
      auto                            on = [&](const gp_Pnt& P, double& s) {
        for (int it = 0; it < 60; ++it)
        {
          const double h  = 1e-7 * std::max(1.0, std::abs(p1 - p0));
          auto         pt = [&](double t) {
            const gp_Pnt2d w = c1f->Value(t);
            return g->Value(w.X(), w.Y());
          };
          const gp_Vec T((pt(s + h).XYZ() - pt(s - h).XYZ()) / (2 * h));
          const gp_Vec w(pt(s), P);
          if (T.SquareMagnitude() < 1e-300)
            return false;
          const double ds = w.Dot(T) / T.SquareMagnitude();
          s += ds;
          if (std::abs(ds) <= 1e-13 * std::max(1.0, std::abs(s)))
            return pt(s).Distance(P) <= 1e3 * tol;
        }
        return false;
      };
      if (!on(pA, s0) || !on(pB, s1) || !(s1 > s0))
        failed(en + ": a seam's ends off its line");
      bld.MakeEdge(n.e);
      bld.UpdateEdge(n.e, occ::down_cast<Geom2d_Curve>(c1f->Copy()), occ::down_cast<Geom2d_Curve>(c2r->Copy()), g, TopLoc_Location(), 1e3 * tol);
      bld.Range(n.e, s0, s1);
      bld.Add(n.e, A.Oriented(TopAbs_FORWARD));
      bld.Add(n.e, B.Oriented(TopAbs_REVERSED));
      if (!BRepLib::BuildCurve3d(n.e, 1e-2 * fit_tol))
        failed(en + ": a seam's curve not made");
      n.seam_pc[a] = {c1f, c2r};
      n.s0 = s0, n.s1 = s1;
      continue;
    }
    const int a = fi[0], b = fi[1];
    occ::handle<Geom_Curve> c;
    double                  s0 = 0, s1 = 0;
    // The curve's parameters at the ends' new places, the same way round as the edge (a closed one a whole turn).
    auto trim = [&](const occ::handle<Geom_Curve>& cand, double& r0, double& r1) {
      GeomAPI_ProjectPointOnCurve qa(pA, cand), qb(pB, cand);
      if (qa.NbPoints() == 0 || qb.NbPoints() == 0)
        return false;
      r0 = qa.LowerDistanceParameter(), r1 = qb.LowerDistanceParameter();
      if (!param_on(cand, pA, r0) || !param_on(cand, pB, r1))
        return false;
      if (cand->Value(r0).Distance(pA) > 1e3 * tol || cand->Value(r1).Distance(pB) > 1e3 * tol)
        return false;
      if (cand->IsPeriodic())
      {
        const double per = cand->Period();
        if (closed)
          r1 = r0 + per;
        else
          while (r1 <= r0)
            r1 += per;
        while (r1 - r0 > per + 1e-12)
          r1 -= per;
      }
      return r1 > r0;
    };
    // The faces' offsets' intersection (a free-form one's only round the edge: its curve's box on the face and a few
    // distances more), its branch through the ends' new places the edge's way.
    auto near_edge = [&](int f) -> occ::handle<Geom_Surface> {
      const occ::handle<Geom_Surface>& g = sf[(size_t)f].off;
      if (!sf[(size_t)f].general || g->IsUPeriodic() || g->IsVPeriodic())
        return g;
      double                          e0, e1;
      const occ::handle<Geom2d_Curve> pc = curve_on(e, TopoDS::Face(faces(f)), e0, e1);
      if (pc.IsNull())
        return g;
      double u0 = 1e300, u1 = -1e300, v0 = 1e300, v1 = -1e300;
      for (int k = 0; k <= 16; ++k)
      {
        const gp_Pnt2d q = pc->Value(e0 + (e1 - e0) * k / 16);
        u0 = std::min(u0, q.X()), u1 = std::max(u1, q.X()), v0 = std::min(v0, q.Y()), v1 = std::max(v1, q.Y());
      }
      gp_Pnt P;
      gp_Vec du, dv;
      g->D1(0.5 * (u0 + u1), 0.5 * (v0 + v1), P, du, dv);
      if (du.Magnitude() < 1e-12 || dv.Magnitude() < 1e-12)
        return g;
      // (Past the edge's box by as far as its ends moved, twice, and a few distances.)
      const double reach = 2 * std::max(pA.Distance(BRep_Tool::Pnt(va)), pB.Distance(BRep_Tool::Pnt(vb))) + 4 * dmax;
      const double mu = reach / du.Magnitude(), mv = reach / dv.Magnitude();
      double       U0, U1, V0, V1;
      g->Bounds(U0, U1, V0, V1);
      u0 = std::max(U0, u0 - mu), u1 = std::min(U1, u1 + mu), v0 = std::max(V0, v0 - mv), v1 = std::min(V1, v1 + mv);
      if (!(u1 > u0) || !(v1 > v0))
        return g;
      return new Geom_RectangularTrimmedSurface(g, u0, u1, v0, v1);
    };
    auto intersect = [&]() {
      try
      {
        GeomAPI_IntSS is(near_edge(a), near_edge(b), 1e-9 * size);
        if (!is.IsDone())
          return false;
        for (int m = 1; m <= is.NbLines(); ++m)
        {
          occ::handle<Geom_Curve> cand = is.Line(m);
          if (auto tc = occ::down_cast<Geom_TrimmedCurve>(cand); !tc.IsNull())
            cand = tc->BasisCurve();
          for (int way = 0; way < 2; ++way)
          {
            double r0, r1;
            if (!trim(cand, r0, r1))
            {
              cand = cand->Reversed();
              continue;
            }
            gp_Pnt P;
            gp_Vec T;
            cand->D1(r0, P, T);
            if (T.Dot(t0) > 0)
            {
              c = cand, s0 = r0, s1 = r1;
              return true;
            }
            cand = cand->Reversed();
          }
        }
      }
      catch (const Standard_Failure&)
      {
      }
      return false;
    };
    if (D[(size_t)a] == 0 && D[(size_t)b] == 0)
    {
      // Both faces stay: the same curve, its ends where they now are.
      // (Past its curve's end: the faces' intersection, run on.)
      c = oc;
      if (!trim(c, s0, s1))
      {
        c = nullptr;
        if (!intersect())
          failed(en + ": its ends off its curve");
      }
      else
        for (const int f : fi)
        {
          double                          q0, q1;
          const occ::handle<Geom2d_Curve> pc = curve_on(e, TopoDS::Face(faces(f)), q0, q1);
          if (!pc.IsNull())
            n.pc[f] = occ::down_cast<Geom2d_Curve>(pc->Copy());
        }
    }
    else if (tangent[(size_t)i])
    {
      // Tangent faces moved alike: the edge's points moved along their common normal (a line or circle exactly, any
      // other curve fitted at its own parameters); on a face whose offset keeps its parameters, the same curves.
      const TopoDS_Face& Fa = TopoDS::Face(faces(a));
      auto moved_at = [&](double s) {
        const gp_XYZ p = oc->Value(s).XYZ();
        return gp_Pnt(p + outward_normal(Fa, world[(size_t)a], p) * D[(size_t)a]);
      };
      GeomAdaptor_Curve ga(oc, c0, c1);
      if (ga.GetType() == GeomAbs_Line)
        c = new Geom_Line(pA, gp_Dir(gp_Vec(pA, pB)));
      else if (ga.GetType() == GeomAbs_Circle)
      {
        GC_MakeCircle mc(moved_at(c0 + 0.1 * (c1 - c0)), moved_at(c0 + 0.5 * (c1 - c0)), moved_at(c0 + 0.9 * (c1 - c0)));
        if (!mc.IsDone())
          failed(en + ": a tangent circle's offset not made");
        c = mc.Value();
      }
      if (!c.IsNull())
      {
        if (!trim(c, s0, s1))
          failed(en + ": a tangent edge's offset off its ends");
      }
      else
      {
        const int                          np = 33;
        occ::handle<TColgp_HArray1OfPnt>   h3 = new TColgp_HArray1OfPnt(1, np);
        occ::handle<NCollection_HArray1<double>> hs = new NCollection_HArray1<double>(1, np);
        if (moved_at(c0).Distance(pA) > 1e3 * tol || moved_at(c1).Distance(pB) > 1e3 * tol)
          failed(en + ": a tangent edge's ends moved along it");
        for (int k = 0; k < np; ++k)
        {
          const double s = c0 + (c1 - c0) * k / (np - 1);
          h3->SetValue(k + 1, k == 0 ? pA : k == np - 1 ? pB : moved_at(s));
          hs->SetValue(k + 1, s);
        }
        GeomAPI_Interpolate in(h3, hs, false, 1e-15);
        in.Perform();
        if (!in.IsDone())
          failed(en + ": a tangent edge's offset not fitted");
        c  = in.Curve();
        s0 = c0, s1 = c1;
        // (A cone's offset is framed anew: its curve projected below.)
        for (const int f : fi)
          if (sf[(size_t)f].kind != GeomAbs_Cone)
          {
            double                          q0, q1;
            const occ::handle<Geom2d_Curve> pc = curve_on(e, TopoDS::Face(faces(f)), q0, q1);
            if (!pc.IsNull())
              n.pc[f] = occ::down_cast<Geom2d_Curve>(pc->Copy());
          }
      }
    }
    else if (sf[(size_t)a].general || sf[(size_t)b].general)
    {
      // A free-form face's: marched in its faces' parameters.
      const quad::Surf* S2[2] = {&sf[(size_t)a], &sf[(size_t)b]};
      const TopoDS_Face F2[2] = {TopoDS::Face(faces(a).Oriented(TopAbs_FORWARD)), TopoDS::Face(faces(b).Oriented(TopAbs_FORWARD))};
      quad::OffsetRun   run;
      bool              ok = false;
      try
      {
        ok = quad::march_offsets(S2, e, F2, pA, pB, size, fit_tol, run);
      }
      catch (const Standard_Failure&)
      {
        ok = false;
      }
      if (!ok)
      {
        if (!intersect())
          failed(en + ": its offset neither marched nor intersected (faces " + std::to_string(a) + " kind " + std::to_string((int)sf[(size_t)a].kind) + ", " + std::to_string(b) + " kind " + std::to_string((int)sf[(size_t)b].kind) + ")");
      }
      else
      {
        for (const int f : fi)
        {
          work[(size_t)f] += run.c3->NbPoles();
          if (face_budget > 0 && work[(size_t)f] > face_budget)
            failed(face_name(f) + ": over its budget (" + std::to_string(work[(size_t)f]) + " points)");
        }
        c  = run.c3;
        s0 = run.s0, s1 = run.s1;
        for (int g = 0; g < 2; ++g)
          if (!run.pc[g].IsNull())
            n.pc[fi[(size_t)g]] = run.pc[g];
      }
    }
    else if (sf[(size_t)a].kind == GeomAbs_Plane && sf[(size_t)b].kind == GeomAbs_Plane)
    {
      if (pA.Distance(pB) <= 10 * tol || gp_Vec(pA, pB).Dot(t0) <= 0)
        failed(en + ": its offset vanishes or turns round");
      c  = new Geom_Line(pA, gp_Dir(gp_Vec(pA, pB)));
      s0 = 0, s1 = pA.Distance(pB);
    }
    else
    {
      // Two analytic offsets: their intersection's branch through the ends' new places.
      if (!intersect())
        failed(en + ": no branch of its faces' offsets' intersection through its ends");
    }
    gp_Pnt P;
    gp_Vec T;
    c->D1(s0, P, T);
    if (!(s1 > s0) || T.Dot(t0) <= 0)
      failed(en + ": its offset turns round (faces " + std::to_string(a) + ", " + std::to_string(b) + ", " + std::to_string(pA.Distance(pB) * 1e3) + " mm long, was " + std::to_string(BRep_Tool::Pnt(va).Distance(BRep_Tool::Pnt(vb)) * 1e3) + ")");
    bld.MakeEdge(n.e, c, 1e3 * tol);
    bld.Range(n.e, s0, s1);
    bld.Add(n.e, A.Oriented(TopAbs_FORWARD));
    bld.Add(n.e, B.Oriented(TopAbs_REVERSED));
    n.s0 = s0, n.s1 = s1;
  }
  lap("edges");
  // The faces: each one's loops of new edges on its new surface.
  TopoDS_Shell shell;
  bld.MakeShell(shell);
  hist = new BRepTools_History();
  std::vector<TopoDS_Face> nfs((size_t)nf + 1);
  for (int i = 1; i <= nf; ++i)
  {
    const TopoDS_Face& f = TopoDS::Face(faces(i));
    if (gone_face[(size_t)i])
    {
      hist->Remove(f);
      continue;
    }
    const TopoDS_Face               ff = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    const occ::handle<Geom_Surface> g = new_surface(i);
    TopoDS_Face                     of;
    bld.MakeFace(of, g, 1e3 * tol);
    for (TopoDS_Iterator w(ff, false); w.More(); w.Next())
    {
      TopoDS_Wire nw;
      bld.MakeWire(nw);
      for (TopoDS_Iterator x(w.Value(), false); x.More(); x.Next())
        if (const int k = edges.FindIndex(x.Value()); !gone_edge[(size_t)k])
          bld.Add(nw, ex[(size_t)k].e.Oriented(x.Value().Orientation()));
      bld.Add(of, nw.Oriented(w.Value().Orientation()));
    }
    std::set<int> done;
    for (TopExp_Explorer x(ff, TopAbs_EDGE); x.More(); x.Next())
    {
      const int k = edges.FindIndex(x.Current());
      if (gone_edge[(size_t)k] || !done.insert(k).second)
        continue;
      NewEdge& n = ex[(size_t)k];
      if (const auto sp = n.seam_pc.find(i); sp != n.seam_pc.end())
        continue; // (made on this surface)
      if (const auto pc = n.pc.find(i); pc != n.pc.end())
      {
        bld.UpdateEdge(n.e, pc->second, of, 1e3 * tol);
        bld.Range(n.e, of, n.s0, n.s1);
        continue;
      }
      if (sf[(size_t)i].kind != GeomAbs_Plane)
      {
        ShapeFix_Edge fe;
        fe.FixAddPCurve(n.e, of, false, 1e3 * tol);
        double                    q0, q1;
        occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(n.e, of, q0, q1);
        if (pc.IsNull())
          failed("edge " + std::to_string(k) + ": no curve on " + face_name(i));
        // (Projected onto a closed surface it can come out a period off the old one, its loop then open: moved back
        // to where the old one starts (the offset keeps the face's parameters but for a cone's v).)
        double                          o0, o1;
        const TopoDS_Edge               oe = TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD));
        const occ::handle<Geom2d_Curve> op = curve_on(oe, ff, o0, o1);
        if (!op.IsNull() && (g->IsUPeriodic() || g->IsVPeriodic()))
        {
          const gp_Pnt2d a = pc->Value(q0), b = op->Value(o0);
          const double   du = g->IsUPeriodic() ? std::round((b.X() - a.X()) / g->UPeriod()) * g->UPeriod() : 0.0;
          const double   dv = g->IsVPeriodic() ? std::round((b.Y() - a.Y()) / g->VPeriod()) * g->VPeriod() : 0.0;
          if (du != 0 || dv != 0)
          {
            pc = occ::down_cast<Geom2d_Curve>(pc->Translated(gp_Vec2d(du, dv)));
            bld.UpdateEdge(n.e, pc, of, BRep_Tool::Tolerance(n.e));
            bld.Range(n.e, of, q0, q1);
          }
        }
      }
    }
    nfs[(size_t)i] = TopoDS::Face(of.Oriented(f.Orientation()));
    bld.Add(shell, nfs[(size_t)i]);
    hist->AddModified(f, nfs[(size_t)i]);
  }
  for (int i = 1; i <= ne; ++i)
    if (gone_edge[(size_t)i])
      hist->Remove(edges(i));
    else
      hist->AddModified(edges(i), ex[(size_t)i].e);
  for (int i = 1; i <= nv; ++i)
    hist->AddModified(verts(i), vx[(size_t)i]);
  // (With faces removed: the open shell of the others.)
  TopoDS_Shape out = shell;
  if (!any_removed)
  {
    TopoDS_Solid solid_out;
    bld.MakeSolid(solid_out);
    bld.Add(solid_out, shell);
    out = solid_out;
  }
  lap("faces");
  BRepLib::SameParameter(out, 1e3 * tol, true);
  for (int k = 1; k <= ne; ++k)
    if (!gone_edge[(size_t)k] && BRep_Tool::Tolerance(ex[(size_t)k].e) > 1e4 * tol)
    {
      report << "edge " << k << " (faces";
      for (const int f : efi[(size_t)k])
        report << " " << f;
      report << ") tolerance " << BRep_Tool::Tolerance(ex[(size_t)k].e) * 1e3 << " mm\n";
    }
  for (int i = 1; i <= nf; ++i)
  {
    if (gone_face[(size_t)i])
      continue;
    double t = BRep_Tool::Tolerance(nfs[(size_t)i]);
    for (TopExp_Explorer x(nfs[(size_t)i], TopAbs_EDGE); x.More(); x.Next())
      t = std::max(t, BRep_Tool::Tolerance(TopoDS::Edge(x.Current())));
    report << face_name(i) << " moved " << D[(size_t)i] * 1e3 << " mm, tolerance " << t * 1e3 << " mm\n";
  }
  lap("same parameter");
  if (!BRepCheck_Analyzer(out).IsValid())
  {
    std::string bad;
    for (int i = 1; i <= nf && bad.empty(); ++i)
      if (!gone_face[(size_t)i] && !BRepCheck_Analyzer(nfs[(size_t)i]).IsValid())
      {
        bad = face_name(i);
        BRepCheck_Analyzer an(nfs[(size_t)i]);
        for (TopExp_Explorer x(nfs[(size_t)i], TopAbs_WIRE); x.More(); x.Next())
          if (const auto& rs = an.Result(x.Current()); !rs.IsNull())
            for (const BRepCheck_Status st : rs->StatusOnShape(nfs[(size_t)i]))
              if (st != BRepCheck_NoError)
                bad += ", wire status " + std::to_string((int)st);
        if (const auto& rs = an.Result(nfs[(size_t)i]); !rs.IsNull())
          for (const BRepCheck_Status st : rs->Status())
            if (st != BRepCheck_NoError)
              bad += ", status " + std::to_string((int)st);
        for (TopExp_Explorer x(nfs[(size_t)i], TopAbs_EDGE); x.More(); x.Next())
          if (!BRepCheck_Analyzer(x.Current()).IsValid())
            bad += ", edge " + std::to_string(edges.FindIndex(x.Current()));
      }
    if (std::getenv("NOSHAPE_OFFSET_DEBUG"))
      for (int i = 1; i <= nf; ++i)
        if (!gone_face[(size_t)i] && !BRepCheck_Analyzer(nfs[(size_t)i]).IsValid())
          for (TopExp_Explorer w(nfs[(size_t)i], TopAbs_WIRE); w.More(); w.Next())
            for (TopoDS_Iterator x(w.Current()); x.More(); x.Next())
            {
              const TopoDS_Edge&              e = TopoDS::Edge(x.Value());
              double                          q0, q1;
              const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, nfs[(size_t)i], q0, q1);
              const bool                      rv = e.Orientation() == TopAbs_REVERSED;
              TopoDS_Vertex                   a, b;
              TopExp::Vertices(e, a, b, true);
              const gp_Pnt2d u0 = pc.IsNull() ? gp_Pnt2d() : pc->Value(rv ? q1 : q0), u1 = pc.IsNull() ? gp_Pnt2d() : pc->Value(rv ? q0 : q1);
              std::fprintf(stderr, "offset_solid: face %d edge %d%s 2d (%.6g %.6g) -> (%.6g %.6g), 3d (%.4f %.4f %.4f) -> (%.4f %.4f %.4f) tol %g\n", i, edges.FindIndex(e), rv ? " rev" : "", u0.X(), u0.Y(), u1.X(), u1.Y(), BRep_Tool::Pnt(a).X() * 1e3, BRep_Tool::Pnt(a).Y() * 1e3, BRep_Tool::Pnt(a).Z() * 1e3, BRep_Tool::Pnt(b).X() * 1e3, BRep_Tool::Pnt(b).Y() * 1e3, BRep_Tool::Pnt(b).Z() * 1e3, BRep_Tool::Tolerance(e) * 1e3);
            }
    failed("the result not valid" + (bad.empty() ? std::string() : " (" + bad + ")"));
  }
  lap("check");
  if (any_removed)
    return out;
  // A volume that moved the other way from the faces' areas times their distances: turned inside out somewhere.
  // (Adaptive to 1e-3: the test is loose, and the fixed-order integration took 30 ms a solid of B-spline faces,
  // c2-fan-blade-sweep's offset thread copies.)
  GProp_GProps p0, p1;
  BRepGProp::VolumeProperties(so, p0, 1e-3);
  BRepGProp::VolumeProperties(out, p1, 1e-3);
  double change = 0, scale = 0;
  for (int i = 1; i <= nf; ++i)
  {
    if (D[(size_t)i] == 0)
      continue;
    GProp_GProps a;
    BRepGProp::SurfaceProperties(faces(i), a);
    change += a.Mass() * D[(size_t)i];
    scale += a.Mass() * std::abs(D[(size_t)i]);
  }
  report << "volume " << p0.Mass() * 1e9 << " -> " << p1.Mass() * 1e9 << " mm3 (first order " << (p0.Mass() + change) * 1e9 << ")\n";
  if (p1.Mass() <= 0 || std::abs(p1.Mass() - p0.Mass() - change) > 0.5 * scale + 1e-9 * p0.Mass())
    failed("its volume off the faces' moves");
  lap("volumes");
  return out;
}

} // namespace

OpOut op_offset_solid(const OcShape& s, const OcShapeList& faces, rust::Slice<const double> distances, uint32_t face_budget)
{
  require_non_null(s.shape, "shape");
  if (distances.size() != 1 && distances.size() != faces.items.size())
    fail_invalid("distances must be one value or one per face");
  t_offset_report.clear();
  // (A case to replay: NOSHAPE_OFFSET_DEBUG=<dir> writes each input and its moved faces there.)
  if (const char* dir = std::getenv("NOSHAPE_OFFSET_DEBUG"); dir != nullptr && dir[0] == '/')
  {
    static std::atomic<int> seq{0};
    const std::string       base = std::string(dir) + "/offset-" + std::to_string(++seq);
    BRepTools::Write(s.shape, (base + ".brep").c_str());
    ShapeIMap all;
    TopExp::MapShapes(s.shape, TopAbs_FACE, all);
    if (FILE* f = std::fopen((base + ".txt").c_str(), "w"))
    {
      for (size_t k = 0; k < faces.items.size(); ++k)
        std::fprintf(f, "%d %.17g\n", all.FindIndex(faces.items[k]) - 1, distances.size() == 1 ? distances[0] : distances[k]);
      std::fclose(f);
    }
  }
  std::vector<double>       d(distances.begin(), distances.end());
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     h;
  std::ostringstream        report;
  try
  {
    const auto         t0 = std::chrono::steady_clock::now();
    const TopoDS_Shape r = offset_solid_impl(s.shape, faces.items, d, (int)face_budget, h, report);
    const auto         t1 = std::chrono::steady_clock::now();
    OpOut              o  = make_result(args, r, h.get());
    report << "offset " << std::chrono::duration<double, std::milli>(t1 - t0).count() << " ms, result and history " << std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t1).count() << " ms\n";
    t_offset_report      = report.str();
    if (std::getenv("NOSHAPE_OFFSET_DEBUG"))
      std::fprintf(stderr, "offset_solid: done\n%s", t_offset_report.c_str());
    return o;
  }
  catch (const Failed& f)
  {
    t_offset_report = report.str() + f.why;
    if (std::getenv("NOSHAPE_OFFSET_DEBUG"))
      std::fprintf(stderr, "offset_solid: failed: %s\n", t_offset_report.c_str());
    fail_op("offset: " + f.why);
  }
}

} // namespace noshape
