// Feature operations: prism, revolve, sweep, loft, thicken, shell, fillet,
// chamfer, draft, defeaturing, offsets, sewing, filling, rib.
#include "internal.hpp"
#include <optional>
#include <gp_Cylinder.hxx>
#include <gp_Lin.hxx>
#include <NCollection_Array2.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <Precision.hxx>
#include <ShapeFix_Edge.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <ShapeUpgrade_ShapeDivideClosed.hxx>

#include <BRepAdaptor_Surface.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Defeaturing.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBndLib.hxx>
#include <NCollection_HArray1.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <BRepAdaptor_CompCurve.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepFeat_MakeLinearForm.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <limits>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BRepLib.hxx>
#include <BRepLib_FindSurface.hxx>
#include <BRepOffsetAPI_DraftAngle.hxx>
#include <BRepOffsetAPI_MakeFilling.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakePipe.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <gp_Pln.hxx>
#include <TopTools_HSequenceOfShape.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <GeomConvert_ApproxCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <memory>
#include <BRepAlgoAPI_Fuse.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepTools.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <Geom_Plane.hxx>
#include <Law_Linear.hxx>
#include <NCollection_Array1.hxx>
#include <ShapeFix_FixSmallFace.hxx>
#include <ShapeFix_Solid.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp_Ax1.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Trsf2d.hxx>
#include <Geom2d_Curve.hxx>
#include <gp_Trsf.hxx>

namespace noshape {

namespace {

using HHist = occ::handle<BRepTools_History>;

template <class Algo>
HHist history_of(const std::vector<TopoDS_Shape>& args, Algo& algo)
{
  return new BRepTools_History(to_list(args), algo);
}

// Merge `next` into `acc` (acc may be null -> becomes next).
void merge_into(HHist& acc, const HHist& next)
{
  if (acc.IsNull())
    acc = next;
  else
    acc->Merge(next);
}

void check_sweepable(const TopoDS_Shape& s)
{
  require_non_null(s, "profile");
  switch (s.ShapeType())
  {
    case TopAbs_FACE:
    case TopAbs_WIRE:
    case TopAbs_EDGE:
    case TopAbs_VERTEX:
    case TopAbs_SHELL:
    case TopAbs_COMPOUND:
      return;
    default:
      fail_invalid("profile must be a face, shell, wire, edge, vertex or compound of those");
  }
}

// Planar profile plane (for draft neutral plane etc.).
bool profile_plane(const TopoDS_Shape& s, gp_Pln& out)
{
  BRepLib_FindSurface fs(s, 1e-6, true);
  if (!fs.Found())
    return false;
  occ::handle<Geom_Plane> pl = occ::down_cast<Geom_Plane>(fs.Surface());
  if (pl.IsNull())
    return false;
  out = pl->Pln();
  if (!fs.Location().IsIdentity())
    out.Transform(fs.Location().Transformation());
  return true;
}

double bbox_diag(const std::vector<TopoDS_Shape>& shapes)
{
  Bnd_Box b;
  for (const auto& s : shapes)
    if (!s.IsNull())
      BRepBndLib::Add(s, b, false);
  if (b.IsVoid())
    return 1.0;
  return std::sqrt(b.SquareExtent());
}

TopoDS_Edge first_adjacent_face_edge_check(const TopoDS_Shape& s, const TopoDS_Shape& e,
                                           const char* what)
{
  const TopoDS_Edge& edge = as_edge(e, what);
  ShapeIMap          m;
  TopExp::MapShapes(s, TopAbs_EDGE, m);
  if (!m.Contains(edge))
    fail_invalid(std::string(what) + " does not belong to the shape");
  return edge;
}

void check_faces_in(const TopoDS_Shape& s, const std::vector<TopoDS_Shape>& faces)
{
  ShapeIMap m;
  TopExp::MapShapes(s, TopAbs_FACE, m);
  for (const auto& f : faces)
  {
    as_face(f, "face");
    if (!m.Contains(f))
      fail_invalid("face does not belong to the shape");
  }
}

TopoDS_Wire outer_or_wire(const TopoDS_Shape& s, const char* what)
{
  require_non_null(s, what);
  if (s.ShapeType() == TopAbs_FACE)
    return BRepTools::OuterWire(TopoDS::Face(s));
  return as_wire(s, what);
}

int count_wires(const TopoDS_Shape& f)
{
  int n = 0;
  for (TopExp_Explorer e(f, TopAbs_WIRE); e.More(); e.Next())
    ++n;
  return n;
}

// Rebuild a closed wire so that it starts at the vertex closest to `p`.
TopoDS_Wire rotate_wire_start(const TopoDS_Wire& w, const gp_Pnt& p)
{
  std::vector<TopoDS_Edge>   edges;
  std::vector<TopoDS_Vertex> starts;
  for (BRepTools_WireExplorer ex(w); ex.More(); ex.Next())
  {
    edges.push_back(ex.Current());
    starts.push_back(ex.CurrentVertex());
  }
  if (edges.size() < 2)
    return w;
  size_t best = 0;
  double bd   = 1e300;
  for (size_t i = 0; i < starts.size(); ++i)
  {
    double d = BRep_Tool::Pnt(starts[i]).Distance(p);
    if (d < bd)
    {
      bd   = d;
      best = i;
    }
  }
  if (best == 0)
    return w;
  BRepBuilderAPI_MakeWire mk;
  for (size_t k = 0; k < edges.size(); ++k)
    mk.Add(edges[(best + k) % edges.size()]);
  if (!mk.IsDone())
    return w;
  TopoDS_Wire r = mk.Wire();
  r.Orientation(w.Orientation() == TopAbs_REVERSED ? TopAbs_FORWARD : TopAbs_FORWARD);
  return r;
}

std::vector<TopoDS_Shape> images_vec(const HHist& h, const TopoDS_Shape& s)
{
  std::vector<TopoDS_Shape> v;
  ShapeList                 l = images_of(h, s);
  for (ShapeList::Iterator it(l); it.More(); it.Next())
    v.push_back(it.Value());
  return v;
}

// History of a BRepTools_Modifier-based algorithm (DraftAngle, GTransform):
// every input sub-shape maps to ModifiedShape(s).
template <class Algo>
HHist modifier_history(const TopoDS_Shape& input, Algo& algo)
{
  HHist h = new BRepTools_History();
  for (TopAbs_ShapeEnum k : {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
  {
    ShapeIMap m;
    TopExp::MapShapes(input, k, m);
    for (int i = 1; i <= m.Extent(); ++i)
    {
      TopoDS_Shape r;
      try
      {
        r = algo.ModifiedShape(m(i));
      }
      catch (const Standard_Failure&)
      {
        continue;
      }
      if (!r.IsNull() && !r.IsSame(m(i)) && r.ShapeType() == k)
        h->AddModified(m(i), r);
    }
  }
  return h;
}

ShapeList single(const TopoDS_Shape& s)
{
  ShapeList l;
  if (!s.IsNull())
    l.Append(s);
  return l;
}

// The full circles of a profile restarted at gp_Ax2's default x for
// `z_new` (the frame frame_cylinders_toward_sketch gives their cylinders):
// the swept seam then sits at the new frame's u = 0, as an OCCT primitive's
// (a seam left at u = pi made the booleans of a part with its mirror image
// wrong). Same circle, same sense; the old edges and vertices are recorded
// in `rs`.
TopoDS_Shape seat_circles(const TopoDS_Shape& base, const gp_Dir& z_new, occ::handle<BRepTools_ReShape>& rs)
{
  const gp_Dir x_new = gp_Ax2(gp_Pnt(0, 0, 0), z_new).XDirection();
  ShapeIMap    edges;
  TopExp::MapShapes(base, TopAbs_EDGE, edges);
  bool any = false;
  for (int i = 1; i <= edges.Extent(); ++i)
  {
    const TopoDS_Edge e = TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     v0, v1;
    TopExp::Vertices(e, v0, v1);
    if (v0.IsNull() || !v0.IsSame(v1) || BRep_Tool::Degenerated(e))
      continue;
    BRepAdaptor_Curve c(e);
    if (c.GetType() != GeomAbs_Circle || std::abs(c.LastParameter() - c.FirstParameter() - 2 * M_PI) > 1e-9)
      continue;
    const gp_Circ ci = c.Circle();
    if (!ci.Axis().Direction().IsParallel(z_new, 1e-9))
      continue;
    // Already starting there.
    if (gp_Vec(ci.Location(), BRep_Tool::Pnt(v0)).Normalized().IsEqual(gp_Vec(x_new), 1e-12, 1e-12))
      continue;
    BRepBuilderAPI_MakeEdge mk(gp_Circ(gp_Ax2(ci.Location(), ci.Axis().Direction(), x_new), ci.Radius()));
    if (!mk.IsDone())
      continue;
    TopoDS_Edge ne = mk.Edge();
    BRep_Builder().UpdateEdge(ne, std::max(BRep_Tool::Tolerance(e), Precision::Confusion()));
    rs->Replace(e, ne);
    rs->Replace(v0.Oriented(TopAbs_FORWARD), TopExp::FirstVertex(ne).Oriented(TopAbs_FORWARD));
    any = true;
  }
  return any ? rs->Apply(base) : base;
}

// Parasolid's frame of an extruded cylinder (live "fs (10-26 b)"): on the
// axis at the far end, z back toward the sketch, x gp_Ax2's default for that
// z (live (a) z -z x -x, (b) z +z x +x, (e) z +y x +z). OCCT's prism takes
// the profile circle's frame, z along the sketch normal. Faces whose axis
// points along `away` (OCCT's: -sweep, and the prism is swept back) are
// re-framed in place with the same handedness: with z reversed a point's
// angle u becomes a - u (a = the old x's angle in the new frame) and v
// becomes -v, a half turn of the (u, v) plane, so each pcurve turns by pi
// and the face keeps its orientation.
// Re-frames cylinder face `f` in place to Parasolid's frame: z = `z_world`
// (the old axis or its reverse), x gp_Ax2's default for that z, origin
// `origin_world` (on the axis), the old frame's handedness kept (so the
// normal is). A point's (u, v) maps to (u + a, v + h) when z stays, to
// (a - u, h - v) when it flips (a half turn of the (u, v) plane): the pcurves
// are moved accordingly, the u range made to start in [0, 2 pi). False when
// `f` isn't such a cylinder.
bool reframe_cylinder(const TopoDS_Face& f, const gp_Dir& z_world, const gp_Pnt& origin_world)
{
  BRep_Builder                          B;
  TopLoc_Location                       loc;
  occ::handle<Geom_CylindricalSurface> cyl = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(f, loc));
  if (cyl.IsNull())
    return false;
  const gp_Ax3  old      = cyl->Position();
  const gp_Trsf to_world = loc.Transformation();
  const gp_Trsf to_local = to_world.Inverted();
  const double  s        = old.Direction().Transformed(to_world).Dot(z_world);
  if (std::abs(std::abs(s) - 1) > 1e-9)
    return false;
  const bool    flip  = s < 0;
  const gp_Dir  x_new = gp_Ax2(gp_Pnt(0, 0, 0), z_world).XDirection();
  const gp_Pnt  o_new = origin_world.Transformed(to_local);
  gp_Ax3        nw(o_new, z_world.Transformed(to_local), x_new.Transformed(to_local));
  if (!old.Direct())
    nw.YReverse();
  const gp_Dir xo = old.XDirection();
  const double a  = std::atan2(xo.Dot(nw.YDirection()), xo.Dot(nw.XDirection()));
  // The old origin's height on the new axis.
  const double h = gp_Vec(o_new, old.Location()).Dot(gp_Vec(nw.Direction()));
  double       u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  const double two_pi = 2 * M_PI;
  gp_Trsf2d    t;
  if (flip)
  {
    // u' = a - u: the new range [a - u1, a - u0] starts in [0, 2 pi).
    const double ap = u1 + std::fmod(std::fmod(a - u1, two_pi) + two_pi, two_pi);
    t.SetRotation(gp_Pnt2d(ap / 2, h / 2), M_PI);
  }
  else
  {
    // u' = u + a: the new range [u0 + a, u1 + a] starts in [0, 2 pi).
    const double ap = std::fmod(std::fmod(u0 + a, two_pi) + two_pi, two_pi) - u0;
    t.SetTranslation(gp_Vec2d(ap, h));
  }
  occ::handle<Geom_CylindricalSurface> ns = new Geom_CylindricalSurface(nw, cyl->Radius());
  ShapeIMap                            done;
  // Pcurves as stored (a reversed face swaps a seam's two).
  const TopoDS_Face                    ff = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
  for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
  {
    if (done.Contains(e.Current()))
      continue;
    done.Add(e.Current());
    const TopoDS_Edge          fwd = TopoDS::Edge(e.Current().Oriented(TopAbs_FORWARD));
    double                     a1, b1;
    occ::handle<Geom2d_Curve> c1 = BRep_Tool::CurveOnSurface(fwd, ff, a1, b1);
    if (c1.IsNull())
      fail_op("cylinder face without a pcurve");
    occ::handle<Geom2d_Curve> n1  = occ::down_cast<Geom2d_Curve>(c1->Transformed(t));
    const double               tol = BRep_Tool::Tolerance(fwd);
    if (BRep_Tool::IsClosed(fwd, f))
    {
      double                     a2, b2;
      occ::handle<Geom2d_Curve> c2 = BRep_Tool::CurveOnSurface(TopoDS::Edge(fwd.Reversed()), ff, a2, b2);
      occ::handle<Geom2d_Curve> n2 = occ::down_cast<Geom2d_Curve>(c2->Transformed(t));
      B.UpdateEdge(fwd, n1, n2, ns, loc, tol);
    }
    else
      B.UpdateEdge(fwd, n1, ns, loc, tol);
    B.Range(fwd, ns, loc, a1, b1);
  }
  B.UpdateFace(f, ns, loc, BRep_Tool::Tolerance(f));
  return true;
}

void frame_cylinders_toward_sketch(const TopoDS_Shape& shape, const gp_Dir& away)
{
  ShapeIMap faces;
  TopExp::MapShapes(shape, TopAbs_FACE, faces);
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    const TopoDS_Face&                    f = TopoDS::Face(faces(i));
    TopLoc_Location                       loc;
    occ::handle<Geom_CylindricalSurface> cyl = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(f, loc));
    // OCCT's frame points along the sweep: z = -sweep, indirect when the
    // circle's axis opposes it. Its origin (the profile circle's centre at
    // the far end) stays.
    if (cyl.IsNull() || !(cyl->Position().Direction().Transformed(loc.Transformation()).Dot(away) > 1 - 1e-9))
      continue;
    reframe_cylinder(f, away.Reversed(), cyl->Position().Location().Transformed(loc.Transformation()));
  }
}

// The axial extent [lo, hi] of face `f` along `ax` (from its edges).
std::pair<double, double> axial_extent(const TopoDS_Face& f, const gp_Ax1& ax)
{
  double lo = 1e300, hi = -1e300;
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    if (BRep_Tool::Degenerated(TopoDS::Edge(x.Current())))
      continue;
    BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
    for (int k = 0; k <= 8; ++k)
    {
      const double h = gp_Vec(ax.Location(), c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / 8)).Dot(gp_Vec(ax.Direction()));
      lo = std::min(lo, h);
      hi = std::max(hi, h);
    }
  }
  return {lo, hi};
}

} // namespace

// ---------------------------------------------------------------- prism

OpOut op_prism(const OcShape& profile, P3 start, P3 end, double draft_angle)
{
  check_sweepable(profile.shape);
  gp_Vec vs = to_vec(start), ve = to_vec(end);
  gp_Vec d  = ve - vs;
  if (d.Magnitude() < 1e-9)
    fail_invalid("extrude length must be non-zero");
  std::vector<TopoDS_Shape> args{profile.shape};
  HHist                     hist;
  TopoDS_Shape              base = profile.shape;
  std::unique_ptr<BRepBuilderAPI_Transform> tr;
  // Parasolid sweeps an extrude from its far end back toward the sketch
  // (live "fs (10-26 b)"): the side edges run toward the sketch (a block's
  // vertical edges −z) and a cylinder's frame sits at the far end cap, z
  // toward the sketch, which std's patterns about an edge and tangent planes
  // follow (oracle CP2, P6). So the profile goes to the far end and is swept
  // back (a draft keeps the sketch end: its neutral plane).
  const bool   back = std::abs(draft_angle) <= 1e-12;
  const gp_Vec from = back ? ve : vs;
  // Swept back, the cylinders are re-framed in place (frame_cylinders_
  // toward_sketch adds pcurves to their edges): the prism must not share
  // edges with the caller's profile (a sketch region other threads read:
  // qa BOOL-THREAD-CRASH), so the profile is copied.
  if (from.Magnitude() > 1e-12 || back)
  {
    gp_Trsf t;
    t.SetTranslation(from);
    tr = std::make_unique<BRepBuilderAPI_Transform>(base, t, back);
    if (!tr->IsDone())
      fail_op("could not translate profile");
    base = tr->Shape();
    merge_into(hist, history_of(args, *tr));
  }
  occ::handle<BRepTools_ReShape> seat;
  if (back)
  {
    seat = new BRepTools_ReShape;
    base = seat_circles(base, gp_Dir(-d), seat);
    merge_into(hist, seat->History());
  }
  BRepPrimAPI_MakePrism prism(base, back ? -d : d, false, true);
  if (!prism.IsDone())
    fail_op("extrude failed");
  TopoDS_Shape result = prism.Shape();
  merge_into(hist, history_of({base}, prism));
  if (back)
    frame_cylinders_toward_sketch(result, gp_Dir(d));

  HHist post; // history after the prism (draft)
  if (std::abs(draft_angle) > 1e-12)
  {
    if (std::abs(draft_angle) >= M_PI / 2.0)
      fail_invalid("draft angle must be less than 90 degrees");
    gp_Pln pln;
    if (!profile_plane(base, pln))
      fail_invalid("draft extrude requires a planar profile");
    gp_Dir                   pull(d);
    BRepOffsetAPI_DraftAngle draft(result);
    ShapeIMap                sides;
    for (TopExp_Explorer e(base, TopAbs_EDGE); e.More(); e.Next())
      for (ShapeList::Iterator it(prism.Generated(e.Current())); it.More(); it.Next())
        if (it.Value().ShapeType() == TopAbs_FACE)
          sides.Add(it.Value());
    if (sides.Extent() == 0)
      fail_invalid("draft extrude requires a face or closed wire profile");
    for (int i = 1; i <= sides.Extent(); ++i)
    {
      // Positive angle narrows the profile along the extrude direction.
      draft.Add(TopoDS::Face(sides(i)), pull, draft_angle, pln, true);
      if (!draft.AddDone())
        fail_op("draft could not be applied to a side face");
    }
    draft.Build();
    if (!draft.IsDone())
      fail_op("draft extrude failed");
    post = modifier_history(result, draft);
    merge_into(hist, post);
    result = draft.Shape();
  }

  HistoryBuilder hb(args, result);
  hb.add_history(hist.get());
  auto moved = [&](const TopoDS_Shape& s) -> TopoDS_Shape {
    const TopoDS_Shape m = tr ? tr->ModifiedShape(s) : s;
    return seat.IsNull() ? m : seat->Value(m);
  };
  // The start cap is at the sketch end (the prism's last when swept back).
  hb.add_mapped(0, REL_FIRST, [&](const TopoDS_Shape& s) {
    return images_of(post, back ? prism.LastShape(moved(s)) : prism.FirstShape(moved(s)));
  });
  hb.add_mapped(0, REL_LAST, [&](const TopoDS_Shape& s) {
    return images_of(post, back ? prism.FirstShape(moved(s)) : prism.LastShape(moved(s)));
  });
  return hb.finish();
}

OpOut op_prism_until(const OcShape& profile, P3 dirp, const OcShape& until, double start_offset)
{
  check_sweepable(profile.shape);
  require_non_null(until.shape, "up-to target");
  gp_Dir                    dir  = to_dir(dirp, "extrude direction");
  std::vector<TopoDS_Shape> args{profile.shape, until.shape};
  HHist                     hist;
  TopoDS_Shape              base = profile.shape;
  std::unique_ptr<BRepBuilderAPI_Transform> tr;
  // Copied: its cylinders are re-framed in place below (see op_prism).
  {
    gp_Trsf t;
    t.SetTranslation(gp_Vec(dir) * start_offset);
    tr = std::make_unique<BRepBuilderAPI_Transform>(base, t, true);
    base = tr->Shape();
    merge_into(hist, history_of({profile.shape}, *tr));
  }
  // The profile's circles keep their start (no seat_circles as in op_prism):
  // seated, a boss up to a cylinder lying along x put its seam on the
  // cylinder's seam line and the fuse lost 0.035 mm³ of it (kernel_robustness
  // patterns_match_onshape M3).
  // Through-all length: generous multiple of the combined bounding box.
  const double          len = 2.0 * bbox_diag({base, until.shape}) + 1.0;
  BRepPrimAPI_MakePrism prism(base, gp_Vec(dir) * len, false, true);
  if (!prism.IsDone())
    fail_op("extrude failed");
  const TopoDS_Shape longPrism = prism.Shape();
  merge_into(hist, history_of({base}, prism));

  // Split the long prism by the target and keep the pieces touching the start cap.
  BRepAlgoAPI_Splitter sp;
  sp.SetArguments(single(longPrism));
  sp.SetTools(single(until.shape));
  sp.SetRunParallel(bop_parallel());
  sp.SetNonDestructive(true);
  sp.Build();
  if (sp.HasErrors())
    fail_op("up-to extrude: split failed: " + algo_errors(sp));
  HHist spH = history_of({longPrism, until.shape}, sp);
  merge_into(hist, spH);

  ShapeIMap startFaces, endFaces, targetFaces;
  for (TopExp_Explorer e(prism.FirstShape(), TopAbs_FACE); e.More(); e.Next())
    for (const TopoDS_Shape& img : images_vec(spH, e.Current()))
      startFaces.Add(img);
  for (TopExp_Explorer e(prism.LastShape(), TopAbs_FACE); e.More(); e.Next())
    for (const TopoDS_Shape& img : images_vec(spH, e.Current()))
      endFaces.Add(img);
  for (TopExp_Explorer e(until.shape, TopAbs_FACE); e.More(); e.Next())
    for (const TopoDS_Shape& img : images_vec(spH, e.Current()))
      targetFaces.Add(img);

  std::vector<TopoDS_Shape> keep;
  bool                      reachesEnd = false;
  for (TopExp_Explorer e(sp.Shape(), TopAbs_SOLID); e.More(); e.Next())
  {
    bool touchesStart = false, touchesEnd = false;
    for (TopExp_Explorer f(e.Current(), TopAbs_FACE); f.More(); f.Next())
    {
      if (startFaces.Contains(f.Current()))
        touchesStart = true;
      if (endFaces.Contains(f.Current()))
        touchesEnd = true;
    }
    if (touchesStart)
    {
      keep.push_back(e.Current());
      reachesEnd = reachesEnd || touchesEnd;
    }
  }
  if (keep.empty())
    fail_op("up-to extrude produced no solid (is the profile a face?)");
  if (reachesEnd)
    fail_op("up-to target does not fully bound the extruded profile");
  TopoDS_Shape result = keep.size() == 1 ? keep[0] : compound_of(keep);
  // Parasolid's frame of the extruded cylinders (live "core (2026-10-26)"
  // (3)): z back toward the sketch, x gp_Ax2's default, origin on the axis
  // 0.002 mm past the face's far end (Onshape extrudes blind to the target's
  // extent over the profile plus 2 um, then trims).
  {
    ShapeIMap faces;
    TopExp::MapShapes(result, TopAbs_FACE, faces);
    for (int i = 1; i <= faces.Extent(); ++i)
    {
      const TopoDS_Face&  f = TopoDS::Face(faces(i));
      BRepAdaptor_Surface fs(f, false);
      if (fs.GetType() != GeomAbs_Cylinder || !fs.Cylinder().Axis().Direction().IsParallel(dir, 1e-9))
        continue;
      const gp_Ax1 cax(fs.Cylinder().Location(), dir);
      const auto [lo, hi] = axial_extent(f, cax);
      reframe_cylinder(f, dir.Reversed(), cax.Location().Translated(gp_Vec(dir) * (hi + 2e-6)));
    }
  }

  HistoryBuilder hb(args, result);
  hb.add_history(hist.get());
  auto moved = [&](const TopoDS_Shape& s) -> TopoDS_Shape {
    return tr ? tr->ModifiedShape(s) : s;
  };
  hb.add_mapped(0, REL_FIRST, [&](const TopoDS_Shape& s) {
    return images_of(spH, prism.FirstShape(moved(s)));
  });
  // End cap(s): result faces that come from the target.
  std::vector<TopoDS_Shape> caps;
  for (TopExp_Explorer f(result, TopAbs_FACE); f.More(); f.Next())
    if (targetFaces.Contains(f.Current()))
      caps.push_back(f.Current());
  hb.add_cap(0, REL_LAST, compound_of(caps));
  return hb.finish();
}

