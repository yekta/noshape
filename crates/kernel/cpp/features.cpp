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
#include <ShapeFix_Solid.hxx>
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

OpOut op_loft(const OcShapeList& sections, bool solid, bool ruled, bool closed,
              rust::Slice<const double> start_points)
{
  const size_t n = sections.items.size();
  if (n < 2)
    fail_invalid("loft needs at least 2 sections");
  if (!start_points.empty() && start_points.size() != 3 * n)
    fail_invalid("loft start points must be one (x, y, z) per section");
  BRepOffsetAPI_ThruSections ts(solid, ruled, 1e-6);
  ts.CheckCompatibility(true);
  std::vector<TopoDS_Shape> args;
  TopoDS_Wire               firstWire;
  for (size_t i = 0; i < n; ++i)
  {
    const TopoDS_Shape& s = sections.items[i];
    require_non_null(s, "loft section");
    args.push_back(s);
    if (s.ShapeType() == TopAbs_VERTEX)
    {
      if (i != 0 && i != n - 1)
        fail_invalid("only the first and last loft sections can be points");
      ts.AddVertex(TopoDS::Vertex(s));
      continue;
    }
    TopoDS_Wire w = outer_or_wire(s, "loft section");
    if (!start_points.empty() && std::isfinite(start_points[3 * i]))
      w = rotate_wire_start(
        w, gp_Pnt(start_points[3 * i], start_points[3 * i + 1], start_points[3 * i + 2]));
    if (i == 0)
      firstWire = w;
    ts.AddWire(w);
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
  HHist          h = history_of(args, ts);
  HistoryBuilder hb(args, ts.Shape());
  hb.add_history(h.get());
  if (!closed)
  {
    hb.add_cap(0, REL_FIRST, ts.FirstShape());
    hb.add_cap((uint32_t)n - 1, REL_LAST, ts.LastShape());
  }
  return hb.finish();
}

// ---------------------------------------------------------------- thicken / shell

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

OpOut op_fillet(const OcShape& s, const OcShapeList& edges, rust::Slice<const double> radii)
{
  require_non_null(s.shape, "shape");
  if (edges.items.empty())
    fail_invalid("fillet needs at least one edge");
  if (radii.size() != 1 && radii.size() != edges.items.size())
    fail_invalid("fillet radii must be one value or one per edge");
  BRepFilletAPI_MakeFillet mk(s.shape);
  for (size_t i = 0; i < edges.items.size(); ++i)
  {
    const double r = radii.size() == 1 ? radii[0] : radii[i];
    if (!(r > 1e-9))
      fail_invalid("fillet radius must be positive");
    mk.Add(r, first_adjacent_face_edge_check(s.shape, edges.items[i], "fillet edge"));
  }
  mk.Build();
  if (!mk.IsDone())
    fail_op("fillet failed (radius too large or unsupported edge configuration)");
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     h = history_of(args, mk);
  return make_result(args, mk.Shape(), h.get());
}

OpOut op_fillet_variable(const OcShape& s, const OcShape& edge, rust::Slice<const double> params,
                         rust::Slice<const double> radii)
{
  require_non_null(s.shape, "shape");
  if (params.size() != radii.size() || params.size() < 2)
    fail_invalid("variable fillet needs >= 2 (parameter, radius) pairs");
  NCollection_Array1<gp_Pnt2d> ur(1, (int)params.size());
  for (size_t i = 0; i < params.size(); ++i)
  {
    if (!(radii[i] > 1e-9))
      fail_invalid("fillet radius must be positive");
    if (params[i] < 0 || params[i] > 1)
      fail_invalid("variable fillet parameters must be in [0, 1]");
    if (i > 0 && !(params[i] > params[i - 1]))
      fail_invalid("variable fillet parameters must be increasing");
    ur((int)i + 1) = gp_Pnt2d(params[i], radii[i]);
  }
  BRepFilletAPI_MakeFillet mk(s.shape);
  mk.Add(ur, first_adjacent_face_edge_check(s.shape, edge.shape, "fillet edge"));
  mk.Build();
  if (!mk.IsDone())
    fail_op("variable fillet failed");
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     h = history_of(args, mk);
  return make_result(args, mk.Shape(), h.get());
}

OpOut op_chamfer(const OcShape& s, const OcShapeList& edges, const OcShapeList& faces,
                 uint8_t mode, double d1, double d2)
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
  ShapeAncMap anc;
  TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, anc);
  BRepFilletAPI_MakeChamfer mk(s.shape);
  for (size_t i = 0; i < edges.items.size(); ++i)
  {
    TopoDS_Edge e = first_adjacent_face_edge_check(s.shape, edges.items[i], "chamfer edge");
    if (mode == 0)
    {
      mk.Add(d1, e);
      continue;
    }
    TopoDS_Face f;
    if (i < faces.items.size() && !faces.items[i].IsNull())
      f = as_face(faces.items[i], "chamfer reference face");
    else
    {
      const ShapeList* l = anc.Seek(e);
      if (l == nullptr || l->IsEmpty())
        fail_invalid("chamfer edge has no adjacent face");
      f = TopoDS::Face(l->First());
    }
    if (mode == 1)
      mk.Add(d1, d2, e, f);
    else if (mode == 2)
      mk.AddDA(d1, d2, e, f);
    else
      fail_invalid("unknown chamfer mode");
  }
  mk.Build();
  if (!mk.IsDone())
    fail_op("chamfer failed (distance too large?)");
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     h = history_of(args, mk);
  return make_result(args, mk.Shape(), h.get());
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
