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
#include <ShapeConstruct_ProjectCurveOnSurface.hxx>
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
#include <BRepAdaptor_Curve2d.hxx>
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
#include <BRepCheck_Result.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <limits>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <ElCLib.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepGProp.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepLib.hxx>
#include <BRepOffset_MakeSimpleOffset.hxx>
#include <BRepTools_Modifier.hxx>
#include <ShapeCustom_ConvertToBSpline.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <gp_Torus.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <Geom_BoundedSurface.hxx>
#include <Geom2d_Line.hxx>
#include <GeomLib.hxx>
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
#include <GeomAPI_ExtremaCurveCurve.hxx>
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
#include <GC_MakeCircle.hxx>
#include <Geom_OffsetSurface.hxx>
#include <GeomAPI_PointsToBSpline.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_SphericalSurface.hxx>
#include <functional>
#include <GeomProjLib.hxx>
#include <BOPTools_AlgoTools3D.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom2dConvert.hxx>
#include <GeomConvert.hxx>
#include <GeomConvert_CompCurveToBSplineCurve.hxx>
#include <ShapeAnalysis.hxx>
#include <memory>
#include <typeinfo>
#include <tuple>
#include <BRepTools_ReShape.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2dAPI_InterCurveCurve.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <TColgp_HArray1OfPnt2d.hxx>
#include <TColStd_HArray1OfReal.hxx>
#include <Geom_Plane.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BOPDS_IteratorSI.hxx>
#include <IntTools_Context.hxx>
#include <BOPAlgo_CheckerSI.hxx>
#include <BOPDS_DS.hxx>
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