// ---------------------------------------------------------------- revolve

OpOut op_revol(const OcShape& profile, P3 origin, P3 axis, double a0, double a1)
{
  check_sweepable(profile.shape);
  gp_Ax1 ax(to_pnt(origin), to_dir(axis, "revolve axis"));
  double sweep = a1 - a0;
  if (!(std::abs(sweep) > 1e-9))
    fail_invalid("revolve angle must be non-zero");
  const bool full = std::abs(sweep) >= 2.0 * M_PI - 1e-9;
  std::vector<TopoDS_Shape> args{profile.shape};
  HHist                     hist;
  TopoDS_Shape              base = profile.shape;
  std::unique_ptr<BRepBuilderAPI_Transform> tr;
  // Parasolid's revolved cylinders (live "core (2026-10-26)" (5), "fs (10-26
  // b)" (f)): z against the axis as drawn, x gp_Ax2's default, origin at
  // mid-height (re-framed below). A full turn keeps the profile where it is:
  // turning it to that x moved the seams other features are aligned with
  // (the tapping screws' crown sphere against its recess).
  const gp_Dir z_new = ax.Direction().Reversed();
  const double start = full ? 0.0 : a0;
  // Copied: its cylinders are re-framed in place below (see op_prism).
  {
    gp_Trsf t;
    t.SetRotation(ax, start);
    tr   = std::make_unique<BRepBuilderAPI_Transform>(base, t, true);
    base = tr->Shape();
    merge_into(hist, history_of(args, *tr));
  }
  std::unique_ptr<BRepPrimAPI_MakeRevol> rv;
  if (full)
    rv = std::make_unique<BRepPrimAPI_MakeRevol>(base, ax, false);
  else
    rv = std::make_unique<BRepPrimAPI_MakeRevol>(base, ax, sweep, false);
  if (!rv->IsDone())
    fail_op("revolve failed (does the profile cross the axis?)");
  TopoDS_Shape result = rv->Shape();
  merge_into(hist, history_of({base}, *rv));
  {
    ShapeIMap faces;
    TopExp::MapShapes(result, TopAbs_FACE, faces);
    const gp_Lin axis_line(ax);
    for (int i = 1; i <= faces.Extent(); ++i)
    {
      const TopoDS_Face&  f = TopoDS::Face(faces(i));
      BRepAdaptor_Surface fs(f, false);
      if (fs.GetType() != GeomAbs_Cylinder || !fs.Cylinder().Axis().IsParallel(ax, 1e-9)
          || axis_line.Distance(fs.Cylinder().Location()) > 1e-9 * (1 + fs.Cylinder().Radius()))
        continue;
      const auto [lo, hi] = axial_extent(f, ax);
      reframe_cylinder(f, z_new, ax.Location().Translated(gp_Vec(ax.Direction()) * ((lo + hi) / 2)));
    }
  }
  HistoryBuilder hb(args, result);
  hb.add_history(hist.get());
  if (!full)
  {
    auto moved = [&](const TopoDS_Shape& s) -> TopoDS_Shape {
      return tr ? tr->ModifiedShape(s) : s;
    };
    hb.add_mapped(0, REL_FIRST,
                  [&](const TopoDS_Shape& s) { return single(rv->FirstShape(moved(s))); });
    hb.add_mapped(0, REL_LAST,
                  [&](const TopoDS_Shape& s) { return single(rv->LastShape(moved(s))); });
  }
  return hb.finish();
}

// A copy of `s` whose cylinders coaxial with (`origin`, `axis`) take
// Parasolid's frame with z = `axis` and their origin at `origin` (a Hole's
// cylinders: at its start, z along the drill, live "fs (10-26 b)" (g)).
std::unique_ptr<OcShape> op_frame_cylinders_at(const OcShape& s, P3 origin, P3 axis)
{
  require_non_null(s.shape, "shape");
  const gp_Pnt o = to_pnt(origin);
  const gp_Dir z = to_dir(axis, "axis");
  BRepBuilderAPI_Copy cp(s.shape);
  const TopoDS_Shape  r = cp.Shape();
  const gp_Lin        line(o, z);
  ShapeIMap           faces;
  TopExp::MapShapes(r, TopAbs_FACE, faces);
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    const TopoDS_Face&  f = TopoDS::Face(faces(i));
    BRepAdaptor_Surface fs(f, false);
    if (fs.GetType() != GeomAbs_Cylinder || !fs.Cylinder().Axis().IsParallel(gp_Ax1(o, z), 1e-9)
        || line.Distance(fs.Cylinder().Location()) > 1e-9 * (1 + fs.Cylinder().Radius()))
      continue;
    reframe_cylinder(f, z, o);
  }
  return wrap(r);
}

// The shape with its closed (periodic) faces split once across their seams,
// with the history of the split. A boolean against a tool lying close
// along one whole periodic face can fail where it works on the halves (an
// elliptic boss's face blend); the halves share the face's surface, so a
// unify merges them back.
OpOut op_divide_closed(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  ShapeUpgrade_ShapeDivideClosed d(s.shape);
  d.SetNbSplitPoints(1);
  d.Perform();
  HHist h;
  if (!d.GetContext().IsNull())
    h = d.GetContext()->History();
  return make_result({s.shape}, d.Result(), h.get());
}

// ---------------------------------------------------------------- sweep

OpOut op_pipe(const OcShape& profile, const OcShape& spine)
{
  check_sweepable(profile.shape);
  TopoDS_Wire            sw = as_wire(spine.shape, "sweep path");
  BRepOffsetAPI_MakePipe mk(sw, profile.shape, GeomFill_IsCorrectedFrenet, false);
  mk.Build();
  if (!mk.IsDone())
    fail_op("sweep failed");
  std::vector<TopoDS_Shape> args{profile.shape, sw};
  HHist                     h = history_of(args, mk);
  HistoryBuilder            hb(args, mk.Shape());
  hb.add_history(h.get());
  hb.add_cap(0, REL_FIRST, mk.FirstShape());
  hb.add_cap(0, REL_LAST, mk.LastShape());
  return hb.finish();
}

// Whether every edge of the path is a helix: a straight, sloped line on a
// cylindrical or conical surface (as make_helix builds it).
static bool is_helix_path(const TopoDS_Wire& w)
{
  bool any = false;
  for (TopExp_Explorer e(w, TopAbs_EDGE); e.More(); e.Next())
  {
    occ::handle<Geom2d_Curve> pc;
    occ::handle<Geom_Surface> sf;
    TopLoc_Location           loc;
    double                    f, l;
    BRep_Tool::CurveOnSurface(TopoDS::Edge(e.Current()), pc, sf, loc, f, l);
    if (pc.IsNull() || sf.IsNull())
      return false;
    occ::handle<Geom2d_Line> line = occ::down_cast<Geom2d_Line>(pc);
    if (line.IsNull() || (occ::down_cast<Geom_CylindricalSurface>(sf).IsNull() && occ::down_cast<Geom_ConicalSurface>(sf).IsNull()))
      return false;
    const gp_Dir2d d = line->Direction();
    if (std::abs(d.X()) < 1e-9 || std::abs(d.Y()) < 1e-9)
      return false;
    any = true;
  }
  return any;
}

OpOut op_pipe_shell(const OcShapeList& profiles, const OcShape& spine, const SweepOpts& o,
                    const OcShape& aux)
{
  if (profiles.items.empty())
    fail_invalid("sweep needs at least one profile");
  TopoDS_Wire sw = as_wire(spine.shape, "sweep path");
  // Along a helix the minimal-twist frame is the Frenet frame: the profile
  // keeps its angle to the axis and ends as it started (a knurl's grooves end
  // flush with the part, live 224 faces). OCCT's corrected Frenet tilts it.
  const int mode = o.mode == 0 && is_helix_path(sw) ? 1 : o.mode;

  // A single face profile with holes: use BRepOffsetAPI_MakePipe (supports faces).
  if (profiles.items.size() == 1 && profiles.items[0].ShapeType() == TopAbs_FACE &&
      count_wires(profiles.items[0]) > 1)
  {
    if (std::abs(o.scale_end - 1.0) > 1e-12)
      fail_unsupported("scaled sweep of a face with holes");
    GeomFill_Trihedron tri;
    switch (mode)
    {
      case 0:
        tri = GeomFill_IsCorrectedFrenet;
        break;
      case 1:
        tri = GeomFill_IsFrenet;
        break;
      case 5:
        tri = GeomFill_IsDiscreteTrihedron;
        break;
      default:
        fail_unsupported("this sweep orientation mode with a face that has holes");
    }
    BRepOffsetAPI_MakePipe mk(sw, profiles.items[0], tri, false);
    mk.Build();
    if (!mk.IsDone())
      fail_op("sweep failed");
    std::vector<TopoDS_Shape> args{profiles.items[0], sw};
    HHist                     h = history_of(args, mk);
    HistoryBuilder            hb(args, mk.Shape());
    hb.add_history(h.get());
    hb.add_cap(0, REL_FIRST, mk.FirstShape());
    hb.add_cap(0, REL_LAST, mk.LastShape());
    return hb.finish();
  }

  if (mode == 6)
  {
    // Lock profile faces: the trihedron's binormal follows the faces' normal
    // (BRepFill_EdgeOnSurfLaw), so each path edge needs its curve on one of
    // them; added on a copy (same sub-shape order, so the history still maps).
    require_non_null(aux.shape, "faces to lock");
    sw = TopoDS::Wire(BRepBuilderAPI_Copy(sw).Shape());
    Bnd_Box sb;
    BRepBndLib::Add(sw, sb);
    const double tol = std::max(Precision::Confusion(), (sb.IsVoid() ? 1.0 : std::sqrt(sb.SquareExtent())) * 1e-6);
    for (TopExp_Explorer ee(sw, TopAbs_EDGE); ee.More(); ee.Next())
    {
      const TopoDS_Edge& e  = TopoDS::Edge(ee.Current());
      bool               on = false;
      for (TopExp_Explorer ef(aux.shape, TopAbs_FACE); ef.More() && !on; ef.Next())
      {
        const TopoDS_Face& f = TopoDS::Face(ef.Current());
        double             a, b;
        if (BRep_Tool::Curve(e, a, b).IsNull())
          continue;
        BRepAdaptor_Curve c(e);
        on = true;
        for (int i = 0; i <= 8 && on; ++i)
        {
          BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(c.Value(a + (b - a) * i / 8)).Vertex(), f);
          on = d.IsDone() && d.Value() <= tol;
        }
        if (on)
        {
          ShapeFix_Edge sfe;
          sfe.FixAddPCurve(e, f, false, tol);
          on = !BRep_Tool::CurveOnSurface(e, f, a, b).IsNull();
        }
      }
      if (!on)
        fail_invalid("the sweep path must lie on the faces to lock");
    }
  }
  BRepOffsetAPI_MakePipeShell mk(sw);
  {
    // OCCT's defaults (1e-4 absolute) are tuned for millimetre parts: in metres
    // that's 0.1 mm of approximation error on the swept surfaces.
    Bnd_Box sb;
    BRepBndLib::Add(sw, sb);
    const double size = sb.IsVoid() ? 1.0 : std::sqrt(sb.SquareExtent());
    mk.SetTolerance(std::min(1e-4, std::max(1e-9, size * 1e-6)), std::min(1e-4, std::max(1e-9, size * 1e-6)), 1e-2);
  }
  const bool twisted = std::abs(o.twist) > 1e-12;
  if (twisted)
  {
    // The profile's X direction follows an auxiliary curve winding round the
    // path: a rotation-minimizing frame (double reflection) turned by the
    // twist in proportion to arc length, at a distance of the profile's size.
    Bnd_Box pb;
    for (const auto& p : profiles.items)
      BRepBndLib::Add(p, pb);
    BRepAdaptor_CompCurve cc(sw);
    const double          L = GCPnts_AbscissaPoint::Length(cc);
    if (!(L > 1e-12))
      fail_invalid("sweep path has no length");
    const double delta = std::max(pb.IsVoid() ? 0.0 : std::sqrt(pb.SquareExtent()), L * 1e-2);
    const int    n     = std::max(32, (int)std::ceil(std::abs(o.twist) / (M_PI / 16.0)) + 16);
    NCollection_Array1<gp_Pnt> pts(1, n + 1);
    gp_Pnt                     P;
    gp_Vec                     T;
    cc.D1(cc.FirstParameter(), P, T);
    gp_Vec R = std::abs(T.Normalized().Z()) < 0.9 ? T.Crossed(gp_Vec(0, 0, 1)) : T.Crossed(gp_Vec(1, 0, 0));
    R.Normalize();
    gp_Pnt prevP = P;
    gp_Vec prevT = T.Normalized();
    for (int i = 0; i <= n; ++i)
    {
      const double s = L * i / n;
      GCPnts_AbscissaPoint ap(cc, s, cc.FirstParameter());
      const double         u = i == 0 ? cc.FirstParameter() : (i == n ? cc.LastParameter() : ap.Parameter());
      cc.D1(u, P, T);
      T.Normalize();
      if (i > 0)
      {
        // Double reflection (Wang et al. 2008).
        gp_Vec v1 = gp_Vec(prevP, P);
        double c1 = v1.SquareMagnitude();
        if (c1 > 1e-30)
        {
          gp_Vec rL = R - v1 * (2.0 / c1 * v1.Dot(R));
          gp_Vec tL = prevT - v1 * (2.0 / c1 * v1.Dot(prevT));
          gp_Vec v2 = T - tL;
          double c2 = v2.SquareMagnitude();
          R = c2 > 1e-30 ? rL - v2 * (2.0 / c2 * v2.Dot(rL)) : rL;
        }
        R = (R - T * R.Dot(T)).Normalized();
      }
      prevP = P;
      prevT = T;
      const double th = o.twist * s / L;
      const gp_Vec B  = T.Crossed(R);
      pts(i + 1) = P.Translated((R * std::cos(th) + B * std::sin(th)) * delta);
    }
    occ::handle<NCollection_HArray1<gp_Pnt>> hp = new NCollection_HArray1<gp_Pnt>(1, n + 1);
    for (int i = 1; i <= n + 1; ++i)
      hp->SetValue(i, pts(i));
    GeomAPI_Interpolate interp(hp, false, 1e-9);
    interp.Perform();
    if (!interp.IsDone())
      fail_op("sweep twist: auxiliary curve failed");
    TopoDS_Wire aw = BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(interp.Curve()).Edge()).Wire();
    mk.SetMode(aw, false, BRepFill_NoContact);
  }
  else
  switch (mode)
  {
    case 0:
      mk.SetMode(false);
      break;
    case 1:
      mk.SetMode(true);
      break;
    case 2:
      mk.SetMode(gp_Ax2(gp_Pnt(0, 0, 0), to_dir(o.dir, "fixed direction"),
                        to_dir(o.xdir, "fixed x direction")));
      break;
    case 3:
      mk.SetMode(to_dir(o.dir, "binormal direction"));
      break;
    case 4: {
      TopoDS_Wire aw = as_wire(aux.shape, "auxiliary spine");
      mk.SetMode(aw, true, o.with_contact ? BRepFill_Contact : BRepFill_NoContact);
      break;
    }
    case 5:
      mk.SetDiscreteMode();
      break;
    case 6:
      require_non_null(aux.shape, "spine support");
      if (!mk.SetMode(aux.shape))
        fail_invalid("invalid spine support surface");
      break;
    default:
      fail_invalid("unknown sweep mode");
  }
  switch (o.transition)
  {
    case 1:
      mk.SetTransitionMode(BRepBuilderAPI_Transformed);
      break;
    case 2:
      mk.SetTransitionMode(BRepBuilderAPI_RoundCorner);
      break;
    default:
      mk.SetTransitionMode(BRepBuilderAPI_Transformed);
      break;
  }
  bool                      anyClosed = false;
  std::vector<TopoDS_Shape> args;
  std::vector<TopoDS_Shape> used;
  for (const auto& p : profiles.items)
  {
    require_non_null(p, "profile");
    TopoDS_Shape prof;
    if (p.ShapeType() == TopAbs_VERTEX)
      prof = p;
    else
      prof = outer_or_wire(p, "profile");
    if (prof.ShapeType() == TopAbs_WIRE && BRep_Tool::IsClosed(prof))
      anyClosed = true;
    args.push_back(p);
    used.push_back(prof);
  }
  if (std::abs(o.scale_end - 1.0) > 1e-12)
  {
    if (used.size() != 1)
      fail_unsupported("scale law with multiple sweep sections");
    if (!(o.scale_end > 0))
      fail_invalid("sweep end scale must be positive");
    occ::handle<Law_Linear> law = new Law_Linear();
    law->Set(0.0, 1.0, 1.0, o.scale_end);
    mk.SetLaw(used[0], law, o.with_contact, o.with_correction);
  }
  else
  {
    for (const auto& p : used)
      mk.Add(p, o.with_contact, o.with_correction);
  }
  if (!mk.IsReady())
    fail_invalid("sweep is not ready (invalid profiles)");
  mk.Build();
  if (!mk.IsDone())
    fail_op("sweep failed (status " + std::to_string((int)mk.GetStatus()) + ")");
  if (o.make_solid && anyClosed)
  {
    if (!mk.MakeSolid())
      fail_op("sweep: could not make a solid");
  }
  // History is keyed on the wires actually swept; map faces of face profiles
  // through their outer wires (the wires are sub-shapes of the faces).
  std::vector<TopoDS_Shape> hargs = args;
  hargs.push_back(sw);
  HHist          h = history_of(hargs, mk);
  HistoryBuilder hb(hargs, mk.Shape());
  hb.add_history(h.get());
  hb.add_cap(0, REL_FIRST, mk.FirstShape());
  hb.add_cap((uint32_t)args.size() - 1, REL_LAST, mk.LastShape());
  return hb.finish();
}

// ---------------------------------------------------------------- loft

// Loft end conditions at the start and at the end (loft.fs
// createProfileConditions): kind 0 free, 1 along the vector, 2 in the plane
// normal to it; the magnitude; the vector (null: the profile's plane normal).
struct LoftEnds
{
  int    start     = 0;
  double start_mag = 0;
  gp_Vec start_dir;
  int    end       = 0;
  double end_mag   = 0;
  gp_Vec end_dir;
};

// Natural cubic spline through rows R[k] (each `dim` doubles) at parameters
// t[k], as piecewise Bezier poles: 3 (n - 1) + 1 of them. A non-empty `d0` /
// `dn` clamps the derivative at the start / end instead.
static std::vector<std::vector<double>> natural_bezier(const std::vector<std::vector<double>>& R,
                                                       const std::vector<double>&              t,
                                                       const std::vector<double>&              d0 = {},
                                                       const std::vector<double>&              dn = {})
{
  const size_t n = R.size(), dim = R[0].size();
  std::vector<double> h(n - 1);
  for (size_t k = 0; k + 1 < n; ++k)
    h[k] = t[k + 1] - t[k];
  // Tridiagonal system for the derivatives D[k], zero second derivative at the ends.
  std::vector<double> a(n, 0), b(n, 0), c(n, 0);
  std::vector<std::vector<double>> d(n, std::vector<double>(dim, 0));
  for (size_t k = 0; k < n; ++k)
  {
    if (k == 0 && !d0.empty())
    {
      b[k] = 1, c[k] = 0;
      d[k] = d0;
    }
    else if (k == 0)
    {
      b[k] = 2, c[k] = 1;
      for (size_t i = 0; i < dim; ++i)
        d[k][i] = 3 * (R[1][i] - R[0][i]) / h[0];
    }
    else if (k == n - 1 && !dn.empty())
    {
      a[k] = 0, b[k] = 1;
      d[k] = dn;
    }
    else if (k == n - 1)
    {
      a[k] = 1, b[k] = 2;
      for (size_t i = 0; i < dim; ++i)
        d[k][i] = 3 * (R[k][i] - R[k - 1][i]) / h[k - 1];
    }
    else
    {
      a[k] = 1 / h[k - 1], b[k] = 2 * (1 / h[k - 1] + 1 / h[k]), c[k] = 1 / h[k];
      for (size_t i = 0; i < dim; ++i)
        d[k][i] = 3 * ((R[k][i] - R[k - 1][i]) / (h[k - 1] * h[k - 1]) + (R[k + 1][i] - R[k][i]) / (h[k] * h[k]));
    }
  }
  for (size_t k = 1; k < n; ++k)
  {
    const double m = a[k] / b[k - 1];
    b[k] -= m * c[k - 1];
    for (size_t i = 0; i < dim; ++i)
      d[k][i] -= m * d[k - 1][i];
  }
  std::vector<std::vector<double>> D(n, std::vector<double>(dim));
  for (size_t k = n; k-- > 0;)
    for (size_t i = 0; i < dim; ++i)
      D[k][i] = (d[k][i] - (k + 1 < n ? c[k] * D[k + 1][i] : 0)) / b[k];
  std::vector<std::vector<double>> out;
  for (size_t k = 0; k + 1 < n; ++k)
  {
    std::vector<double> b1(dim), b2(dim);
    for (size_t i = 0; i < dim; ++i)
    {
      b1[i] = R[k][i] + h[k] * D[k][i] / 3;
      b2[i] = R[k + 1][i] - h[k] * D[k + 1][i] / 3;
    }
    out.push_back(R[k]);
    out.push_back(b1);
    out.push_back(b2);
  }
  out.push_back(R[n - 1]);
  return out;
}

