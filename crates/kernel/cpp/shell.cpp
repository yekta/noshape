// Robust shell (hollow) of a solid.
//
// The wall is the solid minus a "cavity": the solid offset inward by the
// thickness, with the faces to remove kept in place (outward shells: the solid
// offset outward minus the solid). Three ways to build it, cheapest first,
// each result checked (valid solid, volumes consistent):
//   1. BRepOffset_MakeOffset of the whole solid (intersection joins),
//   2. a boolean construction that doesn't need OCCT's offset topology:
//      the union of every kept face thickened by the wall thickness ("slabs"),
//      plus filler prisms where offsets of neighbouring faces leave a gap
//      (concave edges; edges along removed faces, where the offset of a kept
//      face extends up to the removed face's surface), intersected with the
//      solid. Handles faces whose offsets vanish (a chamfer cone at a fillet
//      corner, thinner than the wall) and walls that merge.
// Both give the same geometry where they succeed (intersection
// semantics: offset faces are extended until they meet) and the same history:
// faces on the original faces are Modified from them, offset faces are
// Generated from their face, faces on a removed face (the rim) are Modified
// from it.

#include "internal.hpp"
#include <chrono>
#include <ShapeFix_Shape.hxx>
#include <cstring>
#include <cstdio>
#include <cstdlib>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <Extrema_GenLocateExtPS.hxx>
#include <ShapeFix_Face.hxx>
#include <NCollection_Array2.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <GeomAPI_PointsToBSplineSurface.hxx>
#include <Geom_BSplineSurface.hxx>
#include <GeomLProp_SLProps.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <BRepFill.hxx>
#include <BRepAlgoAPI_Splitter.hxx>

#include <BRepAdaptor_Curve.hxx>
#include <IntCurvesFace_ShapeIntersector.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <limits>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <ElCLib.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepGProp.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <gp_Torus.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <gp_Lin.hxx>
#include <ElSLib.hxx>
#include <BRepOffset_Analyse.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <Geom2dAdaptor_Curve.hxx>
#include <Geom_Line.hxx>
#include <Geom2d_Curve.hxx>
#include <gp_Lin2d.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <ShapeUpgrade_ShapeDivideClosed.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <Standard_Failure.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Pln.hxx>

#include <algorithm>

#include <cmath>
#include <map>
#include <functional>
#include <GeomProjLib.hxx>
#include <Geom_Plane.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BOPAlgo_ArgumentAnalyzer.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <ShapeFix_Edge.hxx>
#include <GeomAPI_IntSS.hxx>
#include <TopAbs.hxx>
#include <Geom_Surface.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Mat.hxx>
#include <array>
#include <set>

namespace noshape {

// The way this thread's last shell was built ("regions", "offset", "slabs").
thread_local const char* t_shell_path = "";
rust::String             last_shell_path() { return rust::String(t_shell_path); }

namespace {

// An OCCT Boolean of `a` and `b` built under the thread's cancel / budget
// indicator, so a budget stops it by throwing (its memory freed) rather than
// by the timer's jump (MEM-RUNAWAY-508).
template <class Op>
void build_stoppable(Op& op, const TopoDS_Shape& a, const TopoDS_Shape& b)
{
  TopTools_ListOfShape x, y;
  x.Append(a);
  y.Append(b);
  op.SetArguments(x);
  op.SetTools(y);
  op.Build(new_cancel_indicator()->Start());
}

using HHist = occ::handle<BRepTools_History>;

double volume_of(const TopoDS_Shape& s)
{
  return volume_props(s).Mass();
}

int solid_count(const TopoDS_Shape& s)
{
  int n = 0;
  for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next())
    ++n;
  return n;
}

bool valid_solid(const TopoDS_Shape& s)
{
  return !s.IsNull() && solid_count(s) > 0 && BRepCheck_Analyzer(s).IsValid();
}

// A compound holding one solid -> that solid.
TopoDS_Shape single_solid(const TopoDS_Shape& s)
{
  if (s.ShapeType() == TopAbs_COMPOUND && solid_count(s) == 1)
    for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next())
      return e.Current();
  return s;
}

// Shells of an offset result as one outward-oriented solid.
TopoDS_Shape solid_of(const TopoDS_Shape& s)
{
  if (s.IsNull())
    return s;
  for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next())
  {
    TopoDS_Solid so = TopoDS::Solid(e.Current());
    BRepLib::OrientClosedSolid(so);
    return so;
  }
  BRepBuilderAPI_MakeSolid ms;
  int                      n = 0;
  for (TopExp_Explorer e(s, TopAbs_SHELL); e.More(); e.Next(), ++n)
    ms.Add(TopoDS::Shell(e.Current()));
  if (n == 0 || !ms.IsDone())
    return TopoDS_Shape();
  TopoDS_Solid so = TopoDS::Solid(ms.Shape());
  BRepLib::OrientClosedSolid(so);
  return so;
}

// Where faces of the result come from: tool face -> (face of the input, relation).
struct Source
{
  TopoDS_Shape face;
  uint8_t      rel;
};
using SourceMap = NCollection_DataMap<TopoDS_Shape, Source, TopTools_ShapeMapHasher>;

// Images of `s` through a chain of histories (null entries are skipped).
ShapeList images_through(const std::vector<HHist>& chain, const TopoDS_Shape& s)
{
  ShapeList cur;
  cur.Append(s);
  for (const HHist& h : chain)
  {
    if (h.IsNull())
      continue;
    ShapeList next;
    for (ShapeList::Iterator it(cur); it.More(); it.Next())
    {
      const ShapeList imgs = images_of(h, it.Value());
      for (ShapeList::Iterator jt(imgs); jt.More(); jt.Next())
        if (!jt.Value().IsNull())
          next.Append(jt.Value());
    }
    cur = next;
  }
  return cur;
}

// The final history (input solid -> result) from the boolean chain the
// result went through and the provenance of the tool faces.
HHist shell_history(const TopoDS_Shape& solid, const TopoDS_Shape& result,
                    const std::vector<HHist>& solid_chain, const std::vector<HHist>& tool_chain,
                    const SourceMap& sources, const std::vector<TopoDS_Shape>& removed)
{
  HHist     h = new BRepTools_History();
  ShapeIMap out;
  for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    TopExp::MapShapes(result, k, out);
  ShapeIMap modified; // result faces already Modified from an input face
  // Tool faces standing for an input face (a drafted face, the offset of a
  // removed face): its images even when the input face itself is cut away.
  NCollection_DataMap<TopoDS_Shape, ShapeList, TopTools_ShapeMapHasher> stand_in;
  for (SourceMap::Iterator it(sources); it.More(); it.Next())
  {
    if (it.Value().rel != REL_MODIFIED)
      continue;
    const ShapeList imgs = images_through(tool_chain, it.Key());
    for (ShapeList::Iterator jt(imgs); jt.More(); jt.Next())
      if (out.Contains(jt.Value()))
      {
        if (!stand_in.IsBound(it.Value().face))
          stand_in.Bind(it.Value().face, ShapeList());
        stand_in.ChangeFind(it.Value().face).Append(jt.Value());
      }
  }
  for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
  {
    ShapeIMap in;
    TopExp::MapShapes(solid, k, in);
    for (int i = 1; i <= in.Extent(); ++i)
    {
      const TopoDS_Shape& s    = in(i);
      bool                live = false;
      ShapeList           imgs = images_through(solid_chain, s);
      if (const ShapeList* extra = stand_in.Seek(s))
        for (ShapeList::Iterator it(*extra); it.More(); it.Next())
          imgs.Append(it.Value());
      ShapeIMap seen;
      for (ShapeList::Iterator it(imgs); it.More(); it.Next())
      {
        if (!out.Contains(it.Value()) || seen.Contains(it.Value()))
          continue;
        seen.Add(it.Value());
        live = true;
        if (k == TopAbs_FACE)
          modified.Add(it.Value());
        if (!it.Value().IsSame(s))
          h->AddModified(s, it.Value());
      }
      if (!live)
        h->Remove(s);
    }
  }
  for (SourceMap::Iterator it(sources); it.More(); it.Next())
  {
    if (it.Value().rel == REL_MODIFIED)
      continue;
    const ShapeList imgs = images_through(tool_chain, it.Key());
    for (ShapeList::Iterator jt(imgs); jt.More(); jt.Next())
    {
      const TopoDS_Shape& img = jt.Value();
      if (out.Contains(img) && !img.IsSame(it.Value().face) && !modified.Contains(img))
        h->AddGenerated(it.Value().face, img);
    }
  }
  // Faces lying on a removed face (the rim) are Modified from it.
  if (!removed.empty())
  {
    for (TopExp_Explorer e(result, TopAbs_FACE); e.More(); e.Next())
    {
      if (modified.Contains(e.Current()))
        continue;
      const TopoDS_Face& f = TopoDS::Face(e.Current());
      GProp_GProps       g;
      BRepGProp::SurfaceProperties(f, g);
      gp_Pnt c = g.CentreOfMass();
      BRepAdaptor_Surface fs(f);
      for (const TopoDS_Shape& r : removed)
      {
        BRepAdaptor_Surface rs(TopoDS::Face(r));
        if (fs.GetType() != rs.GetType())
          continue;
        GeomAPI_ProjectPointOnSurf pr(c, BRep_Tool::Surface(TopoDS::Face(r)));
        if (pr.NbPoints() > 0 && pr.LowerDistance() < 1e-6 * (1.0 + c.XYZ().Modulus()))
        {
          if (!h->IsRemoved(r))
            h->AddModified(r, f);
          modified.Add(f);
          break;
        }
      }
    }
  }
  return h;
}

// ------------------------------------------------------------------ offset

struct Built
{
  TopoDS_Shape result;
  HHist        history;
};

bool inner_point(const TopoDS_Face& f, gp_Pnt& p, gp_Dir& n);

// Path 1: offset the whole solid (removed faces stay in place), then
// subtract. `sign` is -1 for an inward wall, +1 outward.
bool by_offset(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t,
               double sign, Built& out)
{
  const bool dbg = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       no  = [&](const char* why) {
    if (dbg)
      std::fprintf(stderr, "offset: %s\n", why);
    return false;
  };
  TopoDS_Shape offset;
  SourceMap    sources;
  try
  {
    BRepOffset_MakeOffset mo;
    mo.Initialize(solid, sign * t, 1e-6, BRepOffset_Skin, false, false, GeomAbs_Intersection);
    for (const TopoDS_Shape& f : removed)
      mo.SetOffsetOnFace(TopoDS::Face(f), 0.0);
    mo.MakeOffsetShape(new_cancel_indicator()->Start());
    if (!mo.IsDone())
      return no("BRepOffset failed");
    offset = solid_of(mo.Shape());
    if (offset.IsNull())
      return no("no solid");
    ShapeIMap rm;
    for (const TopoDS_Shape& f : removed)
      rm.Add(f);
    const BRepAlgo_Image& img = mo.OffsetFacesFromShapes();
    for (TopExp_Explorer e(solid, TopAbs_FACE); e.More(); e.Next())
    {
      const TopoDS_Shape& f = e.Current();
      if (!img.HasImage(f))
        continue;
      ShapeList last;
      img.LastImage(f, last);
      for (ShapeList::Iterator it(last); it.More(); it.Next())
        sources.Bind(it.Value(), Source{f, rm.Contains(f) ? (uint8_t)REL_MODIFIED : (uint8_t)REL_GENERATED});
    }
  }
  catch (const Standard_Failure&)
  {
    return no("BRepOffset threw");
  }
  const TopoDS_Shape& outer = sign < 0 ? solid : offset;
  const TopoDS_Shape& inner = sign < 0 ? offset : solid;
  BRepAlgoAPI_Cut     cut;
  build_stoppable(cut, outer, inner);
  if (cut.HasErrors() || !cut.IsDone())
    return no("cut failed");
  TopoDS_Shape r = cut.Shape();
  if (!valid_solid(r))
    return no("invalid result");
  // A correct offset lies inside the outer solid: the volumes must add up
  // (catches self-intersecting offsets that BRepCheck accepts).
  const double vo = volume_of(outer), vi = volume_of(inner), vr = volume_of(r);
  if (!(vi > 0 && vr > 0 && vi < vo && std::abs(vo - vi - vr) <= 1e-6 * vo))
    return no("volumes don't add up");
  // The part's own faces that stay (inward: the outside, outward: the
  // cavity) are all still there: a point inside each lies on the result. An
  // offset that is wrong but adds up loses some of them (live "kern Shell
  // bracket": a chamfer round the top, 1 mm: 12 faces, Parasolid's 34).
  {
    const TopoDS_Shape so = single_solid(r);
    ShapeIMap kept_out;
    for (const TopoDS_Shape& f : removed)
      kept_out.Add(f);
    for (TopExp_Explorer e(solid, TopAbs_FACE); e.More(); e.Next())
    {
      gp_Pnt p;
      gp_Dir n;
      if (kept_out.Contains(e.Current()) || !inner_point(TopoDS::Face(e.Current()), p, n))
        continue;
      // (On the result's boundary: its distance, not the solid classifier,
      // which calls points on a face OUT at times.)
      BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(p).Vertex(), so);
      if (!d.IsDone() || d.Value() > 1e-7)
        return no("a kept face is lost");
    }
  }
  // Offsets extended past a face's boundary come out as separate faces on
  // the same surface; merge them so the topology (and the names) match the
  // slab construction's.
  ShapeUpgrade_UnifySameDomain usd(r, true, true, false);
  usd.SetLinearTolerance(1e-7);
  usd.Build();
  HHist hc = cut.History(), hs = usd.History();
  r = single_solid(usd.Shape());
  out.result = r;
  // The rim is the cut image of the removed faces: no need to search for it.
  out.history = shell_history(solid, r, {hc, hs}, {hc, hs}, sources, {});
  return true;
}

// Live G50/G51: Parasolid can't shell a box whose two fillets (radius r,
// perpendicular, meeting at a mitre under the removed face) run into a 45°
// chamfer, for walls from t0 up to (not including) r: there the rim triangle
// the inner chamfer cuts off is as wide as the inner fillets (its legs,
// c − 2r + √2 t, reach 2 (r − t)). With d the signed distance from the
// fillets' corner on the removed face to the chamfer's plane (≤ 0: the
// fillets meet before the chamfer), t0 = (2 − √2)(r − d/√2). Live: chamfer
// 5 / 4 / 6 / 5 / 3 with fillet 3 / 3 / 3 / 4 / 2 fail from 2.053 / 2.344 /
// 1.760 / 3.222 / 1.466 mm (last good 2.050 / 2.341 / 1.757 / 3.219 / 1.463);
// chamfer 5 with fillet 2 (no mitre) never fails.
bool in_onshape_mitre_band(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t)
{
  ShapeAncMap v2f, e2f;
  TopExp::MapShapesAndAncestors(solid, TopAbs_VERTEX, TopAbs_FACE, v2f);
  TopExp::MapShapesAndAncestors(solid, TopAbs_EDGE, TopAbs_FACE, e2f);
  auto shares_edge = [&](const TopoDS_Shape& a, const TopoDS_Shape& b) {
    for (TopExp_Explorer e(a, TopAbs_EDGE); e.More(); e.Next())
      if (const ShapeList* l = e2f.Seek(e.Current()))
        for (ShapeList::Iterator it(*l); it.More(); it.Next())
          if (it.Value().IsSame(b))
            return true;
    return false;
  };
  for (const TopoDS_Shape& rf : removed)
    for (TopExp_Explorer v(rf, TopAbs_VERTEX); v.More(); v.Next())
    {
      const ShapeList* at = v2f.Seek(v.Current());
      if (at == nullptr)
        continue;
      std::vector<TopoDS_Face> cyl;
      for (ShapeList::Iterator it(*at); it.More(); it.Next())
        if (!it.Value().IsSame(rf) && BRepAdaptor_Surface(TopoDS::Face(it.Value()), false).GetType() == GeomAbs_Cylinder)
        {
          bool dup = false;
          for (const TopoDS_Face& c : cyl)
            dup = dup || c.IsSame(it.Value());
          if (!dup)
            cyl.push_back(TopoDS::Face(it.Value()));
        }
      if (cyl.size() != 2 || !shares_edge(cyl[0], cyl[1]))
        continue;
      const gp_Cylinder c1 = BRepAdaptor_Surface(cyl[0], false).Cylinder(), c2 = BRepAdaptor_Surface(cyl[1], false).Cylinder();
      const double      r  = c1.Radius();
      if (std::abs(c2.Radius() - r) > 1e-9 * r || std::abs(c1.Axis().Direction().Dot(c2.Axis().Direction())) > 1e-9)
        continue;
      // The chamfer: a plane next to both fillets, at 45° to both axes.
      for (TopExp_Explorer f(solid, TopAbs_FACE); f.More(); f.Next())
      {
        const TopoDS_Face& F = TopoDS::Face(f.Current());
        BRepAdaptor_Surface a(F, false);
        if (F.IsSame(rf) || a.GetType() != GeomAbs_Plane || !shares_edge(F, cyl[0]) || !shares_edge(F, cyl[1]))
          continue;
        gp_Dir n = a.Plane().Axis().Direction();
        if (!a.Plane().Position().Direct())
          n.Reverse();
        if (F.Orientation() == TopAbs_REVERSED)
          n.Reverse();
        if (std::abs(std::abs(n.Dot(c1.Axis().Direction())) - M_SQRT1_2) > 1e-6 || std::abs(std::abs(n.Dot(c2.Axis().Direction())) - M_SQRT1_2) > 1e-6)
          continue;
        const double d  = gp_Vec(a.Plane().Location(), BRep_Tool::Pnt(TopoDS::Vertex(v.Current()))).Dot(gp_Vec(n));
        if (d > 1e-9 * r)
          continue;
        const double t0 = (2 - M_SQRT2) * (r - d / M_SQRT2);
        if (t > t0 * (1 + 1e-9) && t < r * (1 - 1e-9))
          return true;
      }
    }
  return false;
}

