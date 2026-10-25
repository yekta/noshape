// Booleans, split, unify, enclose, transforms, sections, projections.
#include "internal.hpp"

#include <BOPAlgo_MakerVolume.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <Message_ProgressIndicator.hxx>
#include <Message_ProgressScope.hxx>
#include <BRepBndLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepGProp.hxx>
#include <OSD_Parallel.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <Geom_Surface.hxx>
#include <TopExp.hxx>
#include <TopoDS.hxx>
#include <BRepOffsetAPI_NormalProjection.hxx>
#include <BRepProj_Projection.hxx>
#include <Bnd_Box.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_GTrsf.hxx>
#include <gp_Trsf.hxx>

#include <BRepAdaptor_Curve.hxx>
#include <BRepLib.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRep_Builder.hxx>
#include <TopoDS_Wire.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <functional>
#include <map>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <atomic>

namespace noshape {

namespace {

using HHist = occ::handle<BRepTools_History>;

// The thread's cancel flag (set_thread_cancel): a Rust AtomicBool, read by
// the booleans' progress indicator so a stale regeneration stops inside OCCT.
thread_local const std::atomic<bool>* t_cancel = nullptr;

bool cancelled(const std::atomic<bool>* flag)
{
  return flag != nullptr && flag->load(std::memory_order_relaxed);
}

// Only answers UserBreak (it shows nothing).
class CancelIndicator : public Message_ProgressIndicator
{
public:
  explicit CancelIndicator(const std::atomic<bool>* flag) : myFlag(flag) {}
  bool UserBreak() override { return cancelled(myFlag); }
  void Show(const Message_ProgressScope&, const bool) override {}

private:
  const std::atomic<bool>* myFlag;
};

// Build `algo`, stoppable through the thread's cancel flag.
template <class A> void build_cancellable(A& algo)
{
  if (t_cancel == nullptr)
  {
    algo.Build();
    return;
  }
  occ::handle<CancelIndicator> pi = new CancelIndicator(t_cancel);
  algo.Build(pi->Start());
}

// Build `algo`, stoppable through `flag` (a pool thread has no thread flag).
template <class A> void build_with(A& algo, const std::atomic<bool>* flag)
{
  if (flag == nullptr)
  {
    algo.Build();
    return;
  }
  occ::handle<CancelIndicator> pi = new CancelIndicator(flag);
  algo.Build(pi->Start());
}

void fail_if_cancelled()
{
  if (cancelled(t_cancel))
    fail_op("cancelled");
}

void apply_opts(BRepAlgoAPI_BuilderAlgo& a, const BoolOpts& o)
{
  a.SetRunParallel(o.parallel);
  a.SetUseOBB(o.obb);
  if (o.fuzzy > 0)
    a.SetFuzzyValue(o.fuzzy);
  switch (o.glue)
  {
    case 1:
    case 3: // checked: see op_boolean
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

// Whether two adjacent faces lie on one surface, or two edges meeting at a
// vertex of only those two lie on one line, i.e. whether
// ShapeUpgrade_UnifySameDomain could change anything. Linear in the shape;
// unifying a shape it would leave unchanged costs seconds on large results
// (a part with 1000 patterned ribs: 14 s).
bool same_domain_candidates(const TopoDS_Shape& s)
{
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  auto plane_of = [](const TopoDS_Face& f, gp_Pln& pl) {
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> g = BRep_Tool::Surface(f, loc);
    occ::handle<Geom_Plane>         p = occ::down_cast<Geom_Plane>(g);
    if (p.IsNull())
      return false;
    pl = p->Pln().Transformed(loc.Transformation());
    return true;
  };
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const ShapeList& fs = ef(i);
    if (fs.Extent() != 2)
      continue;
    const TopoDS_Face a = TopoDS::Face(fs.First()), b = TopoDS::Face(fs.Last());
    TopLoc_Location   la, lb;
    const occ::handle<Geom_Surface> ga = BRep_Tool::Surface(a, la), gb = BRep_Tool::Surface(b, lb);
    if (ga == gb && la.IsEqual(lb))
      return true;
    gp_Pln pa, pb;
    const bool planar_a = plane_of(a, pa), planar_b = plane_of(b, pb);
    if (planar_a && planar_b)
    {
      if (pa.Axis().Direction().IsParallel(pb.Axis().Direction(), 1e-9) && pa.Distance(pb.Location()) < 1e-7)
        return true;
      continue;
    }
    if (planar_a || planar_b)
      continue;
    // Two curved faces: the same kind of surface may be one domain.
    if (ga->DynamicType() == gb->DynamicType())
      return true;
  }
  // Collinear edges that could merge: a vertex shared by exactly two
  // straight edges on one line.
  ShapeAncMap ve;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_VERTEX, TopAbs_EDGE, ve);
  for (int i = 1; i <= ve.Extent(); ++i)
  {
    const ShapeList& es = ve(i);
    if (es.Extent() != 2)
      continue;
    double                        f1, l1, f2, l2;
    const occ::handle<Geom_Curve> c1 = BRep_Tool::Curve(TopoDS::Edge(es.First()), f1, l1);
    const occ::handle<Geom_Curve> c2 = BRep_Tool::Curve(TopoDS::Edge(es.Last()), f2, l2);
    if (c1.IsNull() || c2.IsNull())
      continue;
    occ::handle<Geom_Line> a = occ::down_cast<Geom_Line>(c1), b = occ::down_cast<Geom_Line>(c2);
    if (a.IsNull() != b.IsNull())
      continue;
    if (a.IsNull())
    {
      if (c1->DynamicType() == c2->DynamicType())
        return true;
      continue;
    }
    TopLoc_Location l1o, l2o;
    BRep_Tool::Curve(TopoDS::Edge(es.First()), l1o, f1, l1);
    BRep_Tool::Curve(TopoDS::Edge(es.Last()), l2o, f2, l2);
    const gp_Lin la = a->Lin().Transformed(l1o.Transformation()), lb = b->Lin().Transformed(l2o.Transformation());
    if (la.Direction().IsParallel(lb.Direction(), 1e-9) && la.Distance(lb.Location()) < 1e-7)
      return true;
  }
  return false;
}

// A planar face's plane, its normal along the face's outside (the face's
// orientation applied).
bool outer_plane(const TopoDS_Face& f, gp_Pln& pl)
{
  TopLoc_Location                 loc;
  const occ::handle<Geom_Plane>   p = occ::down_cast<Geom_Plane>(BRep_Tool::Surface(f, loc));
  if (p.IsNull())
    return false;
  pl = p->Pln().Transformed(loc.Transformation());
  if (f.Orientation() == TopAbs_REVERSED)
    pl = gp_Pln(gp_Ax3(pl.Location(), pl.Axis().Direction().Reversed(), pl.XAxis().Direction()));
  return true;
}

// Points along an edge in its orientation (`n` samples, its end excluded).
void edge_samples(const TopoDS_Edge& e, int n, std::vector<gp_Pnt>& out)
{
  BRepAdaptor_Curve c(e);
  const double      f = c.FirstParameter(), l = c.LastParameter();
  const bool        rev = e.Orientation() == TopAbs_REVERSED;
  for (int i = 0; i < n; ++i)
  {
    const double u = (double)i / n;
    out.push_back(c.Value(rev ? l + (f - l) * u : f + (l - f) * u));
  }
}

// What UnifySameDomain does for planes, in linear time: each connected
// group of coplanar, same-facing planar faces becomes one face bounded by
// the group's boundary edges (wires walked through a vertex map), then
// chains of collinear straight edges through vertices of only those two
// edges become one edge. Groups it can't rebuild simply (a vertex where the
// boundary touches itself, seam edges, more than one outer loop) and
// curved faces are left to UnifySameDomain. UnifySameDomain re-fixes every
// merged face, O(edges²) per face: 3.6 s for 1000 full-width fins, whose
// side faces merge into two combs of 6000 edges.
TopoDS_Shape merge_planar(const TopoDS_Shape& s, HHist& hist)
{
  ShapeIMap faces;
  TopExp::MapShapes(s, TopAbs_FACE, faces);
  const int           nf = faces.Extent();
  std::vector<char>   planar((size_t)nf + 1, 0);
  std::vector<gp_Pln> pl((size_t)nf + 1);
  for (int i = 1; i <= nf; ++i)
    planar[(size_t)i] = outer_plane(TopoDS::Face(faces(i)), pl[(size_t)i]);
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  std::vector<int> up((size_t)nf + 1);
  for (int i = 0; i <= nf; ++i)
    up[(size_t)i] = i;
  std::function<int(int)> root = [&](int i) { return up[(size_t)i] == i ? i : up[(size_t)i] = root(up[(size_t)i]); };
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const ShapeList& fs = ef(i);
    if (fs.Extent() != 2)
      continue;
    const int a = faces.FindIndex(fs.First()), b = faces.FindIndex(fs.Last());
    if (a == b || !planar[(size_t)a] || !planar[(size_t)b])
      continue;
    const gp_Pln &pa = pl[(size_t)a], &pb = pl[(size_t)b];
    if (pa.Axis().Direction().IsEqual(pb.Axis().Direction(), 1e-9) && pa.Distance(pb.Location()) < 1e-7)
      up[(size_t)root(a)] = root(b);
  }
  std::map<int, std::vector<int>> groups;
  for (int i = 1; i <= nf; ++i)
    groups[root(i)].push_back(i);
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape();
  // As UnifySameDomain records it: every merged face modified into the new
  // one, inner edges and vertices removed.
  HHist                          h = new BRepTools_History();
  std::vector<TopoDS_Shape>      gone; // inner edges and vertices of merged groups
  bool                           any = false;
  BRep_Builder                   B;
  for (const auto& [r, g] : groups)
  {
    if (g.size() < 2)
      continue;
    // The group's edges as the faces use them (orientations composed with
    // the faces': all face the same way), and how often.
    ShapeIMap        em;
    std::vector<int> count;
    std::vector<TopoDS_Edge> oriented;
    bool             ok = true;
    for (int fi : g)
      for (TopExp_Explorer ex(faces(fi), TopAbs_EDGE); ex.More(); ex.Next())
      {
        const int k = em.Add(ex.Current());
        if (k > (int)count.size())
        {
          count.push_back(0);
          oriented.push_back(TopoDS::Edge(ex.Current()));
        }
        ++count[(size_t)k - 1];
        if (count[(size_t)k - 1] == 2 && ex.Current().Orientation() == oriented[(size_t)k - 1].Orientation())
          ok = false; // the same edge twice the same way: a seam
        if (count[(size_t)k - 1] > 2 || BRep_Tool::Degenerated(TopoDS::Edge(ex.Current())))
          ok = false;
      }
    if (!ok)
      continue;
    // The boundary: edges used once, walked into loops by their oriented
    // start vertex.
    NCollection_IndexedDataMap<TopoDS_Shape, std::vector<int>, TopTools_ShapeMapHasher> from;
    std::vector<int> boundary;
    for (int k = 0; k < (int)count.size(); ++k)
      if (count[(size_t)k] == 1)
      {
        boundary.push_back(k);
        const TopoDS_Vertex v = TopExp::FirstVertex(oriented[(size_t)k], true);
        const int           j = from.Add(v, {});
        from(j).push_back(k);
      }
    for (int j = 1; j <= from.Extent() && ok; ++j)
      ok = from(j).size() == 1; // the boundary touches itself here
    if (!ok || boundary.empty())
      continue;
    std::vector<char>                     used(count.size(), 0);
    std::vector<std::vector<TopoDS_Edge>> loops;
    for (int k0 : boundary)
    {
      if (used[(size_t)k0])
        continue;
      std::vector<TopoDS_Edge> loop;
      int                      k = k0;
      while (ok && !used[(size_t)k])
      {
        used[(size_t)k] = 1;
        loop.push_back(oriented[(size_t)k]);
        const TopoDS_Vertex end = TopExp::LastVertex(oriented[(size_t)k], true);
        const std::vector<int>* next = from.Seek(end);
        if (next == nullptr)
          ok = false;
        else
          k = (*next)[0];
      }
      if (!ok || k != k0)
      {
        ok = false;
        break;
      }
      loops.push_back(std::move(loop));
    }
    if (!ok)
      continue;
    // Outer loop: counterclockwise about the outside normal.
    const gp_Pln& p = pl[(size_t)g[0]];
    const gp_Ax3  ax = p.Position();
    std::vector<double> area;
    for (const auto& loop : loops)
    {
      std::vector<gp_Pnt> pts;
      for (const TopoDS_Edge& e : loop)
      {
        TopLoc_Location loc;
        double          f, l;
        const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, loc, f, l);
        edge_samples(e, !c.IsNull() && c->IsKind(STANDARD_TYPE(Geom_Line)) ? 1 : 16, pts);
      }
      double a = 0;
      for (size_t i = 0; i < pts.size(); ++i)
      {
        const gp_Vec d0(ax.Location(), pts[i]), d1(ax.Location(), pts[(i + 1) % pts.size()]);
        const double u0 = d0.Dot(ax.XDirection()), v0 = d0.Dot(ax.YDirection());
        const double u1 = d1.Dot(ax.XDirection()), v1 = d1.Dot(ax.YDirection());
        a += u0 * v1 - u1 * v0;
      }
      // gp_Ax3 may be left-handed: then (X, Y) turn the other way about the normal.
      area.push_back(ax.Direct() ? a : -a);
    }
    int outer = -1, outers = 0;
    for (size_t i = 0; i < area.size(); ++i)
      if (area[i] > 0)
        outer = (int)i, ++outers;
    if (outers != 1)
      continue;
    // The merged face on the first face's surface, in its orientation.
    const TopoDS_Face& f0  = TopoDS::Face(faces(g[0]));
    const bool         rev = f0.Orientation() == TopAbs_REVERSED;
    TopLoc_Location    loc;
    const occ::handle<Geom_Surface>& surf = BRep_Tool::Surface(f0, loc);
    TopoDS_Face        nf_;
    B.MakeFace(nf_, surf, loc, BRep_Tool::Tolerance(f0));
    auto add_loop = [&](const std::vector<TopoDS_Edge>& loop) {
      TopoDS_Wire w;
      B.MakeWire(w);
      for (const TopoDS_Edge& e : loop)
        B.Add(w, rev ? e.Reversed() : TopoDS_Shape(e));
      w.Closed(true);
      B.Add(nf_, w);
    };
    add_loop(loops[(size_t)outer]);
    for (size_t i = 0; i < loops.size(); ++i)
      if ((int)i != outer)
        add_loop(loops[i]);
    for (const auto& loop : loops)
      for (const TopoDS_Edge& e : loop)
        BRepLib::BuildPCurveForEdgeOnPlane(e, nf_);
    if (rev)
      nf_.Reverse();
    rs->Replace(faces(g[0]), nf_);
    for (size_t i = 1; i < g.size(); ++i)
      rs->Remove(faces(g[i]));
    for (int fi : g)
      h->AddModified(faces(fi), nf_);
    // Inner edges, and vertices only they used, disappear.
    ShapeIMap kept_v;
    for (int k : boundary)
    {
      kept_v.Add(TopExp::FirstVertex(oriented[(size_t)k]));
      kept_v.Add(TopExp::LastVertex(oriented[(size_t)k]));
    }
    for (int k = 0; k < (int)count.size(); ++k)
      if (count[(size_t)k] == 2)
      {
        gone.push_back(em(k + 1));
        for (const TopoDS_Vertex& v : {TopExp::FirstVertex(oriented[(size_t)k]), TopExp::LastVertex(oriented[(size_t)k])})
          if (!kept_v.Contains(v))
            gone.push_back(v);
      }
    any = true;
  }
  TopoDS_Shape r = s;
  if (any)
  {
    r = rs->Apply(s);
    for (const TopoDS_Shape& x : gone)
      if (!h->IsRemoved(x))
        h->Remove(x);
    if (hist.IsNull())
      hist = new BRepTools_History();
    hist->Merge(h);
  }
  // Collinear straight edges through vertices of only those two.
  ShapeAncMap ve, ef2;
  TopExp::MapShapesAndUniqueAncestors(r, TopAbs_VERTEX, TopAbs_EDGE, ve);
  TopExp::MapShapesAndUniqueAncestors(r, TopAbs_EDGE, TopAbs_FACE, ef2);
  auto line_of = [](const TopoDS_Edge& e, gp_Lin& lin) {
    TopLoc_Location loc;
    double          f, l;
    const occ::handle<Geom_Line> c = occ::down_cast<Geom_Line>(BRep_Tool::Curve(e, loc, f, l));
    if (c.IsNull())
      return false;
    lin = c->Lin().Transformed(loc.Transformation());
    return true;
  };
  auto planar_faces_of = [&](const TopoDS_Shape& e) {
    const ShapeList* fs = ef2.Seek(e);
    if (fs == nullptr || fs->Extent() != 2)
      return false;
    gp_Pln p;
    for (ShapeList::Iterator it(*fs); it.More(); it.Next())
      if (!outer_plane(TopoDS::Face(it.Value()), p))
        return false;
    return true;
  };
  ShapeIMap mergeable; // vertices
  for (int i = 1; i <= ve.Extent(); ++i)
  {
    const ShapeList& es = ve(i);
    if (es.Extent() != 2)
      continue;
    const TopoDS_Edge e1 = TopoDS::Edge(es.First()), e2 = TopoDS::Edge(es.Last());
    gp_Lin l1, l2;
    if (e1.IsSame(e2) || !line_of(e1, l1) || !line_of(e2, l2) || !l1.Direction().IsParallel(l2.Direction(), 1e-9) || l1.Distance(l2.Location()) > 1e-7)
      continue;
    const ShapeList *f1 = ef2.Seek(e1), *f2 = ef2.Seek(e2);
    if (f1 == nullptr || f2 == nullptr || f1->Extent() != 2 || f2->Extent() != 2 || !planar_faces_of(e1) || !planar_faces_of(e2))
      continue;
    const bool same = (f1->First().IsSame(f2->First()) && f1->Last().IsSame(f2->Last())) || (f1->First().IsSame(f2->Last()) && f1->Last().IsSame(f2->First()));
    if (same)
      mergeable.Add(ve.FindKey(i));
  }
  if (mergeable.IsEmpty())
    return r;
  // Chains: edges joined through mergeable vertices.
  ShapeIMap        edges;
  std::vector<int> eup;
  for (int i = 1; i <= mergeable.Extent(); ++i)
    for (ShapeList::Iterator it(ve.FindFromKey(mergeable(i))); it.More(); it.Next())
      if (edges.Add(it.Value()) > (int)eup.size())
        eup.push_back((int)eup.size());
  std::function<int(int)> eroot = [&](int i) { return eup[(size_t)i] == i ? i : eup[(size_t)i] = eroot(eup[(size_t)i]); };
  for (int i = 1; i <= mergeable.Extent(); ++i)
  {
    const ShapeList& es = ve.FindFromKey(mergeable(i));
    eup[(size_t)eroot(edges.FindIndex(es.First()) - 1)] = eroot(edges.FindIndex(es.Last()) - 1);
  }
  std::map<int, std::vector<int>> chains;
  for (int i = 0; i < edges.Extent(); ++i)
    chains[eroot(i)].push_back(i);
  occ::handle<BRepTools_ReShape> rs2 = new BRepTools_ReShape();
  HHist                          h2  = new BRepTools_History();
  std::vector<TopoDS_Shape>      gone2;
  for (const auto& [c, members] : chains)
  {
    // The chain's two end vertices; a closed chain (a full loop of one
    // line) can't be.
    std::vector<TopoDS_Vertex> ends;
    for (int m : members)
    {
      const TopoDS_Edge e = TopoDS::Edge(edges(m + 1));
      for (const TopoDS_Vertex& v : {TopExp::FirstVertex(e), TopExp::LastVertex(e)})
      {
        if (mergeable.Contains(v))
        {
          bool seen = false;
          for (const auto& g2 : gone2)
            seen = seen || g2.IsSame(v);
          if (!seen)
            gone2.push_back(v);
        }
        else
          ends.push_back(v);
      }
    }
    if (ends.size() != 2)
      return r;
    const TopoDS_Edge e0 = TopoDS::Edge(edges(members[0] + 1));
    gp_Lin            lin;
    line_of(TopoDS::Edge(e0.Oriented(TopAbs_FORWARD)), lin);
    // Along the first edge's own direction.
    const gp_Pnt a = BRep_Tool::Pnt(ends[0]), b = BRep_Tool::Pnt(ends[1]);
    const bool   ab = gp_Vec(a, b).Dot(gp_Vec(lin.Direction())) > 0;
    BRepBuilderAPI_MakeEdge mk(ab ? ends[0] : ends[1], ab ? ends[1] : ends[0]);
    if (!mk.IsDone())
      return r;
    const TopoDS_Edge ne = mk.Edge();
    for (ShapeList::Iterator it(ef2.FindFromKey(e0)); it.More(); it.Next())
      BRepLib::BuildPCurveForEdgeOnPlane(ne, TopoDS::Face(it.Value()));
    rs2->Replace(e0.Oriented(TopAbs_FORWARD), ne);
    for (size_t i = 1; i < members.size(); ++i)
      rs2->Remove(edges(members[i] + 1));
    for (int m : members)
      h2->AddModified(edges(m + 1), ne);
  }
  const TopoDS_Shape r2 = rs2->Apply(r);
  // The faces rebuilt around the new edges.
  {
    const occ::handle<BRepTools_History> rebuilt = rs2->History();
    for (TopExp_Explorer e(r, TopAbs_FACE); e.More(); e.Next())
      for (ShapeList::Iterator it(rebuilt->Modified(e.Current())); it.More(); it.Next())
        if (!h2->Modified(e.Current()).Contains(it.Value()))
          h2->AddModified(e.Current(), it.Value());
  }
  for (const TopoDS_Shape& v : gone2)
    if (!h2->IsRemoved(v))
      h2->Remove(v);
  if (hist.IsNull())
    hist = new BRepTools_History();
  hist->Merge(h2);
  return r2;
}