// Lofts through three or more sections: Parasolid interpolates between the
// sections with a natural cubic spline at chord-length parameters (Onshape G10: Ø20 → square 12 → Ø20,
// 6125.607 mm³), OCCT with one polynomial (a parabola through three,
// 5917.6). Each lateral face of the loft spans every section; its surface is
// rebuilt in place from the section rows, so topology and history stay.
// Rows with differing weights (a circle and a square) are interpolated
// pointwise, as Parasolid does, through polynomial approximations (1e-8 of
// the part's size). False (shape untouched) if anything doesn't fit.
//
// End conditions clamp the columns' end derivatives. Live "psb (10-24 b)
// loft end derivative follow-up": with Normal to profile the first inner
// pole of a face's corner column sits m (0.9 h + 0.1 c) / 3 along the profile
// normal (h the column's spacing along it, c its chord), exact at every
// corner; between its two corners a face blends those values with a
// smoothstep (flat at the corners, as live's interior columns within 1e-3).
// With Tangent to profile the pole sits m / 3 times the chord's part in the
// profile plane away. A free end stays natural.
//
// `drop` (>= 0): a section that is only there to make ThruSections smooth
// (it lofts two sections ruled); the columns skip it.
static bool natural_loft(const TopoDS_Shape& loft, std::vector<TopoDS_Wire> wires, const LoftEnds& ends = {}, int drop = -1)
{
  // The surface's sections, and the ones the columns go through.
  const std::vector<TopoDS_Wire> all = wires;
  const int                      nts = (int)all.size();
  if (drop >= 0)
    wires.erase(wires.begin() + drop);
  const int n = (int)wires.size();
  Bnd_Box   box;
  BRepBndLib::Add(loft, box);
  const double size = box.IsVoid() ? 1.0 : std::sqrt(box.SquareExtent());
  const double tol = 1e-7 * size;
  // The end profiles' plane normals, pointing along the loft.
  auto centre = [](const TopoDS_Shape& w) {
    GProp_GProps g;
    BRepGProp::LinearProperties(w, g);
    return g.CentreOfMass();
  };
  auto normal_of = [&](int k, int toward, const gp_Vec& given, gp_Vec& out) {
    if (given.Magnitude() > 1e-12)
      out = given.Normalized();
    else
    {
      BRepLib_FindSurface fs(wires[k], 1e-6 * size, true);
      if (!fs.Found())
        return false;
      occ::handle<Geom_Plane> pl = occ::down_cast<Geom_Plane>(fs.Surface());
      if (pl.IsNull())
        return false;
      out = gp_Vec(pl->Position().Direction());
    }
    const gp_Vec along(centre(wires[k]), centre(wires[toward]));
    if (out.Dot(along) * (toward > k ? 1 : -1) < 0)
      out.Reverse();
    return true;
  };
  gp_Vec n0, n1;
  if (ends.start != 0 && !normal_of(0, 1, ends.start_dir, n0))
    return false;
  if (ends.end != 0 && !normal_of(n - 1, n - 2, ends.end_dir, n1))
    return false;
  struct Update
  {
    TopoDS_Face                      face;
    occ::handle<Geom_BSplineSurface> surface;
  };
  std::vector<Update>                                     faces;
  std::vector<std::pair<TopoDS_Edge, occ::handle<Geom_Curve>>> seams;
  for (TopExp_Explorer ex(loft, TopAbs_FACE); ex.More(); ex.Next())
  {
    const TopoDS_Face&        f = TopoDS::Face(ex.Current());
    TopLoc_Location           loc;
    occ::handle<Geom_Surface> gs = BRep_Tool::Surface(f, loc);
    auto                      bs = occ::down_cast<Geom_BSplineSurface>(gs);
    if (bs.IsNull())
      continue; // a cap
    if (!loc.IsIdentity() || bs->IsUPeriodic() || bs->IsVPeriodic())
      return false;
    double u0, u1, v0, v1;
    bs->Bounds(u0, u1, v0, v1);
    // ThruSections (iso-parametric) puts section k at v0 + k/(n-1) (v1 - v0).
    std::vector<occ::handle<Geom_BSplineCurve>> rows;
    for (int k = 0; k < nts; ++k)
    {
      auto row = occ::down_cast<Geom_BSplineCurve>(bs->VIso(v0 + (v1 - v0) * k / (nts - 1)));
      if (row.IsNull())
        return false;
      // (Loosely: ThruSections carries circles as polynomial approximations.)
      for (double s : {0.25, 0.5, 0.75})
      {
        BRepExtrema_DistShapeShape dist(BRepBuilderAPI_MakeVertex(row->Value(u0 + (u1 - u0) * s)).Vertex(), all[k]);
        if (!dist.IsDone() || dist.Value() > 1e-4 * size)
          return false;
      }
      rows.push_back(row);
    }
    if (drop >= 0)
      rows.erase(rows.begin() + drop);
    bool rational = false, same_weights = true;
    for (const auto& r : rows)
      rational = rational || r->IsRational();
    for (int k = 1; k < n && rational; ++k)
      for (int j = 1; j <= rows[0]->NbPoles(); ++j)
        if (std::abs(rows[k]->Weight(j) - rows[0]->Weight(j)) > 1e-12)
          same_weights = false;
    if (rational && !same_weights)
    {
      int degree = 0;
      for (auto& r : rows)
      {
        GeomConvert_ApproxCurve approx(r, 1e-8 * size, GeomAbs_C2, 200, 9);
        if (!approx.IsDone() || !approx.HasResult())
          return false;
        r      = approx.Curve();
        degree = std::max(degree, r->Degree());
      }
      for (auto& r : rows)
        r->IncreaseDegree(degree);
      occ::handle<Geom_BSplineCurve> all = occ::down_cast<Geom_BSplineCurve>(rows[0]->Copy());
      for (const auto& r : rows)
        all->InsertKnots(r->Knots(), r->Multiplicities(), Precision::PConfusion(), false);
      for (auto& r : rows)
        r->InsertKnots(all->Knots(), all->Multiplicities(), Precision::PConfusion(), false);
      for (const auto& r : rows)
        if (r->NbPoles() != all->NbPoles() || r->IsRational())
          return false;
    }
    // A Normal condition's smoothstep needs cubic rows (polygon sides are
    // lines).
    if ((ends.start == 1 || ends.end == 1) && rows[0]->Degree() < 3)
      for (auto& r : rows)
        r->IncreaseDegree(3);
    auto corner = [&](int a, int b, const gp_Vec& nrm, double u) {
      const gp_Vec d(rows[a]->Value(u), rows[b]->Value(u));
      return 0.9 * std::abs(d.Dot(nrm)) + 0.1 * d.Magnitude();
    };
    double ra0 = 0, ra1 = 0, rb0 = 0, rb1 = 0;
    if (ends.start == 1)
      ra0 = corner(0, 1, n0, u0), ra1 = corner(0, 1, n0, u1);
    if (ends.end == 1)
      rb0 = corner(n - 1, n - 2, n1, u0), rb1 = corner(n - 1, n - 2, n1, u1);
    // Smoothstep 3s² − 2s³ in the rows' polar form at pole j's knots.
    const NCollection_Array1<double>& flat = rows[0]->KnotSequence();
    const int                         deg  = rows[0]->Degree();
    auto                              step = [&](int j) {
      double e1 = 0, e2 = 0, e3 = 0;
      for (int i = 1; i <= deg; ++i)
      {
        const double x = (flat(flat.Lower() + j - 1 + i) - u0) / (u1 - u0);
        e3 += e2 * x;
        e2 += e1 * x;
        e1 += x;
      }
      const double c2 = deg * (deg - 1) / 2.0, c3 = deg * (deg - 1) * (deg - 2) / 6.0;
      return 3 * e2 / c2 - 2 * e3 / c3;
    };
    // Column by column: (x, y, z) or homogeneous (w x, w y, w z, w) with equal weights.
    const int  nu = rows[0]->NbPoles();
    const bool homogeneous = rows[0]->IsRational();
    // Chord-length parameters: the mean distance between neighbouring
    // sections (live L2: square 40 -> Ø40 -> square 20 turned 45°, 45412.22
    // in Onshape's band; uniform ones give 45554).
    std::vector<double> t(n), len(n, 0);
    for (int k = 1; k < n; ++k)
    {
      double    sum = 0;
      const int ns  = 64;
      for (int i = 0; i <= ns; ++i)
      {
        const double u = u0 + (u1 - u0) * i / ns;
        sum += rows[k]->Value(u).Distance(rows[k - 1]->Value(u));
      }
      len[k] = len[k - 1] + sum / (ns + 1);
    }
    for (int k = 0; k < n; ++k)
      t[k] = v0 + (v1 - v0) * (len[n - 1] > 0 ? len[k] / len[n - 1] : double(k) / (n - 1));
    for (int k = 1; k < n; ++k)
      if (!(t[k] > t[k - 1]))
        return false;
    const int                  nv = 3 * (n - 1) + 1;
    NCollection_Array2<gp_Pnt> poles(1, nu, 1, nv);
    NCollection_Array2<double> weights(1, nu, 1, nv);
    for (int j = 1; j <= nu; ++j)
    {
      std::vector<std::vector<double>> R;
      for (const auto& r : rows)
      {
        const gp_Pnt p = r->Pole(j);
        const double w = homogeneous ? r->Weight(j) : 1.0;
        R.push_back({p.X() * w, p.Y() * w, p.Z() * w, w});
      }
      // Clamped ends: derivatives along t (the first / last span standing
      // for a two-profile loft's whole parameter range).
      auto clamp = [&](int kind, double mag, const gp_Vec& nrm, double r0, double r1, int a, int b) {
        std::vector<double> d;
        if (kind == 0)
          return d;
        gp_Vec v;
        if (kind == 1)
          v = nrm * (mag * (r0 + (r1 - r0) * step(j)));
        else
        {
          const double wa = R[a][3], wb = R[b][3];
          const gp_Vec chord(gp_Pnt(R[a][0] / wa, R[a][1] / wa, R[a][2] / wa), gp_Pnt(R[b][0] / wb, R[b][1] / wb, R[b][2] / wb));
          v = (chord - nrm * chord.Dot(nrm)) * (mag * (a < b ? 1 : -1));
        }
        v /= std::abs(t[b] - t[a]);
        const double w = R[a][3];
        d = {v.X() * w, v.Y() * w, v.Z() * w, 0.0};
        return d;
      };
      const auto col = natural_bezier(R, t, clamp(ends.start, ends.start_mag, n0, ra0, ra1, 0, 1),
                                      clamp(ends.end, ends.end_mag, n1, rb0, rb1, n - 1, n - 2));
      for (int i = 0; i < nv; ++i)
      {
        const double w = col[i][3];
        if (!(w > 0))
          return false;
        poles(j, i + 1)   = gp_Pnt(col[i][0] / w, col[i][1] / w, col[i][2] / w);
        weights(j, i + 1) = w;
      }
    }
    NCollection_Array1<double> vknots(1, n);
    NCollection_Array1<int>    vmults(1, n);
    for (int k = 0; k < n; ++k)
    {
      vknots(k + 1) = t[k];
      vmults(k + 1) = (k == 0 || k == n - 1) ? 4 : 3;
    }
    occ::handle<Geom_BSplineSurface> ns =
      homogeneous ? new Geom_BSplineSurface(poles, weights, rows[0]->Knots(), vknots, rows[0]->Multiplicities(), vmults,
                                            rows[0]->Degree(), 3)
                  : new Geom_BSplineSurface(poles, rows[0]->Knots(), vknots, rows[0]->Multiplicities(), vmults,
                                            rows[0]->Degree(), 3);
    faces.push_back({f, ns});
    // Seams (u = const, parameter v): their curves become the new surface's iso-lines.
    for (TopExp_Explorer ee(f, TopAbs_EDGE); ee.More(); ee.Next())
    {
      const TopoDS_Edge&          e = TopoDS::Edge(ee.Current());
      double                      pf, pl;
      occ::handle<Geom2d_Curve>   pc   = BRep_Tool::CurveOnSurface(e, f, pf, pl);
      occ::handle<Geom2d_Line>    line = occ::down_cast<Geom2d_Line>(pc);
      if (line.IsNull() || BRep_Tool::Degenerated(e))
        return false;
      const gp_Dir2d dir = line->Direction();
      if (std::abs(dir.X()) > 1e-12)
        continue; // a section edge: v = const, unchanged
      if (dir.Y() < 0 || std::abs(line->Location().Y()) > 1e-12 || !e.Location().IsIdentity())
        return false;
      seams.emplace_back(e, ns->UIso(line->Location().X()));
    }
  }
  if (faces.empty())
    return false;
  // Pcurves belong to the (old) surface: carry them over to the new one.
  // (A seam of a face closed on itself has two.)
  struct PCurve
  {
    TopoDS_Edge               edge;
    TopoDS_Face               face;
    occ::handle<Geom2d_Curve> forward, reversed;
  };
  std::vector<PCurve> pcurves;
  for (const auto& u : faces)
  {
    ShapeIMap edges;
    TopExp::MapShapes(u.face, TopAbs_EDGE, edges);
    for (int i = 1; i <= edges.Extent(); ++i)
    {
      TopoDS_Edge e = TopoDS::Edge(edges(i));
      double      pf, pl;
      e.Orientation(TopAbs_FORWARD);
      PCurve p{e, u.face, BRep_Tool::CurveOnSurface(e, u.face, pf, pl), {}};
      if (BRep_Tool::IsClosed(e, u.face))
        p.reversed = BRep_Tool::CurveOnSurface(TopoDS::Edge(e.Reversed()), u.face, pf, pl);
      pcurves.push_back(p);
    }
  }
  BRep_Builder B;
  for (const auto& u : faces)
    B.UpdateFace(u.face, u.surface, TopLoc_Location(), BRep_Tool::Tolerance(u.face));
  for (const auto& p : pcurves)
    if (p.reversed.IsNull())
      B.UpdateEdge(p.edge, p.forward, p.face, BRep_Tool::Tolerance(p.edge));
    else
      B.UpdateEdge(p.edge, p.forward, p.reversed, p.face, BRep_Tool::Tolerance(p.edge));
  for (const auto& [e, c] : seams)
    B.UpdateEdge(e, c, BRep_Tool::Tolerance(e));
  BRepLib::SameParameter(loft, tol, true);
  BRepLib::UpdateTolerances(loft, false);
  return true;
}

namespace {
HHist compose(const TopoDS_Shape& input, const HHist& h12, const HHist& h23);
} // namespace

// Polygon-like sections with different vertex counts (a triangle to a
// square): like Parasolid (Onshape G12), each section with fewer vertices
// gets vertices at the reference section's arc-length positions that none of
// its own vertices is nearest to, so the sections match vertex to vertex and
// the loft's faces stay ruled between them (triangle -> square: the top edge
// is split at its middle, 3920 mm³). OCCT's polar matching twists them.
// Returns false (nothing changed) when it doesn't apply; `hist` records the
// split edges (Modified).
static bool match_vertex_counts(std::vector<TopoDS_Wire>& wires, HHist& hist)
{
  struct Seq
  {
    std::vector<TopoDS_Edge>   edges; // in wire order, oriented
    std::vector<TopoDS_Vertex> starts;
    std::vector<double>        len;
    double                     total = 0;
  };
  auto seq_of = [](const TopoDS_Wire& w) {
    Seq q;
    for (BRepTools_WireExplorer ex(w); ex.More(); ex.Next())
    {
      q.edges.push_back(ex.Current());
      q.starts.push_back(ex.CurrentVertex());
      BRepAdaptor_Curve c(ex.Current());
      q.len.push_back(GCPnts_AbscissaPoint::Length(c));
      q.total += q.len.back();
    }
    return q;
  };
  std::vector<Seq> seqs;
  size_t           m = 0;
  for (const auto& w : wires)
  {
    seqs.push_back(seq_of(w));
    if (seqs.back().edges.size() < 2 || seqs.back().total <= 0)
      return false; // a circle or a single closed curve: OCCT's polar matching is Onshape's
    m = std::max(m, seqs.back().edges.size());
  }
  bool differ = false;
  for (const auto& q : seqs)
    differ = differ || q.edges.size() != m;
  if (!differ)
    return false;
  auto params = [](const Seq& q) {
    std::vector<double> t{0};
    for (size_t k = 0; k + 1 < q.len.size(); ++k)
      t.push_back(t.back() + q.len[k] / q.total);
    return t;
  };
  // Point at normalized arc length t along a sequence.
  auto point_at = [](const Seq& q, double t) {
    double d = t * q.total;
    for (size_t k = 0; k < q.edges.size(); ++k)
    {
      if (d <= q.len[k] || k + 1 == q.edges.size())
      {
        BRepAdaptor_Curve c(q.edges[k]);
        const bool        fwd = q.edges[k].Orientation() != TopAbs_REVERSED;
        GCPnts_AbscissaPoint ap(c, fwd ? d : -d, fwd ? c.FirstParameter() : c.LastParameter());
        return std::make_pair(k, ap.Parameter());
      }
      d -= q.len[k];
    }
    return std::make_pair(q.edges.size() - 1, 0.0);
  };
  hist = new BRepTools_History();
  for (size_t si = 0; si < wires.size(); ++si)
  {
    const Seq& q = seqs[si];
    if (q.edges.size() == m)
      continue;
    // Reference: the nearest section with the most vertices.
    size_t ref = 0, best_gap = SIZE_MAX;
    for (size_t r = 0; r < seqs.size(); ++r)
      if (seqs[r].edges.size() == m && (r > si ? r - si : si - r) < best_gap)
        ref = r, best_gap = r > si ? r - si : si - r;
    const Seq&                R  = seqs[ref];
    const std::vector<double> sw = params(q);
    const size_t              k  = q.edges.size();
    // For each rotation of the reference: monotone assignment of our vertices
    // (ours[0] -> its rotated vertex 0) minimizing the arc-length offsets; the
    // other reference vertices become insertions. Keep the rotation whose
    // matched points are closest in space.
    double                 best_cost = 1e300;
    std::vector<double>    best_t; // normalized params on W of the m points, in reference order from rot
    size_t                 best_rot = 0;
    for (size_t rot = 0; rot < m; ++rot)
    {
      std::vector<double> tr(m);
      const std::vector<double> t0 = params(R);
      for (size_t j = 0; j < m; ++j)
        tr[j] = std::fmod(t0[(rot + j) % m] - t0[rot] + 1.0, 1.0);
      // dp[i][j]: our vertex i at reference vertex j (j increasing), cost of |s - t|.
      std::vector<std::vector<double>> dp(k, std::vector<double>(m, 1e300));
      std::vector<std::vector<int>>    from(k, std::vector<int>(m, -1));
      dp[0][0] = 0;
      for (size_t i = 1; i < k; ++i)
        for (size_t j = i; j < m; ++j)
          for (size_t p = i - 1; p < j; ++p)
            if (dp[i - 1][p] < 1e300)
            {
              const double c = dp[i - 1][p] + std::abs(sw[i] - tr[j]);
              if (c < dp[i][j])
                dp[i][j] = c, from[i][j] = (int)p;
            }
      size_t jl = k - 1;
      for (size_t j = k - 1; j < m; ++j)
        if (dp[k - 1][j] < dp[k - 1][jl])
          jl = j;
      std::vector<int> at(k);
      at[k - 1] = (int)jl;
      for (size_t i = k - 1; i > 0; --i)
        at[i - 1] = from[i][at[i]];
      std::vector<double> tw(m, -1);
      for (size_t i = 0; i < k; ++i)
        tw[at[i]] = sw[i];
      for (size_t j = 0; j < m; ++j)
        if (tw[j] < 0)
          tw[j] = tr[j];
      double cost = 0;
      for (size_t j = 0; j < m; ++j)
      {
        const auto [e, prm] = point_at(q, tw[j]);
        cost += BRepAdaptor_Curve(q.edges[e]).Value(prm).Distance(BRep_Tool::Pnt(R.starts[(rot + j) % m]));
      }
      if (cost < best_cost - 1e-12)
        best_cost = cost, best_t = tw, best_rot = rot;
    }
    // Split our edges at the inserted points (params not at our vertices).
    std::vector<TopoDS_Edge> out;
    std::vector<double>      cuts;
    for (double t : best_t)
      if (std::none_of(sw.begin(), sw.end(), [&](double s) { return std::abs(s - t) < 1e-12; }))
        cuts.push_back(t);
    std::sort(cuts.begin(), cuts.end());
    for (size_t e = 0; e < k; ++e)
    {
      std::vector<double> mine;
      for (double t : cuts)
        if (point_at(q, t).first == e)
          mine.push_back(point_at(q, t).second);
      if (mine.empty())
      {
        out.push_back(q.edges[e]);
        continue;
      }
      const TopoDS_Edge&      E   = q.edges[e];
      const bool              fwd = E.Orientation() != TopAbs_REVERSED;
      double                  f, l;
      occ::handle<Geom_Curve> c = BRep_Tool::Curve(E, f, l);
      std::vector<double>     prm{fwd ? f : l};
      prm.insert(prm.end(), mine.begin(), mine.end());
      prm.push_back(fwd ? l : f);
      std::vector<TopoDS_Vertex> vs{TopExp::FirstVertex(E, true)};
      for (size_t i = 0; i < mine.size(); ++i)
        vs.push_back(BRepBuilderAPI_MakeVertex(c->Value(mine[i])));
      vs.push_back(TopExp::LastVertex(E, true));
      for (size_t i = 0; i + 1 < prm.size(); ++i)
      {
        TopoDS_Edge piece = fwd ? BRepBuilderAPI_MakeEdge(c, vs[i], vs[i + 1], prm[i], prm[i + 1]).Edge()
                                : TopoDS::Edge(BRepBuilderAPI_MakeEdge(c, vs[i + 1], vs[i], prm[i + 1], prm[i]).Edge().Reversed());
        TopoDS_Edge key = E;
        key.Orientation(TopAbs_FORWARD);
        hist->AddModified(key, piece);
        out.push_back(piece);
      }
    }
    // Start where the reference starts.
    const double t_start = best_t[(m - best_rot) % m];
    size_t       start   = 0;
    double       acc     = 0;
    for (size_t i = 0; i < out.size(); ++i)
    {
      if (std::abs(acc - t_start) < 1e-9)
        start = i;
      BRepAdaptor_Curve c(out[i]);
      acc += GCPnts_AbscissaPoint::Length(c) / q.total;
    }
    BRepBuilderAPI_MakeWire mk;
    for (size_t i = 0; i < out.size(); ++i)
      mk.Add(out[(start + i) % out.size()]);
    if (!mk.IsDone())
      return false;
    wires[si] = mk.Wire();
  }
  return true;
}

// A closed single-curve section (a circle) lofted to polygons: Parasolid
// splits it where the nearest polygon's corners point from its centre (live
// L1: square 40 -> circle Ø30 split at 45°, each corner straight to the circle
// point on its diagonal, 6 faces; OCCT also splits at the circle's own seam,
// 7). Sections of equal vertex counts then pair start to start, each
// polygon's start the corner nearest in angle to the previous section's,
// clockwise about the loft's direction on a tie (live L2 bodydetails: square
// 40 -> circle Ø40 -> square 20 turned 45°: the side edges run from each
// corner to the top corner 45° clockwise, 6 faces). `hist` records the circles' pieces (Modified). False when there's
// no such section (nothing changed).
static bool split_closed_sections(std::vector<TopoDS_Wire>& wires, HHist& hist)
{
  auto edges_of = [](const TopoDS_Wire& w) {
    std::vector<TopoDS_Edge> es;
    for (BRepTools_WireExplorer ex(w); ex.More(); ex.Next())
      es.push_back(ex.Current());
    return es;
  };
  auto starts_of = [](const TopoDS_Wire& w) {
    std::vector<gp_Pnt> ps;
    for (BRepTools_WireExplorer ex(w); ex.More(); ex.Next())
      ps.push_back(BRep_Tool::Pnt(ex.CurrentVertex()));
    return ps;
  };
  auto centre = [](const TopoDS_Shape& w) {
    GProp_GProps g;
    BRepGProp::LinearProperties(w, g);
    return g.CentreOfMass();
  };
  const std::vector<TopoDS_Wire> before = wires;
  bool                           split  = false;
  for (size_t si = 0; si < wires.size(); ++si)
  {
    const std::vector<TopoDS_Edge> own = edges_of(before[si]);
    if (own.size() != 1 || own[0].Orientation() == TopAbs_REVERSED)
      continue;
    double                  f = 0, l = 0;
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(own[0], f, l);
    if (c.IsNull() || !c->IsPeriodic() || !BRep_Tool::IsClosed(own[0]))
      continue;
    size_t ref = SIZE_MAX, gap = SIZE_MAX;
    for (size_t r = 0; r < wires.size(); ++r)
      if (r != si && edges_of(before[r]).size() >= 2 && (r > si ? r - si : si - r) < gap)
        ref = r, gap = r > si ? r - si : si - r;
    if (ref == SIZE_MAX)
      continue;
    const gp_Pnt        cr = centre(before[ref]), cs = centre(before[si]);
    const double        period = l - f;
    std::vector<double> ts;
    for (const gp_Pnt& v : starts_of(before[ref]))
    {
      GeomAPI_ProjectPointOnCurve pr(cs.Translated(gp_Vec(cr, v)), c, f, l);
      if (pr.NbPoints() == 0)
        return false;
      ts.push_back(pr.LowerDistanceParameter());
    }
    // From the first corner's point on, increasing along the curve.
    const double t0 = ts[0];
    for (double& t : ts)
      t = t0 + std::fmod(std::fmod(t - t0, period) + period, period);
    std::sort(ts.begin() + 1, ts.end());
    ts.push_back(t0 + period);
    BRepBuilderAPI_MakeWire mk;
    TopoDS_Edge             key = own[0];
    key.Orientation(TopAbs_FORWARD);
    TopoDS_Vertex first, prev;
    for (size_t k = 0; k + 1 < ts.size(); ++k)
    {
      if (!(ts[k + 1] - ts[k] > 1e-9))
        return false;
      TopoDS_Vertex v0 = k == 0 ? BRepBuilderAPI_MakeVertex(c->Value(ts[0])).Vertex() : prev;
      if (k == 0)
        first = v0;
      TopoDS_Vertex v1 = k + 2 == ts.size() ? first : BRepBuilderAPI_MakeVertex(c->Value(ts[k + 1])).Vertex();
      TopoDS_Edge   e  = BRepBuilderAPI_MakeEdge(c, v0, v1, ts[k], ts[k + 1]).Edge();
      prev             = v1;
      mk.Add(e);
      if (hist.IsNull())
        hist = new BRepTools_History();
      hist->AddModified(key, e);
    }
    if (!mk.IsDone())
      return false;
    wires[si] = mk.Wire();
    split     = true;
  }
  if (!split)
    return false;
  // Pair the starts when the counts now agree.
  const size_t m = edges_of(wires[0]).size();
  for (const auto& w : wires)
    if (edges_of(w).size() != m)
      return true;
  for (size_t i = 1; i < wires.size(); ++i)
  {
    const std::vector<gp_Pnt> prev = starts_of(wires[i - 1]), mine = starts_of(wires[i]);
    const gp_Pnt              cp = centre(wires[i - 1]), cm = centre(wires[i]);
    const gp_Vec              axis(cp, cm);
    if (axis.Magnitude() < Precision::Confusion())
      continue;
    const gp_Dir z(axis);
    const gp_Vec a(cp, prev[0]);
    size_t       best = 0;
    double       bd   = 1e300;
    for (size_t k = 0; k < mine.size(); ++k)
    {
      const gp_Vec b(cm, mine[k]);
      // Signed angle from a to b about z, in (-pi, pi]; a tie goes clockwise.
      const double ang = a.AngleWithRef(b, gp_Vec(z));
      const double d   = std::abs(ang) - (ang < 0 ? 1e-9 : 0);
      if (d < bd)
        bd = d, best = k;
    }
    if (best != 0)
      wires[i] = rotate_wire_start(wires[i], mine[best]);
  }
  return true;
}

