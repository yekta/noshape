// Booleans, split, unify, enclose, transforms, sections, projections.
#include "internal.hpp"

#include <BOPAlgo_MakerVolume.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepOffsetAPI_NormalProjection.hxx>
#include <BRepProj_Projection.hxx>
#include <Bnd_Box.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_GTrsf.hxx>
#include <gp_Trsf.hxx>

namespace noshape {

namespace {

using HHist = occ::handle<BRepTools_History>;

void apply_opts(BRepAlgoAPI_BuilderAlgo& a, const BoolOpts& o)
{
  a.SetRunParallel(o.parallel);
  if (o.fuzzy > 0)
    a.SetFuzzyValue(o.fuzzy);
  switch (o.glue)
  {
    case 1:
      a.SetGlue(BOPAlgo_GlueShift);
      break;
    case 2:
      a.SetGlue(BOPAlgo_GlueFull);
      break;
    default:
      a.SetGlue(BOPAlgo_GlueOff);
  }
  a.SetNonDestructive(true);
  a.SetToFillHistory(true);
}

void check_all(const std::vector<TopoDS_Shape>& v, const char* what)
{
  for (const auto& s : v)
    require_non_null(s, what);
}

std::vector<TopoDS_Shape> concat(const std::vector<TopoDS_Shape>& a,
                                 const std::vector<TopoDS_Shape>& b)
{
  std::vector<TopoDS_Shape> r = a;
  r.insert(r.end(), b.begin(), b.end());
  return r;
}

// Optional ShapeUpgrade_UnifySameDomain post-processing, composing history.
TopoDS_Shape unify(const TopoDS_Shape& s, HHist& hist)
{
  ShapeUpgrade_UnifySameDomain u(s, true, true, false);
  u.Build();
  if (!u.History().IsNull())
  {
    if (hist.IsNull())
      hist = new BRepTools_History();
    hist->Merge(u.History());
  }
  return u.Shape();
}

} // namespace

OpOut op_boolean(uint8_t op, const OcShapeList& args, const OcShapeList& tools,
                 const BoolOpts& opts)
{
  if (args.items.empty())
    fail_invalid("boolean needs at least one argument");
  if (tools.items.empty())
    fail_invalid("boolean needs at least one tool");
  check_all(args.items, "boolean argument");
  check_all(tools.items, "boolean tool");
  std::unique_ptr<BRepAlgoAPI_BooleanOperation> b;
  switch (op)
  {
    case 0:
      b = std::make_unique<BRepAlgoAPI_Fuse>();
      break;
    case 1:
      b = std::make_unique<BRepAlgoAPI_Cut>();
      break;
    case 2:
      b = std::make_unique<BRepAlgoAPI_Common>();
      break;
    default:
      fail_invalid("unknown boolean operation");
  }
  b->SetArguments(to_list(args.items));
  b->SetTools(to_list(tools.items));
  apply_opts(*b, opts);
  b->Build();
  if (b->HasErrors() || !b->IsDone())
    fail_op("boolean failed: " + algo_errors(*b));
  std::vector<TopoDS_Shape> all = concat(args.items, tools.items);
  HHist        hist = new BRepTools_History(to_list(all), *b);
  TopoDS_Shape res  = b->Shape();
  if (opts.unify)
    res = unify(res, hist);
  return make_result(all, res, hist.get());
}

OpOut op_split(const OcShapeList& args, const OcShapeList& tools, const BoolOpts& opts)
{
  if (args.items.empty())
    fail_invalid("split needs at least one argument");
  if (tools.items.empty())
    fail_invalid("split needs at least one tool");
  check_all(args.items, "split argument");
  check_all(tools.items, "split tool");
  BRepAlgoAPI_Splitter sp;
  sp.SetArguments(to_list(args.items));
  sp.SetTools(to_list(tools.items));
  apply_opts(sp, opts);
  sp.Build();
  if (sp.HasErrors() || !sp.IsDone())
    fail_op("split failed: " + algo_errors(sp));
  std::vector<TopoDS_Shape> all  = concat(args.items, tools.items);
  HHist                     hist = new BRepTools_History(to_list(all), sp);
  TopoDS_Shape              res  = sp.Shape();
  if (opts.unify)
    res = unify(res, hist);
  return make_result(all, res, hist.get());
}

OpOut op_unify(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  HHist        hist;
  TopoDS_Shape res = unify(s.shape, hist);
  return make_result({s.shape}, res, hist.get());
}

OpOut op_enclose(const OcShapeList& shapes)
{
  if (shapes.items.empty())
    fail_invalid("enclose needs shapes");
  check_all(shapes.items, "enclose input");
  BOPAlgo_MakerVolume mv;
  mv.SetArguments(to_list(shapes.items));
  mv.SetIntersect(true);
  mv.SetRunParallel(true);
  mv.Perform();
  if (mv.HasErrors())
    fail_op("enclose failed: " + algo_errors(mv));
  TopoDS_Shape res = mv.Shape();
  int          n   = 0;
  for (TopExp_Explorer e(res, TopAbs_SOLID); e.More(); e.Next())
    ++n;
  if (n == 0)
    fail_op("selections do not enclose a region");
  HHist hist = new BRepTools_History(to_list(shapes.items), mv);
  return make_result(shapes.items, res, hist.get());
}

static gp_Trsf trsf_of(rust::Slice<const double> m)
{
  if (m.size() != 12)
    fail_invalid("transform matrix must have 12 values (3x4 row-major)");
  for (double v : m)
    if (!std::isfinite(v))
      fail_invalid("transform matrix contains non-finite values");
  gp_Trsf t;
  t.SetValues(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
  return t;
}

OpOut op_transform(const OcShape& s, rust::Slice<const double> m, bool copy)
{
  require_non_null(s.shape, "shape");
  gp_Trsf t = trsf_of(m);
  if (has_mesh_faces(s.shape))
  {
    HHist              hist = new BRepTools_History();
    const TopoDS_Shape r    = transform_mesh(s.shape, t, *hist);
    return make_result({s.shape}, r, hist.get());
  }
  BRepBuilderAPI_Transform tr(s.shape, t, copy);
  if (!tr.IsDone())
    fail_op("transform failed");
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     hist = new BRepTools_History(to_list(args), tr);
  return make_result(args, tr.Shape(), hist.get());
}

OpOut op_gtransform(const OcShape& s, rust::Slice<const double> m)
{
  require_non_null(s.shape, "shape");
  if (m.size() != 12)
    fail_invalid("transform matrix must have 12 values (3x4 row-major)");
  gp_GTrsf g;
  for (int r = 0; r < 3; ++r)
  {
    for (int c = 0; c < 3; ++c)
      g.SetValue(r + 1, c + 1, m[r * 4 + c]);
    g.SetValue(r + 1, 4, m[r * 4 + 3]);
  }
  g.SetForm();
  if (has_mesh_faces(s.shape))
  {
    HHist              hist = new BRepTools_History();
    const TopoDS_Shape r    = gtransform_mesh(s.shape, g, *hist);
    return make_result({s.shape}, r, hist.get());
  }
  BRepBuilderAPI_GTransform tr(s.shape, g, true);
  if (!tr.IsDone())
    fail_op("general transform failed");
  std::vector<TopoDS_Shape> args{s.shape};
  HHist                     hist = new BRepTools_History();
  for (TopAbs_ShapeEnum k : {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
  {
    ShapeIMap m;
    TopExp::MapShapes(s.shape, k, m);
    for (int i = 1; i <= m.Extent(); ++i)
    {
      TopoDS_Shape r = tr.ModifiedShape(m(i));
      if (!r.IsNull() && !r.IsSame(m(i)) && r.ShapeType() == k)
        hist->AddModified(m(i), r);
    }
  }
  return make_result(args, tr.Shape(), hist.get());
}

OpOut op_section(const OcShape& a, const OcShape& b)
{
  require_non_null(a.shape, "first shape");
  require_non_null(b.shape, "second shape");
  BRepAlgoAPI_Section sec(a.shape, b.shape, false);
  sec.Approximation(true);
  sec.ComputePCurveOn1(true);
  sec.ComputePCurveOn2(true);
  sec.SetRunParallel(true);
  sec.Build();
  if (sec.HasErrors() || !sec.IsDone())
    fail_op("section failed: " + algo_errors(sec));
  std::vector<TopoDS_Shape> args{a.shape, b.shape};
  HHist                     hist = new BRepTools_History(to_list(args), sec);
  return make_result(args, sec.Shape(), hist.get());
}

std::unique_ptr<OcShape> section_plane(const OcShape& s, const PlaneF& plane)
{
  require_non_null(s.shape, "shape");
  BRepAlgoAPI_Section sec(s.shape, to_pln(plane), false);
  sec.Approximation(true);
  sec.SetRunParallel(true);
  sec.Build();
  if (sec.HasErrors() || !sec.IsDone())
    fail_op("plane section failed: " + algo_errors(sec));
  return wrap(sec.Shape());
}

std::unique_ptr<OcShape> section_caps(const OcShape& s, const PlaneF& plane)
{
  require_non_null(s.shape, "shape");
  Bnd_Box b;
  BRepBndLib::Add(s.shape, b, false);
  if (b.IsVoid())
    fail_invalid("shape has no geometry");
  const double D  = std::sqrt(b.SquareExtent()) + 1.0;
  gp_Pln       pl = to_pln(plane);
  double       x0, y0, z0, x1, y1, z1;
  b.Get(x0, y0, z0, x1, y1, z1);
  gp_Pnt c(0.5 * (x0 + x1), 0.5 * (y0 + y1), 0.5 * (z0 + z1));
  // Center (u, v) of the box projected on the plane.
  gp_Vec       oc(pl.Location(), c);
  const double u = oc.Dot(gp_Vec(pl.XAxis().Direction()));
  const double v = oc.Dot(gp_Vec(pl.YAxis().Direction()));
  BRepBuilderAPI_MakeFace mf(pl, u - D, u + D, v - D, v + D);
  if (!mf.IsDone())
    fail_op("could not build section plane face");
  BRepAlgoAPI_Common com(s.shape, mf.Face());
  if (com.HasErrors() || !com.IsDone())
    fail_op("section caps failed: " + algo_errors(com));
  return wrap(com.Shape());
}

// Curves to project: every wire (including face boundaries) and every edge
// that is not part of a wire, so compounds of curves or faces are accepted
// (BRepProj_Projection itself only takes a single edge or wire).
static std::vector<TopoDS_Shape> projectable_curves(const TopoDS_Shape& s)
{
  std::vector<TopoDS_Shape> out;
  if (s.ShapeType() == TopAbs_EDGE || s.ShapeType() == TopAbs_WIRE)
  {
    out.push_back(s);
    return out;
  }
  for (TopExp_Explorer e(s, TopAbs_WIRE); e.More(); e.Next())
    out.push_back(e.Current());
  for (TopExp_Explorer e(s, TopAbs_EDGE, TopAbs_WIRE); e.More(); e.Next())
    out.push_back(e.Current());
  return out;
}

std::unique_ptr<OcShape> project_along(const OcShape& curves, const OcShape& target, P3 dir)
{
  require_non_null(curves.shape, "curves");
  require_non_null(target.shape, "target");
  const gp_Dir              d     = to_dir(dir, "projection direction");
  std::vector<TopoDS_Shape> items = projectable_curves(curves.shape);
  if (items.empty())
    fail_invalid("nothing to project: the curves contain no edges");
  std::vector<TopoDS_Shape> results;
  for (const auto& c : items)
  {
    BRepProj_Projection pr(c, target.shape, d);
    if (!pr.IsDone())
      continue;
    TopoDS_Shape r = pr.Shape();
    if (!r.IsNull())
      results.push_back(r);
  }
  if (results.empty())
    fail_op("projection failed (curves do not project onto the target)");
  return wrap(results.size() == 1 ? results[0] : compound_of(results));
}

std::unique_ptr<OcShape> project_normal(const OcShape& curves, const OcShape& target)
{
  require_non_null(curves.shape, "curves");
  require_non_null(target.shape, "target");
  std::vector<TopoDS_Shape> items = projectable_curves(curves.shape);
  if (items.empty())
    fail_invalid("nothing to project: the curves contain no edges");
  BRepOffsetAPI_NormalProjection np(target.shape);
  for (const auto& c : items)
    np.Add(c);
  np.Build();
  if (!np.IsDone())
    fail_op("normal projection failed");
  return wrap(np.Shape());
}

} // namespace noshape