// Whether a blind hole's bottom lies less than 2t above a kept face under
// it: live (behaviours "core (2026-10-27 b)", qa5 P1-5's block, 26 cases)
// fails the shell as soon as the hole's lowest flat level (the shoulder of a
// drilled tip, or a flat bottom) is less than 2t above the outer floor, its
// offset t below the cavity floor; a through hole never. (Live also fails at
// t <= ~1 when the tip cone's offset apex dips under the cavity floor: two
// points only, not ruled here.)
bool blind_hole_floor_too_thin(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t)
{
  ShapeIMap rm;
  for (const TopoDS_Shape& f : removed)
    rm.Add(f);
  IntCurvesFace_ShapeIntersector hits;
  bool                           loaded = false;
  for (TopExp_Explorer e(solid, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    if (rm.Contains(f))
      continue;
    BRepAdaptor_Surface a(f, false);
    // A hole's wall: concave (the outward normal points at the axis), all round.
    if (a.GetType() != GeomAbs_Cylinder || a.Cylinder().Position().Direct() != (f.Orientation() == TopAbs_REVERSED))
      continue;
    double u0, u1, v0, v1;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    if (u1 - u0 < 2 * M_PI - 1e-6)
      continue;
    if (!loaded)
    {
      hits.Load(solid, 1e-9);
      loaded = true;
    }
    const gp_Cylinder cyl = a.Cylinder();
    const gp_Ax3&     ax  = cyl.Position();
    const double      r = cyl.Radius(), eps = 1e-3 * r;
    for (const auto& [v, sense] : {std::pair{v0, -1.0}, std::pair{v1, 1.0}})
    {
      // From inside the hole near its end, a quarter radius off the axis (a
      // drilled tip's apex sits on it), on along the axis: into the bottom
      // (none at an open end), then out through the face under it.
      const gp_Dir d = sense > 0 ? ax.Direction() : ax.Direction().Reversed();
      const gp_Pnt p = ElSLib::Value(0, v, gp_Cylinder(ax, 0.25 * r)).Translated(gp_Vec(d) * -eps);
      hits.Perform(gp_Lin(p, d), 0, RealLast());
      std::vector<std::pair<double, int>> ws;
      for (int i = 1; i <= hits.NbPnt(); ++i)
        ws.emplace_back(hits.WParameter(i), i);
      std::sort(ws.begin(), ws.end());
      if (ws.size() < 2 || hits.Transition(ws[0].second) != IntCurveSurface_In || hits.Transition(ws[1].second) != IntCurveSurface_Out)
        continue;
      const TopoDS_Face& floor = hits.Face(ws[1].second);
      BRepAdaptor_Surface fa(floor, false);
      if (rm.Contains(floor) || fa.GetType() != GeomAbs_Plane || !fa.Plane().Axis().Direction().IsParallel(d, 1e-9))
        continue;
      if (ws[1].first - eps < 2 * t * (1 - 1e-9))
        return true;
    }
  }
  return false;
}

// How close an inward shell's offset walls come to touching: a kept concave
// cylinder (a hole's wall, offset to r + t) and a kept plane (offset t into
// the material) with the axis parallel to it on its material side at about
// r + 2 t, the line where they'd touch on both faces; the smallest
// |distance - (r + 2 t)|, or infinity. Live "kern (10-23 09:00) Shell
// bracket": Parasolid fails when they touch exactly (the Ø10 hole 10 mm from
// the bracket's front at 2.5 mm; 2.499 and 2.501 work).
double offsets_touch_gap(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t)
{
  double gap = std::numeric_limits<double>::infinity();
  ShapeIMap rm;
  for (const TopoDS_Shape& f : removed)
    rm.Add(f);
  auto inside = [](const TopoDS_Face& f, const gp_Pnt& p) {
    GeomAPI_ProjectPointOnSurf pr(p, BRep_Tool::Surface(f));
    if (pr.NbPoints() == 0 || pr.LowerDistance() > 1e-7)
      return false;
    double u, v;
    pr.LowerDistanceParameters(u, v);
    BRepClass_FaceClassifier fc(f, gp_Pnt2d(u, v), 1e-9);
    return fc.State() == TopAbs_IN;
  };
  std::vector<std::pair<TopoDS_Face, gp_Pln>> planes;
  std::vector<std::pair<TopoDS_Face, gp_Cylinder>> holes;
  for (TopExp_Explorer e(solid, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    if (rm.Contains(f))
      continue;
    BRepAdaptor_Surface a(f, false);
    if (a.GetType() == GeomAbs_Plane)
      planes.emplace_back(f, a.Plane());
    // Concave: the outward normal points at the axis.
    else if (a.GetType() == GeomAbs_Cylinder && (a.Cylinder().Position().Direct() == (f.Orientation() == TopAbs_REVERSED)))
      holes.emplace_back(f, a.Cylinder());
  }
  for (const auto& [cf, cyl] : holes)
  {
    const gp_Dir ax = cyl.Axis().Direction();
    // The hole's middle along its axis (its face's box centre on the axis).
    Bnd_Box bb;
    BRepBndLib::Add(cf, bb, false);
    const gp_Pnt mid((bb.CornerMin().XYZ() + bb.CornerMax().XYZ()) / 2);
    const gp_Pnt c = ElCLib::Value(ElCLib::Parameter(gp_Lin(cyl.Axis()), mid), gp_Lin(cyl.Axis()));
    for (const auto& [pf, pl] : planes)
    {
      gp_Dir n = pl.Axis().Direction();
      if (!pl.Position().Direct())
        n.Reverse();
      if (pf.Orientation() == TopAbs_REVERSED)
        n.Reverse();
      if (std::abs(n.Dot(ax)) > 1e-9)
        continue;
      // Signed: the axis on the material side (inside the plane) is negative.
      const double s = gp_Vec(pl.Location(), c).Dot(gp_Vec(n));
      const double g = std::abs(-s - (cyl.Radius() + 2 * t));
      if (s >= 0 || g >= gap)
        continue;
      const gp_Pnt on_hole = c.Translated(gp_Vec(n) * cyl.Radius());
      const gp_Pnt on_plane = c.Translated(gp_Vec(n) * -s);
      if (inside(cf, on_hole) && inside(pf, on_plane))
        gap = g;
    }
  }
  return gap;
}

// ------------------------------------------------------------------ regions

// The convex polytope {x : (x - o_i).n_i <= 0 for all i} as a solid, built
// without booleans (SHELL-SLOW: a cut per plane cost ~18 ms each): each
// plane's square of half-size `big` clipped by the other half-spaces, the
// faces sewn. `faces[i]` gets plane i's face (null when it bounds nothing).
TopoDS_Shape polytope(const std::vector<std::pair<gp_Pnt, gp_Dir>>& hs, double big, std::vector<TopoDS_Shape>& faces)
{
  const double            eps = 1e-12 * big;
  BRepBuilderAPI_Sewing   sew(1e-9 * big);
  std::vector<TopoDS_Face> made(hs.size());
  for (size_t i = 0; i < hs.size(); ++i)
  {
    const gp_Ax3        ax(hs[i].first, hs[i].second);
    const gp_Vec        x(ax.XDirection()), y(ax.YDirection());
    std::vector<gp_Pnt> poly{hs[i].first.Translated(x * -big + y * -big), hs[i].first.Translated(x * big + y * -big),
                             hs[i].first.Translated(x * big + y * big), hs[i].first.Translated(x * -big + y * big)};
    for (size_t j = 0; j < hs.size() && poly.size() >= 3; ++j)
    {
      if (j == i)
        continue;
      const gp_Vec        n(hs[j].second);
      auto                side = [&](const gp_Pnt& p) { return gp_Vec(hs[j].first, p).Dot(n); };
      std::vector<gp_Pnt> next;
      for (size_t k = 0; k < poly.size(); ++k)
      {
        const gp_Pnt& a = poly[k];
        const gp_Pnt& b = poly[(k + 1) % poly.size()];
        const double  sa = side(a), sb = side(b);
        if (sa <= eps)
          next.push_back(a);
        if ((sa < -eps && sb > eps) || (sa > eps && sb < -eps))
          next.push_back(a.Translated(gp_Vec(a, b) * (sa / (sa - sb))));
      }
      poly.swap(next);
    }
    std::vector<gp_Pnt> clean;
    for (const gp_Pnt& p : poly)
      if (clean.empty() || clean.back().Distance(p) > 1e-9 * big)
        clean.push_back(p);
    while (clean.size() > 1 && clean.front().Distance(clean.back()) <= 1e-9 * big)
      clean.pop_back();
    if (clean.size() < 3)
      continue;
    BRepBuilderAPI_MakePolygon w;
    for (const gp_Pnt& p : clean)
      w.Add(p);
    w.Close();
    if (!w.IsDone())
      continue;
    BRepBuilderAPI_MakeFace f(gp_Pln(ax), w.Wire(), true);
    if (!f.IsDone())
      continue;
    made[i] = f.Face();
    sew.Add(made[i]);
  }
  sew.Perform();
  TopoDS_Shape so = solid_of(sew.SewedShape());
  faces.assign(hs.size(), TopoDS_Shape());
  for (size_t i = 0; i < hs.size(); ++i)
    if (!made[i].IsNull())
      faces[i] = sew.IsModified(made[i]) ? sew.Modified(made[i]) : TopoDS_Shape(made[i]);
  return so;
}

TopoDS_Face         polygon_face(const std::vector<gp_Pnt>& pts);
std::vector<gp_Pnt> clip_at_axis(const std::vector<gp_Pnt>& in, const gp_Pnt& c, const gp_Vec& radial);

// Path 0, inward walls of convex solids bounded by planes and convex
// cylinders (fillets) or cones with planar faces removed: the cavity is where every
// kept face's offset leaves it, Parasolid's rule of offset faces extended
// until they meet. A plane's offset is a half-space; a fillet's offset
// bounds the angular sector its face spans (the sector beyond the offset
// radius is removed, closed by the sector's sides where a removed face
// is tangent: Onshape's wall strips). OCCT's offset changes the topology
// there (live G50: a chamfer next to fillets under a removed top face makes
// one flat rim triangle in Onshape; BRepOffset gives two sloped faces).
bool by_regions(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t, double sign,
                Built& out)
{
  const bool dbg = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       no  = [&](const char* why) {
    if (dbg)
      std::fprintf(stderr, "regions: %s\n", why);
    return false;
  };
  if (sign > 0)
    return false;
  ShapeIMap rm;
  for (const TopoDS_Shape& f : removed)
  {
    if (BRepAdaptor_Surface(TopoDS::Face(f), false).GetType() != GeomAbs_Plane)
      return no("a removed face isn't planar");
    rm.Add(f);
  }
  // Convex: no concave edge (a seam, a hole's, has one face and isn't
  // analysed).
  try
  {
    BRepOffset_Analyse                        an(solid, 0.01);
    TopTools_IndexedDataMapOfShapeListOfShape ef;
    TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_EDGE, TopAbs_FACE, ef);
    for (int i = 1; i <= ef.Extent(); ++i)
    {
      if (ef(i).Extent() < 2)
        continue;
      const TopoDS_Edge& e = TopoDS::Edge(ef.FindKey(i));
      for (const BRepOffset_Interval& iv : an.Type(e))
        if (iv.Type() != ChFiDS_Convex && iv.Type() != ChFiDS_Tangential)
        {
          if (dbg)
          {
            BRepAdaptor_Curve ec(e);
            const gp_Pnt      m = ec.Value(0.5 * (ec.FirstParameter() + ec.LastParameter()));
            std::fprintf(stderr, "regions: edge type %d (curve %d) at %g %g %g\n", (int)iv.Type(), (int)ec.GetType(), m.X(), m.Y(), m.Z());
          }
          return no("a concave edge");
        }
    }
  }
  catch (const Standard_Failure&)
  {
    return no("edge analysis threw");
  }
  Bnd_Box bb;
  BRepBndLib::Add(solid, bb, false);
  const double margin = 2 * t + 1e-3 * std::sqrt(bb.SquareExtent());
  bb.Enlarge(margin);
  double x0, y0, z0, x1, y1, z1;
  bb.Get(x0, y0, z0, x1, y1, z1);
  const double big = 4 * std::sqrt(bb.SquareExtent());
  SourceMap    sources;
  TopoDS_Shape cavity = BRepPrimAPI_MakeBox(gp_Pnt(x0, y0, z0), gp_Pnt(x1, y1, z1)).Shape();
  // Cuts `tool` from the cavity; `src` gives the tool faces their source.
  auto cut_by = [&](const TopoDS_Shape& tool, const SourceMap& src) {
    const KernelTimer timer("regions.cut_cavity");
    const auto        t0 = std::chrono::steady_clock::now();
    BRepAlgoAPI_Cut c;
    build_stoppable(c, cavity, tool);
    if (dbg)
    {
      int nt = 0, nc = 0;
      for (TopExp_Explorer x(tool, TopAbs_FACE); x.More(); x.Next())
        ++nt;
      for (TopExp_Explorer x(cavity, TopAbs_FACE); x.More(); x.Next())
        ++nc;
      std::fprintf(stderr, "regions: cut %d-face tool from %d-face cavity in %.1f ms\n", nt, nc, std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count());
    }
    if (c.HasErrors() || !c.IsDone())
      return no("a boolean failed");
    const HHist h = c.History();
    SourceMap   next;
    auto        carry = [&](const TopoDS_Shape& f, const Source& s) {
      const ShapeList imgs = images_of(h, f);
      for (ShapeList::Iterator it(imgs); it.More(); it.Next())
        if (!it.Value().IsNull())
          next.Bind(it.Value(), s);
    };
    for (SourceMap::Iterator it(sources); it.More(); it.Next())
      carry(it.Key(), it.Value());
    for (SourceMap::Iterator it(src); it.More(); it.Next())
      carry(it.Key(), it.Value());
    cavity  = c.Shape();
    sources = next;
    return true;
  };
  // How far a fillet's or cone's region runs past its face's end at v
  // (`end`: v0 or v1): the margin (beyond a plane square to the axis is
  // outside the part), none where it meets a cone at an angle (a vertical
  // fillet under a chamfer's cone: run on, its sector would take wall from
  // the cone's where the wall is thinner than the fillet's radius).
  ShapeAncMap ef_all;
  TopExp::MapShapesAndAncestors(solid, TopAbs_EDGE, TopAbs_FACE, ef_all);
  auto run_on = [&](const TopoDS_Face& f, const std::function<double(const gp_Pnt&)>& v_of, double end) {
    const double tol = 1e-7 * (1 + std::abs(end)) + 1e-9;
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(x.Current());
      if (BRep_Tool::Degenerated(e))
        continue;
      BRepAdaptor_Curve c(e);
      const double      a = c.FirstParameter(), b = c.LastParameter();
      if (std::abs(v_of(c.Value(a)) - end) > tol || std::abs(v_of(c.Value(0.5 * (a + b))) - end) > tol || std::abs(v_of(c.Value(b)) - end) > tol)
        continue;
      const ShapeList* fl = ef_all.Seek(e);
      if (fl == nullptr)
        continue;
      for (ShapeList::Iterator it(*fl); it.More(); it.Next())
      {
        if (it.Value().IsSame(f))
          continue;
        if (BRepAdaptor_Surface(TopoDS::Face(it.Value()), false).GetType() == GeomAbs_Cone)
          return 0.0;
      }
    }
    return margin;
  };
  std::vector<std::pair<TopoDS_Face, gp_Cylinder>> cylinders;
  std::vector<std::pair<TopoDS_Face, gp_Torus>>    tori;
  std::vector<std::pair<TopoDS_Face, gp_Cone>>     cones;
  std::vector<TopoDS_Shape>                        tubes;
  SourceMap                                        tube_sources;
  std::vector<std::pair<gp_Pnt, gp_Dir>>           halfspaces;
  std::vector<TopoDS_Shape>                        halfspace_faces;
  for (TopExp_Explorer e(solid, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    if (rm.Contains(f))
      continue;
    BRepAdaptor_Surface a(f, false);
    if (a.GetType() == GeomAbs_Cylinder)
    {
      cylinders.emplace_back(f, a.Cylinder());
      continue;
    }
    if (a.GetType() == GeomAbs_Torus)
    {
      tori.emplace_back(f, a.Torus());
      continue;
    }
    if (a.GetType() == GeomAbs_Cone)
    {
      cones.emplace_back(f, a.Cone());
      continue;
    }
    if (a.GetType() != GeomAbs_Plane)
    {
      if (dbg)
      {
        Bnd_Box fb;
        BRepBndLib::Add(f, fb, false);
        double a0, b0, c0, a1, b1, c1;
        fb.Get(a0, b0, c0, a1, b1, c1);
        std::fprintf(stderr, "regions: face type %d, box %g %g %g .. %g %g %g\n", (int)a.GetType(), a0, b0, c0, a1, b1, c1);
      }
      return no("a face isn't a plane, cylinder, cone or torus");
    }
    // Outward normal (a left-handed plane frame's surface normal is minus its
    // axis); the offset plane t inside; everything beyond it goes.
    gp_Dir n = a.Plane().Axis().Direction();
    if (!a.Plane().Position().Direct())
      n.Reverse();
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    // The offset plane, about the solid's centre: the plane's own origin may
    // lie far from the face (an extruded sketch's caps keep the sketch's
    // origin; fs's std_reference shellTop at x = 900 lost its floor when the
    // half-space was centred there, 2220 vs live 2653.5 mm3).
    const gp_Pnt mid(0.5 * (x0 + x1), 0.5 * (y0 + y1), 0.5 * (z0 + z1));
    const gp_Pnt on = a.Plane().Location().Translated(gp_Vec(n) * -t);
    halfspaces.emplace_back(mid.Translated(gp_Vec(n) * gp_Vec(mid, on).Dot(gp_Vec(n))), n);
    halfspace_faces.push_back(f);
  }
  // The planes' cavity: their offsets with the (enlarged) box, one polytope.
  {
    const KernelTimer timer("regions.polytope");
    for (const gp_Dir& d : {gp_Dir(1, 0, 0), gp_Dir(-1, 0, 0), gp_Dir(0, 1, 0), gp_Dir(0, -1, 0), gp_Dir(0, 0, 1), gp_Dir(0, 0, -1)})
    {
      halfspaces.emplace_back(gp_Pnt(d.X() > 0 ? x1 : x0, d.Y() > 0 ? y1 : y0, d.Z() > 0 ? z1 : z0), d);
      halfspace_faces.push_back(TopoDS_Shape());
    }
    std::vector<TopoDS_Shape> pf;
    cavity = polytope(halfspaces, big, pf);
    if (cavity.IsNull() || solid_count(cavity) != 1)
      return no("the planes' polytope failed");
    sources.Clear();
    for (size_t k = 0; k < pf.size(); ++k)
      if (!pf[k].IsNull() && !halfspace_faces[k].IsNull())
        sources.Bind(pf[k], Source{halfspace_faces[k], (uint8_t)REL_GENERATED});
  }
  for (const auto& [f, tor] : tori)
  {
    // A convex fillet all round an axis (a hole's or a boss's rim): the part
    // of its tube farther than R - t from the tube circle is wall.
    double u0, u1, v0, v1;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    const double R = tor.MinorRadius(), M = tor.MajorRadius();
    if (u1 - u0 < 2 * M_PI - 1e-6 || v1 - v0 >= M_PI - 1e-6 || !(M > R))
      return no("a torus isn't a full rim fillet");
    BRepAdaptor_Surface a(f, true);
    gp_Pnt              p;
    gp_Vec              du, dv;
    a.D1(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    const gp_Ax3& ax = tor.Position();
    // The tube circle's point at p's angle.
    const gp_Vec radial(gp_Lin(ax.Axis()).Normal(p).Direction());
    const gp_Pnt tc = ax.Location().Translated(radial * M);
    if (n.Dot(gp_Vec(tc, p)) <= 0)
      return no("a concave torus");
    // The meridian section at u0: the face's arc between v0 and v1, rays past
    // it (short of the axis), back along the arc at R - t (to the tube
    // centre when the wall is at least as thick as the fillet's radius).
    const gp_Pnt c = ax.Location().Translated((gp_Vec(ax.XDirection()) * std::cos(u0) + gp_Vec(ax.YDirection()) * std::sin(u0)) * M);
    const double L = R + 0.5 * std::min(R, M - R), vm = 0.5 * (v0 + v1), wide = L / std::cos(0.5 * (v1 - v0));
    auto         on = [&](double v, double rad) { return ElSLib::Value(u0, v, gp_Torus(ax, M, rad)); };
    BRepBuilderAPI_MakeWire wire;
    const double            ri = R - t;
    if (ri > 1e-9 * R)
    {
      wire.Add(BRepBuilderAPI_MakeEdge(on(v0, ri), on(v0, L)).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(on(v0, L), on(vm, wide)).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(on(vm, wide), on(v1, L)).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(on(v1, L), on(v1, ri)).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(on(v1, ri), on(vm, ri), on(v0, ri)).Value()).Edge());
    }
    else
    {
      BRepBuilderAPI_MakePolygon poly;
      for (const gp_Pnt& q : {c, on(v0, L), on(vm, wide), on(v1, L)})
        poly.Add(q);
      poly.Close();
      wire.Add(poly.Wire());
    }
    if (!wire.IsDone())
      return no("a rim fillet's section failed");
    TopoDS_Shape region = BRepPrimAPI_MakeRevol(BRepBuilderAPI_MakeFace(wire.Wire(), true).Face(), ax.Axis(), 2 * M_PI).Shape();
    SourceMap src;
    for (TopExp_Explorer x(region, TopAbs_FACE); x.More(); x.Next())
      src.Bind(x.Current(), Source{f, (uint8_t)REL_GENERATED});
    if (!cut_by(region, src))
      return no("a boolean failed");
  }
  for (const auto& fc : cylinders)
  {
    const TopoDS_Face& f   = fc.first;
    const gp_Cylinder& cyl = fc.second;
    // Convex fillet: the outward normal points away from the axis.
    double u0, u1, v0, v1;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    BRepAdaptor_Surface a(f, true);
    gp_Pnt              p;
    gp_Vec              du, dv;
    a.D1(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    const gp_Ax3& ax  = cyl.Position();
    const gp_Pnt  pin = ElSLib::Value(0.5 * (u0 + u1), 0.5 * (v0 + v1), cyl);
    const gp_Vec  radial(gp_Lin(ax.Axis()).Normal(pin).Direction());
    const double  r = cyl.Radius();
    if (n.Dot(radial) < 0)
    {
      // A through hole's wall (concave, all round; a hole ending inside the
      // part would have a concave edge): its offset, a tube of radius r + t,
      // extended along the axis until it meets the other offsets, as the
      // planes' are (SHELL-SLOW: qa's plates with a hole took ~1 s in slabs,
      // ~90 ms this way, same volumes).
      if (u1 - u0 < 2 * M_PI - 1e-6)
        return no("a concave cylinder isn't all round");
      gp_Ax2       tax(ElSLib::Value(0, v0 - margin, gp_Cylinder(ax, 0)), ax.Direction(), ax.XDirection());
      TopoDS_Shape tube = BRepPrimAPI_MakeCylinder(tax, r + t, v1 - v0 + 2 * margin).Shape();
      // Where it may cross a sloped offset (a chamfer's plane, a cone) the
      // tube is added to the wall at the end (see there): cut from the
      // cavity it came out off. Elsewhere the cut is exact and cheaper.
      bool sloped = !cones.empty();
      for (const auto& [o, n] : halfspaces)
      {
        const double c = std::abs(gp_Vec(n).Dot(gp_Vec(ax.Direction())));
        sloped = sloped || (c > 1e-9 && c < 1 - 1e-9);
      }
      if (sloped)
      {
        for (TopExp_Explorer x(tube, TopAbs_FACE); x.More(); x.Next())
          tube_sources.Bind(x.Current(), Source{f, (uint8_t)REL_GENERATED});
        tubes.push_back(tube);
        continue;
      }
      SourceMap src;
      for (TopExp_Explorer x(tube, TopAbs_FACE); x.More(); x.Next())
        src.Bind(x.Current(), Source{f, (uint8_t)REL_GENERATED});
      if (!cut_by(tube, src))
        return no("a hole tube's cut failed");
      continue;
    }
    if (u1 - u0 >= M_PI - 1e-6)
      return no("a fillet spans a half turn");
    // The sector [u0, u1] beyond radius r - t, over the face's axial extent
    // and past its ends (`run_on`): a prism of the sector's section (an arc at
    // r - t; the centre when the wall is at least as thick as the fillet's
    // radius: its offset vanishes and the whole sector goes). Out to just past
    // the face: beyond it the sector is outside the part (a reach of `big`
    // cut across the whole cavity, ~30 ms a fillet).
    const double L  = r + t;
    auto         at = [&](double u, double rad, double v) { return ElSLib::Value(u, v, gp_Cylinder(ax, rad)); };
    auto         v_cyl = [&](const gp_Pnt& q) { double qu, qv; ElSLib::Parameters(cyl, q, qu, qv); return qv; };
    const double va = v0 - run_on(f, v_cyl, v0), vb = v1 + run_on(f, v_cyl, v1), um = 0.5 * (u0 + u1),
                 wide = L / std::cos(0.5 * (u1 - u0));
    BRepBuilderAPI_MakeWire wire;
    const double            ri = r - t;
    if (ri > 1e-9 * r)
    {
      wire.Add(BRepBuilderAPI_MakeEdge(at(u0, ri, va), at(u0, L, va)).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(at(u0, L, va), at(um, wide, va)).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(at(um, wide, va), at(u1, L, va)).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(at(u1, L, va), at(u1, ri, va)).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(at(u1, ri, va), at(um, ri, va), at(u0, ri, va)).Value()).Edge());
    }
    else
    {
      BRepBuilderAPI_MakePolygon poly;
      for (const gp_Pnt& q : {at(u0, 0, va), at(u0, L, va), at(um, wide, va), at(u1, L, va)})
        poly.Add(q);
      poly.Close();
      wire.Add(poly.Wire());
    }
    if (!wire.IsDone())
      return no("a fillet's section failed");
    TopoDS_Shape region = BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(wire.Wire(), true).Face(), gp_Vec(ax.Direction()) * (vb - va)).Shape();
    SourceMap src;
    for (TopExp_Explorer x(region, TopAbs_FACE); x.More(); x.Next())
      src.Bind(x.Current(), Source{f, (uint8_t)REL_GENERATED});
    if (!cut_by(region, src))
      return no("a boolean failed");
  }
  for (const auto& fc : cones)
  {
    const TopoDS_Face& f    = fc.first;
    const gp_Cone&     cone = fc.second;
    // A convex cone (a chamfer over a filleted corner, wf-block): the
    // cylinder's sector with a sloped generator. Its meridian section at u0,
    // between the face's line offset t inside and well outside, past its
    // ends as the cylinders' (`run_on`), clipped at the axis, revolved over
    // the face's angle (SHELL-SLOW: wf-block's top face went to slabs, ~3 s).
    double u0, u1, v0, v1;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    if (u1 - u0 >= M_PI - 1e-6)
      return no("a cone spans a half turn");
    BRepAdaptor_Surface a(f, true);
    gp_Pnt              p;
    gp_Vec              du, dv;
    a.D1(u0, 0.5 * (v0 + v1), p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (n.Magnitude() < 1e-12)
      return no("a degenerate cone");
    n.Normalize();
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    const gp_Ax3& ax = cone.Position();
    const gp_Vec  radial(gp_Lin(ax.Axis()).Normal(p).Direction());
    if (n.Dot(radial) <= 0)
      return no("a concave cone");
    auto         v_cone = [&](const gp_Pnt& q) { double qu, qv; ElSLib::Parameters(cone, q, qu, qv); return qv; };
    const double L = t + margin, va = v0 - run_on(f, v_cone, v0), vb = v1 + run_on(f, v_cone, v1);
    auto         on = [&](double v) { return ElSLib::Value(u0, v, cone); };
    std::vector<gp_Pnt> sec{on(va).Translated(n * -t), on(vb).Translated(n * -t), on(vb).Translated(n * L), on(va).Translated(n * L)};
    sec = clip_at_axis(sec, ax.Location(), radial);
    if (sec.size() < 3)
      return no("a cone's section failed");
    const TopoDS_Face face = polygon_face(sec);
    if (face.IsNull())
      return no("a cone's section failed");
    BRepPrimAPI_MakeRevol mk(face, gp_Ax1(ax.Location(), gp_Dir(ax.XDirection().Crossed(ax.YDirection()))), u1 - u0);
    if (!mk.IsDone())
      return no("a cone's region failed");
    TopoDS_Shape region = mk.Shape();
    SourceMap    src;
    for (TopExp_Explorer x(region, TopAbs_FACE); x.More(); x.Next())
      src.Bind(x.Current(), Source{f, (uint8_t)REL_GENERATED});
    if (!cut_by(region, src))
      return no("a boolean failed");
  }
  std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("regions.final_cut");
  BRepAlgoAPI_Cut c;
  build_stoppable(c, solid, cavity);
  if (c.HasErrors() || !c.IsDone())
    return no("a boolean failed");
  TopoDS_Shape       walls = c.Shape();
  std::vector<HHist> chain{c.History()};
  if (!tubes.empty())
  {
    // The holes' walls: the part within their offset tubes, added to the
    // cut. Cutting the tubes from the cavity first came out off where a tube
    // crosses a sloped offset plane (OCCT's intersection with the big
    // polytope: a Ø6.35 hole 6 mm from a 1 mm top chamfer, t 1: 3479.08807
    // vs the exact 3479.07658 mm³; this way 3479.07652).
    lap = std::make_unique<KernelTimer>("regions.tubes");
    TopoDS_Compound tc;
    BRep_Builder    bb;
    bb.MakeCompound(tc);
    for (const TopoDS_Shape& x : tubes)
      bb.Add(tc, x);
    // Each tube a tool of its own: they can overlap (close holes), which a
    // compound argument mustn't.
    BRepAlgoAPI_Common   m;
    TopTools_ListOfShape objs, tools;
    objs.Append(solid);
    for (const TopoDS_Shape& x : tubes)
      tools.Append(x);
    m.SetArguments(objs);
    m.SetTools(tools);
    m.Build(new_cancel_indicator()->Start());
    if (m.HasErrors() || !m.IsDone())
      return no("a hole tube's common failed");
    BRepAlgoAPI_Fuse fu;
    build_stoppable(fu, walls, m.Shape());
    if (fu.HasErrors() || !fu.IsDone())
      return no("a hole tube's fuse failed");
    // One history for the cut and the common side by side: each input's
    // images in either.
    const HHist hc = c.History(), hm = m.History();
    HHist       both = new BRepTools_History();
    auto        add = [&](const TopoDS_Shape& x, const ShapeList& imgs) {
      if (imgs.IsEmpty())
        both->Remove(x);
      else if (!(imgs.Extent() == 1 && imgs.First().IsSame(x)))
        for (ShapeList::Iterator it(imgs); it.More(); it.Next())
          both->AddModified(x, it.Value());
    };
    for (const TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    {
      ShapeIMap in, cv, tb;
      TopExp::MapShapes(solid, k, in);
      TopExp::MapShapes(cavity, k, cv);
      TopExp::MapShapes(tc, k, tb);
      for (int i = 1; i <= in.Extent(); ++i)
      {
        ShapeList       imgs = images_of(hc, in(i));
        const ShapeList more = images_of(hm, in(i));
        for (ShapeList::Iterator it(more); it.More(); it.Next())
          if (!imgs.Contains(it.Value()))
            imgs.Append(it.Value());
        add(in(i), imgs);
      }
      for (int i = 1; i <= cv.Extent(); ++i)
        add(cv(i), images_of(hc, cv(i)));
      for (int i = 1; i <= tb.Extent(); ++i)
        add(tb(i), images_of(hm, tb(i)));
    }
    chain = {both, fu.History()};
    walls = fu.Shape();
    for (SourceMap::Iterator it(tube_sources); it.More(); it.Next())
      sources.Bind(it.Key(), it.Value());
  }
  lap = std::make_unique<KernelTimer>("regions.unify");
  // Faces the cut split (a hole's wall tube at its seam, kern's bracket:
  // 21 faces, live 20) merged, as the slabs' are.
  ShapeUpgrade_UnifySameDomain usd(walls, true, true, false);
  usd.SetLinearTolerance(1e-7);
  usd.Build();
  TopoDS_Shape r = single_solid(usd.Shape());
  HHist        hs = usd.History();
  lap            = std::make_unique<KernelTimer>("regions.check");
  // Checked where the cut changed the part (its untouched faces are the
  // input's): the whole-part check cost 20 ms here.
  auto ok = [&](const TopoDS_Shape& x) { return !x.IsNull() && solid_count(x) == 1 && valid_changed(solid, x); };
  if (!ok(r))
  {
    r  = single_solid(walls);
    hs = nullptr;
  }
  if (!ok(r))
    return no("invalid result");
  const double vo = volume_of(solid), vr = volume_of(r);
  if (!(vr > 0 && vr < vo))
    return no("volume out of range");
  lap = std::make_unique<KernelTimer>("regions.history");
  if (!hs.IsNull())
    chain.push_back(hs);
  out.result  = r;
  out.history = shell_history(solid, r, chain, chain, sources, {});
  return true;
}

// ------------------------------------------------------------------ slabs

// Outward normal of `f` at the projection of `p`.
gp_Dir normal_at(const TopoDS_Face& f, const gp_Pnt& p)
{
  GeomAPI_ProjectPointOnSurf pr(p, BRep_Tool::Surface(f));
  double                     u = 0, v = 0;
  if (pr.NbPoints() > 0)
    pr.LowerDistanceParameters(u, v);
  BRepAdaptor_Surface s(f);
  BRepLProp_SLProps   lp(s, u, v, 1, 1e-9);
  gp_Dir              n = lp.IsNormalDefined() ? lp.Normal() : gp_Dir(0, 0, 1);
  return f.Orientation() == TopAbs_REVERSED ? n.Reversed() : n;
}

// A point inside face `f` and its outward normal.
bool inner_point(const TopoDS_Face& f, gp_Pnt& p, gp_Dir& n)
{
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  BRepAdaptor_Surface s(f);
  for (int k = 0; k < 49; ++k)
  {
    const double fu = k == 0 ? 0.5 : ((k % 7) + 0.5) / 7.0, fv = k == 0 ? 0.5 : ((k / 7) + 0.5) / 7.0;
    gp_Pnt2d     uv(u0 + (u1 - u0) * fu, v0 + (v1 - v0) * fv);
    BRepClass_FaceClassifier fc(f, uv, 1e-9);
    if (fc.State() != TopAbs_IN)
      continue;
    BRepLProp_SLProps lp(s, uv.X(), uv.Y(), 1, 1e-9);
    if (!lp.IsNormalDefined())
      continue;
    n = f.Orientation() == TopAbs_REVERSED ? lp.Normal().Reversed() : lp.Normal();
    p = lp.Value();
    return true;
  }
  return false;
}

// +1 when `e` runs forward in face `f` (the face lies to the left of it,
// seen from the outward normal), -1 otherwise.
double edge_sense(const TopoDS_Edge& e, const TopoDS_Face& f)
{
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
    if (x.Current().IsSame(e))
      return x.Current().Orientation() == TopAbs_REVERSED ? -1.0 : 1.0;
  return 1.0;
}

// The solid swept by face `f` moving `d` along its outward normal (d < 0:
// into the solid). Faces of the slab other than `f` itself are recorded as
// Generated from `src`.
// The centre a point of a convex blend face rounds: on the axis of a
// cylinder or cone, on the tube circle of a torus. False for other faces.
bool blend_centre(const BRepAdaptor_Surface& s, const gp_Pnt& p, gp_Pnt& c)
{
  switch (s.GetType())
  {
    case GeomAbs_Cylinder:
    case GeomAbs_Cone: {
      const gp_Lin ax(s.GetType() == GeomAbs_Cylinder ? s.Cylinder().Axis() : s.Cone().Axis());
      c = ElCLib::Value(ElCLib::Parameter(ax, p), ax);
      return true;
    }
    case GeomAbs_Sphere:
      c = s.Sphere().Location();
      return true;
    case GeomAbs_Torus: {
      const gp_Torus tor = s.Torus();
      const gp_Ax1   ax = tor.Axis();
      const gp_Lin   l(ax);
      const gp_Pnt   foot = ElCLib::Value(ElCLib::Parameter(l, p), l);
      gp_Vec         radial(foot, p);
      const gp_Vec   along(ax.Direction());
      radial -= along * radial.Dot(along);
      if (radial.Magnitude() < 1e-12)
        return false;
      c = tor.Location().Translated(radial.Normalized() * tor.MajorRadius());
      return true;
    }
    default:
      return false;
  }
}

// A convex cylinder, cone or torus face (a fillet, a chamfer's corner cone:
// its outward normal points away from its centre) whose offset by `d`
// (d < 0: inward) reaches or passes its centre everywhere: the face lies
// within |d| of its axis (cylinder, cone) or tube circle (torus).
bool collapsing_blend(const TopoDS_Face& f, double d)
{
  if (!(d < 0))
    return false;
  BRepAdaptor_Surface s(f);
  gp_Pnt              p, c;
  gp_Dir              n;
  const bool          inner = inner_point(f, p, n);
  const bool          centre = inner && blend_centre(s, p, c);
  if (std::getenv("NOSHAPE_SHELL_DEBUG") && s.GetType() == GeomAbs_Cone)
    std::fprintf(stderr, "slabs: cone: inner %d centre %d convex %g\n", (int)inner, (int)centre, centre ? gp_Vec(c, p).Dot(gp_Vec(n)) : 0.0);
  if (!centre || !(gp_Vec(c, p).Dot(gp_Vec(n)) > 0))
    return false;
  if (s.GetType() == GeomAbs_Torus)
    return -d >= s.Torus().MinorRadius() * (1 - 1e-9);
  if (s.GetType() == GeomAbs_Sphere)
    return -d >= s.Sphere().Radius() * (1 - 1e-9);
  // The farthest point from the axis, on the face's edges (its UV box can
  // reach past a cone's apex triangle: P1-16's chamfer corners).
  const gp_Lin ax(s.GetType() == GeomAbs_Cylinder ? s.Cylinder().Axis() : s.Cone().Axis());
  double       rmax = 0;
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    if (BRep_Tool::Degenerated(TopoDS::Edge(x.Current())))
      continue;
    BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
    for (int k = 0; k <= 8; ++k)
      rmax = std::max(rmax, ax.Distance(c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / 8)));
  }
  if (std::getenv("NOSHAPE_SHELL_DEBUG") && s.GetType() == GeomAbs_Cone)
    std::fprintf(stderr, "slabs: cone rmax %g, wall %g\n", rmax, -d);
  return rmax > 0 && -d >= rmax * (1 - 1e-6);
}