// A volume to 1e-3 (OCCT's adaptive integral at that tolerance), for sanity checks: a hose's walls in 1.6 ms, where
// OCCT's plain integral took 51 and volume_of's segmented one ~90.
double rough_volume_of(const TopoDS_Shape& s)
{
  GProp_GProps g;
  BRepGProp::VolumeProperties(s, g, 1e-3, true);
  return g.Mass();
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
      const TopoDS_Face&  f = TopoDS::Face(e.Current());
      BRepAdaptor_Surface fs(f);
      // (Only a face on a removed face's kind of surface can lie on it: no area integral of the others, a hose's
      // B-spline walls 29 ms.)
      if (std::none_of(removed.begin(), removed.end(), [&](const TopoDS_Shape& r) { return BRepAdaptor_Surface(TopoDS::Face(r), false).GetType() == fs.GetType(); }))
        continue;
      GProp_GProps g;
      BRepGProp::SurfaceProperties(f, g);
      gp_Pnt c = g.CentreOfMass();
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
  auto t_lap = std::chrono::steady_clock::now();
  auto lap   = [&](const char* what) {
    if (!dbg)
      return;
    const auto now = std::chrono::steady_clock::now();
    std::fprintf(stderr, "offset: %s %.0f ms\n", what, std::chrono::duration<double, std::milli>(now - t_lap).count());
    t_lap = now;
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
  lap("offset");
  const TopoDS_Shape& outer = sign < 0 ? solid : offset;
  const TopoDS_Shape& inner = sign < 0 ? offset : solid;
  BRepAlgoAPI_Cut     cut;
  build_stoppable(cut, outer, inner);
  if (cut.HasErrors() || !cut.IsDone())
    return no("cut failed");
  TopoDS_Shape r = cut.Shape();
  lap("cut");
  if (!valid_solid(r))
    return no("invalid result");
  lap("valid");
  // A correct offset lies inside the outer solid: the volumes must add up
  // (catches self-intersecting offsets that BRepCheck accepts).
  // (The inner first: an offset that came out empty fails without the others.)
  const double vi = volume_of(inner);
  if (!(vi > 0))
    return no("the offset is empty");
  const double vo = volume_of(outer), vr = volume_of(r);
  lap("volumes");
  if (std::getenv("NOSHAPE_SHELL_DEBUG"))
    std::fprintf(stderr, "offset: outer %.9g inner %.9g result %.9g mm3\n", vo * 1e9, vi * 1e9, vr * 1e9);
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
  lap("kept faces");
  // Offsets extended past a face's boundary come out as separate faces on
  // the same surface; merge them so the topology (and the names) match the
  // slab construction's.
  ShapeUpgrade_UnifySameDomain usd(r, true, true, false);
  usd.SetLinearTolerance(1e-7);
  usd.Build();
  HHist hc = cut.History(), hs = usd.History();
  r = single_solid(usd.Shape());
  lap("unify");
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
// Whether no removed face opens onto anything: inward, every sampled point of every removed face has the part's other
// side less than a wall's thickness behind it (live 1794: the bottom face of a 0.1 mm slab, Shell 0.5 to 3 mm inward
// all fail; Parasolid's shell has no cavity to open there).
// Whether a removed face has material a wall's depth `t` behind it somewhere (a 7 x 7 sample of the face).
bool face_has_depth(BRepClass3d_SolidClassifier& in, const TopoDS_Face& f, double t, double tol)
{
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  BRepAdaptor_Surface sa(f);
  const int           n = 7;
  for (int a = 0; a < n; ++a)
    for (int b = 0; b < n; ++b)
    {
      const gp_Pnt2d uv(u0 + (u1 - u0) * (a + 0.5) / n, v0 + (v1 - v0) * (b + 0.5) / n);
      if (BRepClass_FaceClassifier(f, uv, tol).State() != TopAbs_IN)
        continue;
      gp_Pnt p;
      gp_Vec du, dv;
      sa.D1(uv.X(), uv.Y(), p, du, dv);
      gp_Vec nrm = du.Crossed(dv);
      if (nrm.Magnitude() < 1e-12)
        continue;
      nrm.Normalize();
      if (f.Orientation() == TopAbs_REVERSED)
        nrm.Reverse();
      // Material all the way to a wall's depth behind it.
      bool deep = true;
      for (int k = 1; k <= 4 && deep; ++k)
      {
        in.Perform(p.Translated(nrm * (-t * k / 4.0)), tol);
        deep = in.State() == TopAbs_IN;
      }
      if (deep)
        return true;
    }
  return false;
}

// The removed faces with nothing a wall's depth behind them. Parasolid keeps such a face as a face when another
// removed face opens (live shell-hang Shell 5: its first removed face stays as it was) and fails when none opens
// (live 1794).
std::vector<bool> shallow_openings(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t)
{
  BRepClass3d_SolidClassifier in(solid);
  Bnd_Box                     box;
  BRepBndLib::Add(solid, box);
  const double      tol = 1e-7 * std::sqrt(box.SquareExtent());
  std::vector<bool> shallow;
  for (const TopoDS_Shape& rf : removed)
    shallow.push_back(!face_has_depth(in, TopoDS::Face(rf), t, tol));
  return shallow;
}

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

// A kept planar face lower than the wall: square to a kept planar neighbour across one of its edges and all of it
// nearer that neighbour's plane than `t` (Shell 4's 0.1 mm skirt under its front rounds). OCCT's offset can't drop it
// (155 ms for an empty offset there); the quadric path makes it vanish.
bool low_strip(const TopoDS_Shape& solid, const ShapeIMap& removed, double t)
{
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_EDGE, TopAbs_FACE, ef);
  auto plane_of = [](const TopoDS_Face& f, gp_Pln& p) {
    BRepAdaptor_Surface s(f, false);
    if (s.GetType() != GeomAbs_Plane)
      return false;
    p = s.Plane();
    return true;
  };
  for (TopExp_Explorer fx(solid, TopAbs_FACE); fx.More(); fx.Next())
  {
    const TopoDS_Face& c = TopoDS::Face(fx.Current());
    gp_Pln             pc;
    if (removed.Contains(c) || !plane_of(c, pc))
      continue;
    for (TopExp_Explorer ex(c, TopAbs_EDGE); ex.More(); ex.Next())
    {
      const ShapeList* fs = ef.Seek(ex.Current());
      if (fs == nullptr || fs->Extent() != 2)
        continue;
      const TopoDS_Face b = TopoDS::Face(fs->First().IsSame(c) ? fs->Last() : fs->First());
      gp_Pln            pb;
      if (b.IsSame(c) || removed.Contains(b) || !plane_of(b, pb) || std::abs(pc.Axis().Direction().Dot(pb.Axis().Direction())) > 1e-6)
        continue;
      bool low = true;
      for (TopExp_Explorer ce(c, TopAbs_EDGE); ce.More() && low; ce.Next())
      {
        if (BRep_Tool::Degenerated(TopoDS::Edge(ce.Current())))
          continue;
        BRepAdaptor_Curve cc(TopoDS::Edge(ce.Current()));
        for (int k = 0; k <= 8 && low; ++k)
          low = pb.Distance(cc.Value(cc.FirstParameter() + (cc.LastParameter() - cc.FirstParameter()) * k / 8)) < t * (1 - 1e-9);
      }
      if (low)
        return true;
    }
  }
  return false;
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

// Slab `r` without INTERNAL faces (a prism of a face with an internal edge sweeps it into an internal face, which
// the slab fuse then can't take; the caps keep the edge, as INTERNAL edges noshape's Boolean takes); `r` itself if
// it has none.
TopoDS_Shape without_internal_faces(const TopoDS_Shape& r)
{
  bool any = false;
  for (TopExp_Explorer x(r, TopAbs_FACE); x.More() && !any; x.Next())
    any = x.Current().Orientation() == TopAbs_INTERNAL || x.Current().Orientation() == TopAbs_EXTERNAL;
  if (!any || r.ShapeType() != TopAbs_SOLID)
    return r;
  BRep_Builder bb;
  TopoDS_Solid so;
  bb.MakeSolid(so);
  for (TopoDS_Iterator sh(r); sh.More(); sh.Next())
  {
    if (sh.Value().ShapeType() != TopAbs_SHELL)
      continue;
    TopoDS_Shell out;
    bb.MakeShell(out);
    for (TopoDS_Iterator f(sh.Value()); f.More(); f.Next())
      if (f.Value().Orientation() == TopAbs_FORWARD || f.Value().Orientation() == TopAbs_REVERSED)
        bb.Add(out, f.Value());
    out.Closed(true);
    bb.Add(so, out);
  }
  return BRepCheck_Analyzer(so).IsValid() ? TopoDS_Shape(so) : r;
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
    r = without_internal_faces(mk.Shape());
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
        // (OCCT's simple offset gives its offset face its 1e-4 working tolerance; the face's real one, so the slab
        // fuse needn't take it as tolerant.)
        BRepLib::UpdateTolerances(c, true);
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

// Path 3 (PARITY-SPEED, cf-hose-routing): inward walls of solids whose kept faces all meet tangentially, at least one
// of them free-form, the removed faces planar (a swept hose, its end caps removed: OCCT's offset spent its 30 s
// budget intersecting the pipes' offsets; live 0.1 s). OCCT's simple offset of the kept faces (each face's offset
// surface, no intersections), closed by a rim on each removed face's plane.
bool by_smooth(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t, double sign, Built& out)
{
  const bool dbg = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       no  = [&](const char* why) {
    if (dbg)
      std::fprintf(stderr, "smooth: %s\n", why);
    return false;
  };
  if (sign > 0 || removed.empty())
    return no("outward, or nothing removed");
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
  ShapeIMap rm;
  for (const TopoDS_Shape& f : removed)
  {
    if (BRepAdaptor_Surface(TopoDS::Face(f)).GetType() != GeomAbs_Plane)
      return no("a removed face is not planar");
    rm.Add(f);
  }
  bool freeform = false;
  for (TopExp_Explorer x(so, TopAbs_FACE); x.More(); x.Next())
  {
    if (rm.Contains(x.Current()))
      continue;
    const TopoDS_Face&  f = TopoDS::Face(x.Current());
    BRepAdaptor_Surface s(f);
    const GeomAbs_SurfaceType k = s.GetType();
    freeform |= k == GeomAbs_BSplineSurface || k == GeomAbs_BezierSurface || k == GeomAbs_OffsetSurface || k == GeomAbs_OtherSurface;
    // The offset stays clear of the face's centres of curvature.
    for (int i = 1; i < 8; ++i)
      for (int j = 1; j < 8; ++j)
      {
        const double u = s.FirstUParameter() + (s.LastUParameter() - s.FirstUParameter()) * i / 8;
        const double v = s.FirstVParameter() + (s.LastVParameter() - s.FirstVParameter()) * j / 8;
        BRepLProp_SLProps pr(s, u, v, 2, 1e-9);
        if (pr.IsCurvatureDefined() && std::max(std::abs(pr.MinCurvature()), std::abs(pr.MaxCurvature())) * t > 0.9)
          return no("a face curves tighter than the wall");
      }
  }
  if (!freeform)
    return no("no free-form face");
  // The outward normal of f at the edge e's parameter w (its curve on f: the surface may run on past the face and
  // fold back), stepped into the face where it is undefined (a sweep's end, its v derivative nought there).
  auto normal = [](const TopoDS_Edge& e, const TopoDS_Face& f, double w, gp_Dir& n) {
    const gp_Pnt2d      uv = BRepAdaptor_Curve2d(e, f).Value(w);
    BRepAdaptor_Surface s(f);
    const double        uc = 0.5 * (s.FirstUParameter() + s.LastUParameter()), vc = 0.5 * (s.FirstVParameter() + s.LastVParameter());
    for (double step : {0.0, 1e-6, 1e-4})
    {
      BRepLProp_SLProps lp(s, uv.X() + (uc - uv.X()) * step, uv.Y() + (vc - uv.Y()) * step, 1, 1e-9);
      if (lp.IsNormalDefined())
      {
        n = f.Orientation() == TopAbs_REVERSED ? lp.Normal().Reversed() : lp.Normal();
        return true;
      }
    }
    return false;
  };
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_EDGE, TopAbs_FACE, ef);
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const ShapeList& fl = ef(i);
    if (fl.Extent() != 2)
      continue;
    const bool r1 = rm.Contains(fl.First()), r2 = rm.Contains(fl.Last());
    if (r1 && r2)
      return no("two removed faces meet");
    if (r1 || r2)
      continue;
    const TopoDS_Edge& e = TopoDS::Edge(ef.FindKey(i));
    double             w0, w1;
    BRep_Tool::Range(e, w0, w1);
    for (int k = 0; k <= 4; ++k)
    {
      gp_Dir n1, n2;
      if (!normal(e, TopoDS::Face(fl.First()), w0 + (w1 - w0) * k / 4, n1) || !normal(e, TopoDS::Face(fl.Last()), w0 + (w1 - w0) * k / 4, n2))
        return no("no normal at a kept edge");
      // (Tangent to within what opens a gap under OCCT's confusion across the wall: a sweep's joints, 4e-5 rad.)
      if (n1.Angle(n2) * t > Precision::Confusion() || n1.Angle(n2) > 1e-3)
        return no("kept faces meet at an angle");
    }
  }
  BRep_Builder bld;
  TopoDS_Shell open;
  bld.MakeShell(open);
  for (TopoDS_Iterator it(old_shell, false); it.More(); it.Next())
    if (!rm.Contains(it.Value()))
      bld.Add(open, it.Value());
  // How many kept faces the offset moved in (against their outward normal) and out.
  auto inward = [&](BRepOffset_MakeSimpleOffset& m, int& in, int& out_) {
    in = out_ = 0;
    for (TopoDS_Iterator it(open, false); it.More(); it.Next())
    {
      const TopoDS_Face& f   = TopoDS::Face(it.Value());
      const TopoDS_Shape img = m.Generated(f.Oriented(TopAbs_FORWARD));
      if (img.IsNull() || img.ShapeType() != TopAbs_FACE)
        continue;
      BRepAdaptor_Surface s(f), o(TopoDS::Face(img));
      const double u = 0.5 * (s.FirstUParameter() + s.LastUParameter()), v = 0.5 * (s.FirstVParameter() + s.LastVParameter());
      BRepLProp_SLProps lp(s, u, v, 1, 1e-9);
      if (!lp.IsNormalDefined())
        continue;
      // (Outward in the solid: the face's orientation in its shell, then the shell's in the solid.)
      const bool   flip = (f.Orientation() == TopAbs_REVERSED) != (old_shell.Orientation() == TopAbs_REVERSED);
      const gp_Dir n    = flip ? lp.Normal().Reversed() : lp.Normal();
      (gp_Vec(s.Value(u, v), o.Value(u, v)).Dot(gp_Vec(n)) < 0 ? in : out_) += 1;
    }
  };
  // (The simple offset follows the faces' orientations in the open shell, not the shell's in the solid.)
  BRepOffset_MakeSimpleOffset mk(open, (old_shell.Orientation() == TopAbs_REVERSED ? -sign : sign) * t);
  mk.Perform();
  if (mk.GetError() != BRepOffsetSimple_OK)
    return no("the simple offset failed");
  int n_in = 0, n_out = 0;
  inward(mk, n_in, n_out);
  if (n_in == 0 || n_out > 0)
    return no("the offset is not inward");
  // The offsets as B-splines (an offset surface took BRepMesh 20-30 s a hose, and its boxes seconds).
  occ::handle<ShapeCustom_ConvertToBSpline> conv = new ShapeCustom_ConvertToBSpline();
  conv->SetExtrusionMode(false);
  conv->SetRevolutionMode(false);
  conv->SetOffsetMode(true);
  conv->SetPlaneMode(false);
  BRepTools_Modifier md(mk.GetResultShape(), conv);
  if (!md.IsDone())
    return no("the offsets' conversion failed");
  // x's image as x.Oriented(o) would be (a converted face whose surface runs the other way comes back reversed).
  auto image = [&](const TopoDS_Shape& x, TopAbs_Orientation o) {
    const TopoDS_Shape g = mk.Generated(x.Oriented(TopAbs_FORWARD));
    if (g.IsNull())
      return g;
    const TopoDS_Shape m = md.ModifiedShape(g);
    return m.IsNull() ? m : m.Oriented(m.Orientation() == g.Orientation() ? o : TopAbs::Reverse(o));
  };
  Bnd_Box bb;
  BRepBndLib::Add(so, bb, false);
  const double size = std::sqrt(bb.SquareExtent());
  SourceMap    sources;
  HHist        h = new BRepTools_History();
  TopoDS_Shell shell;
  bld.MakeShell(shell);
  for (TopoDS_Iterator it(old_shell, false); it.More(); it.Next())
  {
    const TopoDS_Face& f  = TopoDS::Face(it.Value());
    const TopoDS_Face  ff = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    if (!rm.Contains(f))
    {
      const TopoDS_Shape img = image(ff, TopAbs::Reverse(f.Orientation()));
      if (img.IsNull() || img.ShapeType() != TopAbs_FACE)
        return no("a face has no offset");
      bld.Add(shell, f);
      const TopoDS_Face inner = TopoDS::Face(img);
      bld.Add(shell, inner);
      sources.Bind(inner, Source{f, (uint8_t)REL_GENERATED});
      continue;
    }
    // Each wire's rim: the wire and its offset (the other way round), on the removed face's plane.
    const gp_Pln    pl = BRepAdaptor_Surface(ff).Plane();
    TopLoc_Location loc;
    const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(ff, loc);
    for (TopoDS_Iterator w(ff, false); w.More(); w.Next())
    {
      TopoDS_Wire ow;
      bld.MakeWire(ow);
      for (TopoDS_Iterator e(w.Value(), false); e.More(); e.Next())
      {
        const TopoDS_Shape img = image(e.Value(), e.Value().Orientation());
        if (img.IsNull() || img.ShapeType() != TopAbs_EDGE)
          return no("a removed face's edge has no offset");
        BRepAdaptor_Curve c(TopoDS::Edge(img));
        for (int k = 0; k <= 4; ++k)
          if (pl.Distance(c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / 4)) > 1e-9 * size + BRep_Tool::Tolerance(TopoDS::Edge(img)))
            return no("an offset edge leaves the removed face's plane");
        bld.Add(ow, img);
      }
      ow.Closed(true);
      TopoDS_Face rim;
      bld.MakeFace(rim, surf, loc, BRep_Tool::Tolerance(ff));
      bld.Add(rim, w.Value());
      bld.Add(rim, ow.Oriented(TopAbs::Reverse(w.Value().Orientation())));
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
  // (A sanity check: the exact integrals were 180 of a hose's 235 ms, OCCT's plain ones 51.)
  const double vw = rough_volume_of(wall), vs = rough_volume_of(so);
  if (dbg)
    std::fprintf(stderr, "smooth: wall volume %.17g solid %.17g\n", vw, vs);
  if (!(vw > 0) || !(vw <= vs * (1 + 1e-3)))
    return no("the wall's volume is off");
  out.result  = wall;
  out.history = shell_history(solid, wall, {h}, {}, sources, removed);
  return true;
}

// Path 2b (SHELL-SLOW step 2, core 11-01): by_planes for solids of planes, cylinders and cones (the removed faces
// planes). Each kept face's offset is its own kind (a cylinder's radius, a cone's reference radius moved by t); each
// vertex's offset where its faces' offsets meet (Newton; two tangent faces: along their offset tangent line; a seam's
// vertex: along the offset seam); each edge's offset the two offsets' intersection through its ends (GeomAPI_IntSS,
// exact for these), tangent faces' edges moved along their normal. The wall as by_planes builds it, then checked:
// valid, free of self-interference (OCCT's checker: the offsets crossing each other or the part), its
// volume inside the part's.
namespace quad {
struct Surf
{
  GeomAbs_SurfaceType       kind;
  occ::handle<Geom_Surface> off;    // the offset (world coordinates)
  gp_Ax3                    ax;     // the offset's frame (cylinders, cones)
  double                    r = 0;  // the offset's radius (cylinder, sphere) / reference radius (cone) / minor radius (torus)
  double                    ang = 0; // a cone's semi-angle / a torus's major radius
  gp_XYZ                    n;      // planes: outward normal, n.x = c
  double                    c = 0;
  // Free-form faces (B-spline, ...): the face's own surface, its offset distance along the natural normal (0 for a
  // removed face) and the foot point of the last point evaluated (the next projection starts there).
  bool                                  general = false;
  occ::handle<Geom_Surface>             base;
  occ::handle<ShapeAnalysis_Surface>    sas;
  double                                d = 0;
  mutable gp_Pnt2d                      hint;
  mutable bool                          has_hint = false;
  // (The nearest foot over the whole surface, its search grid built once: solve3_damped.)
  mutable std::shared_ptr<GeomAPI_ProjectPointOnSurf> nearest;
  double                                              own[4] = {0, 0, 0, 0}; // the face's own parameters' box
};

// Signed distance-like value of x from s's offset (zero on it).
double eval(const Surf& s, const gp_XYZ& x)
{
  if (s.kind == GeomAbs_Plane)
    return s.n.Dot(x) - s.c;
  if (s.general)
  {
    // The distance from the foot point on the face's surface along its natural normal, less the offset's.
    const gp_Pnt   P(x);
    const gp_Pnt2d uv = s.has_hint ? s.sas->NextValueOfUV(s.hint, P, 1e-12, 1e-9) : s.sas->ValueOfUV(P, 1e-12);
    s.hint            = uv;
    s.has_hint        = true;
    gp_Pnt S;
    gp_Vec Du, Dv;
    s.base->D1(uv.X(), uv.Y(), S, Du, Dv);
    const gp_Vec N = Du.Crossed(Dv);
    const double m = N.Magnitude();
    if (m < 1e-300)
      return 1e300;
    return gp_Vec(S, P).Dot(N) / m - s.d;
  }
  const gp_XYZ w   = x - s.ax.Location().XYZ();
  const double h   = w.Dot(s.ax.Direction().XYZ());
  const double rho = (w - s.ax.Direction().XYZ() * h).Modulus();
  if (s.kind == GeomAbs_Cylinder)
    return rho - s.r;
  if (s.kind == GeomAbs_Sphere)
    return w.Modulus() - s.r;
  if (s.kind == GeomAbs_Torus) // (ang: the major radius)
    return std::sqrt((rho - s.ang) * (rho - s.ang) + h * h) - s.r;
  return (rho - (s.r + h * std::tan(s.ang))) * std::cos(s.ang);
}

// eval's value and its gradient (the offset's normal there, away from it on the positive side).
double eval_grad(const Surf& s, const gp_XYZ& x, gp_XYZ& g)
{
  if (s.kind == GeomAbs_Plane)
  {
    g = s.n;
    return s.n.Dot(x) - s.c;
  }
  if (s.general)
  {
    const gp_Pnt   P(x);
    const gp_Pnt2d uv = s.has_hint ? s.sas->NextValueOfUV(s.hint, P, 1e-12, 1e-9) : s.sas->ValueOfUV(P, 1e-12);
    s.hint            = uv;
    s.has_hint        = true;
    gp_Pnt S;
    gp_Vec Du, Dv;
    s.base->D1(uv.X(), uv.Y(), S, Du, Dv);
    const gp_Vec N = Du.Crossed(Dv);
    const double m = N.Magnitude();
    if (m < 1e-300)
      return 1e300;
    g = N.XYZ() / m;
    return gp_Vec(S, P).Dot(N) / m - s.d;
  }
  const gp_XYZ dir = s.ax.Direction().XYZ();
  const gp_XYZ w   = x - s.ax.Location().XYZ();
  const double h   = w.Dot(dir);
  const gp_XYZ rw  = w - dir * h;
  const double rho = rw.Modulus();
  const gp_XYZ e   = rho > 1e-300 ? rw / rho : gp_XYZ(0, 0, 0);
  if (s.kind == GeomAbs_Cylinder)
  {
    g = e;
    return rho - s.r;
  }
  if (s.kind == GeomAbs_Sphere)
  {
    const double m = w.Modulus();
    g              = m > 1e-300 ? w / m : gp_XYZ(0, 0, 0);
    return m - s.r;
  }
  if (s.kind == GeomAbs_Torus)
  {
    const double m = std::sqrt((rho - s.ang) * (rho - s.ang) + h * h);
    g              = m > 1e-300 ? (e * (rho - s.ang) + dir * h) / m : gp_XYZ(0, 0, 0);
    return m - s.r;
  }
  g = (e - dir * std::tan(s.ang)) * std::cos(s.ang);
  return (rho - (s.r + h * std::tan(s.ang))) * std::cos(s.ang);
}

// solve3 with the surfaces' own gradients: one evaluation of each a step (a march's many points on free-form faces).
bool solve3_grad(const Surf* s[3], gp_XYZ& x, double scale)
{
  for (int it = 0; it < 60; ++it)
  {
    gp_XYZ       g[3];
    const gp_XYZ F(eval_grad(*s[0], x, g[0]), eval_grad(*s[1], x, g[1]), eval_grad(*s[2], x, g[2]));
    if (std::max({std::abs(F.X()), std::abs(F.Y()), std::abs(F.Z())}) < 1e-13 * scale)
      return true;
    gp_Mat J;
    for (int r = 1; r <= 3; ++r)
      J.SetRow(r, g[r - 1]);
    if (std::abs(J.Determinant()) < 1e-12)
      return false;
    x -= J.Inverted() * F;
  }
  return false;
}

// eval_grad by a free-form face's nearest foot, square below the point (elsewhere a projection held at the surface's
// end gives a false sheet of zeros, and the last foot's can jump to another sheet of the offset): 1e300 where there's
// none.
double eval_grad_near(const Surf& q, const gp_XYZ& p, gp_XYZ& g, double scale, bool global = true)
{
  const double e = eval_grad(q, p, g);
  if (!q.general || std::abs(e) > 1e299)
    return e;
  // At foot (u, v): the value and its gradient, where the foot lies square below the point.
  auto at = [&](double u, double v, double& out) {
    gp_Pnt S;
    gp_Vec du, dv;
    q.base->D1(u, v, S, du, dv);
    const gp_Vec d(S, gp_Pnt(p));
    const double m = d.Magnitude();
    if (m > 1e-9 * scale && (std::abs(d.Dot(du)) > 1e-4 * m * du.Magnitude() || std::abs(d.Dot(dv)) > 1e-4 * m * dv.Magnitude()))
      return false;
    const gp_Vec N = du.Crossed(dv);
    if (N.Magnitude() < 1e-300)
      return false;
    g   = N.XYZ() / N.Magnitude();
    out = d.Dot(N) / N.Magnitude() - q.d;
    return true;
  };
  // (The last one's foot where it lies square on the face's own patch; else the nearest square foot over the whole
  // surface: the last one's can jump to another sheet of the offset, or to the surface's end.)
  const double mu = 0.1 * (q.own[1] - q.own[0]), mv = 0.1 * (q.own[3] - q.own[2]);
  auto         own = [&](double u, double v) { return !(q.own[1] > q.own[0]) || (u >= q.own[0] - mu && u <= q.own[1] + mu && v >= q.own[2] - mv && v <= q.own[3] + mv); };
  double       out0 = e;
  if (own(q.hint.X(), q.hint.Y()) && at(q.hint.X(), q.hint.Y(), out0))
    return out0;
  // (A local search from the last foot before the global one.)
  if (q.has_hint)
  {
    const gp_Pnt2d w = q.sas->NextValueOfUV(q.hint, gp_Pnt(p), 1e-9 * scale, 1e-3 * scale);
    if (own(w.X(), w.Y()) && at(w.X(), w.Y(), out0))
    {
      q.hint = w;
      return out0;
    }
  }
  if (!global)
    return 1e300;
  if (!q.nearest)
  {
    double u0, u1, v0, v1;
    q.base->Bounds(u0, u1, v0, v1);
    q.nearest = std::make_shared<GeomAPI_ProjectPointOnSurf>();
    q.nearest->Init(q.base, u0, u1, v0, v1);
  }
  q.nearest->Perform(gp_Pnt(p));
  double out = e;
  // (Of the square feet, the nearest on the face's own patch and a little past it: a high degree's continuation
  // curls back near points by the face's end, c2-rings-loft's loft side by its signet.)
  double best = 1e300, bu = 0, bv = 0;
  for (int k = 1; q.nearest->IsDone() && k <= q.nearest->NbPoints(); ++k)
  {
    double u, v, o2;
    q.nearest->Parameters(k, u, v);
    if (!own(u, v))
      continue;
    if (q.nearest->Distance(k) < best && at(u, v, o2))
      best = q.nearest->Distance(k), bu = u, bv = v;
  }
  if (best < 1e300 && at(bu, bv, out))
  {
    q.hint     = gp_Pnt2d(bu, bv);
    q.has_hint = true;
    return out;
  }
  if (at(q.hint.X(), q.hint.Y(), out))
    return out;
  return 1e300;
}

// solve3_grad damped: each step halved until the largest value falls (a free-form offset's foot can jump between
// steps), to 1e-10 of `scale`.
bool solve3_damped(const Surf* s[3], gp_XYZ& x, double scale)
{
  auto one = [&](const Surf& q, const gp_XYZ& p, gp_XYZ& g) { return eval_grad_near(q, p, g, scale); };
  auto F = [&](const gp_XYZ& p, gp_XYZ* g) {
    gp_XYZ dummy[3];
    gp_XYZ* gg = g ? g : dummy;
    return gp_XYZ(one(*s[0], p, gg[0]), one(*s[1], p, gg[1]), one(*s[2], p, gg[2]));
  };
  auto worst = [](const gp_XYZ& f) { return std::max({std::abs(f.X()), std::abs(f.Y()), std::abs(f.Z())}); };
  for (int it = 0; it < 100; ++it)
  {
    gp_XYZ       g[3];
    const gp_XYZ f = F(x, g);
    const double w = worst(f);
    if (w < 1e-10 * scale)
      return true;
    if (w > 1e299)
      return false;
    gp_Mat J;
    for (int r = 1; r <= 3; ++r)
      J.SetRow(r, g[r - 1]);
    if (std::abs(J.Determinant()) < 1e-12)
      return false;
    const gp_XYZ dx = J.Inverted() * f;
    bool         moved = false;
    for (double h = 1; h >= 1.0 / 1024 && !moved; h /= 2)
    {
      const gp_XYZ y = x - dx * h;
      if (worst(F(y, nullptr)) < w)
        x = y, moved = true;
    }
    if (!moved)
      return false;
  }
  return false;
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

// Gauss-Newton over several offsets (a vertex of more than three faces): their common point, if they have one (every
// residual within 1e-12 of the size).
bool solve_all(const std::vector<const Surf*>& s, gp_XYZ& x, double scale)
{
  for (int it = 0; it < 80; ++it)
  {
    gp_Mat       JtJ(0, 0, 0, 0, 0, 0, 0, 0, 0);
    gp_XYZ       Jtf(0, 0, 0);
    double       worst = 0;
    const double h     = 1e-7 * scale;
    for (const Surf* q : s)
    {
      const double f = eval(*q, x);
      worst          = std::max(worst, std::abs(f));
      gp_XYZ g;
      for (int c = 1; c <= 3; ++c)
      {
        gp_XYZ a = x, b = x;
        a.SetCoord(c, a.Coord(c) + h);
        b.SetCoord(c, b.Coord(c) - h);
        g.SetCoord(c, (eval(*q, a) - eval(*q, b)) / (2 * h));
      }
      for (int r = 1; r <= 3; ++r)
        for (int c = 1; c <= 3; ++c)
          JtJ(r, c) += g.Coord(r) * g.Coord(c);
      Jtf += g * f;
    }
    if (worst < 1e-12 * scale)
      return true;
    if (std::abs(JtJ.Determinant()) < 1e-18)
      return false;
    x -= JtJ.Inverted() * Jtf;
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
// Box-overlapping pairs of a shape's sub-shapes, less those whose two shapes are both in `old`.
class NewPairsIteratorSI : public BOPDS_IteratorSI
{
public:
  NewPairsIteratorSI(const occ::handle<NCollection_BaseAllocator>& a, const ShapeIMap& old) : BOPDS_IteratorSI(a), old_(old) {}

protected:
  void Intersect(const occ::handle<IntTools_Context>& ctx, const bool obb, const double fuzzy) override
  {
    BOPDS_IteratorSI::Intersect(ctx, obb, fuzzy);
    std::vector<char> is_old((size_t)myDS->NbSourceShapes());
    for (int i = 0; i < myDS->NbSourceShapes(); ++i)
      is_old[(size_t)i] = old_.Contains(myDS->Shape(i));
    for (int k = 0; k < myLists.Length(); ++k)
    {
      NCollection_DynamicArray<BOPDS_Pair> kept;
      for (const BOPDS_Pair& p : myLists(k))
      {
        int n1, n2;
        p.Indices(n1, n2);
        if (!is_old[(size_t)n1] || !is_old[(size_t)n2])
          kept.Append(p);
      }
      myLists(k) = std::move(kept);
    }
  }

private:
  const ShapeIMap& old_;
};

// OCCT's self-interference check of a shape, over the pairs where a shape not in `old` takes part.
class NewPairsCheckerSI : public BOPAlgo_CheckerSI
{
public:
  explicit NewPairsCheckerSI(const ShapeIMap& old) : old_(old) {}

protected:
  void Init(const Message_ProgressRange&) override
  {
    Clear();
    myDS = new BOPDS_DS(myAllocator);
    myDS->SetArguments(myArguments);
    myDS->Init(myFuzzyValue);
    myContext   = new IntTools_Context;
    auto* it    = new NewPairsIteratorSI(myAllocator, old_);
    it->SetDS(myDS);
    it->Prepare(myContext, myUseOBB, myFuzzyValue);
    it->UpdateByLevelOfCheck(myLevelOfCheck);
    myIterator = it;
  }

private:
  const ShapeIMap& old_;
};

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
    occ::handle<Geom2d_Curve>        cn;
    // A free-form (offset) surface: projected keeping the edge's own parameter (an intersection curve's pcurves
    // from GeomProjLib ran 1.6 µm off it along a vase's sharp edges: no SameParameter at the wall's tolerance).
    const GeomAbs_SurfaceType kind = GeomAdaptor_Surface(sn).GetType();
    // (Also a surface of extrusion or revolution: GeomProjLib's curve on one fell short of the edge's ends,
    // c2-rings-loft's top.)
    if (kind == GeomAbs_BSplineSurface || kind == GeomAbs_OffsetSurface || kind == GeomAbs_SurfaceOfExtrusion || kind == GeomAbs_SurfaceOfRevolution || kind == GeomAbs_BezierSurface)
    {
      ShapeConstruct_ProjectCurveOnSurface pcs;
      pcs.Init(sn, Precision::Confusion());
      if (!pcs.Perform(c3, a, b, cn))
        cn.Nullify();

      // An offset edge traced on the offset of the original's surface keeps the original's curve there (the same
      // parameters on both): taken where it lies closer to the edge than the projection.
      if (!co.IsNull() && GeomAdaptor_Surface(sn).GetType() == GeomAbs_OffsetSurface && a >= f0 - 1e-9 && b <= f1 + 1e-9)
      {
        auto worst = [&](const occ::handle<Geom2d_Curve>& p2) {
          double w = 0;
          for (int k = 0; k <= 16; ++k)
          {
            const double   s  = a + (b - a) * k / 16;
            const gp_Pnt2d uv = p2->Value(s);
            w                 = std::max(w, sn->Value(uv.X(), uv.Y()).Distance(c3->Value(s)));
          }
          return w;
        };
        // (On a closed surface the original's (a seam's two each on its own side, an edge to the seam's corner on the
        // side it comes to it from): the projection can put a point of the seam on either.)
        // (And wherever the original's lies on it within the tolerance: an offset surface's extension can come back
        // near its other end, where the projection then wanders, c2-rings-loft's loft side.)
        const bool closed = sn->IsUClosed() || sn->IsVClosed();
        if (cn.IsNull() || worst(co) < worst(cn) || worst(co) <= tol || ((seam_edge || closed) && worst(co) <= std::max(tol, 2 * worst(cn))))
          cn = occ::down_cast<Geom2d_Curve>(co->Copy());
      }
    }
    else
      cn = GeomProjLib::Curve2d(c3, a, b, sn);
    if (co.IsNull() || cn.IsNull())
      return nullptr;
    // (On a closed surface an end on its seam can come out on the other side, from the projection or the original
    // itself: c2-rings-loft's loft side. That end's pole moved back by the surface's span.)
    if (const occ::handle<Geom2d_BSplineCurve> bc = occ::down_cast<Geom2d_BSplineCurve>(cn); !bc.IsNull() && bc->NbPoles() >= 3 && (sn->IsUClosed() || sn->IsVClosed()))
    {
      double su0, su1, sv0, sv1;
      sn->Bounds(su0, su1, sv0, sv1);
      const double span_u = sn->IsUClosed() && !sn->IsUPeriodic() ? su1 - su0 : 0, span_v = sn->IsVClosed() && !sn->IsVPeriodic() ? sv1 - sv0 : 0;
      for (const auto [end, next] : {std::pair{1, 2}, std::pair{bc->NbPoles(), bc->NbPoles() - 1}})
      {
        gp_Pnt2d       q  = bc->Pole(end);
        const gp_Pnt2d nq = bc->Pole(next);
        if (span_u > 0 && std::abs(q.X() - nq.X()) > 0.5 * span_u)
          q.SetX(q.X() + (nq.X() > q.X() ? span_u : -span_u));
        if (span_v > 0 && std::abs(q.Y() - nq.Y()) > 0.5 * span_v)
          q.SetY(q.Y() + (nq.Y() > q.Y() ? span_v : -span_v));
        if (!q.IsEqual(bc->Pole(end), 0))
          bc->SetPole(end, q);
      }
    }
    if (per || sn->IsVPeriodic())
    {
      // Original start (in oe's own direction) against the new curve's start, by whole periods in u (and v).
      const gp_Pnt2d qo = co->Value(f0), qn = cn->Value(a);
      const double   du = per ? std::round((qo.X() - qn.X()) / sn->UPeriod()) * sn->UPeriod() : 0.0;
      const double   dv = sn->IsVPeriodic() ? std::round((qo.Y() - qn.Y()) / sn->VPeriod()) * sn->VPeriod() : 0.0;
      if (du != 0 || dv != 0)
        cn->Translate(gp_Vec2d(du, dv));
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

// `bs` continued past its start (or end) in u by `len` (measured along the surface), as its end span's polynomial runs
// on (Parasolid's continuation: live 1799, a flaring vase's outward wall). The clamped end knot moves out by δ and the
// p poles next to it become the end span's blossoms at the new knots (homogeneous for rational surfaces); every other
// pole and knot is kept, so the surface on its own domain is unchanged (checked by the caller). Null: not done.
occ::handle<Geom_BSplineSurface> continued_u(const occ::handle<Geom_BSplineSurface>& bs, double len, bool at_end)
{
  const int p  = bs->UDegree();
  const int nk = bs->NbUKnots();
  const int np = bs->NbUPoles();
  if (nk < 2 || bs->UMultiplicity(at_end ? nk : 1) != p + 1)
    return nullptr;
  double a0, a1, b0, b1;
  bs->Bounds(a0, a1, b0, b1);
  // The end knot e and the span's other end o.
  const double e = at_end ? bs->UKnot(nk) : bs->UKnot(1), o = at_end ? bs->UKnot(nk - 1) : bs->UKnot(2);
  double       speed = 0;
  for (int k = 0; k <= 8; ++k)
  {
    gp_Pnt P;
    gp_Vec du, dv;
    bs->D1(e, b0 + (b1 - b0) * k / 8, P, du, dv);
    speed = std::max(speed, du.Magnitude());
  }
  if (!(speed > 1e-300))
    return nullptr;
  const double moved = at_end ? e + len / speed : e - len / speed;
  // The end span's Bezier poles: o inserted to multiplicity p in a copy (the span's p + 1 poles at that end).
  auto bez = occ::down_cast<Geom_BSplineSurface>(bs->Copy());
  bez->InsertUKnot(o, p, 1e-12, false);
  const int                         nb   = bez->NbUPoles();
  const NCollection_Array1<double>& flat = bs->UKnotSequence();
  const int                         nf   = flat.Length();
  const int                         nv   = bs->NbVPoles();
  const bool                        rat  = bs->IsURational() || bs->IsVRational();
  NCollection_Array2<gp_Pnt>        poles(1, np, 1, nv);
  NCollection_Array2<double>        weights(1, np, 1, nv);
  bs->Poles(poles);
  if (rat)
    bs->Weights(weights);
  else
    weights.Init(1.0);
  for (int j = 1; j <= nv; ++j)
  {
    // Homogeneous Bezier poles of the end span, from o (k = 0) to e (k = p).
    std::vector<std::array<double, 4>> B(p + 1);
    for (int k = 0; k <= p; ++k)
    {
      const int    idx = at_end ? nb - p + k : p + 1 - k; // (start: e is pole 1, o pole p + 1)
      const gp_Pnt q   = bez->Pole(idx, j);
      const double w   = rat ? bez->Weight(idx, j) : 1.0;
      B[k]             = {q.X() * w, q.Y() * w, q.Z() * w, w};
    }
    // Pole i from the end (i < p): blossom(moved × (p − i), the i knots past the end block, nearest first).
    for (int i = 0; i < p; ++i)
    {
      std::vector<double> args(p - i, moved);
      for (int k = 0; k < i; ++k)
        args.push_back(at_end ? flat(flat.Lower() + nf - p - 2 - k) : flat(flat.Lower() + p + 1 + k));
      std::vector<std::array<double, 4>> c = B;
      for (int r = 1; r <= p; ++r)
      {
        const double t = (args[r - 1] - o) / (e - o);
        for (int k = 0; k + r <= p; ++k)
          for (int d = 0; d < 4; ++d)
            c[k][d] = (1 - t) * c[k][d] + t * c[k + 1][d];
      }
      const double w = c[0][3];
      if (!(w > 0))
        return nullptr;
      const int pi    = at_end ? np - i : i + 1;
      poles(pi, j)   = gp_Pnt(c[0][0] / w, c[0][1] / w, c[0][2] / w);
      weights(pi, j) = w;
    }
  }
  NCollection_Array1<double> uk(1, nk), vk(1, bs->NbVKnots());
  NCollection_Array1<int>    um(1, nk), vm(1, bs->NbVKnots());
  bs->UKnots(uk);
  bs->UMultiplicities(um);
  bs->VKnots(vk);
  bs->VMultiplicities(vm);
  uk(at_end ? nk : 1) = moved;
  try
  {
    return rat ? new Geom_BSplineSurface(poles, weights, uk, vk, um, vm, p, bs->VDegree())
               : new Geom_BSplineSurface(poles, uk, vk, um, vm, p, bs->VDegree());
  }
  catch (const Standard_Failure&)
  {
    return nullptr;
  }
}

// `bs` continued past all four of its sides by `len` (u's ends, then v's with the directions swapped); null where a
// side can't be.
occ::handle<Geom_BSplineSurface> continued(const occ::handle<Geom_BSplineSurface>& bs, double len)
{
  occ::handle<Geom_BSplineSurface> s = occ::down_cast<Geom_BSplineSurface>(bs->Copy());
  for (int side = 0; side < 4 && !s.IsNull(); ++side)
  {
    if (side == 2)
      s->ExchangeUV();
    s = continued_u(s, len, side % 2 == 1);
    if (!s.IsNull() && side == 3)
      s->ExchangeUV();
  }
  return s;
}

bool by_quadrics(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double thickness, double sign, Built& out)
{
  const bool dbg = std::getenv("NOSHAPE_SHELL_DEBUG") != nullptr;
  auto       no  = [&](const char* why) {
    if (dbg)
      std::fprintf(stderr, "quadrics: %s\n", why);
    return false;
  };
  // The offset distance against the outward normal: outward walls are the same construction at -t, the part's faces
  // their inside and the offsets their outside.
  const bool   outward = sign > 0;
  const double t       = outward ? -thickness : thickness;
  auto         t_lap   = std::chrono::steady_clock::now();
  auto         lap     = [&](const char* what) {
    if (!dbg)
      return;
    const auto now = std::chrono::steady_clock::now();
    std::fprintf(stderr, "quadrics: %s %.0f ms\n", what, std::chrono::duration<double, std::milli>(now - t_lap).count());
    t_lap = now;
  };
  TopoDS_Shape so, old_shell;
  int          nso = 0, nsh = 0;
  for (TopExp_Explorer x(solid, TopAbs_SOLID); x.More(); x.Next(), ++nso)
    so = x.Current();
  if (nso != 1)
    return no("not one solid");
  // Its shell (a solid with a void fails before: live refuses it, 1757).
  for (TopoDS_Iterator x(so, false); x.More(); x.Next(), ++nsh)
    if (x.Value().ShapeType() == TopAbs_SHELL)
      old_shell = x.Value();
    else
      return no("a solid holding more than shells");
  if (nsh != 1)
    return no("not one shell");
  ShapeIMap faces, edges, verts, rm;
  TopExp::MapShapes(so, TopAbs_FACE, faces);
  TopExp::MapShapes(so, TopAbs_EDGE, edges);
  TopExp::MapShapes(so, TopAbs_VERTEX, verts);
  for (const TopoDS_Shape& f : removed)
    rm.Add(f);
  if (rm.IsEmpty())
    return no("no face removed");
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
    curved = true;
    if (q.kind == GeomAbs_Sphere || q.kind == GeomAbs_Torus)
    {
      // A sphere's or torus's offset: its radius (the minor one) moved along the natural normal; the frame kept.
      q.ax = q.kind == GeomAbs_Sphere ? s.Sphere().Position() : s.Torus().Position();
      const double r0 = q.kind == GeomAbs_Sphere ? s.Sphere().Radius() : s.Torus().MinorRadius();
      q.ang           = q.kind == GeomAbs_Torus ? s.Torus().MajorRadius() : 0.0;
      if (!kept[(size_t)i])
      {
        q.r   = r0;
        q.off = BRep_Tool::Surface(f);
        continue;
      }
      // The natural normal's way against the radius: at (0, 0), the point moved along it, nearer or farther from
      // the centre (the tube's circle).
      gp_Pnt P;
      gp_Vec Du, Dv;
      s.D1(0.3, 0.3, P, Du, Dv);
      gp_Vec N = Du.Crossed(Dv);
      if (N.Magnitude() < 1e-12)
        return no("a face's normal undefined");
      N.Normalize();
      quad::Surf probe = q;
      probe.r          = r0;
      const double out = quad::eval(probe, P.XYZ() + N.XYZ() * (1e-3 * r0)) > 0 ? 1.0 : -1.0;
      q.r              = r0 + out * d;
      if (q.r <= 10 * tol)
        return no("a sphere's or torus's offset collapses");
      if (q.kind == GeomAbs_Sphere)
        q.off = new Geom_SphericalSurface(q.ax, q.r);
      else
      {
        // (A tube past its axis, a spindle torus: only its outer part is the wall's; the checks below decide.)
        q.off = new Geom_ToroidalSurface(q.ax, q.ang, q.r);
      }
      continue;
    }
    if (q.kind != GeomAbs_Cylinder && q.kind != GeomAbs_Cone)
    {
      // A free-form face: its exact offset (Geom_OffsetSurface), where the face is smooth and nowhere curved
      // tighter than the wall (there the offset would fold).
      q.general = true;
      q.base    = BRep_Tool::Surface(f);
      q.sas     = new ShapeAnalysis_Surface(q.base);
      if (!kept[(size_t)i])
      {
        q.off = q.base;
        continue;
      }
      if (!q.base->IsCNu(1) || !q.base->IsCNv(1))
        return no("a free-form face not smooth");
      double u0, u1, v0, v1;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      q.own[0] = u0, q.own[1] = u1, q.own[2] = v0, q.own[3] = v1;
      // (The face's own surface, for the fold check: an extension's curvature on the bounds is the extension's.)
      const occ::handle<Geom_Surface> own = q.base;
      // A B-spline extended past its bounds (tangentially, its own parameters kept): an outward wall's corner and
      // edges lie past the face where it flares (a vase's foot), the offsets must meet there.
      if (auto bs = occ::down_cast<Geom_BSplineSurface>(q.base); !bs.IsNull() && !bs->IsUPeriodic() && !bs->IsVPeriodic())
      {
        const double                     len = std::min(3 * thickness, 0.5 * size);
        try
        {
          // Its own polynomials run on (live 1799); GeomLib's tangent extension where that can't be done.
          occ::handle<Geom_BoundedSurface> ext = continued(bs, len);
          if (ext.IsNull() || std::getenv("NOSHAPE_SHELL_TANGENT_EXTENSION"))
          {
            ext = occ::down_cast<Geom_BoundedSurface>(bs->Copy());
            for (const bool in_u : {true, false})
              for (const bool after : {true, false})
                GeomLib::ExtendSurfByLength(ext, len, 1, in_u, after);
          }
          bool same = !ext.IsNull();
          for (int a = 0; a <= 4 && same; ++a)
            for (int b = 0; b <= 4 && same; ++b)
            {
              const double pu = u0 + (u1 - u0) * a / 4, pv = v0 + (v1 - v0) * b / 4;
              same = ext->Value(pu, pv).Distance(bs->Value(pu, pv)) <= tol;
            }
          if (same)
          {
            q.base = ext;
            q.sas  = new ShapeAnalysis_Surface(q.base);
          }
          else if (dbg)
            std::fprintf(stderr, "quadrics: a B-spline's extension moved its parameters\n");
        }
        catch (const Standard_Failure&)
        {
        }
      }
      const int n = 12;
      for (int a = 0; a <= n; ++a)
        for (int b = 0; b <= n; ++b)
        {
          GeomLProp_SLProps lp(own, u0 + (u1 - u0) * a / n, v0 + (v1 - v0) * b / n, 2, 1e-12);
          if (!lp.IsNormalDefined())
            return no("a free-form face's normal undefined");
          // (It folds only offset towards a centre of curvature, as far as it: d·k ≥ 1 along the surface's normal; a
          // ramp's edge curling away from the wall stays clear, bop-pairs-gpf-5267's Shell 4.)
          if (lp.IsCurvatureDefined() && std::max(lp.MaxCurvature() * d, lp.MinCurvature() * d) > 0.9)
          {
            if (dbg)
              std::fprintf(stderr, "quadrics: fold at (%d, %d): k %g %g /mm, d %g mm, reversed %d\n", a, b, lp.MaxCurvature() / 1e3, lp.MinCurvature() / 1e3, d * 1e3, (int)(f.Orientation() == TopAbs_REVERSED));
            return no("a free-form face's offset folds");
          }
        }
      q.d   = d;
      q.off = new Geom_OffsetSurface(q.base, d);
      continue;
    }
    if (!kept[(size_t)i])
    {
      // A removed cylinder or cone stays: the rim lies on it.
      q.ax  = q.kind == GeomAbs_Cylinder ? s.Cylinder().Position() : s.Cone().Position();
      q.ang = q.kind == GeomAbs_Cone ? s.Cone().SemiAngle() : 0.0;
      q.r   = q.kind == GeomAbs_Cylinder ? s.Cylinder().Radius() : s.Cone().RefRadius();
      q.off = BRep_Tool::Surface(f);
      continue;
    }
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
  lap("offsets");
  (void)curved; // (planes too: those by_planes declines, e.g. a corner of four faces)
  ShapeAncMap ef, vf, ve;
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_EDGE, TopAbs_FACE, ef);
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_VERTEX, TopAbs_FACE, vf);
  TopExp::MapShapesAndUniqueAncestors(so, TopAbs_VERTEX, TopAbs_EDGE, ve);
  // Edges: their faces (a seam: the same twice), whether tangent there.
  std::vector<std::array<int, 2>> efi(ne + 1);
  std::vector<char>               seam(ne + 1), tangent(ne + 1), degen(ne + 1);
  // A curved seam (a closed loft's or swept face's): its offset is the seam's points moved along the face's normal,
  // as a curve where two faces are tangent (c2-rings-loft's ring).
  std::vector<char> cseam(ne + 1);
  std::vector<double> slack(ne + 1); // a nearly tangent edge: how far its faces' offsets part along it
  // A kept face's line tangent to a removed face: its wall ends there in a flat rim normal to it (below).
  std::vector<char> rimt(ne + 1);
  for (int i = 1; i <= ne; ++i)
  {
    const TopoDS_Edge& e = TopoDS::Edge(edges(i));
    if (BRep_Tool::Degenerated(e))
    {
      // A sphere's pole: a degenerate edge again, at its vertex's offset.
      const ShapeList& fl = ef.FindFromKey(e);
      const int        a  = faces.FindIndex(fl.First());
      if (fl.Extent() != 1 || !kept[(size_t)a] || sf[(size_t)a].kind != GeomAbs_Sphere)
      {
        if (dbg)
        {
          const gp_Pnt q = BRep_Tool::Pnt(TopExp::FirstVertex(e));
          std::fprintf(stderr, "quadrics: degenerate edge on %d face(s), face %d kind %d kept %d at %.3f %.3f %.3f\n", fl.Extent(), a, (int)sf[(size_t)a].kind, (int)kept[(size_t)a], q.X() * 1e3, q.Y() * 1e3, q.Z() * 1e3);
        }
        return no("a degenerate edge not a kept sphere's pole");
      }
      efi[(size_t)i]   = {a, a};
      degen[(size_t)i] = 1;
      continue;
    }
    const ShapeList& fl = ef.FindFromKey(e);
    if (fl.Extent() == 1 && BRep_Tool::IsClosed(e, TopoDS::Face(fl.First())))
    {
      const int a     = faces.FindIndex(fl.First());
      efi[(size_t)i]  = {a, a};
      seam[(size_t)i] = 1;
      if (BRepAdaptor_Curve(e).GetType() != GeomAbs_Line)
      {
        if (!kept[(size_t)a] || collapse[(size_t)a])
          return no("a seam not a generator");
        cseam[(size_t)i] = 1;
      }
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
    // A kept face tangent to a removed one (a removed fillet): Parasolid cuts the kept wall off along the line of
    // tangency with a flat rim normal to it, the removed face left only in the other walls' skins (live 1752:
    // 2175.27 mm³, 16 faces; 2045). Its corners as a tangent edge's, each end split (below).
    if (na.Dot(nb) > 1 - 1e-9 && (!kept[(size_t)a] || !kept[(size_t)b]))
    {
      if (outward || c.GetType() != GeomAbs_Line)
        return no("a kept face tangent to a removed one");
      rimt[(size_t)i] = 1;
    }
    if (na.Dot(nb) > 1 - 1e-9)
    {
      tangent[(size_t)i] = 1;
    }
    else if (kept[(size_t)a] && kept[(size_t)b] && !outward)
    {
      // (Two kept faces nearly tangent all along, their offsets t apart parting by less than the edge's own tolerance
      // (t θ): tangent as far as the offset can tell (c2-rings-loft's loft sides and its cylinder, 0.04-0.09°: their
      // offsets' crossing has no well-defined place).)
      const double et = std::max(BRep_Tool::Tolerance(e), 10 * tol);
      bool         parallel = true;
      for (double w : {0.0, 0.25, 0.5, 0.75, 1.0})
      {
        const gp_Pnt q = c.Value(c.FirstParameter() + w * (c.LastParameter() - c.FirstParameter()));
        parallel       = parallel && normal_at(TopoDS::Face(faces(a)), q).Angle(normal_at(TopoDS::Face(faces(b)), q)) * thickness <= et;
      }
      if (parallel)
        tangent[(size_t)i] = 1, slack[(size_t)i] = et;
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
  // Strips lower than the wall: a planar face square to a planar neighbour b across one sharp base edge, every point
  // of it nearer b than t, its edges across from the base tangent to their faces and its two other edges short ends
  // (Shell 4's 0.1 mm skirt under its front rounds). In the wall's inside it vanishes: the rounds' offsets meet b's,
  // its top edges are edges between them and b, its base and ends go, each end's two corners one.
  std::vector<char>                     strip(nf + 1);
  std::vector<std::vector<TopoDS_Edge>> base_by(ne + 1); // a strip's base edge: its top edges, in order
  std::vector<std::vector<int>>         v_faces(nv + 1), v_edges(nv + 1);
  for (int c = 1; c <= nf; ++c)
  {
    if (!kept[(size_t)c] || collapse[(size_t)c] || sf[(size_t)c].kind != GeomAbs_Plane)
      continue;
    const TopoDS_Face fc = TopoDS::Face(faces(c));
    int               nw = 0;
    for (TopExp_Explorer x(fc, TopAbs_WIRE); x.More(); x.Next())
      ++nw;
    if (nw != 1)
      continue;
    std::vector<TopoDS_Edge> uses;
    for (BRepTools_WireExplorer x(BRepTools::OuterWire(fc), fc); x.More(); x.Next())
      uses.push_back(x.Current());
    // The base: a sharp edge to a kept plane square to c, every vertex of c within t of that plane.
    int base = -1, b = -1;
    for (size_t p = 0; p < uses.size() && base < 0; ++p)
    {
      const int e = edges.FindIndex(uses[p]), o = efi[(size_t)e][0] == c ? efi[(size_t)e][1] : efi[(size_t)e][0];
      if (tangent[(size_t)e] || seam[(size_t)e] || !kept[(size_t)o] || collapse[(size_t)o] || sf[(size_t)o].kind != GeomAbs_Plane
          || std::abs(sf[(size_t)o].n.Dot(sf[(size_t)c].n)) > 1e-9 * sf[(size_t)o].n.Modulus() * sf[(size_t)c].n.Modulus())
        continue;
      const gp_Pln pb = BRepAdaptor_Surface(TopoDS::Face(faces(o))).Plane();
      bool         low = true;
      for (TopExp_Explorer x(fc, TopAbs_VERTEX); x.More() && low; x.Next())
        low = pb.Distance(BRep_Tool::Pnt(TopoDS::Vertex(x.Current()))) < 0.9 * t;
      if (low)
      {
        base = (int)p;
        b    = o;
      }
    }
    if (base < 0)
      continue;
    // From the base round: an end, the top edges (tangent), the other end.
    const size_t n = uses.size();
    if (n < 4)
      continue;
    const int e_end1 = edges.FindIndex(uses[(base + 1) % n]), e_end2 = edges.FindIndex(uses[(base + n - 1) % n]);
    std::vector<TopoDS_Edge> top;
    bool                     ok = true;
    for (size_t p = 2; p + 1 < n && ok; ++p)
    {
      const int e = edges.FindIndex(uses[(base + p) % n]), o = efi[(size_t)e][0] == c ? efi[(size_t)e][1] : efi[(size_t)e][0];
      ok = tangent[(size_t)e] && !seam[(size_t)e] && kept[(size_t)o] && !collapse[(size_t)o] && o != b;
      top.push_back(uses[(base + p) % n]);
    }
    for (const int e : {e_end1, e_end2})
    {
      const int o = efi[(size_t)e][0] == c ? efi[(size_t)e][1] : efi[(size_t)e][0];
      ok        = ok && !tangent[(size_t)e] && !seam[(size_t)e] && kept[(size_t)o] && BRepAdaptor_Curve(TopoDS::Edge(edges(e))).GetType() == GeomAbs_Line;
    }
    if (!ok || top.empty() || e_end1 == e_end2)
      continue;
    strip[(size_t)c]                                      = 1;
    const int eb                                          = edges.FindIndex(uses[base]);
    gone[(size_t)eb] = gone[(size_t)e_end1] = gone[(size_t)e_end2] = 1;
    // (Its top edges in order; b's wire takes them from the base's start, either way round: below.)
    base_by[(size_t)eb] = top;
    for (const TopoDS_Edge& e : top)
    {
      const int k = edges.FindIndex(e);
      (efi[(size_t)k][0] == c ? efi[(size_t)k][0] : efi[(size_t)k][1]) = b;
      tangent[(size_t)k] = 0;
    }
    for (const int e : {e_end1, e_end2})
    {
      TopoDS_Vertex v0, v1;
      TopExp::Vertices(TopoDS::Edge(edges(e)), v0, v1);
      const int r0 = find(verts.FindIndex(v0)), r1 = find(verts.FindIndex(v1));
      if (r0 != r1)
        vrep[(size_t)r1] = r0;
    }
  }
  // The faces and edges at each of the strips' vertices: b for the strip, and both corners' where they're one.
  if (std::any_of(strip.begin(), strip.end(), [](char x) { return x != 0; }))
    for (int i = 1; i <= nv; ++i)
    {
      const int r = find(i);
      for (int j = 1; j <= nv; ++j)
      {
        if (find(j) != r)
          continue;
        const TopoDS_Vertex& vj = TopoDS::Vertex(verts(j));
        for (ShapeList::Iterator it(vf.FindFromKey(vj)); it.More(); it.Next())
          v_faces[(size_t)i].push_back(faces.FindIndex(it.Value()));
        for (ShapeList::Iterator it(ve.FindFromKey(vj)); it.More(); it.Next())
          if (!gone[(size_t)edges.FindIndex(it.Value())])
            v_edges[(size_t)i].push_back(edges.FindIndex(it.Value()));
      }
      bool on_strip = false;
      for (int f : v_faces[(size_t)i])
        on_strip = on_strip || strip[(size_t)f];
      if (!on_strip)
      {
        v_faces[(size_t)i].clear();
        v_edges[(size_t)i].clear();
        continue;
      }
      // The strip's faces out, the base's in (where the strip was).
      std::vector<int> fs2;
      for (int f : v_faces[(size_t)i])
        if (!strip[(size_t)f] && std::find(fs2.begin(), fs2.end(), f) == fs2.end())
          fs2.push_back(f);
      for (int e : v_edges[(size_t)i])
        for (int f : efi[(size_t)e])
          if (std::find(fs2.begin(), fs2.end(), f) == fs2.end())
            fs2.push_back(f);
      v_faces[(size_t)i] = fs2;
      std::sort(v_edges[(size_t)i].begin(), v_edges[(size_t)i].end());
      v_edges[(size_t)i].erase(std::unique(v_edges[(size_t)i].begin(), v_edges[(size_t)i].end()), v_edges[(size_t)i].end());
    }
  // Vertices: three faces (two tangent: along their offset tangent line), or a closed face's seam and one face.
  std::vector<gp_XYZ> pts(nv + 1);
  // Vertices whose offsets are two corners and an edge between them (four faces, two tangent edges on one): the
  // second corner, for the tangent edge e2 and the edge w2 across; pts holds the first, for e1 and w1.
  struct SplitCorner
  {
    int           v, hinge, wall, e1, e2, w1, w2;
    gp_XYZ        pb;
    TopoDS_Vertex vb;
    TopoDS_Edge   bridge;
    // A flat rim's end: the first corner on the rim's plane, the second on the line of tangency itself (where the
    // wall's offset crosses it), the bridge on the wall's offset (hinge and e1 / e2 unused, -1).
    bool rim = false;
  };
  std::vector<SplitCorner> splits;
  std::vector<int>         vsplit(nv + 1, -1);
  for (int i = 1; i <= nv; ++i)
  {
    const TopoDS_Vertex& v  = TopoDS::Vertex(verts(i));
    const gp_Pnt         V  = BRep_Tool::Pnt(v);
    const ShapeList&     fl = vf.FindFromKey(v);
    std::vector<int>     fs = v_faces[(size_t)i];
    if (fs.empty())
      for (ShapeList::Iterator it(fl); it.More(); it.Next())
        fs.push_back(faces.FindIndex(it.Value()));
    std::vector<int> es = v_edges[(size_t)i];
    if (es.empty())
      for (ShapeList::Iterator it(ve.FindFromKey(v)); it.More(); it.Next())
        es.push_back(edges.FindIndex(it.Value()));
    auto n_at = [&](int f) { return gp_Vec(normal_at(TopoDS::Face(faces(f)), V)).XYZ(); };
    // The corner on tangent edge `tp` (between `a` and another face) where its offset meets face `third`'s; the reason
    // where it isn't found.
    auto tangent_corner = [&](int tp, int a, int third, gp_XYZ& x) -> const char* {
      BRepAdaptor_Curve c(TopoDS::Edge(edges(tp)));
      if (c.GetType() == GeomAbs_Circle)
      {
        // Faces tangent along a circle: their offsets meet along the circle moved along the normals, in its plane.
        const gp_Circ     ci = c.Circle();
        quad::Surf        pl;
        pl.kind               = GeomAbs_Plane;
        pl.n                  = ci.Axis().Direction().XYZ();
        pl.c                  = pl.n.Dot(ci.Location().XYZ());
        const quad::Surf* s3[3] = {&sf[(size_t)a], &sf[(size_t)third], &pl};
        x = V.XYZ() - n_at(a) * t - n_at(third) * (third >= 0 && kept[(size_t)third] ? t : 0.0);
        if (third < 0 || !quad::solve3(s3, x, size))
          return "a tangent circle's corner not found";
      }
      else if (c.GetType() == GeomAbs_Line)
      {
        gp_Pnt p0;
        gp_Vec d0;
        c.D1(c.FirstParameter(), p0, d0);
        const gp_XYZ o = V.XYZ() - n_at(a) * t;
        if (third < 0 || !quad::solve_line(sf[(size_t)third], o, d0.Normalized().XYZ(), x, size))
          return "a tangent line's corner not found";
      }
      else
      {
        // Tangent along another curve: on the curve moved along the faces' common normal (as its offset edge, below),
        // where it meets the third face's offset (Newton in the curve's parameter, from the vertex's end).
        if (third < 0)
          return "a tangent curve's corner not found";
        const double s0 = c.FirstParameter(), s1 = c.LastParameter();
        double       s  = c.Value(s0).Distance(V) <= c.Value(s1).Distance(V) ? s0 : s1;
        const TopoDS_Face fa = TopoDS::Face(faces(a));
        // (On a free-form face: its curve there traced on the offset, exact and on past the face.)
        double                          g0, g1;
        const occ::handle<Geom2d_Curve> pc = sf[(size_t)a].general && kept[(size_t)a] ? BRep_Tool::CurveOnSurface(TopoDS::Edge(edges(tp)), fa, g0, g1) : nullptr;
        auto q = [&](double u) {
          if (!pc.IsNull())
          {
            const gp_Pnt2d uv = pc->Value(u);
            return sf[(size_t)a].off->Value(uv.X(), uv.Y()).XYZ();
          }
          const gp_Pnt p = c.Value(u);
          return p.XYZ() - gp_Vec(normal_at(fa, p)).XYZ() * t;
        };
        bool found = false;
        {
          // The root nearest the vertex first: the first sign change from its end (samples closer near it), bisected
          // (Newton from the end can jump to another crossing: Shell 4's ramp edge, 38 mm off).
          const double end = s == s0 ? s1 : s0;
          double       pa = s, fa0 = quad::eval(sf[(size_t)third], q(s));
          for (int k = 1; k <= 64 && !found; ++k)
          {
            const double pb = s + (end - s) * (k / 64.0) * (k / 64.0), fb = quad::eval(sf[(size_t)third], q(pb));
            if ((fa0 < 0) == (fb < 0))
            {
              pa  = pb;
              fa0 = fb;
              continue;
            }
            double lo = pa, hi = pb, flo = fa0;
            for (int it = 0; it < 100 && std::abs(hi - lo) > 1e-15 * std::abs(s1 - s0); ++it)
            {
              const double m = 0.5 * (lo + hi), fm = quad::eval(sf[(size_t)third], q(m));
              if ((fm < 0) == (flo < 0))
              {
                lo  = m;
                flo = fm;
              }
              else
                hi = m;
            }
            s     = 0.5 * (lo + hi);
            x     = q(s);
            found = true;
          }
        }
        for (int it = 0; it < 60 && !found; ++it)
        {
          const double f = quad::eval(sf[(size_t)third], q(s));
          if (std::abs(f) < 1e-13 * size)
          {
            x     = q(s);
            found = true;
            break;
          }
          const double h  = 1e-7 * (s1 - s0);
          const double df = (quad::eval(sf[(size_t)third], q(s + h)) - quad::eval(sf[(size_t)third], q(s - h))) / (2 * h);
          if (std::abs(df) < 1e-300)
            break;
          s -= f / df;
        }
        if (!found)
          return "a tangent curve's corner not found";
      }
      return nullptr;
    };
    gp_XYZ x;
    const auto col = std::find_if(fs.begin(), fs.end(), [&](int f) { return collapse[(size_t)f] != 0; });
    // Every face tangent there (a sphere's pole on a plane, a corner sphere's point on two fillets and a side): each
    // offset passes V moved in along the common normal.
    bool all_tangent = fs.size() >= 2 && col == fs.end();
    for (size_t p = 0; p < fs.size() && all_tangent; ++p)
      all_tangent = kept[(size_t)fs[p]] && (n_at(fs[p]).Dot(n_at(fs[0])) > 1 - 1e-9 || (!outward && gp_Dir(n_at(fs[p])).Angle(gp_Dir(n_at(fs[0]))) * thickness <= std::max(BRep_Tool::Tolerance(v), 10 * tol)));
    if (all_tangent)
      x = V.XYZ() - n_at(fs[0]) * t;
    else if (col != fs.end())
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
        if (const char* why = tangent_corner(tp, a, third, x))
          return no(why);
        if (rimt[(size_t)tp])
        {
          // A flat rim's end: the first corner (x) where the line's offset meets the third face's, on the rim's plane;
          // the second where the line itself does, the removed face's offset edge across starting there.
          const int k = kept[(size_t)a] ? a : b, r = k == a ? b : a;
          if (third < 0 || !kept[(size_t)third] || find(i) != i)
            return no("a flat rim's end not on a kept face");
          SplitCorner sp{};
          sp.v     = i;
          sp.hinge = sp.e1 = sp.e2 = -1;
          sp.wall  = third;
          sp.rim   = true;
          sp.w1 = sp.w2 = -1;
          for (int e : es)
          {
            const int p = efi[(size_t)e][0], q = efi[(size_t)e][1];
            if ((p == k && q == third) || (q == k && p == third))
              sp.w1 = e;
            if ((p == r && q == third) || (q == r && p == third))
              sp.w2 = e;
          }
          BRepAdaptor_Curve c(TopoDS::Edge(edges(tp)));
          gp_Pnt            p0;
          gp_Vec            d0;
          c.D1(c.FirstParameter(), p0, d0);
          gp_XYZ y;
          if (sp.w1 < 0 || sp.w2 < 0 || !quad::solve_line(sf[(size_t)third], V.XYZ(), d0.Normalized().XYZ(), y, size))
            return no("a flat rim's end not found");
          // (Within the line: the wall's offset crosses it.)
          const double along = (y - V.XYZ()).Dot(d0.Normalized().XYZ()), len = c.Value(c.FirstParameter()).Distance(c.Value(c.LastParameter()));
          if (std::abs(along) <= 10 * tol || std::abs(along) >= len - 10 * tol)
            return no("a flat rim's end off its line");
          sp.pb             = y;
          vsplit[(size_t)i] = (int)splits.size();
          splits.push_back(sp);
        }
      }
      else
      {
        const quad::Surf* s3[3] = {&sf[(size_t)fs[0]], &sf[(size_t)fs[1]], &sf[(size_t)fs[2]]};
        const gp_XYZ      x0 = V.XYZ() - (n_at(fs[0]) * (kept[(size_t)fs[0]] ? t : 0.0) + n_at(fs[1]) * (kept[(size_t)fs[1]] ? t : 0.0) + n_at(fs[2]) * (kept[(size_t)fs[2]] ? t : 0.0));
        x = x0;
        if (!quad::solve3(s3, x, size))
        {
          // (A start on a removed face's line of tangency is a saddle: start again a wall's thickness along each of
          // the vertex's edges, both ways; the solution nearest the corner.)
          bool   found = false;
          double best  = 0;
          for (int e : es)
          {
            BRepAdaptor_Curve c(TopoDS::Edge(edges(e)));
            const double      u = std::abs(c.Value(c.FirstParameter()).Distance(V)) < std::abs(c.Value(c.LastParameter()).Distance(V)) ? c.FirstParameter() : c.LastParameter();
            gp_Pnt            p0;
            gp_Vec            d0;
            c.D1(u, p0, d0);
            if (d0.Magnitude() < 1e-12)
              continue;
            for (double sg : {1.0, -1.0})
            {
              gp_XYZ y = x0 + d0.Normalized().XYZ() * (sg * t);
              if (!quad::solve3(s3, y, size))
                continue;
              // On a removed face: within its patch (the other root of a removed cylinder lies off it).
              bool on = true;
              for (int f : fs)
                if (!kept[(size_t)f] && sf[(size_t)f].kind != GeomAbs_Plane)
                {
                  const TopoDS_Face&         rf = TopoDS::Face(faces(f));
                  GeomAPI_ProjectPointOnSurf pr(gp_Pnt(y), BRep_Tool::Surface(rf));
                  double                     u0, u1, v0, v1, pu, pv;
                  BRepTools::UVBounds(rf, u0, u1, v0, v1);
                  if (pr.NbPoints() == 0)
                  {
                    on = false;
                    continue;
                  }
                  pr.LowerDistanceParameters(pu, pv);
                  const double eu = 1e-6 * (u1 - u0) + 1e-9, ev = 1e-6 * (v1 - v0) + 1e-9;
                  on = on && pu >= u0 - eu && pu <= u1 + eu && pv >= v0 - ev && pv <= v1 + ev;
                }
              if (on && (!found || (y - V.XYZ()).Modulus() < best))
              {
                found = true;
                best  = (y - V.XYZ()).Modulus();
                x     = y;
              }
            }
          }
          if (!found)
          {
            if (dbg)
            {
              std::fprintf(stderr, "quadrics: corner at %g %g %g, faces", V.X() * 1e3, V.Y() * 1e3, V.Z() * 1e3);
              for (int f : fs)
                std::fprintf(stderr, " %d (kind %d kept %d general %d)", f, (int)sf[(size_t)f].kind, (int)kept[(size_t)f], (int)sf[(size_t)f].general);
              for (int a1 = 0; a1 < 3; ++a1)
                for (int b1 = a1 + 1; b1 < 3; ++b1)
                  std::fprintf(stderr, " angle %d-%d %.4g deg", fs[(size_t)a1], fs[(size_t)b1], std::acos(std::min(1.0, n_at(fs[(size_t)a1]).Dot(n_at(fs[(size_t)b1])))) * 180 / M_PI);
              std::fprintf(stderr, "\n");
            }
            return no("a corner's offsets don't meet");
          }
        }
      }
    }
    else if (fs.size() == 4 && es.size() == 4 && std::count_if(es.begin(), es.end(), [&](int e) { return tangent[(size_t)e] != 0; }) == 2
             && std::none_of(es.begin(), es.end(), [&](int e) { return seam[(size_t)e] != 0 || group[(size_t)e] != 0; })
             && std::all_of(fs.begin(), fs.end(), [&](int f) { return kept[(size_t)f] != 0; }))
    {
      // Two tangent edges on one face (the hinge) and a wall across (Shell 4: a ramp tangent to a fillet tangent to
      // the top, ending on a round wall): the offsets meet in two corners, each tangent edge's where it meets the
      // wall's offset, and the cavity has an edge between them on the hinge's and the wall's offsets (Parasolid's).
      SplitCorner sp{};
      sp.v = i;
      sp.e1 = sp.e2 = sp.w1 = sp.w2 = -1;
      for (int e : es)
        if (tangent[(size_t)e])
          (sp.e1 < 0 ? sp.e1 : sp.e2) = e;
      const int a1 = efi[(size_t)sp.e1][0], b1 = efi[(size_t)sp.e1][1], a2 = efi[(size_t)sp.e2][0], b2 = efi[(size_t)sp.e2][1];
      sp.hinge = (a1 == a2 || a1 == b2) ? a1 : (b1 == a2 || b1 == b2) ? b1 : -1;
      if (sp.hinge < 0)
        return no("a corner of two tangent edges on no common face");
      const int x1 = a1 == sp.hinge ? b1 : a1, x2 = a2 == sp.hinge ? b2 : a2;
      sp.wall = -1;
      for (int f : fs)
        if (f != sp.hinge && f != x1 && f != x2)
          sp.wall = f;
      for (int e : es)
      {
        const int p = efi[(size_t)e][0], q = efi[(size_t)e][1];
        if ((p == x1 && q == sp.wall) || (q == x1 && p == sp.wall))
          sp.w1 = e;
        if ((p == x2 && q == sp.wall) || (q == x2 && p == sp.wall))
          sp.w2 = e;
      }
      if (sp.wall < 0 || sp.w1 < 0 || sp.w2 < 0 || find(i) != i)
        return no("a corner of four faces not two tangent edges and a wall");
      if (const char* why = tangent_corner(sp.e1, efi[(size_t)sp.e1][0], sp.wall, x))
        return no(why);
      if (const char* why = tangent_corner(sp.e2, efi[(size_t)sp.e2][0], sp.wall, sp.pb))
        return no(why);
      if ((sp.pb - x).Modulus() > 10 * tol)
      {
        vsplit[(size_t)i] = (int)splits.size();
        splits.push_back(sp);
      }
    }
    else if (fs.size() > 3 && es.size() == fs.size() && std::none_of(es.begin(), es.end(), [&](int e) { return tangent[(size_t)e] != 0 || seam[(size_t)e] != 0; }))
    {
      // More faces: one corner only where all their offsets meet in a point (a pyramid's apex); else the cavity
      // would need an edge or a face there.
      std::vector<const quad::Surf*> all;
      x = V.XYZ();
      for (int f : fs)
      {
        all.push_back(&sf[(size_t)f]);
        x -= n_at(f) * (kept[(size_t)f] ? t / (double)fs.size() * 3.0 : 0.0);
      }
      if (!quad::solve_all(all, x, size))
        return no("a corner of more faces whose offsets don't meet in a point");
    }
    else if (fs.size() == 2 && std::any_of(es.begin(), es.end(), [&](int e) { return seam[(size_t)e] != 0; }))
    {
      const int sm = *std::find_if(es.begin(), es.end(), [&](int e) { return seam[(size_t)e] != 0; });
      const int a  = efi[(size_t)sm][0], b = fs[0] == a ? fs[1] : fs[0];
      const TopoDS_Edge& se = TopoDS::Edge(edges(sm));
      BRepAdaptor_Curve  c(se);
      gp_Pnt             p0;
      gp_Vec             d0;
      // (Along the seam's direction here: a curved seam's turns.)
      const double sv = BRep_Tool::Parameter(v, se);
      c.D1(sv, p0, d0);
      if (!quad::solve_line(sf[(size_t)b], V.XYZ() - n_at(a) * (kept[(size_t)a] ? t : 0.0), d0.Normalized().XYZ(), x, size))
        return no("a seam's corner not found");
      if (cseam[(size_t)sm])
      {
        // On the offset seam itself (its point at s moved along a's normal): a secant on s from the vertex's, its
        // second guess the seam point the line's corner lies off.
        const TopoDS_Face& fa = TopoDS::Face(faces(a));
        auto off_at = [&](double sp) {
          const gp_Pnt q = c.Value(sp);
          return q.XYZ() - gp_Vec(normal_at(fa, q)).XYZ() * t;
        };
        const double lo = c.FirstParameter(), hi = c.LastParameter();
        double       s0 = sv, s1 = sv;
        double c0 = 0, c1 = 0;
        const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(se, c0, c1);
        if (!c3.IsNull())
        {
          GeomAPI_ProjectPointOnCurve pr(gp_Pnt(x + n_at(a) * t), c3, c0, c1);
          if (pr.NbPoints() > 0)
            s1 = pr.LowerDistanceParameter();
        }
        if (std::abs(s1 - s0) < 1e-12 * (1 + std::abs(s0)))
          s1 = s0 + 1e-6 * (hi - lo);
        double h0 = quad::eval(sf[(size_t)b], off_at(s0)), h1 = quad::eval(sf[(size_t)b], off_at(s1));
        bool   ok = std::abs(h1) <= 1e-9 * size;
        for (int k = 0; k < 40 && !ok; ++k)
        {
          if (std::abs(h1 - h0) < 1e-300)
            break;
          const double s2 = std::min(hi, std::max(lo, s1 - h1 * (s1 - s0) / (h1 - h0)));
          s0 = s1, h0 = h1;
          s1 = s2, h1 = quad::eval(sf[(size_t)b], off_at(s1));
          ok = std::abs(h1) <= 1e-9 * size;
        }
        if (!ok)
          return no("a curved seam's corner not found");
        x = off_at(s1);
      }
    }
    else
      return no("a vertex not of three faces (or a seam and one)");
    if ((x - V.XYZ()).Modulus() > 1e3 * thickness + 1e3 * tol) // (a narrow corner moves t / sin(half its angle))
    {
      if (dbg)
        std::fprintf(stderr, "quadrics: vertex %d at %g %g %g (%zu faces, %zu edges) offset to %g %g %g\n", i, V.X(), V.Y(), V.Z(), fs.size(), es.size(), x.X(), x.Y(), x.Z());
      return no("a corner's offset far from it");
    }
    pts[(size_t)i] = x;
  }
  lap("corners");
  // The edges' offsets.
  BRep_Builder               bld;
  std::vector<TopoDS_Vertex> vx(nv + 1);
  for (int i = 1; i <= nv; ++i)
    if (find(i) == i)
      bld.MakeVertex(vx[(size_t)i], gp_Pnt(pts[(size_t)i]), 1e3 * tol);
  for (SplitCorner& sp : splits)
    bld.MakeVertex(sp.vb, gp_Pnt(sp.pb), 1e3 * tol);
  // An edge's end at vertex v (by its index): the second corner of a split one for its e2 and w2.
  auto second = [&](int e, int v) {
    const int k = vsplit[(size_t)v];
    return k >= 0 && (e == splits[(size_t)k].e2 || e == splits[(size_t)k].w2) ? k : -1;
  };
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
  std::map<int, std::pair<std::vector<double>, std::vector<gp_Pnt>>> marched_at; // a marched edge's points by parameter
  for (int i = 1; i <= ne; ++i)
  {
    // (A removed face's seam has no offset: its rim bands get their own, below.)
    if (gone[(size_t)i] || (seam[(size_t)i] && !kept[(size_t)efi[(size_t)i][0]]))
      continue;
    if (degen[(size_t)i])
    {
      // A pole: degenerate at its vertex's offset (its curve on the face added with the face, below).
      TopoDS_Vertex a, b;
      TopExp::Vertices(TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD)), a, b);
      TopoDS_Edge d;
      bld.MakeEdge(d);
      bld.Degenerated(d, true);
      const TopoDS_Vertex& v = vx[(size_t)find(verts.FindIndex(a))];
      bld.Add(d, v.Oriented(TopAbs_FORWARD));
      bld.Add(d, v.Oriented(TopAbs_REVERSED));
      ex[(size_t)i] = d;
      continue;
    }
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
    const int         sa = second(i, ia), sb = second(i, ib);
    const gp_Pnt      A = gp_Pnt(sa >= 0 ? splits[(size_t)sa].pb : pts[(size_t)ia]), B = gp_Pnt(sb >= 0 ? splits[(size_t)sb].pb : pts[(size_t)ib]);
    BRepAdaptor_Curve oc(e);
    gp_Pnt            o0;
    gp_Vec            t0;
    oc.D1(oc.FirstParameter(), o0, t0);
    occ::handle<Geom_Curve> c;
    double                  p0 = 0, p1 = 0;
    // (A collapsing face's tangent edge: the mitre of its two neighbours' offsets.)
    const int fa = group[(size_t)i] != 0 ? across[(size_t)group[(size_t)i]][0] : efi[(size_t)i][0];
    const int fb = group[(size_t)i] != 0 ? across[(size_t)group[(size_t)i]][1] : efi[(size_t)i][1];
    const bool along = (tangent[(size_t)i] || cseam[(size_t)i]) && group[(size_t)i] == 0;
    const bool tangent_circle = along && oc.GetType() == GeomAbs_Circle;
    // (Tangent along another curve, or a curved seam: its points moved along the faces' common normal, fitted.)
    const bool tangent_free = along && oc.GetType() != GeomAbs_Circle && oc.GetType() != GeomAbs_Line;
    if (!tangent_circle && !tangent_free && (seam[(size_t)i] || (tangent[(size_t)i] && group[(size_t)i] == 0) || (sf[(size_t)fa].kind == GeomAbs_Plane && sf[(size_t)fb].kind == GeomAbs_Plane)))
    {
      if (A.Distance(B) <= 10 * tol || (gp_Vec(A, B).Dot(t0) <= 0 && outward))
      {
        if (dbg)
        {
          const gp_Pnt m = oc.Value(0.5 * (oc.FirstParameter() + oc.LastParameter()));
          std::fprintf(stderr, "quadrics: edge %d (faces %d kind %d, %d kind %d) at %g %g %g, length %g, offset %g\n", i, fa, (int)sf[(size_t)fa].kind, fb, (int)sf[(size_t)fb].kind, m.X(), m.Y(), m.Z(), oc.LastParameter() - oc.FirstParameter(), gp_Vec(A, B).Dot(t0.Normalized()));
        }
        return no("an offset edge vanishes or turns round");
      }
      // (Turned round: built as it lies, the line from A to B back along the edge; its faces' loops then cross, and
      // the repair below drops what lies inside the wall.)
      c  = new Geom_Line(A, gp_Dir(gp_Vec(A, B)));
      p1 = A.Distance(B);
    }
    else
    {
      std::vector<occ::handle<Geom_Curve>> cands;
      if (tangent_circle)
      {
        // The circle moved along the faces' common normal (three of its points): its offset, in its plane.
        const double a0 = oc.FirstParameter(), a1 = oc.LastParameter();
        gp_Pnt       q3[3];
        for (int k = 0; k < 3; ++k)
        {
          const gp_Pnt q = oc.Value(a0 + (a1 - a0) * (0.1 + 0.4 * k));
          q3[k]          = q.Translated(gp_Vec(normal_at(TopoDS::Face(faces(fa)), q)) * -t);
        }
        GC_MakeCircle mc(q3[0], q3[1], q3[2]);
        if (!mc.IsDone())
          return no("a tangent circle's offset failed");
        cands.push_back(mc.Value());
      }
      else if (const int fg = !tangent_free                                                                       ? 0
                              : sf[(size_t)fa].general && kept[(size_t)fa] && !sf[(size_t)fa].off.IsNull() ? fa
                              : sf[(size_t)fb].general && kept[(size_t)fb] && !sf[(size_t)fb].off.IsNull() ? fb
                                                                                                           : 0;
               fg != 0)
      {
        // On a free-form face (either), the edge's own curve there (the offset keeps the surface's parameters) traced
        // on the offset: exact (a fit through its points ran off both offsets by more than the wall's tolerance on a
        // twisted vase's rounded corners).
        double                    f0, f1;
        occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, TopoDS::Face(faces(fg)), f0, f1);
        if (pc.IsNull())
          return no("a tangent curve's offset failed");
        // (On a face round a seam, a curve swinging across it near an end (c2-rings-loft's loft side, its bore edge
        // ending on the seam): made again from the edge's points, each foot on the side of the last.)
        if (!pc->IsPeriodic())
        {
          const TopoDS_Face& gf = TopoDS::Face(faces(fg));
          double             span_u = 0, span_v = 0;
          for (TopExp_Explorer x(gf, TopAbs_EDGE); x.More(); x.Next())
          {
            const TopoDS_Edge& se = TopoDS::Edge(x.Current());
            if (!BRep_Tool::IsClosed(se, gf))
              continue;
            double                          s0, s1, r0, r1;
            const occ::handle<Geom2d_Curve> k1 = BRep_Tool::CurveOnSurface(TopoDS::Edge(se.Oriented(TopAbs_FORWARD)), gf, s0, s1);
            const occ::handle<Geom2d_Curve> k2 = BRep_Tool::CurveOnSurface(TopoDS::Edge(se.Oriented(TopAbs_REVERSED)), gf, r0, r1);
            if (k1.IsNull() || k2.IsNull())
              continue;
            span_u = std::max(span_u, std::abs(k2->Value(s0).X() - k1->Value(s0).X()));
            span_v = std::max(span_v, std::abs(k2->Value(s0).Y() - k1->Value(s0).Y()));
          }
          bool jumps = false;
          for (int k = 1; k <= 64 && !jumps && (span_u > 0 || span_v > 0); ++k)
          {
            const gp_Pnt2d p0 = pc->Value(f0 + (f1 - f0) * (k - 1) / 64), p1 = pc->Value(f0 + (f1 - f0) * k / 64);
            jumps             = (span_u > 0 && std::abs(p1.X() - p0.X()) > 0.25 * span_u) || (span_v > 0 && std::abs(p1.Y() - p0.Y()) > 0.25 * span_v);
          }
          double                        c0, c1;
          const occ::handle<Geom_Curve> c3e = BRep_Tool::Curve(e, c0, c1);
          if (jumps && !c3e.IsNull() && sf[(size_t)fg].general)
          {
            const int                    n = 64;
            NCollection_Array1<gp_Pnt2d> uv(1, n + 1);
            NCollection_Array1<double>   pp(1, n + 1);
            const occ::handle<ShapeAnalysis_Surface> sas = new ShapeAnalysis_Surface(BRep_Tool::Surface(gf));
            for (int k = 0; k <= n; ++k)
            {
              const double w = f0 + (f1 - f0) * k / n;
              gp_Pnt2d     q = k == 0 ? pc->Value(f0) : sas->NextValueOfUV(uv(k), c3e->Value(c0 + (c1 - c0) * k / n), 1e-9 * size);
              if (k > 0)
              {
                if (span_u > 0)
                  q.SetX(q.X() + std::round((uv(k).X() - q.X()) / span_u) * span_u);
                if (span_v > 0)
                  q.SetY(q.Y() + std::round((uv(k).Y() - q.Y()) / span_v) * span_v);
              }
              uv(k + 1) = q;
              pp(k + 1) = w;
            }
            occ::handle<TColgp_HArray1OfPnt2d> huv = new TColgp_HArray1OfPnt2d(uv);
            occ::handle<TColStd_HArray1OfReal> hpp = new TColStd_HArray1OfReal(pp);
            Geom2dAPI_Interpolate              fit2(huv, hpp, false, 1e-12);
            fit2.Perform();
            if (fit2.IsDone())
              pc = fit2.Curve();
            if (dbg)
              std::fprintf(stderr, "quadrics: edge %d's curve on %d swung across its seam: made again\n", i, fg);
          }
        }
        // (Past the edge's ends as far as the surface was extended: an outward wall's offset edge runs on past them.)
        if (occ::down_cast<Geom2d_Line>(pc))
        {
          const double ext = (f1 - f0) * std::min(3 * thickness, 0.5 * size) / std::max(GCPnts_AbscissaPoint::Length(oc), tol);
          f0 -= ext;
          f1 += ext;
        }
        BRepBuilderAPI_MakeEdge tmp(pc, sf[(size_t)fg].off, f0, f1);
        if (!tmp.IsDone() || !BRepLib::BuildCurve3d(tmp.Edge(), 1e-9 * size, GeomAbs_C2, 14, 0))
          return no("a tangent curve's offset failed");
        double                        g0, g1;
        const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(tmp.Edge(), g0, g1);
        if (c3.IsNull())
          return no("a tangent curve's offset failed");
        cands.push_back(new Geom_TrimmedCurve(c3, g0, g1));
      }
      else if (tangent_free)
      {
        // (Between where its ends' corners lie along it: a split corner can lie well along the edge.)
        double a0 = oc.FirstParameter(), a1 = oc.LastParameter();
        {
          double                        c0, c1;
          const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(e, c0, c1);
          if (!c3.IsNull())
          {
            GeomAPI_ProjectPointOnCurve pa(A, c3, c0, c1), pb(B, c3, c0, c1);
            if (pa.NbPoints() > 0 && pb.NbPoints() > 0 && pb.LowerDistanceParameter() > pa.LowerDistanceParameter())
            {
              a0 = pa.LowerDistanceParameter();
              a1 = pb.LowerDistanceParameter();
            }
          }
        }
        const int                  n  = 32;
        NCollection_Array1<gp_Pnt> q(1, n + 1);
        for (int k = 0; k <= n; ++k)
        {
          const gp_Pnt p = oc.Value(a0 + (a1 - a0) * k / n);
          q(k + 1)       = p.Translated(gp_Vec(normal_at(TopoDS::Face(faces(fa)), p)) * -t);
        }
        q(1)     = A;
        q(n + 1) = B;
        GeomAPI_PointsToBSpline fit(q, 3, 8, GeomAbs_C2, 1e-7 * size);
        if (!fit.IsDone())
          return no("a tangent curve's offset failed");
        cands.push_back(fit.Curve());
      }
      else
      {
        // (A free-form offset only round the edge, its curve's box on the face and two walls more: the offsets keep
        // their faces' parameters; OCCT's intersection of whole B-spline offsets was most of c2-rings-loft's edges.)
        auto near_edge = [&](int f) -> occ::handle<Geom_Surface> {
          const occ::handle<Geom_Surface>& g = sf[(size_t)f].off;
          if (!sf[(size_t)f].general || g->IsUPeriodic() || g->IsVPeriodic())
            return g;
          double                          e0, e1;
          const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, TopoDS::Face(faces(f)), e0, e1);
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
          const double mu = 2 * thickness / du.Magnitude(), mv = 2 * thickness / dv.Magnitude();
          double       U0, U1, V0, V1;
          g->Bounds(U0, U1, V0, V1);
          u0 = std::max(U0, u0 - mu), u1 = std::min(U1, u1 + mu), v0 = std::max(V0, v0 - mv), v1 = std::min(V1, v1 + mv);
          if (!(u1 > u0) || !(v1 > v0))
            return g;
          return new Geom_RectangularTrimmedSurface(g, u0, u1, v0, v1);
        };
        GeomAPI_IntSS is(near_edge(fa), near_edge(fb), 1e-9 * size);
        if (!is.IsDone())
          return no("two offsets don't intersect");
        for (int k = 1; k <= is.NbLines(); ++k)
          cands.push_back(is.Line(k));
      }
      // A point's parameter on a candidate: its foot, or an end the point lies just past (an intersection of free-form
      // offsets stops at their bounds, which a corner between tangent faces lies on).
      auto on = [&](const gp_Pnt& p, const occ::handle<Geom_Curve>& cand, double& u) {
        GeomAPI_ProjectPointOnCurve pr(p, cand);
        double best = 1e300;
        if (pr.NbPoints() > 0)
          best = pr.LowerDistance(), u = pr.LowerDistanceParameter();
        if (!cand->IsPeriodic())
          for (const double e : {cand->FirstParameter(), cand->LastParameter()})
            if (!Precision::IsInfinite(e) && cand->Value(e).Distance(p) < best)
              best = cand->Value(e).Distance(p), u = e;
        return best <= 1e3 * tol + slack[(size_t)i];
      };
      for (size_t k = 0; k < cands.size() && c.IsNull(); ++k)
      {
        occ::handle<Geom_Curve> cand = cands[k];
        double                  u0 = 0, u1 = 0;
        if (!on(A, cand, u0) || !on(B, cand, u1))
          continue;
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
      if (c.IsNull() && !tangent[(size_t)i] && (sf[(size_t)fa].general || sf[(size_t)fb].general))
      {
        // No one piece of the offsets' intersection runs from end to end (a free-form offset's intersection comes in
        // pieces with gaps at its patches: Shell 4's ramp round the half cylinder; or stops short of an end where the
        // faces turn tangent: c2-rings-loft's loft sides): marched along the edge instead, each point on both offsets
        // in the plane across the edge there, fitted.
        double                        c0, c1;
        const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(e, c0, c1);
        double                        m0 = oc.FirstParameter(), m1 = oc.LastParameter();
        if (!c3.IsNull())
        {
          GeomAPI_ProjectPointOnCurve pa(A, c3, c0, c1), pb(B, c3, c0, c1);
          if (pa.NbPoints() > 0 && pb.NbPoints() > 0 && pb.LowerDistanceParameter() > pa.LowerDistanceParameter())
          {
            m0 = pa.LowerDistanceParameter();
            m1 = pb.LowerDistanceParameter();
          }
        }
        // (64 steps: 32 left the curves on a free-form offset 30 µm apart between the points.)
        const int                  n = 64;
        NCollection_Array1<gp_Pnt> q(1, n + 1);
        bool                       marched = true;
        // The faces' normals by the foot of each step's point, found from the last step's (a projection from scratch
        // each time was 3 ms a step on Shell 4's ramp).
        struct Foot
        {
          TopoDS_Face                        f;
          occ::handle<ShapeAnalysis_Surface> sas;
          BRepAdaptor_Surface                s;
          gp_Pnt2d                           uv;
          bool                               has = false;
          gp_XYZ normal(const gp_Pnt& p, double tol)
          {
            uv  = has ? sas->NextValueOfUV(uv, p, 1e-12, tol) : sas->ValueOfUV(p, 1e-12);
            has = true;
            BRepLProp_SLProps lp(s, uv.X(), uv.Y(), 1, 1e-9);
            const gp_Dir      d = lp.IsNormalDefined() ? lp.Normal() : gp_Dir(0, 0, 1);
            return (f.Orientation() == TopAbs_REVERSED ? d.Reversed() : d).XYZ();
          }
        };
        Foot foot[2];
        for (int k = 0; k < 2; ++k)
        {
          foot[k].f   = TopoDS::Face(faces(k == 0 ? fa : fb));
          foot[k].sas = new ShapeAnalysis_Surface(BRep_Tool::Surface(foot[k].f));
          foot[k].s.Initialize(foot[k].f);
        }
        for (int k = 1; k < n && marched; ++k)
        {
          gp_Pnt p;
          gp_Vec d;
          // (Closer at the ends: a corner can sit by a curl, where a free-form offset's parameters run fast.)
          oc.D1(m0 + (m1 - m0) * 0.5 * (1 - std::cos(M_PI * k / n)), p, d);
          if (d.Magnitude() < 1e-12)
          {
            marched = false;
            break;
          }
          quad::Surf pl;
          pl.kind                 = GeomAbs_Plane;
          pl.n                    = d.Normalized().XYZ();
          pl.c                    = pl.n.Dot(p.XYZ());
          const quad::Surf* s3[3] = {&sf[(size_t)fa], &sf[(size_t)fb], &pl};
          // (The point t off both faces' tangent planes: (na + nb) t / (1 + na·nb), right for tangent faces too.)
          const gp_XYZ      na    = foot[0].normal(p, 1e-6 * size), nb = foot[1].normal(p, 1e-6 * size);
          gp_XYZ            y     = p.XYZ() - (na + nb) * (t / std::max(1 + na.Dot(nb), 1e-3));
          marched                 = quad::solve3_grad(s3, y, size) && (y - p.XYZ()).Modulus() < 10 * thickness;
          q(k + 1)                = gp_Pnt(y);
        }
        q(1)     = A;
        q(n + 1) = B;
        if (marched)
        {
          // By chord length; its curves on the faces fitted through the same points at the same parameters (below).
          NCollection_Array1<double> pr(1, n + 1);
          pr(1) = 0;
          for (int k = 2; k <= n + 1; ++k)
            pr(k) = pr(k - 1) + q(k).Distance(q(k - 1));
          GeomAPI_PointsToBSpline fit(q, pr, 3, 8, GeomAbs_C2, 1e-7 * size);
          if (fit.IsDone())
          {
            c  = fit.Curve();
            p0 = c->FirstParameter();
            p1 = c->LastParameter();
            marched_at[i] = {std::vector<double>(pr.begin(), pr.end()), std::vector<gp_Pnt>(q.begin(), q.end())};
          }
        }
      }
      if (c.IsNull())
      {
        if (dbg)
        {
          const gp_Pnt m = oc.Value(0.5 * (oc.FirstParameter() + oc.LastParameter()));
          std::fprintf(stderr, "quadrics: edge %d (faces %d kind %d, %d kind %d, tangent %d) at %g %g %g, %zu candidates\n", i, fa, (int)sf[(size_t)fa].kind, fb, (int)sf[(size_t)fb].kind, (int)tangent[(size_t)i], m.X(), m.Y(), m.Z(), cands.size());
          for (const auto& cand : cands)
          {
            GeomAPI_ProjectPointOnCurve pa(A, cand), pb(B, cand);
            const gp_Pnt e0 = cand->Value(cand->FirstParameter()), e1 = cand->Value(cand->LastParameter());
            std::fprintf(stderr, "  candidate: A %g B %g; A %g %g %g, B %g %g %g, ends %g %g %g / %g %g %g, tol %g\n", pa.NbPoints() ? pa.LowerDistance() : -1.0, pb.NbPoints() ? pb.LowerDistance() : -1.0, A.X(), A.Y(), A.Z(), B.X(), B.Y(), B.Z(), e0.X(), e0.Y(), e0.Z(), e1.X(), e1.Y(), e1.Z(), tol);
          }
        }
        return no("no intersection of two offsets through an edge's ends");
      }
    }
    const TopoDS_Vertex& v0 = sa >= 0 ? splits[(size_t)sa].vb : vx[(size_t)ia];
    const TopoDS_Vertex& v1 = sb >= 0 ? splits[(size_t)sb].vb : vx[(size_t)ib];
    // (A nearly tangent edge lies on one face's offset, the other's up to its slack away.)
    if (slack[(size_t)i] > 0)
      for (const auto& [v, u] : {std::pair{v0, p0}, std::pair{v1, p1}})
      {
        const double d = std::max(BRep_Tool::Pnt(v).Distance(c->Value(u)), slack[(size_t)i]);
        if (d > BRep_Tool::Tolerance(v))
          BRep_Builder().UpdateVertex(v, 1.01 * d);
      }
    BRepBuilderAPI_MakeEdge mk(c, v0, v1, p0, p1);
    if (!mk.IsDone())
      return no("an offset edge failed");
    ex[(size_t)i] = mk.Edge();
    if (slack[(size_t)i] > BRep_Tool::Tolerance(ex[(size_t)i]))
      BRep_Builder().UpdateEdge(ex[(size_t)i], slack[(size_t)i]);
    if (group[(size_t)i] != 0)
      group_edge[(size_t)group[(size_t)i]] = i;
  }
  // The split corners' edges: on the hinge's and the wall's offsets, from the first corner to the second.
  for (SplitCorner& sp : splits)
  {
    const gp_Pnt A = gp_Pnt(pts[(size_t)sp.v]), B = gp_Pnt(sp.pb);
    if (sp.rim)
    {
      // A flat rim's end: across the wall's offset along the rim's plane, straight (a planar wall's).
      if (A.Distance(B) <= 10 * tol || std::abs(quad::eval(sf[(size_t)sp.wall], 0.5 * (A.XYZ() + B.XYZ()))) > 1e3 * tol)
        return no("a flat rim's end edge not straight");
      BRepBuilderAPI_MakeEdge mk(new Geom_Line(A, gp_Dir(gp_Vec(A, B))), vx[(size_t)sp.v], sp.vb, 0.0, A.Distance(B));
      if (!mk.IsDone())
        return no("a flat rim's end edge failed");
      sp.bridge = mk.Edge();
      continue;
    }
    GeomAPI_IntSS           is(sf[(size_t)sp.hinge].off, sf[(size_t)sp.wall].off, 1e-9 * size);
    occ::handle<Geom_Curve> c;
    double                  u0 = 0, u1 = 0;
    for (int k = 1; is.IsDone() && k <= is.NbLines() && c.IsNull(); ++k)
    {
      occ::handle<Geom_Curve>     cand = is.Line(k);
      GeomAPI_ProjectPointOnCurve pa(A, cand), pb(B, cand);
      if (pa.NbPoints() == 0 || pb.NbPoints() == 0 || pa.LowerDistance() > 1e3 * tol || pb.LowerDistance() > 1e3 * tol)
        continue;
      u0 = pa.LowerDistanceParameter();
      u1 = pb.LowerDistanceParameter();
      if (cand->IsPeriodic())
      {
        // The short way round.
        const double per = cand->Period();
        u1 = u0 + std::remainder(u1 - u0, per);
      }
      if (u1 < u0)
      {
        cand = cand->Reversed();
        u0   = cand->ReversedParameter(u0);
        u1   = cand->ReversedParameter(u1);
      }
      c = cand;
    }
    if (c.IsNull())
      return no("a split corner's edge not found");
    BRepBuilderAPI_MakeEdge mk(c, vx[(size_t)sp.v], sp.vb, u0, u1);
    if (!mk.IsDone())
      return no("a split corner's edge failed");
    sp.bridge = mk.Edge();
  }
  // A flat rim's line of tangency in three: the removed face's skin at each end, the rim between its ends' second
  // corners (the line's own curve, as the line's way).
  std::map<int, std::array<TopoDS_Edge, 3>>          pieces;
  std::vector<std::pair<TopoDS_Shape, TopoDS_Edge>> h_rim_edges; // (the line, a piece)
  for (int i = 1; i <= ne; ++i)
  {
    if (!rimt[(size_t)i])
      continue;
    const TopoDS_Edge e = TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     a, b;
    TopExp::Vertices(e, a, b);
    const int ka = vsplit[(size_t)verts.FindIndex(a)], kb = vsplit[(size_t)verts.FindIndex(b)];
    if (ka < 0 || kb < 0 || !splits[(size_t)ka].rim || !splits[(size_t)kb].rim || BRep_Tool::IsClosed(e, TopoDS::Face(ef.FindFromKey(e).First())))
      return no("a flat rim's line without both ends");
    double                        f0, f1;
    const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(e, f0, f1);
    GeomAPI_ProjectPointOnCurve   pa(gp_Pnt(splits[(size_t)ka].pb), c3), pb(gp_Pnt(splits[(size_t)kb].pb), c3);
    if (c3.IsNull() || pa.NbPoints() == 0 || pb.NbPoints() == 0)
      return no("a flat rim's line without both ends");
    const double ua = pa.LowerDistanceParameter(), ub = pb.LowerDistanceParameter();
    if (!(f0 < ua && ua < ub && ub < f1))
      return no("a flat rim's ends cross");
    auto piece = [&](const TopoDS_Vertex& v0, const TopoDS_Vertex& v1, double u0, double u1) {
      TopoDS_Edge pe;
      bld.MakeEdge(pe, c3, BRep_Tool::Tolerance(e));
      bld.Add(pe, v0.Oriented(TopAbs_FORWARD));
      bld.Add(pe, v1.Oriented(TopAbs_REVERSED));
      bld.Range(pe, u0, u1);
      return pe;
    };
    pieces[i] = {piece(a, splits[(size_t)ka].vb, f0, ua), piece(splits[(size_t)ka].vb, splits[(size_t)kb].vb, ua, ub), piece(splits[(size_t)kb].vb, b, ub, f1)};
    for (const TopoDS_Edge& pe : pieces[i])
      h_rim_edges.emplace_back(edges(i), pe);
  }
  lap("edges");
  bool ok_wires = true;
  auto offset_wire = [&](const TopoDS_Shape& w) {
    TopoDS_Wire ow;
    bld.MakeWire(ow);
    for (TopoDS_Iterator it(w, false); it.More(); it.Next())
    {
      const int k = edges.FindIndex(it.Value());
      if (gone[(size_t)k] && !base_by[(size_t)k].empty())
      {
        // A strip's base: the strip's top edges' offsets, from where the base starts (its corner and the top's one).
        std::vector<TopoDS_Edge> chain = base_by[(size_t)k];
        const int start = find(verts.FindIndex(TopExp::FirstVertex(TopoDS::Edge(it.Value()), true)));
        if (find(verts.FindIndex(TopExp::FirstVertex(chain.front(), true))) != start)
        {
          std::reverse(chain.begin(), chain.end());
          for (TopoDS_Edge& te : chain)
            te.Reverse();
        }
        for (const TopoDS_Edge& te : chain)
        {
          const int j = edges.FindIndex(te);
          if (ex[(size_t)j].IsNull())
          {
            ok_wires = false;
            continue;
          }
          bld.Add(ow, ex[(size_t)j].Oriented(flip[(size_t)j] ? TopAbs::Reverse(te.Orientation()) : te.Orientation()));
        }
        continue;
      }
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
    // A split corner's edge in the hinge's wire (it has both tangent edges) and the wall's (both edges across): from
    // the first corner where the wire comes into the vertex by the first's edge.
    for (const SplitCorner& sp : splits)
    {
      for (const auto [first, other] : {std::pair{sp.e1, sp.e2}, std::pair{sp.w1, sp.w2}})
      {
        if (first < 0 || other < 0)
          continue;
        TopoDS_Edge in_first;
        bool        has_other = false;
        for (TopoDS_Iterator it(w, false); it.More(); it.Next())
        {
          if (it.Value().IsSame(edges(first)))
            in_first = TopoDS::Edge(it.Value());
          has_other = has_other || it.Value().IsSame(edges(other));
        }
        if (in_first.IsNull() || !has_other)
          continue;
        const bool into = TopExp::LastVertex(in_first, true).IsSame(verts(sp.v));
        bld.Add(ow, into ? sp.bridge : TopoDS::Edge(sp.bridge.Reversed()));
      }
    }
    ow.Closed(true);
    return ow;
  };
  SourceMap    sources;
  HHist        h = new BRepTools_History();
  for (const auto& [line, pe] : h_rim_edges)
    h->AddModified(line, pe);
  // A piece's curve on face `face` (built on `ff`'s surface): the line's on `ff`.
  auto piece_curve = [&](int line, const TopoDS_Edge& pe, const TopoDS_Face& ff, const TopoDS_Face& face) {
    double                          c0, c1, u0, u1;
    const occ::handle<Geom2d_Curve> c2 = BRep_Tool::CurveOnSurface(TopoDS::Edge(edges(line).Oriented(TopAbs_FORWARD)), ff, c0, c1);
    if (c2.IsNull())
      return false;
    BRep_Tool::Range(pe, u0, u1);
    bld.UpdateEdge(pe, c2, face, BRep_Tool::Tolerance(pe));
    bld.Range(pe, face, u0, u1);
    return true;
  };
  // A kept face whose line of tangency a flat rim cuts: the same face with the line's pieces in its place (null when
  // it has none; `ok` false when a piece's curve on it failed).
  auto with_pieces = [&](const TopoDS_Face& ff, bool& ok) {
    TopoDS_Face nf;
    bool        any = false;
    for (TopExp_Explorer x(ff, TopAbs_EDGE); x.More() && !any; x.Next())
      any = pieces.count(edges.FindIndex(x.Current())) > 0;
    if (!any)
      return nf;
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(ff, loc);
    bld.MakeFace(nf, surf, loc, BRep_Tool::Tolerance(ff));
    for (TopoDS_Iterator w(ff, false); w.More(); w.Next())
    {
      TopoDS_Wire nw;
      bld.MakeWire(nw);
      for (TopoDS_Iterator x(w.Value(), false); x.More(); x.Next())
      {
        const int  k  = edges.FindIndex(x.Value());
        const auto pc = pieces.find(k);
        if (pc == pieces.end())
        {
          bld.Add(nw, x.Value());
          continue;
        }
        const bool fwd = x.Value().Orientation() == TopAbs_FORWARD;
        for (int j = 0; j < 3; ++j)
        {
          const TopoDS_Edge& pe = pc->second[(size_t)(fwd ? j : 2 - j)];
          if (!piece_curve(k, pe, ff, nf))
          {
            ok = false;
            return TopoDS_Face();
          }
          bld.Add(nw, pe.Oriented(fwd ? TopAbs_FORWARD : TopAbs_REVERSED));
        }
      }
      nw.Closed(true);
      bld.Add(nf, nw.Oriented(w.Value().Orientation()));
    }
    return nf;
  };
  TopoDS_Shell shell;
  bld.MakeShell(shell);
  for (TopoDS_Iterator it(old_shell, false); it.More(); it.Next())
  {
    const TopoDS_Face& f  = TopoDS::Face(it.Value());
    const int          i  = faces.FindIndex(f);
    const TopoDS_Face  ff = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    if (collapse[(size_t)i] || strip[(size_t)i])
      bld.Add(shell, outward ? TopoDS_Shape(f.Reversed()) : TopoDS_Shape(f)); // (no offset: the neighbours' offsets meet where it was)
    else if (kept[(size_t)i])
    {
      bool              pieces_ok = true;
      const TopoDS_Face nk        = with_pieces(ff, pieces_ok);
      if (!pieces_ok)
        return no("a line's piece without its curve");
      if (!nk.IsNull())
      {
        const TopoDS_Face o = TopoDS::Face(nk.Oriented(f.Orientation()));
        bld.Add(shell, o);
        h->AddModified(f, o);
      }
      else
        bld.Add(shell, outward ? TopoDS_Shape(f.Reversed()) : TopoDS_Shape(f));
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
          if (degen[(size_t)k])
          {
            // The pole's curve on the offset sphere: the original's (the frame is kept).
            double                          c0, c1;
            const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), ff, c0, c1);
            if (pc.IsNull())
              return no("a pole without its curve");
            bld.UpdateEdge(ex[(size_t)k], occ::down_cast<Geom2d_Curve>(pc->Copy()), of, 1e3 * tol);
            bld.Range(ex[(size_t)k], of, c0, c1);
            continue;
          }
          if (const auto mp = marched_at.find(k); mp != marched_at.end())
          {
            // A marched edge: its points' places on the offset, at the 3D curve's parameters, fitted (continuous in a
            // periodic u, then a whole number of turns to the original's start).
            const occ::handle<Geom_Surface>& g = sf[(size_t)i].off;
            const auto& [pr, pts3]             = mp->second;
            NCollection_Array1<gp_Pnt2d> uv(1, (int)pts3.size());
            NCollection_Array1<double>   pp(1, (int)pr.size());
            bool                         on = true;
            // (Each from the last one's place: a free-form offset can fold near a curl, its two sheets through one
            // point.)
            const occ::handle<ShapeAnalysis_Surface> sas = new ShapeAnalysis_Surface(g);
            for (size_t p = 0; p < pts3.size() && on; ++p)
            {
              gp_Pnt2d w2 = p == 0 ? sas->ValueOfUV(pts3[p], 1e3 * tol) : sas->NextValueOfUV(uv((int)p), pts3[p], 1e3 * tol);
              if (p > 0 && g->Value(w2.X(), w2.Y()).Distance(pts3[p]) >= 1e3 * tol)
                w2 = sas->ValueOfUV(pts3[p], 1e3 * tol);
              // (A free-form offset's points by a curl of its surface sit where its parametric offset folds, up to
              // ~50 µm off it: Shell 4's ramp ends; the edge's tolerance says so.)
              on = g->Value(w2.X(), w2.Y()).Distance(pts3[p]) < std::max(1e3 * tol, 1e-3 * size);
              if (!on)
              {
                if (dbg)
                  std::fprintf(stderr, "quadrics: edge %d's marched point %zu of %zu %g off face %d (size %g, tol %g)\n", k, p, pts3.size(), g->Value(w2.X(), w2.Y()).Distance(pts3[p]), i, size, tol);
                break;
              }
              double u = w2.X(), v = w2.Y();
              if (p > 0 && g->IsUPeriodic())
                u += std::round((uv((int)p).X() - u) / g->UPeriod()) * g->UPeriod();
              uv((int)p + 1) = gp_Pnt2d(u, v);
              pp((int)p + 1) = pr[p];
            }
            if (!on)
              return no("a marched edge's curve on its face failed");
            // (Interpolated: PointsToBSpline's 2D fit at given parameters ran 2 radians off its own points.)
            occ::handle<TColgp_HArray1OfPnt2d> huv = new TColgp_HArray1OfPnt2d(uv);
            occ::handle<TColStd_HArray1OfReal> hpp = new TColStd_HArray1OfReal(pp);
            Geom2dAPI_Interpolate              fit2(huv, hpp, false, 1e-12);
            fit2.Perform();
            if (!fit2.IsDone())
              return no("a marched edge's curve on its face failed");
            occ::handle<Geom2d_Curve> c2(fit2.Curve());
            double                    o0, o1;
            if (const occ::handle<Geom2d_Curve> co = BRep_Tool::CurveOnSurface(TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), ff, o0, o1); !co.IsNull() && g->IsUPeriodic())
            {
              const double du = std::round((co->Value(o0).X() - c2->Value(c2->FirstParameter()).X()) / g->UPeriod()) * g->UPeriod();
              if (du != 0)
                c2->Translate(gp_Vec2d(du, 0));
            }
            bld.UpdateEdge(ex[(size_t)k], c2, of, 1e3 * tol);
            bld.Range(ex[(size_t)k], of, pr.front(), pr.back());
            // Its tolerance what the curves are apart between the points (an intersection's approximation).
            {
              double                        a0, a1, w = 0;
              const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(ex[(size_t)k], a0, a1);
              const int ns = 4 * (int)pr.size();
              for (int p = 0; p <= ns; ++p)
              {
                const double   sp = pr.front() + (pr.back() - pr.front()) * p / ns;
                const gp_Pnt2d u2 = c2->Value(sp);
                w                 = std::max(w, g->Value(u2.X(), u2.Y()).Distance(c3->Value(sp)));
              }
              if (1.5 * w > BRep_Tool::Tolerance(ex[(size_t)k]))
              {
                bld.UpdateEdge(ex[(size_t)k], 1.5 * w);
                for (TopExp_Explorer vx2(ex[(size_t)k], TopAbs_VERTEX); vx2.More(); vx2.Next())
                  bld.UpdateVertex(TopoDS::Vertex(vx2.Current()), std::max(BRep_Tool::Tolerance(TopoDS::Vertex(vx2.Current())), 1.5 * w));
              }
            }
            continue;
          }
          if (!quad::add_pcurve(ex[(size_t)k], of, TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), ff, seam[(size_t)k] != 0, 1e3 * tol))
            return no("an offset edge's curve on its face failed");
        }
        for (const SplitCorner& sp : splits)
        {
          if (sp.hinge != i && sp.wall != i)
            continue;
          // A split corner's edge: projected, a whole number of periods on to start where its first corner's edge
          // ends on the face.
          const TopoDS_Edge&              by = ex[(size_t)(sp.hinge == i ? sp.e1 : sp.w1)];
          double                          f0, f1, b0, b1;
          const occ::handle<Geom2d_Curve> pby = BRep_Tool::CurveOnSurface(by, of, f0, f1);
          const occ::handle<Geom_Curve>   c3  = BRep_Tool::Curve(sp.bridge, b0, b1);
          occ::handle<Geom2d_Curve>       pc  = c3.IsNull() ? nullptr : GeomProjLib::Curve2d(c3, b0, b1, sf[(size_t)i].off);
          if (pby.IsNull() || pc.IsNull())
            return no("a split corner's edge's curve on its face failed");
          const gp_Pnt2d at = pby->Value(TopExp::FirstVertex(by).IsSame(vx[(size_t)sp.v]) ? f0 : f1), p = pc->Value(b0);
          const occ::handle<Geom_Surface>& g = sf[(size_t)i].off;
          const double du = g->IsUPeriodic() ? std::round((at.X() - p.X()) / g->UPeriod()) * g->UPeriod() : 0;
          const double dv = g->IsVPeriodic() ? std::round((at.Y() - p.Y()) / g->VPeriod()) * g->VPeriod() : 0;
          if (du != 0 || dv != 0)
            pc = occ::down_cast<Geom2d_Curve>(pc->Translated(gp_Vec2d(du, dv)));
          bld.UpdateEdge(sp.bridge, pc, of, 1e3 * tol);
          bld.Range(sp.bridge, of, b0, b1);
        }
      }
      const TopoDS_Face inner = TopoDS::Face(of.Oriented(outward ? f.Orientation() : TopAbs::Reverse(f.Orientation())));
      bld.Add(shell, inner);
      sources.Bind(inner, Source{f, (uint8_t)REL_GENERATED});
    }
    else
    {
      TopLoc_Location                 loc;
      const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(ff, loc);
      bool                            closed = false;
      for (TopExp_Explorer x(ff, TopAbs_EDGE); x.More() && !closed; x.Next())
        closed = seam[(size_t)edges.FindIndex(x.Current())] != 0;
      if (closed && outward)
        return no("outward, a closed face removed");
      if (closed)
      {
        // A closed removed face (a hole's wall): its wire is circle, seam, circle, seam; one band per circle, between
        // it and its offset, closed by its own piece of the seam (the circle's corner to its offset's, along the
        // seam's line and the seam's way).
        TopoDS_Wire w0;
        int         nw = 0;
        for (TopExp_Explorer x(ff, TopAbs_WIRE); x.More(); x.Next(), ++nw)
          w0 = TopoDS::Wire(x.Current());
        std::vector<TopoDS_Edge> seq;
        for (BRepTools_WireExplorer x(w0, ff); x.More(); x.Next())
          seq.push_back(x.Current());
        if (nw != 1 || seq.size() != 4)
          return no("a closed removed face not between two circles");
        for (size_t p = 0; p < 4; ++p)
        {
          const int k = edges.FindIndex(seq[p]);
          if (seam[(size_t)k])
            continue;
          const TopoDS_Edge& sn = seq[(p + 1) % 4]; // the seam as it follows the circle
          const int          ks = edges.FindIndex(sn);
          if (!seam[(size_t)ks] || ex[(size_t)k].IsNull() || !BRep_Tool::IsClosed(seq[p]))
            return no("a closed removed face not between two circles");
          // The circle's corner and its offset's. The seam's two curves on the face sit at two u (its two uses); the
          // band's piece goes up from the circle on the curve at the u where the circle ends: with that use's
          // orientation, run so that this use starts at the circle.
          const TopoDS_Vertex  vc = TopExp::FirstVertex(seq[p]);
          const int            iv = verts.FindIndex(vc);
          const TopoDS_Vertex& vo = vx[(size_t)find(iv)];
          const gp_Pnt         pc = BRep_Tool::Pnt(vc), po = BRep_Tool::Pnt(vo);
          if (pc.Distance(po) <= 10 * tol)
            return no("a band's seam vanishes");
          auto uv_on = [&](const TopoDS_Edge& e, const TopoDS_Face& face, bool start) {
            double                          c0, c1;
            const occ::handle<Geom2d_Curve> pcv = BRep_Tool::CurveOnSurface(e, face, c0, c1);
            const bool                      rev = e.Orientation() == TopAbs_REVERSED;
            return pcv.IsNull() ? gp_Pnt2d(1e300, 1e300) : pcv->Value((start != rev) ? c0 : c1);
          };
          const gp_Pnt2d end = uv_on(seq[p], ff, false);
          const TopoDS_Edge so = TopoDS::Edge(edges(ks).Oriented(TopAbs_FORWARD));
          const double uF = uv_on(so, ff, true).X(), uR = uv_on(TopoDS::Edge(so.Reversed()), ff, true).X();
          const TopAbs_Orientation way = std::abs(end.X() - uF) <= std::abs(end.X() - uR) ? TopAbs_FORWARD : TopAbs_REVERSED;
          // FORWARD use going up: the piece runs circle → offset; REVERSED use going up: offset → circle.
          const gp_Pnt  a = way == TopAbs_FORWARD ? pc : po, b = way == TopAbs_FORWARD ? po : pc;
          BRepBuilderAPI_MakeEdge mk(new Geom_Line(a, gp_Dir(gp_Vec(a, b))), way == TopAbs_FORWARD ? vc : vo, way == TopAbs_FORWARD ? vo : vc, 0.0, a.Distance(b));
          if (!mk.IsDone())
            return no("a band's seam failed");
          const TopoDS_Edge piece = mk.Edge();
          TopoDS_Face       rim;
          bld.MakeFace(rim, surf, loc, BRep_Tool::Tolerance(ff));
          // (The piece's two curves: its FORWARD use on the seam's FORWARD curve's u, REVERSED on the other.)
          if (!quad::add_pcurve(ex[(size_t)k], rim, TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), ff, false, 1e3 * tol) ||
              !quad::add_pcurve(piece, rim, so, ff, true, 1e3 * tol))
            return no("a band's edge curves failed");
          TopoDS_Wire bw;
          bld.MakeWire(bw);
          // circle, the seam piece up, the offset circle back, the seam piece down.
          bld.Add(bw, seq[p]);
          bld.Add(bw, piece.Oriented(way));
          bld.Add(bw, ex[(size_t)k].Oriented(TopAbs::Reverse(seq[p].Orientation())));
          bld.Add(bw, piece.Oriented(TopAbs::Reverse(way)));
          bw.Closed(true);
          bld.Add(rim, bw);
          BRepLib::SameParameter(piece, 1e3 * tol);
          const TopoDS_Face band = TopoDS::Face(rim.Oriented(f.Orientation()));
          bld.Add(shell, band);
          h->AddModified(f, band);
        }
        continue;
      }
      for (TopoDS_Iterator w(ff, false); w.More(); w.Next())
      {
        std::vector<TopoDS_Edge> seq;
        for (BRepTools_WireExplorer x(TopoDS::Wire(w.Value()), ff); x.More(); x.Next())
          seq.push_back(x.Current());
        if (std::any_of(seq.begin(), seq.end(), [&](const TopoDS_Edge& x) { return rimt[(size_t)edges.FindIndex(x)] != 0; }))
        {
          // Lines of tangency on it: the walls there end in flat rims, and what is left of it is a skin at each run of
          // its other edges between them (live 1752, 2045: a corner round's skins in the floor and the top).
          if (w.Value().Orientation() != TopAbs_FORWARD || seq.size() != (size_t)w.Value().NbChildren())
            return no("a flat rim's face not one forward wire");
          const size_t n = seq.size();
          size_t       s0 = 0;
          while (s0 < n && !rimt[(size_t)edges.FindIndex(seq[s0])])
            ++s0;
          // From a line of tangency: the runs after each.
          for (size_t q = 0; q < n; ++q)
          {
            const size_t at = (s0 + q) % n, nx = (at + 1) % n;
            const int    lk = edges.FindIndex(seq[at]);
            if (!rimt[(size_t)lk])
              continue;
            if (rimt[(size_t)edges.FindIndex(seq[nx])])
              return no("two lines of tangency meet");
            std::vector<TopoDS_Edge> run;
            size_t                   end = nx;
            while (!rimt[(size_t)edges.FindIndex(seq[end])])
            {
              run.push_back(seq[end]);
              end = (end + 1) % n;
            }
            const int    nk = edges.FindIndex(seq[end]);
            const auto&  pp = pieces.at(lk);
            const auto&  pn = pieces.at(nk);
            const TopoDS_Vertex vs = TopExp::FirstVertex(run.front(), true), ve = TopExp::LastVertex(run.back(), true);
            // A line's piece at vertex v: the first (the line's start) or the last, and whether it leaves v.
            auto end_piece = [&](const std::array<TopoDS_Edge, 3>& p3, const TopoDS_Vertex& v, bool leaving) {
              const TopoDS_Edge& pe = TopExp::FirstVertex(p3[0]).IsSame(v) ? p3[0] : p3[2];
              const bool         starts = TopExp::FirstVertex(pe).IsSame(v);
              return TopoDS::Edge(pe.Oriented(starts == leaving ? TopAbs_FORWARD : TopAbs_REVERSED));
            };
            TopoDS_Face skin;
            bld.MakeFace(skin, surf, loc, BRep_Tool::Tolerance(ff));
            TopoDS_Wire sw;
            bld.MakeWire(sw);
            for (const TopoDS_Edge& x : run)
              bld.Add(sw, x);
            const TopoDS_Edge out_piece = end_piece(pn, ve, true), in_piece = end_piece(pp, vs, false);
            bld.Add(sw, out_piece);
            for (auto r = run.rbegin(); r != run.rend(); ++r)
            {
              const int k = edges.FindIndex(*r);
              if (ex[(size_t)k].IsNull())
                return no("a skin's edge without its offset");
              bld.Add(sw, ex[(size_t)k].Oriented(flip[(size_t)k] ? r->Orientation() : TopAbs::Reverse(r->Orientation())));
              if (sf[(size_t)i].kind != GeomAbs_Plane && !quad::add_pcurve(ex[(size_t)k], skin, TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), ff, false, 1e3 * tol))
                return no("an offset edge's curve on a skin failed");
            }
            bld.Add(sw, in_piece);
            sw.Closed(true);
            bld.Add(skin, sw);
            if (!piece_curve(nk, out_piece, ff, skin) || !piece_curve(lk, in_piece, ff, skin))
              return no("a skin's piece without its curve");
            const TopoDS_Face sk = TopoDS::Face(skin.Oriented(f.Orientation()));
            bld.Add(shell, sk);
            h->AddModified(f, sk);
          }
          // The flat rims: on the plane through each line square to its wall, between the line's middle piece, its
          // ends' bridges and the line's offset; facing away from the wall (towards the removed face).
          for (const TopoDS_Edge& x : seq)
          {
            const int k = edges.FindIndex(x);
            if (!rimt[(size_t)k])
              continue;
            const TopoDS_Edge e = TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD));
            TopoDS_Vertex     a, b;
            TopExp::Vertices(e, a, b);
            const SplitCorner& sa = splits[(size_t)vsplit[(size_t)verts.FindIndex(a)]];
            const SplitCorner& sb = splits[(size_t)vsplit[(size_t)verts.FindIndex(b)]];
            const int          wf = kept[(size_t)efi[(size_t)k][0]] ? efi[(size_t)k][0] : efi[(size_t)k][1];
            BRepAdaptor_Curve  c(e);
            const gp_Pnt       m = c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
            gp_Vec             d = c.DN(0.5 * (c.FirstParameter() + c.LastParameter()), 1);
            const gp_Vec       nw(normal_at(TopoDS::Face(faces(wf)), m));
            gp_Vec             out = nw.Crossed(d);
            if (out.Magnitude() < 1e-12)
              return no("a flat rim's plane undefined");
            out.Normalize();
            // (Towards the removed face: a point of it inside, off the line.)
            {
              BRepAdaptor_Surface rs(ff);
              const gp_Pnt        q = rs.Value(0.5 * (rs.FirstUParameter() + rs.LastUParameter()), 0.5 * (rs.FirstVParameter() + rs.LastVParameter()));
              if (gp_Vec(m, q).Dot(out) < 0)
                out.Reverse();
            }
            const gp_Pnt na = BRep_Tool::Pnt(sa.vb), nb = BRep_Tool::Pnt(sb.vb), ca = gp_Pnt(pts[(size_t)sa.v]), cb = gp_Pnt(pts[(size_t)sb.v]);
            const gp_XYZ loop[4] = {na.XYZ(), nb.XYZ(), cb.XYZ(), ca.XYZ()};
            gp_XYZ       newell(0, 0, 0);
            for (int j = 0; j < 4; ++j)
              newell += loop[j].Crossed(loop[(j + 1) % 4]);
            TopoDS_Wire fw;
            bld.MakeWire(fw);
            bld.Add(fw, pieces.at(k)[1]);                                // na -> nb
            bld.Add(fw, TopoDS::Edge(sb.bridge.Reversed()));            // nb -> cb
            bld.Add(fw, TopoDS::Edge(ex[(size_t)k].Reversed()));        // cb -> ca
            bld.Add(fw, sa.bridge);                                      // ca -> na
            fw.Closed(true);
            if (newell.Dot(out.XYZ()) < 0)
              fw.Reverse();
            TopoDS_Face flat;
            bld.MakeFace(flat, new Geom_Plane(gp_Pln(m, gp_Dir(out))), 1e3 * tol);
            bld.Add(flat, fw);
            const TopoDS_Face fl = TopoDS::Face(flat.Oriented(old_shell.Orientation() == TopAbs_REVERSED ? TopAbs_REVERSED : TopAbs_FORWARD));
            bld.Add(shell, fl);
            h->AddModified(f, fl);
          }
          continue;
        }
        TopoDS_Face rim;
        bld.MakeFace(rim, surf, loc, BRep_Tool::Tolerance(ff));
        // (Outward the offset is the rim's outside, the part's edges its inside.)
        bld.Add(rim, outward ? w.Value().Reversed() : w.Value());
        bld.Add(rim, offset_wire(w.Value()).Oriented(outward ? w.Value().Orientation() : TopAbs::Reverse(w.Value().Orientation())));
        if (sf[(size_t)i].kind != GeomAbs_Plane)
          for (TopoDS_Iterator x(w.Value(), false); x.More(); x.Next())
          {
            // The offset edges on the removed face's own surface (its edges have their curves there already).
            const int k = edges.FindIndex(x.Value());
            if (gone[(size_t)k])
              continue;
            if (!quad::add_pcurve(ex[(size_t)k], rim, TopoDS::Edge(edges(k).Oriented(TopAbs_FORWARD)), ff, false, 1e3 * tol))
              return no("an offset edge's curve on a removed face failed");
          }
        const TopoDS_Face r = TopoDS::Face(rim.Oriented(f.Orientation()));
        bld.Add(shell, r);
        h->AddModified(f, r);
      }
    }
  }
  lap("faces");
  for (int i = 1; i <= ne; ++i)
    if (!ex[(size_t)i].IsNull() && !degen[(size_t)i])
      BRepLib::SameParameter(ex[(size_t)i], 1e3 * tol);
  lap("same parameter");
  if (!ok_wires)
    return no("an edge without its offset");
  // Offsets that change the part's topology (c2-rings-loft's Shell 1: a crease's offset and its sides' leave through
  // the removed band where the ring is thinner than the wall; a narrow strip's offset vanishes between its
  // neighbours'): a face's offset edges cross on it. Each crossing is a point on three offsets (or removed faces); the
  // edges through it are split there, the pieces nearer the part's kept faces than the wall (or outside the part)
  // dropped, two faces whose offsets now meet joined by a new edge between their two such points, and the faces' loops
  // made again from what is left. False where it can't (the shell as it was).
  auto repair = [&](const TopoDS_Shell& in, TopoDS_Shell& fixed, HHist& h2) -> bool {
    auto fail = [&](const char* why) {
      if (dbg)
        std::fprintf(stderr, "quadrics: repair: %s\n", why);
      return false;
    };
    auto r_lap = std::chrono::steady_clock::now();
    auto rlap  = [&](const char* what) {
      if (!dbg)
        return;
      const auto now = std::chrono::steady_clock::now();
      std::fprintf(stderr, "quadrics: repair: %s %.0f ms\n", what, std::chrono::duration<double, std::milli>(now - r_lap).count());
      r_lap = now;
    };

    // Each shell face's owner: the part's face whose offset it lies on, or the removed face whose surface (0: a kept
    // face of the part, a flat rim).
    NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> owner;
    std::vector<std::vector<TopoDS_Face>>                           of_owner(nf + 1);
    for (TopoDS_Iterator it(in, false); it.More(); it.Next())
    {
      const TopoDS_Face& F = TopoDS::Face(it.Value());
      int                o = 0;
      if (const Source* s = sources.Seek(F))
        o = faces.FindIndex(s->face);
      else
        for (int i = 1; i <= nf && o == 0; ++i)
          if (!kept[(size_t)i] && BRep_Tool::Surface(F) == BRep_Tool::Surface(TopoDS::Face(faces(i))))
            o = i;
      owner.Bind(F, o);
      if (o > 0)
        of_owner[(size_t)o].push_back(F);
    }
    ShapeAncMap ef2;
    TopExp::MapShapesAndUniqueAncestors(in, TopAbs_EDGE, TopAbs_FACE, ef2);
    std::set<std::pair<int, int>> adj, smooth; // faces next to each other in the part; those nearly tangent there
    for (int k = 1; k <= ne; ++k)
    {
      const std::pair<int, int> pr2{std::min(efi[(size_t)k][0], efi[(size_t)k][1]), std::max(efi[(size_t)k][0], efi[(size_t)k][1])};
      adj.insert(pr2);
      if (slack[(size_t)k] > 0 || tangent[(size_t)k])
        smooth.insert(pr2);
    }
    auto other_face = [&](const TopoDS_Shape& e, const TopoDS_Shape& F) -> TopoDS_Shape {
      const int k = ef2.FindIndex(e);
      if (k == 0 || ef2(k).Extent() != 2)
        return TopoDS_Shape();
      return ef2(k).First().IsSame(F) ? ef2(k).Last() : ef2(k).First();
    };
    // A face's offset's value at a point, where it means something: a free-form face's foot square below the point
    // (not held at the end of the surface it runs on as: 1e300).
    auto value = [&](int j, const gp_XYZ& p) {
      gp_XYZ g;
      return quad::eval_grad_near(sf[(size_t)j], p, g, size);
    };
    // (Without the global projection: 1e300 where the last foot and a local search from it find none.)
    auto quick_value = [&](int j, const gp_XYZ& p) {
      gp_XYZ g;
      return quad::eval_grad_near(sf[(size_t)j], p, g, size, false);
    };
    // The crossings: points on three surfaces.
    struct Triple
    {
      gp_XYZ        p;
      int           f[3];
      TopoDS_Vertex v;
    };
    std::vector<Triple> tps;
    auto has = [](const Triple& t, int a) { return t.f[0] == a || t.f[1] == a || t.f[2] == a; };
    for (TopoDS_Iterator it(in, false); it.More(); it.Next())
    {
      const TopoDS_Face F = TopoDS::Face(it.Value().Oriented(TopAbs_FORWARD));
      const int         a = owner.Find(F);
      if (a == 0)
        continue;
      std::vector<TopoDS_Edge> es;
      for (TopExp_Explorer x(F, TopAbs_EDGE); x.More(); x.Next())
        es.push_back(TopoDS::Edge(x.Current()));
      const occ::handle<Geom_Surface> S = BRep_Tool::Surface(F);
      for (size_t i = 0; i < es.size(); ++i)
        for (size_t j = i + 1; j < es.size(); ++j)
        {
          if (es[i].IsSame(es[j]) || BRep_Tool::Degenerated(es[i]) || BRep_Tool::Degenerated(es[j]))
            continue;
          double                          a0, a1, b0, b1;
          const occ::handle<Geom2d_Curve> ci = BRep_Tool::CurveOnSurface(es[i], F, a0, a1);
          const occ::handle<Geom2d_Curve> cj = BRep_Tool::CurveOnSurface(es[j], F, b0, b1);
          if (ci.IsNull() || cj.IsNull())
            return fail("an edge without its curve on its face");
          if (!ci->IsPeriodic())
            a0 = std::max(a0, ci->FirstParameter()), a1 = std::min(a1, ci->LastParameter());
          if (!cj->IsPeriodic())
            b0 = std::max(b0, cj->FirstParameter()), b1 = std::min(b1, cj->LastParameter());
          if (!(a1 > a0) || !(b1 > b0))
            continue;
          Geom2dAPI_InterCurveCurve ic(new Geom2d_TrimmedCurve(ci, a0, a1), new Geom2d_TrimmedCurve(cj, b0, b1), 1e-9);
          for (int k = 1; k <= ic.NbPoints(); ++k)
          {
            const gp_Pnt2d uv = ic.Point(k);
            const gp_Pnt   P  = S->Value(uv.X(), uv.Y());
            // (Not at either's end: edges meeting at a corner; near their common corner (a twentieth of the wall), two
            // edges meeting there at a narrow angle.)
            bool at_end = false;
            for (const TopoDS_Edge& e : {es[i], es[j]})
              for (TopExp_Explorer vx2(e, TopAbs_VERTEX); vx2.More(); vx2.Next())
              {
                const TopoDS_Vertex& V      = TopoDS::Vertex(vx2.Current());
                bool                 common = false;
                for (TopExp_Explorer vx3(e.IsSame(es[i]) ? es[j] : es[i], TopAbs_VERTEX); vx3.More(); vx3.Next())
                  common = common || vx3.Current().IsSame(V);
                at_end = at_end || P.Distance(BRep_Tool::Pnt(V)) <= std::max(common ? 0.05 * thickness : 10 * BRep_Tool::Tolerance(V), 1e-6 * size);
              }
            if (at_end)
              continue;
            // (A seam bounds the face's parameters: no crossing.)
            if (BRep_Tool::IsClosed(es[i], F) || BRep_Tool::IsClosed(es[j], F))
              continue;
            const TopoDS_Shape gi = other_face(es[i], F), gj = other_face(es[j], F);
            if (gi.IsNull() || gj.IsNull())
              return fail("a crossing edge not between two faces");
            const int b = owner.Find(gi), c = owner.Find(gj);
            if (b == 0 || c == 0)
              return fail("a crossing by an edge on a face of the part");
            // (Two edges to the same face lie on one curve: where they touch is no crossing.)
            if (b == c)
              continue;
            if (b == a || c == a)
              return fail("a crossing of two edges with the same faces");
            Triple t{P.XYZ(), {a, b, c}, TopoDS_Vertex()};
            std::sort(t.f, t.f + 3);
            const quad::Surf* s3[3] = {&sf[(size_t)t.f[0]], &sf[(size_t)t.f[1]], &sf[(size_t)t.f[2]]};
            for (const quad::Surf* q : s3)
              q->has_hint = false;
            gp_XYZ y = t.p;
            if (!quad::solve3_grad(s3, y, size) || (y - t.p).Modulus() > 1e-2 * thickness)
            {
              // (Newton off to another root: the crossing itself where it lies on all three.)
              double r = 0;
              for (int k = 0; k < 3; ++k)
                r = std::max(r, std::abs(value(t.f[k], t.p)));
              // (Not on all three: two edges running close by each other near their corner, no crossing.)
              if (r > 1e-3 * thickness)
                continue;
              y = t.p;
            }
            t.p = y;
            // (A corner of the face's found a little off it: no crossing.)
            bool corner = false;
            for (TopExp_Explorer vx2(F, TopAbs_VERTEX); vx2.More() && !corner; vx2.Next())
            {
              const TopoDS_Vertex& V = TopoDS::Vertex(vx2.Current());
              corner                 = (BRep_Tool::Pnt(V).XYZ() - y).Modulus() <= std::max(10 * BRep_Tool::Tolerance(V), 1e-6 * size);
            }
            if (corner)
              continue;
            bool known = false;
            for (const Triple& u : tps)
              known = known || (std::equal(u.f, u.f + 3, t.f) && (u.p - t.p).Modulus() <= 1e-6 * size);
            if (!known)
              tps.push_back(t);
          }
        }
    }
    if (tps.empty())
      return fail("no crossing found");
    if (dbg)
      for (const Triple& t : tps)
        std::fprintf(stderr, "quadrics: repair: faces %d %d %d meet at %.4f %.4f %.4f\n", t.f[0], t.f[1], t.f[2], t.p.X() * 1e3, t.p.Y() * 1e3, t.p.Z() * 1e3);
    rlap("crossings");
    for (Triple& t : tps)
    {
      t.v = BRepBuilderAPI_MakeVertex(gp_Pnt(t.p));
      bld.UpdateVertex(t.v, 1e3 * tol);
    }
    // Whether a point of a new edge lies on the wall's inside: in the part, no nearer its kept faces than the wall.
    std::vector<Bnd_Box> kb(nf + 1), kb_all(nf + 1);
    for (int j = 1; j <= nf; ++j)
    {
      BRepBndLib::Add(faces(j), kb_all[(size_t)j], false);
      if (kept[(size_t)j])
        kb[(size_t)j] = kb_all[(size_t)j];
    }
    BRepClass3d_SolidClassifier inside(so);
    auto on_wall = [&](const gp_Pnt& p, double eps) {
      // In the part: only across a removed face's surface can a point by the wall leave it (the classifier there).
      bool past_removed = false;
      for (int j = 1; j <= nf && !past_removed; ++j)
      {
        if (kept[(size_t)j])
          continue;
        Bnd_Box b = kb_all[(size_t)j];
        b.Enlarge(thickness);
        if (b.IsOut(p))
          continue;
        const double e = quick_value(j, p.XYZ());
        past_removed   = std::abs(e) > 1e299 || e * (faces(j).Orientation() == TopAbs_REVERSED ? -1 : 1) > -eps;
      }
      if (past_removed)
      {
        inside.Perform(p, std::max(eps, 1e3 * tol));
        if (inside.State() == TopAbs_OUT)
          return false;
      }
      // No nearer a kept face than the wall: exactly only where its surface is that near (a point on its offset or
      // a wall past it is no nearer the face than the wall).
      TopoDS_Vertex v;
      for (int j = 1; j <= nf; ++j)
      {
        if (!kept[(size_t)j])
          continue;
        Bnd_Box b = kb[(size_t)j];
        b.Enlarge(thickness);
        if (b.IsOut(p))
          continue;
        const double e = std::abs(quick_value(j, p.XYZ()));
        if (e <= eps || (e < 1e299 && e >= 2 * thickness - eps))
          continue;
        if (v.IsNull())
          v = BRepBuilderAPI_MakeVertex(p);
        BRepExtrema_DistShapeShape d(v, faces(j));
        if (d.IsDone() && d.NbSolution() > 0 && d.Value() < thickness - eps)
          return false;
      }
      return true;
    };
    // The edges split at the points on them, their pieces on the wall kept (an edge by no point: kept or dropped);
    // `ends`: the faces and the point of each kept piece's and new edge's ends.
    NCollection_DataMap<TopoDS_Shape, std::vector<TopoDS_Edge>, TopTools_ShapeMapHasher> pieces_of;
    ShapeIMap                                                                          touched; // faces to make again
    std::set<std::tuple<int, int, const void*>>                                        ends;
    struct Made
    {
      int         b, c;
      TopoDS_Edge e;
    };
    std::vector<Made> made;
    auto end_key = [](int b, int c, const TopoDS_Shape& v) { return std::tuple<int, int, const void*>{std::min(b, c), std::max(b, c), v.TShape().get()}; };
    std::map<std::tuple<const void*, double, double>, bool> wall_cache; // an edge's piece (by its range): on the wall
    auto split_all = [&]() -> bool {
      pieces_of.Clear();
      touched.Clear();
      ends.clear();
      for (int k = 1; k <= ef2.Extent(); ++k)
      {
        const TopoDS_Edge E = TopoDS::Edge(ef2.FindKey(k).Oriented(TopAbs_FORWARD));
        if (ef2(k).Extent() != 2)
          continue;
        const TopoDS_Shape& F1 = ef2(k).First();
        const TopoDS_Shape& F2 = ef2(k).Last();
        const int           o1 = owner.Find(F1), o2 = owner.Find(F2);
        if (o1 == 0 || o2 == 0 || o1 == o2)
          continue;
        double                        c0, c1;
        const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(E, c0, c1);
        if (c3.IsNull())
          return fail("an edge without its curve");
        const double                                  et = BRep_Tool::Tolerance(E);
        std::vector<std::pair<double, TopoDS_Vertex>> cuts;
        for (const Triple& t : tps)
        {
          if (!has(t, o1) || !has(t, o2))
            continue;
          GeomAPI_ProjectPointOnCurve pr(gp_Pnt(t.p), c3, c0, c1);
          if (pr.NbPoints() == 0 || pr.LowerDistance() > std::max(10 * et, 1e-6 * size))
            continue;
          cuts.push_back({pr.LowerDistanceParameter(), t.v});
        }
        const double eps = std::max(1e-3 * thickness, 4 * et);
        if (cuts.empty())
        {
          const auto key = std::tuple<const void*, double, double>{E.TShape().get(), c0, c1};
          auto       hit = wall_cache.find(key);
          if (hit == wall_cache.end())
            hit = wall_cache.emplace(key, on_wall(c3->Value(0.5 * (c0 + c1)), eps)).first;
          if (hit->second)
            continue;
          pieces_of.Bind(E, {});
          touched.Add(F1);
          touched.Add(F2);
          continue;
        }
        std::sort(cuts.begin(), cuts.end(), [](const auto& x, const auto& y) { return x.first < y.first; });
        TopoDS_Vertex v0, v1;
        TopExp::Vertices(E, v0, v1);
        std::vector<std::pair<double, TopoDS_Vertex>> at = {{c0, v0}};
        at.insert(at.end(), cuts.begin(), cuts.end());
        at.push_back({c1, v1});
        std::vector<TopoDS_Edge> kept_pieces;
        for (size_t p = 0; p + 1 < at.size(); ++p)
        {
          if (at[p + 1].first - at[p].first <= 1e-12 * (std::abs(c1 - c0) + 1))
            continue;
          {
            const auto key = std::tuple<const void*, double, double>{E.TShape().get(), at[p].first, at[p + 1].first};
            auto       hit = wall_cache.find(key);
            if (hit == wall_cache.end())
              hit = wall_cache.emplace(key, on_wall(c3->Value(0.5 * (at[p].first + at[p + 1].first)), eps)).first;
            if (!hit->second)
              continue;
          }
          for (const size_t q2 : {p, p + 1})
          {
            const double d = BRep_Tool::Pnt(at[q2].second).Distance(c3->Value(at[q2].first));
            if (1.01 * d > BRep_Tool::Tolerance(at[q2].second))
              bld.UpdateVertex(at[q2].second, 1.01 * d);
          }
          BRepBuilderAPI_MakeEdge mk(c3, TopoDS::Vertex(at[p].second.Oriented(TopAbs_FORWARD)), TopoDS::Vertex(at[p + 1].second.Oriented(TopAbs_REVERSED)), at[p].first, at[p + 1].first);
          if (!mk.IsDone())
            return fail("an edge's piece failed");
          TopoDS_Edge pe = mk.Edge();
          for (const TopoDS_Shape& Fs : {F1, F2})
          {
            const TopoDS_Face Ff = TopoDS::Face(Fs.Oriented(TopAbs_FORWARD));
            double            f0, f1;
            const occ::handle<Geom2d_Curve> pa = BRep_Tool::CurveOnSurface(E, Ff, f0, f1);
            if (pa.IsNull())
              return fail("an edge without its curve on its face");
            if (BRep_Tool::IsClosed(E, Ff))
              bld.UpdateEdge(pe, pa, BRep_Tool::CurveOnSurface(TopoDS::Edge(E.Reversed()), Ff, f0, f1), Ff, et);
            else
              bld.UpdateEdge(pe, pa, Ff, et);
            bld.Range(pe, Ff, at[p].first, at[p + 1].first);
          }
          bld.UpdateEdge(pe, et);
          bld.SameParameter(pe, false);
          bld.SameRange(pe, false);
          BRepLib::SameParameter(pe, std::max(et, 1e3 * tol));
          kept_pieces.push_back(pe);
          ends.insert(end_key(o1, o2, at[p].second));
          ends.insert(end_key(o1, o2, at[p + 1].second));
        }
        pieces_of.Bind(E, kept_pieces);
        touched.Add(F1);
        touched.Add(F2);
      }
      for (const Made& m : made)
      {
        ends.insert(end_key(m.b, m.c, TopExp::FirstVertex(m.e)));
        ends.insert(end_key(m.b, m.c, TopExp::LastVertex(m.e)));
        touched.Add(of_owner[(size_t)m.b][0]);
        touched.Add(of_owner[(size_t)m.c][0]);
      }
      return true;
    };
    // A point on three surfaces, the one known there or a new one.
    auto add_triple = [&](const gp_XYZ& p, int a, int b, int c) -> size_t {
      Triple t{p, {a, b, c}, TopoDS_Vertex()};
      std::sort(t.f, t.f + 3);
      for (size_t k = 0; k < tps.size(); ++k)
        if (std::equal(tps[k].f, tps[k].f + 3, t.f) && (tps[k].p - p).Modulus() <= 1e-5 * size)
          return k;
      t.v = BRepBuilderAPI_MakeVertex(gp_Pnt(p));
      bld.UpdateVertex(t.v, 1e3 * tol);
      tps.push_back(t);
      if (dbg)
        std::fprintf(stderr, "quadrics: repair: faces %d %d %d meet at %.4f %.4f %.4f (traced)\n", t.f[0], t.f[1], t.f[2], p.X() * 1e3, p.Y() * 1e3, p.Z() * 1e3);
      return tps.size() - 1;
    };
    // An edge made from points through them, with its curves on its two faces.
    auto make_edge = [&](int b, int c, std::vector<gp_Pnt> pts, const TopoDS_Vertex& va, const TopoDS_Vertex& vb) -> bool {
      if (of_owner[(size_t)b].size() != 1 || of_owner[(size_t)c].size() != 1)
        return fail("a new edge on a face in more than one piece");
      if (pts.size() < 2)
        return fail("a new edge of one point");
      // (At least eight points: halves of the steps put on both offsets, square to the step.)
      while (pts.size() < 8)
      {
        std::vector<gp_Pnt> more = {pts[0]};
        for (size_t k = 1; k < pts.size(); ++k)
        {
          gp_XYZ           m = 0.5 * (pts[k - 1].XYZ() + pts[k].XYZ());
          const gp_XYZ     d = (pts[k].XYZ() - pts[k - 1].XYZ());
          if (d.Modulus() < 1e-12)
            return fail("a new edge's points on each other");
          quad::Surf pl;
          pl.kind                 = GeomAbs_Plane;
          pl.n                    = d.Normalized();
          pl.c                    = pl.n.Dot(m);
          const quad::Surf* s3[3] = {&sf[(size_t)b], &sf[(size_t)c], &pl};
          const gp_XYZ      m0    = m;
          if (!quad::solve3_damped(s3, m, size) || (m - m0).Modulus() > d.Modulus())
            return fail("a new edge's middle point not found");
          more.push_back(gp_Pnt(m));
          more.push_back(pts[k]);
        }
        pts = more;
      }
      NCollection_Array1<gp_Pnt> q(1, (int)pts.size());
      NCollection_Array1<double> pr(1, (int)pts.size());
      pr(1) = 0;
      for (int k = 1; k <= (int)pts.size(); ++k)
      {
        q(k) = pts[(size_t)k - 1];
        if (k > 1)
          pr(k) = pr(k - 1) + q(k).Distance(q(k - 1));
      }
      GeomAPI_PointsToBSpline fit(q, pr, 3, 8, GeomAbs_C2, 1e-7 * size);
      if (!fit.IsDone())
        return fail("a new edge's curve failed");
      const occ::handle<Geom_BSplineCurve> nc = fit.Curve();
      const double                         n0 = nc->FirstParameter(), n1 = nc->LastParameter();
      BRepBuilderAPI_MakeEdge              mk(nc, va, vb, n0, n1);
      if (!mk.IsDone())
        return fail("a new edge failed");
      TopoDS_Edge ne = mk.Edge();
      for (const int o : {b, c})
      {
        const TopoDS_Face               Ff = TopoDS::Face(of_owner[(size_t)o][0].Oriented(TopAbs_FORWARD));
        TopLoc_Location                 floc;
        const occ::handle<Geom_Surface> S  = BRep_Tool::Surface(Ff, floc);
        if (!floc.IsIdentity())
          return fail("a new edge on a located face");
        // Its points' places on the face (each from the last's), interpolated at the curve's parameters.
        const occ::handle<ShapeAnalysis_Surface> sas = new ShapeAnalysis_Surface(S);
        NCollection_Array1<gp_Pnt2d>              uv(1, (int)pts.size());
        for (int k = 1; k <= (int)pts.size(); ++k)
        {
          gp_Pnt2d w = k == 1 ? sas->ValueOfUV(pts[0], 1e3 * tol) : sas->NextValueOfUV(uv(k - 1), pts[(size_t)k - 1], 1e3 * tol);
          // (A free-form face's offset keeps its parameters: the nearest foot on the face's own patch.)
          if (const quad::Surf& q = sf[(size_t)o]; q.general && kept[(size_t)o])
          {
            gp_XYZ g;
            if (std::abs(quad::eval_grad_near(q, pts[(size_t)k - 1].XYZ(), g, size)) < 1e299)
              w = q.hint;
          }
          const double off_by = std::max({1e3 * tol, 1e-5 * size, 1e-2 * thickness});
          if (S->Value(w.X(), w.Y()).Distance(pts[(size_t)k - 1]) > off_by)
          {
            const gp_Pnt2d w2 = sas->ValueOfUV(pts[(size_t)k - 1], 1e3 * tol);
            if (S->Value(w2.X(), w2.Y()).Distance(pts[(size_t)k - 1]) > off_by)
              return fail("a new edge's point off its face");
            w = w2;
          }
          if (k > 1 && S->IsUPeriodic())
            w.SetX(w.X() + std::round((uv(k - 1).X() - w.X()) / S->UPeriod()) * S->UPeriod());
          if (k > 1 && S->IsVPeriodic())
            w.SetY(w.Y() + std::round((uv(k - 1).Y() - w.Y()) / S->VPeriod()) * S->VPeriod());
          uv(k) = w;
        }
        occ::handle<TColgp_HArray1OfPnt2d> huv = new TColgp_HArray1OfPnt2d(uv);
        occ::handle<TColStd_HArray1OfReal> hpp = new TColStd_HArray1OfReal(pr);
        Geom2dAPI_Interpolate              fit2(huv, hpp, false, 1e-12);
        fit2.Perform();
        if (!fit2.IsDone())
          return fail("a new edge's curve on its face failed");
        occ::handle<Geom2d_Curve> pc(fit2.Curve());
        // (A whole number of periods to where its first end lies on the face's other edges there.)
        if (S->IsUPeriodic() || S->IsVPeriodic())
        {
          const gp_Pnt2d p0 = pc->Value(pc->FirstParameter());
          for (TopExp_Explorer x(Ff, TopAbs_EDGE); x.More(); x.Next())
          {
            double                          e0, e1;
            const occ::handle<Geom2d_Curve> ce = BRep_Tool::CurveOnSurface(TopoDS::Edge(x.Current()), Ff, e0, e1);
            if (ce.IsNull())
              continue;
            const gp_Pnt2d w  = ce->Value(e0).Distance(p0) < ce->Value(e1).Distance(p0) ? ce->Value(e0) : ce->Value(e1);
            const double   du = S->IsUPeriodic() ? std::round((w.X() - p0.X()) / S->UPeriod()) * S->UPeriod() : 0;
            const double   dv = S->IsVPeriodic() ? std::round((w.Y() - p0.Y()) / S->VPeriod()) * S->VPeriod() : 0;
            if (du != 0 || dv != 0)
              pc = occ::down_cast<Geom2d_Curve>(pc->Translated(gp_Vec2d(du, dv)));
            break;
          }
        }
        bld.UpdateEdge(ne, pc, Ff, 1e3 * tol);
        bld.Range(ne, Ff, n0, n1);
        // (Its tolerance what the curves are apart: fitted through the same points at the same parameters.)
        double w = 0;
        for (int k = 0; k <= 64; ++k)
        {
          const double   sp = n0 + (n1 - n0) * k / 64;
          const gp_Pnt2d q2 = pc->Value(sp);
          w                 = std::max(w, S->Value(q2.X(), q2.Y()).Distance(nc->Value(sp)));
        }
        if (1.5 * w > BRep_Tool::Tolerance(ne))
          bld.UpdateEdge(ne, 1.5 * w);
      }
      bld.SameParameter(ne, true);
      bld.SameRange(ne, true);
      for (TopExp_Explorer vx2(ne, TopAbs_VERTEX); vx2.More(); vx2.Next())
        if (BRep_Tool::Tolerance(TopoDS::Vertex(vx2.Current())) < BRep_Tool::Tolerance(ne))
          bld.UpdateVertex(TopoDS::Vertex(vx2.Current()), BRep_Tool::Tolerance(ne));
      made.push_back({b, c, ne});
      return true;
    };
    // The march's last point: its end (in place of a point all but on it).
    auto finish = [&](std::vector<gp_Pnt>& pts, const gp_XYZ& p) {
      while (pts.size() > 1 && pts.back().XYZ().IsEqual(p, 1e-4 * thickness))
        pts.pop_back();
      pts.push_back(gp_Pnt(p));
    };
    struct Polyline
    {
      std::vector<gp_XYZ> pts;
      double              sag = 0;
    };
    std::map<const void*, Polyline> polylines; // an edge's points (by its TShape), for the march's crossing test
    // The distance between segments pq and rs.
    auto segments_apart = [](const gp_XYZ& p, const gp_XYZ& q, const gp_XYZ& r, const gp_XYZ& s2) {
      const gp_XYZ d1 = q - p, d2 = s2 - r, w = p - r;
      const double a = d1.Dot(d1), b = d1.Dot(d2), c = d2.Dot(d2), d = d1.Dot(w), e = d2.Dot(w);
      const double den = a * c - b * b;
      double       sc = den > 1e-30 ? std::clamp((b * e - c * d) / den, 0.0, 1.0) : 0.0;
      double       tc = c > 1e-30 ? std::clamp((b * sc + e) / c, 0.0, 1.0) : 0.0;
      sc              = a > 1e-30 ? std::clamp((b * tc - d) / a, 0.0, 1.0) : 0.0;
      return (w + d1 * sc - d2 * tc).Modulus();
    };
    // From a point where faces b and c meet without an edge of theirs: along their offsets' intersection, the way
    // that stays on the wall, until it comes to another such point of theirs or to a third surface (a new point).
    auto trace = [&](size_t ti, int b, int c) -> bool {
      const quad::Surf* s2[2] = {&sf[(size_t)b], &sf[(size_t)c]};
      s2[0]->has_hint = s2[1]->has_hint = false;
      const double eps  = 1e-3 * thickness;
      const double step = 0.25 * thickness;
      auto         on_curve = [&](const gp_XYZ& from, const gp_XYZ& tg, double h, gp_XYZ& y) {
        for (double hh = h; hh >= h / 64; hh /= 2)
        {
          quad::Surf pl;
          pl.kind                 = GeomAbs_Plane;
          pl.n                    = tg;
          y                       = from + tg * hh;
          pl.c                    = tg.Dot(y);
          const quad::Surf* s3[3] = {s2[0], s2[1], &pl};
          if (quad::solve3_damped(s3, y, size) && (y - from).Modulus() < 2 * hh)
            return true;
        }
        return false;
      };
      auto tangent = [&](const gp_XYZ& x, gp_XYZ& tg) {
        gp_XYZ g0, g1;
        if (std::abs(quad::eval_grad_near(*s2[0], x, g0, size)) > 1e299 || std::abs(quad::eval_grad_near(*s2[1], x, g1, size)) > 1e299)
          return false;
        tg = g0.Crossed(g1);
        if (tg.Modulus() < 1e-9)
          return false;
        tg.Normalize();
        return true;
      };
      const gp_XYZ start = tps[ti].p;
      gp_XYZ       tg;
      if (!tangent(start, tg))
        return fail("a new edge along tangent offsets");
      // (The way on the wall: a fifth of a wall along each.)
      gp_XYZ ya, yb;
      const bool wa = on_curve(start, tg, 0.2 * thickness, ya) && on_wall(gp_Pnt(ya), eps);
      const bool wb = on_curve(start, tg.Reversed(), 0.2 * thickness, yb) && on_wall(gp_Pnt(yb), eps);
      if (wa == wb)
      {
        return fail("a new edge's way not found");
      }
      if (wb)
        tg.Reverse();
      std::vector<gp_Pnt> pts = {gp_Pnt(start)};
      gp_XYZ              x = start;
      // The neighbours' offsets' values at the last two points (a nearly tangent one touches the curve: its value
      // dips to nothing without changing sign).
      std::vector<int> nbs;
      for (int j = 1; j <= nf; ++j)
        if (!has(tps[ti], j) && kept[(size_t)j] && !collapse[(size_t)j] && (adj.count({std::min(j, b), std::max(j, b)}) || adj.count({std::min(j, c), std::max(j, c)})) &&
            !smooth.count({std::min(j, b), std::max(j, b)}) && !smooth.count({std::min(j, c), std::max(j, c)}))
          nbs.push_back(j);
      std::vector<std::array<double, 2>> hist(nbs.size(), {1e300, 1e300});
      // (Each face's value at the last two points asked: a step's end is the next one's start.)
      std::map<int, std::array<std::pair<gp_XYZ, double>, 2>> memo;
      auto val = [&](int j, const gp_XYZ& q) {
        auto& m = memo[j];
        for (const auto& [mp, mv] : m)
          if (mp.IsEqual(q, 0.0) && mv != -1.0)
            return mv;
        const double v = value(j, q);
        m[1]           = m[0];
        m[0]           = {q, v};
        return v;
      };
      for (size_t q2 = 0; q2 < nbs.size(); ++q2)
        hist[q2][1] = std::abs(val(nbs[q2], x));
      int  unchecked = 0;     // steps taken since the last on the wall
      bool dense     = false; // every step tested from here
      for (int it = 0; it < 400; ++it)
      {
        gp_XYZ t2;
        if (!tangent(x, t2))
          return fail("a new edge along tangent offsets");
        if (t2.Dot(tg) < 0)
          t2.Reverse();
        tg = t2;
        gp_XYZ y;
        if (!on_curve(x, tg, step, y))
          return fail("a new edge's point not found");
        // Another point of b's and c's within the step: the edge ends there.
        for (size_t k = 0; k < tps.size(); ++k)
        {
          if (k == ti || !has(tps[k], b) || !has(tps[k], c))
            continue;
          const gp_XYZ d = tps[k].p - x;
          if (d.Modulus() <= (y - x).Modulus() * 1.01 && d.Dot(tg) > 0)
          {
            finish(pts, tps[k].p);
            return make_edge(b, c, pts, tps[ti].v, tps[k].v);
          }
        }
        // Across an edge of b's or c's (on its offset, so within its tolerance of the curve): it ends where it
        // crosses it (nearly tangent faces' offsets cross in no well-defined place, the edge between them does:
        // c2-rings-loft's cylinder offset meeting the bore's and the loft side's at their tangent edge).
        {
          double         es = 2;
          gp_Pnt         ep;
          int            ej = 0;
          const double   L  = (y - x).Modulus();
          const occ::handle<Geom_Line> seg = new Geom_Line(gp_Pnt(x), gp_Dir(y - x));
          for (const int o : {b, c})
            for (const TopoDS_Face& Fo : of_owner[(size_t)o])
              for (TopExp_Explorer ex2(Fo, TopAbs_EDGE); ex2.More(); ex2.Next())
              {
                const TopoDS_Edge&  E  = TopoDS::Edge(ex2.Current());
                const TopoDS_Shape  G  = other_face(E, Fo);
                const int           oj = G.IsNull() ? 0 : owner.Find(G);
                if (oj == 0 || oj == b || oj == c)
                  continue;
                double                        c0, c1;
                const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(E, c0, c1);
                if (c3.IsNull())
                  continue;
                // (First against the edge's polyline, its points cached with how far it strays from the curve.)
                auto pl = polylines.find(E.TShape().get());
                if (pl == polylines.end())
                {
                  Polyline P;
                  for (int k = 0; k <= 64; ++k)
                    P.pts.push_back(c3->Value(c0 + (c1 - c0) * k / 64).XYZ());
                  for (int k = 0; k < 64; ++k)
                    P.sag = std::max(P.sag, (c3->Value(c0 + (c1 - c0) * (k + 0.5) / 64).XYZ() - 0.5 * (P.pts[(size_t)k] + P.pts[(size_t)k + 1])).Modulus());
                  pl = polylines.emplace(E.TShape().get(), std::move(P)).first;
                }
                const double reach0 = std::max(10 * BRep_Tool::Tolerance(E), 1e-3 * thickness);
                double       dmin   = 1e300;
                for (size_t k = 0; k + 1 < pl->second.pts.size() && dmin > reach0 + 2 * pl->second.sag; ++k)
                  dmin = std::min(dmin, segments_apart(x, y, pl->second.pts[k], pl->second.pts[k + 1]));
                if (dmin > reach0 + 2 * pl->second.sag)
                  continue;
                GeomAPI_ExtremaCurveCurve xc(seg, c3, 0, L, c0, c1);
                if (xc.NbExtrema() == 0)
                  continue;
                double sa, sb;
                xc.LowerDistanceParameters(sa, sb);
                const double reach = std::max(10 * BRep_Tool::Tolerance(E), 1e-3 * thickness);
                // (Not where it starts: the edges through its first point.)
                if (xc.LowerDistance() > reach || (pts.size() == 1 && sa < 1e-2 * thickness))
                  continue;
                if (sa / L < es)
                  es = sa / L, ep = c3->Value(sb), ej = oj;
              }
          if (ej != 0)
          {
            // (On both offsets by it, square to the step: the edge lies within its tolerance of it.)
            gp_XYZ     p3 = ep.XYZ();
            quad::Surf pl;
            pl.kind                 = GeomAbs_Plane;
            pl.n                    = (y - x).Normalized();
            pl.c                    = pl.n.Dot(p3);
            const quad::Surf* s3[3] = {s2[0], s2[1], &pl};
            if (!quad::solve3_grad(s3, p3, size) || (p3 - ep.XYZ()).Modulus() > 1e-2 * thickness)
              p3 = ep.XYZ();
            if (dbg)
              std::fprintf(stderr, "quadrics: repair: %d-%d crosses an edge to %d at %.4f %.4f %.4f\n", b, c, ej, p3.X() * 1e3, p3.Y() * 1e3, p3.Z() * 1e3);
            const size_t k = add_triple(p3, b, c, ej);
            finish(pts, tps[k].p);
            return make_edge(b, c, pts, tps[ti].v, tps[k].v);
          }
        }
        // Touching a neighbour's offset at the last point (its value least there, all but nothing): it ends there.
        auto near_face = [&](int j, const gp_XYZ& q) {
          Bnd_Box b = kb_all[(size_t)j];
          b.Enlarge(2 * thickness);
          return !b.IsOut(gp_Pnt(q));
        };
        for (size_t q2 = 0; q2 < nbs.size(); ++q2)
        {
          // (A neighbour whose face is more than two walls away: no touch.)
          if (!near_face(nbs[q2], y))
          {
            hist[q2] = {1e300, 1e300};
            continue;
          }
          const double ey = std::abs(val(nbs[q2], y));
          const bool   dip             = pts.size() >= 2 && hist[q2][1] < 0.1 * thickness && hist[q2][1] < hist[q2][0] && hist[q2][1] < ey;
          hist[q2]                     = {hist[q2][1], ey};
          if (!dip)
            continue;
          // (Its least along the curve between the points before and after, by thirds.)
          const gp_XYZ a0 = pts[pts.size() - 2].XYZ();
          double       lo = 0, hi = 1;
          auto         at = [&](double w) {
            gp_XYZ z = a0 + (y - a0) * w, zz;
            const gp_XYZ dir = (y - a0).Normalized();
            quad::Surf pl;
            pl.kind                 = GeomAbs_Plane;
            pl.n                    = dir;
            pl.c                    = dir.Dot(z);
            const quad::Surf* s3[3] = {s2[0], s2[1], &pl};
            zz                      = z;
            if (!quad::solve3_grad(s3, zz, size))
              zz = z;
            return zz;
          };
          for (int k = 0; k < 40; ++k)
          {
            const double m1 = lo + (hi - lo) / 3, m2 = hi - (hi - lo) / 3;
            const double f1 = std::abs(value(nbs[q2], at(m1)));
            const double f2 = std::abs(value(nbs[q2], at(m2)));
            (f1 < f2 ? hi : lo) = f1 < f2 ? m2 : m1;
          }
          const gp_XYZ p3 = at(0.5 * (lo + hi));
          // (A touch only where its least is within a hundredth of the wall: c2-rings-loft's two loft halves nearly
          // tangent at the top of their crease, their offsets 0.1-3 um apart there.)
          if (std::abs(value(nbs[q2], p3)) > 1e-2 * thickness)
            continue;
          if (dbg)
            std::fprintf(stderr, "quadrics: repair: %d-%d touches %d's offset at %.4f %.4f %.4f\n", b, c, nbs[q2], p3.X() * 1e3, p3.Y() * 1e3, p3.Z() * 1e3);
          const size_t k = add_triple(p3, b, c, nbs[q2]);
          finish(pts, tps[k].p);
          return make_edge(b, c, pts, tps[ti].v, tps[k].v);
        }
        // Across the offset of a face next to b or c in the part (their offsets meet, run on past the faces): it
        // ends there.
        {
          int    nb = 0;
          double ns = 2;
          for (int j = 1; j <= nf; ++j)
          {
            if (has(tps[ti], j) || !kept[(size_t)j] || collapse[(size_t)j] || !(adj.count({std::min(j, b), std::max(j, b)}) || adj.count({std::min(j, c), std::max(j, c)})) ||
                smooth.count({std::min(j, b), std::max(j, b)}) || smooth.count({std::min(j, c), std::max(j, c)}))
              continue;
            if (!near_face(j, y))
              continue;
            // (One on b's or c's offset: two faces of one plane or surface.)
            if (std::abs(val(j, x)) < 1e-6 * thickness && std::abs(val(j, y)) < 1e-6 * thickness)
              continue;
            const double e0 = val(j, x);
            const double e1 = val(j, y);
            if (e0 * e1 > 0 || std::abs(e0) > 1e299 || std::abs(e1) > 1e299)
              continue;
            double lo = 0, hi = 1;
            for (int k = 0; k < 40; ++k)
            {
              const double mid = 0.5 * (lo + hi);
              ((value(j, x + (y - x) * mid) > 0) == (e0 > 0) ? lo : hi) = mid;
            }
            if (hi < ns)
              ns = hi, nb = j;
          }
          if (nb != 0 && on_wall(gp_Pnt(x + (y - x) * ns), eps))
          {
            gp_XYZ            p3    = x + (y - x) * ns;
            const quad::Surf* s3[3] = {s2[0], s2[1], &sf[(size_t)nb]};
            s2[0]->has_hint = s2[1]->has_hint = sf[(size_t)nb].has_hint = false;
            if (!quad::solve3_grad(s3, p3, size) || (p3 - x).Modulus() > 2 * step)
              return fail("a new edge's end not found");
            if (dbg)
              std::fprintf(stderr, "quadrics: repair: %d-%d crosses %d's offset at %.4f %.4f %.4f\n", b, c, nb, p3.X() * 1e3, p3.Y() * 1e3, p3.Z() * 1e3);
            const size_t k = add_triple(p3, b, c, nb);
            finish(pts, tps[k].p);
            return make_edge(b, c, pts, tps[ti].v, tps[k].v);
          }
        }
        // (On the wall tested every fourth step; where a test fails, back to the last point tested and on step by
        // step from there.)
        if (unchecked < 3 && !dense)
        {
          ++unchecked;
          pts.push_back(gp_Pnt(y));
          x = y;
          continue;
        }
        if (on_wall(gp_Pnt(y), eps))
        {
          unchecked = 0;
          pts.push_back(gp_Pnt(y));
          x = y;
          continue;
        }
        if (unchecked > 0)
        {
          pts.resize(pts.size() - (size_t)unchecked);
          x         = pts.back().XYZ();
          unchecked = 0;
          dense     = true;
          for (size_t q2 = 0; q2 < nbs.size(); ++q2)
            hist[q2] = {1e300, std::abs(val(nbs[q2], x))};
          continue;
        }
        // Off the wall: where (bisected), and by which face: one nearer than the wall, or the removed face it leaves
        // the part through.
        double lo = 0, hi = 1;
        for (int k = 0; k < 30; ++k)
        {
          const double mid = 0.5 * (lo + hi);
          (on_wall(gp_Pnt(x + (y - x) * mid), eps) ? lo : hi) = mid;
        }
        const double bs  = hi;
        const gp_Pnt beyond(x + (y - x) * std::min(1.0, hi + 0.05));
        int          best = 0;
        double       bd   = 1e300;
        inside.Perform(beyond, 1e3 * tol);
        const bool out = inside.State() == TopAbs_OUT;
        {
          const TopoDS_Vertex v = BRepBuilderAPI_MakeVertex(beyond);
          for (int j = 1; j <= nf; ++j)
          {
            if (j == b || j == c || collapse[(size_t)j] || (out == (bool)kept[(size_t)j]))
              continue;
            BRepExtrema_DistShapeShape d(v, faces(j));
            if (!d.IsDone() || d.NbSolution() == 0)
              continue;
            const double dj = out ? d.Value() : d.Value() - thickness;
            if (dj < bd)
              bd = dj, best = j;
          }
        }
        if (dbg)
          std::fprintf(stderr, "quadrics: repair: %d-%d leaves the wall at %.4f %.4f %.4f (%s) by %d, %g mm\n", b, c, beyond.X() * 1e3, beyond.Y() * 1e3, beyond.Z() * 1e3, out ? "out" : "in", best, bd * 1e3);
        if (best == 0)
          return fail("a new edge leaves the wall by no surface");
        gp_XYZ            p3      = x + (y - x) * bs;
        const quad::Surf* s3[3]   = {s2[0], s2[1], &sf[(size_t)best]};
        s2[0]->has_hint = s2[1]->has_hint = sf[(size_t)best].has_hint = false;
        if (!quad::solve3_grad(s3, p3, size) || (p3 - x).Modulus() > 2 * step)
          return fail("a new edge's end not found");
        const size_t k = add_triple(p3, b, c, best);
        finish(pts, tps[k].p);
        return make_edge(b, c, pts, tps[ti].v, tps[k].v);
      }
      return fail("a new edge's march didn't end");
    };
    for (int round = 0;; ++round)
    {
      if (round > 32 || tps.size() > 64)
        return fail("too many new points");
      if (!split_all())
        return false;
      rlap("split");
      std::vector<std::tuple<size_t, int, int>> open;
      for (size_t ti = 0; ti < tps.size(); ++ti)
        for (const auto [x, y] : {std::pair{0, 1}, std::pair{0, 2}, std::pair{1, 2}})
          if (!ends.count(end_key(tps[ti].f[x], tps[ti].f[y], tps[ti].v)))
            open.push_back({ti, tps[ti].f[x], tps[ti].f[y]});
      if (open.empty())
        break;
      // (All of this round's: an edge traced from one point can end at another, closing its end too.)
      for (const auto& [ti, b, c] : open)
      {
        bool closed = false;
        for (const Made& m : made)
          closed = closed || (std::min(m.b, m.c) == std::min(b, c) && std::max(m.b, m.c) == std::max(b, c) && (TopExp::FirstVertex(m.e).IsSame(tps[ti].v) || TopExp::LastVertex(m.e).IsSame(tps[ti].v)));
        if (!closed && !trace(ti, b, c))
          return false;
      }
      rlap("traces");
    }
    // The touched faces made again: their edges (pieces in place of split ones, new ones), in loops.
    BRepTools_ReShape re;
    h2 = new BRepTools_History();
    for (int fi = 1; fi <= touched.Extent(); ++fi)
    {
      const TopoDS_Face F  = TopoDS::Face(touched(fi));
      const TopoDS_Face Ff = TopoDS::Face(F.Oriented(TopAbs_FORWARD));
      const int         o  = owner.Find(F);
      std::vector<TopoDS_Edge> use;
      for (TopExp_Explorer x(Ff, TopAbs_EDGE); x.More(); x.Next())
      {
        const TopoDS_Edge& e = TopoDS::Edge(x.Current());
        if (const std::vector<TopoDS_Edge>* ps = pieces_of.Seek(e))
        {
          for (const TopoDS_Edge& pe : *ps)
            use.push_back(TopoDS::Edge(pe.Oriented(e.Orientation())));
          continue;
        }
        use.push_back(e);
      }
      // (On a face round a seam, an edge's curve that jumps across it (SameParameter's re-projection, an input's own
      // curve ending on the seam's other side): made again from its points' feet, each kept on the side of the
      // last.)
      if (sf[(size_t)o].general && kept[(size_t)o])
      {
        double span_u = 0, span_v = 0;
        for (const TopoDS_Edge& e : use)
        {
          if (!BRep_Tool::IsClosed(e, Ff))
            continue;
          double                          e0, e1, g0, g1;
          const occ::handle<Geom2d_Curve> c1 = BRep_Tool::CurveOnSurface(TopoDS::Edge(e.Oriented(TopAbs_FORWARD)), Ff, e0, e1);
          const occ::handle<Geom2d_Curve> c2 = BRep_Tool::CurveOnSurface(TopoDS::Edge(e.Oriented(TopAbs_REVERSED)), Ff, g0, g1);
          if (c1.IsNull() || c2.IsNull())
            continue;
          const gp_Pnt2d d = gp_Pnt2d(c2->Value(e0).XY() - c1->Value(e0).XY());
          span_u           = std::max(span_u, std::abs(d.X()));
          span_v           = std::max(span_v, std::abs(d.Y()));
        }
        for (const TopoDS_Edge& e : use)
        {
          if ((span_u <= 0 && span_v <= 0) || BRep_Tool::IsClosed(e, Ff))
            continue;
          const TopoDS_Edge               ef = TopoDS::Edge(e.Oriented(TopAbs_FORWARD));
          double                          e0, e1, c0, c1;
          const occ::handle<Geom2d_Curve> ce = BRep_Tool::CurveOnSurface(ef, Ff, e0, e1);
          const occ::handle<Geom_Curve>   c3 = BRep_Tool::Curve(ef, c0, c1);
          if (ce.IsNull() || c3.IsNull())
            continue;
          bool jumps = false;
          for (int k = 1; k <= 32 && !jumps; ++k)
          {
            const gp_Pnt2d p0 = ce->Value(e0 + (e1 - e0) * (k - 1) / 32), p1 = ce->Value(e0 + (e1 - e0) * k / 32);
            jumps             = (span_u > 0 && std::abs(p1.X() - p0.X()) > 0.5 * span_u) || (span_v > 0 && std::abs(p1.Y() - p0.Y()) > 0.5 * span_v);
          }
          if (!jumps)
            continue;
          const int                                n = 32;
          NCollection_Array1<gp_Pnt2d>             uv(1, n + 1);
          NCollection_Array1<double>               pp(1, n + 1);
          const occ::handle<ShapeAnalysis_Surface> sas2 = new ShapeAnalysis_Surface(BRep_Tool::Surface(Ff));
          for (int k = 0; k <= n; ++k)
          {
            const double w = c0 + (c1 - c0) * k / n;
            // (Along the curve from its own start, each foot from the last: the nearest foot by the face's edge can
            // lie on the surface's extension past it.)
            gp_Pnt2d q = k == 0 ? ce->Value(e0) : sas2->NextValueOfUV(uv(k), c3->Value(w), 1e3 * tol);
            const gp_Pnt2d ref = k == 0 ? ce->Value(e0) : uv(k);
            if (span_u > 0)
              q.SetX(q.X() + std::round((ref.X() - q.X()) / span_u) * span_u);
            if (span_v > 0)
              q.SetY(q.Y() + std::round((ref.Y() - q.Y()) / span_v) * span_v);
            uv(k + 1) = q;
            pp(k + 1) = w;
          }
          occ::handle<TColgp_HArray1OfPnt2d> huv = new TColgp_HArray1OfPnt2d(uv);
          occ::handle<TColStd_HArray1OfReal> hpp = new TColStd_HArray1OfReal(pp);
          Geom2dAPI_Interpolate              fit2(huv, hpp, false, 1e-12);
          fit2.Perform();
          if (!fit2.IsDone())
            return fail("an edge's curve across a seam not made again");
          const occ::handle<Geom2d_Curve> nc2(fit2.Curve());
          bld.UpdateEdge(ef, nc2, Ff, BRep_Tool::Tolerance(e));
          bld.Range(ef, Ff, c0, c1);
          const occ::handle<Geom_Surface> Sx = BRep_Tool::Surface(Ff);
          double                          dev = 0;
          for (int k = 0; k <= 4 * n; ++k)
          {
            const double   w  = c0 + (c1 - c0) * k / (4 * n);
            const gp_Pnt2d q2 = nc2->Value(w);
            dev               = std::max(dev, Sx->Value(q2.X(), q2.Y()).Distance(c3->Value(w)));
          }
          if (1.5 * dev > BRep_Tool::Tolerance(e))
          {
            bld.UpdateEdge(ef, 1.5 * dev);
            for (TopExp_Explorer vx2(ef, TopAbs_VERTEX); vx2.More(); vx2.Next())
              if (BRep_Tool::Tolerance(TopoDS::Vertex(vx2.Current())) < 1.5 * dev)
                bld.UpdateVertex(TopoDS::Vertex(vx2.Current()), 1.5 * dev);
          }
          if (dbg)
            std::fprintf(stderr, "quadrics: repair: an edge's curve on %d made again across its seam (%g off)\n", o, dev);
        }
      }
      std::vector<TopoDS_Edge> fresh;
      for (const Made& m : made)
        if ((m.b == o || m.c == o) && of_owner[(size_t)o][0].IsSame(F))
          fresh.push_back(m.e);
      if (use.empty())
      {
        // (Nothing of it left: its offset lies inside the wall.)
        if (dbg)
          std::fprintf(stderr, "quadrics: repair: face of %d gone\n", o);
        TopoDS_Compound none;
        bld.MakeCompound(none);
        h2->Remove(F);
        re.Replace(F, none);
        continue;
      }
      // The new edges' way: out of the point where more edges come in than go out.
      auto start = [&](const TopoDS_Edge& e) { return TopExp::FirstVertex(e, true); };
      auto end   = [&](const TopoDS_Edge& e) { return TopExp::LastVertex(e, true); };
      for (const TopoDS_Edge& ne : fresh)
      {
        int bal = 0;
        for (const TopoDS_Edge& e : use)
          bal += (end(e).IsSame(TopExp::FirstVertex(ne)) ? 1 : 0) - (start(e).IsSame(TopExp::FirstVertex(ne)) ? 1 : 0);
        use.push_back(TopoDS::Edge(ne.Oriented(bal > 0 ? TopAbs_FORWARD : TopAbs_REVERSED)));
      }
      // Loops: each edge on to one starting where it ends.
      std::vector<char>                     used(use.size());
      std::vector<std::vector<TopoDS_Edge>> loops;
      for (size_t s0 = 0; s0 < use.size(); ++s0)
      {
        if (used[s0])
          continue;
        std::vector<TopoDS_Edge> lp;
        size_t                   cur = s0;
        used[cur]                    = 1;
        lp.push_back(use[cur]);
        for (;;)
        {
          const TopoDS_Vertex v = end(use[cur]);
          if (v.IsSame(start(lp.front())))
            break;
          // (Of the edges starting there, the one starting where this one ends on the face: a seam's two ways.)
          auto uv_at = [&](const TopoDS_Edge& e, bool at_start) {
            double                          e0, e1;
            const occ::handle<Geom2d_Curve> ce = BRep_Tool::CurveOnSurface(e, Ff, e0, e1);
            const bool                      rv = e.Orientation() == TopAbs_REVERSED;
            return ce.IsNull() ? gp_Pnt2d(1e300, 1e300) : ce->Value((at_start != rv) ? e0 : e1);
          };
          const gp_Pnt2d here = uv_at(use[cur], false);
          size_t         nx   = use.size();
          double         best = 1e300;
          for (size_t k = 0; k < use.size(); ++k)
            if (!used[k] && start(use[k]).IsSame(v) && uv_at(use[k], true).Distance(here) < best)
              nx = k, best = uv_at(use[k], true).Distance(here);
          if (nx == use.size())
            return fail("a face's loop doesn't close");
          used[nx] = 1;
          lp.push_back(use[nx]);
          cur = nx;
        }
        loops.push_back(lp);
      }
      // A free-form face's offset closed round a seam but not periodic (an extension runs on past its ends), its pieces
      // now on both sides of the seam: built on the face's own surface made periodic there, offset alike (the same
      // parameters, so the edges' curves carry over).
      TopoDS_Face      Fb = Ff;
      std::set<size_t> on_fb; // loops (by index) merged onto Fb
      {
        bool split_at_seam = false;
        for (size_t la = 0; la < loops.size() && !split_at_seam; ++la)
          for (const TopoDS_Edge& e : loops[la])
            if (BRep_Tool::IsClosed(e, Ff))
              for (size_t lb = la + 1; lb < loops.size() && !split_at_seam; ++lb)
                for (const TopoDS_Edge& e2 : loops[lb])
                  split_at_seam = split_at_seam || e2.IsSame(e);
        const occ::handle<Geom_Surface> S0 = BRep_Tool::Surface(Ff);
        occ::handle<Geom_BSplineSurface> own = occ::down_cast<Geom_BSplineSurface>(BRep_Tool::Surface(TopoDS::Face(faces(o))));
        if (split_at_seam && !S0->IsUPeriodic() && !S0->IsVPeriodic() && sf[(size_t)o].general && kept[(size_t)o] && !own.IsNull())
        {
          try
          {
            own = occ::down_cast<Geom_BSplineSurface>(own->Copy());
            // (Closed within a hair (its seam's first and last poles that far apart, c2-rings-loft's loft side): the
            // two rows made one, the edges' tolerances take the hair.)
            const double hair = 1e-5 * size;
            for (const bool in_u : {true, false})
            {
              if (in_u ? own->IsUClosed() : own->IsVClosed())
                continue;
              const int    n1 = in_u ? own->NbUPoles() : own->NbVPoles(), n2 = in_u ? own->NbVPoles() : own->NbUPoles();
              double       gap = 0;
              for (int k = 1; k <= n2; ++k)
                gap = std::max(gap, (in_u ? own->Pole(1, k) : own->Pole(k, 1)).Distance(in_u ? own->Pole(n1, k) : own->Pole(k, n1)));
              if (gap > hair)
                continue;
              // (Its weights too, for a rational surface: OCCT calls it closed only with both rows' alike.)
              bool alike = true;
              for (int k = 1; k <= n2 && own->IsURational() + own->IsVRational() > 0; ++k)
              {
                const double w1 = in_u ? own->Weight(1, k) : own->Weight(k, 1), w2 = in_u ? own->Weight(n1, k) : own->Weight(k, n1);
                alike           = alike && std::abs(w1 - w2) <= 1e-6 * std::max(std::abs(w1), std::abs(w2));
              }
              if (!alike)
                continue;
              for (int k = 1; k <= n2; ++k)
              {
                const gp_Pnt m((in_u ? own->Pole(1, k) : own->Pole(k, 1)).XYZ() * 0.5 + (in_u ? own->Pole(n1, k) : own->Pole(k, n1)).XYZ() * 0.5);
                const double w = own->IsURational() || own->IsVRational() ? 0.5 * ((in_u ? own->Weight(1, k) : own->Weight(k, 1)) + (in_u ? own->Weight(n1, k) : own->Weight(k, n1))) : 1.0;
                if (in_u)
                  own->SetPole(1, k, m, w), own->SetPole(n1, k, m, w);
                else
                  own->SetPole(k, 1, m, w), own->SetPole(k, n1, m, w);
              }
            }
            if (own->IsUClosed())
              own->SetUPeriodic();
            if (own->IsVClosed())
              own->SetVPeriodic();
            if (own->IsUPeriodic() || own->IsVPeriodic())
              bld.MakeFace(Fb, new Geom_OffsetSurface(own, sf[(size_t)o].d), BRep_Tool::Tolerance(Ff));
          }
          catch (const Standard_Failure&)
          {
            Fb = Ff;
          }
        }
      }
      // Two loops through one seam the two ways: the piece no longer goes round; one loop without the seam, the second
      // one's curves on the face a period on, to follow on from the first's.
      {
        const occ::handle<Geom_Surface> Sf = BRep_Tool::Surface(Fb);
        // (Closed but not periodic: the two pieces stay two faces, the seam between them.)
        for (bool merged = Sf->IsUPeriodic() || Sf->IsVPeriodic(); merged;)
        {
          merged = false;
          for (size_t la = 0; la < loops.size() && !merged; ++la)
            for (size_t ia = 0; ia < loops[la].size() && !merged; ++ia)
            {
              if (!BRep_Tool::IsClosed(loops[la][ia], Ff))
                continue;
              for (size_t lb = 0; lb < loops.size() && !merged; ++lb)
              {
                if (lb == la)
                  continue;
                for (size_t ib = 0; ib < loops[lb].size() && !merged; ++ib)
                {
                  if (!loops[lb][ib].IsSame(loops[la][ia]) || loops[lb][ib].Orientation() == loops[la][ia].Orientation())
                    continue;
                  // (The two loops' edges' curves onto the periodic surface, where it isn't the face's own.)
                  if (!Fb.IsSame(Ff))
                  {
                    for (const size_t l : {la, lb})
                      for (const TopoDS_Edge& e : loops[l])
                      {
                        const TopoDS_Edge               ef = TopoDS::Edge(e.Oriented(TopAbs_FORWARD));
                        double                          e0, e1;
                        const occ::handle<Geom2d_Curve> c1 = BRep_Tool::CurveOnSurface(ef, Ff, e0, e1);
                        if (c1.IsNull())
                          return fail("a loop's edge without its curve on the face");
                        if (BRep_Tool::IsClosed(ef, Ff))
                          bld.UpdateEdge(ef, c1, BRep_Tool::CurveOnSurface(TopoDS::Edge(ef.Reversed()), Ff, e0, e1), Fb, BRep_Tool::Tolerance(ef));
                        else
                          bld.UpdateEdge(ef, c1, Fb, BRep_Tool::Tolerance(ef));
                        bld.Range(ef, Fb, e0, e1);
                        // (Just past the face's own end the periodic surface isn't the extension the offset ran on:
                        // the edge's tolerance takes the difference.)
                        const occ::handle<Geom_Surface> s1 = BRep_Tool::Surface(Ff), s2b = BRep_Tool::Surface(Fb);
                        double                          w  = 0;
                        for (int k = 0; k <= 32; ++k)
                        {
                          const gp_Pnt2d q2 = c1->Value(e0 + (e1 - e0) * k / 32);
                          w                 = std::max(w, s1->Value(q2.X(), q2.Y()).Distance(s2b->Value(q2.X(), q2.Y())));
                        }
                        if (w > 0.1 * thickness)
                          return fail("a face's own surface far from its offset's extension");
                        if (BRep_Tool::Tolerance(ef) < BRep_Tool::Tolerance(ef) + 1.5 * w && w > 0)
                        {
                          bld.UpdateEdge(ef, BRep_Tool::Tolerance(ef) + 1.5 * w);
                          for (TopExp_Explorer vx2(ef, TopAbs_VERTEX); vx2.More(); vx2.Next())
                            if (BRep_Tool::Tolerance(TopoDS::Vertex(vx2.Current())) < BRep_Tool::Tolerance(ef))
                              bld.UpdateVertex(TopoDS::Vertex(vx2.Current()), BRep_Tool::Tolerance(ef));
                        }
                      }
                    on_fb.insert(la);
                  }
                  std::vector<TopoDS_Edge> A, B;
                  for (size_t k = 1; k < loops[la].size(); ++k)
                    A.push_back(loops[la][(ia + k) % loops[la].size()]);
                  for (size_t k = 1; k < loops[lb].size(); ++k)
                    B.push_back(loops[lb][(ib + k) % loops[lb].size()]);
                  if (A.empty() || B.empty())
                    return fail("a seam's loop of nothing else");
                  // (A's last end and B's first start in the face's parameters: B's curves moved by whole periods.)
                  auto uv_end = [&](const TopoDS_Edge& e, bool at_start) {
                    double                          e0, e1;
                    const occ::handle<Geom2d_Curve> ce = BRep_Tool::CurveOnSurface(e, Fb, e0, e1);
                    const bool                      rev = e.Orientation() == TopAbs_REVERSED;
                    return ce->Value((at_start != rev) ? e0 : e1);
                  };
                  const gp_Pnt2d qa = uv_end(A.back(), false), qb = uv_end(B.front(), true);
                  const double   du = Sf->IsUPeriodic() ? std::round((qa.X() - qb.X()) / Sf->UPeriod()) * Sf->UPeriod() : 0;
                  const double   dv = Sf->IsVPeriodic() ? std::round((qa.Y() - qb.Y()) / Sf->VPeriod()) * Sf->VPeriod() : 0;
                  if (du != 0 || dv != 0)
                    for (const TopoDS_Edge& e : B)
                    {
                      double                          e0, e1;
                      const occ::handle<Geom2d_Curve> ce = BRep_Tool::CurveOnSurface(TopoDS::Edge(e.Oriented(TopAbs_FORWARD)), Fb, e0, e1);
                      if (ce.IsNull() || BRep_Tool::IsClosed(e, Fb))
                        return fail("a seam's loop with a seam of its own");
                      const occ::handle<Geom2d_Curve> moved = occ::down_cast<Geom2d_Curve>(ce->Translated(gp_Vec2d(du, dv)));
                      bld.UpdateEdge(TopoDS::Edge(e.Oriented(TopAbs_FORWARD)), moved, Fb, BRep_Tool::Tolerance(e));
                      bld.Range(TopoDS::Edge(e.Oriented(TopAbs_FORWARD)), Fb, e0, e1);
                    }
                  A.insert(A.end(), B.begin(), B.end());
                  loops[la] = A;
                  loops.erase(loops.begin() + (long)lb);
                  {
                    std::set<size_t> moved;
                    for (const size_t l : on_fb)
                      if (l != lb)
                        moved.insert(l > lb ? l - 1 : l);
                    moved.insert(la > lb ? la - 1 : la);
                    on_fb = moved;
                  }
                  merged = true;
                }
              }
            }
        }
      }
      // Outer loops (going round anticlockwise in the face's parameters) and the holes inside them.
      struct Loop
      {
        std::vector<gp_Pnt2d> poly;
        double                area;
        TopoDS_Wire           w;
        bool                  on_fb = false;
      };
      std::vector<Loop> ls;
      for (size_t li = 0; li < loops.size(); ++li)
      {
        const auto&        lp = loops[li];
        const TopoDS_Face& Fl = on_fb.count(li) ? Fb : Ff;
        Loop               L;
        L.on_fb               = on_fb.count(li) > 0;
        bld.MakeWire(L.w);
        for (const TopoDS_Edge& e : lp)
        {
          bld.Add(L.w, e);
          double                          e0, e1;
          const occ::handle<Geom2d_Curve> ce = BRep_Tool::CurveOnSurface(e, Fl, e0, e1);
          if (ce.IsNull())
            return fail("a loop's edge without its curve on the face");
          const bool rev = e.Orientation() == TopAbs_REVERSED;
          for (int k = 0; k < 16; ++k)
            L.poly.push_back(ce->Value(rev ? e1 - (e1 - e0) * k / 16 : e0 + (e1 - e0) * k / 16));
        }
        L.w.Closed(true);
        L.area = 0;
        for (size_t k = 0; k < L.poly.size(); ++k)
        {
          const gp_Pnt2d& a = L.poly[k];
          const gp_Pnt2d& b = L.poly[(k + 1) % L.poly.size()];
          L.area += 0.5 * (a.X() * b.Y() - b.X() * a.Y());
        }
        // (Outer or a hole by whether the loop alone bounds a face holding no far point: the area in the parameters
        // misleads across a periodic surface's seam.)
        {
          TopLoc_Location                 lloc;
          const occ::handle<Geom_Surface> lS = BRep_Tool::Surface(Fl, lloc);
          TopoDS_Face                     one;
          bld.MakeFace(one, lS, lloc, BRep_Tool::Tolerance(Fl));
          bld.Add(one, L.w);
          L.area = std::abs(L.area) * (ShapeAnalysis::IsOuterBound(one) ? 1 : -1);
        }
        ls.push_back(L);
      }
      auto inside2d = [](const std::vector<gp_Pnt2d>& poly, const gp_Pnt2d& p) {
        bool c = false;
        for (size_t i = 0, j = poly.size() - 1; i < poly.size(); j = i++)
          if (((poly[i].Y() > p.Y()) != (poly[j].Y() > p.Y())) && p.X() < (poly[j].X() - poly[i].X()) * (p.Y() - poly[i].Y()) / (poly[j].Y() - poly[i].Y()) + poly[i].X())
            c = !c;
        return c;
      };
      std::vector<size_t> outers;
      for (size_t k = 0; k < ls.size(); ++k)
        if (ls[k].area > 0)
          outers.push_back(k);
      if (outers.empty())
        return fail("a face made again without an outer loop");
      std::vector<TopoDS_Face> nfs(outers.size());
      for (size_t k = 0; k < outers.size(); ++k)
      {
        TopLoc_Location                 kloc;
        const occ::handle<Geom_Surface> kS = BRep_Tool::Surface(ls[outers[k]].on_fb ? Fb : Ff, kloc);
        bld.MakeFace(nfs[k], kS, kloc, BRep_Tool::Tolerance(Ff));
        bld.Add(nfs[k], ls[outers[k]].w);
      }
      for (size_t k = 0; k < ls.size(); ++k)
      {
        if (ls[k].area > 0)
          continue;
        size_t best = outers.size();
        for (size_t m = 0; m < outers.size(); ++m)
          if (ls[outers[m]].on_fb == ls[k].on_fb && inside2d(ls[outers[m]].poly, ls[k].poly[0]) && (best == outers.size() || ls[outers[m]].area < ls[outers[best]].area))
            best = m;
        if (best == outers.size())
          return fail("a hole in no outer loop");
        bld.Add(nfs[best], ls[k].w);
      }
      TopoDS_Compound cf;
      bld.MakeCompound(cf);
      for (TopoDS_Face& nf2 : nfs)
      {
        const TopoDS_Face of2 = TopoDS::Face(nf2.Oriented(F.Orientation()));
        bld.Add(cf, of2);
        h2->AddModified(F, of2);
        if (const Source* s = sources.Seek(F))
          sources.Bind(of2, *s);
      }
      if (dbg)
        std::fprintf(stderr, "quadrics: repair: face of %d in %zu pieces from %zu loops\n", o, nfs.size(), ls.size());
      re.Replace(F, cf);
    }
    rlap("faces");
    TopoDS_Shape r = re.Apply(in);
    // Two edges meeting at a vertex of theirs alone, between the same two faces (a seam's end, once the pieces on its
    // two sides are one face): one edge, their curves joined.
    {
      ShapeAncMap ve3, ef3;
      TopExp::MapShapesAndUniqueAncestors(r, TopAbs_VERTEX, TopAbs_EDGE, ve3);
      TopExp::MapShapesAndUniqueAncestors(r, TopAbs_EDGE, TopAbs_FACE, ef3);
      BRepTools_ReShape re2;
      ShapeIMap         used;
      for (int k = 1; k <= ve3.Extent(); ++k)
      {
        if (ve3(k).Extent() != 2)
          continue;
        const TopoDS_Vertex& V  = TopoDS::Vertex(ve3.FindKey(k));
        const TopoDS_Edge    e1 = TopoDS::Edge(ve3(k).First().Oriented(TopAbs_FORWARD)), e2 = TopoDS::Edge(ve3(k).Last().Oriented(TopAbs_FORWARD));
        if (e1.IsSame(e2) || used.Contains(e1) || used.Contains(e2) || BRep_Tool::Degenerated(e1) || BRep_Tool::Degenerated(e2))
          continue;
        const ShapeList& f1 = ef3.FindFromKey(e1);
        const ShapeList& f2 = ef3.FindFromKey(e2);
        if (f1.Extent() != 2 || f2.Extent() != 2 || !((f1.First().IsSame(f2.First()) && f1.Last().IsSame(f2.Last())) || (f1.First().IsSame(f2.Last()) && f1.Last().IsSame(f2.First()))))
          continue;
        // (Each way round so that the first ends at V and the second starts there.)
        const bool r1 = !TopExp::LastVertex(e1).IsSame(V), r2 = !TopExp::FirstVertex(e2).IsSame(V);
        double     a1, b1, a2, b2;
        const occ::handle<Geom_Curve> g1 = BRep_Tool::Curve(e1, a1, b1), g2 = BRep_Tool::Curve(e2, a2, b2);
        if (g1.IsNull() || g2.IsNull())
          continue;
        auto bs3 = [](const occ::handle<Geom_Curve>& g, double a, double b, bool rev) {
          occ::handle<Geom_BSplineCurve> c = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(g, a, b));
          if (rev)
            c->Reverse();
          return c;
        };
        GeomConvert_CompCurveToBSplineCurve j3(bs3(g1, a1, b1, r1));
        const double                        tj = std::max({BRep_Tool::Tolerance(e1), BRep_Tool::Tolerance(e2), BRep_Tool::Tolerance(V)});
        if (!j3.Add(bs3(g2, a2, b2, r2), 2 * tj))
          continue;
        const occ::handle<Geom_BSplineCurve> c3 = j3.BSplineCurve();
        const TopoDS_Vertex v0 = r1 ? TopExp::LastVertex(e1) : TopExp::FirstVertex(e1), v1 = r2 ? TopExp::FirstVertex(e2) : TopExp::LastVertex(e2);
        BRepBuilderAPI_MakeEdge mk(c3, v0, v1, c3->FirstParameter(), c3->LastParameter());
        if (!mk.IsDone())
          continue;
        TopoDS_Edge ne = mk.Edge();
        bool        ok = true;
        for (const TopoDS_Shape& Fs : {f1.First(), f1.Last()})
        {
          const TopoDS_Face Fj = TopoDS::Face(Fs.Oriented(TopAbs_FORWARD));
          double            p0, p1, q0, q1;
          const occ::handle<Geom2d_Curve> h1 = BRep_Tool::CurveOnSurface(e1, Fj, p0, p1), h2c = BRep_Tool::CurveOnSurface(e2, Fj, q0, q1);
          if (h1.IsNull() || h2c.IsNull() || BRep_Tool::IsClosed(e1, Fj) || BRep_Tool::IsClosed(e2, Fj))
          {
            ok = false;
            break;
          }
          // (Its points' places on the face, each from the last from the first edge's start, interpolated at the
          // joined curve's parameters.)
          TopLoc_Location                          jl;
          const occ::handle<Geom_Surface>          Sj  = BRep_Tool::Surface(Fj, jl);
          const occ::handle<ShapeAnalysis_Surface> saj = new ShapeAnalysis_Surface(Sj);
          const int                                n   = 48;
          NCollection_Array1<gp_Pnt2d>             uvj(1, n + 1);
          NCollection_Array1<double>               ppj(1, n + 1);
          const double                             j0 = c3->FirstParameter(), j1 = c3->LastParameter();
          for (int k = 0; k <= n; ++k)
          {
            const double w = j0 + (j1 - j0) * k / n;
            gp_Pnt2d     q = k == 0 ? h1->Value(r1 ? p1 : p0) : saj->NextValueOfUV(uvj(k), c3->Value(w), 1e3 * tol);
            // (Continuous through a period.)
            if (k > 0 && Sj->IsUPeriodic())
              q.SetX(q.X() + std::round((uvj(k).X() - q.X()) / Sj->UPeriod()) * Sj->UPeriod());
            if (k > 0 && Sj->IsVPeriodic())
              q.SetY(q.Y() + std::round((uvj(k).Y() - q.Y()) / Sj->VPeriod()) * Sj->VPeriod());
            uvj(k + 1) = q;
            ppj(k + 1) = w;
          }
          Geom2dAPI_Interpolate fj(new TColgp_HArray1OfPnt2d(uvj), new TColStd_HArray1OfReal(ppj), false, 1e-12);
          fj.Perform();
          if (!fj.IsDone() || !jl.IsIdentity())
          {
            ok = false;
            break;
          }
          const occ::handle<Geom2d_Curve> pj(fj.Curve());
          double                          dev = 0;
          for (int k = 0; k <= 4 * n; ++k)
          {
            const double   w = j0 + (j1 - j0) * k / (4 * n);
            const gp_Pnt2d q = pj->Value(w);
            dev              = std::max(dev, Sj->Value(q.X(), q.Y()).Distance(c3->Value(w)));
          }
          if (dev > 1e-2 * thickness)
          {
            ok = false;
            break;
          }
          bld.UpdateEdge(ne, pj, Fj, std::max(tj, 1.5 * dev));
          bld.Range(ne, Fj, j0, j1);
          if (1.5 * dev > BRep_Tool::Tolerance(ne))
            bld.UpdateEdge(ne, 1.5 * dev);
        }
        if (!ok)
          continue;
        bld.SameParameter(ne, true);
        bld.SameRange(ne, true);
        for (TopExp_Explorer vx2(ne, TopAbs_VERTEX); vx2.More(); vx2.Next())
          if (BRep_Tool::Tolerance(TopoDS::Vertex(vx2.Current())) < BRep_Tool::Tolerance(ne))
            bld.UpdateVertex(TopoDS::Vertex(vx2.Current()), BRep_Tool::Tolerance(ne));
        // (In place of the first the way it ran; the second gone.)
        re2.Replace(e1, r1 ? TopoDS::Edge(ne.Reversed()) : ne);
        re2.Remove(e2);
        used.Add(e1);
        used.Add(e2);
        if (dbg)
        {
          const gp_Pnt q = BRep_Tool::Pnt(V);
          std::fprintf(stderr, "quadrics: repair: two edges joined at %.4f %.4f %.4f\n", q.X() * 1e3, q.Y() * 1e3, q.Z() * 1e3);
        }
      }
      if (used.Extent() > 0)
      {
        r = re2.Apply(r);
        h2->Merge(re2.History());
        SourceMap moved;
        for (SourceMap::Iterator it(sources); it.More(); it.Next())
        {
          const TopoDS_Shape nw = re2.Value(it.Key());
          if (!nw.IsNull() && !nw.IsSame(it.Key()))
            moved.Bind(nw, it.Value());
        }
        for (SourceMap::Iterator it(moved); it.More(); it.Next())
          sources.Bind(it.Key(), it.Value());
      }
    }
    rlap("joins");
    bld.MakeShell(fixed);
    for (TopExp_Explorer x(r, TopAbs_FACE); x.More(); x.Next())
      bld.Add(fixed, x.Current());
    fixed.Closed(true);
    return true;
  };
  shell.Closed(true);
  TopoDS_Solid outer_wall;
  bld.MakeSolid(outer_wall);
  bld.Add(outer_wall, shell.Oriented(old_shell.Orientation()));
  TopoDS_Shape wall = outer_wall;
  HHist        h2;
  if (BRepCheck_Analyzer wan0(wall); !wan0.IsValid() && !outward)
  {
    TopoDS_Shell fixed;
    bool         ok = false;
    try
    {
      ok = repair(shell, fixed, h2);
    }
    catch (const Standard_Failure& e)
    {
      if (dbg)
        std::fprintf(stderr, "quadrics: repair: %s %s\n", typeid(e).name(), e.GetMessageString());
    }
    if (ok)
    {
      TopoDS_Solid sol;
      bld.MakeSolid(sol);
      bld.Add(sol, fixed.Oriented(old_shell.Orientation()));
      wall = sol;
      lap("repair");
    }
  }
  if (BRepCheck_Analyzer wan(wall); !wan.IsValid())
  {
    if (const char* dir = std::getenv("NOSHAPE_SHELL_DUMP"))
      BRepTools::Write(wall, (std::string(dir) + "/quadrics-invalid.brep").c_str());
    if (dbg)
      for (const TopAbs_ShapeEnum k : {TopAbs_VERTEX, TopAbs_EDGE, TopAbs_WIRE, TopAbs_FACE, TopAbs_SHELL})
        for (TopExp_Explorer x(wall, k); x.More(); x.Next())
        {
          const occ::handle<BRepCheck_Result> r = wan.Result(x.Current());
          if (r.IsNull())
            continue;
          Bnd_Box bb;
          BRepBndLib::Add(x.Current(), bb);
          const gp_Pnt c = bb.IsVoid() ? gp_Pnt() : gp_Pnt((bb.CornerMin().XYZ() + bb.CornerMax().XYZ()) / 2);
          for (const BRepCheck_Status st : r->Status())
            if (st != BRepCheck_NoError)
              std::fprintf(stderr, "quadrics: kind %d status %d at %g %g %g\n", (int)k, (int)st, c.X(), c.Y(), c.Z());
          for (r->InitContextIterator(); r->MoreShapeInContext(); r->NextShapeInContext())
            for (const BRepCheck_Status st : r->StatusOnShape())
              if (st != BRepCheck_NoError)
              {
                Bnd_Box cb;
                BRepBndLib::Add(r->ContextualShape(), cb);
                const gp_Pnt cc = cb.IsVoid() ? gp_Pnt() : gp_Pnt((cb.CornerMin().XYZ() + cb.CornerMax().XYZ()) / 2);
                const int    surf = r->ContextualShape().ShapeType() == TopAbs_FACE ? (int)BRepAdaptor_Surface(TopoDS::Face(r->ContextualShape())).GetType() : -1;
                std::fprintf(stderr, "quadrics: kind %d status %d in context (%d, surface %d at %.3f %.3f %.3f mm) at %g %g %g\n", (int)k, (int)st, (int)r->ContextualShape().ShapeType(), surf, cc.X() * 1e3, cc.Y() * 1e3, cc.Z() * 1e3, c.X(), c.Y(), c.Z());
              }
        }
    return no("the wall is invalid");
  }
  lap("check");
  {
    // Only where an offset takes part: the part's own shapes can touch within their tolerances (Shell 4's two
    // vertices 0.1 mm apart, one 0.1 mm loose), and checking them took Shell 4 0.5 s.
    ShapeIMap in;
    TopExp::MapShapes(so, in);
    quad::NewPairsCheckerSI        checker(in);
    NCollection_List<TopoDS_Shape> args;
    args.Append(wall);
    checker.SetArguments(args);
    checker.SetNonDestructive(true);
    checker.SetRunParallel(true);
    // (Up to edges against faces: an offset crossing another shows as the one's edge through the other; the faces'
    // own intersections, B-spline offsets', were most of the check: c2-rings-loft's 0.7 s.)
    checker.SetLevelOfCheck(4);
    checker.Perform();
    const BOPDS_DS& ds       = *checker.PDS();
    bool            crossing = false;
    for (NCollection_Map<BOPDS_Pair>::Iterator it(ds.Interferences()); it.More(); it.Next())
    {
      int n1, n2;
      it.Value().Indices(n1, n2);
      if (ds.IsNewShape(n1) || ds.IsNewShape(n2) || (in.Contains(ds.Shape(n1)) && in.Contains(ds.Shape(n2))))
        continue;
      crossing = true;
      if (dbg)
        for (const int n : {n1, n2})
        {
          Bnd_Box bb;
          BRepBndLib::Add(ds.Shape(n), bb);
          const gp_Pnt c = bb.IsVoid() ? gp_Pnt() : gp_Pnt((bb.CornerMin().XYZ() + bb.CornerMax().XYZ()) / 2);
          std::fprintf(stderr, "quadrics: interference [type %d at %.3f %.3f %.3f]\n", (int)ds.Shape(n).ShapeType(), c.X() * 1e3, c.Y() * 1e3, c.Z() * 1e3);
        }
    }
    if (crossing)
      return no("the offsets cross each other or the part");
  }
  lap("self-interference");
  const double vw = rough_volume_of(wall), vs = rough_volume_of(so);
  lap("volumes");
  if (dbg)
    std::fprintf(stderr, "quadrics: wall volume %.17g solid %.17g\n", vw, vs);
  if (!(vw > 0) || !(outward || vw <= vs * (1 + 1e-3)))
    return no("the wall's volume is off");
  // A repaired wall no thinner than it should be: a point half a wall in from each kept face, where it is in the part,
  // is in the wall (core's review: a piece the repair dropped wrongly leaves a valid wall too small).
  if (!h2.IsNull())
  {
    BRepClass3d_SolidClassifier in_part(so), in_wall(wall);
    for (int i = 1; i <= nf; ++i)
    {
      if (!kept[(size_t)i])
        continue;
      const TopoDS_Face& f = TopoDS::Face(faces(i));
      gp_Pnt             q;
      gp_Pnt2d           uv;
      if (BOPTools_AlgoTools3D::PointInFace(f, q, uv, new IntTools_Context()) != 0)
        continue;
      const gp_XYZ p = q.XYZ() - normal_at(f, q).XYZ() * (0.5 * thickness);
      in_part.Perform(gp_Pnt(p), 1e3 * tol);
      if (in_part.State() != TopAbs_IN)
        continue;
      in_wall.Perform(gp_Pnt(p), 1e3 * tol);
      if (in_wall.State() == TopAbs_OUT)
      {
        if (dbg)
          std::fprintf(stderr, "quadrics: half a wall in from face %d, at %.4f %.4f %.4f, not in the wall\n", i, p.X() * 1e3, p.Y() * 1e3, p.Z() * 1e3);
        return no("a repaired wall thinner than the wall");
      }
    }
  }
  out.result  = wall;
  out.history = shell_history(solid, wall, {h, h2}, {}, sources, removed);
  return true;
}