// Loft connections (Onshape's Match connections): `conns[c][w]` is
// connection c's point on section wire w. Each wire is split at its points,
// turned to run like the first wire (by its normal), started at the first
// connection, and the edges between consecutive connections are evened out
// (the longest split at its middle) so that ThruSections pairs connection
// with connection. `hist` records the split edges (Modified). False when a
// point is off its wire or the connections come in different orders.
static bool connect_sections(std::vector<TopoDS_Wire>& wires, const std::vector<std::vector<gp_Pnt>>& conns,
                             double tol, HHist& hist)
{
  const size_t k = conns.size(), nw = wires.size();
  auto edges_of = [](const TopoDS_Wire& w) {
    std::vector<TopoDS_Edge> es;
    for (BRepTools_WireExplorer ex(w); ex.More(); ex.Next())
      es.push_back(ex.Current());
    return es;
  };
  auto key_of = [](const TopoDS_Edge& e) {
    TopoDS_Edge key = e;
    key.Orientation(TopAbs_FORWARD);
    return key;
  };
  // Each piece made here and the original edge it came from.
  std::vector<std::pair<TopoDS_Edge, TopoDS_Edge>> origin;
  auto root = [&](const TopoDS_Edge& e) {
    for (const auto& [piece, orig] : origin)
      if (piece.IsSame(e))
        return orig;
    return key_of(e);
  };
  // Pieces of `E` split at curve parameters `cuts`, in wire order.
  auto split = [&](const TopoDS_Edge& E, std::vector<double> cuts) {
    std::vector<TopoDS_Edge> out;
    const bool              fwd = E.Orientation() != TopAbs_REVERSED;
    double                  f, l;
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(E, f, l);
    std::sort(cuts.begin(), cuts.end());
    if (!fwd)
      std::reverse(cuts.begin(), cuts.end());
    std::vector<double> prm{fwd ? f : l};
    prm.insert(prm.end(), cuts.begin(), cuts.end());
    prm.push_back(fwd ? l : f);
    std::vector<TopoDS_Vertex> vs{TopExp::FirstVertex(E, true)};
    for (double t : cuts)
      vs.push_back(BRepBuilderAPI_MakeVertex(c->Value(t)));
    vs.push_back(TopExp::LastVertex(E, true));
    for (size_t i = 0; i + 1 < prm.size(); ++i)
    {
      TopoDS_Edge piece = fwd ? BRepBuilderAPI_MakeEdge(c, vs[i], vs[i + 1], prm[i], prm[i + 1]).Edge()
                              : TopoDS::Edge(BRepBuilderAPI_MakeEdge(c, vs[i + 1], vs[i], prm[i + 1], prm[i]).Edge().Reversed());
      origin.emplace_back(piece, root(E));
      out.push_back(piece);
    }
    return out;
  };
  auto normal_of = [](const std::vector<TopoDS_Edge>& es) {
    gp_Vec nrm;
    std::vector<gp_Pnt> pts;
    for (const auto& e : es)
    {
      BRepAdaptor_Curve c(e);
      for (int i = 0; i < 8; ++i)
      {
        const double t = e.Orientation() != TopAbs_REVERSED ? c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * i / 8
                                                            : c.LastParameter() - (c.LastParameter() - c.FirstParameter()) * i / 8;
        pts.push_back(c.Value(t));
      }
    }
    for (size_t i = 0; i < pts.size(); ++i)
      nrm += gp_Vec(pts[i].XYZ()).Crossed(gp_Vec(pts[(i + 1) % pts.size()].XYZ()));
    return nrm;
  };
  std::vector<std::vector<TopoDS_Edge>> all(nw);
  std::vector<std::vector<size_t>>      at(nw, std::vector<size_t>(k));
  gp_Vec                                n0;
  for (size_t w = 0; w < nw; ++w)
  {
    std::vector<TopoDS_Edge> es = edges_of(wires[w]);
    if (es.empty())
      return false;
    // Run like the first wire.
    const gp_Vec nrm = normal_of(es);
    if (w == 0)
      n0 = nrm;
    else if (nrm.Dot(n0) < 0)
    {
      std::reverse(es.begin(), es.end());
      for (auto& e : es)
        e = TopoDS::Edge(e.Reversed());
    }
    // Each point: its edge and parameter (or the vertex starting an edge).
    std::vector<std::vector<double>> cuts(es.size());
    std::vector<std::pair<size_t, double>> where(k);
    for (size_t c = 0; c < k; ++c)
    {
      double best = 1e300;
      for (size_t i = 0; i < es.size(); ++i)
      {
        double                  f, l;
        occ::handle<Geom_Curve> cv = BRep_Tool::Curve(es[i], f, l);
        if (cv.IsNull())
          return false;
        GeomAPI_ProjectPointOnCurve pr(conns[c][w], cv, f, l);
        for (int j = 1; j <= pr.NbPoints(); ++j)
          if (pr.Distance(j) < best)
            best = pr.Distance(j), where[c] = {i, pr.Parameter(j)};
        for (double t : {f, l})
          if (cv->Value(t).Distance(conns[c][w]) < best)
            best = cv->Value(t).Distance(conns[c][w]), where[c] = {i, t};
      }
      if (best > tol)
        return false;
      double f, l;
      BRep_Tool::Range(es[where[c].first], f, l);
      if (std::abs(where[c].second - f) > 1e-9 && std::abs(where[c].second - l) > 1e-9)
        cuts[where[c].first].push_back(where[c].second);
    }
    // Split, and find the edge each connection starts.
    std::vector<TopoDS_Edge> out;
    for (size_t i = 0; i < es.size(); ++i)
      if (cuts[i].empty())
        out.push_back(es[i]);
      else
        for (const auto& piece : split(es[i], cuts[i]))
          out.push_back(piece);
    for (size_t c = 0; c < k; ++c)
    {
      size_t best = 0;
      double bd   = 1e300;
      for (size_t i = 0; i < out.size(); ++i)
      {
        const double d = BRep_Tool::Pnt(TopExp::FirstVertex(out[i], true)).Distance(conns[c][w]);
        if (d < bd)
          bd = d, best = i;
      }
      if (bd > tol)
        return false;
      at[w][c] = best;
    }
    // Start at the first connection.
    const size_t s0 = at[w][0];
    std::rotate(out.begin(), out.begin() + (long)s0, out.end());
    for (auto& a : at[w])
      a = (a + out.size() - s0) % out.size();
    all[w] = out;
  }
  // The connections in the same order round every wire.
  auto order = [&](size_t w) {
    std::vector<size_t> o(k);
    for (size_t c = 0; c < k; ++c)
      o[c] = c;
    std::sort(o.begin(), o.end(), [&](size_t a, size_t b) { return at[w][a] < at[w][b]; });
    return o;
  };
  const std::vector<size_t> o0 = order(0);
  for (size_t w = 0; w < nw; ++w)
  {
    if (order(w) != o0)
      return false;
    for (size_t c = 1; c < k; ++c)
      if (at[w][o0[c]] == at[w][o0[c - 1]])
        return false; // two connections at one point
  }
  // Even out the edges between consecutive connections.
  for (size_t j = 0; j < k; ++j)
  {
    auto span = [&](size_t w) {
      const size_t a = at[w][o0[j]], b = j + 1 < k ? at[w][o0[j + 1]] : all[w].size();
      return std::make_pair(a, b);
    };
    size_t target = 0;
    for (size_t w = 0; w < nw; ++w)
      target = std::max(target, span(w).second - span(w).first);
    for (size_t w = 0; w < nw; ++w)
      while (span(w).second - span(w).first < target)
      {
        const auto [a, b] = span(w);
        size_t     longest = a;
        double     len     = -1;
        for (size_t i = a; i < b; ++i)
        {
          BRepAdaptor_Curve c(all[w][i]);
          const double      li = GCPnts_AbscissaPoint::Length(c);
          if (li > len)
            len = li, longest = i;
        }
        BRepAdaptor_Curve    c(all[w][longest]);
        GCPnts_AbscissaPoint mid(c, len / 2, c.FirstParameter());
        if (!mid.IsDone())
          return false;
        const auto pieces = split(all[w][longest], {mid.Parameter()});
        all[w].erase(all[w].begin() + (long)longest);
        all[w].insert(all[w].begin() + (long)longest, pieces.begin(), pieces.end());
        for (size_t c2 = 0; c2 < k; ++c2)
          if (at[w][c2] > longest)
            at[w][c2] += 1;
      }
  }
  for (size_t w = 0; w < nw; ++w)
  {
    BRepBuilderAPI_MakeWire mk;
    for (const auto& e : all[w])
    {
      mk.Add(e);
      const TopoDS_Edge orig = root(e);
      if (!orig.IsSame(e))
      {
        if (hist.IsNull())
          hist = new BRepTools_History();
        hist->AddModified(orig, e);
      }
    }
    if (!mk.IsDone())
      return false;
    wires[w] = mk.Wire();
  }
  return true;
}

// Loft along a path (Onshape's Path): the matched section wires carried
// along `spine` (MakePipeShell), and `count` sections cut from that at even
// steps of the path between the first and last profile, so the loft runs
// through them all (live "psb (10-24 f)": a □20 along a quarter arc gives
// 25100.80 with 5 sections, 25128.64 with 10, the exact sweep 25132.74).
// False when the sections can't be made (the wires are left as they were).
static bool path_sections(std::vector<TopoDS_Wire>& wires, const TopoDS_Wire& spine, int count)
{
  // (Profiles in between aren't placed yet.)
  if (wires.size() != 2 || count < 1)
    return false;
  BRepOffsetAPI_MakePipeShell ps(spine);
  for (const auto& w : wires)
    ps.Add(w, false, false);
  ps.Build();
  if (!ps.IsDone())
    return false;
  const TopoDS_Shape    carried = ps.Shape();
  BRepAdaptor_CompCurve cc(spine);
  const double          total = GCPnts_AbscissaPoint::Length(cc);
  // Each end profile's place on the path: the arc length of the path point
  // nearest its centre (sampled, then narrowed).
  auto place = [&](const TopoDS_Wire& w) {
    GProp_GProps g;
    BRepGProp::LinearProperties(w, g);
    const gp_Pnt c  = g.CentreOfMass();
    const double u0 = cc.FirstParameter(), u1 = cc.LastParameter();
    const int    N  = 2000;
    int          bi = 0;
    for (int i = 1; i <= N; ++i)
      if (cc.Value(u0 + (u1 - u0) * i / N).Distance(c) < cc.Value(u0 + (u1 - u0) * bi / N).Distance(c))
        bi = i;
    double lo = u0 + (u1 - u0) * std::max(0, bi - 1) / N, hi = u0 + (u1 - u0) * std::min(N, bi + 1) / N;
    for (int i = 0; i < 80; ++i)
    {
      const double a = lo + (hi - lo) / 3, b = hi - (hi - lo) / 3;
      if (cc.Value(a).Distance(c) < cc.Value(b).Distance(c))
        hi = b;
      else
        lo = a;
    }
    return GCPnts_AbscissaPoint::Length(cc, u0, 0.5 * (lo + hi));
  };
  const double s0 = place(wires.front()), s1 = place(wires.back());
  if (std::abs(s1 - s0) < 1e-9 * (1 + total))
    return false;
  std::vector<TopoDS_Wire> out{wires.front()};
  // Each section runs like the one before and starts where it does.
  auto normal_of = [](const TopoDS_Wire& w) {
    BRepAdaptor_CompCurve c(w);
    gp_Vec                nrm;
    const int             m = 64;
    for (int k = 0; k < m; ++k)
    {
      const gp_Pnt a = c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / m);
      const gp_Pnt b = c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * (k + 1) / m);
      nrm += gp_Vec(a.XYZ()).Crossed(gp_Vec(b.XYZ()));
    }
    return nrm;
  };
  auto align = [&](TopoDS_Wire w) {
    const TopoDS_Wire& prev = out.back();
    if (normal_of(w).Dot(normal_of(prev)) < 0)
      w = TopoDS::Wire(w.Reversed());
    BRepTools_WireExplorer first(prev);
    return rotate_wire_start(w, BRep_Tool::Pnt(first.CurrentVertex()));
  };
  for (int i = 1; i <= count; ++i)
  {
    const double         s = s0 + (s1 - s0) * i / (count + 1);
    GCPnts_AbscissaPoint ap(cc, s, cc.FirstParameter());
    if (!ap.IsDone())
      return false;
    gp_Pnt P;
    gp_Vec T;
    cc.D1(ap.Parameter(), P, T);
    if (T.Magnitude() < 1e-12)
      return false;
    BRepAlgoAPI_Section sec(carried, gp_Pln(P, gp_Dir(T)), false);
    sec.Approximation(true);
    sec.Build();
    if (!sec.IsDone())
      return false;
    occ::handle<TopTools_HSequenceOfShape> edges = new TopTools_HSequenceOfShape;
    for (TopExp_Explorer ex(sec.Shape(), TopAbs_EDGE); ex.More(); ex.Next())
      edges->Append(ex.Current());
    occ::handle<TopTools_HSequenceOfShape> found;
    ShapeAnalysis_FreeBounds::ConnectEdgesToWires(edges, 1e-6, false, found);
    if (found.IsNull() || found->Length() == 0)
      return false;
    // The loop nearest the path point.
    TopoDS_Wire best;
    double      bd = 1e300;
    for (int k = 1; k <= found->Length(); ++k)
    {
      BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(P).Vertex(), found->Value(k));
      if (d.IsDone() && d.Value() < bd)
        bd = d.Value(), best = TopoDS::Wire(found->Value(k));
    }
    out.push_back(align(best));
  }
  // The end profile too (same edges, so its history is untouched).
  out.push_back(align(wires.back()));
  wires = out;
  return true;
}

static OpOut loft_impl(const OcShapeList& sections, bool solid, bool ruled, bool closed,
                       rust::Slice<const double> start_points, const LoftEnds& ends, bool allow_natural,
                       const TopoDS_Shape& path = TopoDS_Shape(), int path_count = 0)
{
  const size_t n = sections.items.size();
  if (n < 2)
    fail_invalid("loft needs at least 2 sections");
  // One point per section, or several connections (k points per section,
  // connection by connection).
  if (!start_points.empty() && start_points.size() % (3 * n) != 0)
    fail_invalid("loft start points must be one (x, y, z) per section");
  const size_t connections = start_points.size() / (3 * n);
  const bool conditions = ends.start != 0 || ends.end != 0;
  bool       natural    = allow_natural && !ruled && !closed && (n >= 3 || conditions);
  for (const auto& s : sections.items)
    natural = natural && !s.IsNull() && s.ShapeType() != TopAbs_VERTEX;
  BRepOffsetAPI_ThruSections ts(solid, ruled, 1e-6);
  if (natural)
    ts.SetParType(Approx_IsoParametric);
  std::vector<TopoDS_Shape> args;
  std::vector<TopoDS_Wire>  wires;
  bool                      points = false;
  for (size_t i = 0; i < n; ++i)
  {
    const TopoDS_Shape& s = sections.items[i];
    require_non_null(s, "loft section");
    args.push_back(s);
    if (s.ShapeType() == TopAbs_VERTEX)
    {
      if (i != 0 && i != n - 1)
        fail_invalid("only the first and last loft sections can be points");
      points = true;
      continue;
    }
    TopoDS_Wire w = outer_or_wire(s, "loft section");
    if (connections == 1 && std::isfinite(start_points[3 * i]))
      w = rotate_wire_start(
        w, gp_Pnt(start_points[3 * i], start_points[3 * i + 1], start_points[3 * i + 2]));
    wires.push_back(w);
  }
  // Vertex counts matched like Parasolid; then OCCT must keep that matching.
  HHist split;
  bool  connected = false;
  if (connections >= 2)
  {
    if (points)
      fail_invalid("loft connections need wire sections");
    std::vector<std::vector<gp_Pnt>> conns(connections);
    for (size_t c = 0; c < connections; ++c)
      for (size_t i = 0; i < n; ++i)
      {
        const double* q = &start_points[3 * (c * n + i)];
        if (!std::isfinite(q[0]))
          fail_invalid("every loft connection needs a point on each section");
        conns[c].push_back(gp_Pnt(q[0], q[1], q[2]));
      }
    Bnd_Box box;
    for (const auto& w : wires)
      BRepBndLib::Add(w, box);
    const double tol = 1e-6 * (box.IsVoid() ? 1.0 : std::sqrt(box.SquareExtent()));
    if (!connect_sections(wires, conns, tol, split))
      fail_op("the loft connections don't match the sections");
    connected = true;
  }
  const bool circles = !points && start_points.empty() && split_closed_sections(wires, split);
  HHist      more;
  const bool counts  = !points && start_points.empty() && match_vertex_counts(wires, more);
  if (counts)
    split = split.IsNull() ? more : compose(compound_of(args), split, more);
  // Along a path: the sections cut from the profiles carried along it.
  bool along = false;
  if (!path.IsNull() && !points)
  {
    if (!path_sections(wires, as_wire(path, "loft path"), path_count))
      fail_op("the loft can't follow the path");
    along   = true;
    natural = allow_natural && !ruled && !closed;
    if (natural)
      ts.SetParType(Approx_IsoParametric);
  }
  const bool matched = circles || counts || connected || along;
  ts.CheckCompatibility(!matched);
  TopoDS_Wire firstWire = wires.empty() ? TopoDS_Wire() : wires.front();
  // OCCT lofts two sections ruled; an end condition needs the smooth
  // surface: a section halfway (the first moved there), dropped again by
  // natural_loft, makes ThruSections smooth it.
  std::vector<TopoDS_Wire> smooth = wires;
  int                      drop   = -1;
  if (natural && n == 2)
  {
    GProp_GProps g0, g1;
    BRepGProp::LinearProperties(wires[0], g0);
    BRepGProp::LinearProperties(wires[1], g1);
    gp_Trsf half;
    half.SetTranslation(gp_Vec(g0.CentreOfMass(), g1.CentreOfMass()) * 0.5);
    smooth.insert(smooth.begin() + 1, TopoDS::Wire(BRepBuilderAPI_Transform(wires[0], half, true).Shape()));
    drop = 1;
  }
  if (along)
    for (const auto& w : wires)
      ts.AddWire(w);
  else
    for (size_t i = 0, w = 0; i < n; ++i)
      if (sections.items[i].ShapeType() == TopAbs_VERTEX)
        ts.AddVertex(TopoDS::Vertex(sections.items[i]));
      else
      {
        ts.AddWire(wires[w++]);
        if (drop >= 0 && i == 0)
          ts.AddWire(smooth[1]);
      }
  if (closed)
  {
    if (firstWire.IsNull())
      fail_invalid("closed loft requires wire sections");
    ts.AddWire(firstWire);
  }
  ts.Build();
  if (!ts.IsDone())
    fail_op("loft failed (incompatible sections?)");
  if (natural && (!natural_loft(ts.Shape(), smooth, ends, drop) || !BRepCheck_Analyzer(ts.Shape()).IsValid()))
    return loft_impl(sections, solid, ruled, closed, start_points, ends, false, path, path_count);
  HHist h = history_of(args, ts);
  if (matched)
  {
    // Section edges -> their pieces -> loft faces.
    std::vector<TopoDS_Shape> wargs(wires.begin(), wires.end());
    h = compose(compound_of(args), split, history_of(wargs, ts));
  }
  HistoryBuilder hb(args, ts.Shape());
  hb.add_history(h.get());
  if (!closed)
  {
    hb.add_cap(0, REL_FIRST, ts.FirstShape());
    hb.add_cap((uint32_t)n - 1, REL_LAST, ts.LastShape());
  }
  return hb.finish();
}

OpOut op_loft(const OcShapeList& sections, bool solid, bool ruled, bool closed,
              rust::Slice<const double> start_points, rust::Slice<const double> conditions, const OcShape& path,
              int32_t path_count)
{
  LoftEnds ends;
  if (conditions.size() == 10)
  {
    const auto& c = conditions;
    ends = {(int)c[0], c[1], gp_Vec(c[2], c[3], c[4]), (int)c[5], c[6], gp_Vec(c[7], c[8], c[9])};
  }
  return loft_impl(sections, solid, ruled, closed, start_points, ends, true, path.shape, path_count);
}

// ---------------------------------------------------------------- thicken / shell

static int solid_count_of(const TopoDS_Shape& s)
{
  int n = 0;
  for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next())
    ++n;
  return n;
}

// Reverse solids whose signed volume is negative (inside-out results).
static TopoDS_Shape orient_solids_outward(const TopoDS_Shape& s)
{
  auto signed_volume = [](const TopoDS_Shape& x) {
    GProp_GProps g;
    BRepGProp::VolumeProperties(x, g);
    return g.Mass();
  };
  if (s.IsNull())
    return s;
  if (s.ShapeType() == TopAbs_SOLID)
    return signed_volume(s) < 0 ? s.Reversed() : s;
  bool any_negative = false;
  for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next())
    if (signed_volume(e.Current()) < 0)
      any_negative = true;
  if (!any_negative)
    return s;
  std::vector<TopoDS_Shape> parts;
  for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next())
  {
    const TopoDS_Shape& so = e.Current();
    parts.push_back(signed_volume(so) < 0 ? so.Reversed() : so);
  }
  return parts.size() == 1 ? parts[0] : compound_of(parts);
}

