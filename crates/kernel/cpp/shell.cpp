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

#include <BRepAdaptor_Curve.hxx>
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
#include <gp_Lin.hxx>
#include <ElSLib.hxx>
#include <BRepOffset_Analyse.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
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

namespace noshape {

namespace {

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
  TopoDS_Shape offset;
  SourceMap    sources;
  try
  {
    BRepOffset_MakeOffset mo;
    mo.Initialize(solid, sign * t, 1e-6, BRepOffset_Skin, false, false, GeomAbs_Intersection);
    for (const TopoDS_Shape& f : removed)
      mo.SetOffsetOnFace(TopoDS::Face(f), 0.0);
    mo.MakeOffsetShape();
    if (!mo.IsDone())
      return false;
    offset = solid_of(mo.Shape());
    if (offset.IsNull())
      return false;
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
    return false;
  }
  const TopoDS_Shape& outer = sign < 0 ? solid : offset;
  const TopoDS_Shape& inner = sign < 0 ? offset : solid;
  BRepAlgoAPI_Cut     cut(outer, inner);
  if (cut.HasErrors() || !cut.IsDone())
    return false;
  TopoDS_Shape r = cut.Shape();
  if (!valid_solid(r))
    return false;
  // A correct offset lies inside the outer solid: the volumes must add up
  // (catches self-intersecting offsets that BRepCheck accepts).
  const double vo = volume_of(outer), vi = volume_of(inner), vr = volume_of(r);
  if (!(vi > 0 && vr > 0 && vi < vo && std::abs(vo - vi - vr) <= 1e-6 * vo))
    return false;
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
        return false;
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

// Path 0, inward walls of convex solids bounded by planes and convex
// cylinders (fillets) with planar faces removed: the cavity is where every
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
  if (sign > 0)
    return false;
  ShapeIMap rm;
  for (const TopoDS_Shape& f : removed)
  {
    if (BRepAdaptor_Surface(TopoDS::Face(f), false).GetType() != GeomAbs_Plane)
      return false;
    rm.Add(f);
  }
  // Convex: no concave edge.
  try
  {
    BRepOffset_Analyse an(solid, 0.01);
    for (TopExp_Explorer e(solid, TopAbs_EDGE); e.More(); e.Next())
      for (const BRepOffset_Interval& iv : an.Type(TopoDS::Edge(e.Current())))
        if (iv.Type() != ChFiDS_Convex && iv.Type() != ChFiDS_Tangential)
          return false;
  }
  catch (const Standard_Failure&)
  {
    return false;
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
    BRepAlgoAPI_Cut c(cavity, tool);
    if (c.HasErrors() || !c.IsDone())
      return false;
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
  std::vector<std::pair<TopoDS_Face, gp_Cylinder>> cylinders;
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
    if (a.GetType() != GeomAbs_Plane)
      return false;
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
    const gp_Pnt o  = mid.Translated(gp_Vec(n) * gp_Vec(mid, on).Dot(gp_Vec(n)));
    gp_Ax3       ax(o, n);
    gp_Pnt       c0 = o.Translated(gp_Vec(ax.XDirection()) * -big + gp_Vec(ax.YDirection()) * -big);
    TopoDS_Shape hs = BRepPrimAPI_MakeBox(gp_Ax2(c0, n, ax.XDirection()), 2 * big, 2 * big, big).Shape();
    SourceMap    src;
    for (TopExp_Explorer x(hs, TopAbs_FACE); x.More(); x.Next())
    {
      BRepAdaptor_Surface s(TopoDS::Face(x.Current()), false);
      if (s.Plane().Axis().Direction().IsParallel(n, 1e-9) && std::abs(gp_Vec(o, s.Plane().Location()).Dot(gp_Vec(n))) < 1e-9 * big)
        src.Bind(x.Current(), Source{f, (uint8_t)REL_GENERATED});
    }
    if (!cut_by(hs, src))
      return false;
  }
  for (const auto& [f, cyl] : cylinders)
  {
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
    if (n.Dot(radial) <= 0 || u1 - u0 >= M_PI - 1e-6)
      return false;
    // The sector [u0, u1] beyond radius r - t, over the face's axial extent
    // and 2t more each way: a prism of the sector polygon minus the cylinder.
    const double L  = big;
    auto         at = [&](double u, double rad, double v) { return ElSLib::Value(u, v, gp_Cylinder(ax, rad)); };
    const double va = v0 - margin, vb = v1 + margin, um = 0.5 * (u0 + u1), wide = L / std::cos(0.5 * (u1 - u0));
    std::vector<gp_Pnt> base{at(u0, 0, va), at(u0, L, va), at(um, wide, va), at(u1, L, va)};
    BRepBuilderAPI_MakePolygon poly;
    for (const gp_Pnt& q : base)
      poly.Add(q);
    poly.Close();
    TopoDS_Shape sector = BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(poly.Wire(), true).Face(), gp_Vec(ax.Direction()) * (vb - va)).Shape();
    // A wall at least as thick as the fillet's radius: its offset vanishes
    // and the whole sector goes.
    TopoDS_Shape region = sector;
    if (r - t > 1e-9 * r)
    {
      gp_Ax2          cax(ElSLib::Value(0, va - 1, gp_Cylinder(ax, 0)), ax.Direction(), ax.XDirection());
      TopoDS_Shape    inner = BRepPrimAPI_MakeCylinder(cax, r - t, vb - va + 2).Shape();
      BRepAlgoAPI_Cut k(sector, inner);
      if (k.HasErrors() || !k.IsDone())
        return false;
      region = k.Shape();
    }
    SourceMap src;
    for (TopExp_Explorer x(region, TopAbs_FACE); x.More(); x.Next())
      src.Bind(x.Current(), Source{f, (uint8_t)REL_GENERATED});
    if (!cut_by(region, src))
      return false;
  }
  BRepAlgoAPI_Cut c(solid, cavity);
  if (c.HasErrors() || !c.IsDone())
    return false;
  TopoDS_Shape r = single_solid(c.Shape());
  if (!valid_solid(r) || solid_count(r) != 1)
    return false;
  const double vo = volume_of(solid), vr = volume_of(r);
  if (!(vr > 0 && vr < vo))
    return false;
  const HHist hc = c.History();
  out.result     = r;
  out.history    = shell_history(solid, r, {hc}, {hc}, sources, {});
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
TopoDS_Shape slab(const TopoDS_Face& f, double d, const TopoDS_Shape& src, SourceMap& sources)
{
  gp_Pnt p;
  gp_Dir n;
  if (!inner_point(f, p, n))
    return TopoDS_Shape();
  BRepAdaptor_Surface s(f);
  TopoDS_Shape        r;
  if (s.GetType() == GeomAbs_Plane)
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
    for (double sg : {1.0, -1.0})
    {
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

// Path 2. `sign` is -1 for an inward wall, +1 outward.
bool by_slabs(const TopoDS_Shape& solid, const std::vector<TopoDS_Shape>& removed, double t,
              double sign, Built& out)
{
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
      if (m.Dot(mean) > 0)
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
  if (tools.IsEmpty())
    return false;
  TopoDS_Shape union_;
  HHist        hu;
  if (tools.Extent() == 1)
    union_ = tools.First();
  else
  {
    ShapeList first, rest = tools;
    first.Append(rest.First());
    rest.RemoveFirst();
    BRepAlgoAPI_Fuse fu;
    fu.SetArguments(first);
    fu.SetTools(rest);
    fu.SetRunParallel(true);
    fu.Build();
    if (fu.HasErrors() || !fu.IsDone())
      return false;
    union_ = fu.Shape();
    hu = fu.History();
  }
  TopoDS_Shape r;
  HHist        hb;
  if (sign < 0)
  {
    BRepAlgoAPI_Common co(solid, union_);
    if (co.HasErrors() || !co.IsDone())
      return false;
    r = co.Shape();
    hb = co.History();
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
    BRepAlgoAPI_Cut cu;
    cu.SetArguments(objs);
    cu.SetTools(tls);
    cu.Build();
    if (cu.HasErrors() || !cu.IsDone())
      return false;
    r = cu.Shape();
    hb = cu.History();
  }
  // Merge the faces split up by the slab construction.
  ShapeUpgrade_UnifySameDomain usd(r, true, true, false);
  usd.SetLinearTolerance(1e-7);
  usd.Build();
  HHist hs = usd.History();
  r = single_solid(usd.Shape());
  if (!valid_solid(r))
    return false;
  const double vr = volume_of(r);
  if (!(vr > 0) || (sign < 0 && !(vr < volume_of(solid))))
    return false;
  out.result = r;
  out.history = shell_history(solid, r, {hb, hs}, {hu, hb, hs}, sources, removed);
  return true;
}

} // namespace

OpOut op_shell_solid(const OcShape& solid, const OcShapeList& faces, double thickness, bool outward,
                     uint8_t method)
{
  require_non_null(solid.shape, "solid");
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
  const double gap = outward ? std::numeric_limits<double>::infinity() : offsets_touch_gap(solid.shape, removed, thickness);
  if (gap <= 1e-8)
    fail_op("shell failed (Parasolid can't build walls whose offsets touch)");
  // Walls that nearly touch (within ten times OCCT's offset tolerance) merge
  // or part by less than it sees: its offset keeps both faces whole and
  // overlapping (2.501 mm: 0.003 mm³ off). The slab construction trims them.
  const bool offset_ok = gap > 1e-5;
  Built      b;
  // method: 0 = automatic, 1 = offset only, 2 = slabs only (tests).
  const bool done = (method == 0 && by_regions(solid.shape, removed, thickness, sign, b)) ||
                    (method != 2 && (offset_ok || method == 1) && by_offset(solid.shape, removed, thickness, sign, b)) ||
                    (method != 1 && by_slabs(solid.shape, removed, thickness, sign, b));
  if (!done)
    fail_op("shell failed");
  return make_result({solid.shape}, b.result, b.history.get());
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
  BRepAlgoAPI_Common c(a, b);
  if (c.HasErrors() || !c.IsDone())
    return TopoDS_Shape();
  TopoDS_Shape r = c.Shape();
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
OpOut op_draft_wedges(const OcShape& solid, const OcShapeList& faces, P3 pull, const PlaneF& neutral,
                      double angle)
{
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
      TopoDS_Shape in  = BRepPrimAPI_MakePrism(f, gp_Vec(n) * -D).Shape();
      TopoDS_Shape out = BRepPrimAPI_MakePrism(f, gp_Vec(n) * D).Shape();
      TopoDS_Shape wr = common_of(in, half_box(Q0, n2, size));
      TopoDS_Shape wa = common_of(out, half_box(Q0, n2.Reversed(), size));
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
    BRepAlgoAPI_Cut cu;
    cu.SetArguments(args);
    cu.SetTools(removal);
    cu.SetRunParallel(true);
    cu.Build();
    if (cu.HasErrors() || !cu.IsDone())
      fail_op("draft fallback: cut failed");
    r = cu.Shape();
    chain.push_back(cu.History());
  }
  if (!addition.IsEmpty())
  {
    ShapeList args;
    args.Append(r);
    BRepAlgoAPI_Fuse fu;
    fu.SetArguments(args);
    fu.SetTools(addition);
    fu.SetRunParallel(true);
    fu.Build();
    if (fu.HasErrors() || !fu.IsDone())
      fail_op("draft fallback: fuse failed");
    r = fu.Shape();
    chain.push_back(fu.History());
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

} // namespace noshape
