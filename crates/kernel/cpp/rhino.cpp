// Rhino .3dm export through openNURBS (MIT-style license, built separately like
// OCCT: scripts/build-opennurbs.sh, OPENNURBS_ROOT). Every shape becomes one
// Rhino Brep: OCCT geometry converted to NURBS with its pcurves
// (BRepBuilderAPI_NurbsConvert, exact), faces with their trims (seams and
// singular trims at poles as Rhino has them too), in millimetres.
// Without openNURBS (agents' dev builds) the writer reports that it isn't
// available; CI and releases always build with it (NOSHAPE_REQUIRE_OPENNURBS).

#include "internal.hpp"

#if defined(NOSHAPE_OPENNURBS)

#include <BRepBuilderAPI_NurbsConvert.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomConvert.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <ShapeUpgrade_ShapeDivideClosed.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>

#include "opennurbs.h"

#include <map>
#include <mutex>
#include <string>

namespace noshape {

namespace {

void on_begin()
{
  static std::once_flag once;
  std::call_once(once, [] { ON::Begin(); });
}

// OCCT flat knots (cv + order of them) without the outermost two: openNURBS' knot vector.
template <class Flat>
void set_knots(double* on_knots, const Flat& flat)
{
  for (int i = flat.Lower() + 1; i < flat.Upper(); ++i)
    on_knots[i - flat.Lower() - 1] = flat(i);
}

ON_NurbsSurface* nurbs_surface(occ::handle<Geom_BSplineSurface> s)
{
  if (s->IsUPeriodic())
    s->SetUNotPeriodic();
  if (s->IsVPeriodic())
    s->SetVNotPeriodic();
  const bool       rat = s->IsURational() || s->IsVRational();
  ON_NurbsSurface* n   = new ON_NurbsSurface(3, rat, s->UDegree() + 1, s->VDegree() + 1, s->NbUPoles(), s->NbVPoles());
  set_knots(n->m_knot[0], s->UKnotSequence());
  set_knots(n->m_knot[1], s->VKnotSequence());
  for (int i = 1; i <= s->NbUPoles(); ++i)
    for (int j = 1; j <= s->NbVPoles(); ++j)
    {
      const gp_Pnt p = s->Pole(i, j);
      if (rat)
      {
        const double w = s->Weight(i, j);
        n->SetCV(i - 1, j - 1, ON_4dPoint(p.X() * w, p.Y() * w, p.Z() * w, w));
      }
      else
        n->SetCV(i - 1, j - 1, ON_3dPoint(p.X(), p.Y(), p.Z()));
    }
  return n;
}

ON_NurbsCurve* nurbs_curve(occ::handle<Geom_BSplineCurve> c)
{
  if (c->IsPeriodic())
    c->SetNotPeriodic();
  const bool     rat = c->IsRational();
  ON_NurbsCurve* n   = new ON_NurbsCurve(3, rat, c->Degree() + 1, c->NbPoles());
  set_knots(n->m_knot, c->KnotSequence());
  for (int i = 1; i <= c->NbPoles(); ++i)
  {
    const gp_Pnt p = c->Pole(i);
    if (rat)
    {
      const double w = c->Weight(i);
      n->SetCV(i - 1, ON_4dPoint(p.X() * w, p.Y() * w, p.Z() * w, w));
    }
    else
      n->SetCV(i - 1, ON_3dPoint(p.X(), p.Y(), p.Z()));
  }
  return n;
}

ON_NurbsCurve* nurbs_curve_2d(occ::handle<Geom2d_BSplineCurve> c)
{
  if (c->IsPeriodic())
    c->SetNotPeriodic();
  const bool     rat = c->IsRational();
  ON_NurbsCurve* n   = new ON_NurbsCurve(2, rat, c->Degree() + 1, c->NbPoles());
  set_knots(n->m_knot, c->KnotSequence());
  for (int i = 1; i <= c->NbPoles(); ++i)
  {
    const gp_Pnt2d p = c->Pole(i);
    const double   w = rat ? c->Weight(i) : 1.0;
    if (rat)
      n->SetCV(i - 1, ON_4dPoint(p.X() * w, p.Y() * w, 0, w)); // dim 2: x, y, w
    else
      n->SetCV(i - 1, ON_3dPoint(p.X(), p.Y(), 0));
  }
  return n;
}

// Faces bounded only by their own seams (a whole sphere or torus) are split:
// Onshape's .3dm import makes a sheet, not a solid, of a body without
// edges between faces (G17).
TopoDS_Shape split_seam_only_faces(const TopoDS_Shape& in)
{
  for (TopExp_Explorer ef(in, TopAbs_FACE); ef.More(); ef.Next())
  {
    const TopoDS_Face& F    = TopoDS::Face(ef.Current());
    bool               only = true;
    for (TopExp_Explorer ee(F, TopAbs_EDGE); ee.More() && only; ee.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(ee.Current());
      only                 = BRep_Tool::Degenerated(e) || BRep_Tool::IsClosed(e, F);
    }
    if (only)
    {
      ShapeUpgrade_ShapeDivideClosed d(in);
      d.SetNbSplitPoints(1);
      d.Perform();
      return d.Result();
    }
  }
  return in;
}

ON_Brep* to_brep(const TopoDS_Shape& in)
{
  BRepBuilderAPI_NurbsConvert conv(split_seam_only_faces(in), true);
  if (!conv.IsDone())
    fail_op("rhino: NURBS conversion failed");
  const TopoDS_Shape s = conv.Shape();
  ON_Brep*           b = new ON_Brep();
  ShapeIMap          verts, edges;
  TopExp::MapShapes(s, TopAbs_VERTEX, verts);
  for (int i = 1; i <= verts.Extent(); ++i)
  {
    const TopoDS_Vertex& v = TopoDS::Vertex(verts(i));
    const gp_Pnt         p = BRep_Tool::Pnt(v);
    b->NewVertex(ON_3dPoint(p.X(), p.Y(), p.Z()), BRep_Tool::Tolerance(v));
  }
  std::map<int, int> edge_of; // OCCT edge index -> ON edge index
  TopExp::MapShapes(s, TopAbs_EDGE, edges);
  for (int i = 1; i <= edges.Extent(); ++i)
  {
    TopoDS_Edge e = TopoDS::Edge(edges(i));
    e.Orientation(TopAbs_FORWARD);
    if (BRep_Tool::Degenerated(e))
      continue;
    double                  f, l;
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
    if (c.IsNull())
      fail_op("rhino: an edge without a 3D curve");
    const int c3 = b->AddEdgeCurve(nurbs_curve(GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(c, f, l))));
    const int v0 = verts.FindIndex(TopExp::FirstVertex(e)) - 1, v1 = verts.FindIndex(TopExp::LastVertex(e)) - 1;
    edge_of[i]   = b->NewEdge(b->m_V[v0], b->m_V[v1], c3, nullptr, BRep_Tool::Tolerance(e)).m_edge_index;
  }
  for (TopExp_Explorer ef(s, TopAbs_FACE); ef.More(); ef.Next())
  {
    const TopoDS_Face&        F = TopoDS::Face(ef.Current());
    TopLoc_Location           loc;
    occ::handle<Geom_Surface> gs = BRep_Tool::Surface(F, loc);
    while (auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(gs))
      gs = t->BasisSurface();
    auto bs = occ::down_cast<Geom_BSplineSurface>(gs);
    if (bs.IsNull())
      fail_op("rhino: a face isn't a B-spline after conversion");
    bs = occ::down_cast<Geom_BSplineSurface>(bs->Copy());
    if (!loc.IsIdentity())
      bs->Transform(loc.Transformation());
    ON_NurbsSurface* ns   = nurbs_surface(bs);
    const int        si   = b->AddSurface(ns);
    ON_BrepFace&     face = b->NewFace(si);
    face.m_bRev           = F.Orientation() == TopAbs_REVERSED;
    // Loops in the face's own (forward) parameter space: face on the left.
    const TopoDS_Face  fwd   = TopoDS::Face(F.Oriented(TopAbs_FORWARD));
    const TopoDS_Wire  outer = BRepTools::OuterWire(fwd);
    for (TopExp_Explorer ew(fwd, TopAbs_WIRE); ew.More(); ew.Next())
    {
      const TopoDS_Wire& W    = TopoDS::Wire(ew.Current());
      ON_BrepLoop&       loop = b->NewLoop(W.IsSame(outer) ? ON_BrepLoop::outer : ON_BrepLoop::inner, b->m_F[face.m_face_index]);
      for (BRepTools_WireExplorer we(W, fwd); we.More(); we.Next())
      {
        const TopoDS_Edge&        e = we.Current();
        double                    f, l;
        occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, fwd, f, l);
        if (pc.IsNull())
          fail_op("rhino: an edge without a curve on its face");
        ON_NurbsCurve* c2 = nurbs_curve_2d(Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(pc, f, l)));
        const bool     rev = e.Orientation() == TopAbs_REVERSED;
        if (rev)
          c2->Reverse();
        const int c2i = b->AddTrimCurve(c2);
        if (BRep_Tool::Degenerated(e))
        {
          const int vi = verts.FindIndex(TopExp::FirstVertex(e)) - 1;
          b->NewSingularTrim(b->m_V[vi], loop, ns->IsIsoparametric(*c2), c2i);
          continue;
        }
        TopoDS_Edge ef_ = e;
        ef_.Orientation(TopAbs_FORWARD);
        ON_BrepTrim& t = b->NewTrim(b->m_E[edge_of.at(edges.FindIndex(ef_))], rev, loop, c2i);
        t.m_tolerance[0] = t.m_tolerance[1] = BRep_Tool::Tolerance(e);
      }
    }
  }
  b->SetTrimIsoFlags();
  b->SetTrimTypeFlags();
  b->SetTolerancesBoxesAndFlags(true, true, true, true, true, true, true, true);
  return b;
}

} // namespace