OpOut op_thicken(const OcShape& s, double t1, double t2)
{
  require_non_null(s.shape, "shape");
  const double total = t1 + t2;
  if (!(std::abs(total) > 1e-9))
    fail_invalid("thicken: total thickness must be non-zero");
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     hist;
  TopoDS_Shape              base = s.shape;
  // A closed surface (no free edges, e.g. a whole sphere) thickens into a
  // hollow solid: the outer and inner offsets bound it. The simple thicken
  // can't do that (it needs open boundaries for the side walls).
  {
    ShapeAncMap ef;
    TopExp::MapShapesAndAncestors(base, TopAbs_EDGE, TopAbs_FACE, ef);
    bool closed = ef.Extent() > 0 && solid_count_of(base) == 0;
    for (int i = 1; i <= ef.Extent() && closed; ++i)
      closed = BRep_Tool::Degenerated(TopoDS::Edge(ef.FindKey(i))) || ef(i).Extent() == 2 ||
               (ef(i).Extent() == 1 && BRep_Tool::IsClosed(TopoDS::Edge(ef.FindKey(i)), TopoDS::Face(ef(i).First())));
    if (closed)
    {
      auto offset = [&](double d, HHist& h) -> TopoDS_Shape {
        if (std::abs(d) < 1e-12)
          return base;
        BRepOffsetAPI_MakeOffsetShape off;
        off.PerformByJoin(base, d, 1e-7, BRepOffset_Skin, false, false, GeomAbs_Arc);
        if (!off.IsDone())
          fail_op("thicken failed (thickness too large for the surface's curvature?)");
        merge_into(h, history_of({base}, off));
        return off.Shape();
      };
      HHist        h;
      TopoDS_Shape a = offset(t1, h), b = offset(-t2, h);
      auto solid_from = [](const TopoDS_Shape& x) -> TopoDS_Shape {
        BRepBuilderAPI_MakeSolid ms;
        int                      shells = 0;
        for (TopExp_Explorer e(x, TopAbs_SHELL); e.More(); e.Next(), ++shells)
          ms.Add(TopoDS::Shell(e.Current()));
        if (shells == 0)
        {
          // Bare faces (the input surface itself): one shell of them.
          BRep_Builder bb;
          TopoDS_Shell sh;
          bb.MakeShell(sh);
          for (TopExp_Explorer e(x, TopAbs_FACE); e.More(); e.Next())
            bb.Add(sh, e.Current());
          ms.Add(sh);
        }
        if (!ms.IsDone())
          return TopoDS_Shape();
        TopoDS_Solid so = TopoDS::Solid(ms.Shape());
        BRepLib::OrientClosedSolid(so);
        return so;
      };
      TopoDS_Shape sa = solid_from(a), sb = solid_from(b);
      if (sa.IsNull() || sb.IsNull())
        fail_op("thicken failed");
      if (volume_props(sa).Mass() < volume_props(sb).Mass())
        std::swap(sa, sb);
      BRepAlgoAPI_Cut cut(sa, sb);
      if (cut.HasErrors() || !cut.IsDone())
        fail_op("thicken failed");
      merge_into(h, history_of({sa, sb}, cut));
      TopoDS_Shape r = cut.Shape();
      if (!BRepCheck_Analyzer(r).IsValid() || !(volume_props(r).Mass() > 0))
        fail_op("thicken failed");
      return make_result(args, r, h.get());
    }
  }
  if (std::abs(t2) > 1e-12)
  {
    BRepOffsetAPI_MakeOffsetShape off;
    off.PerformBySimple(base, -t2);
    if (!off.IsDone())
      fail_op("thicken: offset of the second side failed");
    merge_into(hist, history_of({base}, off));
    base = off.Shape();
  }
  BRepOffsetAPI_MakeThickSolid mk;
  mk.MakeThickSolidBySimple(base, total);
  if (!mk.IsDone())
    fail_op("thicken failed");
  merge_into(hist, history_of({base}, mk));
  // BRepOffsetAPI_MakeThickSolid may return an inside-out solid (negative
  // volume) depending on the face orientation and the offset sign: orient
  // every solid outward. Only orientations change, so sub-shape identities
  // (and therefore the history) stay valid.
  return make_result(args, orient_solids_outward(mk.Shape()), hist.get());
}

OpOut op_shell(const OcShape& solid, const OcShapeList& faces, double thickness, uint8_t join)
{
  require_non_null(solid.shape, "solid");
  if (!(std::abs(thickness) > 1e-9))
    fail_invalid("shell thickness must be non-zero");
  check_faces_in(solid.shape, faces.items);
  const GeomAbs_JoinType    jt = join == 1 ? GeomAbs_Intersection : GeomAbs_Arc;
  std::vector<TopoDS_Shape> args{solid.shape};
  if (faces.items.empty())
  {
    // Closed hollow solid: offset the whole boundary, then subtract.
    BRepOffsetAPI_MakeOffsetShape off;
    off.PerformByJoin(solid.shape, thickness, 1e-6, BRepOffset_Skin, false, false, jt);
    if (!off.IsDone())
      fail_op("shell failed (thickness too large?)");
    TopoDS_Shape offs = off.Shape();
    for (TopExp_Explorer e(offs, TopAbs_SOLID); e.More(); e.Next())
    {
      TopoDS_Solid so = TopoDS::Solid(e.Current());
      BRepLib::OrientClosedSolid(so);
      offs = so;
      break;
    }
    HHist                hist = history_of(args, off);
    const TopoDS_Shape&  outer = thickness < 0 ? solid.shape : offs;
    const TopoDS_Shape&  inner = thickness < 0 ? offs : solid.shape;
    BRepAlgoAPI_Cut      cut(outer, inner);
    if (cut.HasErrors() || !cut.IsDone())
      fail_op("shell failed: " + algo_errors(cut));
    hist->Merge(history_of({outer, inner}, cut));
    return make_result(args, cut.Shape(), hist.get());
  }
  BRepOffsetAPI_MakeThickSolid mk;
  mk.MakeThickSolidByJoin(solid.shape, to_list(faces.items), thickness, 1e-6, BRepOffset_Skin,
                          false, false, jt);
  if (!mk.IsDone())
    fail_op("shell failed (thickness too large?)");
  HHist h = history_of(args, mk);
  return make_result(args, mk.Shape(), h.get());
}

// ---------------------------------------------------------------- fillet / chamfer
//
// `checked` blends must give a valid solid (Onshape never returns a broken
// part). OCCT fails, or returns an invalid solid, when a blend consumes a
// face exactly (radius = the face's width, e.g. R5 on every edge of a 10 mm
// block, or a chamfer as wide as the corner fillet it runs round); a blend a
// hair smaller (1e-7..1e-5 relative) succeeds, and the sliver faces it leaves
// are then removed. An input that is invalid already keeps OCCT's result.

// Closed and oriented: every edge bounds faces twice, once each way (a
// seam twice in its face). BRepCheck's topology-only pass cost 6 ms on live's
// 30-feature part.
bool closed_and_oriented(const TopoDS_Shape& r)
{
  NCollection_DataMap<TopoDS_Shape, std::pair<int, int>, TopTools_ShapeMapHasher> uses;
  for (TopExp_Explorer f(r, TopAbs_FACE); f.More(); f.Next())
    for (TopExp_Explorer e(f.Current(), TopAbs_EDGE); e.More(); e.Next())
    {
      if (BRep_Tool::Degenerated(TopoDS::Edge(e.Current())))
        continue;
      if (!uses.IsBound(e.Current()))
        uses.Bind(e.Current(), {0, 0});
      std::pair<int, int>& u = uses.ChangeFind(e.Current());
      ++u.first;
      u.second += e.Current().Orientation() == TopAbs_FORWARD ? 1 : e.Current().Orientation() == TopAbs_REVERSED ? -1 : 2;
    }
  for (decltype(uses)::Iterator it(uses); it.More(); it.Next())
    if (it.Value().first != 2 || it.Value().second != 0)
      return false;
  return true;
}

// An op's result checked where the op changed it (a blend's, a shell's) (REGEN-SPEED: the
// whole-part check cost more than the blend, 7.6 vs 5.9 ms a fillet on live's
// 30-feature part): its faces that aren't the input's in full (geometry,
// wires, edges, vertices: 3-5 of ~30 there), the whole shape only for
// closure and orientation. An input face kept as it is was the input's.
bool valid_changed(const TopoDS_Shape& input, const TopoDS_Shape& r)
{
  if (r.IsNull())
    return false;
  ShapeIMap old;
  TopExp::MapShapes(input, TopAbs_FACE, old);
  TopoDS_Compound changed;
  BRep_Builder    B;
  B.MakeCompound(changed);
  int n = 0;
  for (TopExp_Explorer e(r, TopAbs_FACE); e.More(); e.Next())
    if (!old.Contains(e.Current()))
    {
      B.Add(changed, e.Current());
      ++n;
    }
  if (!closed_and_oriented(r))
    return false;
  // In parallel for many faces (the pool's workers use this frame).
  BudgetHold hold;
  return n == 0 || BRepCheck_Analyzer(changed, true, n > 16).IsValid();
}


namespace {

// Builds one blend at `scale` x the requested sizes into (out, h); false if
// OCCT fails.
using BlendBuild = std::function<bool(double scale, TopoDS_Shape& out, HHist& h)>;

bool valid_shape(const TopoDS_Shape& s)
{
  // In parallel: the check of a whole part after each blend cost more than
  // the blend (live's 30-feature part, REGEN-SPEED). Not left by a budget
  // mid-check (the pool's workers use this frame).
  BudgetHold hold;
  return !s.IsNull() && BRepCheck_Analyzer(s, true, true).IsValid();
}

// Remove faces narrower than `width` (slivers left by a blend that almost
// consumes a face), composing the history.
TopoDS_Shape without_slivers(const TopoDS_Shape& s, double width, HHist& h)
{
  ShapeFix_FixSmallFace fx;
  fx.Init(s);
  fx.SetPrecision(width);
  fx.SetMaxTolerance(width * 10);
  fx.Perform();
  TopoDS_Shape r = fx.FixShape();
  if (!fx.Context().IsNull())
  {
    HHist rh = fx.Context()->History();
    if (!rh.IsNull())
      merge_into(h, rh);
  }
  return r;
}

// Runs `build` (see BlendBuild) with the fallbacks of checked blends; returns
// the result and its history relative to `input`.
std::pair<TopoDS_Shape, HHist> checked_blend(const TopoDS_Shape& input, const BlendBuild& build,
                                             bool checked, double size, const char* what)
{
  TopoDS_Shape r;
  HHist        h;
  bool         ok = false;
  try
  {
    ok = build(1.0, r, h);
  }
  catch (const Standard_Failure&)
  {
    if (!checked)
      throw;
  }
  if (!checked)
  {
    if (!ok)
      fail_op(std::string(what) + " failed (radius too large or unsupported edge configuration)");
    return {r, h};
  }
  if (ok && valid_shape(r))
    return {r, h};
  // Smallest step that works (an L bracket's 5 mm walls filleted R2.5 all
  // round need 1e-5; volume error ~1e-6 relative).
  for (const double eps : {1e-7, 1e-6, 1e-5})
  {
    try
    {
      TopoDS_Shape r2;
      HHist        h2;
      if (build(1.0 - eps, r2, h2))
      {
        TopoDS_Shape r3 = without_slivers(r2, std::max(size * eps * 50, 1e-9), h2);
        if (valid_shape(r3))
          return {r3, h2};
        if (valid_shape(r2))
          return {r2, h2};
      }
    }
    catch (const Standard_Failure&)
    {
    }
  }
  if (ok && !valid_shape(input))
    return {r, h};
  fail_op(std::string(what) + (ok ? " gave an invalid solid" : " failed"));
}

// History input -> output of two steps (BRepTools_History::Merge loses shapes
// Generated from shapes the first step Modified, e.g. a fillet face generated
// by an edge of a scaled copy).
HHist compose(const TopoDS_Shape& input, const HHist& h12, const HHist& h23)
{
  if (h23.IsNull())
    return h12;
  HHist r = new BRepTools_History();
  // Records the images of `y` (an image of `x` after step 1) after step 2;
  // false when `y` is removed.
  auto step2 = [&](const TopoDS_Shape& x, const TopoDS_Shape& y, bool generated) {
    // A removed shape (a filleted edge) can still have generated images.
    const ShapeList g = h23->Generated(y);
    for (ShapeList::Iterator it(g); it.More(); it.Next())
      r->AddGenerated(x, it.Value());
    if (h23->IsRemoved(y))
      return false;
    const ShapeList& m = h23->Modified(y);
    if (m.IsEmpty())
    {
      if (!y.IsSame(x))
        generated ? r->AddGenerated(x, y) : r->AddModified(x, y);
    }
    else
      for (ShapeList::Iterator it(m); it.More(); it.Next())
        generated ? r->AddGenerated(x, it.Value()) : r->AddModified(x, it.Value());
    return true;
  };
  for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
  {
    ShapeIMap in;
    TopExp::MapShapes(input, k, in);
    for (int i = 1; i <= in.Extent(); ++i)
    {
      const TopoDS_Shape& x = in(i);
      if (!h12.IsNull() && h12->IsRemoved(x))
      {
        // Removed in step 1 (a filleted edge) but with generated images.
        const ShapeList g = h12->Generated(x);
        for (ShapeList::Iterator it(g); it.More(); it.Next())
          step2(x, it.Value(), true);
        r->Remove(x);
        continue;
      }
      const ShapeList m    = h12.IsNull() ? ShapeList() : h12->Modified(x);
      bool            live = false;
      if (m.IsEmpty())
        live = step2(x, x, false);
      else
        for (ShapeList::Iterator it(m); it.More(); it.Next())
          live = step2(x, it.Value(), false) || live;
      if (!h12.IsNull())
      {
        const ShapeList g = h12->Generated(x);
        for (ShapeList::Iterator it(g); it.More(); it.Next())
          step2(x, it.Value(), true);
      }
      if (!live && r->Modified(x).IsEmpty() && r->Generated(x).IsEmpty())
        r->Remove(x);
    }
  }
  return r;
}

// The input of a blend at the working scale (see set_working_scale): OCCT's
// blend algorithms use absolute approximation tolerances tuned for
// millimetre-sized parts and fail on the same part modelled in metres.
struct Scaled
{
  Scaled(const TopoDS_Shape& s, double k)
      : factor(k), input(s)
  {
    if (k == 1.0)
    {
      shape = s;
      return;
    }
    gp_Trsf t;
    t.SetScale(gp_Pnt(0, 0, 0), k);
    BRepBuilderAPI_Transform tr(s, t, true);
    shape = tr.Shape();
    to    = new BRepTools_History(to_list({s}), tr);
  }
  // Image of a sub-shape of the input in the scaled shape.
  TopoDS_Shape image(const TopoDS_Shape& x) const
  {
    if (to.IsNull())
      return x;
    const ShapeList& m = to->Modified(x);
    return m.IsEmpty() ? x : m.First();
  }
  // Result of the scaled operation, back at the input's scale, with the
  // history from the input.
  OpOut finish(const TopoDS_Shape& r, const HHist& h) const
  {
    if (to.IsNull())
      return make_result({input}, r, h.get());
    auto [out, all] = unscaled(r, h);
    return make_result({input}, out, all.get());
  }
  // The same as a shape and history (`h` non-null).
  std::pair<TopoDS_Shape, HHist> unscaled(const TopoDS_Shape& r, const HHist& h) const
  {
    if (to.IsNull())
      return {r, h};
    gp_Trsf t;
    t.SetScale(gp_Pnt(0, 0, 0), 1.0 / factor);
    BRepBuilderAPI_Transform tr(r, t, true);
    HHist back = new BRepTools_History(to_list({r}), tr);
    return {tr.Shape(), compose(input, compose(input, to, h), back)};
  }
  double       factor;
  TopoDS_Shape input, shape;
  HHist        to;
};

// A checked blend tries the input as it is first (the common case: no
// copies, results identical to an unchecked blend); only when OCCT fails or
// gives an invalid solid does it run at the working scale with the fallbacks.
// `make(sc)` returns the BlendBuild for the input scaled by `sc`.
using MakeBuild = std::function<BlendBuild(const Scaled& sc)>;


OpOut scaled_blend(const TopoDS_Shape& input, bool checked, double size, const char* what,
                   const MakeBuild& make)
{
  if (checked)
  {
    const Scaled as_is(input, 1.0);
    TopoDS_Shape r;
    HHist        h;
    bool         ok = false;
    try
    {
      const KernelTimer timer("blend.as_is");
      ok = make(as_is)(1.0, r, h);
    }
    catch (const Standard_Failure&)
    {
    }
    bool valid = false;
    {
      const KernelTimer timer("blend.as_is_check");
      valid = ok && valid_changed(input, r);
    }
    if (valid)
    {
      const KernelTimer timer("blend.finish");
      return as_is.finish(r, h);
    }
  }
  const KernelTimer timer("blend.scaled");
  const Scaled sc(input, checked ? working_scale() : 1.0);
  auto [r, h] = checked_blend(sc.shape, make(sc), checked, size * sc.factor, what);
  return sc.finish(r, h);
}

// Images of `x` through `h` (null: unchanged), or nothing when removed.
void add_images(const HHist& h, const TopoDS_Shape& x, const TopoDS_Shape& from, const HHist& r)
{
  if (h.IsNull())
  {
    r->AddModified(from, x);
    return;
  }
  const ShapeList g = h->Generated(x);
  for (ShapeList::Iterator it(g); it.More(); it.Next())
    r->AddGenerated(from, it.Value());
  if (h->IsRemoved(x))
    return;
  const ShapeList& m = h->Modified(x);
  if (m.IsEmpty())
    r->AddModified(from, x);
  for (ShapeList::Iterator it(m); it.More(); it.Next())
    r->AddModified(from, it.Value());
}

// A fillet whose edges end against a face they don't bound (live G46: a
// fillet after a chamfer ends on the chamfer face) where OCCT can't run the
// fillet out: built on the part without those faces (removed, their
// neighbours extended back), then trimmed back to the part. That is the
// solid Parasolid makes (G46: 44487.2158 mm³, 9/21/14 faces/edges/vertices
// against Onshape's 44487.2085, 9/21/14). Null when it doesn't apply.
std::optional<std::pair<TopoDS_Shape, HHist>> fillet_past_end_faces(const TopoDS_Shape& input, const std::vector<TopoDS_Edge>& es,
                                                                   const std::vector<double>& rs)
{
  ShapeAncMap e2f, v2f;
  TopExp::MapShapesAndAncestors(input, TopAbs_EDGE, TopAbs_FACE, e2f);
  TopExp::MapShapesAndAncestors(input, TopAbs_VERTEX, TopAbs_FACE, v2f);
  ShapeIMap ends;
  for (const TopoDS_Edge& e : es)
  {
    const ShapeList* own = e2f.Seek(e);
    for (TopExp_Explorer v(e, TopAbs_VERTEX); v.More(); v.Next())
    {
      const ShapeList* at = v2f.Seek(v.Current());
      if (at == nullptr)
        continue;
      for (ShapeList::Iterator it(*at); it.More(); it.Next())
      {
        bool mine = false;
        if (own != nullptr)
          for (ShapeList::Iterator o(*own); o.More(); o.Next())
            mine = mine || o.Value().IsSame(it.Value());
        BRepAdaptor_Surface a(TopoDS::Face(it.Value()), false);
        if (!mine && a.GetType() == GeomAbs_Plane)
          ends.Add(it.Value());
      }
    }
  }
  if (ends.IsEmpty())
    return std::nullopt;
  for (int i = 1; i <= ends.Extent(); ++i)
    for (const TopoDS_Edge& e : es)
      for (TopExp_Explorer x(ends(i), TopAbs_EDGE); x.More(); x.Next())
        if (x.Current().IsSame(e))
          return std::nullopt;
  try
  {
    BRepAlgoAPI_Defeaturing df;
    df.SetShape(input);
    ShapeList faces;
    for (int i = 1; i <= ends.Extent(); ++i)
      faces.Append(ends(i));
    df.AddFacesToRemove(faces);
    df.SetToFillHistory(true);
    df.Build();
    if (!df.IsDone() || df.HasErrors())
      return std::nullopt;
    const TopoDS_Shape full = df.Shape();
    if (!BRepCheck_Analyzer(full).IsValid())
      return std::nullopt;
    HHist              h1   = history_of({input}, df);
    BRepFilletAPI_MakeFillet mk(full);
    for (size_t i = 0; i < es.size(); ++i)
    {
      const ShapeList im = images_of(h1, es[i]);
      if (im.Extent() != 1)
        return std::nullopt;
      mk.Add(rs[i], TopoDS::Edge(im.First()));
    }
    // OCCT's fillet can crash on the extended part (qa FILLET-SEGV:
    // ChFi3d_Builder::PerformIntersectionAtEnd): then this doesn't apply.
    if (!run_guarded([&] { mk.Build(); }) || !mk.IsDone())
      return std::nullopt;
    HHist        h2      = history_of({full}, mk);
    TopoDS_Shape rounded = mk.Shape();
    HHist        corners;
    rounded = exact_vertex_blends(rounded, corners);
    if (!corners.IsNull())
      h2 = compose(full, h2, corners);
    BRepAlgoAPI_Common cm(rounded, input);
    cm.SetToFillHistory(true);
    cm.Build();
    if (!cm.IsDone() || cm.HasErrors())
      return std::nullopt;
    TopoDS_Shape out = cm.Shape();
    int solids = 0;
    for (TopExp_Explorer x(out, TopAbs_SOLID); x.More(); x.Next())
      ++solids;
    if (!valid_shape(out) || solids != 1)
      return std::nullopt;
    HHist h3 = history_of({rounded, input}, cm);
    // Every input shape's images: through the rounded part, and as an
    // argument of the trim itself; removed only when both lose it.
    HHist through = compose(input, compose(input, h1, h2), h3);
    HHist r       = new BRepTools_History();
    for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    {
      ShapeIMap in;
      TopExp::MapShapes(input, k, in);
      for (int i = 1; i <= in.Extent(); ++i)
      {
        const TopoDS_Shape& x = in(i);
        HHist               one = new BRepTools_History();
        add_images(through, x, x, one);
        add_images(h3, x, x, one);
        const ShapeList m = one->Modified(x);
        const ShapeList g = one->Generated(x);
        ShapeIMap       seen;
        bool            any = false;
        for (ShapeList::Iterator it(m); it.More(); it.Next())
        {
          if (seen.Contains(it.Value()))
            continue;
          seen.Add(it.Value());
          any = true;
          if (!it.Value().IsSame(x))
            r->AddModified(x, it.Value());
        }
        for (ShapeList::Iterator it(g); it.More(); it.Next())
        {
          r->AddGenerated(x, it.Value());
          any = true;
        }
        if (!any)
          r->Remove(x);
      }
    }
    return std::make_pair(out, r);
  }
  catch (const Standard_Failure&)
  {
    return std::nullopt;
  }
}

} // namespace