// The solid sector of a convex blend face: everything between the face and
// its centre over the face's (u, v) box (a cylinder or cone wedge from the
// axis; for a torus its tube's disc sector revolved about the axis). Exact
// primitives: a revolved section with an edge on the axis made the
// booleans fail at a cone's apex (qa5).
TopoDS_Shape blend_sector(const TopoDS_Face& f)
{
  BRepAdaptor_Surface s(f);
  double              u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  if (!(u1 - u0 > 1e-9) || !(v1 - v0 > 1e-12))
    return TopoDS_Shape();
  gp_Ax1 axis;
  if (s.GetType() == GeomAbs_Cylinder)
    axis = s.Cylinder().Axis();
  else if (s.GetType() == GeomAbs_Cone)
    axis = s.Cone().Axis();
  else if (s.GetType() == GeomAbs_Torus)
    axis = s.Torus().Axis();
  else
    return TopoDS_Shape();
  // The surface's angle u runs about its axis in the sense of the frame's
  // z: rotating a point by +du about `axis` lands on u + du when the frame
  // is direct.
  const gp_Ax3 frame = s.GetType() == GeomAbs_Cylinder ? s.Cylinder().Position() : s.GetType() == GeomAbs_Cone ? s.Cone().Position() : s.Torus().Position();
  const gp_Dir z = frame.Direct() ? axis.Direction() : axis.Direction().Reversed();
  const gp_Ax1 turn(axis.Location(), z);
  TopoDS_Shape r;
  if (s.GetType() == GeomAbs_Torus)
  {
    // The tube's disc sector at u0: centre, the arc from v0 to v1.
    const gp_Pnt a = s.Value(u0, v0), m = s.Value(u0, (v0 + v1) / 2), b = s.Value(u0, v1);
    gp_Pnt       c;
    if (!blend_centre(s, m, c))
      return TopoDS_Shape();
    TopoDS_Wire w;
    try
    {
      occ::handle<Geom_TrimmedCurve> arc = GC_MakeArcOfCircle(a, m, b).Value();
      BRepBuilderAPI_MakeWire        mw(BRepBuilderAPI_MakeEdge(c, a).Edge(), BRepBuilderAPI_MakeEdge(arc).Edge(), BRepBuilderAPI_MakeEdge(b, c).Edge());
      if (!mw.IsDone())
        return TopoDS_Shape();
      w = mw.Wire();
    }
    catch (const Standard_Failure&)
    {
      return TopoDS_Shape();
    }
    BRepBuilderAPI_MakeFace mf(w, true);
    if (!mf.IsDone())
      return TopoDS_Shape();
    BRepPrimAPI_MakeRevol mk(mf.Face(), turn, u1 - u0);
    if (!mk.IsDone())
      return TopoDS_Shape();
    r = mk.Shape();
  }
  else
  {
    const gp_Lin ax(axis);
    const gp_Pnt a0 = s.Value(u0, v0), a1 = s.Value(u0, v1);
    double       h0 = ElCLib::Parameter(ax, a0), h1 = ElCLib::Parameter(ax, a1);
    double       r0 = ax.Distance(a0), r1 = ax.Distance(a1);
    if (s.GetType() == GeomAbs_Cone)
    {
      // The axial extent from the face's edges, the radii from the cone
      // (the UV box can reach past an apex triangle's apex: P1-16).
      h0 = 1e300;
      h1 = -1e300;
      for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
      {
        BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
        for (int k = 0; k <= 8; ++k)
        {
          const double h = ElCLib::Parameter(ax, c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / 8));
          h0 = std::min(h0, h);
          h1 = std::max(h1, h);
        }
      }
      const gp_Cone cone = s.Cone();
      const double  t    = std::tan(cone.SemiAngle());
      const double  hl   = ElCLib::Parameter(ax, cone.Location());
      auto          rad  = [&](double h) {
        const double r = cone.RefRadius() + (h - hl) * t;
        return std::abs(r) < 1e-9 * (1 + cone.RefRadius()) ? 0.0 : std::abs(r);
      };
      r0 = rad(h0);
      r1 = rad(h1);
      if (!(h1 > h0) || (r0 == 0 && r1 == 0))
        return TopoDS_Shape();
    }
    // Base at the lower end along z, x towards the face's u0 side.
    const double hz0 = h0 * axis.Direction().Dot(z), hz1 = h1 * axis.Direction().Dot(z);
    const bool   base0 = hz0 <= hz1;
    const gp_Pnt base = ElCLib::Value(base0 ? h0 : h1, ax);
    // Towards the face's u0 side: the frame's direction at angle u0.
    const gp_Vec xv = gp_Vec(frame.XDirection()) * std::cos(u0) + gp_Vec(frame.YDirection()) * std::sin(u0);
    const double H  = std::abs(h1 - h0);
    if (xv.Magnitude() < 1e-12 || !(H > 1e-12))
      return TopoDS_Shape();
    const gp_Ax2 fr(base, z, gp_Dir(xv));
    // A cone's sector as the cylinder wedge of its larger radius: the
    // common with the solid trims the rest, and a wedge with the cone's apex
    // made the booleans fail (P1-16's chamfer corners).
    r = BRepPrimAPI_MakeCylinder(fr, s.GetType() == GeomAbs_Cylinder ? s.Cylinder().Radius() : std::max(r0, r1), H, u1 - u0).Shape();
  }
  if (r.IsNull() || !BRepCheck_Analyzer(r).IsValid() || !(volume_of(r) > 0))
    return TopoDS_Shape();
  return r;
}

TopoDS_Face polygon_face(const std::vector<gp_Pnt>& pts);

// The slab of a cylinder or cone face bounded by two of its parallels and two
// of its rulings (closed faces are split first; a cone's may end at its apex),
// exact: its section in the plane of the first ruling (a parallelogram: the
// normal is the same along a ruling) revolved about the axis. OCCT's simple
// offset makes a cone's walls along the parallels B-spline surfaces and a
// cylinder's edges B-spline curves, and the slabs' fuse took seconds on them
// (Shell on a chamfered part: 5–13 s). Null when the face is anything else.
TopoDS_Shape revolved_slab(const TopoDS_Face& f, double d)
{
  BRepAdaptor_Surface s(f);
  if (s.GetType() != GeomAbs_Cone && s.GetType() != GeomAbs_Cylinder)
    return TopoDS_Shape();
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  if (!(u1 - u0 > 1e-6) || !(u1 - u0 < 2 * M_PI - 1e-6) || !(v1 - v0 > 1e-9))
    return TopoDS_Shape();
  int wires = 0;
  for (TopExp_Explorer w(f, TopAbs_WIRE); w.More(); w.Next())
    ++wires;
  if (wires != 1)
    return TopoDS_Shape();
  const double tu = 1e-9 * (1 + std::abs(u0) + std::abs(u1)), tv = 1e-9 * (1 + std::abs(v0) + std::abs(v1));
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(x.Current());
    double                     a, b;
    const Handle(Geom2d_Curve) c = BRep_Tool::CurveOnSurface(e, f, a, b);
    if (c.IsNull())
      return TopoDS_Shape();
    Geom2dAdaptor_Curve ac(c, a, b);
    if (ac.GetType() != GeomAbs_Line)
      return TopoDS_Shape();
    const gp_Lin2d l = ac.Line();
    if (std::abs(l.Direction().Y()) < 1e-12)
    {
      const double v = l.Location().Y();
      if (std::abs(v - v0) > tv && std::abs(v - v1) > tv)
        return TopoDS_Shape();
    }
    else if (std::abs(l.Direction().X()) < 1e-12)
    {
      const double u = l.Location().X();
      if (std::abs(u - u0) > tu && std::abs(u - u1) > tu)
        return TopoDS_Shape();
    }
    else
      return TopoDS_Shape();
  }
  gp_Pnt pm;
  gp_Vec du, dv;
  s.D1(u0, (v0 + v1) / 2, pm, du, dv);
  gp_Vec n = du.Crossed(dv);
  if (n.Magnitude() < 1e-12)
    return TopoDS_Shape();
  n.Normalize();
  if (f.Orientation() == TopAbs_REVERSED)
    n.Reverse();
  const gp_Pnt pa = s.Value(u0, v0), pb = s.Value(u0, v1);
  const gp_Ax3 ax3 = s.GetType() == GeomAbs_Cone ? s.Cone().Position() : s.Cylinder().Position();
  const gp_Ax1 axis(ax3.Location(), ax3.Direct() ? ax3.Direction() : ax3.Direction().Reversed());
  // The section stays on one side of the axis (a cone's apex on it).
  const gp_Lin al(axis);
  auto         radial_of = [&](const gp_Pnt& p) { return gp_Vec(ElCLib::Value(ElCLib::Parameter(al, p), al), p); };
  gp_Vec       radial = radial_of(pa).Magnitude() > radial_of(pb).Magnitude() ? radial_of(pa) : radial_of(pb);
  if (radial.Magnitude() < 1e-7)
    return TopoDS_Shape();
  radial.Normalize();
  const std::vector<gp_Pnt> quad = {pa, pb, pb.Translated(n * d), pa.Translated(n * d)};
  int                       on_axis = 0;
  for (const gp_Pnt& q : quad)
  {
    const double side = gp_Vec(axis.Location(), q).Dot(radial);
    if (side < -1e-9)
      return TopoDS_Shape();
    if (side <= 1e-7)
    {
      if (al.Distance(q) > 1e-9)
        return TopoDS_Shape();
      ++on_axis;
    }
  }
  // Only the apex (one of the face's corners) may lie on the axis.
  if (on_axis > 1 || (on_axis == 1 && al.Distance(pa) > 1e-9 && al.Distance(pb) > 1e-9))
    return TopoDS_Shape();
  const TopoDS_Face face = polygon_face(quad);
  if (face.IsNull())
    return TopoDS_Shape();
  BRepPrimAPI_MakeRevol mk(face, axis, u1 - u0);
  if (!mk.IsDone())
    return TopoDS_Shape();
  TopoDS_Shape r = mk.Shape();
  if (r.ShapeType() != TopAbs_SOLID)
    return TopoDS_Shape();
  // Its corners on the face's are the face's to the last bit, and its straight edges run through them
  // (rotation rounds them 1e-16 off: OCCT then grows tolerances where they meet other slabs' edges, and
  // the Rust Boolean declines such meetings).
  std::vector<gp_Pnt> corners;
  for (TopExp_Explorer x(f, TopAbs_VERTEX); x.More(); x.Next())
    corners.push_back(BRep_Tool::Pnt(TopoDS::Vertex(x.Current())));
  BRep_Builder bb;
  ShapeIMap    vs, es;
  TopExp::MapShapes(r, TopAbs_VERTEX, vs);
  TopExp::MapShapes(r, TopAbs_EDGE, es);
  for (int i = 1; i <= vs.Extent(); ++i)
  {
    const TopoDS_Vertex& v = TopoDS::Vertex(vs(i));
    const gp_Pnt         p = BRep_Tool::Pnt(v);
    for (const gp_Pnt& c : corners)
      if (p.Distance(c) < 1e-9 && p.Distance(c) > 0)
      {
        bb.UpdateVertex(v, c, BRep_Tool::Tolerance(v));
        break;
      }
  }
  for (int i = 1; i <= es.Extent(); ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(es(i));
    double             a, b;
    TopLoc_Location    loc;
    const Handle(Geom_Curve) c = BRep_Tool::Curve(e, loc, a, b);
    if (c.IsNull() || !loc.IsIdentity() || c->DynamicType() != STANDARD_TYPE(Geom_Line))
      continue;
    TopoDS_Vertex v0, v1;
    TopExp::Vertices(e, v0, v1);
    const gp_Pnt p0 = BRep_Tool::Pnt(v0), p1 = BRep_Tool::Pnt(v1);
    if (v0.Orientation() != TopAbs_FORWARD || p0.Distance(p1) < 1e-9 || std::abs(p0.Distance(p1) - (b - a)) > 1e-9)
      continue;
    const gp_Dir dir(gp_Vec(p0, p1));
    bb.UpdateEdge(e, new Geom_Line(p0.Translated(gp_Vec(dir) * -a), dir), BRep_Tool::Tolerance(e));
  }
  return r;
}

TopoDS_Shape slab(const TopoDS_Face& f, double d, const TopoDS_Shape& src, SourceMap& sources)
{
  gp_Pnt p;
  gp_Dir n;
  if (!inner_point(f, p, n))
    return TopoDS_Shape();
  BRepAdaptor_Surface s(f);
  TopoDS_Shape        r;
  if (collapsing_blend(f, d))
  {
    // A convex blend whose inward offset reaches its axis (the wall at
    // least as thick as the fillet radius): every point of its sector is
    // within the wall, so the slab is the whole sector, axis to face
    // (qa5 P1-16: R1 fillets shelled 1 and 2.5 mm; the offset collapses
    // to the axis or vanishes, and the sector was left out of the wall).
    r = blend_sector(f);
    if (std::getenv("NOSHAPE_SHELL_DEBUG"))
      std::fprintf(stderr, "slabs: collapsing blend type %d: sector %s volume %.6g\n", (int)s.GetType(), r.IsNull() ? "failed" : "ok", r.IsNull() ? 0.0 : volume_of(r));
    if (r.IsNull())
      return TopoDS_Shape();
  }
  else if (s.GetType() == GeomAbs_Plane)
  {
    BRepPrimAPI_MakePrism mk(f, gp_Vec(n) * d);
    if (!mk.IsDone())
      return TopoDS_Shape();
    r = mk.Shape();
  }
  else
  {
    // OCCT's simple offset follows the surface normal, not always the face
    // orientation: try both signs and keep the slab on the requested side.
    const gp_Pnt q = p.Translated(gp_Vec(n) * (d * 0.5));
    if (!std::getenv("NOSHAPE_SHELL_OCCT_SLABS"))
    {
      r = revolved_slab(f, d);
      if (!r.IsNull())
      {
        BRepClass3d_SolidClassifier cl(r, q, 1e-7);
        if (cl.State() != TopAbs_IN || !BRepCheck_Analyzer(r).IsValid())
          r.Nullify();
      }
    }
    if (r.IsNull() && std::getenv("NOSHAPE_SHELL_DEBUG"))
      std::fprintf(stderr, "slabs: simple offset slab for a face of type %d\n", (int)s.GetType());
    for (double sg : {1.0, -1.0})
    {
      if (!r.IsNull())
        break;
      BRepOffsetAPI_MakeThickSolid mk;
      mk.MakeThickSolidBySimple(f, sg * d);
      if (!mk.IsDone())
        continue;
      TopoDS_Shape c = solid_of(mk.Shape());
      if (c.IsNull())
        continue;
      BRepClass3d_SolidClassifier cl(c, q, 1e-7);
      if (cl.State() == TopAbs_IN)
      {
        r = c;
        break;
      }
    }
    if (r.IsNull() || !BRepCheck_Analyzer(r).IsValid())
      return TopoDS_Shape();
  }
  // Independent copy: booleans mishandle a tool sharing a face with the object.
  BRepBuilderAPI_Copy cp(r);
  r = cp.Shape();
  for (TopExp_Explorer e(r, TopAbs_FACE); e.More(); e.Next())
    sources.Bind(e.Current(), Source{src, (uint8_t)REL_GENERATED});
  return r;
}

TopoDS_Face polygon_face(const std::vector<gp_Pnt>& pts)
{
  BRepBuilderAPI_MakePolygon poly;
  for (const gp_Pnt& p : pts)
    poly.Add(p);
  poly.Close();
  BRepBuilderAPI_MakeFace mf(poly.Wire(), true);
  return mf.IsDone() ? mf.Face() : TopoDS_Face();
}

// Keep the part of a polygon (in a plane containing `axis`) on the side of
// the axis given by `radial`.
std::vector<gp_Pnt> clip_at_axis(const std::vector<gp_Pnt>& in, const gp_Pnt& c, const gp_Vec& radial)
{
  std::vector<gp_Pnt> r;
  const size_t        n = in.size();
  auto                side = [&](const gp_Pnt& p) { return gp_Vec(c, p).Dot(radial); };
  for (size_t i = 0; i < n; ++i)
  {
    const gp_Pnt &a = in[i], &b = in[(i + 1) % n];
    const double  sa = side(a), sb = side(b);
    if (sa >= 0)
      r.push_back(a);
    if ((sa >= 0) != (sb >= 0))
    {
      const double k = sa / (sa - sb);
      r.push_back(gp_Pnt(a.XYZ() + (b.XYZ() - a.XYZ()) * k));
    }
  }
  return r;
}

double polygon_area(const std::vector<gp_Pnt>& p)
{
  gp_Vec a(0, 0, 0);
  for (size_t i = 1; i + 1 < p.size(); ++i)
    a += gp_Vec(p[0], p[i]).Crossed(gp_Vec(p[0], p[i + 1]));
  return a.Magnitude() / 2;
}

// Sweep a cross-section polygon along edge `e` (section(param) gives it at
// a parameter). Each polygon side's face gets the source given by `owner`.
using Section = std::function<std::vector<gp_Pnt>(double)>;
TopoDS_Shape sweep_section(const TopoDS_Edge& e, const Section& section,
                           const std::vector<TopoDS_Shape>& owner, SourceMap& sources)
{
  BRepAdaptor_Curve c(e);
  const double      a = c.FirstParameter(), b = c.LastParameter();
  const double      scale = std::max(1e-9, c.Value(a).Distance(c.Value(b)));
  auto              ok = [&](const std::vector<gp_Pnt>& p) { return p.size() >= 3 && polygon_area(p) > 1e-10 * scale * scale; };
  TopoDS_Shape      r;
  std::vector<TopoDS_Shape> sides;
  if (c.GetType() == GeomAbs_Line || c.GetType() == GeomAbs_Circle)
  {
    std::vector<gp_Pnt> s0 = section(a);
    if (c.GetType() == GeomAbs_Circle)
    {
      const gp_Circ ci = c.Circle();
      s0 = clip_at_axis(s0, ci.Location(), gp_Vec(ci.Location(), c.Value(a)));
    }
    if (!ok(s0))
      return TopoDS_Shape();
    TopoDS_Face face = polygon_face(s0);
    if (face.IsNull())
      return TopoDS_Shape();
    std::vector<TopoDS_Shape> edges;
    for (BRepTools_WireExplorer w(BRepTools::OuterWire(face)); w.More(); w.Next())
      edges.push_back(w.Current());
    if (c.GetType() == GeomAbs_Line)
    {
      BRepPrimAPI_MakePrism mk(face, gp_Vec(c.Value(a), c.Value(b)));
      if (!mk.IsDone())
        return TopoDS_Shape();
      r = mk.Shape();
      for (const TopoDS_Shape& x : edges)
        sides.push_back(mk.Generated(x).IsEmpty() ? TopoDS_Shape() : mk.Generated(x).First());
    }
    else
    {
      BRepPrimAPI_MakeRevol mk(face, c.Circle().Axis(), b - a);
      if (!mk.IsDone())
        return TopoDS_Shape();
      r = mk.Shape();
      for (const TopoDS_Shape& x : edges)
        sides.push_back(mk.Generated(x).IsEmpty() ? TopoDS_Shape() : mk.Generated(x).First());
    }
    // Polygon sides follow the section points except where clipping
    // inserted some; map by nearest midpoint to the section's sides.
    std::vector<gp_Pnt> s1 = section(a);
    for (size_t i = 0; i < sides.size() && i < edges.size(); ++i)
    {
      if (sides[i].IsNull())
        continue;
      BRepAdaptor_Curve ec(TopoDS::Edge(edges[i]));
      gp_Pnt            m = ec.Value((ec.FirstParameter() + ec.LastParameter()) / 2);
      size_t            best = 0;
      double            bd = 1e300;
      for (size_t j = 0; j < s1.size(); ++j)
      {
        const gp_Pnt &p = s1[j], &q = s1[(j + 1) % s1.size()];
        gp_Vec        pq(p, q);
        double        l2 = pq.SquareMagnitude();
        double        k = l2 > 0 ? std::clamp(gp_Vec(p, m).Dot(pq) / l2, 0.0, 1.0) : 0.0;
        double        dd = m.Distance(gp_Pnt(p.XYZ() + pq.XYZ() * k));
        if (dd < bd)
        {
          bd = dd;
          best = j;
        }
      }
      if (best < owner.size() && !owner[best].IsNull())
        sources.Bind(sides[i], Source{owner[best], (uint8_t)REL_GENERATED});
    }
    return r;
  }
  BRepOffsetAPI_ThruSections ts(true, true);
  const int                  n = 8;
  for (int i = 0; i <= n; ++i)
  {
    std::vector<gp_Pnt> s = section(a + (b - a) * i / n);
    if (!ok(s))
      return TopoDS_Shape();
    BRepBuilderAPI_MakePolygon poly;
    for (const gp_Pnt& p : s)
      poly.Add(p);
    poly.Close();
    ts.AddWire(poly.Wire());
  }
  ts.Build();
  if (!ts.IsDone())
    return TopoDS_Shape();
  r = ts.Shape();
  for (TopExp_Explorer x(r, TopAbs_FACE); x.More(); x.Next())
    if (!owner.empty() && !owner[0].IsNull())
      sources.Bind(x.Current(), Source{owner[0], (uint8_t)REL_GENERATED});
  return r;
}

// The bounded convex polyhedron { p : (p - o).m <= 0 for every (o, m) }, as
// a solid (null when empty or degenerate). `size` sets the tolerances.
TopoDS_Shape convex_polyhedron(const std::vector<std::pair<gp_Pnt, gp_Vec>>& planes, double size)
{
  const double        eps = 1e-9 * std::max(1.0, size);
  const size_t        n   = planes.size();
  std::vector<gp_Vec> m;
  std::vector<double> c;
  for (const auto& [o, v] : planes)
  {
    gp_Vec u = v.Normalized();
    m.push_back(u);
    c.push_back(u.Dot(gp_Vec(o.XYZ())));
  }
  std::vector<gp_Pnt> pts;
  for (size_t i = 0; i < n; ++i)
    for (size_t j = i + 1; j < n; ++j)
      for (size_t k = j + 1; k < n; ++k)
      {
        const double det = m[i].Dot(m[j].Crossed(m[k]));
        if (std::abs(det) < 1e-9)
          continue;
        gp_Vec p = (m[j].Crossed(m[k]) * c[i] + m[k].Crossed(m[i]) * c[j] + m[i].Crossed(m[j]) * c[k]) / det;
        bool   in = true;
        for (size_t q = 0; q < n && in; ++q)
          in = m[q].Dot(p) - c[q] <= eps * 10;
        if (!in)
          continue;
        gp_Pnt pp(p.XYZ());
        bool   dup = false;
        for (const gp_Pnt& x : pts)
          dup = dup || x.Distance(pp) < eps * 100;
        if (!dup)
          pts.push_back(pp);
      }
  if (pts.size() < 4)
    return TopoDS_Shape();
  BRepBuilderAPI_Sewing sew(eps * 1000);
  int                   faces = 0;
  for (size_t q = 0; q < n; ++q)
  {
    std::vector<gp_Pnt> on;
    for (const gp_Pnt& p : pts)
      if (std::abs(m[q].Dot(gp_Vec(p.XYZ())) - c[q]) < eps * 100)
        on.push_back(p);
    if (on.size() < 3)
      continue;
    gp_XYZ ctr(0, 0, 0);
    for (const gp_Pnt& p : on)
      ctr += p.XYZ();
    ctr /= (double)on.size();
    gp_Dir x;
    bool   have_x = false;
    for (const gp_Pnt& p : on)
      if ((p.XYZ() - ctr).Modulus() > eps * 100)
      {
        x      = gp_Dir(p.XYZ() - ctr);
        have_x = true;
        break;
      }
    if (!have_x)
      continue;
    const gp_Vec y = m[q].Crossed(gp_Vec(x));
    std::sort(on.begin(), on.end(), [&](const gp_Pnt& a, const gp_Pnt& b) {
      const gp_Vec va(a.XYZ() - ctr), vb(b.XYZ() - ctr);
      return std::atan2(va.Dot(y), va.Dot(gp_Vec(x))) < std::atan2(vb.Dot(y), vb.Dot(gp_Vec(x)));
    });
    if (polygon_area(on) < eps * eps * 100)
      continue;
    TopoDS_Face f = polygon_face(on);
    if (f.IsNull())
      continue;
    sew.Add(f);
    ++faces;
  }
  if (faces < 4)
    return TopoDS_Shape();
  sew.Perform();
  TopoDS_Shape r = solid_of(sew.SewedShape());
  if (r.IsNull() || !BRepCheck_Analyzer(r).IsValid() || !(volume_of(r) > 0))
    return TopoDS_Shape();
  return r;
}

// A cone or cylinder band `f` (two arcs about its axis joined by two
// generatrix lines) with one arc on a removed face: its slab plus the filler
// up to the removed face's plane, as one revolved section clipped at the axis.
// Marks `f` and the arc in `done`. Null when `f` isn't such a band.
TopoDS_Shape revolved_band(const TopoDS_Face& f, const ShapeAncMap& ef, const ShapeIMap& rm, double d,
                           SourceMap& sources, ShapeIMap& done)
{
  if (collapsing_blend(f, d))
    return TopoDS_Shape();
  BRepAdaptor_Surface s(f);
  gp_Ax1              axis;
  if (s.GetType() == GeomAbs_Cone)
    axis = s.Cone().Axis();
  else if (s.GetType() == GeomAbs_Cylinder)
    axis = s.Cylinder().Axis();
  else
    return TopoDS_Shape();
  std::vector<TopoDS_Edge> arcs;
  int                      lines = 0;
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(x.Current());
    if (BRep_Tool::Degenerated(e))
      return TopoDS_Shape();
    BRepAdaptor_Curve c(e);
    if (c.GetType() == GeomAbs_Circle && c.Circle().Axis().IsCoaxial(axis, 1e-9, 1e-9 * (1 + c.Circle().Radius())))
      arcs.push_back(e);
    else if (c.GetType() == GeomAbs_Line)
      ++lines;
    else
      return TopoDS_Shape();
  }
  if (arcs.size() != 2 || lines != 2)
    return TopoDS_Shape();
  // The arc on a removed face.
  int         k = -1;
  TopoDS_Face g;
  for (int i = 0; i < 2 && k < 0; ++i)
  {
    const ShapeList* fl = ef.Seek(arcs[i]);
    if (fl == nullptr)
      continue;
    for (ShapeList::Iterator it(*fl); it.More(); it.Next())
      if (!it.Value().IsSame(f) && rm.Contains(it.Value()))
      {
        k = i;
        g = TopoDS::Face(it.Value());
      }
  }
  if (k < 0)
    return TopoDS_Shape();
  const TopoDS_Edge& e = arcs[k];
  BRepAdaptor_Curve  c(e), c2(arcs[1 - k]);
  const double       a = c.FirstParameter(), b = c.LastParameter();
  gp_Pnt             E;
  gp_Vec             T;
  c.D1(a, E, T);
  T.Normalize();
  const gp_Circ ci = c.Circle(), ci2 = c2.Circle();
  const gp_Vec  radial = gp_Vec(ci.Location(), E).Normalized();
  const gp_Pnt  P0 = ci2.Location().Translated(radial * ci2.Radius());
  const gp_Vec  n0(normal_at(f, P0)), n1(normal_at(f, E)), ng(normal_at(g, E));
  const gp_Vec  u = -(n1.Crossed(T * edge_sense(e, f)));
  std::vector<gp_Pnt> poly{P0, E};
  const double        un = u.Dot(ng);
  const gp_Pnt        B = E.Translated(n1 * d);
  double              ext = 0;
  if (un > 1e-9)
    ext = std::min(std::max(0.0, -d * n1.Dot(ng) / un), 20 * std::abs(d));
  if (ext > 1e-9 * std::abs(d))
    poly.push_back(B.Translated(u * ext));
  else
    poly.push_back(B);
  poly.push_back(P0.Translated(n0 * d));
  poly = clip_at_axis(poly, ci.Location(), radial);
  if (poly.size() < 3 || polygon_area(poly) < 1e-12 * d * d)
    return TopoDS_Shape();
  TopoDS_Face sec = polygon_face(poly);
  if (sec.IsNull())
    return TopoDS_Shape();
  BRepPrimAPI_MakeRevol mk(sec, ci.Axis(), b - a);
  if (!mk.IsDone())
    return TopoDS_Shape();
  TopoDS_Shape r = mk.Shape();
  if (!BRepCheck_Analyzer(r).IsValid() || !(volume_of(r) > 0))
    return TopoDS_Shape();
  for (TopExp_Explorer x(r, TopAbs_FACE); x.More(); x.Next())
    sources.Bind(x.Current(), Source{f, (uint8_t)REL_GENERATED});
  done.Add(f);
  done.Add(e);
  return r;
}