// Optional ShapeUpgrade_UnifySameDomain post-processing, composing history
// (skipped when it could change nothing; planar faces and straight edges
// merged first by merge_planar).
TopoDS_Shape unify(const TopoDS_Shape& s0, HHist& hist)
{
  if (!same_domain_candidates(s0))
    return s0;
  const TopoDS_Shape s = merge_planar(s0, hist);
  if (!same_domain_candidates(s))
    return s;
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

namespace
{

double volume_of(const TopoDS_Shape& x)
{
  GProp_GProps g;
  BRepGProp::VolumeProperties(x, g);
  return std::abs(g.Mass());
}

// The union's volume is the sum of the inputs': they only touch.
bool volumes_add_up(const std::vector<TopoDS_Shape>& inputs, const TopoDS_Shape& result)
{
  double sum = 0;
  for (const TopoDS_Shape& x : inputs)
    sum += volume_of(x);
  return std::abs(volume_of(result) - sum) <= 1e-7 * std::max(sum, 1e-12);
}

void for_sub_shapes(const TopoDS_Shape& s, const std::function<void(const TopoDS_Shape&)>& f)
{
  for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
  {
    ShapeIMap m;
    TopExp::MapShapes(s, k, m);
    for (int i = 1; i <= m.Extent(); ++i)
      f(m(i));
  }
}

// Fuse many tools that reach into the arguments (pattern instances through a
// part): each tool minus the arguments (small independent cuts, in
// parallel), then a glue fuse of what is left, which only touches the
// arguments. One fuse of all the tools at once is quadratic in their number
// (every tool's intersection walks the vertices all the others left on the
// shared faces: 50 s for 1000 ribs through a block, 4.5 s this way). False
// when it doesn't apply (a cut failed, the volumes don't add up).
bool fuse_trimmed(const std::vector<TopoDS_Shape>& a, const std::vector<TopoDS_Shape>& t, const BoolOpts& opts, TopoDS_Shape& out, HHist& hist)
{
  const int                                     n = (int)t.size();
  std::vector<TopoDS_Shape>                     trimmed((size_t)n);
  std::vector<std::unique_ptr<BRepAlgoAPI_Cut>> cuts((size_t)n);
  std::vector<char>                             failed((size_t)n, 0);
  const ShapeList                               args = to_list(a);
  const std::atomic<bool>*                      flag = t_cancel;
  OSD_Parallel::For(0, n, [&](int i) {
    if (cancelled(flag))
    {
      failed[(size_t)i] = 1;
      return;
    }
    auto                 c = std::make_unique<BRepAlgoAPI_Cut>();
    ShapeList one;
    one.Append(t[(size_t)i]);
    c->SetArguments(one);
    c->SetTools(args);
    c->SetRunParallel(false);
    c->SetNonDestructive(true);
    c->SetToFillHistory(true);
    c->Build();
    if (c->HasErrors() || !c->IsDone())
      failed[(size_t)i] = 1;
    else
      trimmed[(size_t)i] = c->Shape();
    cuts[(size_t)i] = std::move(c);
  });
  std::vector<TopoDS_Shape> rest;
  for (int i = 0; i < n; ++i)
  {
    if (failed[(size_t)i])
      return false;
    TopExp_Explorer e(trimmed[(size_t)i], TopAbs_SOLID);
    if (e.More())
      rest.push_back(trimmed[(size_t)i]);
  }
  if (rest.empty())
    return false;
  BRepAlgoAPI_Fuse f;
  f.SetArguments(args);
  f.SetTools(to_list(rest));
  BoolOpts glue = opts;
  glue.glue     = 1;
  apply_opts(f, glue);
  build_cancellable(f);
  if (f.HasErrors() || !f.IsDone() || !volumes_add_up(concat(a, rest), f.Shape()))
    return false;
  // History from the original inputs: the arguments as the fuse has them;
  // a tool's sub-shapes through its cut, then through the fuse.
  occ::handle<BRepTools_History> h2 = new BRepTools_History(to_list(concat(a, rest)), f);
  HHist                          h  = new BRepTools_History();
  auto through_fuse = [&](const TopoDS_Shape& s, const TopoDS_Shape& x, bool generated) {
    // Where `x` (a stage-1 image of `s`) ends up; false if the fuse removed it.
    bool kept = false;
    if (!h2->IsRemoved(x))
    {
      const ShapeList& m = h2->Modified(x);
      if (m.IsEmpty())
      {
        kept = true;
        if (!x.IsSame(s))
          generated ? h->AddGenerated(s, x) : h->AddModified(s, x);
      }
      for (ShapeList::Iterator it(m); it.More(); it.Next())
      {
        kept = true;
        generated ? h->AddGenerated(s, it.Value()) : h->AddModified(s, it.Value());
      }
    }
    for (ShapeList::Iterator it(h2->Generated(x)); it.More(); it.Next())
      h->AddGenerated(s, it.Value());
    return kept;
  };
  for (const TopoDS_Shape& x : a)
    for_sub_shapes(x, [&](const TopoDS_Shape& s) {
      if (h2->IsRemoved(s))
        h->Remove(s);
      for (ShapeList::Iterator it(h2->Modified(s)); it.More(); it.Next())
        h->AddModified(s, it.Value());
      for (ShapeList::Iterator it(h2->Generated(s)); it.More(); it.Next())
        h->AddGenerated(s, it.Value());
    });
  for (int i = 0; i < n; ++i)
  {
    BRepAlgoAPI_Cut& c = *cuts[(size_t)i];
    for_sub_shapes(t[(size_t)i], [&](const TopoDS_Shape& s) {
      if (c.IsDeleted(s))
      {
        h->Remove(s);
        return;
      }
      const ShapeList& m1   = c.Modified(s);
      bool                        kept = false;
      if (m1.IsEmpty())
        kept = through_fuse(s, s, false);
      for (ShapeList::Iterator it(m1); it.More(); it.Next())
        kept |= through_fuse(s, it.Value(), false);
      for (ShapeList::Iterator it(c.Generated(s)); it.More(); it.Next())
        through_fuse(s, it.Value(), true);
      if (!kept)
        h->Remove(s);
    });
  }
  out  = f.Shape();
  hist = h;
  return true;
}

// One stage of a staged boolean: where each sub-shape of its inputs went.
struct Stage
{
  HHist     hist;
  ShapeIMap inputs; // the stage's input sub-shapes (faces, edges, vertices)
};

Stage stage_of(const std::vector<TopoDS_Shape>& inputs, const HHist& h)
{
  Stage st;
  st.hist = h;
  for (const TopoDS_Shape& x : inputs)
    for_sub_shapes(x, [&](const TopoDS_Shape& s) { st.inputs.Add(s); });
  return st;
}

// History from the original inputs through stages run one after another;
// a sub-shape a stage didn't take as input passes it unchanged (a later
// stage's histories only know their own inputs: per-slab fuses).
HHist compose(const std::vector<TopoDS_Shape>& originals, const std::vector<std::vector<Stage>>& stages)
{
  HHist h = new BRepTools_History();
  for (const TopoDS_Shape& o : originals)
    for_sub_shapes(o, [&](const TopoDS_Shape& s) {
      // (shape, generated) images, stage by stage.
      std::vector<std::pair<TopoDS_Shape, bool>> cur{{s, false}};
      for (const std::vector<Stage>& level : stages)
      {
        std::vector<std::pair<TopoDS_Shape, bool>> next;
        for (const auto& [x, gen] : cur)
        {
          const Stage* st = nullptr;
          for (const Stage& c : level)
            if (c.inputs.Contains(x))
            {
              st = &c;
              break;
            }
          if (st == nullptr)
          {
            next.emplace_back(x, gen);
            continue;
          }
          if (!st->hist->IsRemoved(x))
          {
            const ShapeList& m = st->hist->Modified(x);
            if (m.IsEmpty())
              next.emplace_back(x, gen);
            for (ShapeList::Iterator it(m); it.More(); it.Next())
              next.emplace_back(it.Value(), gen);
          }
          for (ShapeList::Iterator it(st->hist->Generated(x)); it.More(); it.Next())
            next.emplace_back(it.Value(), true);
        }
        // Dedupe (a face split and merged again comes back once).
        cur.clear();
        for (const auto& n : next)
        {
          bool seen = false;
          for (const auto& c : cur)
            seen = seen || (c.first.IsSame(n.first) && c.second == n.second);
          if (!seen)
            cur.push_back(n);
        }
      }
      bool kept = false;
      for (const auto& [y, gen] : cur)
      {
        if (gen)
          h->AddGenerated(s, y);
        else
        {
          kept = true;
          if (!y.IsSame(s))
            h->AddModified(s, y);
        }
      }
      if (!kept)
        h->Remove(s);
    });
  return h;
}

// Solids that meet only on shared faces (the same TShape in both, as the
// splitter leaves the cuts between slabs) joined into one: their faces
// without the shared ones, one closed shell. Null when the parts aren't
// like that (an edge not used by exactly two faces, a part of several
// solids); then a glue fuse joins them.
TopoDS_Shape join_on_shared_faces(const std::vector<TopoDS_Shape>& parts, std::vector<TopoDS_Shape>& shared)
{
  ShapeIMap                 fm;
  std::vector<int>          count;
  std::vector<TopoDS_Shape> oriented;
  for (const TopoDS_Shape& p : parts)
  {
    int solids = 0;
    for (TopExp_Explorer e(p, TopAbs_SOLID); e.More(); e.Next())
      ++solids;
    if (solids != 1)
      return TopoDS_Shape();
    for (TopExp_Explorer e(p, TopAbs_FACE); e.More(); e.Next())
    {
      const int k = fm.Add(e.Current());
      if (k > (int)count.size())
      {
        count.push_back(0);
        oriented.push_back(e.Current());
      }
      if (++count[(size_t)k - 1] > 2)
        return TopoDS_Shape();
      if (count[(size_t)k - 1] == 2 && e.Current().Orientation() == oriented[(size_t)k - 1].Orientation())
        return TopoDS_Shape(); // a shared face must face opposite ways
    }
  }
  BRep_Builder B;
  TopoDS_Shell sh;
  B.MakeShell(sh);
  for (size_t k = 0; k < count.size(); ++k)
  {
    if (count[k] == 1)
      B.Add(sh, oriented[k]);
    else
      shared.push_back(oriented[k]);
  }
  // Closed: every edge used twice (a seam twice by its face).
  NCollection_IndexedDataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> uses;
  for (TopExp_Explorer f(sh, TopAbs_FACE); f.More(); f.Next())
    for (TopExp_Explorer e(f.Current(), TopAbs_EDGE); e.More(); e.Next())
    {
      const int i = uses.Add(e.Current(), 0);
      ++uses(i);
    }
  for (int i = 1; i <= uses.Extent(); ++i)
    if (uses(i) != 2 && !BRep_Tool::Degenerated(TopoDS::Edge(uses.FindKey(i))))
      return TopoDS_Shape();
  sh.Closed(true);
  TopoDS_Solid so;
  B.MakeSolid(so);
  B.Add(so, sh);
  return so;
}

// A glue fuse of many tools that only touch one target (pattern instances
// on a part), in slabs: the target cut by planes between groups of tools,
// each slab fused with its own tools (in parallel), the slabs glued back
// together and unified (the cuts vanish). One fuse of all the tools is
// superlinear: the target's long faces and edges collect every instance's
// intersections (1000 full-width fins on a block: 7.7 s and 3.6 s of unify;
// in 16 slabs 2.7 s). False when it doesn't apply: too few tools, no gaps
// between them, a step failing or the volumes not adding up.
bool fuse_slabs(const TopoDS_Shape& target, const std::vector<TopoDS_Shape>& t, const BoolOpts& opts, TopoDS_Shape& out, HHist& hist)
{
  const int n = (int)t.size();
  if (n < 128)
    return false;
  // The tools' boxes; the axis along which they leave the most gaps.
  std::vector<Bnd_Box> boxes((size_t)n);
  for (int i = 0; i < n; ++i)
  {
    BRepBndLib::Add(t[(size_t)i], boxes[(size_t)i], false);
    if (boxes[(size_t)i].IsVoid())
      return false;
  }
  Bnd_Box tb;
  BRepBndLib::Add(target, tb, false);
  if (tb.IsVoid())
    return false;
  auto lo = [&](int i, int ax) {
    double x[6];
    boxes[(size_t)i].Get(x[0], x[1], x[2], x[3], x[4], x[5]);
    return x[ax];
  };
  auto hi = [&](int i, int ax) {
    double x[6];
    boxes[(size_t)i].Get(x[0], x[1], x[2], x[3], x[4], x[5]);
    return x[3 + ax];
  };
  const double gap_tol = 1e-6 * std::sqrt(tb.SquareExtent());
  int                 best_axis = -1;
  std::vector<int>    best_order;
  std::vector<int>    best_cuts; // index in the order where a gap starts a new group
  for (int ax = 0; ax < 3; ++ax)
  {
    std::vector<int> order((size_t)n);
    for (int i = 0; i < n; ++i)
      order[(size_t)i] = i;
    std::sort(order.begin(), order.end(), [&](int p, int q) { return lo(p, ax) < lo(q, ax); });
    std::vector<int> cuts;
    double           reach = hi(order[0], ax);
    for (int k = 1; k < n; ++k)
    {
      if (lo(order[(size_t)k], ax) > reach + gap_tol)
        cuts.push_back(k);
      reach = std::max(reach, hi(order[(size_t)k], ax));
    }
    if (cuts.size() > best_cuts.size())
      best_axis = ax, best_order = order, best_cuts = cuts;
  }
  if (best_axis < 0 || best_cuts.empty())
    return false;
  // About 64 tools per slab, at most 32 slabs, cut at the gaps nearest to
  // equal counts.
  const int        slabs = std::min(32, std::max(2, (n + 63) / 64));
  std::vector<int> chosen;
  for (int s = 1; s < slabs; ++s)
  {
    const int want = s * n / slabs;
    auto      it   = std::lower_bound(best_cuts.begin(), best_cuts.end(), want);
    if (it == best_cuts.end())
      it = std::prev(it);
    if (it != best_cuts.begin() && std::abs(*std::prev(it) - want) < std::abs(*it - want))
      it = std::prev(it);
    if (chosen.empty() || *it > chosen.back())
      chosen.push_back(*it);
  }
  if (chosen.empty())
    return false;
  const int ax = best_axis;
  // The cut planes, halfway across each gap.
  gp_Dir dir(ax == 0 ? 1 : 0, ax == 1 ? 1 : 0, ax == 2 ? 1 : 0);
  std::vector<double> at;
  ShapeList           planes;
  const double        size = std::sqrt(tb.SquareExtent()) * 2 + 1;
  for (int c : chosen)
  {
    double reach = -1e300;
    for (int k = 0; k < c; ++k)
      reach = std::max(reach, hi(best_order[(size_t)k], ax));
    const double x = 0.5 * (reach + lo(best_order[(size_t)c], ax));
    at.push_back(x);
    gp_Pnt o(0, 0, 0);
    o.SetCoord(ax + 1, x);
    planes.Append(BRepBuilderAPI_MakeFace(gp_Pln(o, dir), -size, size, -size, size).Face());
  }
  const std::atomic<bool>* flag = t_cancel;
  BRepAlgoAPI_Splitter     sp;
  ShapeList                one;
  one.Append(target);
  sp.SetArguments(one);
  sp.SetTools(planes);
  sp.SetRunParallel(true);
  sp.SetNonDestructive(true);
  build_with(sp, flag);
  if (sp.HasErrors() || !sp.IsDone() || cancelled(flag))
    return false;
  HHist h1 = new BRepTools_History(one, sp);
  // The target's pieces and the tools by slab.
  const int                              nslab = (int)at.size() + 1;
  auto                                   slab_of = [&](double x) { return (int)(std::upper_bound(at.begin(), at.end(), x) - at.begin()); };
  std::vector<std::vector<TopoDS_Shape>> pieces((size_t)nslab), tools((size_t)nslab);
  for (TopExp_Explorer e(sp.Shape(), TopAbs_SOLID); e.More(); e.Next())
  {
    Bnd_Box b;
    BRepBndLib::Add(e.Current(), b, false);
    double x[6];
    b.Get(x[0], x[1], x[2], x[3], x[4], x[5]);
    pieces[(size_t)slab_of(0.5 * (x[ax] + x[3 + ax]))].push_back(e.Current());
  }
  for (int i = 0; i < n; ++i)
    tools[(size_t)slab_of(0.5 * (lo(i, ax) + hi(i, ax)))].push_back(t[(size_t)i]);
  // Each slab with its tools: glue when they only touch, else trimmed.
  std::vector<TopoDS_Shape> res((size_t)nslab);
  std::vector<HHist>        hs((size_t)nslab);
  std::vector<char>         failed((size_t)nslab, 0);
  OSD_Parallel::For(0, nslab, [&](int k) {
    const auto& p = pieces[(size_t)k];
    const auto& q = tools[(size_t)k];
    if (cancelled(flag) || (p.empty() && q.empty()))
    {
      failed[(size_t)k] = !(p.empty() && q.empty());
      return;
    }
    if (p.empty() || q.empty())
    {
      // Nothing to fuse: the pieces (or free tools) as they are.
      res[(size_t)k] = compound_of(p.empty() ? q : p);
      hs[(size_t)k]  = new BRepTools_History();
      return;
    }
    BRepAlgoAPI_Fuse f;
    f.SetArguments(to_list(p));
    f.SetTools(to_list(q));
    f.SetGlue(BOPAlgo_GlueShift);
    f.SetUseOBB(opts.obb);
    f.SetRunParallel(false);
    f.SetNonDestructive(true);
    f.SetToFillHistory(true);
    build_with(f, flag);
    if (cancelled(flag))
    {
      failed[(size_t)k] = 1;
      return;
    }
    if (!f.HasErrors() && f.IsDone() && !cancelled(flag) && volumes_add_up(concat(p, q), f.Shape()))
    {
      res[(size_t)k] = f.Shape();
      hs[(size_t)k]  = new BRepTools_History(to_list(concat(p, q)), f);
      return;
    }
    TopoDS_Shape r;
    HHist        h;
    if (fuse_trimmed(p, q, opts, r, h))
    {
      res[(size_t)k] = r;
      hs[(size_t)k]  = h;
    }
    else
      failed[(size_t)k] = 1;
  });
  for (char f : failed)
    if (f)
      return false;
  if (cancelled(flag))
    return false;
  std::vector<Stage> level2;
  std::vector<TopoDS_Shape> parts;
  for (int k = 0; k < nslab; ++k)
  {
    if (res[(size_t)k].IsNull())
      continue;
    level2.push_back(stage_of(concat(pieces[(size_t)k], tools[(size_t)k]), hs[(size_t)k]));
    parts.push_back(res[(size_t)k]);
  }
  if (parts.size() < 2)
    return false;
  // The slabs back together: they share the cut faces. (An abort is
  // checked between these steps: each takes ~0.1 s on a 2000-face part.)
  std::vector<TopoDS_Shape> shared;
  const TopoDS_Shape        joined = join_on_shared_faces(parts, shared);
  if (cancelled(flag))
    return false;
  if (!joined.IsNull() && volumes_add_up(concat({target}, t), joined))
  {
    if (cancelled(flag))
      return false;
    HHist h3 = new BRepTools_History();
    for (const TopoDS_Shape& f : shared)
      h3->Remove(f);
    std::vector<std::vector<Stage>> stages;
    stages.push_back({stage_of({target}, h1)});
    stages.push_back(std::move(level2));
    stages.push_back({stage_of(shared, h3)});
    hist = compose(concat({target}, t), stages);
    if (cancelled(flag))
      return false;
    out = unify(joined, hist);
    return true;
  }
  BRepAlgoAPI_Fuse g;
  g.SetArguments(to_list({parts[0]}));
  g.SetTools(to_list(std::vector<TopoDS_Shape>(parts.begin() + 1, parts.end())));
  g.SetGlue(BOPAlgo_GlueShift);
  g.SetRunParallel(true);
  g.SetNonDestructive(true);
  g.SetToFillHistory(true);
  build_cancellable(g);
  if (g.HasErrors() || !g.IsDone() || cancelled(flag) || !volumes_add_up(concat({target}, t), g.Shape()))
    return false;
  HHist h3 = new BRepTools_History(to_list(parts), g);
  std::vector<std::vector<Stage>> stages;
  stages.push_back({stage_of({target}, h1)});
  stages.push_back(std::move(level2));
  stages.push_back({stage_of(parts, h3)});
  hist = compose(concat({target}, t), stages);
  if (cancelled(flag))
    return false;
  // Without the cuts: unified (the caller's unify then finds nothing).
  out = unify(g.Shape(), hist);
  return true;
}

} // namespace

// Planes between groups of boxes for slab-wise work: along the axis where
// the boxes leave the most gaps, about `per_slab` boxes a slab (at most 32
// slabs), halfway across the gaps nearest to equal counts. False when the
// boxes leave no gap.
static bool slab_planes(const std::vector<Bnd_Box>& boxes, const Bnd_Box& whole, int per_slab, int& axis, std::vector<double>& at, ShapeList& planes)
{
  const int n = (int)boxes.size();
  auto      lo = [&](int i, int ax) {
    double x[6];
    boxes[(size_t)i].Get(x[0], x[1], x[2], x[3], x[4], x[5]);
    return x[ax];
  };
  auto hi = [&](int i, int ax) {
    double x[6];
    boxes[(size_t)i].Get(x[0], x[1], x[2], x[3], x[4], x[5]);
    return x[3 + ax];
  };
  int              best_axis = -1;
  std::vector<int> best_order, best_cuts;
  for (int ax = 0; ax < 3; ++ax)
  {
    std::vector<int> order((size_t)n);
    for (int i = 0; i < n; ++i)
      order[(size_t)i] = i;
    std::sort(order.begin(), order.end(), [&](int p, int q) { return lo(p, ax) < lo(q, ax); });
    std::vector<int> cuts;
    double           r = hi(order[0], ax);
    for (int k = 1; k < n; ++k)
    {
      if (lo(order[(size_t)k], ax) > r)
        cuts.push_back(k);
      r = std::max(r, hi(order[(size_t)k], ax));
    }
    if (cuts.size() > best_cuts.size())
      best_axis = ax, best_order = order, best_cuts = cuts;
  }
  if (best_axis < 0 || best_cuts.empty())
    return false;
  const int        slabs = std::min(32, std::max(2, (n + per_slab - 1) / per_slab));
  std::vector<int> chosen;
  for (int s = 1; s < slabs; ++s)
  {
    const int want = s * n / slabs;
    auto      it   = std::lower_bound(best_cuts.begin(), best_cuts.end(), want);
    if (it == best_cuts.end())
      it = std::prev(it);
    if (it != best_cuts.begin() && std::abs(*std::prev(it) - want) < std::abs(*it - want))
      it = std::prev(it);
    if (chosen.empty() || *it > chosen.back())
      chosen.push_back(*it);
  }
  axis = best_axis;
  gp_Dir       dir(axis == 0 ? 1 : 0, axis == 1 ? 1 : 0, axis == 2 ? 1 : 0);
  const double size = std::sqrt(whole.SquareExtent()) * 2 + 1;
  for (int c : chosen)
  {
    double r = -1e300;
    for (int k = 0; k < c; ++k)
      r = std::max(r, hi(best_order[(size_t)k], axis));
    const double x = 0.5 * (r + lo(best_order[(size_t)c], axis));
    at.push_back(x);
    gp_Pnt o(0, 0, 0);
    o.SetCoord(axis + 1, x);
    planes.Append(BRepBuilderAPI_MakeFace(gp_Pln(o, dir), -size, size, -size, size).Face());
  }
  return true;
}

// Fillets / chamfers of many edges far apart (400 hole rims of a plate) in
// slabs: the part cut by planes in the gaps between the edges' reach, each
// slab blended on its own (in parallel), the slabs joined on their shared
// cut faces and the cuts merged away. One OCCT blend of all the edges costs
// ~11 ms an edge on the plate (4.3 s for 400, Onshape 0.4 s): every edge's
// blend works against the whole of the long faces. False when it doesn't
// apply (few edges, no gaps, a slab failing); the caller then blends whole.
bool blend_in_slabs(const TopoDS_Shape& target, const std::vector<TopoDS_Edge>& edges, double reach, const SlabBlend& blend,
                    TopoDS_Shape& out, occ::handle<BRepTools_History>& hist)
{
  const int n = (int)edges.size();
  if (n < 48)
    return false;
  std::vector<Bnd_Box> boxes((size_t)n);
  for (int i = 0; i < n; ++i)
  {
    BRepBndLib::Add(edges[(size_t)i], boxes[(size_t)i], false);
    if (boxes[(size_t)i].IsVoid())
      return false;
    boxes[(size_t)i].Enlarge(reach);
  }
  Bnd_Box tb;
  BRepBndLib::Add(target, tb, false);
  if (tb.IsVoid())
    return false;
  int                 ax = 0;
  std::vector<double> at;
  ShapeList           planes;
  if (!slab_planes(boxes, tb, 24, ax, at, planes))
    return false;
  const std::atomic<bool>* flag = t_cancel;
  BRepAlgoAPI_Splitter     sp;
  ShapeList                one;
  one.Append(target);
  sp.SetArguments(one);
  sp.SetTools(planes);
  sp.SetRunParallel(true);
  sp.SetNonDestructive(true);
  build_with(sp, flag);
  if (sp.HasErrors() || !sp.IsDone() || cancelled(flag))
    return false;
  HHist                     h1 = new BRepTools_History(one, sp);
  std::vector<TopoDS_Shape> pieces;
  for (TopExp_Explorer e(sp.Shape(), TopAbs_SOLID); e.More(); e.Next())
    pieces.push_back(e.Current());
  std::vector<ShapeIMap> piece_shapes(pieces.size());
  for (size_t k = 0; k < pieces.size(); ++k)
    for (TopAbs_ShapeEnum t : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
      TopExp::MapShapes(pieces[k], t, piece_shapes[k]);
  // The image of a sub-shape of the part in piece k (null if none there).
  auto image_in = [&](size_t k, const TopoDS_Shape& x) {
    const ShapeList& m = h1->Modified(x);
    if (m.IsEmpty())
      return piece_shapes[k].Contains(x) ? x : TopoDS_Shape();
    for (ShapeList::Iterator it(m); it.More(); it.Next())
      if (piece_shapes[k].Contains(it.Value()))
        return it.Value();
    return TopoDS_Shape();
  };
  // Each edge to the piece holding it (whole: no plane crosses its reach).
  std::vector<std::vector<int>> mine(pieces.size());
  for (int i = 0; i < n; ++i)
  {
    if (h1->Modified(edges[(size_t)i]).Extent() > 1)
      return false;
    int owner = -1;
    for (size_t k = 0; k < pieces.size() && owner < 0; ++k)
      if (!image_in(k, edges[(size_t)i]).IsNull())
        owner = (int)k;
    if (owner < 0)
      return false;
    mine[(size_t)owner].push_back(i);
  }
  std::vector<TopoDS_Shape> res(pieces.size());
  std::vector<HHist>        hs(pieces.size());
  std::vector<char>         failed(pieces.size(), 0);
  OSD_Parallel::For(0, (int)pieces.size(), [&](int k) {
    if (cancelled(flag))
    {
      failed[(size_t)k] = 1;
      return;
    }
    if (mine[(size_t)k].empty())
    {
      res[(size_t)k] = pieces[(size_t)k];
      hs[(size_t)k]  = new BRepTools_History();
      return;
    }
    try
    {
      const auto image = [&](const TopoDS_Shape& x) { return image_in((size_t)k, x); };
      if (!blend(pieces[(size_t)k], mine[(size_t)k], image, res[(size_t)k], hs[(size_t)k]))
        failed[(size_t)k] = 1;
    }
    catch (...)
    {
      failed[(size_t)k] = 1;
    }
  });
  for (char f : failed)
    if (f)
      return false;
  std::vector<TopoDS_Shape> shared;
  const TopoDS_Shape        joined = join_on_shared_faces(res, shared);
  if (joined.IsNull())
    return false;
  std::vector<Stage> level2;
  for (size_t k = 0; k < pieces.size(); ++k)
    level2.push_back(stage_of({pieces[k]}, hs[k]));
  HHist h3 = new BRepTools_History();
  for (const TopoDS_Shape& f : shared)
    h3->Remove(f);
  std::vector<std::vector<Stage>> stages;
  stages.push_back({stage_of({target}, h1)});
  stages.push_back(std::move(level2));
  stages.push_back({stage_of(shared, h3)});
  HHist h = compose({target}, stages);
  if (cancelled(flag))
    return false;
  const TopoDS_Shape r = unify(joined, h);
  // The whole joined part (806 faces for 400 holes): checked in parallel.
  if (!BRepCheck_Analyzer(r, true, true).IsValid())
    return false;
  out  = r;
  hist = h;
  return true;
}

// A cut by many tools apart from each other (400 holes through a plate) in
// slabs: the target cut by planes in the gaps between the tools, each slab
// cut by its own tools in parallel, the slabs joined on their shared cut
// faces and unified. One cut of all the tools is superlinear: the target's
// long faces collect every tool's intersections (400 holes: 0.9 s, in 16
// slabs ~0.3 s). False when it doesn't apply.
static bool cut_slabs(const TopoDS_Shape& target, const std::vector<TopoDS_Shape>& t, const BoolOpts& opts, TopoDS_Shape& out, HHist& hist)
{
  const int n = (int)t.size();
  if (n < 64)
    return false;
  Bnd_Box tb;
  BRepBndLib::Add(target, tb, false);
  if (tb.IsVoid())
    return false;
  const double         gap = 1e-6 * std::sqrt(tb.SquareExtent());
  std::vector<Bnd_Box> boxes((size_t)n);
  for (int i = 0; i < n; ++i)
  {
    BRepBndLib::Add(t[(size_t)i], boxes[(size_t)i], false);
    if (boxes[(size_t)i].IsVoid())
      return false;
    boxes[(size_t)i].Enlarge(gap);
  }
  int                 ax = 0;
  std::vector<double> at;
  ShapeList           planes;
  if (!slab_planes(boxes, tb, 25, ax, at, planes))
    return false;
  const std::atomic<bool>* flag = t_cancel;
  BRepAlgoAPI_Splitter     sp;
  ShapeList                one;
  one.Append(target);
  sp.SetArguments(one);
  sp.SetTools(planes);
  sp.SetRunParallel(true);
  sp.SetNonDestructive(true);
  build_with(sp, flag);
  if (sp.HasErrors() || !sp.IsDone() || cancelled(flag))
    return false;
  HHist      h1      = new BRepTools_History(one, sp);
  const int  nslab   = (int)at.size() + 1;
  auto       slab_of = [&](double x) { return (int)(std::upper_bound(at.begin(), at.end(), x) - at.begin()); };
  auto       mid     = [&](const Bnd_Box& b) {
    double x[6];
    b.Get(x[0], x[1], x[2], x[3], x[4], x[5]);
    return 0.5 * (x[ax] + x[3 + ax]);
  };
  std::vector<std::vector<TopoDS_Shape>> pieces((size_t)nslab), tools((size_t)nslab);
  for (TopExp_Explorer e(sp.Shape(), TopAbs_SOLID); e.More(); e.Next())
  {
    Bnd_Box b;
    BRepBndLib::Add(e.Current(), b, false);
    pieces[(size_t)slab_of(mid(b))].push_back(e.Current());
  }
  for (int i = 0; i < n; ++i)
    tools[(size_t)slab_of(mid(boxes[(size_t)i]))].push_back(t[(size_t)i]);
  std::vector<TopoDS_Shape> res((size_t)nslab);
  std::vector<HHist>        hs((size_t)nslab);
  std::vector<char>         failed((size_t)nslab, 0);
  OSD_Parallel::For(0, nslab, [&](int k) {
    const auto& p = pieces[(size_t)k];
    const auto& q = tools[(size_t)k];
    if (cancelled(flag))
    {
      failed[(size_t)k] = 1;
      return;
    }
    if (p.empty())
      return;
    if (q.empty())
    {
      res[(size_t)k] = compound_of(p);
      hs[(size_t)k]  = new BRepTools_History();
      return;
    }
    BRepAlgoAPI_Cut c;
    c.SetArguments(to_list(p));
    c.SetTools(to_list(q));
    c.SetUseOBB(opts.obb);
    c.SetRunParallel(false);
    c.SetNonDestructive(true);
    c.SetToFillHistory(true);
    build_with(c, flag);
    if (c.HasErrors() || !c.IsDone())
    {
      failed[(size_t)k] = 1;
      return;
    }
    res[(size_t)k] = c.Shape();
    hs[(size_t)k]  = new BRepTools_History(to_list(concat(p, q)), c);
  });
  for (char f : failed)
    if (f)
      return false;
  std::vector<Stage>        level2;
  std::vector<TopoDS_Shape> parts;
  for (int k = 0; k < nslab; ++k)
  {
    if (res[(size_t)k].IsNull())
      continue;
    level2.push_back(stage_of(concat(pieces[(size_t)k], tools[(size_t)k]), hs[(size_t)k]));
    parts.push_back(res[(size_t)k]);
  }
  std::vector<TopoDS_Shape> shared;
  const TopoDS_Shape        joined = parts.size() < 2 ? TopoDS_Shape() : join_on_shared_faces(parts, shared);
  if (joined.IsNull())
    return false;
  HHist h3 = new BRepTools_History();
  for (const TopoDS_Shape& f : shared)
    h3->Remove(f);
  std::vector<std::vector<Stage>> stages;
  stages.push_back({stage_of({target}, h1)});
  stages.push_back(std::move(level2));
  stages.push_back({stage_of(shared, h3)});
  HHist h = compose(concat({target}, t), stages);
  if (cancelled(flag))
    return false;
  const TopoDS_Shape r = unify(joined, h);
  // The whole joined part (406 faces for 400 holes): checked in parallel.
  if (!BRepCheck_Analyzer(r, true, true).IsValid())
    return false;
  out  = r;
  hist = h;
  return true;
}

// The result's facets back as mesh faces; `hist` becomes the history from
// the original (unfaceted) inputs.
static TopoDS_Shape remesh_facets(const TopoDS_Shape& res, MeshFacets& facets, HHist& hist)
{
  if (facets.source.empty())
    return res;
  occ::handle<BRepTools_History> back;
  const TopoDS_Shape             out = unfacet_mesh(res, facets, back);
  const occ::handle<BRepTools_History> h = facets.to_facets;
  h->Merge(hist);
  if (!back.IsNull())
    h->Merge(back);
  hist = h;
  return out;
}

void set_thread_cancel(size_t flag)
{
  t_cancel = reinterpret_cast<const std::atomic<bool>*>(flag);
}

OpOut op_boolean(uint8_t op, const OcShapeList& args, const OcShapeList& tools,
                 const BoolOpts& opts)
{
  if (args.items.empty())
    fail_invalid("boolean needs at least one argument");
  if (tools.items.empty())
    fail_invalid("boolean needs at least one tool");
  check_all(args.items, "boolean argument");
  check_all(tools.items, "boolean tool");
  auto make = [op]() -> std::unique_ptr<BRepAlgoAPI_BooleanOperation> {
    switch (op)
    {
      case 0:
        return std::make_unique<BRepAlgoAPI_Fuse>();
      case 1:
        return std::make_unique<BRepAlgoAPI_Cut>();
      case 2:
        return std::make_unique<BRepAlgoAPI_Common>();
      default:
        fail_invalid("unknown boolean operation");
    }
  };
  std::unique_ptr<BRepAlgoAPI_BooleanOperation> b = make();
  // Mesh bodies take part as planar facets (see facet_mesh).
  MeshFacets                facets;
  std::vector<TopoDS_Shape> a, t;
  for (const TopoDS_Shape& x : args.items)
    a.push_back(facet_mesh(x, facets));
  for (const TopoDS_Shape& x : tools.items)
    t.push_back(facet_mesh(x, facets));
  // Many touching tools into one solid (patterns): in slabs.
  if (op == 0 && opts.glue == 3 && a.size() == 1 && facets.source.empty())
  {
    TopoDS_Shape out;
    HHist        h;
    const bool   done = fuse_slabs(a[0], t, opts, out, h);
    fail_if_cancelled();
    if (done)
      return make_result(concat(args.items, tools.items), out, h.get());
  }
  // Many tools apart from each other cut out of one solid (holes): in slabs.
  if (op == 1 && opts.unify && a.size() == 1 && facets.source.empty())
  {
    TopoDS_Shape out;
    HHist        h;
    const bool   done = cut_slabs(a[0], t, opts, out, h);
    fail_if_cancelled();
    if (done)
      return make_result(concat(args.items, tools.items), out, h.get());
  }
  b->SetArguments(to_list(a));
  b->SetTools(to_list(t));
  BoolOpts first = opts;
  if (first.glue == 3 && op != 0) // glue is for fusing touching shapes only
    first.glue = 0;
  apply_opts(*b, first);
  build_cancellable(*b);
  fail_if_cancelled();
  // Checked glue (patterns): GlueShift is only right when the arguments
  // merely touch; then the union's volume is the sum of theirs. Tools that
  // reach into the arguments: trimmed first (fuse_trimmed). Otherwise (a
  // cut, overlapping tools) the boolean runs again without glue.
  HHist        hist;
  TopoDS_Shape res;
  if (first.glue == 3 && (b->HasErrors() || !b->IsDone() || !volumes_add_up(concat(a, t), b->Shape())))
  {
    const bool trimmed = t.size() > 1 && fuse_trimmed(a, t, opts, res, hist);
    fail_if_cancelled();
    if (trimmed)
    {
      res  = remesh_facets(opts.unify ? unify(res, hist) : res, facets, hist);
      return make_result(concat(args.items, tools.items), res, hist.get());
    }
    else
    {
      BoolOpts plain = opts;
      plain.glue     = 0;
      b              = make();
      b->SetArguments(to_list(a));
      b->SetTools(to_list(t));
      apply_opts(*b, plain);
      build_cancellable(*b);
    }
  }
  fail_if_cancelled();
  if (b->HasErrors() || !b->IsDone())
    fail_op("boolean failed: " + algo_errors(*b));
  hist = new BRepTools_History(to_list(concat(a, t)), *b);
  res  = b->Shape();
  fail_if_cancelled();
  if (opts.unify)
    res = unify(res, hist);
  res = remesh_facets(res, facets, hist);
  return make_result(concat(args.items, tools.items), res, hist.get());
}

OpOut op_split(const OcShapeList& args, const OcShapeList& tools, const BoolOpts& opts)
{
  if (args.items.empty())
    fail_invalid("split needs at least one argument");
  if (tools.items.empty())
    fail_invalid("split needs at least one tool");
  check_all(args.items, "split argument");
  check_all(tools.items, "split tool");
  MeshFacets                facets;
  std::vector<TopoDS_Shape> a, t;
  for (const TopoDS_Shape& x : args.items)
    a.push_back(facet_mesh(x, facets));
  for (const TopoDS_Shape& x : tools.items)
    t.push_back(facet_mesh(x, facets));
  BRepAlgoAPI_Splitter sp;
  sp.SetArguments(to_list(a));
  sp.SetTools(to_list(t));
  apply_opts(sp, opts);
  sp.Build();
  if (sp.HasErrors() || !sp.IsDone())
    fail_op("split failed: " + algo_errors(sp));
  HHist        hist = new BRepTools_History(to_list(concat(a, t)), sp);
  TopoDS_Shape res  = sp.Shape();
  if (opts.unify)
    res = unify(res, hist);
  res = remesh_facets(res, facets, hist);
  return make_result(concat(args.items, tools.items), res, hist.get());
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