// A fillet wider than the faces it rolls on, which OCCT can't build (live
// "fail preview 2": 50 mm on a top edge of a 40 x 30 x 20 mm block). Onshape
// regenerates it as the part cut by the ball's sweep: the section between the
// edge, its tangent points on the two planes and the ball's centre, less the
// ball, along the edge (24000 -> 7458.6158 mm3, matched exactly; where OCCT
// can build the fillet, the same solid as OCCT's). Only straight convex edges
// between two planes, each end closed by a plane square to the edge, no two
// edges sharing a vertex. Null when it doesn't apply.
std::optional<std::pair<TopoDS_Shape, HHist>> fillet_as_cut(const TopoDS_Shape& input, const std::vector<TopoDS_Edge>& es,
                                                            const std::vector<double>& rs)
{
  ShapeAncMap e2f, v2f;
  TopExp::MapShapesAndAncestors(input, TopAbs_EDGE, TopAbs_FACE, e2f);
  TopExp::MapShapesAndAncestors(input, TopAbs_VERTEX, TopAbs_FACE, v2f);
  // A planar face's outward normal.
  auto outward = [](const TopoDS_Face& f, gp_Vec& n) {
    BRepAdaptor_Surface a(f, false);
    if (a.GetType() != GeomAbs_Plane)
      return false;
    gp_Dir d = a.Plane().Axis().Direction();
    if (!a.Plane().Position().Direct())
      d.Reverse();
    if (f.Orientation() == TopAbs_REVERSED)
      d.Reverse();
    n = gp_Vec(d);
    return true;
  };
  ShapeIMap                 used;
  ShapeList                 tools;
  // Per edge: the ball's axis (a point and the edge's direction) and radius.
  std::vector<std::pair<gp_Ax1, double>> balls;
  for (size_t i = 0; i < es.size(); ++i)
  {
    const TopoDS_Edge& E = es[i];
    const double       r = rs.size() == 1 ? rs[0] : rs[i];
    BRepAdaptor_Curve  c(E);
    const ShapeList*   fl = e2f.Seek(E);
    if (c.GetType() != GeomAbs_Line || fl == nullptr || fl->Extent() != 2)
      return std::nullopt;
    const TopoDS_Face f1 = TopoDS::Face(fl->First()), f2 = TopoDS::Face(fl->Last());
    gp_Vec            n1, n2;
    if (f1.IsSame(f2) || !outward(f1, n1) || !outward(f2, n2))
      return std::nullopt;
    // Convex: (n1 x n2) . t > 0, t the edge's direction as face 1 runs it.
    TopoDS_Edge in1;
    for (TopExp_Explorer x(f1, TopAbs_EDGE); x.More(); x.Next())
      if (x.Current().IsSame(E))
        in1 = TopoDS::Edge(x.Current());
    if (in1.IsNull())
      return std::nullopt;
    TopoDS_Vertex va, vb;
    TopExp::Vertices(in1, va, vb, true);
    if (va.IsNull() || vb.IsNull() || va.IsSame(vb) || used.Contains(va) || used.Contains(vb))
      return std::nullopt;
    used.Add(va);
    used.Add(vb);
    const gp_Pnt a = BRep_Tool::Pnt(va), b = BRep_Tool::Pnt(vb);
    const gp_Vec t(a, b);
    const double L = t.Magnitude();
    if (L < Precision::Confusion() || n1.Crossed(n2).Dot(t) <= 1e-9 * L)
      return std::nullopt;
    const gp_Vec u = t / L;
    // Each end: three faces, the third a plane square to the edge.
    for (const TopoDS_Vertex& v : {va, vb})
    {
      const ShapeList* at = v2f.Seek(v);
      ShapeIMap        faces;
      if (at != nullptr)
        for (ShapeList::Iterator it(*at); it.More(); it.Next())
          faces.Add(it.Value());
      if (faces.Extent() != 3)
        return std::nullopt;
      bool capped = false;
      for (int k = 1; k <= 3; ++k)
      {
        gp_Vec n;
        capped = capped || (!faces(k).IsSame(f1) && !faces(k).IsSame(f2) && outward(TopoDS::Face(faces(k)), n) && std::abs(std::abs(n.Dot(u)) - 1) < 1e-9);
      }
      if (!capped)
        return std::nullopt;
    }
    // The section: edge point, tangent point on face 1, ball centre, on face 2.
    const double half = std::acos(std::max(-1.0, std::min(1.0, n1.Dot(n2)))) / 2;
    const gp_Vec bis  = (n1 + n2).Normalized();
    const gp_Pnt C    = a.Translated(-bis * (r / std::cos(half)));
    BRepBuilderAPI_MakePolygon sec(a, C.Translated(n1 * r), C, C.Translated(n2 * r), true);
    if (!sec.IsDone())
      return std::nullopt;
    const TopoDS_Shape wedge = BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(sec.Wire(), true).Face(), t).Shape();
    const TopoDS_Shape ball  = BRepPrimAPI_MakeCylinder(gp_Ax2(C, gp_Dir(u)), r, L).Shape();
    BRepAlgoAPI_Cut    tool(wedge, ball);
    if (!tool.IsDone() || tool.HasErrors())
      return std::nullopt;
    tools.Append(tool.Shape());
    balls.emplace_back(gp_Ax1(C, gp_Dir(u)), r);
  }
  BRepAlgoAPI_Cut      cut;
  ShapeList            args;
  args.Append(input);
  cut.SetArguments(args);
  cut.SetTools(tools);
  cut.Build();
  if (!cut.IsDone() || cut.HasErrors() || !valid_shape(cut.Shape()))
    return std::nullopt;
  HHist h = history_of({input}, cut);
  // The faces the cut made: from the ball they belong to, generated by its edge.
  ShapeIMap old;
  for (TopExp_Explorer f(input, TopAbs_FACE); f.More(); f.Next())
  {
    const ShapeList& m = h->Modified(f.Current());
    for (ShapeList::Iterator it(m); it.More(); it.Next())
      old.Add(it.Value());
    if (m.IsEmpty() && !h->IsRemoved(f.Current()))
      old.Add(f.Current());
  }
  for (TopExp_Explorer f(cut.Shape(), TopAbs_FACE); f.More(); f.Next())
  {
    if (old.Contains(f.Current()))
      continue;
    BRepAdaptor_Surface a(TopoDS::Face(f.Current()), false);
    if (a.GetType() != GeomAbs_Cylinder)
      continue;
    for (size_t i = 0; i < balls.size(); ++i)
      if (std::abs(a.Cylinder().Radius() - balls[i].second) < 1e-9 * balls[i].second && gp_Lin(balls[i].first).Distance(a.Cylinder().Location()) < 1e-9 * balls[i].second)
        h->AddGenerated(es[i], f.Current());
  }
  return std::make_pair(cut.Shape(), h);
}