// Where two lines meet in their common plane (closest point of the first).
bool meet(const gp_Pnt& p, const gp_Vec& a, const gp_Pnt& q, const gp_Vec& b, gp_Pnt& out)
{
  const gp_Vec w(p, q);
  const double aa = a.Dot(a), ab = a.Dot(b), bb = b.Dot(b), aw = a.Dot(w), bw = b.Dot(w);
  const double den = aa * bb - ab * ab;
  if (!(std::abs(den) > 1e-12 * aa * bb))
    return false;
  out = p.Translated(a * ((aw * bb - ab * bw) / den));
  return true;
}

// The walls across a convex cylinder or torus blend whose inward offset
// collapses (|d| at least its radius; live SHELL-WALLS: R2 fillets round a
// plate's top, shelled 2.5 mm with the top removed): Parasolid's offsets
// meet as if the blend weren't there, so a kept neighbour's offset runs on
// to the removed neighbour's surface, or to the other kept neighbour's
// offset. Swept along the blend's spine (its axis, or a torus's tube
// circle), the section at a station is the polygon between the
// neighbours' tangent lines at the blend arc's ends (meeting at the sharp
// corner C) and their offsets. The common with the solid trims it.
TopoDS_Shape collapsed_blend_filler(const TopoDS_Face& f, const ShapeAncMap& ef, const ShapeIMap& rm, double d,
                                    SourceMap& sources)
{
  BRepAdaptor_Surface s(f);
  const bool          torus = s.GetType() == GeomAbs_Torus;
  if (!torus && s.GetType() != GeomAbs_Cylinder)
    return TopoDS_Shape();
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  const gp_Ax3 fr = torus ? s.Torus().Position() : s.Cylinder().Position();
  // The spine edge, parameterised like the surface's spine parameter (v on
  // a cylinder, u on a torus).
  TopoDS_Edge spine;
  if (torus)
  {
    const gp_Dir zd = fr.Direct() ? fr.Direction() : fr.Direction().Reversed();
    spine = BRepBuilderAPI_MakeEdge(gp_Circ(gp_Ax2(fr.Location(), zd, fr.XDirection()), s.Torus().MajorRadius()), u0, u1).Edge();
  }
  else
    spine = BRepBuilderAPI_MakeEdge(gp_Lin(fr.Location(), fr.Direction()), v0, v1).Edge();
  // The arc's ends at a spine parameter: point and direction out of the arc.
  auto ends = [&](double t, gp_Pnt pt[2], gp_Vec dir[2]) {
    gp_Vec du, dv;
    if (torus)
    {
      s.D1(t, v0, pt[0], du, dv);
      dir[0] = -dv;
      s.D1(t, v1, pt[1], du, dv);
      dir[1] = dv;
    }
    else
    {
      s.D1(u0, t, pt[0], du, dv);
      dir[0] = -du;
      s.D1(u1, t, pt[1], du, dv);
      dir[1] = du;
    }
  };
  // The neighbour across the edge through each end (at the mid station).
  const double tm = torus ? (u0 + u1) / 2 : (v0 + v1) / 2;
  gp_Pnt       pm[2];
  gp_Vec       dm[2];
  ends(tm, pm, dm);
  const double tol = 1e-6 * std::abs(d);
  TopoDS_Shape nb[2];
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(x.Current());
    const ShapeList*   fl = ef.Seek(e);
    if (fl == nullptr || BRep_Tool::Degenerated(e) || BRep_Tool::IsClosed(e, f))
      continue;
    for (ShapeList::Iterator it(*fl); it.More(); it.Next())
    {
      if (it.Value().IsSame(f))
        continue;
      for (int k = 0; k < 2; ++k)
      {
        BRepExtrema_DistShapeShape dd(BRepBuilderAPI_MakeVertex(pm[k]).Vertex(), e);
        if (nb[k].IsNull() && dd.IsDone() && dd.Value() < tol)
          nb[k] = it.Value();
      }
    }
  }
  if (nb[0].IsNull() || nb[1].IsNull())
    return TopoDS_Shape();
  const bool kept[2] = {!rm.Contains(nb[0]), !rm.Contains(nb[1])};
  if (!kept[0] && !kept[1])
    return TopoDS_Shape();
  Section section = [&, kept, d](double t) -> std::vector<gp_Pnt> {
    gp_Pnt p[2];
    gp_Vec dir[2];
    ends(t, p, dir);
    gp_Vec n[2];
    for (int k = 0; k < 2; ++k)
      n[k] = gp_Vec(normal_at(f, p[k]));
    gp_Pnt c;
    if (!meet(p[0], dir[0], p[1], dir[1], c))
      return {};
    if (kept[0] && kept[1])
    {
      gp_Pnt m;
      if (!meet(p[0].Translated(n[0] * d), dir[0], p[1].Translated(n[1] * d), dir[1], m))
        return {};
      auto foot = [&](const gp_Pnt& q, const gp_Vec& v) {
        const gp_Vec u = v.Normalized();
        return q.Translated(u * gp_Vec(q, m).Dot(u));
      };
      return {c, foot(p[0], dir[0]), m, foot(p[1], dir[1])};
    }
    const int    k = kept[0] ? 0 : 1;
    const gp_Pnt b = p[k].Translated(n[k] * d);
    gp_Pnt       q;
    if (!meet(b, dir[k], p[1 - k], dir[1 - k], q))
      return {};
    return {c, p[k], b, q};
  };
  std::vector<TopoDS_Shape> owner;
  if (kept[0] && kept[1])
    owner = {nb[0], nb[0], nb[1], nb[1]};
  else
  {
    const int k = kept[0] ? 0 : 1;
    owner = {nb[k], nb[k], nb[k], nb[1 - k]};
  }
  return sweep_section(spine, section, owner, sources);
}

// The wall round a convex sphere corner (a vertex blend where three
// filleted walls meet, live SHELL-FILLETED-BOX) whose inward offset
// collapses: as at a sharp corner, the region between each wall's plane and
// its offset (a removed wall: its plane only, the region reaching past the
// sphere), the convex polyhedron of those half-spaces. The common with the
// solid trims it.
TopoDS_Shape collapsed_corner_filler(const TopoDS_Face& f, const TopoDS_Shape& solid, const ShapeIMap& rm, double d,
                                     SourceMap& sources)
{
  BRepAdaptor_Surface s(f);
  if (s.GetType() != GeomAbs_Sphere)
    return TopoDS_Shape();
  const gp_Pnt        S = s.Sphere().Location();
  const double        R = s.Sphere().Radius();
  std::vector<gp_Dir> ns;
  for (TopExp_Explorer x(f, TopAbs_VERTEX); x.More(); x.Next())
  {
    const gp_Vec v(S, BRep_Tool::Pnt(TopoDS::Vertex(x.Current())));
    if (v.Magnitude() < R * 0.5)
      continue;
    const gp_Dir n(v);
    bool         dup = false;
    for (const gp_Dir& m : ns)
      dup = dup || m.IsEqual(n, 1e-7);
    if (!dup)
      ns.push_back(n);
  }
  if (ns.size() != 3)
    return TopoDS_Shape();
  const double                           tol = 1e-6 * (R + std::abs(d));
  std::vector<std::pair<gp_Pnt, gp_Vec>> hs;
  struct Inner
  {
    gp_Pnt       o;
    gp_Dir       n;
    TopoDS_Shape wall;
  };
  std::vector<Inner> inner; // offset planes and their walls
  for (const gp_Dir& n : ns)
  {
    // The wall: a planar face of the solid in the plane through S + R n.
    const gp_Pnt o = S.Translated(gp_Vec(n) * R);
    TopoDS_Face  wall;
    for (TopExp_Explorer x(solid, TopAbs_FACE); x.More() && wall.IsNull(); x.Next())
    {
      const TopoDS_Face& g = TopoDS::Face(x.Current());
      BRepAdaptor_Surface gs(g);
      if (gs.GetType() != GeomAbs_Plane)
        continue;
      const gp_Pln pl = gs.Plane();
      gp_Dir       gn = pl.Axis().Direction();
      if (g.Orientation() == TopAbs_REVERSED)
        gn.Reverse();
      if (gn.IsEqual(n, 1e-7) && pl.Distance(o) < tol)
        wall = g;
    }
    if (wall.IsNull())
      return TopoDS_Shape();
    hs.push_back({o, gp_Vec(n)});
    if (rm.Contains(wall))
      hs.push_back({o.Translated(gp_Vec(n) * -(std::abs(d) + R)), -gp_Vec(n)});
    else
    {
      const gp_Pnt oi = o.Translated(gp_Vec(n) * d);
      hs.push_back({oi, -gp_Vec(n)});
      inner.push_back({oi, n, wall});
    }
  }
  TopoDS_Shape poly = convex_polyhedron(hs, std::abs(d) + R);
  if (poly.IsNull())
    return TopoDS_Shape();
  for (TopExp_Explorer e(poly, TopAbs_FACE); e.More(); e.Next())
  {
    GProp_GProps g;
    BRepGProp::SurfaceProperties(e.Current(), g);
    for (const Inner& in : inner)
      if (std::abs(gp_Vec(in.o, g.CentreOfMass()).Dot(gp_Vec(in.n))) < tol)
        sources.Bind(e.Current(), Source{in.wall, (uint8_t)REL_GENERATED});
  }
  return poly;
}

// Path 2. `sign` is -1 for an inward wall, +1 outward.
// ------------------------------------------------------------------ planes

// Path 2a (SHELL-SLOW, core 11-01): inward walls of solids bounded by planes only, the slabs' wall built directly.
// Every kept face moves in by t, every removed face stays; at each vertex (three faces) the three planes meet in the
// cavity's corner, each edge's offset joins two corners, parallel to the edge. The wall: the kept faces, each one's
// offset (its wires' edges replaced by their offsets) and, on each removed face, the rim between its wires and their
// offsets: one solid with shared edges, no Boolean. Only where that is the slabs' wall too: no offset edge vanishes
// or turns round, and no new face crosses or touches another one away from the edges they share (thin walls,
// pockets narrower than 2t: the slabs then). Not: curved faces, vertices of more than three faces, two removed faces
// meeting.
namespace planes {
// A planar polygon of the wall (loops of points, each point a vertex id), for the crossing test.
struct Poly
{
  gp_XYZ                        n;      // unit normal
  double                        c = 0;  // n.x = c
  std::vector<std::vector<int>> loops;  // vertex ids
  bool                          fresh;  // an offset or rim face (old faces are tested against these only)
  Bnd_Box                       box;
};

// The intervals of line {o + s d} (in poly's plane) inside poly: crossings of its loops, even-odd.
std::vector<std::pair<double, double>> chords(const Poly& p, const std::vector<gp_XYZ>& pts, const gp_XYZ& o, const gp_XYZ& d)
{
  const gp_XYZ        m = p.n.Crossed(d); // in the plane, across the line
  std::vector<double> s;
  for (const auto& lp : p.loops)
    for (size_t i = 0; i < lp.size(); ++i)
    {
      const gp_XYZ& a  = pts[(size_t)lp[i]];
      const gp_XYZ& b  = pts[(size_t)lp[(i + 1) % lp.size()]];
      const double  sa = (a - o).Dot(m), sb = (b - o).Dot(m);
      if ((sa > 0) == (sb > 0))
        continue;
      const gp_XYZ x = a + (b - a) * (sa / (sa - sb));
      s.push_back((x - o).Dot(d));
    }
  std::sort(s.begin(), s.end());
  std::vector<std::pair<double, double>> out;
  for (size_t i = 0; i + 1 < s.size(); i += 2)
    out.push_back({s[i], s[i + 1]});
  return out;
}

// Whether polys a and b meet anywhere but along the edges they share (`shared`: their common edges as vertex-id
// pairs); touching within `tol` counts.
bool cross(const Poly& a, const Poly& b, const std::vector<gp_XYZ>& pts, const std::vector<std::pair<int, int>>& shared, double tol)
{
  gp_XYZ d = a.n.Crossed(b.n);
  const double sn = d.Modulus();
  if (sn < 1e-9)
    return std::abs(a.c - b.c * (a.n.Dot(b.n) > 0 ? 1.0 : -1.0)) <= tol; // parallel: coplanar ones are taken to meet
  d /= sn;
  // A point on both planes: o = alpha a.n + beta b.n.
  const double k = a.n.Dot(b.n), det = 1 - k * k;
  const gp_XYZ o = a.n * ((a.c - k * b.c) / det) + b.n * ((b.c - k * a.c) / det);
  const auto   ia = chords(a, pts, o, d), ib = chords(b, pts, o, d);
  // The shared edges' spans along the line, not counted.
  std::vector<std::pair<double, double>> skip;
  for (const auto& [u, v] : shared)
  {
    double s0 = (pts[(size_t)u] - o).Dot(d), s1 = (pts[(size_t)v] - o).Dot(d);
    if (s0 > s1)
      std::swap(s0, s1);
    skip.push_back({s0 - tol, s1 + tol});
  }
  for (const auto& [a0, a1] : ia)
    for (const auto& [b0, b1] : ib)
    {
      const double lo = std::max(a0, b0), hi = std::min(a1, b1);
      if (hi - lo < -tol)
        continue;
      // [lo, hi] less the shared spans: anything left meets.
      std::vector<std::pair<double, double>> left{{lo - tol, hi + tol}};
      for (const auto& [s0, s1] : skip)
      {
        std::vector<std::pair<double, double>> next;
        for (const auto& [x0, x1] : left)
        {
          if (s1 <= x0 || s0 >= x1)
          {
            next.push_back({x0, x1});
            continue;
          }
          if (s0 > x0)
            next.push_back({x0, s0});
          if (s1 < x1)
            next.push_back({s1, x1});
        }
        left.swap(next);
      }
      for (const auto& [x0, x1] : left)
        if (x1 - x0 > 2 * tol)
          return true;
    }
  return false;
}
} // namespace planes

namespace planes {
// Whether two segments of a loop cross (in the plane, dropping n's largest axis), adjacent ones aside.
bool self_crossing(const Poly& p, const std::vector<gp_XYZ>& pts, double tol)
{
  int drop = 0;
  for (int k = 1; k < 3; ++k)
    if (std::abs(p.n.Coord(k + 1)) > std::abs(p.n.Coord(drop + 1)))
      drop = k;
  auto xy = [&](int id) {
    const gp_XYZ& q = pts[(size_t)id];
    return drop == 0 ? gp_XY(q.Y(), q.Z()) : drop == 1 ? gp_XY(q.X(), q.Z()) : gp_XY(q.X(), q.Y());
  };
  std::vector<std::pair<int, int>> segs;
  for (const auto& lp : p.loops)
    for (size_t i = 0; i < lp.size(); ++i)
      segs.push_back({lp[i], lp[(i + 1) % lp.size()]});
  auto orient = [](const gp_XY& a, const gp_XY& b, const gp_XY& c) { return (b - a).Crossed(c - a); };
  for (size_t i = 0; i < segs.size(); ++i)
    for (size_t j = i + 1; j < segs.size(); ++j)
    {
      const auto [a, b] = segs[i];
      const auto [c, d] = segs[j];
      if (a == c || a == d || b == c || b == d)
        continue;
      const gp_XY A = xy(a), B = xy(b), C = xy(c), D = xy(d);
      const double ab = (B - A).Modulus(), cd = (D - C).Modulus();
      const double o1 = orient(A, B, C) / ab, o2 = orient(A, B, D) / ab, o3 = orient(C, D, A) / cd, o4 = orient(C, D, B) / cd;
      // Crossing or touching within tol.
      if (std::min(o1, o2) <= tol && std::max(o1, o2) >= -tol && std::min(o3, o4) <= tol && std::max(o3, o4) >= -tol)
      {
        // (Collinear and apart along the line: no.)
        const gp_XY u = (B - A) / ab;
        const double s0 = (C - A).Dot(u), s1 = (D - A).Dot(u);
        if (std::abs(o1) <= tol && std::abs(o2) <= tol && (std::max(s0, s1) < -tol || std::min(s0, s1) > ab + tol))
          continue;
        return true;
      }
    }
  return false;
}
} // namespace planes

bool by_planes_build(const TopoDS_Shape& so, const ShapeIMap& faces, const ShapeIMap& edges, const ShapeIMap& verts,
                     const std::vector<std::array<int, 2>>& efi, const std::vector<char>& kept, const std::vector<gp_XYZ>& fn,
                     const std::vector<double>& fc, const std::vector<gp_XYZ>& pts, int nv, double t, double tol,
                     const std::vector<TopoDS_Shape>& removed, const TopoDS_Shape& solid, Built& out, bool dbg)
{
  auto no = [&](const char* why) {
    if (dbg)
      std::fprintf(stderr, "planes: %s\n", why);
    return false;
  };
  const int nf = faces.Extent(), ne = edges.Extent();
  // The wall's polygons: kept faces (old), their offsets and the rims (fresh).
  std::vector<planes::Poly> polys;
  auto loops_of = [&](const TopoDS_Face& f, int shift) {
    std::vector<std::vector<int>> out_loops;
    const TopoDS_Face             ff = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    for (TopExp_Explorer w(ff, TopAbs_WIRE); w.More(); w.Next())
    {
      std::vector<int> lp;
      for (BRepTools_WireExplorer x(TopoDS::Wire(w.Current()), ff); x.More(); x.Next())
        lp.push_back(shift + verts.FindIndex(x.CurrentVertex()));
      out_loops.push_back(lp);
    }
    return out_loops;
  };
  auto add_poly = [&](const gp_XYZ& n, double c, std::vector<std::vector<int>> lps, bool fresh) {
    planes::Poly p;
    p.n     = n;
    p.c     = c;
    p.loops = std::move(lps);
    p.fresh = fresh;
    for (const auto& lp : p.loops)
      for (int id : lp)
        p.box.Add(gp_Pnt(pts[(size_t)id]));
    p.box.Enlarge(10 * tol);
    polys.push_back(std::move(p));
  };
  for (int i = 1; i <= nf; ++i)
  {
    const TopoDS_Face& f = TopoDS::Face(faces(i));
    const auto         lps = loops_of(f, 0);
    if (kept[(size_t)i])
    {
      add_poly(fn[(size_t)i], fc[(size_t)i], lps, false);
      add_poly(fn[(size_t)i], fc[(size_t)i] - t, loops_of(f, nv), true);
    }
    else
      for (const auto& lp : lps)
      {
        std::vector<int> off;
        for (int id : lp)
          off.push_back(id + nv);
        add_poly(fn[(size_t)i], fc[(size_t)i], {lp, off}, true);
      }
  }
  for (const planes::Poly& p : polys)
    if (p.fresh && planes::self_crossing(p, pts, tol))
      return no("an offset face crosses itself");
  auto edge_keys = [](const planes::Poly& p) {
    std::set<std::pair<int, int>> s;
    for (const auto& lp : p.loops)
      for (size_t i = 0; i < lp.size(); ++i)
        s.insert(std::minmax(lp[i], lp[(i + 1) % lp.size()]));
    return s;
  };
  std::vector<std::set<std::pair<int, int>>> keys;
  for (const planes::Poly& p : polys)
    keys.push_back(edge_keys(p));
  for (size_t i = 0; i < polys.size(); ++i)
    for (size_t j = i + 1; j < polys.size(); ++j)
    {
      if ((!polys[i].fresh && !polys[j].fresh) || polys[i].box.IsOut(polys[j].box))
        continue;
      std::vector<std::pair<int, int>> shared;
      for (const auto& k : keys[i])
        if (keys[j].count(k) != 0)
          shared.push_back(k);
      if (planes::cross(polys[i], polys[j], pts, shared, 10 * tol))
        return no("an offset face meets another face");
    }
  // The wall: the solid's shell with each removed face replaced by its rims and each kept face's offset added.
  BRep_Builder               bld;
  std::vector<TopoDS_Vertex> vx(nv + 1);
  for (int i = 1; i <= nv; ++i)
    bld.MakeVertex(vx[(size_t)i], gp_Pnt(pts[(size_t)(nv + i)]), Precision::Confusion());
  std::vector<TopoDS_Edge> ex(ne + 1);
  for (int i = 1; i <= ne; ++i)
  {
    TopoDS_Vertex a, b;
    TopExp::Vertices(TopoDS::Edge(edges(i)), a, b);
    const TopoDS_Vertex& va = vx[(size_t)verts.FindIndex(a)];
    const TopoDS_Vertex& vb = vx[(size_t)verts.FindIndex(b)];
    BRepBuilderAPI_MakeEdge mk(va, vb);
    if (!mk.IsDone())
      return no("an offset edge failed");
    ex[(size_t)i] = mk.Edge();
  }
  // A wire's offset: its edges' offsets, each the same way round in it.
  auto offset_wire = [&](const TopoDS_Shape& w) {
    TopoDS_Wire ow;
    bld.MakeWire(ow);
    for (TopoDS_Iterator it(w, false); it.More(); it.Next())
      bld.Add(ow, ex[(size_t)edges.FindIndex(it.Value())].Oriented(it.Value().Orientation()));
    ow.Closed(true);
    return ow;
  };
  SourceMap   sources;
  HHist       h = new BRepTools_History();
  TopoDS_Shell shell;
  bld.MakeShell(shell);
  TopoDS_Shape old_shell;
  for (TopExp_Explorer x(so, TopAbs_SHELL); x.More(); x.Next())
    old_shell = x.Current();
  for (TopoDS_Iterator it(old_shell, false); it.More(); it.Next())
  {
    const TopoDS_Face& f   = TopoDS::Face(it.Value());
    const int          i   = faces.FindIndex(f);
    const TopoDS_Face  ff  = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    TopLoc_Location    loc;
    const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(ff, loc);
    if (kept[(size_t)i])
    {
      bld.Add(shell, f);
      // The offset: the face's twin moved in by t, facing the other way in the wall.
      const gp_XYZ                    mv = fn[(size_t)i] * -t;
      occ::handle<Geom_Surface>       moved = occ::down_cast<Geom_Surface>(surf->Translated(gp_Vec(mv).Transformed(loc.Transformation().Inverted())));
      TopoDS_Face                     of;
      bld.MakeFace(of, moved, loc, BRep_Tool::Tolerance(ff));
      for (TopoDS_Iterator w(ff, false); w.More(); w.Next())
        bld.Add(of, offset_wire(w.Value()).Oriented(w.Value().Orientation()));
      const TopoDS_Face inner = TopoDS::Face(of.Oriented(TopAbs::Reverse(f.Orientation())));
      bld.Add(shell, inner);
      sources.Bind(inner, Source{f, (uint8_t)REL_GENERATED});
    }
    else
      // Each wire's rim: the wire and its offset (the other way round), on the removed face's surface.
      for (TopoDS_Iterator w(ff, false); w.More(); w.Next())
      {
        TopoDS_Face rim;
        bld.MakeFace(rim, surf, loc, BRep_Tool::Tolerance(ff));
        bld.Add(rim, w.Value());
        bld.Add(rim, offset_wire(w.Value()).Oriented(TopAbs::Reverse(w.Value().Orientation())));
        const TopoDS_Face r = TopoDS::Face(rim.Oriented(f.Orientation()));
        bld.Add(shell, r);
        h->AddModified(f, r);
      }
  }
  shell.Closed(true);
  TopoDS_Solid wall;
  bld.MakeSolid(wall);
  bld.Add(wall, shell.Oriented(old_shell.Orientation()));
  if (!BRepCheck_Analyzer(wall).IsValid())
    return no("the wall is invalid");
  const double vw = volume_of(wall), vs = volume_of(so);
  if (dbg)
    std::fprintf(stderr, "planes: wall volume %.17g solid %.17g\n", vw, vs);
  if (!(vw > 0) || !(vw <= vs * (1 + 1e-9)))
    return no("the wall's volume is off");
  out.result  = wall;
  out.history = shell_history(solid, wall, {h}, {}, sources, removed);
  return true;
}

// NOSHAPE_SHELL_COMPARE: a wall by planes against the slabs' (volume, counts, each input face's images by kind,
// area and centre), printed.
void compare_with_slabs(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t, double sign, const Built& mine);

