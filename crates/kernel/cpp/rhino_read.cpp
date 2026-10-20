// Rhino .3dm import through openNURBS, as Onshape imports Rhino files:
// Breps (and extrusions, bare surfaces) as solids or surfaces with their
// trims, meshes as mesh bodies, curves as wire bodies, block instances
// placed; each object with its name and display colour, in metres. Without
// openNURBS (agents' dev builds) the reader reports that it isn't available.

#include "internal.hpp"

#if defined(NOSHAPE_OPENNURBS)

#include <BRepLib.hxx>
#include <BSplCLib.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <TopoDS_Compound.hxx>

#include "opennurbs.h"

#include <map>
#include <mutex>

namespace noshape {

namespace {

void on_begin_read()
{
  static std::once_flag once;
  std::call_once(once, [] { ON::Begin(); });
}

// openNURBS knots (count = cv + order - 2) -> OCCT distinct knots and multiplicities.
void occt_knots(const double* k, int n, int order, std::vector<double>& knots, std::vector<int>& mults)
{
  std::vector<double> flat;
  flat.push_back(k[0]);
  for (int i = 0; i < n; ++i)
    flat.push_back(k[i]);
  flat.push_back(k[n - 1]);
  (void)order;
  for (double x : flat)
  {
    if (!knots.empty() && std::abs(x - knots.back()) <= 1e-14 * std::max(1.0, std::abs(x)))
      ++mults.back();
    else
    {
      knots.push_back(x);
      mults.push_back(1);
    }
  }
}

template <class T> NCollection_Array1<T> arr(const std::vector<T>& v)
{
  NCollection_Array1<T> a(1, (int)v.size());
  for (size_t i = 0; i < v.size(); ++i)
    a((int)i + 1) = v[i];
  return a;
}

occ::handle<Geom_BSplineCurve> curve3d(const ON_Curve& c, double scale)
{
  ON_NurbsCurve nc;
  if (!c.GetNurbForm(nc))
    fail_op("rhino: a curve without a NURBS form");
  std::vector<double> k;
  std::vector<int>    m;
  occt_knots(nc.m_knot, nc.KnotCount(), nc.m_order, k, m);
  const int                  n = nc.m_cv_count;
  NCollection_Array1<gp_Pnt> p(1, n);
  NCollection_Array1<double> w(1, n);
  for (int i = 0; i < n; ++i)
  {
    ON_4dPoint q;
    nc.GetCV(i, q);
    const double wt = nc.m_is_rat ? q.w : 1.0;
    p(i + 1)        = gp_Pnt(q.x / wt * scale, q.y / wt * scale, q.z / wt * scale);
    w(i + 1)        = wt;
  }
  return nc.m_is_rat ? new Geom_BSplineCurve(p, w, arr(k), arr(m), nc.m_order - 1) : new Geom_BSplineCurve(p, arr(k), arr(m), nc.m_order - 1);
}

occ::handle<Geom2d_BSplineCurve> curve2d(const ON_Curve& c)
{
  ON_NurbsCurve nc;
  if (!c.GetNurbForm(nc))
    fail_op("rhino: a trim curve without a NURBS form");
  std::vector<double> k;
  std::vector<int>    m;
  occt_knots(nc.m_knot, nc.KnotCount(), nc.m_order, k, m);
  const int                    n = nc.m_cv_count;
  NCollection_Array1<gp_Pnt2d> p(1, n);
  NCollection_Array1<double>   w(1, n);
  for (int i = 0; i < n; ++i)
  {
    ON_4dPoint q;
    nc.GetCV(i, q);
    const double wt = nc.m_is_rat ? q.w : 1.0;
    p(i + 1)        = gp_Pnt2d(q.x / wt, q.y / wt);
    w(i + 1)        = wt;
  }
  return nc.m_is_rat ? new Geom2d_BSplineCurve(p, w, arr(k), arr(m), nc.m_order - 1) : new Geom2d_BSplineCurve(p, arr(k), arr(m), nc.m_order - 1);
}

occ::handle<Geom_BSplineSurface> surface(const ON_Surface& s, double scale)
{
  ON_NurbsSurface ns;
  if (!s.GetNurbForm(ns))
    fail_op("rhino: a surface without a NURBS form");
  std::vector<double> uk, vk;
  std::vector<int>    um, vm;
  occt_knots(ns.m_knot[0], ns.KnotCount(0), ns.m_order[0], uk, um);
  occt_knots(ns.m_knot[1], ns.KnotCount(1), ns.m_order[1], vk, vm);
  const int                  nu = ns.m_cv_count[0], nv = ns.m_cv_count[1];
  NCollection_Array2<gp_Pnt> p(1, nu, 1, nv);
  NCollection_Array2<double> w(1, nu, 1, nv);
  for (int i = 0; i < nu; ++i)
    for (int j = 0; j < nv; ++j)
    {
      ON_4dPoint q;
      ns.GetCV(i, j, q);
      const double wt = ns.m_is_rat ? q.w : 1.0;
      p(i + 1, j + 1) = gp_Pnt(q.x / wt * scale, q.y / wt * scale, q.z / wt * scale);
      w(i + 1, j + 1) = wt;
    }
  return ns.m_is_rat ? new Geom_BSplineSurface(p, w, arr(uk), arr(vk), arr(um), arr(vm), ns.m_order[0] - 1, ns.m_order[1] - 1)
                     : new Geom_BSplineSurface(p, arr(uk), arr(vk), arr(um), arr(vm), ns.m_order[0] - 1, ns.m_order[1] - 1);
}

// openNURBS edges and trims are proxies: a sub-domain of their curve,
// possibly reversed. The 3D curve over the edge's part of it, in the edge's
// direction.
occ::handle<Geom_BSplineCurve> edge_curve(const ON_BrepEdge& e, double scale)
{
  occ::handle<Geom_BSplineCurve> c   = curve3d(*e.EdgeCurveOf(), scale);
  const ON_Interval              dom = e.ProxyCurveDomain();
  if (dom[0] > c->FirstParameter() + 1e-12 || dom[1] < c->LastParameter() - 1e-12)
    c->Segment(std::max(dom[0], c->FirstParameter()), std::min(dom[1], c->LastParameter()));
  if (e.ProxyCurveIsReversed())
    c->Reverse();
  return c;
}

// A trim's 2D curve running along its edge, reparametrized onto the edge's
// range [t0, t1] (OCCT's pcurves share the edge's parameter; SameParameter
// refines the match).
occ::handle<Geom2d_BSplineCurve> trim_curve(const ON_BrepTrim& t, bool along_edge, double t0, double t1)
{
  occ::handle<Geom2d_BSplineCurve> c   = curve2d(*t.TrimCurveOf());
  const ON_Interval                dom = t.ProxyCurveDomain();
  if (dom[0] > c->FirstParameter() + 1e-12 || dom[1] < c->LastParameter() - 1e-12)
    c->Segment(std::max(dom[0], c->FirstParameter()), std::min(dom[1], c->LastParameter()));
  if (t.ProxyCurveIsReversed())
    c->Reverse();
  if (along_edge && t.m_bRev3d)
    c->Reverse();
  if (t1 > t0)
  {
    NCollection_Array1<double> k = c->Knots();
    BSplCLib::Reparametrize(t0, t1, k);
    c->SetKnots(k);
  }
  return c;
}

// An ON_Brep as OCCT faces sharing edges; a solid when it is closed.
TopoDS_Shape brep_shape(const ON_Brep& b, double scale, double tol)
{
  BRep_Builder               B;
  std::vector<TopoDS_Vertex> verts(b.m_V.Count());
  for (int i = 0; i < b.m_V.Count(); ++i)
  {
    const ON_3dPoint& q = b.m_V[i].point;
    B.MakeVertex(verts[i], gp_Pnt(q.x * scale, q.y * scale, q.z * scale), std::max(tol, b.m_V[i].m_tolerance * scale));
  }
  std::vector<TopoDS_Edge> edges(b.m_E.Count());
  for (int i = 0; i < b.m_E.Count(); ++i)
  {
    const ON_BrepEdge& e = b.m_E[i];
    const ON_Curve*    c = e.EdgeCurveOf();
    TopoDS_Edge        E;
    if (c)
    {
      occ::handle<Geom_BSplineCurve> gc = edge_curve(e, scale);
      B.MakeEdge(E, gc, std::max(tol, e.m_tolerance * scale));
      B.Range(E, gc->FirstParameter(), gc->LastParameter());
    }
    else
      B.MakeEdge(E);
    TopoDS_Vertex v0 = verts[e.m_vi[0]], v1 = verts[e.m_vi[1]];
    v0.Orientation(TopAbs_FORWARD);
    v1.Orientation(TopAbs_REVERSED);
    B.Add(E, v0);
    B.Add(E, v1);
    edges[i] = E;
  }
  TopoDS_Shell shell;
  B.MakeShell(shell);
  for (int fi = 0; fi < b.m_F.Count(); ++fi)
  {
    const ON_BrepFace& f = b.m_F[fi];
    const ON_Surface*  s = f.SurfaceOf();
    if (!s)
      continue;
    occ::handle<Geom_BSplineSurface> gs = surface(*s, scale);
    TopoDS_Face                      F;
    B.MakeFace(F, gs, tol);
    for (int li = 0; li < f.m_li.Count(); ++li)
    {
      const ON_BrepLoop& L = b.m_L[f.m_li[li]];
      TopoDS_Wire        W;
      B.MakeWire(W);
      for (int ti = 0; ti < L.m_ti.Count(); ++ti)
      {
        const ON_BrepTrim& t  = b.m_T[L.m_ti[ti]];
        const ON_Curve*    c2 = t.TrimCurveOf();
        if (!c2)
          continue;
        if (t.m_ei < 0)
        {
          // A singular trim (a pole): a degenerate edge.
          occ::handle<Geom2d_BSplineCurve> pc = trim_curve(t, false, 0, 0);
          TopoDS_Edge   E;
          TopoDS_Vertex V = verts[t.m_vi[0]];
          B.MakeEdge(E);
          B.UpdateEdge(E, pc, F, tol);
          B.Range(E, pc->FirstParameter(), pc->LastParameter());
          B.Degenerated(E, true);
          V.Orientation(TopAbs_FORWARD);
          B.Add(E, V);
          V.Orientation(TopAbs_REVERSED);
          B.Add(E, V);
          B.Add(W, E);
          continue;
        }
        TopoDS_Edge E = edges[t.m_ei];
        // A seam: the edge's two trims in this face give it two pcurves.
        int mate = -1;
        if (t.m_type == ON_BrepTrim::seam)
          for (int k = 0; k < b.m_E[t.m_ei].m_ti.Count(); ++k)
          {
            const int o = b.m_E[t.m_ei].m_ti[k];
            if (o != L.m_ti[ti] && b.m_T[o].Face() == &f)
              mate = o;
          }
        // The pcurve runs along the 3D curve, on its range.
        double e0, e1;
        BRep_Tool::Range(E, e0, e1);
        occ::handle<Geom2d_Curve> c = trim_curve(t, true, e0, e1);
        if (mate >= 0)
        {
          occ::handle<Geom2d_Curve> c2m = trim_curve(b.m_T[mate], true, e0, e1);
          // OCCT: first pcurve for the FORWARD use of the edge.
          if (!t.m_bRev3d)
            B.UpdateEdge(E, c, c2m, F, tol);
          else
            B.UpdateEdge(E, c2m, c, F, tol);
        }
        else
          B.UpdateEdge(E, c, F, tol);
        TopoDS_Edge Eo = E;
        Eo.Orientation(t.m_bRev3d ? TopAbs_REVERSED : TopAbs_FORWARD);
        B.Add(W, Eo);
      }
      B.Add(F, W);
    }
    if (f.m_bRev)
      F.Reverse();
    B.Add(shell, F);
  }
  TopoDS_Shape out = shell;
  if (b.IsSolid())
  {
    TopoDS_Solid so;
    B.MakeSolid(so);
    shell.Closed(true);
    B.Add(so, shell);
    out = so;
  }
  return heal_import(out, tol);
}

TopoDS_Shape mesh_shape_of(const ON_Mesh& m, double scale)
{
  std::vector<gp_Pnt> nodes;
  for (int i = 0; i < m.VertexCount(); ++i)
  {
    const ON_3dPoint q = m.Vertex(i);
    nodes.emplace_back(q.x * scale, q.y * scale, q.z * scale);
  }
  std::vector<std::array<int, 3>> tris;
  for (int i = 0; i < m.FaceCount(); ++i)
  {
    const ON_MeshFace& f = m.m_F[i];
    tris.push_back({f.vi[0], f.vi[1], f.vi[2]});
    if (f.IsQuad())
      tris.push_back({f.vi[0], f.vi[2], f.vi[3]});
  }
  return mesh_compound(mesh_bodies(nodes, std::move(tris), 0.0));
}

} // namespace

std::vector<XtPart> read_rhino_parts(const std::string& path)
{
  on_begin_read();
  ONX_Model  model;
  ON_wString err;
  ON_TextLog log(err);
  if (!model.Read(path.c_str(), &log))
    fail_io("rhino: cannot read " + path);
  const double scale = ON::UnitScale(model.m_settings.m_ModelUnitsAndTolerances.m_unit_system, ON::LengthUnitSystem::Meters);
  const double tol   = std::max(1e-7, model.m_settings.m_ModelUnitsAndTolerances.m_absolute_tolerance * scale * 0.1);
  std::vector<XtPart> out;
  auto colour_of = [&](const ON_3dmObjectAttributes& a, XtPart& p) {
    ON_Color c = a.m_color;
    if (a.ColorSource() == ON::color_from_layer)
    {
      const ON_Layer* l = ON_Layer::Cast(model.LayerFromIndex(a.m_layer_index).ModelComponent());
      if (l)
        c = l->Color();
    }
    else if (a.ColorSource() != ON::color_from_object)
      return;
    p.colour     = {c.FractionRed(), c.FractionGreen(), c.FractionBlue()};
    p.has_colour = true;
  };
  // One object's geometry (placed by `xf`) as a part.
  std::function<void(const ON_Geometry*, const ON_3dmObjectAttributes&, const ON_Xform&, int)> add = [&](const ON_Geometry* g, const ON_3dmObjectAttributes& a,
                                                                                                      const ON_Xform& xf, int depth) {
    if (!g || depth > 16)
      return;
    std::unique_ptr<ON_Geometry> placed;
    if (!xf.IsIdentity())
    {
      placed.reset(g->Duplicate());
      placed->Transform(xf);
      g = placed.get();
    }
    XtPart p;
    p.name = ON_String(a.m_name).Array() ? ON_String(a.m_name).Array() : "";
    colour_of(a, p);
    if (const ON_InstanceRef* ir = ON_InstanceRef::Cast(g))
    {
      const ON_InstanceDefinition* def = ON_InstanceDefinition::Cast(model.ComponentFromId(ON_ModelComponent::Type::InstanceDefinition, ir->m_instance_definition_uuid).ModelComponent());
      if (!def)
        return;
      for (int k = 0; k < def->InstanceGeometryIdList().Count(); ++k)
      {
        const ON_ModelGeometryComponent* mg = ON_ModelGeometryComponent::Cast(model.ComponentFromId(ON_ModelComponent::Type::ModelGeometry, def->InstanceGeometryIdList()[k]).ModelComponent());
        if (mg)
          add(mg->Geometry(nullptr), mg->Attributes(nullptr) ? *mg->Attributes(nullptr) : a, ir->m_xform, depth + 1);
      }
      return;
    }
    if (const ON_Brep* b = ON_Brep::Cast(g))
      p.shape = brep_shape(*b, scale, tol);
    else if (const ON_Mesh* m = ON_Mesh::Cast(g))
      p.shape = mesh_shape_of(*m, scale);
    else if (const ON_Curve* c = ON_Curve::Cast(g))
    {
      BRep_Builder B;
      TopoDS_Edge  E;
      occ::handle<Geom_BSplineCurve> gc = curve3d(*c, scale);
      B.MakeEdge(E, gc, tol);
      TopoDS_Vertex v0, v1;
      B.MakeVertex(v0, gc->Value(gc->FirstParameter()), tol);
      B.MakeVertex(v1, gc->Value(gc->LastParameter()), tol);
      if (gc->IsClosed())
        v1 = v0;
      v0.Orientation(TopAbs_FORWARD);
      v1.Orientation(TopAbs_REVERSED);
      B.Add(E, v0);
      B.Add(E, v1);
      p.shape = E;
    }
    else
    {
      // Extrusions, bare surfaces, SubDs: their Brep form.
      std::unique_ptr<ON_Brep> b(g->BrepForm());
      if (!b)
        return;
      p.shape = brep_shape(*b, scale, tol);
    }
    out.push_back(std::move(p));
  };
  ONX_ModelComponentIterator it(model, ON_ModelComponent::Type::ModelGeometry);
  for (const ON_ModelComponent* c = it.FirstComponent(); c; c = it.NextComponent())
  {
    const ON_ModelGeometryComponent* g = ON_ModelGeometryComponent::Cast(c);
    if (!g)
      continue;
    const ON_3dmObjectAttributes* a = g->Attributes(nullptr);
    // Objects inside block definitions come in through their instances.
    if (a && a->Mode() == ON::object_mode::idef_object)
      continue;
    add(g->Geometry(nullptr), a ? *a : ON_3dmObjectAttributes::Unset, ON_Xform::IdentityTransformation, 0);
  }
  if (out.empty())
    fail_invalid("rhino: no objects in the file");
  return out;
}

} // namespace noshape

#else

namespace noshape {

std::vector<XtPart> read_rhino_parts(const std::string&)
{
  fail_unsupported("Rhino import is not available in this build (built without openNURBS)");
}

} // namespace noshape

#endif
