// Feature operations: prism, revolve, sweep, loft, thicken, shell, fillet,
// chamfer, draft, defeaturing, offsets, sewing, filling, rib.
#include "internal.hpp"

#include <BRepAdaptor_Surface.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
#include <BRepAlgoAPI_Cut.hxx>
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
#include <BRepLib.hxx>
#include <BRepLib_FindSurface.hxx>
#include <BRepOffsetAPI_DraftAngle.hxx>
#include <BRepOffsetAPI_MakeFilling.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakePipe.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <Geom2d_Line.hxx>
#include <GeomConvert_ApproxCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepTools.hxx>
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
  if (vs.Magnitude() > 1e-12)
  {
    gp_Trsf t;
    t.SetTranslation(vs);
    tr = std::make_unique<BRepBuilderAPI_Transform>(base, t, false);
    if (!tr->IsDone())
      fail_op("could not translate profile");
    base = tr->Shape();
    merge_into(hist, history_of(args, *tr));
  }
  BRepPrimAPI_MakePrism prism(base, d, false, true);
  if (!prism.IsDone())
    fail_op("extrude failed");
  TopoDS_Shape result = prism.Shape();
  merge_into(hist, history_of({base}, prism));

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
    return tr ? tr->ModifiedShape(s) : s;
  };
  hb.add_mapped(0, REL_FIRST, [&](const TopoDS_Shape& s) {
    return images_of(post, prism.FirstShape(moved(s)));
  });
  hb.add_mapped(0, REL_LAST, [&](const TopoDS_Shape& s) {
    return images_of(post, prism.LastShape(moved(s)));
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
  if (std::abs(start_offset) > 1e-12)
  {
    gp_Trsf t;
    t.SetTranslation(gp_Vec(dir) * start_offset);
    tr = std::make_unique<BRepBuilderAPI_Transform>(base, t, false);
    base = tr->Shape();
    merge_into(hist, history_of({profile.shape}, *tr));
  }
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
  sp.SetRunParallel(true);
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
  if (std::abs(a0) > 1e-12 && !full)
  {
    gp_Trsf t;
    t.SetRotation(ax, a0);
    tr   = std::make_unique<BRepBuilderAPI_Transform>(base, t, false);
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

OpOut op_pipe_shell(const OcShapeList& profiles, const OcShape& spine, const SweepOpts& o,
                    const OcShape& aux)
{
  if (profiles.items.empty())
    fail_invalid("sweep needs at least one profile");
  TopoDS_Wire sw = as_wire(spine.shape, "sweep path");

  // A single face profile with holes: use BRepOffsetAPI_MakePipe (supports faces).
  if (profiles.items.size() == 1 && profiles.items[0].ShapeType() == TopAbs_FACE &&
      count_wires(profiles.items[0]) > 1)
  {
    if (std::abs(o.scale_end - 1.0) > 1e-12)
      fail_unsupported("scaled sweep of a face with holes");
    GeomFill_Trihedron tri;
    switch (o.mode)
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
  switch (o.mode)
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

// Natural cubic spline through rows R[k] (each `dim` doubles) at parameters
// t[k], as piecewise Bezier poles: 3 (n - 1) + 1 of them.
static std::vector<std::vector<double>> natural_bezier(const std::vector<std::vector<double>>& R,
                                                       const std::vector<double>&              t)
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
    if (k == 0)
    {
      b[k] = 2, c[k] = 1;
      for (size_t i = 0; i < dim; ++i)
        d[k][i] = 3 * (R[1][i] - R[0][i]) / h[0];
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
// sections with a natural cubic spline (Onshape G10: Ø20 → square 12 → Ø20,
// 6125.607 mm³), OCCT with one polynomial (a parabola through three,
// 5917.6). Each lateral face of the loft spans every section; its surface is
// rebuilt in place from the section rows, so topology and history stay.
// Rows with differing weights (a circle and a square) are interpolated
// pointwise, as Parasolid does, through polynomial approximations (1e-8 of
// the part's size). False (shape untouched) if anything doesn't fit.
static bool natural_loft(const TopoDS_Shape& loft, const std::vector<TopoDS_Wire>& wires)
{
  const int n = (int)wires.size();
  Bnd_Box   box;
  BRepBndLib::Add(loft, box);
  const double size = box.IsVoid() ? 1.0 : std::sqrt(box.SquareExtent());
  const double tol = 1e-7 * size;
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
    for (int k = 0; k < n; ++k)
    {
      auto row = occ::down_cast<Geom_BSplineCurve>(bs->VIso(v0 + (v1 - v0) * k / (n - 1)));
      if (row.IsNull())
        return false;
      // (Loosely: ThruSections carries circles as polynomial approximations.)
      for (double s : {0.25, 0.5, 0.75})
      {
        BRepExtrema_DistShapeShape dist(BRepBuilderAPI_MakeVertex(row->Value(u0 + (u1 - u0) * s)).Vertex(), wires[k]);
        if (!dist.IsDone() || dist.Value() > 1e-4 * size)
          return false;
      }
      rows.push_back(row);
    }
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
    // Column by column: (x, y, z) or homogeneous (w x, w y, w z, w) with equal weights.
    const int  nu = rows[0]->NbPoles();
    const bool homogeneous = rows[0]->IsRational();
    std::vector<double> t(n);
    for (int k = 0; k < n; ++k)
      t[k] = v0 + (v1 - v0) * k / (n - 1);
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
      const auto col = natural_bezier(R, t);
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

static OpOut loft_impl(const OcShapeList& sections, bool solid, bool ruled, bool closed,
                       rust::Slice<const double> start_points, bool allow_natural)
{
  const size_t n = sections.items.size();
  if (n < 2)
    fail_invalid("loft needs at least 2 sections");
  if (!start_points.empty() && start_points.size() != 3 * n)
    fail_invalid("loft start points must be one (x, y, z) per section");
  bool natural = allow_natural && !ruled && !closed && n >= 3;
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
    if (!start_points.empty() && std::isfinite(start_points[3 * i]))
      w = rotate_wire_start(
        w, gp_Pnt(start_points[3 * i], start_points[3 * i + 1], start_points[3 * i + 2]));
    wires.push_back(w);
  }
  // Vertex counts matched like Parasolid; then OCCT must keep that matching.
  HHist      split;
  const bool matched = !points && start_points.empty() && match_vertex_counts(wires, split);
  ts.CheckCompatibility(!matched);
  TopoDS_Wire firstWire = wires.empty() ? TopoDS_Wire() : wires.front();
  for (size_t i = 0, w = 0; i < n; ++i)
    if (sections.items[i].ShapeType() == TopAbs_VERTEX)
      ts.AddVertex(TopoDS::Vertex(sections.items[i]));
    else
      ts.AddWire(wires[w++]);
  if (closed)
  {
    if (firstWire.IsNull())
      fail_invalid("closed loft requires wire sections");
    ts.AddWire(firstWire);
  }
  ts.Build();
  if (!ts.IsDone())
    fail_op("loft failed (incompatible sections?)");
  if (natural && (!natural_loft(ts.Shape(), wires) || !BRepCheck_Analyzer(ts.Shape()).IsValid()))
    return loft_impl(sections, solid, ruled, closed, start_points, false);
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
              rust::Slice<const double> start_points)
{
  return loft_impl(sections, solid, ruled, closed, start_points, true);
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

namespace {

// Builds one blend at `scale` x the requested sizes into (out, h); false if
// OCCT fails.
using BlendBuild = std::function<bool(double scale, TopoDS_Shape& out, HHist& h)>;

bool valid_shape(const TopoDS_Shape& s)
{
  return !s.IsNull() && BRepCheck_Analyzer(s).IsValid();
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
    gp_Trsf t;
    t.SetScale(gp_Pnt(0, 0, 0), 1.0 / factor);
    BRepBuilderAPI_Transform tr(r, t, true);
    HHist back = new BRepTools_History(to_list({r}), tr);
    HHist all  = compose(input, compose(input, to, h), back);
    return make_result({input}, tr.Shape(), all.get());
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
      ok = make(as_is)(1.0, r, h);
    }
    catch (const Standard_Failure&)
    {
    }
    if (ok && valid_shape(r))
      return as_is.finish(r, h);
  }
  const Scaled sc(input, checked ? working_scale() : 1.0);
  auto [r, h] = checked_blend(sc.shape, make(sc), checked, size * sc.factor, what);
  return sc.finish(r, h);
}

} // namespace

OpOut op_fillet(const OcShape& s, const OcShapeList& edges, rust::Slice<const double> radii,
                bool checked)
{
  require_non_null(s.shape, "shape");
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
  return scaled_blend(s.shape, checked, rmax, "fillet", [&](const Scaled& sc) -> BlendBuild {
    return [&sc, &es, &rs](double scale, TopoDS_Shape& out, HHist& h) {
      BRepFilletAPI_MakeFillet mk(sc.shape);
      for (size_t i = 0; i < es.size(); ++i)
        mk.Add(rs[i] * scale * sc.factor, TopoDS::Edge(sc.image(es[i])));
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

OpOut op_chamfer(const OcShape& s, const OcShapeList& edges, const OcShapeList& faces,
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
  try
  {
    return scaled_blend(s.shape, checked, size, "chamfer", make);
  }
  catch (const std::runtime_error&)
  {
    // Onshape reports a chamfer wider than a face it runs over (or than the
    // radius of an arc it follows) separately.
    for (const TopoDS_Edge& e : es)
    {
      BRepAdaptor_Curve c(e);
      if (c.GetType() == GeomAbs_Circle && c.Circle().Radius() < size * (1 - 1e-9))
        fail_op("chamfer distance too large");
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

// ---------------------------------------------------------------- draft

OpOut op_draft(const OcShape& s, const OcShapeList& faces, P3 dir, const PlaneF& neutral,
               double angle)
{
  require_non_null(s.shape, "shape");
  if (faces.items.empty())
    fail_invalid("draft needs at least one face");
  if (!(std::abs(angle) < M_PI / 2.0))
    fail_invalid("draft angle must be less than 90 degrees");
  check_faces_in(s.shape, faces.items);
  gp_Dir                   pull = to_dir(dir, "pull direction");
  gp_Pln                   pln  = to_pln(neutral);
  BRepOffsetAPI_DraftAngle mk(s.shape);
  for (const auto& f : faces.items)
  {
    mk.Add(TopoDS::Face(f), pull, angle, pln, true);
    if (!mk.AddDone())
      fail_op("draft cannot be applied to one of the faces");
  }
  mk.Build();
  if (!mk.IsDone())
    fail_op("draft failed");
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     h = modifier_history(s.shape, mk);
  return make_result(args, mk.Shape(), h.get());
}

// ---------------------------------------------------------------- defeature / offsets

OpOut op_defeature(const OcShape& s, const OcShapeList& faces)
{
  require_non_null(s.shape, "shape");
  if (faces.items.empty())
    fail_invalid("delete face needs at least one face");
  check_faces_in(s.shape, faces.items);
  BRepAlgoAPI_Defeaturing mk;
  mk.SetShape(s.shape);
  mk.AddFacesToRemove(to_list(faces.items));
  mk.SetRunParallel(true);
  mk.SetToFillHistory(true);
  mk.Build();
  if (!mk.IsDone() || mk.HasErrors())
    fail_op("delete face failed: " + algo_errors(mk));
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     h = history_of(args, mk);
  return make_result(args, mk.Shape(), h.get());
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
              rust::Slice<const int32_t> orders, rust::Slice<const double> points)
{
  if (edges.items.empty())
    fail_invalid("fill needs boundary edges");
  if (points.size() % 3 != 0)
    fail_invalid("fill points must be (x, y, z) triples");
  BRepOffsetAPI_MakeFilling mk;
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