void write_rhino(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Str path, uint32_t version)
{
  on_begin();
  ONX_Model model;
  model.m_settings.m_ModelUnitsAndTolerances.m_unit_system      = ON_UnitSystem(ON::LengthUnitSystem::Millimeters);
  model.m_settings.m_ModelUnitsAndTolerances.m_absolute_tolerance = 0.001;
  for (size_t i = 0; i < shapes.items.size(); ++i)
  {
    require_non_null(shapes.items[i], "shape");
    ON_Brep*        b = to_brep(shapes.items[i]);
    ON_wString      err;
    ON_TextLog      log(err);
    if (!b->IsValid(&log))
    {
      const std::string msg = ON_String(err).Array() ? ON_String(err).Array() : "";
      delete b;
      fail_op("rhino: invalid brep: " + msg.substr(0, 2000));
    }
    ON_3dmObjectAttributes attr;
    if (i < names.size())
      attr.m_name = ON_wString(std::string(names[i]).c_str());
    // (Copied: handing the Brep over with AddManagedModelGeometryComponent double-frees.)
    model.AddModelGeometryComponent(b, &attr);
    delete b;
  }
  // Rhino N -> archive version: 2, 3, 4, then 50, 60, 70, 80.
  const int v = version == 0 ? 0 : version <= 4 ? (int)version : (int)version * 10;
  ON_wString err;
  ON_TextLog log(err);
  if (!model.Write(std::string(path).c_str(), v, &log))
    fail_op("rhino: writing " + std::string(path) + " failed: " + std::string(ON_String(err).Array() ? ON_String(err).Array() : ""));
}

