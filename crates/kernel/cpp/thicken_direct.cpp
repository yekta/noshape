// A sheet thickened to one side, built directly where its faces meet at sharp edges (c2-surf-workshop-split's Thicken 1:
// OCCT's intersection-join offset took 250-400 ms, live 29). Each face's offset, trimmed or extended to its
// neighbours' along the curve where their offsets cross, the walls ruled from the free edges to their offsets, sewn.
// Anything else (smooth or seam edges, a free corner whose walls don't lie in one plane) is left to OCCT.
#include "internal.hpp"
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFill.hxx>
#include <BRepLib.hxx>
#include <BRepTools.hxx>
#include <BRepTools_History.hxx>
#include <BRep_Builder.hxx>
#include <GProp_GProps.hxx>
#include <BRepGProp.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <Geom2dAPI_ProjectPointOnCurve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_OffsetSurface.hxx>
#include <Geom_Plane.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <math_Gauss.hxx>
#include <math_Matrix.hxx>
#include <math_Vector.hxx>
#include <NCollection_HArray1.hxx>

namespace noshape {

namespace {

using HHist = occ::handle<BRepTools_History>;

struct OffFace
{
  TopoDS_Face               face;
  occ::handle<Geom_Surface> off;
};

occ::handle<Geom_Surface> offset_surface(const TopoDS_Face& f, double d)
{
  const occ::handle<Geom_Surface> s  = BRep_Tool::Surface(f);
  const double                    od = f.Orientation() == TopAbs_REVERSED ? -d : d;
  if (const occ::handle<Geom_Plane> pl = occ::down_cast<Geom_Plane>(s); !pl.IsNull())
  {
    gp_Pln p = pl->Pln();
    p.Translate(gp_Vec(p.Axis().Direction()) * od);
    return new Geom_Plane(p);
  }
  return new Geom_OffsetSurface(s, od, true);
}

// Newton on n unknowns (the faces' u, v) for n equations; `f` fills the residual and Jacobian.
template <class F> bool newton(math_Vector& x, int n, F f)
{
  math_Vector r(1, n);
  math_Matrix j(1, n, 1, n);
  for (int it = 0; it < 40; ++it)
  {
    f(x, r, j);
    if (r.Norm() < 1e-11)
      return true;
    math_Gauss g(j);
    if (!g.IsDone())
      return false;
    math_Vector dx(1, n);
    g.Solve(r, dx);
    x -= dx;
  }
  f(x, r, j);
  return r.Norm() < 1e-9;
}

void d1(const occ::handle<Geom_Surface>& s, double u, double v, gp_Pnt& p, gp_Vec& du, gp_Vec& dv) { s->D1(u, v, p, du, dv); }

// Where the offsets of faces a and b cross the plane through `m` normal to `t`; x = (ua, va, ub, vb).
bool on_two(const occ::handle<Geom_Surface>& a, const occ::handle<Geom_Surface>& b, const gp_Pnt& m, const gp_Vec& t, math_Vector& x)
{
  return newton(x, 4, [&](const math_Vector& y, math_Vector& r, math_Matrix& j) {
    gp_Pnt pa, pb;
    gp_Vec ua, va, ub, vb;
    d1(a, y(1), y(2), pa, ua, va);
    d1(b, y(3), y(4), pb, ub, vb);
    const gp_Vec dd(pb, pa);
    for (int k = 1; k <= 3; ++k)
    {
      r(k)    = dd.Coord(k);
      j(k, 1) = ua.Coord(k);
      j(k, 2) = va.Coord(k);
      j(k, 3) = -ub.Coord(k);
      j(k, 4) = -vb.Coord(k);
    }
    r(4)    = gp_Vec(m, pa).Dot(t);
    j(4, 1) = ua.Dot(t);
    j(4, 2) = va.Dot(t);
    j(4, 3) = 0;
    j(4, 4) = 0;
  });
}

// The point on the offsets of faces a, b and c; x = (ua, va, ub, vb, uc, vc).
bool on_three(const occ::handle<Geom_Surface>& a, const occ::handle<Geom_Surface>& b, const occ::handle<Geom_Surface>& c, math_Vector& x)
{
  return newton(x, 6, [&](const math_Vector& y, math_Vector& r, math_Matrix& j) {
    gp_Pnt pa, pb, pc;
    gp_Vec ua, va, ub, vb, uc, vc;
    d1(a, y(1), y(2), pa, ua, va);
    d1(b, y(3), y(4), pb, ub, vb);
    d1(c, y(5), y(6), pc, uc, vc);
    j.Init(0);
    for (int k = 1; k <= 3; ++k)
    {
      r(k)        = pa.Coord(k) - pb.Coord(k);
      r(k + 3)    = pa.Coord(k) - pc.Coord(k);
      j(k, 1)     = ua.Coord(k);
      j(k, 2)     = va.Coord(k);
      j(k, 3)     = -ub.Coord(k);
      j(k, 4)     = -vb.Coord(k);
      j(k + 3, 1) = ua.Coord(k);
      j(k + 3, 2) = va.Coord(k);
      j(k + 3, 5) = -uc.Coord(k);
      j(k + 3, 6) = -vc.Coord(k);
    }
  });
}

gp_Pnt2d uv_of(const TopoDS_Vertex& v, const TopoDS_Face& f) { return BRep_Tool::Parameters(v, f); }

// The underlying 2D curve to extend a pcurve along: a line's own (unbounded); otherwise the pcurve as it is.
occ::handle<Geom2d_Curve> extensible(const occ::handle<Geom2d_Curve>& c)
{
  occ::handle<Geom2d_Curve> b = c;
  while (const occ::handle<Geom2d_TrimmedCurve> t = occ::down_cast<Geom2d_TrimmedCurve>(b))
    b = t->BasisCurve();
  return b->IsKind(STANDARD_TYPE(Geom2d_Line)) ? b : c;
}

struct VImage
{
  TopoDS_Vertex                                v;
  gp_Pnt                                       p;
  std::vector<std::pair<TopoDS_Face, gp_Pnt2d>> uv; // on each face's offset
  bool has(const TopoDS_Face& f, gp_Pnt2d& out) const
  {
    for (const auto& [g, q] : uv)
      if (g.IsSame(f))
      {
        out = q;
        return true;
      }
    return false;
  }
};

} // namespace

// `sheet`'s faces thickened by `d` along their normals (signed, one side), into `out` with `hist` (each face generating
// its offset, each edge and vertex its image, as OCCT's); false where it can't (the caller falls back to OCCT).
bool thicken_direct(const TopoDS_Shape& sheet, double d, TopoDS_Shape& out, HHist& hist)
{
  const double tol = 1e-7;
  ShapeIMap faces;
  TopExp::MapShapes(sheet, TopAbs_FACE, faces);
  if (faces.Extent() < 2 || faces.Extent() > 64)
    return false;
  ShapeAncMap ef, vf, ve;
  TopExp::MapShapesAndUniqueAncestors(sheet, TopAbs_EDGE, TopAbs_FACE, ef);
  TopExp::MapShapesAndUniqueAncestors(sheet, TopAbs_VERTEX, TopAbs_FACE, vf);
  TopExp::MapShapesAndUniqueAncestors(sheet, TopAbs_VERTEX, TopAbs_EDGE, ve);
  std::vector<OffFace> off;
  for (int i = 1; i <= faces.Extent(); ++i)
    off.push_back({TopoDS::Face(faces(i)), offset_surface(TopoDS::Face(faces(i)), d)});
  auto off_of = [&](const TopoDS_Shape& f) -> const occ::handle<Geom_Surface>& { return off[faces.FindIndex(f) - 1].off; };
  // The oriented normal of face f at uv.
  auto normal = [](const TopoDS_Face& f, const gp_Pnt2d& uv) {
    gp_Pnt p;
    gp_Vec du, dv;
    BRep_Tool::Surface(f)->D1(uv.X(), uv.Y(), p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (n.Magnitude() < 1e-12)
      return gp_Vec();
    n.Normalize();
    return f.Orientation() == TopAbs_REVERSED ? -n : n;
  };
  // Edges: free (one face) or sharp (two faces at an angle); smooth, seam and degenerated ones are OCCT's.
  std::vector<bool> sharp(ef.Extent() + 1, false);
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(ef.FindKey(i));
    if (BRep_Tool::Degenerated(e) || ef(i).Extent() > 2)
      return false;
    if (ef(i).Extent() == 1)
    {
      if (BRep_Tool::IsClosed(e, TopoDS::Face(ef(i).First())))
        return false;
      continue;
    }
    const TopoDS_Face fa = TopoDS::Face(ef(i).First()), fb = TopoDS::Face(ef(i).Last());
    double a0, a1;
    const occ::handle<Geom2d_Curve> pa = BRep_Tool::CurveOnSurface(e, fa, a0, a1);
    const occ::handle<Geom2d_Curve> pb = BRep_Tool::CurveOnSurface(e, fb, a0, a1);
    if (pa.IsNull() || pb.IsNull())
      return false;
    bool all = true;
    for (const double s : {0.1, 0.5, 0.9})
    {
      const double t  = a0 + (a1 - a0) * s;
      const gp_Vec na = normal(fa, pa->Value(t)), nb = normal(fb, pb->Value(t));
      if (na.Magnitude() < 0.5 || nb.Magnitude() < 0.5)
        return false;
      all = all && na.Angle(nb) > 0.01;
    }
    if (!all)
      return false;
    sharp[i] = true;
  }
  // Each vertex's image.
  std::vector<VImage> vimg(vf.Extent() + 1);
  for (int i = 1; i <= vf.Extent(); ++i)
  {
    const TopoDS_Vertex   v = TopoDS::Vertex(vf.FindKey(i));
    std::vector<TopoDS_Face> fs;
    for (const TopoDS_Shape& f : vf(i))
      fs.push_back(TopoDS::Face(f));
    std::vector<int> sh, fr;
    for (const TopoDS_Shape& e : ve.FindFromKey(v))
    {
      const int k = ef.FindIndex(e);
      (sharp[k] ? sh : fr).push_back(k);
    }
    VImage& im = vimg[i];
    if (fs.size() == 1)
    {
      const gp_Pnt2d uv = uv_of(v, fs[0]);
      im.p             = off_of(fs[0])->Value(uv.X(), uv.Y());
      im.uv.push_back({fs[0], uv});
    }
    else if (fs.size() == 3 && sh.size() >= 2)
    {
      math_Vector x(1, 6);
      for (int k = 0; k < 3; ++k)
      {
        const gp_Pnt2d uv = uv_of(v, fs[k]);
        x(2 * k + 1)      = uv.X();
        x(2 * k + 2)      = uv.Y();
      }
      if (!on_three(off_of(fs[0]), off_of(fs[1]), off_of(fs[2]), x))
        return false;
      im.p = off_of(fs[0])->Value(x(1), x(2));
      for (int k = 0; k < 3; ++k)
        im.uv.push_back({fs[k], gp_Pnt2d(x(2 * k + 1), x(2 * k + 2))});
    }
    else if (fs.size() == 2 && sh.size() == 1 && fr.size() == 2)
    {
      // A sharp edge reaching the free boundary: the image where its offsets' crossing meets the walls' plane (both
      // free edges' walls in one plane: their tangents and normals there).
      gp_Vec pn[2];
      for (int k = 0; k < 2; ++k)
      {
        const TopoDS_Edge  e = TopoDS::Edge(ef.FindKey(fr[k]));
        const TopoDS_Face  f = TopoDS::Face(ef(fr[k]).First());
        double             c0, c1;
        const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, c0, c1);
        if (c.IsNull())
          return false;
        const double t = BRep_Tool::Parameter(v, e);
        gp_Pnt       q;
        gp_Vec       tg;
        c->D1(t, q, tg);
        pn[k] = tg.Crossed(normal(f, uv_of(v, f)));
        if (pn[k].Magnitude() < 1e-12)
          return false;
        pn[k].Normalize();
      }
      if (pn[0].Crossed(pn[1]).Magnitude() > 1e-6)
        return false;
      const gp_Pnt vp = BRep_Tool::Pnt(v);
      math_Vector  x(1, 4);
      const gp_Pnt2d u0 = uv_of(v, fs[0]), u1 = uv_of(v, fs[1]);
      x(1) = u0.X();
      x(2) = u0.Y();
      x(3) = u1.X();
      x(4) = u1.Y();
      if (!on_two(off_of(fs[0]), off_of(fs[1]), vp, pn[0], x))
        return false;
      im.p = off_of(fs[0])->Value(x(1), x(2));
      im.uv.push_back({fs[0], gp_Pnt2d(x(1), x(2))});
      im.uv.push_back({fs[1], gp_Pnt2d(x(3), x(4))});
    }
    else
      return false;
    BRep_Builder bb;
    bb.MakeVertex(im.v, im.p, tol);
  }
  auto image_of_vertex = [&](const TopoDS_Vertex& v) -> const VImage& { return vimg[vf.FindIndex(v)]; };
  // Each edge's image, with its pcurves on the offsets.
  std::vector<TopoDS_Edge> eimg(ef.Extent() + 1);
  BRep_Builder             bb;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const TopoDS_Edge e = TopoDS::Edge(ef.FindKey(i).Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     v0, v1;
    TopExp::Vertices(e, v0, v1);
    const VImage &i0 = image_of_vertex(v0), &i1 = image_of_vertex(v1);
    if (!sharp[i])
    {
      // A free edge: its pcurve on the face, on the offset, between its ends' images.
      const TopoDS_Face             f = TopoDS::Face(ef(i).First());
      double                        a0, a1;
      const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, f, a0, a1);
      if (pc.IsNull())
        return false;
      gp_Pnt2d q0, q1;
      if (!i0.has(f, q0) || !i1.has(f, q1))
        return false;
      const occ::handle<Geom2d_Curve> ext = extensible(pc);
      auto at = [&](const gp_Pnt2d& q, double end, double& t) {
        if (q.Distance(pc->Value(end)) < 1e-12)
        {
          t = end;
          return true;
        }
        Geom2dAPI_ProjectPointOnCurve pr(q, ext);
        if (pr.NbPoints() == 0 || pr.LowerDistance() > 1e-7)
          return false;
        t = pr.LowerDistanceParameter();
        return true;
      };
      double t0, t1;
      if (!at(q0, a0, t0) || !at(q1, a1, t1) || !(t1 > t0))
        return false;
      BRepBuilderAPI_MakeEdge me(ext, off_of(f), i0.v, i1.v, t0, t1);
      if (!me.IsDone())
        return false;
      TopoDS_Edge img = me.Edge();
      if (!BRepLib::BuildCurve3d(img, 1e-7))
        return false;
      eimg[i] = img;
      continue;
    }
    // A sharp edge: where its faces' offsets cross, marched along it in the planes normal to it.
    const TopoDS_Face             fa = TopoDS::Face(ef(i).First()), fb = TopoDS::Face(ef(i).Last());
    double                        c0, c1;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, c0, c1);
    if (c.IsNull())
      return false;
    // The parameter whose normal plane holds `p`, from the vertex's own (a projection can land on another part of a
    // curved rim).
    auto param = [&](const gp_Pnt& p, double t) {
      for (int it = 0; it < 50; ++it)
      {
        gp_Pnt m;
        gp_Vec d1, d2;
        c->D2(t, m, d1, d2);
        const gp_Vec w(m, p);
        const double g = w.Dot(d1), dg = w.Dot(d2) - d1.SquareMagnitude();
        if (std::abs(dg) < 1e-30)
          break;
        const double step = g / dg;
        t -= step;
        if (std::abs(step) < 1e-14 * (1 + std::abs(t)))
          break;
      }
      return t;
    };
    const double t0 = param(i0.p, BRep_Tool::Parameter(v0, e)), t1 = param(i1.p, BRep_Tool::Parameter(v1, e));
    if (!(t1 > t0) || t0 < c0 - (c1 - c0) || t1 > c1 + (c1 - c0))
      return false;
    // (Each end's image beside the edge there, not on another part of it.)
    if (c->Value(t0).Distance(i0.p) > 10 * std::abs(d) || c->Value(t1).Distance(i1.p) > 10 * std::abs(d))
      return false;
    gp_Pnt2d s0, s1;
    if (!i0.has(fa, s0) || !i0.has(fb, s1))
      return false;
    gp_Pnt2d e0, e1;
    if (!i1.has(fa, e0) || !i1.has(fb, e1))
      return false;
    // Sampled more finely until the curves through the samples hold the crossing between them too.
    occ::handle<Geom_BSplineCurve>   cc;
    occ::handle<Geom2d_BSplineCurve> pca, pcb;
    for (int n = 16; n <= 256 && cc.IsNull(); n *= 2)
    {
      occ::handle<NCollection_HArray1<gp_Pnt>>   pts = new NCollection_HArray1<gp_Pnt>(1, n + 1);
      occ::handle<NCollection_HArray1<gp_Pnt2d>> qa  = new NCollection_HArray1<gp_Pnt2d>(1, n + 1);
      occ::handle<NCollection_HArray1<gp_Pnt2d>> qb  = new NCollection_HArray1<gp_Pnt2d>(1, n + 1);
      occ::handle<NCollection_HArray1<double>>   par = new NCollection_HArray1<double>(1, n + 1);
      std::vector<math_Vector>                   xs;
      math_Vector                                x(1, 4);
      x(1) = s0.X();
      x(2) = s0.Y();
      x(3) = s1.X();
      x(4) = s1.Y();
      for (int k = 0; k <= n; ++k)
      {
        const double t = t0 + (t1 - t0) * k / n;
        gp_Pnt       m;
        gp_Vec       tg;
        c->D1(t, m, tg);
        if (k == n)
        {
          x(1) = e0.X();
          x(2) = e0.Y();
          x(3) = e1.X();
          x(4) = e1.Y();
        }
        else if (k > 0 && !on_two(off_of(fa), off_of(fb), m, tg.Normalized(), x))
          return false;
        pts->SetValue(k + 1, k == 0 ? i0.p : k == n ? i1.p : off_of(fa)->Value(x(1), x(2)));
        qa->SetValue(k + 1, gp_Pnt2d(x(1), x(2)));
        qb->SetValue(k + 1, gp_Pnt2d(x(3), x(4)));
        par->SetValue(k + 1, t);
        xs.push_back(x);
      }
      GeomAPI_Interpolate   ip(pts, par, false, 1e-12);
      Geom2dAPI_Interpolate ia(qa, par, false, 1e-12), ib(qb, par, false, 1e-12);
      ip.Perform();
      ia.Perform();
      ib.Perform();
      if (!ip.IsDone() || !ia.IsDone() || !ib.IsDone())
        return false;
      bool holds = true;
      for (int k = 0; k < n && holds; ++k)
      {
        const double t = t0 + (t1 - t0) * (k + 0.5) / n;
        gp_Pnt       m;
        gp_Vec       tg;
        c->D1(t, m, tg);
        math_Vector y = xs[k];
        if (!on_two(off_of(fa), off_of(fb), m, tg.Normalized(), y))
          return false;
        const gp_Pnt   q  = off_of(fa)->Value(y(1), y(2));
        const gp_Pnt2d ua = ia.Curve()->Value(t), ub = ib.Curve()->Value(t);
        holds = ip.Curve()->Value(t).Distance(q) < tol && off_of(fa)->Value(ua.X(), ua.Y()).Distance(q) < tol &&
                off_of(fb)->Value(ub.X(), ub.Y()).Distance(q) < tol;
      }
      if (holds)
      {
        cc  = ip.Curve();
        pca = ia.Curve();
        pcb = ib.Curve();
      }
    }
    if (cc.IsNull())
      return false;
    TopoDS_Edge img;
    bb.MakeEdge(img, cc, tol);
    bb.Add(img, i0.v.Oriented(TopAbs_FORWARD));
    bb.Add(img, i1.v.Oriented(TopAbs_REVERSED));
    bb.Range(img, t0, t1);
    bb.UpdateEdge(img, pca, off_of(fa), TopLoc_Location(), tol);
    bb.UpdateEdge(img, pcb, off_of(fb), TopLoc_Location(), tol);
    bb.Range(img, off_of(fa), TopLoc_Location(), t0, t1);
    bb.Range(img, off_of(fb), TopLoc_Location(), t0, t1);
    BRepLib::SameParameter(img, tol);
    eimg[i] = img;
  }
  // The offset faces: each face's wires with its edges' images.
  std::vector<TopoDS_Face> fimg;
  for (const OffFace& o : off)
  {
    TopoDS_Face nf;
    bb.MakeFace(nf, o.off, tol);
    for (TopExp_Explorer w(o.face.Oriented(TopAbs_FORWARD), TopAbs_WIRE); w.More(); w.Next())
    {
      TopoDS_Wire nw;
      bb.MakeWire(nw);
      for (TopExp_Explorer x(w.Current().Oriented(TopAbs_FORWARD), TopAbs_EDGE); x.More(); x.Next())
        bb.Add(nw, eimg[ef.FindIndex(x.Current())].Oriented(x.Current().Orientation()));
      nw.Orientation(w.Current().Orientation());
      bb.Add(nf, nw);
    }
    nf.Orientation(o.face.Orientation());
    fimg.push_back(nf);
  }
  // The walls: ruled from each free edge to its image.
  std::vector<TopoDS_Face> walls;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    if (sharp[i])
      continue;
    TopoDS_Face w = BRepFill::Face(TopoDS::Edge(ef.FindKey(i).Oriented(TopAbs_FORWARD)), eimg[i]);
    if (w.IsNull())
      return false;
    // A wall in a plane (a straight or planar edge offset along it) on that plane, as live's.
    if (BRepBuilderAPI_MakeFace mf(BRepTools::OuterWire(w), true); mf.IsDone())
      w = mf.Face();
    walls.push_back(w);
  }
  BRepBuilderAPI_Sewing sew(1e-6);
  for (int i = 1; i <= faces.Extent(); ++i)
    sew.Add(faces(i));
  for (const TopoDS_Face& f : fimg)
    sew.Add(f);
  for (const TopoDS_Face& f : walls)
    sew.Add(f);
  sew.Perform();
  const TopoDS_Shape sewn = sew.SewedShape();
  if (sew.NbFreeEdges() > 0 || sew.NbMultipleEdges() > 0)
    return false;
  BRepBuilderAPI_MakeSolid ms;
  int                      shells = 0;
  for (TopExp_Explorer x(sewn, TopAbs_SHELL); x.More(); x.Next(), ++shells)
    ms.Add(TopoDS::Shell(x.Current()));
  if (shells != 1 || !ms.IsDone())
    return false;
  TopoDS_Solid so = TopoDS::Solid(ms.Shape());
  BRepLib::SameParameter(so, tol, true);
  BRepLib::UpdateTolerances(so);
  BRepLib::OrientClosedSolid(so);
  if (!BRepCheck_Analyzer(so).IsValid())
    return false;
  GProp_GProps g;
  BRepGProp::VolumeProperties(so, g);
  if (g.Mass() < 0)
    so.Reverse();
  if (!(std::abs(g.Mass()) > 0))
    return false;
  // As OCCT's: each face, edge and vertex as sewn, and generating its image.
  HHist h = new BRepTools_History();
  auto  now = [&](const TopoDS_Shape& s) { return sew.IsModifiedSubShape(s) ? sew.ModifiedSubShape(s) : s; };
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    const TopoDS_Shape m = now(faces(i));
    if (!m.IsSame(faces(i)))
      h->AddModified(faces(i), m);
    h->AddGenerated(faces(i), now(fimg[i - 1]));
  }
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const TopoDS_Shape m = now(ef.FindKey(i));
    if (!m.IsSame(ef.FindKey(i)))
      h->AddModified(ef.FindKey(i), m);
    h->AddGenerated(ef.FindKey(i), now(eimg[i]));
  }
  for (int i = 1; i <= vf.Extent(); ++i)
  {
    const TopoDS_Shape m = now(vf.FindKey(i));
    if (!m.IsSame(vf.FindKey(i)))
      h->AddModified(vf.FindKey(i), m);
    h->AddGenerated(vf.FindKey(i), now(vimg[i].v));
  }
  // Walls in one plane, one face (c2-surf-workshop-split's Thicken 1: the ring round the hole's foot, 11 faces as live).
  ShapeUpgrade_UnifySameDomain u(so, false, true, false);
  u.Build();
  if (!u.Shape().IsSame(so) && BRepCheck_Analyzer(u.Shape()).IsValid())
  {
    h->Merge(u.History());
    out = u.Shape();
  }
  else
    out = so;
  hist = h;
  return true;
}

} // namespace noshape