// Parasolid bounds an opening's cavity by the removed face's own plane, extended past its edges: a kept face meeting a
// removed planar face along a concave edge keeps no wall behind that plane (live shell-hang Shell 5: the pocket
// behind its 45° opening runs on over the face beside it, up to the top's wall, and a wall's thickness past the
// opening's end). The kept face's slab, less what lies behind each such opening within a wall of its outline; the
// slab's faces keep their sources.
TopoDS_Shape clip_behind_openings(const TopoDS_Shape& slab, const TopoDS_Face& f, const ShapeAncMap& ef, const ShapeIMap& rm,
                                  double t, double size, SourceMap& sources)
{
  TopoDS_Shape r = slab;
  GProp_GProps g;
  BRepGProp::SurfaceProperties(f, g);
  const gp_Pnt c = g.CentreOfMass();
  ShapeIMap    seen;
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    const int k = ef.FindIndex(x.Current());
    if (k == 0)
      continue;
    for (ShapeList::Iterator it(ef(k)); it.More(); it.Next())
    {
      if (!rm.Contains(it.Value()) || seen.Contains(it.Value()))
        continue;
      seen.Add(it.Value());
      const TopoDS_Face& rf = TopoDS::Face(it.Value());
      BRepAdaptor_Surface a(rf, false);
      if (a.GetType() != GeomAbs_Plane)
        continue;
      gp_Pln pl = a.Plane();
      gp_Dir n  = pl.Axis().Direction();
      if (rf.Orientation() == TopAbs_REVERSED)
        n.Reverse();
      // Concave: the kept face lies in front of the removed face's plane.
      if (gp_Vec(pl.Location(), c).Dot(gp_Vec(n)) <= 1e-7 * size)
        continue;
      // A wall deep behind the opening (its box in its plane), and a wall past its edges shared with this face only
      // (live keeps Shell 5's corner past its opening's other edge).
      gp_Ax3 ax(pl.Location(), n);
      auto   box2 = [&](const TopoDS_Shape& sh) {
        std::array<double, 4> b{1e300, -1e300, 1e300, -1e300};
        for (TopExp_Explorer ee(sh, TopAbs_EDGE); ee.More(); ee.Next())
        {
          BRepAdaptor_Curve ec(TopoDS::Edge(ee.Current()));
          for (int q = 0; q <= 16; ++q)
          {
            const gp_Pnt pt = ec.Value(ec.FirstParameter() + (ec.LastParameter() - ec.FirstParameter()) * q / 16.0);
            const gp_Vec rel(ax.Location(), pt);
            const double u = rel.Dot(gp_Vec(ax.XDirection())), v = rel.Dot(gp_Vec(ax.YDirection()));
            b = {std::min(b[0], u), std::max(b[1], u), std::min(b[2], v), std::max(b[3], v)};
          }
        }
        return b;
      };
      const std::array<double, 4> rb = box2(rf);
      TopoDS_Compound             common_edges;
      BRep_Builder().MakeCompound(common_edges);
      for (TopExp_Explorer fe(f, TopAbs_EDGE); fe.More(); fe.Next())
        for (TopExp_Explorer re(rf, TopAbs_EDGE); re.More(); re.Next())
          if (fe.Current().IsSame(re.Current()))
            BRep_Builder().Add(common_edges, fe.Current());
      const std::array<double, 4> eb  = box2(common_edges);
      const double                tol = 1e-6 * (1 + size);
      const double                u0  = rb[0] - (eb[0] < rb[0] + tol ? t : 0), u1 = rb[1] + (eb[1] > rb[1] - tol ? t : 0);
      const double                v0  = rb[2] - (eb[2] < rb[2] + tol ? t : 0), v1 = rb[3] + (eb[3] > rb[3] - tol ? t : 0);
      TopoDS_Face                 mouth = BRepBuilderAPI_MakeFace(gp_Pln(ax), u0, u1, v0, v1).Face();
      TopoDS_Shape behind = BRepPrimAPI_MakePrism(mouth, gp_Vec(n) * -t).Shape();
      BRepAlgoAPI_Cut cm(r, behind);
      if (!cm.IsDone() || solid_count(cm.Shape()) != 1)
        continue;
      const TopoDS_Shape clipped = single_solid(cm.Shape());
      for (TopExp_Explorer y(r, TopAbs_FACE); y.More(); y.Next())
      {
        const Source* src = sources.Seek(y.Current());
        if (!src)
          continue;
        for (ShapeList::Iterator m(cm.Modified(y.Current())); m.More(); m.Next())
          sources.Bind(m.Value(), *src);
      }
      if (std::getenv("NOSHAPE_SHELL_DEBUG"))
        std::fprintf(stderr, "slabs: a slab clipped at a removed face's plane (%.6g -> %.6g)\n", volume_of(r), volume_of(clipped));
      r = clipped;
    }
  }
  return r;
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
  Bnd_Box whole;
  BRepBndLib::Add(solid, whole);
  const double size = std::sqrt(whole.SquareExtent()) * 2 + 10 * t;
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
      // A piece of a split closed face can come out invalid (82ac: a sunk boss's half, its split edge's curve out
      // of range: OCCT's simple offset throws), and then had no slab, the wall left out (live keeps it): fixed, again.
      if (s.IsNull() && closed && !BRepCheck_Analyzer(p).IsValid() && !std::getenv("NOSHAPE_SHELL_NO_PIECE_FIX"))
      {
        if (std::getenv("NOSHAPE_SHELL_DEBUG"))
          std::fprintf(stderr, "slabs: an invalid piece of a closed face fixed\n");
        try
        {
          ShapeFix_Shape fx(p);
          fx.Perform();
          if (fx.Shape().ShapeType() == TopAbs_FACE && BRepCheck_Analyzer(fx.Shape()).IsValid())
            s = slab(TopoDS::Face(fx.Shape()), d, f, sources);
        }
        catch (const Standard_Failure&)
        {
        }
      }
      if (!s.IsNull() && sign < 0 && !std::getenv("NOSHAPE_SHELL_NO_OPENING_CLIP"))
      {
        try
        {
          s = clip_behind_openings(s, f, ef, rm, t, size, sources);
        }
        catch (const Standard_Failure&)
        {
        }
      }
      // A slab that can't be built otherwise is a convex blend thinner than the wall: its neighbours' slabs cover it.
      if (!s.IsNull())
        tools.Append(s);
      else if (std::getenv("NOSHAPE_SHELL_DEBUG"))
      {
        std::fprintf(stderr, "slabs: no slab for a face of type %d (area %.6g)\n", (int)BRepAdaptor_Surface(p).GetType(), [&] { GProp_GProps g; BRepGProp::SurfaceProperties(p, g); return g.Mass(); }());
        if (const char* dir = std::getenv("NOSHAPE_SHELL_DUMP_TOOLS"))
        {
          static std::atomic<int> nf{0};
          BRepTools::Write(p, (std::string(dir) + "/noslab" + std::to_string(nf++) + ".brep").c_str());
        }
      }
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
  // Each on a copy of its own, sharing nothing with the part or each other (the provenance of its faces with it):
  // the shell corpus's 211 shells as before (outcomes and volumes to 1e-9), 15 % faster in all, the slowest fuses
  // 1.5-1.8x (SHELL-SLOW).
  {
    ShapeList copies;
    SourceMap moved;
    for (ShapeList::Iterator it(tools); it.More(); it.Next())
    {
      BRepBuilderAPI_Copy cp(it.Value(), true, false);
      ShapeIMap           faces;
      TopExp::MapShapes(it.Value(), TopAbs_FACE, faces);
      for (int k = 1; k <= faces.Extent(); ++k)
        if (const Source* src = sources.Seek(faces(k)))
          moved.Bind(cp.ModifiedShape(faces(k)), *src);
      copies.Append(cp.Shape());
    }
    for (SourceMap::Iterator it(sources); it.More(); it.Next())
      if (!moved.IsBound(it.Key()))
        moved.Bind(it.Key(), it.Value());
    tools   = copies;
    sources = moved;
  }
  // Each tool's tolerances its real ones (OCCT's constructions leave nominal ones: faces at 1e-5 to 1e-4 whose
  // real deviation is 1e-7, which made the fuse "a tolerant input" for noshape's Boolean and widened OCCT's), where
  // it shares nothing with the part (no tolerance of the part changes).
  {
    ShapeIMap part;
    TopExp::MapShapes(solid, TopAbs_VERTEX, part);
    for (ShapeList::Iterator it(tools); it.More(); it.Next())
    {
      bool shared = false;
      for (TopExp_Explorer v(it.Value(), TopAbs_VERTEX); v.More() && !shared; v.Next())
        shared = part.Contains(v.Current());
      if (!shared)
        BRepLib::UpdateTolerances(it.Value(), true);
    }
  }
  if (dbg)
  {
    // Each tool's validity and signed volume (a tool turned inside out spoils the fuse).
    int k = 0;
    for (ShapeList::Iterator it(tools); it.More(); it.Next(), ++k)
    {
      const bool   ok = BRepCheck_Analyzer(it.Value()).IsValid();
      const double v  = volume_of(it.Value());
      if (std::getenv("NOSHAPE_SHELL_DUMP_TOOLS"))
        BRepTools::Write(it.Value(), (std::string(std::getenv("NOSHAPE_SHELL_DUMP_TOOLS")) + "/tool" + std::to_string(k) + ".brep").c_str());
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
  TopoDS_Shape r;
  HHist        hb;
  HHist        hc; // the pocket's cut (the fused slabs' faces -> the pocket's), when the pocket is cut out
  // Very many slabs, inward: the cavity grown from the openings, the slabs meeting it fused, what lies beyond them and
  // touches a removed face, again with the slabs meeting that, until none is new; then cut out of the solid (with
  // every slab in, the regions reaching no opening too: voids). A part thinner than two walls away from its openings
  // (shell-hang Shell 5: live hollows a 2890 mm³ pocket behind its opening, every other face as it was) never fuses
  // the far slabs (all 128 of Shell 5's took 22 s and gave an invalid common). A deep region far from every opening
  // that no slab near the cavity reaches stays solid.
  // (Only past 112 slabs: 82ac's 102, its cavity most of the part, fuse at once in 7 s; grown, 10.5 s.)
  const bool pocket = sign < 0 && tools.Extent() > 112 && !std::getenv("NOSHAPE_SHELL_NO_POCKET");
  if (pocket)
  {
    std::vector<TopoDS_Shape> all;
    std::vector<Bnd_Box>      boxes;
    for (ShapeList::Iterator it(tools); it.More(); it.Next())
    {
      all.push_back(it.Value());
      Bnd_Box b;
      BRepBndLib::Add(it.Value(), b);
      boxes.push_back(b);
    }
    std::vector<bool> used(all.size(), false);
    Bnd_Box              reach;
    std::vector<Bnd_Box> removed_boxes(removed.size());
    for (size_t k = 0; k < removed.size(); ++k)
    {
      BRepBndLib::Add(removed[k], reach);
      BRepBndLib::Add(removed[k], removed_boxes[k]);
      removed_boxes[k].Enlarge(1e-6 * (1 + t));
    }
    reach.Enlarge(t);
    ShapeList open;
    for (int round = 0; round < 20; ++round)
    {
      bool grew = false;
      for (size_t k = 0; k < all.size(); ++k)
        if (!used[k] && !boxes[k].IsOut(reach))
          used[k] = grew = true;
      if (!grew && round > 0)
        break;
      ShapeList sel;
      for (size_t k = 0; k < all.size(); ++k)
        if (used[k])
          sel.Append(all[k]);
      if (sel.IsEmpty())
        return false;
      TopoDS_Shape u = sel.First();
      HHist        uh;
      if (sel.Extent() > 1)
      {
        ShapeList first, rest = sel;
        first.Append(rest.First());
        rest.RemoveFirst();
        if (!quick_boolean(0, first, rest, bop_parallel(), u, uh))
          return false;
      }
      lap("pocket fuse");
      if (dbg)
        std::fprintf(stderr, "slabs: pocket union %.6g, %d solids, valid %d\n", volume_of(u), solid_count(u), (int)BRepCheck_Analyzer(u).IsValid());
      ShapeList a1, a2;
      a1.Append(solid);
      a2.Append(u);
      TopoDS_Shape deep;
      HHist        dh;
      if (!quick_boolean(1, a1, a2, bop_parallel(), deep, dh))
        return false;
      lap("pocket cut");
      open.Clear();
      Bnd_Box    next;
      const bool every = std::all_of(used.begin(), used.end(), [](bool b) { return b; });
      for (TopExp_Explorer e(deep, TopAbs_SOLID); e.More(); e.Next())
      {
        // With every slab in, the regions that reach no opening are voids, as Parasolid makes them (82ac: three
        // shells).
        // (Slivers between slabs, a hundredth of a wall's cube, are no regions: 0.06 mm³ ones on Shell 5 at 3 mm.)
        if (volume_of(e.Current()) < 1e-2 * t * t * t)
          continue;
        // Open: a piece of a removed face bounds it (a point inside one of its faces lies on one); a region meeting
        // a removed face only along an edge or at a vertex stays solid (shell-hang Shell 5's 18 mm³ corner, live).
        bool touches = every;
        for (TopExp_Explorer ff(e.Current(), TopAbs_FACE); ff.More() && !touches; ff.Next())
        {
          gp_Pnt p;
          gp_Dir n;
          if (!inner_point(TopoDS::Face(ff.Current()), p, n))
            continue;
          const TopoDS_Vertex v = BRepBuilderAPI_MakeVertex(p).Vertex();
          for (size_t k = 0; k < removed.size() && !touches; ++k)
          {
            if (removed_boxes[k].IsOut(p))
              continue;
            BRepExtrema_DistShapeShape dd(v, removed[k]);
            touches = dd.IsDone() && dd.Value() < 1e-7 * (1 + t);
          }
        }
        if (dbg)
          std::fprintf(stderr, "slabs: pocket region %.6g, %s\n", volume_of(e.Current()), touches ? "open" : "closed");
        if (touches)
        {
          open.Append(e.Current());
          BRepBndLib::Add(e.Current(), next);
        }
      }
      if (dbg)
        std::fprintf(stderr, "slabs: pocket round %d: %d of %d slabs, %d open regions\n", round, sel.Extent(), tools.Extent(), open.Extent());
      union_ = u;
      hu = uh;
      hc = dh;
      if (open.IsEmpty())
        return false;
      next.Enlarge(1e-6 * (1 + t));
      reach = next;
    }
    lap("pocket regions");
    ShapeList a1;
    a1.Append(solid);
    TopoDS_Shape cut;
    HHist        ch;
    if (!quick_boolean(1, a1, open, bop_parallel(), cut, ch))
      return false;
    r  = cut;
    hb = ch;
    lap("pocket out");
  }
  else if (tools.Extent() == 1)
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
  if (dbg)
    std::fprintf(stderr, "slabs: %d tools, union volume %.6g\n", tools.Extent(), volume_of(union_));
  if (!pocket && sign < 0)
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
  else if (!pocket)
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
  if (dbg)
    std::fprintf(stderr, "slabs: common volume %.6g valid %d\n", volume_of(r), (int)valid_solid(r));
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
  // Inward, every kept face stays whole: the result's area is at least theirs (82ac's sibling ca43c447: only four
  // pieces of tube walls came out of the fuse, 258 mm³ of a 0.1 mm skin over 32000 mm²).
  if (sign < 0)
  {
    auto area = [](const TopoDS_Shape& x) {
      GProp_GProps g;
      BRepGProp::SurfaceProperties(x, g);
      return g.Mass();
    };
    double kept = area(solid);
    for (const TopoDS_Shape& f : removed)
      kept -= area(f);
    if (area(r) < kept * (1 - 1e-6))
    {
      if (std::getenv("NOSHAPE_SHELL_DEBUG"))
        std::fprintf(stderr, "slabs: result area %.9g under the kept faces' %.9g\n", area(r), kept);
      return false;
    }
  }
  out.result = r;
  out.history = hc.IsNull() ? shell_history(solid, r, {hb, hs}, {hu, hb, hs}, sources, removed) : shell_history(solid, r, {hb, hs}, {hu, hc, hb, hs}, sources, removed);
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
  if (const char* dir = std::getenv("NOSHAPE_SHELL_DUMP"))
  {
    BRepTools::Write(mine.result, (std::string(dir) + "/compare-ours.brep").c_str());
    BRepTools::Write(slabs.result, (std::string(dir) + "/compare-slabs.brep").c_str());
  }
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

// A hollow (no face removed) inward past the part's half (live 1765: a 40 mm cube hollowed 25 mm leaves 63000 mm³,
// a 10 mm void): Parasolid offsets every face and the offset body comes back turned inside out, every edge reversed;
// that inverted polytope is taken out as a void. Planar parts with every vertex of three faces, the inversion complete
// (every offset edge reversed); else not this case.
bool by_inversion(const TopoDS_Shape& solid, double t, Built& out)
{
  ShapeIMap faces, verts, edges;
  TopExp::MapShapes(solid, TopAbs_FACE, faces);
  TopExp::MapShapes(solid, TopAbs_VERTEX, verts);
  TopExp::MapShapes(solid, TopAbs_EDGE, edges);
  std::vector<gp_Pln> off(faces.Extent() + 1);
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    const TopoDS_Face& f = TopoDS::Face(faces(i));
    BRepAdaptor_Surface s(f);
    if (s.GetType() != GeomAbs_Plane)
      return false;
    int wires = 0;
    for (TopoDS_Iterator w(f); w.More(); w.Next())
      ++wires;
    if (wires != 1)
      return false;
    gp_Pln pl = s.Plane();
    gp_Dir n  = pl.Axis().Direction();
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    off[(size_t)i] = gp_Pln(pl.Location().Translated(gp_Vec(n) * -t), n);
  }
  ShapeAncMap vf;
  TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_VERTEX, TopAbs_FACE, vf);
  std::vector<gp_Pnt> p(verts.Extent() + 1);
  for (int i = 1; i <= verts.Extent(); ++i)
  {
    const ShapeList& fl = vf.FindFromKey(verts(i));
    if (fl.Extent() != 3)
      return false;
    gp_Mat m;
    gp_XYZ rhs;
    int    r = 1;
    for (ShapeList::Iterator it(fl); it.More(); it.Next(), ++r)
    {
      const gp_Pln& q = off[(size_t)faces.FindIndex(it.Value())];
      const gp_XYZ  n = q.Axis().Direction().XYZ();
      m.SetRow(r, n);
      rhs.SetCoord(r, n.Dot(q.Location().XYZ()));
    }
    if (std::abs(m.Determinant()) < 1e-12)
      return false;
    p[(size_t)i] = gp_Pnt(m.Inverted() * rhs);
  }
  for (int i = 1; i <= edges.Extent(); ++i)
  {
    TopoDS_Vertex a, b;
    TopExp::Vertices(TopoDS::Edge(edges(i)), a, b);
    const gp_Vec d0(BRep_Tool::Pnt(a), BRep_Tool::Pnt(b)), d1(p[(size_t)verts.FindIndex(a)], p[(size_t)verts.FindIndex(b)]);
    if (!(d0.Dot(d1) < 0) || d1.Magnitude() < 1e-9)
      return false;
  }
  // The inverted polytope: each face's polygon on its offset plane, sewn.
  BRepBuilderAPI_Sewing sew(1e-7);
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    BRepBuilderAPI_MakePolygon poly;
    for (BRepTools_WireExplorer w(BRepTools::OuterWire(TopoDS::Face(faces(i))), TopoDS::Face(faces(i))); w.More(); w.Next())
      poly.Add(p[(size_t)verts.FindIndex(w.CurrentVertex())]);
    poly.Close();
    if (!poly.IsDone())
      return false;
    BRepBuilderAPI_MakeFace mf(off[(size_t)i], poly.Wire(), true);
    if (!mf.IsDone())
      return false;
    sew.Add(mf.Face());
  }
  sew.Perform();
  TopoDS_Shell sh;
  for (TopExp_Explorer x(sew.SewedShape(), TopAbs_SHELL); x.More(); x.Next())
    sh = TopoDS::Shell(x.Current());
  if (sh.IsNull() || !BRep_Tool::IsClosed(sh))
    return false;
  TopoDS_Solid inv = BRepBuilderAPI_MakeSolid(sh).Solid();
  BRepLib::OrientClosedSolid(inv);
  if (!BRepCheck_Analyzer(inv).IsValid() || !(volume_of(inv) > 0))
    return false;
  BRepAlgoAPI_Cut cut(solid, inv);
  if (!cut.IsDone() || cut.HasErrors())
    return false;
  const TopoDS_Shape r = cut.Shape();
  if (std::abs(volume_of(r) - (volume_of(solid) - volume_of(inv))) > 1e-9 * volume_of(solid) || !BRepCheck_Analyzer(r).IsValid())
    return false;
  out.result  = r;
  out.history = cut.History();
  return true;
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
  // Parasolid refuses to shell a solid with a void ("Could not shell part with selections."), a plain block with a
  // box or cylinder void too, a face removed or none (live 1753 / 1757).
  if (!outward)
    for (TopExp_Explorer x(solid.shape, TopAbs_SOLID); x.More(); x.Next())
    {
      int shells = 0;
      for (TopoDS_Iterator it(x.Current(), false); it.More(); it.Next())
        shells += it.Value().ShapeType() == TopAbs_SHELL;
      if (shells > 1)
        fail_op("shell failed (Parasolid can't shell a solid with a void: live 1757)");
    }
  if (!outward && !removed.empty())
  {
    const std::vector<bool> shallow = shallow_openings(solid.shape, removed, thickness);
    if (std::all_of(shallow.begin(), shallow.end(), [](bool b) { return b; }))
      fail_op("shell failed (Parasolid has no cavity to open behind the removed faces: live 1794)");
    std::vector<TopoDS_Shape> open;
    for (size_t i = 0; i < removed.size(); ++i)
      if (!shallow[i])
        open.push_back(removed[i]);
    removed = open;
  }
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
  ShapeIMap removed_set;
  for (const TopoDS_Shape& f : removed)
    removed_set.Add(f);
  const bool offset_ok = gap > 1e-5 && !collapses && !low_strip(solid.shape, removed_set, thickness);
  // Free-form faces meeting at sharp edges (a twisted hexagon vase's sides): OCCT's offset spends seconds intersecting
  // their offsets (2.6-3 s of 3.7 on the vase, 2 mm), the direct construction builds the same wall in 1.4-2.4 s and
  // nearer live's (live 1769: 47868.377 / OCCT 47868.366 / live 47868.386 mm³): it goes first there.
  bool sharp_freeform = false;
  {
    ShapeAncMap ef;
    TopExp::MapShapesAndUniqueAncestors(solid.shape, TopAbs_EDGE, TopAbs_FACE, ef);
    auto freeform = [](const TopoDS_Shape& f) {
      const GeomAbs_SurfaceType k = BRepAdaptor_Surface(TopoDS::Face(f)).GetType();
      return k == GeomAbs_BSplineSurface || k == GeomAbs_BezierSurface || k == GeomAbs_OffsetSurface || k == GeomAbs_OtherSurface;
    };
    for (int i = 1; i <= ef.Extent() && !sharp_freeform; ++i)
    {
      const ShapeList& fl = ef(i);
      if (fl.Extent() != 2 || fl.First().IsSame(fl.Last()) || !freeform(fl.First()) || !freeform(fl.Last()))
        continue;
      BRepAdaptor_Curve c(TopoDS::Edge(ef.FindKey(i)));
      const gp_Pnt      m = c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
      sharp_freeform    = normal_at(TopoDS::Face(fl.First()), m).Angle(normal_at(TopoDS::Face(fl.Last()), m)) > 1e-3;
    }
  }
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
  bool       quadrics_tried = false; // (its second place in the order: not again once failed)
  const bool done = (method == 0 && removed.empty() && !outward && timed("inversion", [&] { return by_inversion(solid.shape, thickness, b); })) ||
                    (method == 0 && timed("regions", [&] { return by_regions(solid.shape, removed, thickness, sign, b); })) ||
                    (method == 0 && !std::getenv("NOSHAPE_SHELL_NO_SMOOTH") && timed("smooth", [&] { return by_smooth(solid.shape, removed, thickness, sign, b); })) ||
                    (method == 0 && sharp_freeform && !std::getenv("NOSHAPE_SHELL_NO_QUADRICS") && timed("quadrics", [&] { return quadrics_tried = true, by_quadrics(solid.shape, removed, thickness, sign, b); })) ||
                    ((method == 0 || method == 1) && (offset_ok || method == 1) && timed("offset", [&] { return by_offset(solid.shape, removed, thickness, sign, b); })) ||
                    ((method == 0 || method == 3) && !std::getenv("NOSHAPE_SHELL_NO_PLANES") && timed("planes", [&] {
                       const bool ok = by_planes(solid.shape, removed, thickness, sign, b);
                       if (ok && std::getenv("NOSHAPE_SHELL_COMPARE"))
                         compare_with_slabs(solid.shape, removed, thickness, sign, b);
                       return ok;
                     })) ||
                    ((method == 0 || method == 4) && !quadrics_tried && !std::getenv("NOSHAPE_SHELL_NO_QUADRICS") && timed("quadrics", [&] {
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