bool by_planes(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t, double sign, Built& out)
{
  const bool dbg = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       no  = [&](const char* why) {
    if (dbg)
      std::fprintf(stderr, "planes: %s\n", why);
    return false;
  };
  if (sign > 0)
    return no("outward");
  TopoDS_Shape so;
  int          nso = 0, nsh = 0;
  for (TopExp_Explorer x(solid, TopAbs_SOLID); x.More(); x.Next(), ++nso)
    so = x.Current();
  if (nso != 1)
    return no("not one solid");
  for (TopExp_Explorer x(so, TopAbs_SHELL); x.More(); x.Next())
    ++nsh;
  if (nsh != 1)
    return no("not one shell");
  ShapeIMap faces, edges, verts, rm;
  TopExp::MapShapes(so, TopAbs_FACE, faces);
  TopExp::MapShapes(so, TopAbs_EDGE, edges);
  TopExp::MapShapes(so, TopAbs_VERTEX, verts);
  for (const TopoDS_Shape& f : removed)
    rm.Add(f);
  Bnd_Box bb;
  BRepBndLib::Add(so, bb, false);
  const double size = std::sqrt(bb.SquareExtent());
  const double tol  = 1e-9 * size + 1e-12;
  // Each face's plane n.x = c (n outward), and the offset one n.x = c - t for a kept face.
  const int           nf = faces.Extent();
  std::vector<gp_XYZ> fn(nf + 1);
  std::vector<double> fc(nf + 1), oc(nf + 1);
  std::vector<char>   kept(nf + 1);
  for (int i = 1; i <= nf; ++i)
  {
    const TopoDS_Face&  f = TopoDS::Face(faces(i));
    BRepAdaptor_Surface s(f);
    if (s.GetType() != GeomAbs_Plane)
      return no("a face isn't planar");
    const gp_Ax3 ax = s.Plane().Position();
    gp_XYZ       n  = ax.XDirection().XYZ().Crossed(ax.YDirection().XYZ());
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    fn[i]   = n;
    fc[i]   = n.Dot(ax.Location().XYZ());
    kept[i] = !rm.Contains(f);
    oc[i]   = kept[i] ? fc[i] - t : fc[i];
  }
  ShapeAncMap ef, vf;
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_EDGE, TopAbs_FACE, ef);
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_VERTEX, TopAbs_FACE, vf);
  ShapeAncMap ve;
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_VERTEX, TopAbs_EDGE, ve);
  // Each edge's two faces.
  const int                        ne = edges.Extent();
  std::vector<std::array<int, 2>>  efi(ne + 1);
  for (int i = 1; i <= ne; ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(edges(i));
    if (BRep_Tool::Degenerated(e))
      return no("a degenerate edge");
    const ShapeList& fl = ef.FindFromKey(e);
    if (fl.Extent() != 2)
      return no("an edge not between two faces");
    const int a = faces.FindIndex(fl.First()), b = faces.FindIndex(fl.Last());
    if (a == b || BRep_Tool::IsClosed(e, TopoDS::Face(fl.First())))
      return no("a seam");
    if (!kept[a] && !kept[b])
      return no("two removed faces meet");
    if (std::abs(fn[a].Dot(fn[b])) > 1 - 1e-9)
      return no("two faces of one plane meet");
    if (BRepAdaptor_Curve(e).GetType() != GeomAbs_Line)
      return no("an edge not straight");
    efi[i] = {a, b};
  }
  // Each vertex: three faces, three edges; the cavity's corner where their planes meet.
  const int           nv = verts.Extent();
  std::vector<gp_XYZ> pts(2 * nv + 1); // 1..nv: the solid's vertices, nv + 1..2 nv: their offsets
  for (int i = 1; i <= nv; ++i)
  {
    const TopoDS_Vertex& v  = TopoDS::Vertex(verts(i));
    const ShapeList&     fl = vf.FindFromKey(v);
    if (fl.Extent() != 3 || ve.FindFromKey(v).Extent() != 3)
      return no("a vertex not of three faces");
    int k[3], j = 0;
    for (ShapeList::Iterator it(fl); it.More(); it.Next())
      k[j++] = faces.FindIndex(it.Value());
    gp_Mat m(fn[k[0]], fn[k[1]], fn[k[2]]); // columns; rows wanted
    m.Transpose();
    const double det = m.Determinant();
    if (std::abs(det) < 1e-9)
      return no("a corner's planes nearly meet in a line");
    pts[(size_t)i]      = BRep_Tool::Pnt(v).XYZ();
    pts[(size_t)(nv + i)] = m.Inverted() * gp_XYZ(oc[k[0]], oc[k[1]], oc[k[2]]);
  }
  // Every edge's offset as long as it is and the same way round (within the part's size).
  for (int i = 1; i <= ne; ++i)
  {
    TopoDS_Vertex a, b;
    TopExp::Vertices(TopoDS::Edge(edges(i)), a, b);
    const int    ia = verts.FindIndex(a), ib = verts.FindIndex(b);
    const gp_XYZ D  = pts[(size_t)ib] - pts[(size_t)ia], d = pts[(size_t)(nv + ib)] - pts[(size_t)(nv + ia)];
    const double L2 = D.SquareModulus();
    if (L2 <= tol * tol || d.Dot(D) / L2 < 1e-6)
      return no("an offset edge vanishes or turns round");
  }
  return by_planes_build(so, faces, edges, verts, efi, kept, fn, fc, pts, nv, t, tol, removed, solid, out, dbg);
}

// ------------------------------------------------------------------ quadrics

// Path 2b (SHELL-SLOW step 2, core 11-01): by_planes for solids of planes, cylinders and cones (the removed faces
// planes). Each kept face's offset is its own kind (a cylinder's radius, a cone's reference radius moved by t); each
// vertex's offset where its faces' offsets meet (Newton; two tangent faces: along their offset tangent line; a seam's
// vertex: along the offset seam); each edge's offset the two offsets' intersection through its ends (GeomAPI_IntSS,
// exact for these), tangent faces' edges moved along their normal. The wall as by_planes builds it, then checked:
// valid, free of self-interference (BOPAlgo_ArgumentAnalyzer: the offsets crossing each other or the part), its
// volume inside the part's.
namespace quad {
struct Surf
{
  GeomAbs_SurfaceType       kind;
  occ::handle<Geom_Surface> off;    // the offset (world coordinates)
  gp_Ax3                    ax;     // the offset's frame (cylinders, cones)
  double                    r = 0;  // the offset's radius (cylinder) / reference radius (cone)
  double                    ang = 0;
  gp_XYZ                    n;      // planes: outward normal, n.x = c
  double                    c = 0;
};

// Signed distance-like value of x from s's offset (zero on it).
double eval(const Surf& s, const gp_XYZ& x)
{
  if (s.kind == GeomAbs_Plane)
    return s.n.Dot(x) - s.c;
  const gp_XYZ w   = x - s.ax.Location().XYZ();
  const double h   = w.Dot(s.ax.Direction().XYZ());
  const double rho = (w - s.ax.Direction().XYZ() * h).Modulus();
  if (s.kind == GeomAbs_Cylinder)
    return rho - s.r;
  return (rho - (s.r + h * std::tan(s.ang))) * std::cos(s.ang);
}

// Newton on `fs` (as many unknowns as functions: 3 for a point, 1 along a line).
bool solve3(const Surf* s[3], gp_XYZ& x, double scale)
{
  for (int it = 0; it < 60; ++it)
  {
    const gp_XYZ F(eval(*s[0], x), eval(*s[1], x), eval(*s[2], x));
    if (std::max({std::abs(F.X()), std::abs(F.Y()), std::abs(F.Z())}) < 1e-13 * scale)
      return true;
    gp_Mat       J;
    const double h = 1e-7 * scale;
    for (int c = 1; c <= 3; ++c)
    {
      gp_XYZ a = x, b = x;
      a.SetCoord(c, a.Coord(c) + h);
      b.SetCoord(c, b.Coord(c) - h);
      J.SetCol(c, (gp_XYZ(eval(*s[0], a), eval(*s[1], a), eval(*s[2], a)) - gp_XYZ(eval(*s[0], b), eval(*s[1], b), eval(*s[2], b))) / (2 * h));
    }
    if (std::abs(J.Determinant()) < 1e-12)
      return false;
    x -= J.Inverted() * F;
  }
  return false;
}

bool solve_line(const Surf& s, const gp_XYZ& o, const gp_XYZ& d, gp_XYZ& x, double scale)
{
  double l = 0;
  for (int it = 0; it < 60; ++it)
  {
    const double f = eval(s, o + d * l);
    if (std::abs(f) < 1e-13 * scale)
    {
      x = o + d * l;
      return true;
    }
    const double h = 1e-7 * scale, df = (eval(s, o + d * (l + h)) - eval(s, o + d * (l - h))) / (2 * h);
    if (std::abs(df) < 1e-12)
      return false;
    l -= f / df;
  }
  return false;
}
} // namespace quad

namespace quad {
// Edge `ne`'s curve on face `nf` (an offset of `of`, its surface the offset of `of`'s in the same frame): projected,
// then moved by whole turns in u to start where `oe`'s curve on `of` starts (a seam: both of its curves, in `oe`'s
// order).
bool add_pcurve(const TopoDS_Edge& ne, const TopoDS_Face& nf, const TopoDS_Edge& oe, const TopoDS_Face& of, bool seam_edge, double tol)
{
  double                          a, b;
  const occ::handle<Geom_Curve>   c3 = BRep_Tool::Curve(ne, a, b);
  const occ::handle<Geom_Surface> sn = BRep_Tool::Surface(nf);
  const bool                      per = sn->IsUPeriodic();
  auto one = [&](TopAbs_Orientation o) -> occ::handle<Geom2d_Curve> {
    double                           f0, f1;
    const occ::handle<Geom2d_Curve>  co = BRep_Tool::CurveOnSurface(TopoDS::Edge(oe.Oriented(o)), of, f0, f1);
    occ::handle<Geom2d_Curve>        cn = GeomProjLib::Curve2d(c3, a, b, sn);
    if (co.IsNull() || cn.IsNull())
      return nullptr;
    if (per)
    {
      // Original start (in oe's own direction) against the new curve's start.
      const double uo = co->Value(o == TopAbs_REVERSED ? f0 : f0).X();
      const double un = cn->Value(a).X();
      const double P  = sn->UPeriod();
      const double du = std::round((uo - un) / P) * P;
      if (du != 0)
        cn->Translate(gp_Vec2d(du, 0));
    }
    return cn;
  };
  BRep_Builder bld;
  if (seam_edge)
  {
    // The seam's two curves: oe FORWARD's and REVERSED's on `of`, each placed at its own u.
    const occ::handle<Geom2d_Curve> c1 = one(TopAbs_FORWARD), c2 = one(TopAbs_REVERSED);
    if (c1.IsNull() || c2.IsNull())
      return false;
    bld.UpdateEdge(ne, c1, c2, nf, tol);
  }
  else
  {
    const occ::handle<Geom2d_Curve> c1 = one(TopAbs_FORWARD);
    if (c1.IsNull())
      return false;
    bld.UpdateEdge(ne, c1, nf, tol);
  }
  bld.Range(ne, nf, a, b);
  return true;
}
} // namespace quad

bool by_quadrics(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t, double sign, Built& out)
{
  const bool dbg = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       no  = [&](const char* why) {
    if (dbg)
      std::fprintf(stderr, "quadrics: %s\n", why);
    return false;
  };
  if (sign > 0)
    return no("outward");
  TopoDS_Shape so, old_shell;
  int          nso = 0, nsh = 0;
  for (TopExp_Explorer x(solid, TopAbs_SOLID); x.More(); x.Next(), ++nso)
    so = x.Current();
  if (nso != 1)
    return no("not one solid");
  for (TopExp_Explorer x(so, TopAbs_SHELL); x.More(); x.Next(), ++nsh)
    old_shell = x.Current();
  if (nsh != 1)
    return no("not one shell");
  ShapeIMap faces, edges, verts, rm;
  TopExp::MapShapes(so, TopAbs_FACE, faces);
  TopExp::MapShapes(so, TopAbs_EDGE, edges);
  TopExp::MapShapes(so, TopAbs_VERTEX, verts);
  for (const TopoDS_Shape& f : removed)
    rm.Add(f);
  Bnd_Box bb;
  BRepBndLib::Add(so, bb, false);
  const double size = std::sqrt(bb.SquareExtent());
  const double tol  = 1e-9 * size + 1e-12;
  const int    nf = faces.Extent(), ne = edges.Extent(), nv = verts.Extent();
  // The faces' offsets.
  std::vector<quad::Surf> sf(nf + 1);
  std::vector<char>       kept(nf + 1), collapse(nf + 1);
  bool                    curved = false;
  for (int i = 1; i <= nf; ++i)
  {
    const TopoDS_Face&  f = TopoDS::Face(faces(i));
    BRepAdaptor_Surface s(f);
    kept[(size_t)i] = !rm.Contains(f);
    const double d  = kept[(size_t)i] ? (f.Orientation() == TopAbs_REVERSED ? t : -t) : 0.0; // along the natural normal
    quad::Surf&  q  = sf[(size_t)i];
    q.kind          = s.GetType();
    if (q.kind == GeomAbs_Plane)
    {
      const gp_Ax3 ax = s.Plane().Position();
      gp_XYZ       n  = ax.XDirection().XYZ().Crossed(ax.YDirection().XYZ());
      q.off           = new Geom_Plane(gp_Pln(ax).Translated(gp_Vec(n * d)));
      if (f.Orientation() == TopAbs_REVERSED)
        n.Reverse();
      q.n = n;
      q.c = n.Dot(ax.Location().XYZ()) - (kept[(size_t)i] ? t : 0.0);
      continue;
    }
    if (!kept[(size_t)i])
      return no("a removed face isn't planar");
    curved = true;
    if (q.kind != GeomAbs_Cylinder && q.kind != GeomAbs_Cone)
      return no("a face neither a plane, a cylinder nor a cone");
    // The point at (0, 0) moved along the natural normal; the offset's reference radius where its generator crosses
    // the frame's plane.
    gp_Pnt P;
    gp_Vec Du, Dv;
    s.D1(0, 0, P, Du, Dv);
    const gp_Vec N = Du.Crossed(Dv);
    if (N.Magnitude() < 1e-12)
      return no("a face's normal undefined at its frame");
    q.ax            = q.kind == GeomAbs_Cylinder ? s.Cylinder().Position() : s.Cone().Position();
    q.ang           = q.kind == GeomAbs_Cone ? s.Cone().SemiAngle() : 0.0;
    const gp_XYZ O  = q.ax.Location().XYZ(), Z = q.ax.Direction().XYZ(), X = q.ax.XDirection().XYZ();
    const gp_XYZ P1 = P.XYZ() + N.Normalized().XYZ() * d;
    const gp_XYZ G  = Dv.Normalized().XYZ();
    const double gz = G.Dot(Z);
    if (std::abs(gz) < 1e-9)
      return no("a cone's generator square to its axis");
    const gp_XYZ at0 = P1 - G * ((P1 - O).Dot(Z) / gz);
    q.r              = (at0 - O).Dot(X);
    if (q.r <= 10 * tol)
    {
      // A fillet no thicker than the wall: its offset vanishes, its neighbours' offsets meet (below).
      if (q.kind != GeomAbs_Cylinder)
        return no("a cone's offset collapses");
      collapse[(size_t)i] = 1;
      continue;
    }
    if (q.kind == GeomAbs_Cylinder)
      q.off = new Geom_CylindricalSurface(q.ax, q.r);
    else
      q.off = new Geom_ConicalSurface(q.ax, q.ang, q.r);
  }
  if (!curved)
    return no("planes only");
  ShapeAncMap ef, vf, ve;
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_EDGE, TopAbs_FACE, ef);
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_VERTEX, TopAbs_FACE, vf);
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_VERTEX, TopAbs_EDGE, ve);
  // Edges: their faces (a seam: the same twice), whether tangent there.
  std::vector<std::array<int, 2>> efi(ne + 1);
  std::vector<char>               seam(ne + 1), tangent(ne + 1);
  for (int i = 1; i <= ne; ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(edges(i));
    if (BRep_Tool::Degenerated(e))
      return no("a degenerate edge");
    const ShapeList& fl = ef.FindFromKey(e);
    if (fl.Extent() == 1 && BRep_Tool::IsClosed(e, TopoDS::Face(fl.First())))
    {
      const int a     = faces.FindIndex(fl.First());
      efi[(size_t)i]  = {a, a};
      seam[(size_t)i] = 1;
      if (BRepAdaptor_Curve(e).GetType() != GeomAbs_Line || !kept[(size_t)a])
        return no("a seam not a kept face's generator");
      continue;
    }
    if (fl.Extent() != 2)
      return no("an edge not between two faces");
    const int a = faces.FindIndex(fl.First()), b = faces.FindIndex(fl.Last());
    if (!kept[(size_t)a] && !kept[(size_t)b])
      return no("two removed faces meet");
    efi[(size_t)i] = {a, b};
    BRepAdaptor_Curve c(e);
    const gp_Pnt      m = c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
    const gp_Dir      na = normal_at(TopoDS::Face(faces(a)), m), nb = normal_at(TopoDS::Face(faces(b)), m);
    if (na.Dot(nb) > 1 - 1e-9)
    {
      if (!kept[(size_t)a] || !kept[(size_t)b])
        return no("a kept face tangent to a removed one");
      if (c.GetType() != GeomAbs_Line)
        return no("faces tangent along a curve");
      tangent[(size_t)i] = 1;
    }
  }
  // Collapsing fillets: a strip between two tangent faces a and b and two caps. In the wall's inside the strip is
  // the edge where a's and b's offsets meet, its end arcs points (their two corners one).
  std::vector<std::array<int, 2>> across(nf + 1, {0, 0}); // a collapsing face's two tangent neighbours
  std::vector<int>                group(ne + 1, 0);       // a tangent edge of a collapsing face: that face
  std::vector<char>               gone(ne + 1);           // an end arc of a collapsing face
  std::vector<int>                vrep(nv + 1);
  for (int i = 1; i <= nv; ++i)
    vrep[(size_t)i] = i;
  std::function<int(int)> find = [&](int i) { return vrep[(size_t)i] == i ? i : vrep[(size_t)i] = find(vrep[(size_t)i]); };
  for (int c = 1; c <= nf; ++c)
  {
    if (!collapse[(size_t)c])
      continue;
    std::vector<int> es;
    ShapeIMap        seen;
    for (TopExp_Explorer x(faces(c), TopAbs_EDGE); x.More(); x.Next())
      if (!seen.Contains(x.Current()))
      {
        seen.Add(x.Current());
        es.push_back(edges.FindIndex(x.Current()));
      }
    std::vector<int> tan, ends;
    for (int e : es)
      (tangent[(size_t)e] ? tan : ends).push_back(e);
    if (es.size() != 4 || tan.size() != 2 || ends.size() != 2)
      return no("a collapsing fillet not a strip");
    const int a = efi[(size_t)tan[0]][0] == c ? efi[(size_t)tan[0]][1] : efi[(size_t)tan[0]][0];
    const int b = efi[(size_t)tan[1]][0] == c ? efi[(size_t)tan[1]][1] : efi[(size_t)tan[1]][0];
    if (a == b || collapse[(size_t)a] || collapse[(size_t)b] || !kept[(size_t)a] || !kept[(size_t)b])
      return no("a collapsing fillet's neighbours");
    for (int e : ends)
    {
      const int cap = efi[(size_t)e][0] == c ? efi[(size_t)e][1] : efi[(size_t)e][0];
      if (cap == a || cap == b || collapse[(size_t)cap])
        return no("a collapsing fillet's caps");
      gone[(size_t)e] = 1;
      TopoDS_Vertex v0, v1;
      TopExp::Vertices(TopoDS::Edge(edges(e)), v0, v1);
      const int r0 = find(verts.FindIndex(v0)), r1 = find(verts.FindIndex(v1));
      if (r0 == r1)
        return no("a collapsing fillet's end arc closed");
      vrep[(size_t)r1] = r0;
    }
    across[(size_t)c] = {a, b};
    group[(size_t)tan[0]] = group[(size_t)tan[1]] = c;
  }
  // Vertices: three faces (two tangent: along their offset tangent line), or a closed face's seam and one face.
  std::vector<gp_XYZ> pts(nv + 1);
  for (int i = 1; i <= nv; ++i)
  {
    const TopoDS_Vertex& v  = TopoDS::Vertex(verts(i));
    const gp_Pnt         V  = BRep_Tool::Pnt(v);
    const ShapeList&     fl = vf.FindFromKey(v);
    std::vector<int>     fs;
    for (ShapeList::Iterator it(fl); it.More(); it.Next())
      fs.push_back(faces.FindIndex(it.Value()));
    std::vector<int> es;
    for (ShapeList::Iterator it(ve.FindFromKey(v)); it.More(); it.Next())
      es.push_back(edges.FindIndex(it.Value()));
    auto n_at = [&](int f) { return gp_Vec(normal_at(TopoDS::Face(faces(f)), V)).XYZ(); };
    gp_XYZ x;
    const auto col = std::find_if(fs.begin(), fs.end(), [&](int f) { return collapse[(size_t)f] != 0; });
    if (col != fs.end())
    {
      // A corner of a collapsing fillet: where its neighbours' offsets meet the cap's.
      const int c = *col;
      if (fs.size() != 3 || std::count_if(fs.begin(), fs.end(), [&](int f) { return collapse[(size_t)f] != 0; }) != 1)
        return no("a collapsing fillet's corner not of three faces");
      int cap = 0;
      for (int f : fs)
        if (f != c && f != across[(size_t)c][0] && f != across[(size_t)c][1])
          cap = f;
      const int         a = across[(size_t)c][0], b = across[(size_t)c][1];
      const quad::Surf* s3[3] = {&sf[(size_t)a], &sf[(size_t)b], &sf[(size_t)cap]};
      x = V.XYZ() - (n_at(a) + n_at(b)) * t - n_at(cap) * (kept[(size_t)cap] ? t : 0.0);
      if (cap == 0 || !quad::solve3(s3, x, size))
        return no("a collapsing fillet's corner not found");
    }
    else if (fs.size() == 3 && es.size() == 3)
    {
      int tp = -1;
      for (int e : es)
        if (tangent[(size_t)e])
          tp = tp < 0 ? e : -2;
      if (tp == -2)
        return no("a corner of two tangent edges");
      if (tp > 0)
      {
        const int a = efi[(size_t)tp][0], b = efi[(size_t)tp][1];
        int       third = -1;
        for (int f : fs)
          if (f != a && f != b)
            third = f;
        BRepAdaptor_Curve c(TopoDS::Edge(edges(tp)));
        gp_Pnt            p0;
        gp_Vec            d0;
        c.D1(c.FirstParameter(), p0, d0);
        const gp_XYZ o = V.XYZ() - n_at(a) * t;
        if (third < 0 || !quad::solve_line(sf[(size_t)third], o, d0.Normalized().XYZ(), x, size))
          return no("a tangent line's corner not found");
      }
      else
      {
        const quad::Surf* s3[3] = {&sf[(size_t)fs[0]], &sf[(size_t)fs[1]], &sf[(size_t)fs[2]]};
        x = V.XYZ() - (n_at(fs[0]) * (kept[(size_t)fs[0]] ? t : 0.0) + n_at(fs[1]) * (kept[(size_t)fs[1]] ? t : 0.0) + n_at(fs[2]) * (kept[(size_t)fs[2]] ? t : 0.0));
        if (!quad::solve3(s3, x, size))
          return no("a corner's offsets don't meet");
      }
    }
    else if (fs.size() == 2 && std::any_of(es.begin(), es.end(), [&](int e) { return seam[(size_t)e] != 0; }))
    {
      const int sm = *std::find_if(es.begin(), es.end(), [&](int e) { return seam[(size_t)e] != 0; });
      const int a  = efi[(size_t)sm][0], b = fs[0] == a ? fs[1] : fs[0];
      BRepAdaptor_Curve c(TopoDS::Edge(edges(sm)));
      gp_Pnt            p0;
      gp_Vec            d0;
      c.D1(c.FirstParameter(), p0, d0);
      if (!quad::solve_line(sf[(size_t)b], V.XYZ() - n_at(a) * t, d0.Normalized().XYZ(), x, size))
        return no("a seam's corner not found");
    }
    else
      return no("a vertex not of three faces (or a seam and one)");
    if ((x - V.XYZ()).Modulus() > 10 * t + 1e3 * tol)
      return no("a corner's offset far from it");
    pts[(size_t)i] = x;
  }
  // The edges' offsets.
  BRep_Builder               bld;
  std::vector<TopoDS_Vertex> vx(nv + 1);
  for (int i = 1; i <= nv; ++i)
    if (find(i) == i)
      bld.MakeVertex(vx[(size_t)i], gp_Pnt(pts[(size_t)i]), 1e3 * tol);
  for (int i = 1; i <= nv; ++i)
    if (find(i) != i)
    {
      if (gp_Pnt(pts[(size_t)i]).Distance(gp_Pnt(pts[(size_t)find(i)])) > 1e3 * tol)
        return no("a collapsing fillet's end corners apart");
      vx[(size_t)i] = vx[(size_t)find(i)];
    }
  std::vector<TopoDS_Edge> ex(ne + 1);
  std::vector<char>        flip(ne + 1);
  std::vector<int>         group_edge(nf + 1, 0); // a collapsing face's edge (the first of its tangent edges)
  for (int i = 1; i <= ne; ++i)
  {
    if (gone[(size_t)i])
      continue;
    const TopoDS_Edge e = TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     a, b;
    TopExp::Vertices(e, a, b);
    const int         ia = find(verts.FindIndex(a)), ib = find(verts.FindIndex(b));
    if (const int c = group[(size_t)i]; c != 0 && group_edge[(size_t)c] != 0)
    {
      // The collapsing face's other tangent edge: the same mitre edge, maybe the other way round.
      const int    k  = group_edge[(size_t)c];
      TopoDS_Vertex ka, kb;
      TopExp::Vertices(TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), ka, kb);
      ex[(size_t)i]   = ex[(size_t)k];
      flip[(size_t)i] = find(verts.FindIndex(ka)) != ia;
      continue;
    }
    const gp_Pnt      A = gp_Pnt(pts[(size_t)ia]), B = gp_Pnt(pts[(size_t)ib]);
    BRepAdaptor_Curve oc(e);
    gp_Pnt            o0;
    gp_Vec            t0;
    oc.D1(oc.FirstParameter(), o0, t0);
    occ::handle<Geom_Curve> c;
    double                  p0 = 0, p1 = 0;
    // (A collapsing face's tangent edge: the mitre of its two neighbours' offsets.)
    const int fa = group[(size_t)i] != 0 ? across[(size_t)group[(size_t)i]][0] : efi[(size_t)i][0];
    const int fb = group[(size_t)i] != 0 ? across[(size_t)group[(size_t)i]][1] : efi[(size_t)i][1];
    if (seam[(size_t)i] || (tangent[(size_t)i] && group[(size_t)i] == 0) || (sf[(size_t)fa].kind == GeomAbs_Plane && sf[(size_t)fb].kind == GeomAbs_Plane))
    {
      if (A.Distance(B) <= 10 * tol || gp_Vec(A, B).Dot(t0) <= 0)
        return no("an offset edge vanishes or turns round");
      c  = new Geom_Line(A, gp_Dir(gp_Vec(A, B)));
      p1 = A.Distance(B);
    }
    else
    {
      GeomAPI_IntSS is(sf[(size_t)fa].off, sf[(size_t)fb].off, 1e-9 * size);
      if (!is.IsDone())
        return no("two offsets don't intersect");
      for (int k = 1; k <= is.NbLines() && c.IsNull(); ++k)
      {
        occ::handle<Geom_Curve>     cand = is.Line(k);
        GeomAPI_ProjectPointOnCurve pa(A, cand), pb(B, cand);
        if (pa.NbPoints() == 0 || pb.NbPoints() == 0 || pa.LowerDistance() > 1e3 * tol || pb.LowerDistance() > 1e3 * tol)
          continue;
        double  u0 = pa.LowerDistanceParameter(), u1 = pb.LowerDistanceParameter();
        gp_Pnt  q;
        gp_Vec  d;
        cand->D1(u0, q, d);
        if (d.Dot(t0) < 0)
        {
          cand = cand->Reversed();
          u0   = cand->ReversedParameter(u0);
          u1   = cand->ReversedParameter(u1);
        }
        if (cand->IsPeriodic())
        {
          const double per = cand->Period();
          while (u1 <= u0 + 1e-12)
            u1 += per;
          if (!(a.IsSame(b)) && u1 - u0 >= per - 1e-9)
            u1 -= per;
        }
        if (!(u1 > u0))
          continue;
        c  = cand;
        p0 = u0;
        p1 = u1;
      }
      if (c.IsNull())
        return no("no intersection of two offsets through an edge's ends");
    }
    BRepBuilderAPI_MakeEdge mk(c, vx[(size_t)ia], vx[(size_t)ib], p0, p1);
    if (!mk.IsDone())
      return no("an offset edge failed");
    ex[(size_t)i] = mk.Edge();
    if (group[(size_t)i] != 0)
      group_edge[(size_t)group[(size_t)i]] = i;
  }
  bool ok_wires = true;
  auto offset_wire = [&](const TopoDS_Shape& w) {
    TopoDS_Wire ow;
    bld.MakeWire(ow);
    for (TopoDS_Iterator it(w, false); it.More(); it.Next())
    {
      const int k = edges.FindIndex(it.Value());
      if (gone[(size_t)k])
        continue;
      if (ex[(size_t)k].IsNull())
      {
        if (dbg)
        {
          BRepAdaptor_Curve oc(TopoDS::Edge(it.Value()));
          const gp_Pnt      m = oc.Value(0.5 * (oc.FirstParameter() + oc.LastParameter()));
          std::fprintf(stderr, "quadrics: no offset for edge %d (faces %d %d, tangent %d, group %d) at %g %g %g\n", k, efi[(size_t)k][0], efi[(size_t)k][1], (int)tangent[(size_t)k], group[(size_t)k], m.X(), m.Y(), m.Z());
        }
        ok_wires = false;
        continue;
      }
      bld.Add(ow, ex[(size_t)k].Oriented(flip[(size_t)k] ? TopAbs::Reverse(it.Value().Orientation()) : it.Value().Orientation()));
    }
    ow.Closed(true);
    return ow;
  };
  SourceMap    sources;
  HHist        h = new BRepTools_History();
  TopoDS_Shell shell;
  bld.MakeShell(shell);
  for (TopoDS_Iterator it(old_shell, false); it.More(); it.Next())
  {
    const TopoDS_Face& f  = TopoDS::Face(it.Value());
    const int          i  = faces.FindIndex(f);
    const TopoDS_Face  ff = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    if (collapse[(size_t)i])
      bld.Add(shell, f); // (no offset: the neighbours' offsets meet where it was)
    else if (kept[(size_t)i])
    {
      bld.Add(shell, f);
      TopoDS_Face of;
      bld.MakeFace(of, sf[(size_t)i].off, 1e3 * tol);
      for (TopoDS_Iterator w(ff, false); w.More(); w.Next())
        bld.Add(of, offset_wire(w.Value()).Oriented(w.Value().Orientation()));
      if (sf[(size_t)i].kind != GeomAbs_Plane)
      {
        ShapeIMap done;
        for (TopExp_Explorer x(ff, TopAbs_EDGE); x.More(); x.Next())
        {
          const int k = edges.FindIndex(x.Current());
          if (done.Contains(edges(k)) || gone[(size_t)k])
            continue;
          done.Add(edges(k));
          if (!quad::add_pcurve(ex[(size_t)k], of, TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), ff, seam[(size_t)k] != 0, 1e3 * tol))
            return no("an offset edge's curve on its face failed");
        }
      }
      const TopoDS_Face inner = TopoDS::Face(of.Oriented(TopAbs::Reverse(f.Orientation())));
      bld.Add(shell, inner);
      sources.Bind(inner, Source{f, (uint8_t)REL_GENERATED});
    }
    else
    {
      TopLoc_Location                 loc;
      const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(ff, loc);
      for (TopoDS_Iterator w(ff, false); w.More(); w.Next())
      {
        TopoDS_Face rim;
        bld.MakeFace(rim, surf, loc, BRep_Tool::Tolerance(ff));
        bld.Add(rim, w.Value());
        bld.Add(rim, offset_wire(w.Value()).Oriented(TopAbs::Reverse(w.Value().Orientation())));
        const TopoDS_Face r = TopoDS::Face(rim.Oriented(f.Orientation()));
        bld.Add(shell, r);
        h->AddModified(f, r);
      }
    }
  }
  for (int i = 1; i <= ne; ++i)
    if (!ex[(size_t)i].IsNull())
      BRepLib::SameParameter(ex[(size_t)i], 1e3 * tol);
  if (!ok_wires)
    return no("an edge without its offset");
  shell.Closed(true);
  TopoDS_Solid wall;
  bld.MakeSolid(wall);
  bld.Add(wall, shell.Oriented(old_shell.Orientation()));
  if (!BRepCheck_Analyzer(wall).IsValid())
  {
    if (const char* dir = std::getenv("NOSHAPE_SHELL_DUMP"))
      BRepTools::Write(wall, (std::string(dir) + "/quadrics-invalid.brep").c_str());
    return no("the wall is invalid");
  }
  {
    BOPAlgo_ArgumentAnalyzer an;
    an.SetShape1(wall);
    an.ArgumentTypeMode()   = false;
    an.SelfInterMode()      = true;
    an.SmallEdgeMode()      = false;
    an.RebuildFaceMode()    = false;
    an.TangentMode()        = false;
    an.MergeVertexMode()    = false;
    an.MergeEdgeMode()      = false;
    an.ContinuityMode()     = false;
    an.CurveOnSurfaceMode() = false;
    an.Perform();
    if (an.HasFaulty())
      return no("the offsets cross each other or the part");
  }
  const double vw = volume_of(wall), vs = volume_of(so);
  if (dbg)
    std::fprintf(stderr, "quadrics: wall volume %.17g solid %.17g\n", vw, vs);
  if (!(vw > 0) || !(vw <= vs * (1 + 1e-9)))
    return no("the wall's volume is off");
  out.result  = wall;
  out.history = shell_history(solid, wall, {h}, {}, sources, removed);
  return true;
}