uint32_t rhino_valid_breps(rust::Str path)
{
  on_begin();
  ONX_Model  model;
  ON_wString err;
  ON_TextLog log(err);
  if (!model.Read(std::string(path).c_str(), &log))
    fail_op("rhino: cannot read " + std::string(path));
  uint32_t                  n = 0;
  ONX_ModelComponentIterator it(model, ON_ModelComponent::Type::ModelGeometry);
  for (const ON_ModelComponent* c = it.FirstComponent(); c; c = it.NextComponent())
  {
    const ON_ModelGeometryComponent* g = ON_ModelGeometryComponent::Cast(c);
    const ON_Brep*                   b = g ? ON_Brep::Cast(g->Geometry(nullptr)) : nullptr;
    if (!b)
      continue;
    if (!b->IsValid(&log))
      fail_op("rhino: invalid brep in file: " + std::string(ON_String(err).Array() ? ON_String(err).Array() : "").substr(0, 2000));
    ++n;
  }
  return n;
}

} // namespace noshape

#else

namespace noshape {

void write_rhino(const OcShapeList&, rust::Slice<const rust::String>, rust::Str, uint32_t)
{
  fail_unsupported("Rhino export needs openNURBS: build with OPENNURBS_ROOT (scripts/build-opennurbs.sh)");
}

uint32_t rhino_valid_breps(rust::Str)
{
  fail_unsupported("Rhino needs openNURBS: build with OPENNURBS_ROOT (scripts/build-opennurbs.sh)");
}

} // namespace noshape

#endif