// Whether a fillet that failed would have to roll past the faces it touches
// (Parasolid's "Unfilleted edge overlapped by fillet."; live "kern blend
// sweep" 2a: R1 round a Ø6 hole whose rim lies on an R8 fillet, where the
// rim meets that fillet at 29° the ball can't sit). Along each edge, the ball
// centre at distance r from both surfaces (Newton in the plane across the
// edge): where there is none, the fillet can't stay on its faces.
bool fillet_overflows(const TopoDS_Shape& input, const std::vector<TopoDS_Edge>& es, const std::vector<double>& rs)
{
  ShapeAncMap e2f;
  TopExp::MapShapesAndAncestors(input, TopAbs_EDGE, TopAbs_FACE, e2f);
  // Signed distance into the material from face f's surface, its foot and gradient.
  auto dist = [](const TopoDS_Face& f, const gp_Pnt& c, gp_Pnt2d& uv, gp_Vec& grad) {
    occ::handle<Geom_Surface>  sf = BRep_Tool::Surface(f);
    GeomAPI_ProjectPointOnSurf pr(c, sf);
    if (pr.NbPoints() == 0)
      return std::numeric_limits<double>::quiet_NaN();
    double u, v;
    pr.LowerDistanceParameters(u, v);
    uv = gp_Pnt2d(u, v);
    gp_Pnt p;
    gp_Vec du, dv;
    sf->D1(u, v, p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (n.Magnitude() < 1e-12)
      return std::numeric_limits<double>::quiet_NaN();
    n.Normalize();
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    grad = -n; // into the material
    return gp_Vec(c, p).Dot(n);
  };
  for (size_t i = 0; i < es.size(); ++i)
  {
    const double     r  = rs.size() == 1 ? rs[0] : rs[i];
    const ShapeList* fl = e2f.Seek(es[i]);
    if (fl == nullptr || fl->Extent() != 2)
      continue;
    const TopoDS_Face f1 = TopoDS::Face(fl->First()), f2 = TopoDS::Face(fl->Last());
    BRepAdaptor_Curve c(es[i]);
    for (int k = 0; k < 16; ++k)
    {
      const double par = c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * (k + 0.5) / 16;
      gp_Pnt       P;
      gp_Vec       t;
      c.D1(par, P, t);
      if (t.Magnitude() < 1e-12)
        continue;
      gp_Pnt2d uv1, uv2;
      gp_Vec   g1, g2;
      if (std::isnan(dist(f1, P, uv1, g1)) || std::isnan(dist(f2, P, uv2, g2)))
        continue;
      // Convex edges only (the ball inside the material, between the faces).
      const gp_Vec bis = g1 + g2;
      if (bis.Magnitude() < 1e-9)
        continue;
      gp_Pnt C = P.Translated(bis.Normalized() * r);
      const gp_Vec u = (g1 - t.Normalized() * g1.Dot(t.Normalized())).Normalized();
      const gp_Vec v = t.Normalized().Crossed(u);
      bool ok = false;
      for (int it = 0; it < 40; ++it)
      {
        const double d1 = dist(f1, C, uv1, g1), d2 = dist(f2, C, uv2, g2);
        if (std::isnan(d1) || std::isnan(d2))
          break;
        const double e1 = d1 - r, e2 = d2 - r;
        if (std::abs(e1) < 1e-9 * (1 + r) && std::abs(e2) < 1e-9 * (1 + r))
        {
          ok = true;
          break;
        }
        const double a = g1.Dot(u), b = g1.Dot(v), cc = g2.Dot(u), d = g2.Dot(v), det = a * d - b * cc;
        if (std::abs(det) < 1e-12)
          break;
        const double su = (-e1 * d + e2 * b) / det, sv = (-a * e2 + cc * e1) / det;
        C.Translate(u * su + v * sv);
        if (C.Distance(P) > 100 * r)
          break;
      }
      // (A contact beyond its face isn't it: Parasolid lets the fillet run
      // over onto the next face, and fails otherwise with its general
      // message, live F2: R12 on a 10 mm block's top edges.)
      if (!ok)
        return true;
    }
  }
  return false;
}

// BRepFilletAPI_MakeFillet on `es` (radii `rs`) of the input scaled by `sc`.
static BlendBuild fillet_build(const Scaled& sc, const std::vector<TopoDS_Edge>& es,
                               const std::vector<double>& rs)
{
  return [&sc, &es, &rs](double scale, TopoDS_Shape& out, HHist& h) {
    BRepFilletAPI_MakeFillet mk(sc.shape);
    for (size_t i = 0; i < es.size(); ++i)
      mk.Add(rs[i] * scale * sc.factor, TopoDS::Edge(sc.image(es[i])));
    mk.Build();
    if (!mk.IsDone())
      return false;
    out = mk.Shape();
    h   = history_of({sc.shape}, mk);
    // OCCT's patch where the loop runs over smaller corner fillets -> Parasolid's.
    HHist corners;
    out = exact_vertex_blends(out, corners);
    if (!corners.IsNull())
      h = compose(sc.shape, h, corners);
    return true;
  };
}

// A fillet along a closed edge that OCCT can't take all the way round (skb:
// the 0.002 in mouth of the #8 flat countersunk Type VI recess, a closed
// B-spline with its seam vertex on a lobe): the same fillet with the edge
// split in two at the middle of its range, which OCCT builds as one face,
// Parasolid's (live: 14 faces, the fillet one B-spline face). Null when no
// edge is closed or that fails too.
static std::optional<std::pair<TopoDS_Shape, HHist>> fillet_split_closed(
    const TopoDS_Shape& input, const std::vector<TopoDS_Edge>& es, const std::vector<double>& rs,
    double rmax)
{
  ShapeAncMap e2f;
  TopExp::MapShapesAndAncestors(input, TopAbs_EDGE, TopAbs_FACE, e2f);
  occ::handle<BRepTools_ReShape>    reshape = new BRepTools_ReShape;
  BRep_Builder                      B;
  std::vector<TopoDS_Edge>          es2;
  std::vector<double>               rs2;
  std::vector<std::pair<TopoDS_Edge, std::vector<TopoDS_Edge>>> split;
  for (size_t i = 0; i < es.size(); ++i)
  {
    const TopoDS_Edge fe = TopoDS::Edge(es[i].Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     v0, v1;
    TopExp::Vertices(fe, v0, v1);
    double                    f = 0, l = 0;
    occ::handle<Geom_Curve>   c  = BRep_Tool::Curve(fe, f, l);
    const ShapeList*          fl = e2f.Seek(fe);
    bool                      ok = !c.IsNull() && !v0.IsNull() && v0.IsSame(v1) && fl != nullptr;
    if (ok)
      for (ShapeList::Iterator it(*fl); it.More(); it.Next())
        ok = ok && !BRep_Tool::IsClosed(fe, TopoDS::Face(it.Value()));
    if (!ok)
    {
      es2.push_back(es[i]);
      rs2.push_back(rs[i]);
      continue;
    }
    const double  tol = BRep_Tool::Tolerance(fe), m = 0.5 * (f + l);
    TopoDS_Vertex vm;
    B.MakeVertex(vm, c->Value(m), tol);
    auto piece = [&](double a, double b, const TopoDS_Vertex& va, const TopoDS_Vertex& vb) {
      TopoDS_Edge n;
      B.MakeEdge(n, c, tol);
      B.Add(n, va.Oriented(TopAbs_FORWARD));
      B.Add(n, vb.Oriented(TopAbs_REVERSED));
      B.Range(n, a, b);
      for (ShapeList::Iterator it(*fl); it.More(); it.Next())
      {
        const TopoDS_Face&        F = TopoDS::Face(it.Value());
        double                    pf, pl;
        occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(fe, F, pf, pl);
        B.UpdateEdge(n, pc, F, tol);
        B.Range(n, F, a, b);
      }
      B.SameRange(n, BRep_Tool::SameRange(fe));
      B.SameParameter(n, BRep_Tool::SameParameter(fe));
      return n;
    };
    const std::vector<TopoDS_Edge> two = {piece(f, m, v0, vm), piece(m, l, vm, v0)};
    TopoDS_Compound                cc;
    B.MakeCompound(cc);
    for (const TopoDS_Edge& n : two)
    {
      B.Add(cc, n);
      es2.push_back(n);
      rs2.push_back(rs[i]);
    }
    reshape->Replace(fe, cc);
    split.emplace_back(fe, two);
  }
  if (split.empty())
    return std::nullopt;
  try
  {
    const TopoDS_Shape cut = reshape->Apply(input);
    if (!valid_shape(cut))
      return std::nullopt;
    // Input -> split: the closed edges to their halves, the rest as rebuilt.
    HHist       rh = reshape->History();
    HHist       h1 = new BRepTools_History();
    for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    {
      ShapeIMap in;
      TopExp::MapShapes(input, k, in);
      for (int i = 1; i <= in.Extent(); ++i)
      {
        const TopoDS_Shape& x    = in(i);
        bool                done = false;
        for (const auto& [e, two] : split)
          if (e.IsSame(x))
          {
            for (const TopoDS_Edge& n : two)
              h1->AddModified(x, n);
            done = true;
          }
        if (done || rh.IsNull())
          continue;
        const ShapeList& m = rh->Modified(x);
        for (ShapeList::Iterator it(m); it.More(); it.Next())
          h1->AddModified(x, it.Value());
        if (rh->IsRemoved(x))
          h1->Remove(x);
      }
    }
    const Scaled sc(cut, working_scale());
    auto [r, h] = checked_blend(sc.shape, fillet_build(sc, es2, rs2), true, rmax * sc.factor, "fillet");
    auto [out, h2] = sc.unscaled(r, h.IsNull() ? HHist(new BRepTools_History()) : h);
    return std::make_pair(out, compose(input, h1, h2));
  }
  catch (const Standard_Failure&)
  {
    return std::nullopt;
  }
  catch (const std::runtime_error&)
  {
    return std::nullopt;
  }
}

// CPU seconds a fillet, chamfer, draft or face deletion may take (qa
// FILLET-HANG: > 1 h in OCCT's blend approximation; DRAFT-HANG: > 20 min in
// its defeaturing); NOSHAPE_BLEND_BUDGET overrides (tests).
double blend_budget_seconds()
{
  const char* v = std::getenv("NOSHAPE_BLEND_BUDGET");
  return v != nullptr ? std::atof(v) : 30.0;
}

static OpOut op_fillet_unbudgeted(const OcShape& s, const OcShapeList& edges, rust::Slice<const double> radii,
                bool checked)
{
  require_non_null(s.shape, "shape");
  if (std::getenv("NOSHAPE_DUMP_OPS"))
  {
    std::ostringstream p;
    p.precision(17);
    p << "fillet checked " << checked << " radii";
    for (double r : radii)
      p << " " << r;
    dump_op("fillet", s.shape, edges.items, p.str());
  }
  if (edges.items.empty())
    fail_invalid("fillet needs at least one edge");
  if (radii.size() != 1 && radii.size() != edges.items.size())
    fail_invalid("fillet radii must be one value or one per edge");
  std::vector<TopoDS_Edge> es;
  std::vector<double>      rs;
  double                   rmax = 0;
  for (size_t i = 0; i < edges.items.size(); ++i)
  {
    const double r = radii.size() == 1 ? radii[0] : radii[i];
    if (!(r > 1e-9))
      fail_invalid("fillet radius must be positive");
    es.push_back(first_adjacent_face_edge_check(s.shape, edges.items[i], "fillet edge"));
    rs.push_back(r);
    rmax = std::max(rmax, r);
  }
  if (checked)
  {
    const SlabBlend blend = [&](const TopoDS_Shape& slab, const std::vector<int>& idx,
                                const std::function<TopoDS_Shape(const TopoDS_Shape&)>& image, TopoDS_Shape& r, HHist& rh) {
      BRepFilletAPI_MakeFillet mk(slab);
      for (int i : idx)
        mk.Add(rs[(size_t)i], TopoDS::Edge(image(es[(size_t)i])));
      mk.Build();
      if (!mk.IsDone())
        return false;
      r  = mk.Shape();
      rh = history_of({slab}, mk);
      HHist corners;
      r = exact_vertex_blends(r, corners);
      if (!corners.IsNull())
        rh = compose(slab, rh, corners);
      return valid_shape(r);
    };
    TopoDS_Shape out;
    HHist        h;
    if (blend_in_slabs(s.shape, es, 2 * rmax, blend, out, h))
      return make_result({s.shape}, out, h.get());
  }
  try
  {
    return scaled_blend(s.shape, checked, rmax, "fillet",
                        [&](const Scaled& sc) { return fillet_build(sc, es, rs); });
  }
  catch (const std::runtime_error&)
  {
    if (!checked)
      throw;
    auto past = fillet_split_closed(s.shape, es, rs, rmax);
    if (!past)
      past = fillet_past_end_faces(s.shape, es, rs);
    if (!past)
      past = fillet_as_cut(s.shape, es, rs);
    if (!past && fillet_overflows(s.shape, es, rs))
      fail_op("fillet failed: unfilleted edge overlapped by fillet");
    if (!past)
      throw;
    return make_result({s.shape}, past->first, past->second.get());
  }
}

OpOut op_fillet(const OcShape& s, const OcShapeList& edges, rust::Slice<const double> radii,
                bool checked)
{
  OpOut out;
  const int r = run_budgeted(blend_budget_seconds(), [&] { out = op_fillet_unbudgeted(s, edges, radii, checked); });
  if (r == 1)
    fail_op("fillet took too long");
  if (r == 2)
    fail_op("cancelled");
  return out;
}

static OpOut op_fillet_variable_unbudgeted(const OcShape& s, const OcShape& edge, rust::Slice<const double> params,
                         rust::Slice<const double> radii, bool checked)
{
  require_non_null(s.shape, "shape");
  if (params.size() != radii.size() || params.size() < 2)
    fail_invalid("variable fillet needs >= 2 (parameter, radius) pairs");
  double rmax = 0;
  for (size_t i = 0; i < params.size(); ++i)
  {
    if (!(radii[i] > 1e-9))
      fail_invalid("fillet radius must be positive");
    if (params[i] < 0 || params[i] > 1)
      fail_invalid("variable fillet parameters must be in [0, 1]");
    if (i > 0 && !(params[i] > params[i - 1]))
      fail_invalid("variable fillet parameters must be increasing");
    rmax = std::max(rmax, radii[i]);
  }
  const TopoDS_Edge   e = first_adjacent_face_edge_check(s.shape, edge.shape, "fillet edge");
  std::vector<double> ps(params.begin(), params.end()), rs(radii.begin(), radii.end());
  return scaled_blend(s.shape, checked, rmax, "variable fillet", [&](const Scaled& sc) -> BlendBuild {
    return [&sc, &ps, &rs, e](double scale, TopoDS_Shape& out, HHist& h) {
      NCollection_Array1<gp_Pnt2d> ur(1, (int)ps.size());
      for (size_t i = 0; i < ps.size(); ++i)
        ur((int)i + 1) = gp_Pnt2d(ps[i], rs[i] * scale * sc.factor);
      BRepFilletAPI_MakeFillet mk(sc.shape);
      mk.Add(ur, TopoDS::Edge(sc.image(e)));
      mk.Build();
      if (!mk.IsDone())
        return false;
      out = mk.Shape();
      h   = history_of({sc.shape}, mk);
      return true;
    };
  });
}

OpOut op_fillet_variable(const OcShape& s, const OcShape& edge, rust::Slice<const double> params,
                         rust::Slice<const double> radii, bool checked)
{
  OpOut out;
  const int r = run_budgeted(blend_budget_seconds(), [&] { out = op_fillet_variable_unbudgeted(s, edge, params, radii, checked); });
  if (r == 1)
    fail_op("fillet variable took too long");
  if (r == 2)
    fail_op("cancelled");
  return out;
}

namespace {

// A rational quadratic arc: poles and weights.
struct Arc3
{
  gp_Pnt p[3];
  double w[3];
};

// The section of a fillet cylinder by the plane through A and P (both on it)
// that contains `t`, from A to P: an ellipse arc, exact as a rational
// quadratic (the circle arc's poles moved along the axis onto the plane).
std::optional<Arc3> fillet_section(const gp_Cylinder& cyl, const gp_Pnt& A, const gp_Pnt& P, const gp_Dir& t)
{
  const gp_Ax1 ax = cyl.Axis();
  const double R  = cyl.Radius();
  const gp_Vec at(ax.Direction());
  auto         on_axis = [&](const gp_Pnt& x) { return ax.Location().Translated(at * gp_Vec(ax.Location(), x).Dot(at)); };
  const gp_Pnt OA = on_axis(A);
  gp_Vec       ea(OA, A), eb(on_axis(P), P);
  if (std::abs(ea.Magnitude() - R) > 1e-6 * R || std::abs(eb.Magnitude() - R) > 1e-6 * R)
    return std::nullopt;
  ea.Normalize();
  eb.Normalize();
  const double th = ea.Angle(eb);
  if (th < 1e-3 || th > M_PI - 1e-3)
    return std::nullopt;
  const gp_Vec ep = (eb - ea * eb.Dot(ea)).Normalized();
  gp_Vec       n  = gp_Vec(A, P).Crossed(gp_Vec(t));
  if (n.Magnitude() < 1e-12 * R)
    return std::nullopt;
  n.Normalize();
  const double nt = n.Dot(at);
  if (std::abs(nt) < 1e-9)
    return std::nullopt;
  const double ab[3][2] = {{1, 0}, {1, std::tan(th / 2)}, {std::cos(th), std::sin(th)}};
  Arc3         r;
  for (int i = 0; i < 3; ++i)
  {
    const gp_Pnt q = OA.Translated(ea * (R * ab[i][0]) + ep * (R * ab[i][1]));
    r.p[i]         = q.Translated(at * (n.Dot(gp_Vec(q, A)) / nt));
    r.w[i]         = i == 1 ? std::cos(th / 2) : 1.0;
  }
  if (r.p[2].Distance(P) > 1e-6 * R)
    return std::nullopt;
  return r;
}

occ::handle<Geom_BSplineCurve> arc_curve(const Arc3& a)
{
  NCollection_Array1<gp_Pnt> p(1, 3);
  NCollection_Array1<double> w(1, 3), k(1, 2);
  NCollection_Array1<int>    m(1, 2);
  for (int i = 0; i < 3; ++i)
  {
    p(i + 1) = a.p[i];
    w(i + 1) = a.w[i];
  }
  k(1) = 0;
  k(2) = 1;
  m(1) = m(2) = 3;
  return new Geom_BSplineCurve(p, w, k, m, 2);
}

TopoDS_Face polygon_face(const std::vector<gp_Pnt>& pts)
{
  BRepBuilderAPI_MakePolygon mp;
  for (const gp_Pnt& p : pts)
    mp.Add(p);
  mp.Close();
  return BRepBuilderAPI_MakeFace(mp.Wire(), true).Face();
}

// Outward normal of a planar face.
gp_Dir face_normal(const TopoDS_Face& f)
{
  BRepAdaptor_Surface a(f, false);
  gp_Dir              n = a.Plane().Axis().Direction();
  // A left-handed frame's surface normal is minus its axis.
  if (!a.Plane().Position().Direct())
    n.Reverse();
  return f.Orientation() == TopAbs_REVERSED ? n.Reversed() : n;
}

// Equal-distance chamfers of straight edges between two planes that end, at
// one vertex, where two fillets on those planes meet (live G46 / G49: a box
// with R3 fillets on two top edges, the vertical edge between them
// chamfered). Parasolid runs the chamfer face up to the fillets' tangent
// lines and closes the corner with a blend patch: ruled between the
// fillets' sections by the planes through the chamfer's top corners and the
// point where the fillets meet (on the top face), along the chamfered edge's
// direction; rulings join equal angles on the two fillets. Built here as the
// part minus a tool bounded by the chamfer plane, those section planes and
// the patch. Onshape's patch is a cubic approximation of that surface
// (faces, edges and areas match to 0.007 mm²; volume 44790.075 against
// 44789.973 read from Onshape's own Parasolid file). Null when it doesn't apply.
std::optional<std::pair<TopoDS_Shape, HHist>> chamfer_into_fillet_corners(const TopoDS_Shape& input, const std::vector<TopoDS_Edge>& es, double d)
{
  ShapeAncMap e2f, v2f, v2e;
  TopExp::MapShapesAndAncestors(input, TopAbs_EDGE, TopAbs_FACE, e2f);
  TopExp::MapShapesAndAncestors(input, TopAbs_VERTEX, TopAbs_FACE, v2f);
  TopExp::MapShapesAndAncestors(input, TopAbs_VERTEX, TopAbs_EDGE, v2e);
  auto faces_of = [](const ShapeAncMap& m, const TopoDS_Shape& x) {
    std::vector<TopoDS_Face> out;
    if (const ShapeList* l = m.Seek(x))
      for (ShapeList::Iterator it(*l); it.More(); it.Next())
      {
        bool seen = false;
        for (const TopoDS_Face& f : out)
          seen = seen || f.IsSame(it.Value());
        if (!seen)
          out.push_back(TopoDS::Face(it.Value()));
      }
    return out;
  };
  auto is_plane = [](const TopoDS_Face& f) { return BRepAdaptor_Surface(f, false).GetType() == GeomAbs_Plane; };
  // The edge at `v` between faces a and b, if any.
  auto edge_between = [&](const TopoDS_Shape& v, const TopoDS_Face& a, const TopoDS_Face& b) -> TopoDS_Edge {
    if (const ShapeList* l = v2e.Seek(v))
      for (ShapeList::Iterator it(*l); it.More(); it.Next())
      {
        const auto fs = faces_of(e2f, it.Value());
        bool       ha = false, hb = false;
        for (const TopoDS_Face& f : fs)
        {
          ha = ha || f.IsSame(a);
          hb = hb || f.IsSame(b);
        }
        if (ha && hb && fs.size() == 2)
          return TopoDS::Edge(it.Value());
      }
    return TopoDS_Edge();
  };
  ShapeList                tools;
  std::vector<TopoDS_Face> chamfer_faces, patches;
  // Per straight edge: the edge and the fillets' mitre edge at its corner
  // (tangent to it there, so tangent propagation selects it too; the patch
  // is the chamfer running on along it).
  std::vector<std::pair<TopoDS_Edge, TopoDS_Edge>> made;
  for (const TopoDS_Edge& e : es)
  {
    BRepAdaptor_Curve c(e);
    if (c.GetType() != GeomAbs_Line)
      continue;
    const auto ef = faces_of(e2f, e);
    if (ef.size() != 2 || !is_plane(ef[0]) || !is_plane(ef[1]))
      return std::nullopt;
    TopoDS_Vertex va, vb;
    TopExp::Vertices(e, va, vb);
    // Which end is the fillets' corner: faces F1, F2 and two cylinders.
    struct Corner
    {
      TopoDS_Vertex v;
      gp_Cylinder   cyl[2];
      gp_Pnt        P;
      TopoDS_Edge   mitre;
    };
    std::optional<Corner> corner;
    TopoDS_Vertex         base;
    for (const TopoDS_Vertex& v : {va, vb})
    {
      const auto vf = faces_of(v2f, v);
      std::vector<TopoDS_Face> others;
      for (const TopoDS_Face& f : vf)
        if (!f.IsSame(ef[0]) && !f.IsSame(ef[1]))
          others.push_back(f);
      if (others.size() == 1 && is_plane(others[0]))
      {
        if (!base.IsNull())
          return std::nullopt;
        base = v;
        continue;
      }
      if (others.size() != 2 || corner)
        return std::nullopt;
      Corner k;
      k.v = v;
      for (int i = 0; i < 2; ++i)
      {
        // The cylinder next to F_i along an edge through v.
        int found = -1;
        for (int j = 0; j < 2; ++j)
        {
          BRepAdaptor_Surface a(others[(size_t)j], false);
          if (a.GetType() == GeomAbs_Cylinder && !edge_between(v, ef[(size_t)i], others[(size_t)j]).IsNull())
          {
            found      = j;
            k.cyl[i]   = a.Cylinder();
          }
        }
        if (found < 0)
          return std::nullopt;
      }
      const TopoDS_Edge m = edge_between(v, others[0], others[1]);
      if (m.IsNull())
        return std::nullopt;
      TopoDS_Vertex m0, m1;
      TopExp::Vertices(m, m0, m1);
      k.P     = BRep_Tool::Pnt(m0.IsSame(v) ? m1 : m0);
      k.mitre = m;
      corner  = k;
    }
    if (!corner || base.IsNull())
      return std::nullopt;
    const gp_Pnt V0 = BRep_Tool::Pnt(base), V1 = BRep_Tool::Pnt(corner->v);
    const double L  = V0.Distance(V1);
    if (L < 1e-9)
      return std::nullopt;
    const gp_Dir t(gp_Vec(V0, V1));
    // In-face directions away from the edge.
    gp_Vec w[2];
    for (int i = 0; i < 2; ++i)
    {
      w[i] = gp_Vec(t).Crossed(gp_Vec(face_normal(ef[(size_t)i])));
      const gp_Pnt mid = V0.Translated(gp_Vec(t) * (L / 2) + w[i] * (d * 1e-3));
      BRepClass_FaceClassifier fc(ef[(size_t)i], mid, Precision::Confusion());
      if (fc.State() != TopAbs_IN)
        w[i].Reverse();
    }
    const gp_Pnt A[2] = {V1.Translated(w[0] * d), V1.Translated(w[1] * d)};
    // The chamfer's top corners on the fillets' tangent lines.
    for (int i = 0; i < 2; ++i)
      if (std::abs(gp_Lin(corner->cyl[i].Axis()).Distance(A[i]) - corner->cyl[i].Radius()) > 1e-6 * corner->cyl[i].Radius())
        return std::nullopt;
    std::optional<Arc3> sec[2];
    for (int i = 0; i < 2; ++i)
      if (!(sec[i] = fillet_section(corner->cyl[i], A[i], corner->P, t)))
        return std::nullopt;
    // Wider than a fillet: Onshape's "Distance of chamfer on face too large."
    if (d >= std::min(corner->cyl[0].Radius(), corner->cyl[1].Radius()) * (1 - 1e-9))
      return std::nullopt;
    const double R  = std::max(corner->cyl[0].Radius(), corner->cyl[1].Radius());
    const double mg = 2 * (d + R);
    auto         h  = [&](const gp_Pnt& x) { return gp_Vec(V0, x).Dot(gp_Vec(t)); };
    auto         at = [&](const gp_Pnt& x, double z) { return x.Translated(gp_Vec(t) * (z - h(x))); };
    const double lo = -mg, hi = h(corner->P) + mg;
    gp_Vec       along(A[0], A[1]);
    along.Normalize();
    const gp_Pnt Q1 = A[0].Translated(along * -mg), Q2 = A[1].Translated(along * mg);
    const gp_Pnt K  = V0.Translated((w[0] + w[1]).Normalized() * -mg);
    const gp_Pnt P  = corner->P;
    std::vector<TopoDS_Shape> faces;
    const TopoDS_Face cf = polygon_face({at(Q1, lo), at(Q2, lo), at(Q2, hi), at(A[1], hi), A[1], A[0], at(A[0], hi), at(Q1, hi)});
    faces.push_back(cf);
    faces.push_back(polygon_face({at(Q1, lo), at(K, lo), at(K, hi), at(Q1, hi)}));
    faces.push_back(polygon_face({at(K, lo), at(Q2, lo), at(Q2, hi), at(K, hi)}));
    faces.push_back(polygon_face({at(Q1, lo), at(Q2, lo), at(K, lo)}));
    faces.push_back(polygon_face({at(Q1, hi), at(K, hi), at(Q2, hi), at(A[1], hi), at(P, hi), at(A[0], hi)}));
    occ::handle<Geom_BSplineCurve> cv[2];
    for (int i = 0; i < 2; ++i)
    {
      cv[i] = arc_curve(*sec[i]);
      BRepBuilderAPI_MakeWire mw;
      mw.Add(BRepBuilderAPI_MakeEdge(cv[i]).Edge());
      mw.Add(BRepBuilderAPI_MakeEdge(P, at(P, hi)).Edge());
      mw.Add(BRepBuilderAPI_MakeEdge(at(P, hi), at(A[i], hi)).Edge());
      mw.Add(BRepBuilderAPI_MakeEdge(at(A[i], hi), A[i]).Edge());
      if (!mw.IsDone())
        return std::nullopt;
      BRepBuilderAPI_MakeFace mf(mw.Wire(), true);
      if (!mf.IsDone())
        return std::nullopt;
      faces.push_back(mf.Face());
    }
    // The patch: rows the two sections (u), quadratic along them (v).
    NCollection_Array2<gp_Pnt> sp(1, 2, 1, 3);
    NCollection_Array2<double> sw(1, 2, 1, 3);
    for (int i = 0; i < 2; ++i)
      for (int j = 0; j < 3; ++j)
      {
        sp(i + 1, j + 1) = sec[i]->p[j];
        sw(i + 1, j + 1) = sec[i]->w[j];
      }
    NCollection_Array1<double> uk(1, 2), vk(1, 2);
    NCollection_Array1<int>    um(1, 2), vm(1, 2);
    uk(1) = vk(1) = 0;
    uk(2) = vk(2) = 1;
    um(1) = um(2) = 2;
    vm(1) = vm(2) = 3;
    occ::handle<Geom_BSplineSurface> S  = new Geom_BSplineSurface(sp, sw, uk, vk, um, vm, 1, 2);
    const TopoDS_Face                pf = BRepBuilderAPI_MakeFace(S, 0, 1, 0, 1, Precision::Confusion()).Face();
    faces.push_back(pf);
    BRepBuilderAPI_Sewing sew(10 * Precision::Confusion());
    for (const TopoDS_Shape& f : faces)
      sew.Add(f);
    sew.Perform();
    TopoDS_Shell shell;
    int          shells = 0;
    for (TopExp_Explorer x(sew.SewedShape(), TopAbs_SHELL); x.More(); x.Next(), ++shells)
      shell = TopoDS::Shell(x.Current());
    if (shells != 1 || !BRep_Tool::IsClosed(shell))
      return std::nullopt;
    ShapeFix_Solid fx(BRepBuilderAPI_MakeSolid(shell).Solid());
    fx.Perform();
    const TopoDS_Shape T = fx.Solid();
    if (!valid_shape(T))
      return std::nullopt;
    tools.Append(T);
    // The tool's chamfer and patch faces (sewing may replace them).
    chamfer_faces.push_back(TopoDS::Face(sew.Modified(cf)));
    patches.push_back(TopoDS::Face(sew.Modified(pf)));
    made.emplace_back(e, corner->mitre);
  }
  // Every selected edge is one of those straight edges or their mitres.
  if (made.empty())
    return std::nullopt;
  for (const TopoDS_Edge& e : es)
  {
    bool ours = false;
    for (const auto& [line, mitre] : made)
      ours = ours || e.IsSame(line) || e.IsSame(mitre);
    if (!ours)
      return std::nullopt;
  }
  BRepAlgoAPI_Cut cut;
  ShapeList       args;
  args.Append(input);
  cut.SetArguments(args);
  cut.SetTools(tools);
  cut.SetToFillHistory(true);
  cut.Build();
  if (!cut.IsDone() || cut.HasErrors())
    return std::nullopt;
  const TopoDS_Shape out = cut.Shape();
  int                solids = 0;
  for (TopExp_Explorer x(out, TopAbs_SOLID); x.More(); x.Next())
    ++solids;
  if (solids != 1 || !valid_shape(out))
    return std::nullopt;
  std::vector<TopoDS_Shape> all{input};
  for (ShapeList::Iterator it(tools); it.More(); it.Next())
    all.push_back(it.Value());
  HHist hc = history_of(all, cut);
  // The chamfer and patch faces are generated by their edge.
  HHist g = new BRepTools_History();
  for (size_t i = 0; i < made.size(); ++i)
  {
    const auto& [line, mitre] = made[i];
    bool mitre_selected = false;
    for (const TopoDS_Edge& e : es)
      mitre_selected = mitre_selected || e.IsSame(mitre);
    g->AddGenerated(line, chamfer_faces[i]);
    g->AddGenerated(mitre_selected ? mitre : line, patches[i]);
  }
  return std::make_pair(out, compose(input, g, hc));
}

} // namespace

// A chamfer wider than a face it runs over, which OCCT can't build (qa4 r5
// CHAMFER-OVERFLOW: 25 mm on a top edge of a 60 x 40 x 20 mm block; live OK,
// 30000 mm3). Onshape cuts the part by the chamfer's plane: the triangle
// between the edge and its offset lines on the two planes, along the edge,
// clipped by whatever the plane runs past. Only straight convex edges
// between two planes, each end closed by a plane square to the edge, no two
// edges sharing a vertex (as fillet_as_cut). Null when it doesn't apply.
static std::optional<std::pair<TopoDS_Shape, HHist>> chamfer_as_cut(const TopoDS_Shape& input, const std::vector<TopoDS_Edge>& es,
                                                                    const std::vector<TopoDS_Face>& fs, uint8_t mode, double d1, double d2)
{
  ShapeAncMap e2f, v2f;
  TopExp::MapShapesAndAncestors(input, TopAbs_EDGE, TopAbs_FACE, e2f);
  TopExp::MapShapesAndAncestors(input, TopAbs_VERTEX, TopAbs_FACE, v2f);
  auto outward = [](const TopoDS_Face& f, gp_Vec& n) {
    BRepAdaptor_Surface a(f, false);
    if (a.GetType() != GeomAbs_Plane)
      return false;
    gp_Dir d = a.Plane().Axis().Direction();
    if (!a.Plane().Position().Direct())
      d.Reverse();
    if (f.Orientation() == TopAbs_REVERSED)
      d.Reverse();
    n = gp_Vec(d);
    return true;
  };
  ShapeIMap used;
  ShapeList tools;
  // Per edge: two points on its chamfer plane (the offset lines).
  std::vector<std::pair<gp_Pnt, gp_Pnt>> planes;
  for (size_t i = 0; i < es.size(); ++i)
  {
    const TopoDS_Edge& E = es[i];
    BRepAdaptor_Curve  c(E);
    const ShapeList*   fl = e2f.Seek(E);
    if (c.GetType() != GeomAbs_Line || fl == nullptr || fl->Extent() != 2)
      return std::nullopt;
    TopoDS_Face f1 = TopoDS::Face(fl->First()), f2 = TopoDS::Face(fl->Last());
    // Face 1 carries the first distance (the reference face, two distances
    // or distance and angle).
    if (mode != 0 && !fs[i].IsNull() && fs[i].IsSame(f2))
      std::swap(f1, f2);
    gp_Vec n1, n2;
    if (f1.IsSame(f2) || !outward(f1, n1) || !outward(f2, n2))
      return std::nullopt;
    TopoDS_Edge in1;
    for (TopExp_Explorer x(f1, TopAbs_EDGE); x.More(); x.Next())
      if (x.Current().IsSame(E))
        in1 = TopoDS::Edge(x.Current());
    if (in1.IsNull())
      return std::nullopt;
    TopoDS_Vertex va, vb;
    TopExp::Vertices(in1, va, vb, true);
    if (va.IsNull() || vb.IsNull() || va.IsSame(vb) || used.Contains(va) || used.Contains(vb))
      return std::nullopt;
    used.Add(va);
    used.Add(vb);
    const gp_Pnt a = BRep_Tool::Pnt(va), b = BRep_Tool::Pnt(vb);
    const gp_Vec t(a, b);
    const double L = t.Magnitude();
    // Convex: (n1 x n2) . t > 0, t the edge's direction as face 1 runs it.
    if (L < Precision::Confusion() || n1.Crossed(n2).Dot(t) <= 1e-9 * L)
      return std::nullopt;
    const gp_Vec u = t / L;
    for (const TopoDS_Vertex& v : {va, vb})
    {
      const ShapeList* at = v2f.Seek(v);
      ShapeIMap        faces;
      if (at != nullptr)
        for (ShapeList::Iterator it(*at); it.More(); it.Next())
          faces.Add(it.Value());
      if (faces.Extent() != 3)
        return std::nullopt;
      bool capped = false;
      for (int k = 1; k <= 3; ++k)
      {
        gp_Vec n;
        capped = capped || (!faces(k).IsSame(f1) && !faces(k).IsSame(f2) && outward(TopoDS::Face(faces(k)), n) && std::abs(std::abs(n.Dot(u)) - 1) < 1e-9);
      }
      if (!capped)
        return std::nullopt;
    }
    // Across the (convex) edge, each face's direction away from it.
    const gp_Vec w1 = (-n2 + n1 * n2.Dot(n1)).Normalized(), w2 = (-n1 + n2 * n1.Dot(n2)).Normalized();
    double s1 = d1, s2 = mode == 1 ? d2 : d1;
    if (mode == 2)
    {
      // Distance d1 on face 1, the chamfer at angle d2 to it: the triangle's
      // third side by the law of sines (d1 tan d2 on a square edge).
      const double theta = std::acos(std::max(-1.0, std::min(1.0, w1.Dot(w2))));
      const double far   = M_PI - theta - d2;
      if (far <= 1e-9)
        return std::nullopt;
      s2 = d1 * std::sin(d2) / std::sin(far);
    }
    const gp_Pnt p1 = a.Translated(w1 * s1), p2 = a.Translated(w2 * s2);
    BRepBuilderAPI_MakePolygon sec(a, p1, p2, true);
    if (!sec.IsDone())
      return std::nullopt;
    tools.Append(BRepPrimAPI_MakePrism(BRepBuilderAPI_MakeFace(sec.Wire(), true).Face(), t).Shape());
    planes.emplace_back(p1, p2);
  }
  BRepAlgoAPI_Cut cut;
  ShapeList       args;
  args.Append(input);
  cut.SetArguments(args);
  cut.SetTools(tools);
  cut.Build();
  if (!cut.IsDone() || cut.HasErrors() || !valid_shape(cut.Shape()))
    return std::nullopt;
  HHist h = history_of({input}, cut);
  // The faces the cut made: from the chamfer plane they lie on, generated by
  // its edge (the chamfer's face).
  ShapeIMap old;
  for (TopExp_Explorer f(input, TopAbs_FACE); f.More(); f.Next())
  {
    const ShapeList& m = h->Modified(f.Current());
    for (ShapeList::Iterator it(m); it.More(); it.Next())
      old.Add(it.Value());
    if (m.IsEmpty() && !h->IsRemoved(f.Current()))
      old.Add(f.Current());
  }
  for (TopExp_Explorer f(cut.Shape(), TopAbs_FACE); f.More(); f.Next())
  {
    if (old.Contains(f.Current()))
      continue;
    BRepAdaptor_Surface a(TopoDS::Face(f.Current()), false);
    if (a.GetType() != GeomAbs_Plane)
      continue;
    for (size_t i = 0; i < planes.size(); ++i)
    {
      const double tol = 1e-9 * (1 + planes[i].first.Distance(planes[i].second));
      if (a.Plane().Distance(planes[i].first) < tol && a.Plane().Distance(planes[i].second) < tol)
        h->AddGenerated(es[i], f.Current());
    }
  }
  return std::make_pair(cut.Shape(), h);
}

static OpOut op_chamfer_unbudgeted(const OcShape& s, const OcShapeList& edges, const OcShapeList& faces,
                 uint8_t mode, double d1, double d2, bool checked)
{
  require_non_null(s.shape, "shape");
  if (edges.items.empty())
    fail_invalid("chamfer needs at least one edge");
  if (!(d1 > 1e-9))
    fail_invalid("chamfer distance must be positive");
  if (mode == 1 && !(d2 > 1e-9))
    fail_invalid("chamfer second distance must be positive");
  if (mode == 2 && !(d2 > 1e-9 && d2 < M_PI / 2.0))
    fail_invalid("chamfer angle must be in (0, 90) degrees");
  if (mode > 2)
    fail_invalid("unknown chamfer mode");
  ShapeAncMap anc;
  TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, anc);
  std::vector<TopoDS_Edge> es;
  std::vector<TopoDS_Face> fs;
  for (size_t i = 0; i < edges.items.size(); ++i)
  {
    es.push_back(first_adjacent_face_edge_check(s.shape, edges.items[i], "chamfer edge"));
    TopoDS_Face f;
    if (mode != 0)
    {
      if (i < faces.items.size() && !faces.items[i].IsNull())
        f = as_face(faces.items[i], "chamfer reference face");
      else
      {
        const ShapeList* l = anc.Seek(es.back());
        if (l == nullptr || l->IsEmpty())
          fail_invalid("chamfer edge has no adjacent face");
        f = TopoDS::Face(l->First());
      }
    }
    fs.push_back(f);
  }
  const double size = mode == 1 ? std::max(d1, d2) : d1;
  auto         make = [&](const Scaled& sc) -> BlendBuild {
    return [&sc, &es, &fs, mode, d1, d2](double scale, TopoDS_Shape& out, HHist& h) {
      const double              k = sc.factor;
      BRepFilletAPI_MakeChamfer mk(sc.shape);
      for (size_t i = 0; i < es.size(); ++i)
      {
        const TopoDS_Edge e = TopoDS::Edge(sc.image(es[i]));
        const TopoDS_Face f = fs[i].IsNull() ? fs[i] : TopoDS::Face(sc.image(fs[i]));
        if (mode == 0)
          mk.Add(d1 * scale * k, e);
        else if (mode == 1)
          mk.Add(d1 * scale * k, d2 * scale * k, e, f);
        else
          mk.AddDA(d1 * scale * k, d2, e, f);
      }
      mk.Build();
      if (!mk.IsDone())
        return false;
      out = mk.Shape();
      h   = history_of({sc.shape}, mk);
      return true;
    };
  };
  // A chamfer into the corner where two fillets meet: Parasolid's blend
  // patch (OCCT's retries "succeed" there with a different solid).
  if (checked && mode == 0)
    if (auto corner = chamfer_into_fillet_corners(s.shape, es, d1))
      return make_result({s.shape}, corner->first, corner->second.get());
  if (checked)
  {
    const SlabBlend blend = [&](const TopoDS_Shape& slab, const std::vector<int>& idx,
                                const std::function<TopoDS_Shape(const TopoDS_Shape&)>& image, TopoDS_Shape& r, HHist& rh) {
      BRepFilletAPI_MakeChamfer mk(slab);
      for (int i : idx)
      {
        const TopoDS_Edge e = TopoDS::Edge(image(es[(size_t)i]));
        if (mode == 0)
          mk.Add(d1, e);
        else
        {
          const TopoDS_Shape f = image(fs[(size_t)i]);
          if (f.IsNull())
            return false;
          if (mode == 1)
            mk.Add(d1, d2, e, TopoDS::Face(f));
          else
            mk.AddDA(d1, d2, e, TopoDS::Face(f));
        }
      }
      mk.Build();
      if (!mk.IsDone())
        return false;
      r  = mk.Shape();
      rh = history_of({slab}, mk);
      return valid_shape(r);
    };
    TopoDS_Shape out;
    HHist        h;
    if (blend_in_slabs(s.shape, es, 2 * size, blend, out, h))
      return make_result({s.shape}, out, h.get());
  }
  try
  {
    return scaled_blend(s.shape, checked, size, "chamfer", make);
  }
  catch (const std::runtime_error&)
  {
    // Over the faces of a plain edge: the part cut by the chamfer's plane.
    if (checked)
      if (auto past = chamfer_as_cut(s.shape, es, fs, mode, d1, d2))
        return make_result({s.shape}, past->first, past->second.get());
    // Onshape reports a chamfer wider than a face it runs over (or than the
    // radius of an arc it follows) separately.
    // Also a chamfer wider than a fillet its edge ends on (live G46: 5 mm on
    // the edge between two R3 fillets).
    ShapeAncMap v2f;
    TopExp::MapShapesAndAncestors(s.shape, TopAbs_VERTEX, TopAbs_FACE, v2f);
    for (const TopoDS_Edge& e : es)
    {
      BRepAdaptor_Curve c(e);
      if (c.GetType() == GeomAbs_Circle && c.Circle().Radius() < size * (1 - 1e-9))
        fail_op("chamfer distance too large");
      for (TopExp_Explorer v(e, TopAbs_VERTEX); v.More(); v.Next())
        if (const ShapeList* at = v2f.Seek(v.Current()))
          for (ShapeList::Iterator it(*at); it.More(); it.Next())
          {
            BRepAdaptor_Surface a(TopoDS::Face(it.Value()), false);
            if (a.GetType() == GeomAbs_Cylinder && a.Cylinder().Radius() < size * (1 - 1e-9))
              fail_op("chamfer distance too large");
          }
      const ShapeList* l = anc.Seek(e);
      if (l == nullptr)
        continue;
      gp_Pnt m = c.Value((c.FirstParameter() + c.LastParameter()) / 2);
      for (ShapeList::Iterator it(*l); it.More(); it.Next())
      {
        double width = 0;
        for (TopExp_Explorer v(it.Value(), TopAbs_VERTEX); v.More(); v.Next())
          width = std::max(width, BRep_Tool::Pnt(TopoDS::Vertex(v.Current())).Distance(m));
        if (width < size * (1 - 1e-9))
          fail_op("chamfer distance too large");
      }
    }
    throw;
  }
}

OpOut op_chamfer(const OcShape& s, const OcShapeList& edges, const OcShapeList& faces,
                 uint8_t mode, double d1, double d2, bool checked)
{
  OpOut out;
  const int r = run_budgeted(blend_budget_seconds(), [&] { out = op_chamfer_unbudgeted(s, edges, faces, mode, d1, d2, checked); });
  if (r == 1)
    fail_op("chamfer took too long");
  if (r == 2)
    fail_op("cancelled");
  return out;
}

// ---------------------------------------------------------------- draft

// The one solid of `s` (null when there are none or several).
static TopoDS_Shape only_solid(const TopoDS_Shape& s)
{
  TopoDS_Shape one;
  int          n = 0;
  for (TopExp_Explorer x(s, TopAbs_SOLID); x.More(); x.Next(), ++n)
    one = x.Current();
  return n == 1 ? one : TopoDS_Shape();
}

// A box filling the half space behind the plane through `o` with normal `n`.
static TopoDS_Shape half_space_box(const gp_Pnt& o, const gp_Dir& n, double size)
{
  gp_Ax3 ax(o, n);
  TopoDS_Face f = BRepBuilderAPI_MakeFace(gp_Pln(ax), -size, size, -size, size).Face();
  return BRepPrimAPI_MakePrism(f, gp_Vec(n) * -2 * size).Shape();
}

// One sloped plane drafted (live N3: the face of a 45° chamfer, 5°): Parasolid
// turns it about its line on the neutral plane; where it no longer bounds the
// part its neighbours run on and meet (N3: the turned plane lies outside, the
// chamfer vanishes, 48000 mm³). A draft that would turn the face over against
// the pull fails (N3 the other way). The part without the face, its
// neighbours extended, behind the turned plane. Null when it doesn't apply.
static std::optional<std::pair<TopoDS_Shape, HHist>> draft_sloped_plane(const TopoDS_Shape& input, const TopoDS_Face& f,
                                                                       const gp_Dir& P, const gp_Pln& neutral, double angle)
{
  BRepAdaptor_Surface a(f, false);
  if (a.GetType() != GeomAbs_Plane)
    return std::nullopt;
  gp_Dir n = a.Plane().Axis().Direction();
  if (!a.Plane().Position().Direct())
    n.Reverse();
  if (f.Orientation() == TopAbs_REVERSED)
    n.Reverse();
  const double np = n.Dot(P);
  if (std::abs(np) < 1e-7 || std::abs(np) > 1 - 1e-7)
    return std::nullopt;
  const gp_Vec nh = gp_Vec(n) - gp_Vec(P) * np;
  const gp_Dir n2(gp_Vec(gp_Dir(nh)) * std::cos(angle) + gp_Vec(P) * std::sin(angle));
  if (np * n2.Dot(P) < 0)
    fail_op("draft would turn the face over");
  // A point of the face's plane on the neutral plane: up the plane's slope.
  GProp_GProps g;
  BRepGProp::SurfaceProperties(f, g);
  const gp_Pnt q = g.CentreOfMass();
  const gp_Vec u = gp_Vec(P) - gp_Vec(n) * np;
  const double h = gp_Vec(neutral.Location(), q).Dot(gp_Vec(neutral.Axis().Direction())) / neutral.Axis().Direction().Dot(P);
  const gp_Pnt Q0 = q.Translated(u * (-h / u.Dot(gp_Vec(P))));
  Bnd_Box box;
  BRepBndLib::Add(input, box);
  const double size = std::sqrt(box.SquareExtent()) * 4 + 1 + gp_Vec(Q0, q).Magnitude();
  BRepAlgoAPI_Defeaturing df;
  df.SetShape(input);
  ShapeList fl;
  fl.Append(f);
  df.AddFacesToRemove(fl);
  df.SetToFillHistory(true);
  df.Build();
  if (!df.IsDone() || df.HasErrors())
    return std::nullopt;
  const TopoDS_Shape healed = df.Shape();
  HHist              h1     = history_of({input}, df);
  BRepAlgoAPI_Common cm(healed, half_space_box(Q0, n2, size));
  cm.SetToFillHistory(true);
  cm.Build();
  if (!cm.IsDone() || cm.HasErrors())
    return std::nullopt;
  const TopoDS_Shape out = only_solid(cm.Shape());
  if (out.IsNull() || !valid_shape(out))
    return std::nullopt;
  HHist h2 = history_of({healed}, cm);
  return std::make_pair(out, compose(input, h1, h2));
}

// A drafted cylinder that closes inside the part (live S3: a Ø10 hole through
// 20 mm narrowing upward 30°): OCCT's cone runs on through its apex into a
// second nappe; Parasolid's face ends at the apex (a conical pocket, solid
// above it). Built again from the input: the hole filled and the cone up to
// its apex cut (a boss: the cylinder cut and the cone added); fusing the far
// nappe back onto OCCT's result leaves two solids on the shared cone. One
// drafted face only; the cone face is the drafted face's image.
static TopoDS_Shape end_cone_at_apex(const TopoDS_Shape& input, const TopoDS_Face& drafted, const TopoDS_Shape& r0,
                                     const gp_Pln& neutral, HHist& h)
{
  Bnd_Box box;
  BRepBndLib::Add(r0, box);
  const double tol = 1e-7 * std::sqrt(box.SquareExtent());
  BRepAdaptor_Surface cyl(drafted, false);
  if (cyl.GetType() != GeomAbs_Cylinder)
    return r0;
  const ShapeList& images = h->Modified(drafted);
  if (images.Extent() != 1 || images.First().ShapeType() != TopAbs_FACE)
    return r0;
  const TopoDS_Face   f = TopoDS::Face(images.First());
  BRepAdaptor_Surface s(f);
  if (s.GetType() != GeomAbs_Cone)
    return r0;
  const gp_Cone cone = s.Cone();
  const gp_Pnt  apex = cone.Apex();
  const gp_Dir  ax   = cone.Axis().Direction();
  double        lo = 1e300, hi = -1e300;
  for (TopExp_Explorer v(f, TopAbs_VERTEX); v.More(); v.Next())
  {
    const double t = gp_Vec(apex, BRep_Tool::Pnt(TopoDS::Vertex(v.Current()))).Dot(gp_Vec(ax));
    lo = std::min(lo, t), hi = std::max(hi, t);
  }
  if (!(lo < -tol && hi > tol))
    return r0;
  const double an = ax.Dot(neutral.Axis().Direction());
  if (std::abs(an) < 1e-9)
    return r0;
  const double tn   = gp_Vec(apex, neutral.Location()).Dot(gp_Vec(neutral.Axis().Direction())) / an;
  const double near = tn > 0 ? hi : lo;
  const double rn   = std::abs(near) * std::tan(std::abs(cone.SemiAngle()));
  // Material side of the original face: a hole's face looks at its axis.
  double u0, u1, v0, v1;
  BRepTools::UVBounds(drafted, u0, u1, v0, v1);
  gp_Pnt p;
  gp_Vec du, dv;
  cyl.D1(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, du, dv);
  gp_Vec nf = du.Crossed(dv);
  if (drafted.Orientation() == TopAbs_REVERSED)
    nf.Reverse();
  const gp_Ax1 cax  = cyl.Cylinder().Axis();
  const gp_Pnt foot = cax.Location().Translated(gp_Vec(cax.Direction()) * gp_Vec(cax.Location(), p).Dot(gp_Vec(cax.Direction())));
  const bool   hole = nf.Dot(gp_Vec(foot, p)) < 0;
  // The original cylinder's solid over the face's extent along its axis.
  double c0 = 1e300, c1 = -1e300;
  for (TopExp_Explorer v(drafted, TopAbs_VERTEX); v.More(); v.Next())
  {
    const double t = gp_Vec(cax.Location(), BRep_Tool::Pnt(TopoDS::Vertex(v.Current()))).Dot(gp_Vec(cax.Direction()));
    c0 = std::min(c0, t), c1 = std::max(c1, t);
  }
  if (!(c1 - c0 > tol))
    return r0;
  const TopoDS_Shape body = BRepPrimAPI_MakeCylinder(gp_Ax2(cax.Location().Translated(gp_Vec(cax.Direction()) * c0), cax.Direction()),
                                                     cyl.Cylinder().Radius(), c1 - c0).Shape();
  const gp_Pnt       base = apex.Translated(gp_Vec(ax) * near);
  const TopoDS_Shape tip  = BRepPrimAPI_MakeCone(gp_Ax2(base, near > 0 ? ax.Reversed() : ax), rn, 0, std::abs(near)).Shape();
  try
  {
    std::unique_ptr<BRepAlgoAPI_BooleanOperation> first, second;
    if (hole)
      first = std::make_unique<BRepAlgoAPI_Fuse>(input, body);
    else
      first = std::make_unique<BRepAlgoAPI_Cut>(input, body);
    if (!first->IsDone() || first->HasErrors())
      return r0;
    const TopoDS_Shape mid = first->Shape();
    if (hole)
      second = std::make_unique<BRepAlgoAPI_Cut>(mid, tip);
    else
      second = std::make_unique<BRepAlgoAPI_Fuse>(mid, tip);
    if (!second->IsDone() || second->HasErrors())
      return r0;
    ShapeUpgrade_UnifySameDomain usd(second->Shape(), true, true, false);
    usd.Build();
    const TopoDS_Shape out = only_solid(usd.Shape());
    if (out.IsNull() || !valid_shape(out))
      return r0;
    HHist all = compose(input, compose(input, history_of({input}, *first), history_of({mid}, *second)), usd.History());
    // The new cone face stands for the drafted face.
    HHist r = new BRepTools_History();
    for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    {
      ShapeIMap in;
      TopExp::MapShapes(input, k, in);
      for (int i = 1; i <= in.Extent(); ++i)
      {
        const TopoDS_Shape& x = in(i);
        if (x.IsSame(drafted))
          continue;
        add_images(all, x, x, r);
        if (all->IsRemoved(x) && r->Modified(x).IsEmpty() && r->Generated(x).IsEmpty())
          r->Remove(x);
      }
    }
    for (TopExp_Explorer x(out, TopAbs_FACE); x.More(); x.Next())
      if (BRepAdaptor_Surface(TopoDS::Face(x.Current()), false).GetType() == GeomAbs_Cone)
        r->AddModified(drafted, x.Current());
    h = r;
    return out;
  }
  catch (const Standard_Failure&)
  {
    return r0;
  }
}

static OpOut op_draft_unbudgeted(const OcShape& s, const OcShapeList& faces, P3 dir, const PlaneF& neutral,
               double angle)
{
  require_non_null(s.shape, "shape");
  if (std::getenv("NOSHAPE_DUMP_OPS"))
  {
    std::ostringstream p;
    p.precision(17);
    p << "draft pull " << dir.x << " " << dir.y << " " << dir.z << " origin " << neutral.origin.x << " " << neutral.origin.y << " "
      << neutral.origin.z << " xdir " << neutral.xdir.x << " " << neutral.xdir.y << " " << neutral.xdir.z << " normal "
      << neutral.normal.x << " " << neutral.normal.y << " " << neutral.normal.z << " angle " << angle;
    dump_op("draft", s.shape, faces.items, p.str());
  }
  if (faces.items.empty())
    fail_invalid("draft needs at least one face");
  if (!(std::abs(angle) < M_PI / 2.0))
    fail_invalid("draft angle must be less than 90 degrees");
  check_faces_in(s.shape, faces.items);
  gp_Dir                   pull = to_dir(dir, "pull direction");
  gp_Pln                   pln  = to_pln(neutral);
  std::vector<TopoDS_Shape> args{s.shape};
  auto sloped = [&]() -> OpOut {
    if (faces.items.size() == 1)
      if (auto r = draft_sloped_plane(s.shape, TopoDS::Face(faces.items[0]), pull, pln, angle))
        return make_result(args, r->first, r->second.get());
    fail_op("draft failed");
  };
  BRepOffsetAPI_DraftAngle mk(s.shape);
  for (const auto& f : faces.items)
  {
    mk.Add(TopoDS::Face(f), pull, angle, pln, true);
    if (!mk.AddDone())
      return sloped();
  }
  mk.Build();
  if (!mk.IsDone())
    return sloped();
  HHist        h = modifier_history(s.shape, mk);
  TopoDS_Shape r = mk.Shape();
  if (faces.items.size() == 1)
    r = end_cone_at_apex(s.shape, TopoDS::Face(faces.items[0]), r, pln, h);
  return make_result(args, r, h.get());
}

OpOut op_draft(const OcShape& s, const OcShapeList& faces, P3 dir, const PlaneF& neutral,
               double angle)
{
  OpOut out;
  const int r = run_budgeted(blend_budget_seconds(), [&] { out = op_draft_unbudgeted(s, faces, dir, neutral, angle); });
  if (r == 1)
    fail_op("draft took too long");
  if (r == 2)
    fail_op("cancelled");
  return out;
}

// ---------------------------------------------------------------- defeature / offsets

static OpOut op_defeature_unbudgeted(const OcShape& s, const OcShapeList& faces)
{
  require_non_null(s.shape, "shape");
  if (faces.items.empty())
    fail_invalid("delete face needs at least one face");
  check_faces_in(s.shape, faces.items);
  BRepAlgoAPI_Defeaturing mk;
  mk.SetShape(s.shape);
  mk.AddFacesToRemove(to_list(faces.items));
  // On this thread, so its CPU budget (run_budgeted) counts the work: in
  // parallel the caller only waits and its budget never ran out (qa
  // DRAFT-HANG: 623 s in one call).
  mk.SetRunParallel(false);
  mk.SetToFillHistory(true);
  mk.Build();
  if (!mk.IsDone() || mk.HasErrors())
    fail_op("delete face failed: " + algo_errors(mk));
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     h = history_of(args, mk);
  return make_result(args, mk.Shape(), h.get());
}

OpOut op_defeature(const OcShape& s, const OcShapeList& faces)
{
  OpOut out;
  // A face deletion takes milliseconds; a third of a blend's budget.
  const int r = run_budgeted(blend_budget_seconds() / 3, [&] { out = op_defeature_unbudgeted(s, faces); });
  if (r == 1)
    fail_op("delete faces took too long");
  if (r == 2)
    fail_op("cancelled");
  return out;
}

OpOut op_offset_faces(const OcShape& s, const OcShapeList& faces,
                      rust::Slice<const double> offsets)
{
  require_non_null(s.shape, "shape");
  if (faces.items.empty())
    fail_invalid("offset face needs at least one face");
  if (offsets.size() != 1 && offsets.size() != faces.items.size())
    fail_invalid("offsets must be one value or one per face");
  check_faces_in(s.shape, faces.items);
  BRepOffset_MakeOffset mo;
  mo.Initialize(s.shape, 0.0, 1e-6, BRepOffset_Skin, false, false, GeomAbs_Intersection);
  bool any = false;
  for (size_t i = 0; i < faces.items.size(); ++i)
  {
    double off = offsets.size() == 1 ? offsets[0] : offsets[i];
    if (std::abs(off) > 1e-12)
      any = true;
    mo.SetOffsetOnFace(TopoDS::Face(faces.items[i]), off);
  }
  std::vector<TopoDS_Shape> args{s.shape};
  if (!any)
    return make_result(args, s.shape, nullptr);
  mo.MakeOffsetShape();
  if (!mo.IsDone())
    fail_op("offset face failed (error " + std::to_string((int)mo.Error()) + ")");
  HHist h = history_of(args, mo);
  return make_result(args, mo.Shape(), h.get());
}

OpOut op_offset_shape(const OcShape& s, double offset, uint8_t join)
{
  require_non_null(s.shape, "shape");
  if (!(std::abs(offset) > 1e-12))
    fail_invalid("offset must be non-zero");
  std::vector<TopoDS_Shape>     args{s.shape};
  BRepOffsetAPI_MakeOffsetShape mk;
  bool                          ok = false;
  try
  {
    mk.PerformByJoin(s.shape, offset, 1e-6, BRepOffset_Skin, false, false,
                     join == 1 ? GeomAbs_Intersection : GeomAbs_Arc);
    ok = mk.IsDone();
  }
  catch (const Standard_Failure&)
  {
    ok = false;
  }
  if (ok)
  {
    HHist h = history_of(args, mk);
    return make_result(args, mk.Shape(), h.get());
  }
  BRepOffsetAPI_MakeOffsetShape simple;
  simple.PerformBySimple(s.shape, offset);
  if (!simple.IsDone())
    fail_op("offset surface failed");
  HHist h = history_of(args, simple);
  return make_result(args, simple.Shape(), h.get());
}

// ---------------------------------------------------------------- sew / fill / rib

OpOut op_sew(const OcShapeList& shapes, double tol, bool make_solid)
{
  if (shapes.items.empty())
    fail_invalid("sew needs at least one shape");
  BRepBuilderAPI_Sewing sew(tol > 0 ? tol : 1e-6);
  for (const auto& s : shapes.items)
  {
    require_non_null(s, "shape");
    sew.Add(s);
  }
  sew.Perform();
  TopoDS_Shape result = sew.SewedShape();
  if (result.IsNull())
    fail_op("sewing produced no shape");
  occ::handle<BRepTools_History> h = new BRepTools_History();
  for (const auto& a : shapes.items)
  {
    for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    {
      ShapeIMap m;
      TopExp::MapShapes(a, k, m);
      for (int i = 1; i <= m.Extent(); ++i)
      {
        if (sew.IsModifiedSubShape(m(i)))
        {
          TopoDS_Shape r = sew.ModifiedSubShape(m(i));
          if (!r.IsNull() && !r.IsSame(m(i)) && BRepTools_History::IsSupportedType(r))
            h->AddModified(m(i), r);
        }
      }
    }
  }
  if (make_solid)
  {
    std::vector<TopoDS_Shape> solids;
    for (TopExp_Explorer e(result, TopAbs_SHELL); e.More(); e.Next())
    {
      const TopoDS_Shell& sh = TopoDS::Shell(e.Current());
      if (!BRep_Tool::IsClosed(sh))
        fail_op("sewn shell is not closed; cannot make a solid");
      BRepBuilderAPI_MakeSolid ms(sh);
      if (!ms.IsDone())
        fail_op("could not make a solid from the sewn shell");
      TopoDS_Solid so = ms.Solid();
      BRepLib::OrientClosedSolid(so);
      solids.push_back(so);
    }
    if (solids.empty())
      fail_op("sewing produced no shell");
    result = solids.size() == 1 ? solids[0] : compound_of(solids);
  }
  return make_result(shapes.items, result, h.get());
}

OpOut op_fill(const OcShapeList& edges, const OcShapeList& supports,
              rust::Slice<const int32_t> orders, rust::Slice<const double> points,
              const OcShapeList& guides)
{
  if (edges.items.empty())
    fail_invalid("fill needs boundary edges");
  if (points.size() % 3 != 0)
    fail_invalid("fill points must be (x, y, z) triples");
  // Guide curves are met to modelling tolerance (live "psb (10-24 d)":
  // within 1e-4 mm): more points on them, more iterations, a finer surface.
  const bool guided = !guides.items.empty();
  Bnd_Box    box;
  for (const auto& e : edges.items)
    BRepBndLib::Add(e, box);
  const double              size = box.IsVoid() ? 1.0 : std::sqrt(box.SquareExtent());
  BRepOffsetAPI_MakeFilling mk(3, guided ? 40 : 15, guided ? 6 : 2, false, 1e-5, guided ? 2e-6 * size : 1e-4, 1e-2, 1e-1,
                               guided ? 10 : 8, guided ? 30 : 9);
  for (size_t i = 0; i < edges.items.size(); ++i)
  {
    const TopoDS_Edge& e   = as_edge(edges.items[i], "fill boundary edge");
    int                ord = i < orders.size() ? orders[i] : 0;
    GeomAbs_Shape      c   = ord >= 2 ? GeomAbs_G2 : (ord == 1 ? GeomAbs_G1 : GeomAbs_C0);
    if (i < supports.items.size() && !supports.items[i].IsNull())
      mk.Add(e, as_face(supports.items[i], "fill support face"), c, true);
    else
    {
      if (c != GeomAbs_C0)
        fail_invalid("tangent/curvature fill continuity requires a support face");
      mk.Add(e, c, true);
    }
  }
  for (size_t i = 0; i + 2 < points.size(); i += 3)
    mk.Add(gp_Pnt(points[i], points[i + 1], points[i + 2]));
  // Guide curves the surface passes through (free, not boundary).
  for (const auto& g : guides.items)
    mk.Add(as_edge(g, "fill guide edge"), GeomAbs_C0, false);
  mk.Build();
  if (!mk.IsDone())
    fail_op("fill surface failed (boundary not closed?)");
  std::vector<TopoDS_Shape> args;
  for (const auto& e : edges.items)
    args.push_back(e);
  HHist h = history_of(args, mk);
  return make_result(args, mk.Shape(), h.get());
}

OpOut op_rib(const OcShape& base, const OcShape& wire, const PlaneF& plane, P3 d1, P3 d2,
             bool fuse)
{
  require_non_null(base.shape, "rib base");
  TopoDS_Wire             w  = as_wire(wire.shape, "rib profile");
  occ::handle<Geom_Plane> gp = new Geom_Plane(to_pln(plane));
  BRepFeat_MakeLinearForm form;
  form.Init(base.shape, w, gp, to_vec(d1), to_vec(d2), fuse ? 1 : 0, true);
  form.Perform();
  if (!form.IsDone())
    fail_op("rib failed (profile must touch the base on both ends)");
  std::vector<TopoDS_Shape> args{base.shape, w};
  HHist                     h = history_of(args, form);
  return make_result(args, form.Shape(), h.get());
}

} // namespace noshape