bool by_slabs(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t,
              double sign, Built& out)
{
  const bool dbg = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       t_last = std::chrono::steady_clock::now();
  auto       lap = [&](const char* what) {
    if (!dbg)
      return;
    const auto now = std::chrono::steady_clock::now();
    std::fprintf(stderr, "slabs: %s %.0f ms\n", what, std::chrono::duration<double, std::milli>(now - t_last).count());
    t_last = now;
  };
  ShapeIMap rm;
  for (const TopoDS_Shape& f : removed)
    rm.Add(f);
  SourceMap   sources;
  ShapeList   tools;
  const double d = sign * t;
  ShapeAncMap ef;
  TopExp::MapShapesAndAncestors(solid, TopAbs_EDGE, TopAbs_FACE, ef);
  // Cone and cylinder bands (chamfers and fillets round an arc) meeting a
  // removed face along an arc about their axis: slab and filler as one
  // revolved section, so the offset face stays one cone (like the offset
  // path's) and the section can be clipped where it crosses the axis.
  ShapeIMap revolved; // faces and edges handled here
  for (TopExp_Explorer x(solid, TopAbs_FACE); x.More(); x.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(x.Current());
    if (rm.Contains(f))
      continue;
    try
    {
      TopoDS_Shape r = revolved_band(f, ef, rm, d, sources, revolved);
      if (!r.IsNull())
        tools.Append(r);
    }
    catch (const Standard_Failure&)
    {
    }
  }
  for (TopExp_Explorer x(solid, TopAbs_FACE); x.More(); x.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(x.Current());
    if (rm.Contains(f) || revolved.Contains(f))
      continue;
    // Closed faces (full cylinders, ...) are split first: thickening a face
    // across its seam gives a slab the booleans can't use.
    std::vector<TopoDS_Face> pieces;
    bool                     closed = false;
    for (TopExp_Explorer ex(f, TopAbs_EDGE); ex.More() && !closed; ex.Next())
      closed = BRep_Tool::IsClosed(TopoDS::Edge(ex.Current()), f);
    if (closed)
    {
      ShapeUpgrade_ShapeDivideClosed dc(f);
      dc.SetNbSplitPoints(1);
      dc.Perform();
      for (TopExp_Explorer y(dc.Result(), TopAbs_FACE); y.More(); y.Next())
      {
        TopoDS_Face p = TopoDS::Face(y.Current());
        pieces.push_back(f.Orientation() == p.Orientation() ? p : TopoDS::Face(p.Reversed()));
      }
    }
    else
      pieces.push_back(f);
    for (const TopoDS_Face& p : pieces)
    {
      TopoDS_Shape s;
      try
      {
        s = slab(p, d, f, sources);
      }
      catch (const Standard_Failure&)
      {
      }
      // A slab that can't be built is a convex blend thinner than the wall:
      // its neighbours' slabs cover it.
      if (!s.IsNull())
        tools.Append(s);
      else if (std::getenv("NOSHAPE_SHELL_DEBUG"))
        std::fprintf(stderr, "slabs: no slab for a face of type %d (area %.6g)\n", (int)BRepAdaptor_Surface(p).GetType(), [&] { GProp_GProps g; BRepGProp::SurfaceProperties(p, g); return g.Mass(); }());
    }
    if (collapsing_blend(f, d))
    {
      TopoDS_Shape fill;
      try
      {
        fill = BRepAdaptor_Surface(f).GetType() == GeomAbs_Sphere ? collapsed_corner_filler(f, solid, rm, d, sources)
                                                                  : collapsed_blend_filler(f, ef, rm, d, sources);
      }
      catch (const Standard_Failure&)
      {
      }
      if (std::getenv("NOSHAPE_SHELL_DEBUG"))
        std::fprintf(stderr, "slabs: collapsed blend filler %s volume %.6g\n", fill.IsNull() ? "none" : "ok", fill.IsNull() ? 0.0 : volume_of(fill));
      if (!fill.IsNull())
        tools.Append(fill);
    }
  }
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(ef.FindKey(i));
    if (BRep_Tool::Degenerated(e) || ef(i).Extent() != 2 || revolved.Contains(e))
      continue;
    const TopoDS_Face f1 = TopoDS::Face(ef(i).First()), f2 = TopoDS::Face(ef(i).Last());
    if (f1.IsSame(f2) || BRep_Tool::IsClosed(e, f1))
      continue;
    const bool r1 = rm.Contains(f1), r2 = rm.Contains(f2);
    if (r1 && r2)
      continue;
    BRepAdaptor_Curve   c(e);
    TopoDS_Shape        fill;
    std::vector<TopoDS_Shape> owner;
    Section             section;
    if (r1 || r2)
    {
      // Along a removed face: the offset of the kept face `f` extends up to
      // the removed face's surface (cross-section: edge point, its offset,
      // and the offset line's hit with the removed face's tangent plane).
      const TopoDS_Face f = r1 ? f2 : f1, g = r1 ? f1 : f2;
      const double      sense = edge_sense(e, f);
      owner = {f, f, g};
      section = [&c, f, g, sense, d](double prm) -> std::vector<gp_Pnt> {
        gp_Pnt E;
        gp_Vec T;
        c.D1(prm, E, T);
        if (T.Magnitude() < 1e-12)
          return {};
        T.Normalize();
        const gp_Vec nf(normal_at(f, E)), ng(normal_at(g, E));
        const gp_Vec u = -(nf.Crossed(T * sense)); // out of f, across the edge
        const double un = u.Dot(ng);
        if (un < 1e-9)
          return {};
        const double a = -d * nf.Dot(ng) / un;
        if (a <= 1e-9 * std::abs(d))
          return {};
        const gp_Pnt B = E.Translated(nf * d);
        return {E, B, B.Translated(u * std::min(a, 20 * std::abs(d)))};
      };
    }
    else
    {
      // Between two kept faces whose offsets move apart (concave edges for an
      // inward wall, convex ones outward): extend both offsets until they meet.
      const double sense = edge_sense(e, f1);
      owner = {f1, f1, f2, f2};
      section = [&c, f1, f2, sense, d](double prm) -> std::vector<gp_Pnt> {
        gp_Pnt E;
        gp_Vec T;
        c.D1(prm, E, T);
        if (T.Magnitude() < 1e-12)
          return {};
        T.Normalize();
        const gp_Vec n1(normal_at(f1, E)), n2(normal_at(f2, E));
        const gp_Vec u1 = n1.Crossed(T * sense); // into f1
        const double gap = u1.Dot(n2) * (d < 0 ? 1.0 : -1.0);
        const double c12 = n1.Dot(n2);
        if (gap < 1e-3 || c12 < -1 + 1e-6)
          return {};
        return {E, E.Translated(n1 * d), E.Translated((n1 + n2) * (d / (1 + c12))), E.Translated(n2 * d)};
      };
    }
    try
    {
      fill = sweep_section(e, section, owner, sources);
    }
    catch (const Standard_Failure&)
    {
    }
    if (!fill.IsNull())
      tools.Append(fill);
  }
  // Corners where every edge between the kept faces opens a gap (box
  // corners for an outward wall, pocket corners inward): fill the vertex's
  // normal cone up to the offset planes.
  ShapeAncMap vf, ve;
  TopExp::MapShapesAndAncestors(solid, TopAbs_VERTEX, TopAbs_FACE, vf);
  TopExp::MapShapesAndAncestors(solid, TopAbs_VERTEX, TopAbs_EDGE, ve);
  for (int i = 1; i <= vf.Extent(); ++i)
  {
    std::vector<TopoDS_Face> fs;
    ShapeIMap                seen;
    bool                     skip = false;
    for (ShapeList::Iterator it(vf(i)); it.More(); it.Next())
      if (!seen.Contains(it.Value()))
      {
        seen.Add(it.Value());
        fs.push_back(TopoDS::Face(it.Value()));
        skip = skip || rm.Contains(it.Value());
      }
    if (skip || fs.size() < 3)
      continue;
    const TopoDS_Vertex& vx = TopoDS::Vertex(vf.FindKey(i));
    const gp_Pnt         V  = BRep_Tool::Pnt(vx);
    std::vector<gp_Vec>  n;
    gp_Vec               mean(0, 0, 0);
    for (const TopoDS_Face& f : fs)
    {
      n.push_back(gp_Vec(normal_at(f, V)));
      mean += n.back();
    }
    // Every edge at the vertex must open a gap.
    bool all_gaps = true;
    std::vector<std::pair<int, int>> pairs;
    ShapeIMap                        seen_edges;
    for (ShapeList::Iterator it(ve.FindFromKey(vx)); it.More() && all_gaps; it.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(it.Value());
      if (BRep_Tool::Degenerated(e) || seen_edges.Contains(e))
        continue;
      seen_edges.Add(e);
      int a = -1, b = -1;
      for (int k = 0; k < (int)fs.size(); ++k)
        for (TopExp_Explorer x(fs[k], TopAbs_EDGE); x.More(); x.Next())
          if (x.Current().IsSame(e))
          {
            (a < 0 ? a : b) = k;
            break;
          }
      if (a < 0 || b < 0 || a == b)
      {
        all_gaps = false;
        break;
      }
      // Convex pair when face b's area centroid lies behind face a's plane.
      GProp_GProps g;
      BRepGProp::SurfaceProperties(fs[b], g);
      const double behind = gp_Vec(V, g.CentreOfMass()).Dot(n[a]);
      all_gaps = d > 0 ? behind < -1e-9 : behind > 1e-9;
      pairs.push_back({a, b});
    }
    if (!all_gaps || pairs.size() != fs.size())
      continue;
    // Half-spaces (p - o).m <= 0: the offset planes, and the cone's sides
    // (planes through V spanned by the normals of faces sharing an edge).
    const double        sg = d > 0 ? 1.0 : -1.0;
    std::vector<std::pair<gp_Pnt, gp_Vec>> hs;
    for (const gp_Vec& m : n)
      hs.push_back({V.Translated(m * d), m * sg});
    for (auto [a, b] : pairs)
    {
      gp_Vec m = n[a].Crossed(n[b]);
      if (m.Magnitude() < 1e-9)
        continue;
      // The cone round the normals the wall grows along: outward round their mean, inward round its opposite (the
      // mean's is the void's: a pocket's floor corners were left out, the wall thinning to a point; live 1747
      // mitres them, 13472 mm³).
      if (m.Dot(mean) * sg > 0)
        m.Reverse();
      hs.push_back({V, m});
    }
    TopoDS_Shape poly;
    try
    {
      poly = convex_polyhedron(hs, std::abs(d));
    }
    catch (const Standard_Failure&)
    {
    }
    if (poly.IsNull())
      continue;
    for (TopExp_Explorer e(poly, TopAbs_FACE); e.More(); e.Next())
    {
      GProp_GProps g;
      BRepGProp::SurfaceProperties(e.Current(), g);
      for (size_t k = 0; k < n.size(); ++k)
        if (std::abs(gp_Vec(V, g.CentreOfMass()).Dot(n[k]) - d) < 1e-7 * (1 + std::abs(d)))
          sources.Bind(e.Current(), Source{fs[k], (uint8_t)REL_GENERATED});
    }
    tools.Append(poly);
  }
  lap("tools");
  if (tools.IsEmpty())
    return false;
  if (dbg)
  {
    // Each tool's validity and signed volume (a tool turned inside out spoils the fuse).
    int k = 0;
    for (ShapeList::Iterator it(tools); it.More(); it.Next(), ++k)
    {
      const bool   ok = BRepCheck_Analyzer(it.Value()).IsValid();
      const double v  = volume_of(it.Value());
      if (!ok || v <= 0)
      {
        std::fprintf(stderr, "slabs: tool %d valid %d volume %.6g\n", k, ok, v);
        if (const char* dir = std::getenv("NOSHAPE_SHELL_DUMP"))
          BRepTools::Write(it.Value(), (std::string(dir) + "/tool" + std::to_string(k) + "-" + std::to_string(tools.Extent()) + ".brep").c_str());
      }
    }
  }
  TopoDS_Shape union_;
  HHist        hu;
  if (tools.Extent() == 1)
    union_ = tools.First();
  else
  {
    ShapeList first, rest = tools;
    first.Append(rest.First());
    rest.RemoveFirst();
    TopoDS_Shape fu_out;
    HHist        fu_hist;
    if (!quick_boolean(0, first, rest, bop_parallel(), fu_out, fu_hist))
      return false;
    union_ = fu_out;
    hu = fu_hist;
  }
  lap("fuse");
  TopoDS_Shape r;
  HHist        hb;
  if (dbg)
    std::fprintf(stderr, "slabs: %d tools, union volume %.6g\n", tools.Extent(), volume_of(union_));
  if (sign < 0)
  {
    ShapeList a1, a2;
    a1.Append(solid);
    a2.Append(union_);
    TopoDS_Shape co_out;
    HHist        co_hist;
    // (OCCT's in parallel: a quarter faster on qa's shell matrix, SHELL-SLOW.)
    if (!quick_boolean(2, a1, a2, bop_parallel(), co_out, co_hist))
    {
      if (dbg)
        std::fprintf(stderr, "slabs: common failed\n");
      return false;
    }
    r = co_out;
    hb = co_hist;
  }
  else
  {
    // Outward: the wall is outside the solid, and stops at the planes of the
    // removed faces.
    ShapeList objs, tls;
    objs.Append(union_);
    tls.Append(solid);
    Bnd_Box box;
    BRepBndLib::Add(union_, box);
    for (const TopoDS_Shape& g : removed)
    {
      gp_Pnt p;
      gp_Dir n;
      if (BRepAdaptor_Surface(TopoDS::Face(g)).GetType() != GeomAbs_Plane || !inner_point(TopoDS::Face(g), p, n))
        continue;
      // A prism of a large rectangle on the removed face's plane, outward.
      const double size = std::sqrt(box.SquareExtent()) * 2 + 10 * t;
      gp_Ax3       ax(p, n);
      TopoDS_Face  big = BRepBuilderAPI_MakeFace(gp_Pln(ax), -size, size, -size, size).Face();
      tls.Append(BRepPrimAPI_MakePrism(big, gp_Vec(n) * size).Shape());
    }
    TopoDS_Shape cu_out;
    HHist        cu_hist;
    if (!quick_boolean(1, objs, tls, false, cu_out, cu_hist))
      return false;
    r = cu_out;
    hb = cu_hist;
  }
  lap("common");
  // Merge the faces split up by the slab construction.
  ShapeUpgrade_UnifySameDomain usd(r, true, true, false);
  usd.SetLinearTolerance(1e-7);
  usd.Build();
  HHist hs = usd.History();
  lap("unify");
  const TopoDS_Shape split = single_solid(r);
  r = single_solid(usd.Shape());
  // Unify can break a valid result (qa5 P1-16 at 1 mm): keep the split one.
  if (!valid_solid(r) && valid_solid(split))
  {
    r = split;
    hs = HHist();
  }
  // Still invalid (P1-16's chamfer corners: the volume is live's, the
  // check fails): healed, when that keeps the volume.
  if (!valid_solid(r) && !r.IsNull())
  {
    ShapeFix_Shape fix(r);
    fix.Perform();
    const TopoDS_Shape fixed = single_solid(fix.Shape());
    const double       v0    = volume_of(r);
    if (valid_solid(fixed) && std::abs(volume_of(fixed) - v0) <= 1e-6 * std::abs(v0))
    {
      const HHist fh = fix.Context().IsNull() ? HHist() : fix.Context()->History();
      if (!fh.IsNull())
      {
        if (hs.IsNull())
          hs = fh;
        else
          hs->Merge(fh);
      }
      r = fixed;
    }
  }
  if (std::getenv("NOSHAPE_SHELL_DEBUG"))
    std::fprintf(stderr, "slabs: result valid %d volume %.6g (solid %.6g)\n", (int)valid_solid(r), r.IsNull() ? 0.0 : volume_of(r), volume_of(solid));
  if (!valid_solid(r))
    return false;
  const double vr = volume_of(r), vs = volume_of(solid);
  if (std::getenv("NOSHAPE_SHELL_DEBUG"))
    std::fprintf(stderr, "slabs: result volume %.17g solid %.17g\n", vr, vs);
  // (Inward: not more than the solid. Equal within rounding (walls covering a thin part whole: the part as it was) is
  // not more: OCCT's and noshape's Booleans round that 1e-16 either way, and a Shell failed or not by it
  // (RUST-SHELL-2AFC).)
  if (!(vr > 0) || (sign < 0 && !(vr <= vs * (1 + 1e-9))))
    return false;
  out.result = r;
  out.history = shell_history(solid, r, {hb, hs}, {hu, hb, hs}, sources, removed);
  return true;
}

void compare_with_slabs(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t, double sign, const Built& mine)
{
  Built slabs;
  if (!by_slabs(solid, removed, t, sign, slabs))
  {
    std::fprintf(stderr, "shell-compare: slabs failed, ours built\n");
    return;
  }
  auto counts = [](const TopoDS_Shape& s) {
    int n[3] = {0, 0, 0};
    const TopAbs_ShapeEnum k[3] = {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX};
    for (int i = 0; i < 3; ++i)
    {
      ShapeIMap m;
      TopExp::MapShapes(s, k[i], m);
      n[i] = m.Extent();
    }
    return std::array<int, 3>{n[0], n[1], n[2]};
  };
  auto sig = [&](const Built& b) {
    std::vector<std::string> v;
    ShapeIMap in;
    TopExp::MapShapes(solid, TopAbs_FACE, in);
    for (int i = 1; i <= in.Extent(); ++i)
    {
      const TopoDS_Shape& f = in(i);
      auto one = [&](const char* rel, const ShapeList& l) {
        for (ShapeList::Iterator it(l); it.More(); it.Next())
        {
          GProp_GProps g;
          BRepGProp::SurfaceProperties(it.Value(), g);
          const gp_Pnt c = g.CentreOfMass();
          char buf[200];
          // (Rounded at 1e-12 of the part: 6.5e-34 against 0.)
          auto r = [&](double x) { return std::abs(x) < 1e-12 ? 0.0 : x; };
          std::snprintf(buf, sizeof buf, "f%d %s a%.6e c(%.6e %.6e %.6e)", i, rel, g.Mass(), r(c.X()), r(c.Y()), r(c.Z()));
          v.push_back(buf);
        }
      };
      if (b.history->IsRemoved(f))
        v.push_back("f" + std::to_string(i) + " removed");
      one("mod", b.history->Modified(f));
      one("gen", b.history->Generated(f));
      bool same = false;
      for (TopExp_Explorer x(b.result, TopAbs_FACE); x.More() && !same; x.Next())
        same = x.Current().IsSame(f);
      if (same)
      {
        ShapeList self;
        self.Append(f);
        one("mod", self);
      }
    }
    // (A face listed twice among one face's images is one image.)
    std::sort(v.begin(), v.end());
    v.erase(std::unique(v.begin(), v.end()), v.end());
    return v;
  };
  const double va = volume_of(mine.result), vb = volume_of(slabs.result);
  const auto   ca = counts(mine.result), cb = counts(slabs.result);
  auto         sa = sig(mine), sb = sig(slabs);
  const bool vol_ok = std::abs(va - vb) <= 1e-9 * std::abs(vb);
  std::fprintf(stderr, "shell-compare: %s volume %.17g / %.17g counts %d %d %d / %d %d %d history %s\n", vol_ok && ca == cb ? "same" : "DIFFERENT", va, vb, ca[0], ca[1], ca[2], cb[0], cb[1], cb[2], sa == sb ? "same" : "differs");
  if (sa != sb && std::getenv("NOSHAPE_SHELL_COMPARE_HISTORY"))
  {
    for (const std::string& x : sa)
      std::fprintf(stderr, "  planes %s\n", x.c_str());
    for (const std::string& x : sb)
      std::fprintf(stderr, "  slabs  %s\n", x.c_str());
  }
}

OpOut op_shell_solid_unbudgeted(const OcShape& solid, const OcShapeList& faces, double thickness, bool outward,
                                uint8_t method)
{
  require_non_null(solid.shape, "solid");
  if (std::getenv("NOSHAPE_DUMP_OPS"))
  {
    std::ostringstream p;
    p.precision(17);
    p << "shell thickness " << thickness << " outward " << outward << " method " << (int)method;
    dump_op("shell", solid.shape, faces.items, p.str());
  }
  if (!(thickness > 1e-9))
    fail_invalid("shell thickness must be positive");
  for (const TopoDS_Shape& f : faces.items)
    as_face(f, "face to remove");
  // Faces to remove, oriented as in the solid.
  ShapeIMap all;
  TopExp::MapShapes(solid.shape, TopAbs_FACE, all);
  std::vector<TopoDS_Shape> removed;
  for (const TopoDS_Shape& f : faces.items)
  {
    const int i = all.FindIndex(f);
    if (i <= 0)
      fail_invalid("face to remove is not a face of the solid");
    removed.push_back(all(i));
  }
  const double sign = outward ? 1.0 : -1.0;
  if (!outward && in_onshape_mitre_band(solid.shape, removed, thickness))
    fail_op("shell failed (Parasolid can't build this wall: live G51)");
  if (!outward && blind_hole_floor_too_thin(solid.shape, removed, thickness))
    fail_op("shell failed (Parasolid can't offset a blind hole's bottom past the cavity floor: live 'core (2026-10-27 b)')");
  const double gap = outward ? std::numeric_limits<double>::infinity() : offsets_touch_gap(solid.shape, removed, thickness);
  if (gap <= 1e-8)
    fail_op("shell failed (Parasolid can't build walls whose offsets touch)");
  // Walls that nearly touch (within ten times OCCT's offset tolerance) merge
  // or part by less than it sees: its offset keeps both faces whole and
  // overlapping (2.501 mm: 0.003 mm³ off). The slab construction trims them.
  // OCCT's offset drops the sector of a fillet whose offset collapses or
  // vanishes (16 mm³ short on qa5 P1-16's block at 2.5 mm): slabs then.
  bool collapses = false;
  {
    ShapeIMap rmset;
    for (const TopoDS_Shape& f : removed)
      rmset.Add(f);
    for (TopExp_Explorer e(solid.shape, TopAbs_FACE); e.More() && !collapses; e.Next())
      collapses = !rmset.Contains(e.Current()) && collapsing_blend(TopoDS::Face(e.Current()), sign * thickness);
  }
  const bool offset_ok = gap > 1e-5 && !collapses;
  Built      b;
  t_shell_path = "";
  // method: 0 = automatic, 1 = offset only, 2 = slabs only, 3 = planes only, 4 = quadrics only (tests).
  const bool dbg = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       timed = [&](const char* what, auto&& run) {
    // (Also when it starts: a path the budget stops never reports its end.)
    if (dbg)
      std::fprintf(stderr, "shell: %s ...\n", what);
    const auto t0 = std::chrono::steady_clock::now();
    const bool ok = run();
    if (ok)
      t_shell_path = what;
    if (dbg)
      std::fprintf(stderr, "shell: %s %s in %.0f ms\n", what, ok ? "done" : "failed",
                   std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count());
    return ok;
  };
  const bool done = (method == 0 && timed("regions", [&] { return by_regions(solid.shape, removed, thickness, sign, b); })) ||
                    ((method == 0 || method == 1) && (offset_ok || method == 1) && timed("offset", [&] { return by_offset(solid.shape, removed, thickness, sign, b); })) ||
                    ((method == 0 || method == 3) && !std::getenv("NOSHAPE_SHELL_NO_PLANES") && timed("planes", [&] {
                       const bool ok = by_planes(solid.shape, removed, thickness, sign, b);
                       if (ok && std::getenv("NOSHAPE_SHELL_COMPARE"))
                         compare_with_slabs(solid.shape, removed, thickness, sign, b);
                       return ok;
                     })) ||
                    ((method == 0 || method == 4) && !std::getenv("NOSHAPE_SHELL_NO_QUADRICS") && timed("quadrics", [&] {
                       const bool ok = by_quadrics(solid.shape, removed, thickness, sign, b);
                       if (ok && std::getenv("NOSHAPE_SHELL_COMPARE"))
                         compare_with_slabs(solid.shape, removed, thickness, sign, b);
                       return ok;
                     })) ||
                    ((method == 0 || method == 2) && timed("slabs", [&] { return by_slabs(solid.shape, removed, thickness, sign, b); }));
  if (!done)
    fail_op("shell failed");
  return make_result({solid.shape}, b.result, b.history.get());
}

} // namespace

// On the blends' CPU budget (a failing shell spent 2 min in slab booleans on
// qa's delete-crash document, live fails or builds in milliseconds); booleans
// run serial unless NOSHAPE_BOP_PARALLEL, so the calling thread's clock sees
// all the work.
OpOut op_shell_solid(const OcShape& solid, const OcShapeList& faces, double thickness, bool outward,
                     uint8_t method)
{
  // On its own copy: OCCT's offset and Booleans here change their input.
  const OwnCopy     own(solid);
  const OcShapeList removed = own.subs(faces);
  OpOut             out;
  const int r = run_budgeted(blend_budget_seconds(), [&] { out = op_shell_solid_unbudgeted(own.shape(), removed, thickness, outward, method); });
  if (r == 1)
    fail_op("shell took too long");
  if (r == 2)
    fail_op("cancelled");
  return out;
}


// ------------------------------------------------------------------ draft

namespace {

// A box on the positive side of the plane (o, n), `size` on each side.
TopoDS_Shape half_box(const gp_Pnt& o, const gp_Dir& n, double size)
{
  gp_Ax2 ax(o, n);
  gp_Pnt c = o.Translated(gp_Vec(ax.XDirection()) * -size + gp_Vec(ax.YDirection()) * -size);
  return BRepPrimAPI_MakeBox(gp_Ax2(c, n, ax.XDirection()), 2 * size, 2 * size, size).Shape();
}

TopoDS_Shape common_of(const TopoDS_Shape& a, const TopoDS_Shape& b)
{
  ShapeList    ca, cb;
  TopoDS_Shape r;
  HHist        h;
  ca.Append(a);
  cb.Append(b);
  if (!quick_boolean(2, ca, cb, false, r, h))
    return TopoDS_Shape();
  return solid_count(r) > 0 ? r : TopoDS_Shape();
}

// Bind every face of `w`: on the plane (o, n) -> Modified from `f`, else Generated.
void bind_wedge(const TopoDS_Shape& w, const TopoDS_Face& f, const gp_Pnt& o, const gp_Dir& n, double tol,
                SourceMap& sources)
{
  for (TopExp_Explorer e(w, TopAbs_FACE); e.More(); e.Next())
  {
    GProp_GProps g;
    BRepGProp::SurfaceProperties(e.Current(), g);
    const bool on = BRepAdaptor_Surface(TopoDS::Face(e.Current())).GetType() == GeomAbs_Plane &&
                    std::abs(gp_Vec(o, g.CentreOfMass()).Dot(gp_Vec(n))) < tol;
    sources.Bind(e.Current(), Source{f, on ? (uint8_t)REL_MODIFIED : (uint8_t)REL_GENERATED});
  }
}

} // namespace

// Draft by booleans, for when BRepOffsetAPI_DraftAngle fails or builds an
// invalid solid (faces whose drafted surface vanishes: an R3 fillet drafted
// 30° over 10 mm becomes a cone that ends below the top). Parasolid's
// semantics, verified on Onshape's volumes: each face parallel to the pull
// direction moves inward by h·tan(angle) (h = height above the neutral plane),
// faces that vanish disappear and their neighbours meet. Per face, the wedge
// between it and its drafted surface is cut away (or added below the neutral
// plane). Planar faces and cylinder bands (fillets) parallel to the pull.
OpOut op_draft_wedges(const OcShape& solid_in, const OcShapeList& faces_in, P3 pull, const PlaneF& neutral,
                      double angle)
{
  // On its own copy: its Booleans change their input (GUARD-SEGV-MALLOC-DEADLOCK).
  const OwnCopy     own(solid_in);
  const OcShape&    solid = own.shape();
  const OcShapeList faces = own.subs(faces_in);
  require_non_null(solid.shape, "solid");
  if (!(std::abs(angle) < M_PI / 2.0))
    fail_invalid("draft angle must be less than 90 degrees");
  const gp_Dir P = to_dir(pull, "pull direction");
  const gp_Pnt O = to_pln(neutral).Location();
  ShapeIMap    all;
  TopExp::MapShapes(solid.shape, TopAbs_FACE, all);
  Bnd_Box box;
  BRepBndLib::Add(solid.shape, box);
  const double size = std::sqrt(box.SquareExtent()) * 4 + 1;
  const double tol  = 1e-7 * size;
  const double ta = std::tan(angle), ca = std::cos(angle), sa = std::sin(angle);
  auto         height = [&](const gp_Pnt& p) { return gp_Vec(O, p).Dot(gp_Vec(P)); };
  ShapeList    removal, addition;
  SourceMap    sources;
  for (const TopoDS_Shape& fx : faces.items)
  {
    const int i = all.FindIndex(fx);
    if (i <= 0)
      fail_invalid("face to draft is not a face of the solid");
    const TopoDS_Face f = TopoDS::Face(all(i));
    double            hmin = 1e300, hmax = -1e300;
    for (TopExp_Explorer v(f, TopAbs_VERTEX); v.More(); v.Next())
    {
      const double h = height(BRep_Tool::Pnt(TopoDS::Vertex(v.Current())));
      hmin = std::min(hmin, h);
      hmax = std::max(hmax, h);
    }
    BRepAdaptor_Surface s(f);
    if (s.GetType() == GeomAbs_Plane)
    {
      gp_Pnt q;
      gp_Dir n;
      if (!inner_point(f, q, n) || std::abs(n.Dot(P)) > 1e-7)
        fail_op("draft fallback: a face is not parallel to the pull direction");
      const double D = std::max(std::abs(hmin), std::abs(hmax)) * std::abs(ta) * 1.001 + tol;
      const gp_Pnt Q0 = q.Translated(gp_Vec(P) * -height(q));
      const gp_Dir n2(gp_Vec(n) * ca + gp_Vec(P) * sa);
      // A face bounded by lines: the prisms of a clean polygon through its vertices, on its plane (a 0.1 mm wide
      // chamfer face with 0.1 mm vertex tolerances made OCCT's common of its prism lose or double the wedge,
      // cli-draft-die).
      TopoDS_Shape base = f;
      {
        std::vector<gp_Pnt> pts;
        int                 wires = 0;
        bool                lines = true;
        for (TopExp_Explorer w(f, TopAbs_WIRE); w.More(); w.Next())
          ++wires;
        for (TopExp_Explorer x(f, TopAbs_EDGE); x.More() && lines; x.Next())
          lines = BRepAdaptor_Curve(TopoDS::Edge(x.Current())).GetType() == GeomAbs_Line;
        if (wires == 1 && lines)
          for (BRepTools_WireExplorer wx(BRepTools::OuterWire(f), f); wx.More(); wx.Next())
          {
            const gp_Pnt v = BRep_Tool::Pnt(wx.CurrentVertex());
            pts.push_back(v.Translated(gp_Vec(n) * -gp_Vec(q, v).Dot(gp_Vec(n))));
          }
        const TopoDS_Face poly = pts.size() >= 3 ? polygon_face(pts) : TopoDS_Face();
        if (!poly.IsNull())
          base = poly;
      }
      TopoDS_Shape in  = BRepPrimAPI_MakePrism(base, gp_Vec(n) * -D).Shape();
      TopoDS_Shape out = BRepPrimAPI_MakePrism(base, gp_Vec(n) * D).Shape();
      // The drafted plane's half-space as a box just big enough for the prisms (one the part's size and more
      // left a small face's wedge out: its common with a sliver's prism came back without a solid, cli-draft-die).
      Bnd_Box pb;
      BRepBndLib::Add(in, pb);
      BRepBndLib::Add(out, pb);
      double reach = 0;
      if (!pb.IsVoid())
      {
        double x0, y0, z0, x1, y1, z1;
        pb.Get(x0, y0, z0, x1, y1, z1);
        for (const double x : {x0, x1})
          for (const double y : {y0, y1})
            for (const double z : {z0, z1})
              reach = std::max(reach, Q0.Distance(gp_Pnt(x, y, z)));
      }
      const double hs = 2 * reach + tol;
      TopoDS_Shape wr = common_of(in, half_box(Q0, n2, hs));
      TopoDS_Shape wa = common_of(out, half_box(Q0, n2.Reversed(), hs));
      for (auto [w, list] : {std::pair{wr, &removal}, std::pair{wa, &addition}})
        if (!w.IsNull() && volume_of(w) > tol * tol * tol)
        {
          bind_wedge(w, f, Q0, n2, tol * 10, sources);
          list->Append(w);
        }
      continue;
    }
    if (s.GetType() != GeomAbs_Cylinder || !s.Cylinder().Axis().Direction().IsParallel(P, 1e-7))
      fail_op("draft fallback: unsupported face");
    // A cylinder band: two arcs about the axis joined by two lines.
    std::vector<TopoDS_Edge> arcs;
    int                      lines = 0;
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
    {
      BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
      if (c.GetType() == GeomAbs_Circle)
        arcs.push_back(TopoDS::Edge(x.Current()));
      else if (c.GetType() == GeomAbs_Line)
        ++lines;
    }
    if (arcs.size() != 2 || lines != 2)
      fail_op("draft fallback: unsupported cylinder face");
    const double      R = s.Cylinder().Radius();
    BRepAdaptor_Curve c(arcs[0]);
    const gp_Circ     ci = c.Circle();
    const gp_Pnt      E  = c.Value(c.FirstParameter());
    const gp_Vec      rho = gp_Vec(ci.Location(), E).Normalized();
    gp_Pnt            q;
    gp_Dir            n;
    if (!inner_point(f, q, n))
      fail_op("draft fallback: degenerate face");
    const gp_Pnt axis_q = ci.Location().Translated(gp_Vec(P) * gp_Vec(ci.Location(), q).Dot(gp_Vec(P)));
    const double sgn    = gp_Vec(n).Dot(gp_Vec(axis_q, q)) > 0 ? 1.0 : -1.0; // convex: material at r < R
    // (r, h) in the half-plane through the arc start -> 3D.
    const double h_c = height(ci.Location());
    auto pt = [&](double r, double h) { return ci.Location().Translated(gp_Vec(P) * (h - h_c) + rho * r); };
    auto rd = [&](double h) { return R - sgn * h * ta; };
    std::vector<std::vector<std::pair<double, double>>> pieces;
    const double d0 = rd(hmin) - R, d1 = rd(hmax) - R;
    if (d0 * d1 >= 0)
      pieces.push_back({{R, hmin}, {R, hmax}, {rd(hmax), hmax}, {rd(hmin), hmin}});
    else
    {
      const double h0 = 0;
      pieces.push_back({{R, hmin}, {R, h0}, {rd(hmin), hmin}});
      pieces.push_back({{R, h0}, {R, hmax}, {rd(hmax), hmax}});
    }
    for (const auto& pc : pieces)
    {
      double rmid = 0;
      for (const auto& [r, h] : pc)
        rmid += r;
      rmid /= (double)pc.size();
      const bool removes = sgn * (rmid - R) < 0; // on the material side
      std::vector<gp_Pnt> poly;
      for (const auto& [r, h] : pc)
        poly.push_back(pt(r, h));
      poly = clip_at_axis(poly, ci.Location(), rho);
      if (poly.size() < 3 || polygon_area(poly) < tol * tol)
        continue;
      TopoDS_Face sec = polygon_face(poly);
      if (sec.IsNull())
        continue;
      BRepPrimAPI_MakeRevol mk(sec, ci.Axis(), c.LastParameter() - c.FirstParameter());
      if (!mk.IsDone())
        fail_op("draft fallback: revolve failed");
      TopoDS_Shape w = mk.Shape();
      // The face on the drafted cone stands for `f`.
      for (TopExp_Explorer x(w, TopAbs_FACE); x.More(); x.Next())
      {
        const bool cone = BRepAdaptor_Surface(TopoDS::Face(x.Current())).GetType() == GeomAbs_Cone;
        sources.Bind(x.Current(), Source{f, cone ? (uint8_t)REL_MODIFIED : (uint8_t)REL_GENERATED});
      }
      (removes ? removal : addition).Append(w);
    }
  }
  TopoDS_Shape       r = solid.shape;
  std::vector<HHist> chain;
  if (!removal.IsEmpty())
  {
    ShapeList args;
    args.Append(r);
    TopoDS_Shape cu_out;
    HHist        cu_hist;
    if (!quick_boolean(1, args, removal, bop_parallel(), cu_out, cu_hist))
      fail_op("draft fallback: cut failed");
    r = cu_out;
    chain.push_back(cu_hist);
  }
  if (!addition.IsEmpty())
  {
    ShapeList args;
    args.Append(r);
    TopoDS_Shape fu_out;
    HHist        fu_hist;
    if (!quick_boolean(0, args, addition, bop_parallel(), fu_out, fu_hist))
      fail_op("draft fallback: fuse failed");
    r = fu_out;
    chain.push_back(fu_hist);
  }
  ShapeUpgrade_UnifySameDomain usd(r, true, true, false);
  usd.SetLinearTolerance(1e-7);
  usd.Build();
  chain.push_back(usd.History());
  r = single_solid(usd.Shape());
  if (!valid_solid(r))
    fail_op("draft fallback gave an invalid solid");
  HHist h = shell_history(solid.shape, r, chain, chain, sources, {});
  return make_result({solid.shape}, r, h.get());
}


// Parting-line draft about curved parting edges (Parasolid's taper about a
// parting line, which OCCT's BRepOffsetAPI_DraftAngle can't do: the drafted
// face isn't a cone). Each face in `faces` is replaced by the ruled surface
// leaving its parting edges (those of `edges` it has) on the face's side,
// each ruling at `angle` to the pull in the plane of the pull and the face
// normal; positive angles lean into the material. The material between the
// face and the ruled surface is cut away (or added): a slab of the face is
// split by the ruled sheet and the pieces touching the face are the change.
OpOut op_draft_parting_curve(const OcShape& solid_in, const OcShapeList& faces_in, const OcShapeList& edges_in, P3 pull,
                             rust::Slice<const double> angles)
{
  // On its own copy: its Booleans change their input (GUARD-SEGV-MALLOC-DEADLOCK).
  const OwnCopy     own(solid_in);
  const OcShape&    solid = own.shape();
  const OcShapeList faces = own.subs(faces_in);
  const OcShapeList edges = own.subs(edges_in);
  require_non_null(solid.shape, "solid");
  if (angles.size() != faces.items.size())
    fail_invalid("one draft angle per face");
  for (double angle : angles)
    if (!(std::abs(angle) < M_PI / 2.0))
      fail_invalid("draft angle must be less than 90 degrees");
  const gp_Dir P = to_dir(pull, "pull direction");
  ShapeIMap    all;
  TopExp::MapShapes(solid.shape, TopAbs_FACE, all);
  Bnd_Box box;
  BRepBndLib::Add(solid.shape, box);
  const double size = std::sqrt(box.SquareExtent()) + 1e-3;
  const double tol  = 1e-7 * (size + 1);
  ShapeList    removal, addition;
  SourceMap    sources;
  std::vector<HHist> tool_chain;
  size_t fi = 0;
  for (const TopoDS_Shape& fx : faces.items)
  {
    const double angle = angles[fi++];
    const double ca = std::cos(angle), sa = std::sin(angle);
    const int i = all.FindIndex(fx);
    if (i <= 0)
      fail_invalid("face to draft is not a face of the solid");
    const TopoDS_Face f = TopoDS::Face(all(i));
    gp_Pnt q;
    gp_Dir qn;
    if (!inner_point(f, q, qn))
      fail_op("draft: degenerate face");
    const occ::handle<Geom_Surface> gs = BRep_Tool::Surface(f);
    // The face's reach along the pull from its parting edges: how long the
    // rulings and how thick the slab must be.
    double hmin = 1e300, hmax = -1e300;
    for (TopExp_Explorer v(f, TopAbs_VERTEX); v.More(); v.Next())
    {
      const double h = gp_Vec(gp::Origin(), BRep_Tool::Pnt(TopoDS::Vertex(v.Current()))).Dot(gp_Vec(P));
      hmin = std::min(hmin, h);
      hmax = std::max(hmax, h);
    }
    // A ruling must leave the face's far side; longer ones make a long thin
    // surface OCCT's point projection then misses. The face's extent along
    // the pull from its (tight) box.
    Bnd_Box fb;
    BRepBndLib::AddOptimal(f, fb, false, false);
    double x0, y0, z0, x1, y1, z1;
    fb.Get(x0, y0, z0, x1, y1, z1);
    for (double bx : {x0, x1})
      for (double by : {y0, y1})
        for (double bz : {z0, z1})
        {
          const double h = gp_Vec(bx, by, bz).Dot(gp_Vec(P));
          hmin = std::min(hmin, h);
          hmax = std::max(hmax, h);
        }
    const double len = (hmax - hmin) / std::max(ca, 1e-3) * 1.1 + size * 0.05 + 10 * tol;
    BRepBuilderAPI_Sewing sew(1e-6);
    int n_sheets = 0;
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
    {
      const TopoDS_Edge e = TopoDS::Edge(x.Current());
      bool parting = false;
      for (const TopoDS_Shape& pe : edges.items)
        parting |= pe.IsSame(e);
      if (!parting || BRep_Tool::Degenerated(e))
        continue;
      double a, b;
      const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, a, b);
      if (c.IsNull())
        fail_op("draft: parting edge without a curve");
      // The face's side of the edge along the pull: the face lies left of
      // the edge seen from its outward normal.
      double side;
      {
        gp_Pnt pm;
        gp_Vec tm;
        c->D1((a + b) / 2, pm, tm);
        GeomAPI_ProjectPointOnSurf pr(pm, gs);
        if (!pr.IsDone() || pr.NbPoints() == 0)
          fail_op("draft: parting edge off its face");
        double u, v;
        pr.LowerDistanceParameters(u, v);
        GeomLProp_SLProps lp(gs, u, v, 1, 1e-9);
        if (!lp.IsNormalDefined())
          fail_op("draft: no face normal on the parting edge");
        gp_Vec nm = lp.Normal();
        if (f.Orientation() == TopAbs_REVERSED)
          nm.Reverse();
        const gp_Vec into = nm.Crossed(tm) * edge_sense(e, f);
        side = into.Dot(gp_Vec(P)) >= 0 ? 1.0 : -1.0;
      }
      const bool   closed = c->IsPeriodic() && std::abs((b - a) - c->Period()) < 1e-9;
      // The draft surface's normal along the curve: perpendicular to the
      // curve, at the draft angle to the plane perpendicular to the pull
      // (live: every normal of the drafted face is), the one nearest the
      // face normal.
      const double target = side * sa;
      auto normal_at = [&](double t, gp_Vec& T) -> gp_Vec {
        gp_Pnt p;
        c->D1(t, p, T);
        if (T.Magnitude() < 1e-12)
          fail_op("draft: degenerate parting edge");
        T.Normalize();
        GeomAPI_ProjectPointOnSurf pr(p, gs);
        if (!pr.IsDone() || pr.NbPoints() == 0)
          fail_op("draft: parting edge off its face");
        double u, v;
        pr.LowerDistanceParameters(u, v);
        GeomLProp_SLProps lp(gs, u, v, 1, 1e-9);
        if (!lp.IsNormalDefined())
          fail_op("draft: no face normal on the parting edge");
        gp_Vec n = lp.Normal();
        if (f.Orientation() == TopAbs_REVERSED)
          n.Reverse();
        gp_Vec e1 = n - T * n.Dot(T);
        if (e1.Magnitude() < 1e-12)
          fail_op("draft: degenerate face normal");
        e1.Normalize();
        const gp_Vec e2 = T.Crossed(e1);
        const double pa = e1.Dot(gp_Vec(P)), pb = e2.Dot(gp_Vec(P)), rr = std::hypot(pa, pb);
        if (rr < std::abs(target) - 1e-12)
          fail_op("draft: the parting edge is too steep for the draft angle");
        const double base = std::atan2(pb, pa), dphi = std::acos(std::clamp(target / std::max(rr, 1e-300), -1.0, 1.0));
        double best = -2, phi = 0;
        for (double cand : {base + dphi, base - dphi})
          if (std::cos(cand) > best)
            best = std::cos(cand), phi = cand;
        return e1 * std::cos(phi) + e2 * std::sin(phi);
      };
      const double eps = (b - a) * 1e-4;
      // The ruling's far end at `t`: where the neighbouring tangent planes
      // meet (N × N'), leaving the curve on the face's side along the pull.
      auto far_at = [&](double t) -> gp_Pnt {
        gp_Vec       T, Tq;
        const gp_Vec Nt = normal_at(t, T);
        const double t0 = closed ? t - eps : std::max(a, t - eps), t1 = closed ? t + eps : std::min(b, t + eps);
        const gp_Vec dN = normal_at(t1, Tq) - normal_at(t0, Tq);
        gp_Vec       d  = Nt.Crossed(dN);
        if (d.Magnitude() < 1e-9 * dN.Magnitude() + 1e-15)
          d = Nt.Crossed(T); // a plane: any line in it across the curve
        d.Normalize();
        if (d.Dot(gp_Vec(P)) * side < 0)
          d.Reverse();
        return c->Value(t).Translated(d * len);
      };
      // Not periodic, even around a closed curve (OCCT's point projection
      // misses points on periodic B-spline faces); a closed curve's ends meet
      // with the same tangent.
      const int N = 128;
      occ::handle<NCollection_HArray1<gp_Pnt>> pts = new NCollection_HArray1<gp_Pnt>(1, N + 1), near_pts = new NCollection_HArray1<gp_Pnt>(1, N + 1);
      occ::handle<NCollection_HArray1<double>> prm = new NCollection_HArray1<double>(1, N + 1);
      for (int k = 0; k <= N; ++k)
      {
        const double t = k == N ? b : a + (b - a) * k / N;
        const double tt = closed && k == N ? a : t; // the same point
        near_pts->SetValue(k + 1, c->Value(tt));
        pts->SetValue(k + 1, far_at(tt));
        prm->SetValue(k + 1, t);
      }
      // Both ends interpolated on the same parameters (the same knots), so
      // the rulings join C(t) to its far end: a degree 3 × 1 B-spline (live:
      // SPLINE, degrees 1 and 3).
      GeomAPI_Interpolate ip(pts, prm, false, 1e-9), in(near_pts, prm, false, 1e-9);
      if (closed)
      {
        const gp_Vec tn = c->DN(a, 1);
        const double h  = (b - a) * 1e-4;
        const gp_Vec tf = gp_Vec(far_at(a - h), far_at(a + h)) / (2 * h);
        in.Load(tn, tn, false);
        ip.Load(tf, tf, false);
      }
      ip.Perform();
      in.Perform();
      if (!ip.IsDone() || !in.IsDone())
        fail_op("draft: ruling end curve failed");
      occ::handle<Geom_Curve> far_curve(ip.Curve()), start(in.Curve());  // direct-init: older GCC (CI) rejects the converting copy-init
      TopoDS_Edge e0 = BRepBuilderAPI_MakeEdge(start, a, b).Edge();
      TopoDS_Edge e1 = BRepBuilderAPI_MakeEdge(far_curve, a, b).Edge();
      TopoDS_Face rf = BRepFill::Face(e0, e1);
      if (rf.IsNull())
        fail_op("draft: ruled face failed");
      sew.Add(rf);
      ++n_sheets;
    }
    if (n_sheets == 0)
      fail_invalid("a drafted face has no parting edge");
    sew.Perform();
    const TopoDS_Shape sheet = sew.SewedShape();
    // The slab on the side the ruled surface goes, split by the sheet.
    const double       thick = len * std::abs(sa) * 1.01 + 10 * tol;
    const TopoDS_Shape sl    = slab(f, angle > 0 ? -thick : thick, f, sources);
    if (sl.IsNull())
      fail_op("draft: slab failed");
    BRepAlgoAPI_Splitter sp;
    ShapeList sp_args, sp_tools;
    sp_args.Append(sl);
    sp_tools.Append(sheet);
    sp.SetArguments(sp_args);
    sp.SetTools(sp_tools);
    sp.SetRunParallel(bop_parallel());
    sp.Build(new_cancel_indicator()->Start());
    if (sp.HasErrors())
      fail_op("draft: split failed");
    tool_chain.push_back(sp.History());
    // Pieces touching the face (a face of theirs on its surface).
    for (TopExp_Explorer z(sp.Shape(), TopAbs_SOLID); z.More(); z.Next())
    {
      const TopoDS_Shape w = z.Current();
      bool touches = false;
      for (TopExp_Explorer y(w, TopAbs_FACE); y.More() && !touches; y.Next())
      {
        gp_Pnt yp;
        gp_Dir yn;
        if (!inner_point(TopoDS::Face(y.Current()), yp, yn))
          continue;
        GeomAPI_ProjectPointOnSurf pr(yp, gs);
        touches = pr.IsDone() && pr.NbPoints() > 0 && pr.LowerDistance() < 10 * tol;
        if (touches)
        {
          BRepClass_FaceClassifier fc(f, yp, 10 * tol);
          touches = fc.State() == TopAbs_IN || fc.State() == TopAbs_ON;
        }
      }
      if (!touches || volume_of(w) < tol * tol * tol)
        continue;
      // Its faces on the ruled sheet stand for `f`.
      for (TopExp_Explorer y(w, TopAbs_FACE); y.More(); y.Next())
      {
        gp_Pnt yp;
        gp_Dir yn;
        if (!inner_point(TopoDS::Face(y.Current()), yp, yn))
          continue;
        BRepExtrema_DistShapeShape dd(BRepBuilderAPI_MakeVertex(yp).Vertex(), sheet);
        const bool on_sheet = dd.IsDone() && dd.Value() < 10 * tol;
        sources.Bind(y.Current(), Source{f, on_sheet ? (uint8_t)REL_MODIFIED : (uint8_t)REL_GENERATED});
      }
      (angle > 0 ? removal : addition).Append(w);
    }
  }
  TopoDS_Shape       r = solid.shape;
  std::vector<HHist> chain;
  if (!removal.IsEmpty())
  {
    ShapeList args;
    args.Append(r);
    TopoDS_Shape cu_out;
    HHist        cu_hist;
    if (!quick_boolean(1, args, removal, bop_parallel(), cu_out, cu_hist))
      fail_op("draft: cut failed");
    r = cu_out;
    chain.push_back(cu_hist);
  }
  if (!addition.IsEmpty())
  {
    ShapeList args;
    args.Append(r);
    TopoDS_Shape fu_out;
    HHist        fu_hist;
    if (!quick_boolean(0, args, addition, bop_parallel(), fu_out, fu_hist))
      fail_op("draft: fuse failed");
    r = fu_out;
    chain.push_back(fu_hist);
  }
  ShapeUpgrade_UnifySameDomain usd(r, true, true, false);
  usd.SetLinearTolerance(1e-7);
  usd.Build();
  chain.push_back(usd.History());
  r = single_solid(usd.Shape());
  if (!valid_solid(r))
    fail_op("draft about a curved parting line gave an invalid solid");
  HHist h = shell_history(solid.shape, r, chain, chain, sources, {});
  return make_result({solid.shape}, r, h.get());
}



namespace {
// A loft face: s in [0, 1] along its profiles (the way the faces go round), w along the loft (0 on the first
// profile, 1 on the last; past them the end spans continued). Two profiles: cubic x linear, ruled; more: cubic x
// cubic.
struct RuledFace
{
  TopoDS_Face                      face;
  occ::handle<Geom_BSplineSurface> s;
  bool                             ruling_in_v = true;
  double                           p0 = 0, p1 = 1; // profile-direction parameters
  double                           r0 = 0, r1 = 1; // ruling-direction parameters (r0 on the first profile)
  bool                             flip_s = false;
  double                           out = 1; // makes the normal outward
};

void ruled_d1(const RuledFace& f, double s, double w, gp_Pnt& P, gp_Vec& Ds, gp_Vec& Dw)
{
  const double ss  = f.flip_s ? 1 - s : s;
  const double prm = f.p0 + (f.p1 - f.p0) * ss;
  const double r   = f.r0 + (f.r1 - f.r0) * w;
  // (Past the face's ends: the end spans' polynomials continued. Along the end tangents instead, live (a) came
  // out 0.046 mm³ over, against 0.021 under: Parasolid continues them between the two.)
  gp_Vec Dp, Dr;
  if (f.ruling_in_v)
    f.s->D1(prm, r, P, Dp, Dr);
  else
    f.s->D1(r, prm, P, Dr, Dp);
  Ds = Dp * ((f.p1 - f.p0) * (f.flip_s ? -1.0 : 1.0));
  Dw = Dr * (f.r1 - f.r0);
}

gp_Pnt ruled_point(const RuledFace& f, double s, double w)
{
  gp_Pnt P;
  gp_Vec Ds, Dw;
  ruled_d1(f, s, w, P, Ds, Dw);
  return P;
}

bool ruled_normal(const RuledFace& f, double s, double w, gp_Vec& n)
{
  gp_Pnt P;
  gp_Vec Ds, Dw;
  ruled_d1(f, s, w, P, Ds, Dw);
  n = Ds.Crossed(Dw);
  if (n.Magnitude() < 1e-12)
    return false;
  n.Normalize();
  n *= f.out;
  return true;
}
} // namespace

// A thin loft's wall grown `t` outward along the loft sheet's normals, the faces' offsets mitred where they crease
// and trimmed to the two profile planes (Onshape's Thin loft with Trim ends, live "skasm (2026-10-31, P2
// feature-sweep Loft 8)" (a): 10 faces, the outer faces meeting along their intersections; the profiles' in-plane
// offsets lofted were 0.15 % over). Built directly (OCCT's Booleans of the offset faces took 6 s): each ruled loft
// face's offset, continued past its rulings in its tangent plane to the mitre with its neighbour's (exact for
// planes), as a B-spline through rows of its points past both planes (neighbours share their mitre columns), cut by
// the planes; the planar rings and the loft's faces close it. Without Trim ends (live (b): 16 faces) the offsets
// end at the profiles' rulings and each end is a cap ruled from the profile edge to its offset row (the wall's end
// normal to it, the top past its plane). Two-profile ruled lofts whose faces turn outward about the profiles' axis
// only; an error otherwise.
OpOut op_thicken_loft_out(const OcShape& sheet_in, double t_in, double t_inner_in, const PlaneF& p0, const PlaneF& p1, bool trim_ends)
{
  require_non_null(sheet_in.shape, "sheet");
  if (!(t_in >= 0 && t_inner_in >= 0 && t_in + t_inner_in > 0))
    fail_invalid("thicken loft: the wall needs a thickness");
  if (t_inner_in > 0 && trim_ends)
    fail_invalid("thicken loft: an inner offset only without Trim ends");
  const bool dbg    = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       t_last = std::chrono::steady_clock::now();
  auto       lap    = [&](const char* what) {
    const auto now = std::chrono::steady_clock::now();
    if (dbg)
      std::fprintf(stderr, "thicken loft: %s %.1f ms\n", what, std::chrono::duration<double, std::milli>(now - t_last).count());
    t_last = now;
  };
  // At the working scale (mm), as the other shell constructions.
  const double k = working_scale();
  ScaledInputs sc(k);
  const TopoDS_Shape sheet = sc.scale(sheet_in.shape);
  const double       t     = t_in * k, t_inner = t_inner_in * k;
  gp_Pln             pl0 = to_pln(p0), pl1 = to_pln(p1);
  pl0.SetLocation(gp_Pnt(pl0.Location().XYZ() * k));
  pl1.SetLocation(gp_Pnt(pl1.Location().XYZ() * k));
  const double tol = 1e-6;
  // The sheet's free edges (its profiles) and their middles: the axis between them.
  ShapeList free_edges;
  {
    ShapeAncMap ef;
    TopExp::MapShapesAndAncestors(sheet, TopAbs_EDGE, TopAbs_FACE, ef);
    for (int i = 1; i <= ef.Extent(); ++i)
      if (ef(i).Extent() == 1)
        free_edges.Append(ef.FindKey(i));
  }
  auto middle = [&](const gp_Pln& pl) {
    gp_XYZ sum(0, 0, 0);
    int    n = 0;
    for (const TopoDS_Shape& e : free_edges)
    {
      BRepAdaptor_Curve c(TopoDS::Edge(e));
      for (int i = 0; i < 16; ++i)
      {
        const gp_Pnt p = c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * (i + 0.5) / 16);
        if (pl.Distance(p) < 1e-5)
        {
          sum += p.XYZ();
          ++n;
        }
      }
    }
    return n > 0 ? gp_Pnt(sum / n) : pl.Location();
  };
  const gp_Pnt c0 = middle(pl0), c1 = middle(pl1);
  if (c0.Distance(c1) < 1e-6)
    fail_invalid("thicken loft: the profiles' planes coincide");
  const gp_Lin axis(c0, gp_Dir(gp_Vec(c0, c1)));
  // The faces, ruled, with their first profile on pl0.
  std::vector<RuledFace> fs;
  for (TopExp_Explorer x(sheet, TopAbs_FACE); x.More(); x.Next())
  {
    RuledFace f;
    f.face = TopoDS::Face(x.Current());
    f.s    = occ::down_cast<Geom_BSplineSurface>(BRep_Tool::Surface(f.face));
    if (f.s.IsNull() || !BRep_Tool::Surface(f.face)->IsKind(STANDARD_TYPE(Geom_BSplineSurface)))
      fail_op("thicken loft: a face not a ruled B-spline");
    double u0, u1, v0, v1;
    f.s->Bounds(u0, u1, v0, v1);
    // The loft's direction: the one whose ends lie in the two planes (a ruled face's linear one).
    auto spans = [&](bool in_v) {
      for (const double q : {0.25, 0.5, 0.75})
      {
        const gp_Pnt a = in_v ? f.s->Value(u0 + (u1 - u0) * q, v0) : f.s->Value(u0, v0 + (v1 - v0) * q);
        const gp_Pnt b = in_v ? f.s->Value(u0 + (u1 - u0) * q, v1) : f.s->Value(u1, v0 + (v1 - v0) * q);
        if (!((pl0.Distance(a) < tol && pl1.Distance(b) < tol) || (pl1.Distance(a) < tol && pl0.Distance(b) < tol)))
          return false;
      }
      return true;
    };
    if (spans(true))
    {
      f.ruling_in_v = true;
      f.p0 = u0, f.p1 = u1, f.r0 = v0, f.r1 = v1;
    }
    else if (spans(false))
    {
      f.ruling_in_v = false;
      f.p0 = v0, f.p1 = v1, f.r0 = u0, f.r1 = u1;
    }
    else
      fail_op("thicken loft: a face not from one profile plane to the other");
    // The first profile on pl0.
    const gp_Pnt a = ruled_point(f, 0.5, 0), b = ruled_point(f, 0.5, 1);
    if (pl0.Distance(a) > pl0.Distance(b))
      std::swap(f.r0, f.r1);
    if (pl0.Distance(ruled_point(f, 0.5, 0)) > tol || pl1.Distance(ruled_point(f, 0.5, 1)) > tol)
      fail_op("thicken loft: a face not between the profile planes");
    // Outward: away from the axis.
    gp_Vec n;
    if (!ruled_normal(f, 0.5, 0.5, n))
      fail_op("thicken loft: a degenerate face");
    const gp_Pnt m = ruled_point(f, 0.5, 0.5);
    gp_Vec       away(axis.Location().Translated(gp_Vec(axis.Direction()) * gp_Vec(axis.Location(), m).Dot(gp_Vec(axis.Direction()))), m);
    if (away.Magnitude() < 1e-9)
      fail_op("thicken loft: a face on the axis");
    if (n.Dot(away) < 0)
      f.out = -1;
    fs.push_back(f);
  }
  const int nf = (int)fs.size();
  if (nf == 0)
    fail_op("thicken loft: no faces");
  // Round order: each face's s = 1 ruling is the next one's s = 0.
  auto ruling = [&](const RuledFace& f, double s) { return std::pair{ruled_point(f, s, 0), ruled_point(f, s, 1)}; };
  auto same   = [&](const std::pair<gp_Pnt, gp_Pnt>& a, const std::pair<gp_Pnt, gp_Pnt>& b) { return a.first.Distance(b.first) < 1e-4 && a.second.Distance(b.second) < 1e-4; };
  std::vector<int>  order{0};
  std::vector<bool> used(nf, false);
  used[0] = true;
  while ((int)order.size() < nf)
  {
    const auto end  = ruling(fs[order.back()], 1);
    int        next = -1;
    for (int j = 0; j < nf && next < 0; ++j)
    {
      if (used[j])
        continue;
      if (same(ruling(fs[j], 0), end))
        next = j;
      else if (same(ruling(fs[j], 1), end))
      {
        fs[j].flip_s = true;
        next         = j;
      }
    }
    if (next < 0)
      fail_op("thicken loft: the faces don't go round");
    used[next] = true;
    order.push_back(next);
  }
  if (!same(ruling(fs[order.back()], 1), ruling(fs[order[0]], 0)))
    fail_op("thicken loft: the faces don't close round");
  // Every face's normals within a half-turn of each other along its profile, outward.
  for (const RuledFace& f : fs)
    for (int i = 0; i <= 8; ++i)
      for (const double w : {0.0, 0.5, 1.0})
      {
        gp_Vec n, n0;
        if (!ruled_normal(f, i / 8.0, w, n) || !ruled_normal(f, 0.5, w, n0) || n.Dot(n0) < 0.05)
          fail_op("thicken loft: a face not convex about the axis");
      }
  // The mitre between face a (its s = 1) and face b (its s = 0) at w: on both tangent planes, t off each.
  auto mitre = [&](const RuledFace& a, const RuledFace& b, double w, gp_Pnt& m) {
    gp_Vec na, nb;
    if (!ruled_normal(a, 1, w, na) || !ruled_normal(b, 0, w, nb))
      return false;
    const double c = na.Dot(nb);
    if (c < -0.9)
      return false;
    // Convex (or tangent) creases only between the profiles: b's side goes back from a's tangent plane. (Past
    // them, cut away by the planes, a tangent end's crease continued may turn a little the other way.)
    gp_Pnt P;
    gp_Vec Ds, Dw;
    ruled_d1(b, 0, w, P, Ds, Dw);
    if (w >= 0 && w <= 1 && Ds.Dot(na) > 1e-6 * Ds.Magnitude())
      return false;
    m = ruled_point(a, 1, w).Translated((na + nb) * (t / (1 + c)));
    return true;
  };
  lap("faces");
  // Rows past both planes: the plane's distance along the rulings, plus a margin.
  // Rows along the rulings, points along each row (by length: dense samples 6 times as many, evenly spaced
  // points keep the iso-parametric interpolation from waving; the fit is checked below).
  const int J = 24, N = 40, K = 6 * N;
  const double margin = 2 * t / c0.Distance(c1) + 0.02;
  const double wlo = trim_ends ? -margin : 0.0, whi = trim_ends ? 1 + margin : 1.0;
  // Where the faces' offsets, continued past their rulings (the profile curves' end spans continued), meet: for
  // face `f`'s row w, its s there and the other face's (s, w) (Newton from the tangent planes' mitre); faces
  // tangent at the crease meet on its offset.
  // The offset being built: `d` along the normals (the inside's, Thickness 2: negative; the creases concave
  // there, the offsets meet before them).
  double d         = t;
  auto   offset_pt = [&](const RuledFace& f, double s, double w) {
    gp_Vec n;
    ruled_normal(f, s, w, n);
    return ruled_point(f, s, w).Translated(n * d);
  };
  // `f_end`: f's side at the crease (1: its s = 1 end), the other face's the opposite.
  auto meet = [&](const RuledFace& f, double f_end, const RuledFace& o, double w, double& sf) {
    gp_Vec nf, no;
    if (!ruled_normal(f, f_end, w, nf) || !ruled_normal(o, 1 - f_end, w, no))
      return false;
    const double c = nf.Dot(no);
    if (c > 1 - 1e-12)
    {
      sf = f_end;
      return true;
    }
    if (c < -0.9)
      return false;
    // Start: the tangent planes' mitre, its distance along each face's profile direction.
    const gp_Pnt Q  = ruled_point(f, f_end, w);
    const gp_Pnt M0 = Q.Translated((nf + no) * (d / (1 + c)));
    gp_Pnt       P;
    gp_Vec       Dsf, Dwf, Dso, Dwo;
    ruled_d1(f, f_end, w, P, Dsf, Dwf);
    ruled_d1(o, 1 - f_end, w, P, Dso, Dwo);
    double x[3] = {f_end + gp_Vec(Q, M0).Dot(Dsf) / Dsf.SquareMagnitude(), (1 - f_end) + gp_Vec(Q, M0).Dot(Dso) / Dso.SquareMagnitude(), w};
    auto   G    = [&](const double* y) { return gp_Vec(offset_pt(o, y[1], y[2]), offset_pt(f, y[0], w)); };
    for (int it = 0; it < 50; ++it)
    {
      const gp_Vec g = G(x);
      if (g.Magnitude() < 1e-10)
      {
        sf = x[0];
        return true;
      }
      gp_Mat J;
      for (int c2 = 0; c2 < 3; ++c2)
      {
        double       a[3] = {x[0], x[1], x[2]}, b[3] = {x[0], x[1], x[2]};
        const double h    = 1e-6;
        a[c2] += h;
        b[c2] -= h;
        const gp_Vec d = (G(a) - G(b)) / (2 * h);
        J.SetCol(c2 + 1, d.XYZ());
      }
      if (std::abs(J.Determinant()) < 1e-300)
        return false;
      const gp_XYZ dx = J.Inverted() * g.XYZ();
      x[0] -= dx.X();
      x[1] -= dx.Y();
      x[2] -= dx.Z();
    }
    return false;
  };
  // Face by face (in `order`), the offset `dist` between its mitres with its neighbours'.
  auto offsets = [&](double dist) {
    d = dist;
    std::vector<std::vector<double>> s_lo(nf, std::vector<double>(J + 1)), s_hi(nf, std::vector<double>(J + 1));
    for (int i = 0; i < nf; ++i)
      for (int j = 0; j <= J; ++j)
      {
        const double w = wlo + (whi - wlo) * j / J;
        gp_Pnt       m;
        if (w >= 0 && w <= 1 && !mitre(fs[order[i]], fs[order[(i + 1) % nf]], w, m))
          fail_op("thicken loft: a crease not convex");
        if (!meet(fs[order[i]], 1, fs[order[(i + 1) % nf]], w, s_hi[i][j]) || !meet(fs[order[i]], 0, fs[order[(i + nf - 1) % nf]], w, s_lo[i][j]))
          fail_op("thicken loft: the offsets' mitre not found");
      }
    std::vector<TopoDS_Face> faces;
    for (int i = 0; i < nf; ++i)
    {
      const RuledFace&           f = fs[order[i]];
      NCollection_Array2<gp_Pnt> grid(1, N + 1, 1, J + 1);
      for (int j = 0; j <= J; ++j)
      {
        const double w = wlo + (whi - wlo) * j / J;
        // The row: the face's offset from one mitre to the other, by length along it.
        const double        a = s_lo[i][j], b = s_hi[i][j];
        std::vector<gp_Pnt> dense;
        for (int q = 0; q <= K; ++q)
          dense.push_back(offset_pt(f, a + (b - a) * q / K, w));
        std::vector<double> len(dense.size(), 0.0);
        for (size_t q = 1; q < dense.size(); ++q)
          len[q] = len[q - 1] + dense[q - 1].Distance(dense[q]);
        for (int c = 0; c <= N; ++c)
        {
          const double L = len.back() * c / N;
          size_t       q = 1;
          while (q + 1 < len.size() && len[q] < L)
            ++q;
          const double u = len[q] - len[q - 1] > 0 ? (L - len[q - 1]) / (len[q] - len[q - 1]) : 0.0;
          grid(c + 1, j + 1) = offset_pt(f, a + (b - a) * (q - 1 + u) / K, w);
        }
      }
      GeomAPI_PointsToBSplineSurface fit;
      fit.Interpolate(grid, Approx_IsoParametric);
      if (!fit.IsDone())
        fail_op("thicken loft: an outer face not fitted");
      {
        // The fit against exact offset points half-way between the data rows (every third), along each.
        double                     worst = 0;
        double                     fu0, fu1, fv0, fv1;
        fit.Surface()->Bounds(fu0, fu1, fv0, fv1);
        // (A local search from the point's place in the iso-parametric grid: a global projector's set-up cost
        // 100 ms a face.)
        const GeomAdaptor_Surface ad(fit.Surface());
        Extrema_GenLocateExtPS     loc(ad);
        for (int j = 1; j < J; j += 3)
        {
          const double w = wlo + (whi - wlo) * (j + 0.5) / J;
          const double a = 0.5 * (s_lo[i][j] + s_lo[i][j + 1]), b = 0.5 * (s_hi[i][j] + s_hi[i][j + 1]);
          for (int q = 1; q < 8; ++q)
          {
            loc.Perform(offset_pt(f, a + (b - a) * q / 8.0, w), fu0 + (fu1 - fu0) * q / 8.0, fv0 + (fv1 - fv0) * (j + 0.5) / J);
            worst = std::max(worst, loc.IsDone() ? std::sqrt(loc.SquareDistance()) : 1.0);
          }
        }
        if (dbg)
          std::fprintf(stderr, "thicken loft: face %d fit within %g\n", i, worst);
        if (worst > 1e-5)
          fail_op("thicken loft: an outer face not fitted closely");
      }
      faces.push_back(BRepBuilderAPI_MakeFace(fit.Surface(), tol).Face());
    }
    return faces;
  };
  const std::vector<TopoDS_Face> outer = t > 0 ? offsets(t) : std::vector<TopoDS_Face>();
  const std::vector<TopoDS_Face> inner = t_inner > 0 ? offsets(-t_inner) : std::vector<TopoDS_Face>();
  lap("outer faces");
  if (!trim_ends)
  {
    // The ends: per face and profile, ruled from the inside's row there (the profile edge, or the inner offset's
    // row) to the outside's (the outer offset's row, or the profile edge when the wall is all inside: flipped),
    // their starts matched; neighbouring caps meet on the line between the two sides' mitre points.
    BRepBuilderAPI_Sewing sew(1e-5);
    for (int i = 0; i < nf; ++i)
    {
      const RuledFace& f = fs[order[i]];
      sew.Add(inner.empty() ? f.face : inner[(size_t)i]);
      sew.Add(outer.empty() ? f.face : outer[(size_t)i]);
      // Side `faces`' row at the first (`first`) or last profile.
      auto end_row = [&](const std::vector<TopoDS_Face>& faces, bool first) -> occ::handle<Geom_Curve> {
        if (faces.empty())
        {
          // The profile edge: the face's iso at the profile's ruling parameter (the edges' own curves carry the
          // working scale in their location, which OCCT applies to the curve but not to its parameters).
          const double r = first ? f.r0 : f.r1;
          return new Geom_TrimmedCurve(f.ruling_in_v ? f.s->VIso(r) : f.s->UIso(r), f.p0, f.p1);
        }
        const occ::handle<Geom_Surface> os = BRep_Tool::Surface(faces[(size_t)i]);
        double                          u0, u1, v0, v1;
        os->Bounds(u0, u1, v0, v1);
        return new Geom_TrimmedCurve(os->VIso(first ? v0 : v1), u0, u1);
      };
      for (const bool first : {true, false})
      {
        occ::handle<Geom_Curve> in = end_row(inner, first), out = end_row(outer, first);
        const gp_Pnt            o0 = out->Value(out->FirstParameter());
        if (in->Value(in->FirstParameter()).Distance(o0) > in->Value(in->LastParameter()).Distance(o0))
          in = in->Reversed();
        const TopoDS_Face cap = BRepFill::Face(BRepBuilderAPI_MakeEdge(in).Edge(), BRepBuilderAPI_MakeEdge(out).Edge());
        if (cap.IsNull())
          fail_op("thicken loft: an end cap failed");
        sew.Add(cap);
      }
    }
    sew.Perform();
    lap("sewn");
    TopoDS_Shell shell;
    int          shells = 0;
    for (TopExp_Explorer x(sew.SewedShape(), TopAbs_SHELL); x.More(); x.Next(), ++shells)
      shell = TopoDS::Shell(x.Current());
    if (dbg)
      std::fprintf(stderr, "thicken loft: %d shells, closed %d, free edges %d\n", shells, shells == 1 && BRep_Tool::IsClosed(shell), sew.NbFreeEdges());
    if (shells != 1 || !BRep_Tool::IsClosed(shell))
      fail_op("thicken loft: the wall not closed");
    BRepBuilderAPI_MakeSolid ms(shell);
    TopoDS_Solid             r = TopoDS::Solid(ms.Shape());
    BRepLib::OrientClosedSolid(r);
    if (dbg)
    {
      BRepCheck_Analyzer an(r);
      std::fprintf(stderr, "thicken loft: valid %d, volume %g\n", an.IsValid(), volume_of(r));
      for (TopExp_Explorer x(r, TopAbs_FACE); x.More(); x.Next())
      {
        GProp_GProps g;
        BRepGProp::SurfaceProperties(x.Current(), g);
        Bnd_Box bb;
        BRepBndLib::AddOptimal(x.Current(), bb, false, false);
        double x0, y0, z0, x1, y1, z1;
        bb.Get(x0, y0, z0, x1, y1, z1);
        std::fprintf(stderr, "  face area %.7f box (%.6f %.6f %.6f)-(%.6f %.6f %.6f)\n", g.Mass(), x0, y0, z0, x1, y1, z1);
      }
    }
    if (!BRepCheck_Analyzer(r).IsValid())
      fail_op("thicken loft: the wall came out invalid");
    lap("solid");
    HHist h;
    return make_result({sheet_in.shape}, sc.unscale(r, h), nullptr);
  }
  // The outer faces cut by the planes: the pieces between.
  BRepBuilderAPI_Sewing tube_sew(1e-5);
  for (const TopoDS_Face& f : outer)
    tube_sew.Add(f);
  tube_sew.Perform();
  const TopoDS_Shape tube = tube_sew.SewedShape();
  Bnd_Box            box;
  BRepBndLib::Add(tube, box);
  const double       big  = std::sqrt(box.SquareExtent()) * 2;
  const TopoDS_Face  f0   = BRepBuilderAPI_MakeFace(pl0, -big, big, -big, big).Face();
  const TopoDS_Face  f1   = BRepBuilderAPI_MakeFace(pl1, -big, big, -big, big).Face();
  BRepAlgoAPI_Splitter sp;
  ShapeList          args, tools;
  args.Append(tube);
  tools.Append(f0);
  tools.Append(f1);
  sp.SetArguments(args);
  sp.SetTools(tools);
  sp.Build();
  if (sp.HasErrors())
    fail_op("thicken loft: the outer faces not cut");
  const gp_Vec up0(gp_Vec(c0, c1)), up1(gp_Vec(c1, c0));
  auto         between = [&](const gp_Pnt& p) { return gp_Vec(c0, p).Dot(up0) > 0 && gp_Vec(c1, p).Dot(up1) > 0; };
  BRepBuilderAPI_Sewing     sew(1e-5);
  std::vector<TopoDS_Shape> kept;
  for (TopExp_Explorer x(sp.Shape(), TopAbs_FACE); x.More(); x.Next())
  {
    gp_Pnt p;
    gp_Dir n;
    if (inner_point(TopoDS::Face(x.Current()), p, n) && between(p))
    {
      sew.Add(x.Current());
      kept.push_back(x.Current());
    }
  }
  if ((int)kept.size() != nf)
    fail_op("thicken loft: the outer faces cut wrong");
  lap("cut by the planes");
  // The rings: each plane's section of the outer faces less the profile there.
  for (const gp_Pln& pl : {pl0, pl1})
  {
    // The pieces' edges on the plane (shared with them), round the profile's.
    occ::handle<NCollection_HSequence<TopoDS_Shape>> es = new NCollection_HSequence<TopoDS_Shape>(), ws, ps = new NCollection_HSequence<TopoDS_Shape>(), pws;
    for (const TopoDS_Shape& piece : kept)
      for (TopExp_Explorer x(piece, TopAbs_EDGE); x.More(); x.Next())
      {
        BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
        const double      a = c.FirstParameter(), b = c.LastParameter();
        if (pl.Distance(c.Value(a)) < 1e-5 && pl.Distance(c.Value(0.5 * (a + b))) < 1e-5 && pl.Distance(c.Value(b)) < 1e-5)
        {
          es->Append(x.Current());
          if (dbg && c.GetType() == GeomAbs_BSplineCurve)
            std::fprintf(stderr, "thicken loft: a ring edge of %d poles\n", c.NbPoles());
        }
      }
    ShapeAnalysis_FreeBounds::ConnectEdgesToWires(es, 1e-5, true, ws);
    for (const TopoDS_Shape& e : free_edges)
    {
      BRepAdaptor_Curve c(TopoDS::Edge(e));
      if (pl.Distance(c.Value(0.5 * (c.FirstParameter() + c.LastParameter()))) < tol)
        ps->Append(e);
    }
    ShapeAnalysis_FreeBounds::ConnectEdgesToWires(ps, 1e-5, true, pws);
    if (ws->Length() != 1 || pws->Length() != 1)
      fail_op("thicken loft: a plane's ring not one loop round one profile");
    BRepBuilderAPI_MakeFace mf(pl, TopoDS::Wire(ws->Value(1)), true);
    mf.Add(TopoDS::Wire(pws->Value(1)));
    if (!mf.IsDone())
      fail_op("thicken loft: a ring failed");
    ShapeFix_Face fix(mf.Face());
    fix.FixOrientation();
    fix.Perform();
    sew.Add(fix.Face());
    lap("ring");
  }
  for (const RuledFace& f : fs)
    sew.Add(f.face);
  sew.Perform();
  lap("sewn");
  TopoDS_Shell shell;
  int          shells = 0;
  for (TopExp_Explorer x(sew.SewedShape(), TopAbs_SHELL); x.More(); x.Next(), ++shells)
    shell = TopoDS::Shell(x.Current());
  if (shells != 1)
    fail_op("thicken loft: the wall not closed");
  BRepBuilderAPI_MakeSolid ms(shell);
  TopoDS_Solid             r = TopoDS::Solid(ms.Shape());
  BRepLib::OrientClosedSolid(r);
  lap("oriented");
  const bool valid = BRepCheck_Analyzer(r).IsValid();
  lap("checked");
  if (!valid)
    fail_op("thicken loft: the wall came out invalid");
  lap("solid");
  HHist h;
  return make_result({sheet_in.shape}, sc.unscale(r, h), nullptr);
}

} // namespace noshape
