// Booleans, split, unify, enclose, transforms, sections, projections.
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <string>
#if defined(_WIN32)
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <psapi.h>
#ifdef _MSC_VER
#pragma comment(lib, "psapi.lib")
#endif
#elif defined(__APPLE__)
#include <mach/mach.h>
#include <sys/sysctl.h>
#else
#include <unistd.h>
#endif
#if defined(__linux__) && defined(__GLIBC__)
#include <malloc.h>
#endif
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
#include <BOPTools_AlgoTools.hxx>
#include <IntCurvesFace_ShapeIntersector.hxx>
#include <IntTools_Context.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepGProp_Face.hxx>
#include <BOPTools_AlgoTools3D.hxx>
#include <BOPAlgo_Builder.hxx>
#include <TopAbs.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Lin.hxx>
#include <ElSLib.hxx>
#include <BRepTools_History.hxx>
#include <BRepTools.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepTools_ReShape.hxx>
#include <Geom_Circle.hxx>
#include <Geom2d_Line.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRep_Builder.hxx>
#include <TColStd_PackedMapOfInteger.hxx>
#include <NCollection_DataMap.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <deque>
#include <mutex>
#include <memory>
#include <algorithm>
#include <TopoDS_Compound.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <Geom_ConicalSurface.hxx>
#include <BRepExtrema_SelfIntersection.hxx>
#include <TopoDS_Wire.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <functional>
#include <map>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <atomic>

namespace noshape {

// The machine's physical memory in MB (0 when unknown).
static uint64_t physical_memory_mb()
{
#if defined(_WIN32)
  MEMORYSTATUSEX m;
  m.dwLength = sizeof m;
  return GlobalMemoryStatusEx(&m) ? (uint64_t)(m.ullTotalPhys >> 20) : 0;
#elif defined(__APPLE__)
  uint64_t size = 0;
  size_t   len  = sizeof size;
  return sysctlbyname("hw.memsize", &size, &len, nullptr, 0) == 0 ? size >> 20 : 0;
#else
  const long pages = sysconf(_SC_PHYS_PAGES), page = sysconf(_SC_PAGESIZE);
  return pages > 0 && page > 0 ? ((uint64_t)pages * (uint64_t)page) >> 20 : 0;
#endif
}

// The process's resident memory in bytes (0 when unknown).
static size_t resident_bytes()
{
#if defined(_WIN32)
  PROCESS_MEMORY_COUNTERS c;
  return GetProcessMemoryInfo(GetCurrentProcess(), &c, sizeof c) ? (size_t)c.WorkingSetSize : 0;
#elif defined(__APPLE__)
  mach_task_basic_info_data_t info;
  mach_msg_type_number_t      n = MACH_TASK_BASIC_INFO_COUNT;
  return task_info(mach_task_self(), MACH_TASK_BASIC_INFO, (task_info_t)&info, &n) == KERN_SUCCESS ? (size_t)info.resident_size : 0;
#else
  FILE* f = std::fopen("/proc/self/statm", "r");
  if (f == nullptr)
    return 0;
  long size = 0, resident = 0;
  const int got = std::fscanf(f, "%ld %ld", &size, &resident);
  std::fclose(f);
  return got == 2 ? (size_t)resident * (size_t)sysconf(_SC_PAGESIZE) : 0;
#endif
}

namespace {

using HHist = occ::handle<BRepTools_History>;

// The thread's cancel flag (set_thread_cancel): a Rust AtomicBool, read by
// the booleans' progress indicator so a stale regeneration stops inside OCCT.
thread_local const std::atomic<bool>* t_cancel = nullptr;

bool cancelled(const std::atomic<bool>* flag)
{
  return flag != nullptr && flag->load(std::memory_order_relaxed);
}

// Memory budget of one boolean (lead 10-25, the user's Diamond knurl: 148
// crossing helical tools took 17 GB and ran on): the process's resident
// memory may grow by this much while an operation builds, then the
// operation stops with an error. Set from Rust before each boolean
// (NOSHAPE_BOOLEAN_MEMORY_MB, default 4096: std::getenv on Windows reads the
// CRT's copy of the environment and misses changes made after startup).
std::atomic<uint64_t> g_memory_budget_mb{8192};

size_t memory_budget() { return (size_t)g_memory_budget_mb.load() << 20; }

// Set when an operation ran out of its budget (also on a pool thread); the
// operation then fails (fail_if_cancelled) instead of trying another way.
std::atomic<bool> g_over_budget{false};

// Stops a build when the thread's cancel flag is set or when the process
// grew past the memory budget since the build started (checked every 50
// ms). It shows nothing.
class CancelIndicator : public Message_ProgressIndicator
{
public:
  explicit CancelIndicator(const std::atomic<bool>* flag) : myFlag(flag), myStart(resident_bytes()) {}
  bool UserBreak() override
  {
    if (cancelled(myFlag) || myOver)
      return true;
    if (myDeadline != nullptr && steady_seconds() > myDeadline->at)
    {
      myDeadline->hit.store(true);
      return true;
    }
    const auto now = std::chrono::steady_clock::now();
    if (now - myChecked < std::chrono::milliseconds(50))
      return false;
    myChecked = now;
    const size_t rss = resident_bytes();
    if (rss > myStart && rss - myStart > memory_budget())
    {
      myOver = true;
      g_over_budget.store(true);
    }
    return myOver;
  }
  void Show(const Message_ProgressScope&, const bool) override {}

private:
  const std::atomic<bool>*              myFlag;
  Deadline*                             myDeadline = t_deadline;
  size_t                                myStart;
  bool                                  myOver = false;
  std::chrono::steady_clock::time_point myChecked = std::chrono::steady_clock::now();
};

// Build `algo`, stoppable through the thread's cancel flag and the memory
// budget.
// OCCT patch 0001 (BOPAlgo_Builder_3.cxx): the next boolean on this thread,
// if a cut of solids, is built from its faces' classification without the
// split solids of the target and the tools.
extern "C" void noshape_bop_cut_by_faces(bool on);

// Sets it for one scope (cleared on any exit, so later booleans run as usual).
struct CutByFaces
{
  explicit CutByFaces(bool on) { noshape_bop_cut_by_faces(on); }
  ~CutByFaces() { noshape_bop_cut_by_faces(false); }
  CutByFaces(const CutByFaces&)            = delete;
  CutByFaces& operator=(const CutByFaces&) = delete;
};

template <class A> void build_cancellable(A& algo)
{
  occ::handle<CancelIndicator> pi = new CancelIndicator(t_cancel);
  algo.Build(pi->Start());
}

// Build `algo`, stoppable through `flag` (a pool thread has no thread flag)
// and the memory budget; never throws (the caller checks the flags).
template <class A> void build_with(A& algo, const std::atomic<bool>* flag)
{
  occ::handle<CancelIndicator> pi = new CancelIndicator(flag);
  algo.Build(pi->Start());
}

void fail_if_cancelled()
{
  if (cancelled(t_cancel))
    fail_op("cancelled");
  if (g_over_budget.load())
    fail_op("operation stopped: it needed more than " + std::to_string(memory_budget() >> 20) + " MB of memory");
}

void apply_opts(BRepAlgoAPI_BuilderAlgo& a, const BoolOpts& o)
{
  a.SetRunParallel(o.parallel && bop_parallel());
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


// NOSHAPE_UNIFY_LOCAL=0: the whole shape always.
bool unify_local_on()
{
  static const bool on = [] {
    const char* v = std::getenv("NOSHAPE_UNIFY_LOCAL");
    return v == nullptr || std::string(v) != "0";
  }();
  return on;
}

// Whether two adjacent faces lie on one surface, or two edges meeting at a
// vertex of only those two lie on one line, i.e. whether
// ShapeUpgrade_UnifySameDomain could change anything. Linear in the shape;
// unifying a shape it would leave unchanged costs seconds on large results
// (a part with 1000 patterned ribs: 14 s). With a scope: only there.
bool same_domain_candidates(const TopoDS_Shape& s, const UnifyScope* sc = nullptr)
{
  const TopoDS_Shape& where = sc != nullptr ? TopoDS_Shape(sc->faces) : s;
  ShapeAncMap         ef;
  unique_ancestors(where, TopAbs_EDGE, TopAbs_FACE, ef);
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
  unique_ancestors(where, TopAbs_VERTEX, TopAbs_EDGE, ve);
  for (int i = 1; i <= ve.Extent(); ++i)
  {
    const ShapeList& es = ve(i);
    if (es.Extent() != 2 || (sc != nullptr && !sc->seeds.Contains(ve.FindKey(i))))
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
TopoDS_Shape merge_planar(const TopoDS_Shape& s, HHist& hist, const UnifyScope* sc = nullptr, UnifyScope* next = nullptr)
{
  // With a scope: its faces only (the groups are there), its seeds' edges.
  const TopoDS_Shape& where = sc != nullptr ? TopoDS_Shape(sc->faces) : s;
  std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("merge.maps");
  ShapeIMap           faces;
  TopExp::MapShapes(where, TopAbs_FACE, faces);
  const int           nf = faces.Extent();
  std::vector<char>   planar((size_t)nf + 1, 0);
  std::vector<gp_Pln> pl((size_t)nf + 1);
  for (int i = 1; i <= nf; ++i)
    planar[(size_t)i] = outer_plane(TopoDS::Face(faces(i)), pl[(size_t)i]);
  ShapeAncMap ef;
  unique_ancestors(where, TopAbs_EDGE, TopAbs_FACE, ef);
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
  lap = std::make_unique<KernelTimer>("merge.groups");
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape();
  // As UnifySameDomain records it: every merged face modified into the new
  // one, inner edges and vertices removed.
  HHist                          h = new BRepTools_History();
  std::vector<TopoDS_Shape>      gone; // inner edges and vertices of merged groups
  bool                           any = false;
  // Face -> the merged face it became (null: merged into another's).
  NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher> merged_into;
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
    // The merged face on the surface of the group's face with the most edges
    // (its edges' pcurves are there already: the 400-fin plate's front face
    // has 800), in its orientation.
    int lead = g[0], most = -1;
    for (int fi : g)
    {
      int n = 0;
      for (TopExp_Explorer ex(faces(fi), TopAbs_EDGE); ex.More(); ex.Next())
        ++n;
      if (n > most)
        most = n, lead = fi;
    }
    const TopoDS_Face& f0  = TopoDS::Face(faces(lead));
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
      {
        double f, l;
        bool   stored = false;
        BRep_Tool::CurveOnSurface(e, surf, loc, f, l, &stored);
        if (!stored)
          BRepLib::BuildPCurveForEdgeOnPlane(e, nf_);
      }
    if (rev)
      nf_.Reverse();
    rs->Replace(faces(g[0]), nf_);
    for (size_t i = 1; i < g.size(); ++i)
      rs->Remove(faces(g[i]));
    merged_into.Bind(faces(g[0]), nf_);
    for (size_t i = 1; i < g.size(); ++i)
      merged_into.Bind(faces(g[i]), TopoDS_Shape());
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
  lap            = std::make_unique<KernelTimer>("merge.rebuild");
  TopoDS_Shape r = s;
  // One solid of one shell: the shell rebuilt with the merged faces (ReShape
  // copied the whole part: most of the time on the 400-fin plate).
  TopoDS_Shape solid, shell;
  int          nsolid = 0, nshell = 0;
  for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next(), ++nsolid)
    solid = e.Current();
  if (nsolid == 1 && s.ShapeType() == TopAbs_SOLID)
    for (TopoDS_Iterator it(solid); it.More(); it.Next(), ++nshell)
      shell = it.Value();
  if (any && nshell == 1 && shell.ShapeType() == TopAbs_SHELL)
  {
    TopoDS_Shell ns;
    B.MakeShell(ns);
    for (TopoDS_Iterator it(shell); it.More(); it.Next())
    {
      const TopoDS_Shape* m = merged_into.Seek(it.Value());
      if (m == nullptr)
        B.Add(ns, it.Value());
      else if (!m->IsNull())
        B.Add(ns, *m);
    }
    ns.Closed(shell.Closed());
    TopoDS_Solid so;
    B.MakeSolid(so);
    B.Add(so, ns.Oriented(shell.Orientation()));
    so.Orientation(solid.Orientation());
    r = so;
    for (const TopoDS_Shape& x : gone)
      if (!h->IsRemoved(x))
        h->Remove(x);
    if (hist.IsNull())
      hist = new BRepTools_History();
    hist->Merge(h);
  }
  else if (any)
  {
    r = rs->Apply(s);
    for (const TopoDS_Shape& x : gone)
      if (!h->IsRemoved(x))
        h->Remove(x);
    if (hist.IsNull())
      hist = new BRepTools_History();
    hist->Merge(h);
  }
  // The scope's faces as they are now, and its seeds with the vertices of
  // the inner edges gone.
  TopoDS_Compound where2;
  ShapeIMap       seeds2;
  if (sc != nullptr)
  {
    B.MakeCompound(where2);
    for (TopoDS_Iterator it(sc->faces); it.More(); it.Next())
    {
      const TopoDS_Shape* m = merged_into.Seek(it.Value());
      if (m == nullptr)
        B.Add(where2, it.Value());
      else if (!m->IsNull())
        B.Add(where2, m->Oriented(it.Value().Orientation()));
    }
    seeds2 = sc->seeds;
    for (const TopoDS_Shape& x : gone)
      if (x.ShapeType() == TopAbs_EDGE)
        for (TopExp_Explorer v(x, TopAbs_VERTEX); v.More(); v.Next())
          seeds2.Add(v.Current());
    if (next != nullptr)
    {
      next->faces = where2;
      next->seeds = seeds2;
    }
  }
  // Collinear straight edges through vertices of only those two.
  lap = std::make_unique<KernelTimer>("merge.edges");
  if (sc != nullptr && std::getenv("NOSHAPE_BOOL_DEBUG"))
  {
    int n = 0;
    for (TopoDS_Iterator it(where2); it.More(); it.Next())
      ++n;
    std::fprintf(stderr, "unify scope: %d faces, %d seeds\n", n, seeds2.Extent());
  }
  ShapeAncMap ve, ef2;
  unique_ancestors(sc != nullptr ? TopoDS_Shape(where2) : r, TopAbs_VERTEX, TopAbs_EDGE, ve);
  unique_ancestors(sc != nullptr ? TopoDS_Shape(where2) : r, TopAbs_EDGE, TopAbs_FACE, ef2);
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
    if (es.Extent() != 2 || (sc != nullptr && !seeds2.Contains(ve.FindKey(i))))
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
  if (sc != nullptr && next != nullptr)
  {
    // The scope's faces rebuilt around the new edges; the new edges' ends.
    const occ::handle<BRepTools_History> rebuilt = rs2->History();
    ShapeIMap                            now;
    for (TopoDS_Iterator it(where2); it.More(); it.Next())
    {
      const ShapeList& m = rebuilt->Modified(it.Value());
      now.Add(m.IsEmpty() ? it.Value() : m.First().Oriented(it.Value().Orientation()));
    }
    BRep_Builder B2;
    B2.MakeCompound(next->faces);
    for (int i = 1; i <= now.Extent(); ++i)
      B2.Add(next->faces, now(i));
    for (const auto& [c, members] : chains)
      for (int m : members)
        for (TopExp_Explorer v(edges(m + 1), TopAbs_VERTEX); v.More(); v.Next())
          next->seeds.Add(v.Current());
  }
  return r2;
}

// Optional ShapeUpgrade_UnifySameDomain post-processing, composing history
// (skipped when it could change nothing; planar faces and straight edges
// merged first by merge_planar).
TopoDS_Shape unify_in(const TopoDS_Shape& s0, HHist& hist, const UnifyScope* sc, bool* clean);

// Whether two adjacent faces on one kind of periodic surface (cylinders,
// cones, ...) could be merged: there USD's result can depend on where their
// wires start (below).
bool round_candidates(const TopoDS_Shape& s, const UnifyScope* sc)
{
  const TopoDS_Shape& where = sc != nullptr ? TopoDS_Shape(sc->faces) : s;
  ShapeAncMap         ef;
  unique_ancestors(where, TopAbs_EDGE, TopAbs_FACE, ef);
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const ShapeList& fs = ef(i);
    if (fs.Extent() != 2)
      continue;
    const TopoDS_Face a = TopoDS::Face(fs.First()), b = TopoDS::Face(fs.Last());
    TopLoc_Location   la, lb;
    const occ::handle<Geom_Surface> ga = BRep_Tool::Surface(a, la), gb = BRep_Tool::Surface(b, lb);
    if (!ga.IsNull() && !gb.IsNull() && ga->DynamicType() == gb->DynamicType() && (ga->IsUPeriodic() || ga->IsVPeriodic()))
      return true;
  }
  return false;
}

// `s` with each wire of its faces on a periodic surface listed from its k-th
// edge on (the history of the change in `h`); null when no wire is that long.
TopoDS_Shape rotate_round_wires(const TopoDS_Shape& s, int k, HHist& h)
{
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape();
  BRep_Builder                   bb;
  bool                           any = false;
  for (TopExp_Explorer fx(s, TopAbs_FACE); fx.More(); fx.Next())
  {
    const TopoDS_Face&              f = TopoDS::Face(fx.Current());
    TopLoc_Location                 l;
    const occ::handle<Geom_Surface> g = BRep_Tool::Surface(f, l);
    if (g.IsNull() || !(g->IsUPeriodic() || g->IsVPeriodic()))
      continue;
    for (TopoDS_Iterator w(f); w.More(); w.Next())
    {
      std::vector<TopoDS_Shape> es;
      for (TopoDS_Iterator e(w.Value(), false, false); e.More(); e.Next())
        es.push_back(e.Value());
      if ((int)es.size() <= k)
        continue;
      std::rotate(es.begin(), es.begin() + k, es.end());
      TopoDS_Wire nw;
      bb.MakeWire(nw);
      for (const TopoDS_Shape& e : es)
        bb.Add(nw, e);
      nw.Closed(w.Value().Closed());
      rs->Replace(w.Value().Oriented(TopAbs_FORWARD), nw);
      any = true;
    }
  }
  if (!any)
    return TopoDS_Shape();
  const TopoDS_Shape r = rs->Apply(s);
  h                    = rs->History();
  return r;
}

// ShapeUpgrade_UnifySameDomain where faces on one periodic surface would
// merge. Its result there depends on where their wires start (a notched
// cylinder fused with its mirror: 3 of 8 starts of one seamed wire, OCCT's own
// among them, gave a solid of 2/3 its volume, a merged face more than a turn
// round; a half pocket mirrored onto itself: 2 of 4 starts of a half cylinder's
// wire lost 2/3 of the pocket), and it changes its input's shared edges in
// place: qa's RUST-MIRROR-NOTCH and RUST-HALF-POCKET-3 (the Rust Boolean's wires
// start elsewhere than OCCT's). So: on a copy, the volume kept, else again with
// those wires started further on; none keeping it: not unified.
// `s` with the sub-shapes of its compounds, solids and shells in reverse
// order (the same faces).
TopoDS_Shape faces_reversed(const TopoDS_Shape& s)
{
  if (s.ShapeType() == TopAbs_FACE || s.ShapeType() > TopAbs_FACE)
    return s;
  std::vector<TopoDS_Shape> kids;
  for (TopoDS_Iterator it(s, false, false); it.More(); it.Next())
    kids.push_back(it.Value());
  TopoDS_Shape r = s.EmptyCopied();
  BRep_Builder B;
  for (auto x = kids.rbegin(); x != kids.rend(); ++x)
    B.Add(r, faces_reversed(*x));
  r.Closed(s.Closed());
  return r;
}

TopoDS_Shape usd_kept_volume(const TopoDS_Shape& s, HHist& hist)
{
  GProp_GProps g0;
  BRepGProp::VolumeProperties(s, g0);
  const double v0 = g0.Mass();
  // USD's result depends on the order of the faces too (a notched cylinder
  // fused with its mirror, qa OCCT-MIRROR-OVERCOUNT: one order of the same
  // faces merged to 4, the other gave 4/3 the volume from every start).
  for (int k = 0; k < 16; ++k)
  {
    BRepBuilderAPI_Copy cp(s, false, false);
    TopoDS_Shape        in = cp.Shape();
    HHist               hc = new BRepTools_History(to_list(std::vector<TopoDS_Shape>{s}), cp);
    HHist               hr;
    if (k >= 8)
      in = faces_reversed(in);
    if (k % 8 > 0)
    {
      in = rotate_round_wires(in, k % 8, hr);
      if (in.IsNull())
      {
        if (k >= 8)
          break;
        k = 7;
        continue;
      }
    }
    ShapeUpgrade_UnifySameDomain u(in, true, true, false);
    u.Build();
    GProp_GProps g1;
    BRepGProp::VolumeProperties(u.Shape(), g1);
    if (std::abs(g1.Mass() - v0) > 1e-9 * std::abs(v0))
      continue;
    if (hist.IsNull())
      hist = new BRepTools_History();
    hist->Merge(hc);
    if (!hr.IsNull())
      hist->Merge(hr);
    if (!u.History().IsNull())
      hist->Merge(u.History());
    return u.Shape();
  }
  // No start keeps it (two half cones meeting at their apex: a blind hole's
  // drill tip mirrored onto itself, qa's MIRROR-BLIND-HOLE-FACES; every start
  // gave an invalid solid, and with it the box's planes stayed split too):
  // the rest unified, the faces on cones with an apex kept apart, else every
  // pair on one kind of periodic surface.
  for (int pass = 0; pass < 2; ++pass)
  {
    BRepBuilderAPI_Copy cp(s, false, false);
    const TopoDS_Shape  in = cp.Shape();
    HHist               hc = new BRepTools_History(to_list(std::vector<TopoDS_Shape>{s}), cp);
    ShapeUpgrade_UnifySameDomain u(in, true, true, false);
    ShapeAncMap                  ef;
    unique_ancestors(in, TopAbs_EDGE, TopAbs_FACE, ef);
    bool kept = false;
    for (int i = 1; i <= ef.Extent(); ++i)
    {
      const ShapeList& fs = ef(i);
      if (fs.Extent() != 2)
        continue;
      const TopoDS_Face a = TopoDS::Face(fs.First()), b = TopoDS::Face(fs.Last());
      TopLoc_Location   la, lb;
      const occ::handle<Geom_Surface> ga = BRep_Tool::Surface(a, la), gb = BRep_Tool::Surface(b, lb);
      if (ga.IsNull() || gb.IsNull() || ga->DynamicType() != gb->DynamicType() || !(ga->IsUPeriodic() || ga->IsVPeriodic()))
        continue;
      auto apex = [](const TopoDS_Face& f) {
        for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
          if (BRep_Tool::Degenerated(TopoDS::Edge(x.Current())))
            return true;
        return false;
      };
      if (pass == 0 && (BRepAdaptor_Surface(a, false).GetType() != GeomAbs_Cone || !(apex(a) || apex(b))))
        continue;
      u.KeepShape(ef.FindKey(i));
      kept = true;
    }
    if (!kept)
      continue;
    u.Build();
    GProp_GProps g1;
    BRepGProp::VolumeProperties(u.Shape(), g1);
    if (std::abs(g1.Mass() - v0) > 1e-9 * std::abs(v0))
      continue;
    if (hist.IsNull())
      hist = new BRepTools_History();
    hist->Merge(hc);
    if (!u.History().IsNull())
      hist->Merge(u.History());
    return u.Shape();
  }
  return s;
}

// NOSHAPE_UNIFY_CHECK=1: a scoped unify is checked against the whole one
// (counts and volume; tests).
void check_scoped_unify(const TopoDS_Shape& s0, const TopoDS_Shape& got)
{
  static const bool on = std::getenv("NOSHAPE_UNIFY_CHECK") != nullptr;
  if (!on)
    return;
  HHist              h;
  const TopoDS_Shape want = unify_in(s0, h, nullptr, nullptr);
  int                n[2][3] = {};
  for (int k = 0; k < 2; ++k)
    for (int j = 0; j < 3; ++j)
    {
      ShapeIMap m;
      TopExp::MapShapes(k == 0 ? got : want, j == 0 ? TopAbs_FACE : j == 1 ? TopAbs_EDGE : TopAbs_VERTEX, m);
      n[k][j] = m.Extent();
    }
  GProp_GProps a, b;
  BRepGProp::VolumeProperties(got, a);
  BRepGProp::VolumeProperties(want, b);
  if (n[0][0] != n[1][0] || n[0][1] != n[1][1] || n[0][2] != n[1][2] || std::abs(a.Mass() - b.Mass()) > 1e-9 * std::abs(b.Mass()))
  {
    std::fprintf(stderr, "UNIFY SCOPED MISMATCH: faces %d/%d edges %d/%d vertices %d/%d volume %.12g/%.12g\n", n[0][0], n[1][0], n[0][1], n[1][1], n[0][2], n[1][2], a.Mass(), b.Mass());
    std::abort();
  }
}

TopoDS_Shape unify(const TopoDS_Shape& s0, HHist& hist)
{
  return unify_in(s0, hist, nullptr, nullptr);
}

// Two faces on one cylinder meeting at a full ring, each a band closed at
// its far end by one circle (a circle extruded both ways from its plane: the
// halves' axes and seams point opposite ways, live's frames), stay two after
// OCCT's unify, which can't move a seam (qa CYL-UNIFY-OPPOSITE; live one
// face). Merged here: one face on the first band's surface over both, its far
// circle, a new circle at the second band's far end (put into that end's cap
// face), one new seam. The shape unchanged when there's no such pair or the
// merge doesn't come out valid with the same volume.
TopoDS_Shape merge_cylinder_bands(const TopoDS_Shape& s, HHist& hist)
{
  ShapeAncMap ef;
  unique_ancestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  auto cyl_of = [](const TopoDS_Face& f, gp_Cylinder& c) {
    TopLoc_Location                      loc;
    occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(f, loc));
    if (cs.IsNull())
      return false;
    c = cs->Cylinder().Transformed(loc.Transformation());
    return true;
  };
  // A band: one wire, one seam, the far end one closed circle, the rest the
  // ring shared with `other`.
  struct Band
  {
    TopoDS_Edge              seam, end;
    std::vector<TopoDS_Edge> ring;
  };
  auto band_of = [&](const TopoDS_Face& f, const TopoDS_Face& other, Band& b) {
    int wires = 0;
    for (TopoDS_Iterator w(f); w.More(); w.Next())
      ++wires;
    if (wires != 1)
      return false;
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(x.Current());
      if (BRep_Tool::IsClosed(e, f))
      {
        if (!b.seam.IsNull() && !b.seam.IsSame(e))
          return false;
        b.seam = e;
        continue;
      }
      const ShapeList* fl = ef.Seek(e);
      if (fl == nullptr || fl->Extent() != 2)
        return false;
      if (fl->First().IsSame(other) || fl->Last().IsSame(other))
      {
        bool seen = false;
        for (const TopoDS_Edge& r : b.ring)
          seen = seen || r.IsSame(e);
        if (!seen)
          b.ring.push_back(e);
      }
      else if (b.end.IsNull())
        b.end = e;
      else if (!b.end.IsSame(e))
        return false;
    }
    if (b.seam.IsNull() || b.end.IsNull() || b.ring.empty())
      return false;
    TopoDS_Vertex v0, v1;
    TopExp::Vertices(b.end, v0, v1);
    BRepAdaptor_Curve c(b.end);
    return !v0.IsNull() && v0.IsSame(v1) && c.GetType() == GeomAbs_Circle;
  };
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    if (ef(i).Extent() != 2)
      continue;
    const TopoDS_Face a = TopoDS::Face(ef(i).First()), b = TopoDS::Face(ef(i).Last());
    gp_Cylinder       ca, cb;
    if (a.IsSame(b) || !cyl_of(a, ca) || !cyl_of(b, cb))
      continue;
    const double r = ca.Radius();
    if (!ca.Axis().Direction().IsParallel(cb.Axis().Direction(), 1e-12) || std::abs(cb.Radius() - r) > 1e-9 * r
        || gp_Lin(ca.Axis()).Distance(cb.Location()) > 1e-9 * (1 + r))
      continue;
    Band ba, bb;
    if (!band_of(a, b, ba) || !band_of(b, a, bb))
      continue;
    TopLoc_Location                 la;
    const occ::handle<Geom_Surface> ga = BRep_Tool::Surface(a, la);
    double                          u0, u1, va0, va1;
    BRepTools::UVBounds(a, u0, u1, va0, va1);
    if (std::abs(u1 - u0 - 2 * M_PI) > 1e-9)
      continue;
    auto v_of = [&](const TopoDS_Edge& e) {
      BRepAdaptor_Curve c(e);
      double            u, v;
      ElSLib::Parameters(ca, c.Value(c.FirstParameter()), u, v);
      return v;
    };
    const double v_af = v_of(ba.end), v_bf = v_of(bb.end);
    const double lo = std::min(v_af, v_bf), hi = std::max(v_af, v_bf);
    if (!(hi - lo > 1e-9))
      continue;
    // a's far circle's vertex on a's seam (u0).
    TopoDS_Vertex a_v0, a_v1;
    TopExp::Vertices(ba.end, a_v0, a_v1);
    double au, av;
    ElSLib::Parameters(ca, BRep_Tool::Pnt(a_v0), au, av);
    if (std::abs(std::remainder(au - u0, 2 * M_PI)) > 1e-7)
      continue;
    const gp_Ax3& A = ca.Position();
    BRep_Builder  B;
    const double  tol = std::max(BRep_Tool::Tolerance(ba.end), BRep_Tool::Tolerance(bb.end));
    // b's far circle in a's frame: its parameter is a's u, its vertex on a's seam.
    TopoDS_Vertex vb;
    B.MakeVertex(vb, ElSLib::Value(u0, v_bf, ca), tol);
    const gp_Dir             n = gp_Dir(gp_Vec(A.XDirection()).Crossed(gp_Vec(A.YDirection())));
    occ::handle<Geom_Circle> circ =
      new Geom_Circle(gp_Ax2(A.Location().Translated(gp_Vec(A.Direction()) * v_bf), n, A.XDirection()), r);
    TopoDS_Edge b_far;
    B.MakeEdge(b_far, circ, tol);
    B.Add(b_far, vb.Oriented(TopAbs_FORWARD));
    B.Add(b_far, vb.Oriented(TopAbs_REVERSED));
    B.Range(b_far, u0, u0 + 2 * M_PI);
    // The seam, from the low circle's vertex up.
    const TopoDS_Vertex    v_lo = v_af < v_bf ? a_v0 : vb, v_hi = v_af < v_bf ? vb : a_v0;
    occ::handle<Geom_Line> line = new Geom_Line(ElSLib::Value(u0, lo, ca), A.Direction());
    TopoDS_Edge            seam;
    B.MakeEdge(seam, line, tol);
    B.Add(seam, v_lo.Oriented(TopAbs_FORWARD));
    B.Add(seam, v_hi.Oriented(TopAbs_REVERSED));
    B.Range(seam, 0, hi - lo);
    TopoDS_Face F;
    B.MakeFace(F, ga, la, BRep_Tool::Tolerance(a));
    B.UpdateEdge(b_far, new Geom2d_Line(gp_Pnt2d(0, v_bf), gp_Dir2d(1, 0)), F, tol);
    B.Range(b_far, F, u0, u0 + 2 * M_PI);
    B.UpdateEdge(seam, new Geom2d_Line(gp_Pnt2d(u0 + 2 * M_PI, lo), gp_Dir2d(0, 1)),
                 new Geom2d_Line(gp_Pnt2d(u0, lo), gp_Dir2d(0, 1)), F, tol);
    B.Range(seam, F, 0, hi - lo);
    // a's far circle keeps its curve on a's surface (F's).
    double                          f2, l2;
    const TopoDS_Edge               a_far = TopoDS::Edge(ba.end.Oriented(TopAbs_FORWARD));
    const occ::handle<Geom2d_Curve> pa    = BRep_Tool::CurveOnSurface(a_far, a, f2, l2);
    if (pa.IsNull())
      continue;
    const bool  a_up = pa->Value(f2 + 0.25 * (l2 - f2)).X() > pa->Value(f2).X();
    const bool  a_low = v_af < v_bf;
    TopoDS_Wire W;
    B.MakeWire(W);
    // Bottom with u growing, the seam up at u0 + 2 pi, the top back, the seam down at u0.
    const TopoDS_Edge low = a_low ? (a_up ? a_far : TopoDS::Edge(a_far.Reversed())) : b_far;
    const TopoDS_Edge top = a_low ? TopoDS::Edge(b_far.Reversed()) : (a_up ? TopoDS::Edge(a_far.Reversed()) : a_far);
    B.Add(W, low);
    B.Add(W, seam.Oriented(TopAbs_FORWARD));
    B.Add(W, top);
    B.Add(W, seam.Oriented(TopAbs_REVERSED));
    B.Add(F, W);
    if (!BRepCheck_Analyzer(F).IsValid())
      continue;
    // b's far circle in its cap, with the old one's sense.
    BRepAdaptor_Curve old_c(bb.end);
    const bool        same  = old_c.Circle().Axis().Direction().Dot(gp_Vec(n)) > 0;
    const TopoDS_Edge b_new = same ? b_far : TopoDS::Edge(b_far.Reversed());
    occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
    rs->Replace(a.Oriented(TopAbs_FORWARD), F);
    rs->Remove(b);
    rs->Replace(bb.end.Oriented(TopAbs_FORWARD), b_new);
    const TopoDS_Shape out = rs->Apply(s);
    if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
      continue;
    GProp_GProps g0, g1;
    BRepGProp::VolumeProperties(s, g0);
    BRepGProp::VolumeProperties(out, g1);
    if (std::abs(g1.Mass() - g0.Mass()) > 1e-9 * std::abs(g0.Mass()))
      continue;
    if (hist.IsNull())
      hist = new BRepTools_History();
    // ReShape's (the caps it rebuilt), b onto the new face too.
    HHist h = rs->History();
    if (h.IsNull())
      h = new BRepTools_History();
    if (h->Modified(a).IsEmpty())
      h->AddModified(a, F);
    h->AddModified(b, F);
    h->AddModified(bb.end, b_far);
    h->Remove(ba.seam);
    h->Remove(bb.seam);
    for (const TopoDS_Edge& e : ba.ring)
      h->Remove(e);
    hist->Merge(h);
    // One pair at a time: the shape changed, look again.
    return merge_cylinder_bands(out, hist);
  }
  return s;
}

// Two faces on one cone meeting along two of its lines, each running from
// the apex (a degenerate edge) to arcs of one circle: a drill tip cut in half
// and mirrored (or patterned) back onto itself. OCCT's unify gives an invalid
// solid there from every wire start (qa MIRROR-BLIND-HOLE-FACES; live one
// face). Merged here, on a copy: one face on the first one's surface over the
// whole turn, one of the two lines its seam, a new degenerate edge at the
// apex. The shape unchanged when there's no such pair or the merge doesn't
// come out valid with the same volume.
TopoDS_Shape merge_apex_cones(const TopoDS_Shape& s, HHist& hist)
{
  ShapeAncMap ef;
  unique_ancestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  auto cone_of = [](const TopoDS_Face& f, gp_Cone& c) {
    TopLoc_Location                  loc;
    occ::handle<Geom_ConicalSurface> cs = occ::down_cast<Geom_ConicalSurface>(BRep_Tool::Surface(f, loc));
    if (cs.IsNull())
      return false;
    c = cs->Cone().Transformed(loc.Transformation());
    return true;
  };
  // A half: one wire of a degenerate edge, two lines shared with `other`, the
  // rest arcs of circles.
  struct Half
  {
    TopoDS_Edge              apex;
    std::vector<TopoDS_Edge> lines, arcs;
  };
  auto half_of = [](const ShapeAncMap& m, const TopoDS_Face& f, const TopoDS_Face& other, Half& h) {
    int wires = 0;
    for (TopoDS_Iterator w(f); w.More(); w.Next())
      ++wires;
    if (wires != 1)
      return false;
    auto add = [](std::vector<TopoDS_Edge>& l, const TopoDS_Edge& e) {
      for (const TopoDS_Edge& x : l)
        if (x.IsSame(e))
          return;
      l.push_back(e);
    };
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(x.Current());
      if (BRep_Tool::Degenerated(e))
      {
        if (!h.apex.IsNull() && !h.apex.IsSame(e))
          return false;
        h.apex = e;
        continue;
      }
      const ShapeList* fl = m.Seek(e);
      if (fl == nullptr || fl->Extent() != 2)
        return false;
      const GeomAbs_CurveType t = BRepAdaptor_Curve(e).GetType();
      if (fl->First().IsSame(other) || fl->Last().IsSame(other))
      {
        if (t != GeomAbs_Line)
          return false;
        add(h.lines, e);
      }
      else
      {
        if (t != GeomAbs_Circle)
          return false;
        add(h.arcs, e);
      }
    }
    return !h.apex.IsNull() && h.lines.size() == 2 && !h.arcs.empty();
  };
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    if (ef(i).Extent() != 2)
      continue;
    const TopoDS_Face a0 = TopoDS::Face(ef(i).First()), b0 = TopoDS::Face(ef(i).Last());
    gp_Cone           ca, cb;
    if (a0.IsSame(b0) || !cone_of(a0, ca) || !cone_of(b0, cb))
      continue;
    const double tol0 = std::max(BRep_Tool::Tolerance(a0), BRep_Tool::Tolerance(b0));
    if (ca.Apex().Distance(cb.Apex()) > tol0 || !ca.Axis().Direction().IsParallel(cb.Axis().Direction(), 1e-9)
        || std::abs(std::abs(ca.SemiAngle()) - std::abs(cb.SemiAngle())) > 1e-9)
      continue;
    Half h0, h1;
    if (!half_of(ef, a0, b0, h0) || !half_of(ef, b0, a0, h1))
      continue;
    // On a copy from here (the edges get curves on the new face).
    BRepBuilderAPI_Copy cp(s, false, false);
    const TopoDS_Shape  c = cp.Shape();
    auto                image = [&](const TopoDS_Shape& x) {
      TopoDS_Shape r = cp.ModifiedShape(x);
      return x.Orientation() == r.Orientation() ? r : r.Reversed();
    };
    const TopoDS_Face a = TopoDS::Face(image(a0)), b = TopoDS::Face(image(b0));
    ShapeAncMap       ec;
    unique_ancestors(c, TopAbs_EDGE, TopAbs_FACE, ec);
    Half ha, hb;
    if (!half_of(ec, a, b, ha) || !half_of(ec, b, a, hb))
      continue;
    cone_of(a, ca);
    TopLoc_Location                 la;
    const occ::handle<Geom_Surface> ga  = BRep_Tool::Surface(a, la);
    const double                    tol = std::max(BRep_Tool::Tolerance(a), BRep_Tool::Tolerance(b));
    auto                            uv  = [&](const gp_Pnt& p) {
      double u, v;
      ElSLib::Parameters(ca, p, u, v);
      return std::make_pair(u, v);
    };
    double u_apex, v_apex;
    std::tie(u_apex, v_apex) = uv(ca.Apex());
    // The seam: the first line, at u0 (its point off the apex).
    const TopoDS_Edge seam = ha.lines[0];
    BRepAdaptor_Curve sc(seam);
    const double      sf = sc.FirstParameter(), sl = sc.LastParameter();
    const gp_Pnt      sp = sc.Value(sf).Distance(ca.Apex()) > sc.Value(sl).Distance(ca.Apex()) ? sc.Value(sf) : sc.Value(sl);
    const double      u0 = uv(sp).first;
    auto              in_turn = [&](double u) { return u0 + std::fmod(std::fmod(u - u0, 2 * M_PI) + 2 * M_PI, 2 * M_PI); };
    // The arcs: v constant (the same for all), u along the parameter (slope +-1).
    struct Arc
    {
      TopoDS_Edge e;
      double      lo, hi, slope, at0;
    };
    std::vector<Arc> arcs;
    bool             ok = true;
    double           v_base = 0;
    for (const std::vector<TopoDS_Edge>* l : {&ha.arcs, &hb.arcs})
      for (const TopoDS_Edge& e : *l)
      {
        BRepAdaptor_Curve cc(e);
        const double      f = cc.FirstParameter(), l2 = cc.LastParameter(), m = 0.5 * (f + l2);
        const auto        pf = uv(cc.Value(f)), pm = uv(cc.Value(m));
        const double      um = in_turn(pm.first);
        const double      d  = std::remainder(pm.first - pf.first, 2 * M_PI);
        const double      slope = d > 0 ? 1.0 : -1.0;
        if (std::abs(std::abs(d) - (m - f)) > 1e-7 || std::abs(pf.second - pm.second) > tol || (!arcs.empty() && std::abs(pm.second - v_base) > tol)
            || ElSLib::Value(pm.first, pm.second, ca).Distance(cc.Value(m)) > tol)
          ok = false;
        v_base = pm.second;
        // u at the parameter t: um + slope (t - m).
        const double ua = um + slope * (f - m), ub = um + slope * (l2 - m);
        arcs.push_back({e, std::min(ua, ub), std::max(ua, ub), slope, um - slope * m});
      }
    if (!ok)
      continue;
    std::sort(arcs.begin(), arcs.end(), [](const Arc& x, const Arc& y) { return x.lo < y.lo; });
    // The arcs end to end over the turn.
    double at = u0;
    for (const Arc& x : arcs)
    {
      ok = ok && std::abs(x.lo - at) < 1e-7;
      at = x.hi;
    }
    if (!ok || std::abs(at - (u0 + 2 * M_PI)) > 1e-7)
      continue;
    // The seam's v along its parameter (slope +-1).
    const double sv0 = uv(sc.Value(sf)).second, sv1 = uv(sc.Value(sl)).second;
    if (std::abs(std::abs(sv1 - sv0) - (sl - sf)) > 1e-7)
      continue;
    const double s_slope = sv1 > sv0 ? 1.0 : -1.0, s_at0 = sv0 - s_slope * sf;
    TopoDS_Vertex apex_v, apex_w;
    TopExp::Vertices(ha.apex, apex_v, apex_w);
    if (apex_v.IsNull())
      continue;
    BRep_Builder B;
    TopoDS_Face  F;
    B.MakeFace(F, ga, la, BRep_Tool::Tolerance(a));
    for (const Arc& x : arcs)
    {
      B.UpdateEdge(x.e, new Geom2d_Line(gp_Pnt2d(x.at0, v_base), gp_Dir2d(x.slope, 0)), F, tol);
    }
    // Going up the right side (u0 + 2 pi) is the seam's forward use when its v grows with its parameter.
    const occ::handle<Geom2d_Curve> right = new Geom2d_Line(gp_Pnt2d(u0 + 2 * M_PI, s_at0), gp_Dir2d(0, s_slope));
    const occ::handle<Geom2d_Curve> left  = new Geom2d_Line(gp_Pnt2d(u0, s_at0), gp_Dir2d(0, s_slope));
    if (s_slope > 0)
      B.UpdateEdge(seam, right, left, F, tol);
    else
      B.UpdateEdge(seam, left, right, F, tol);
    TopoDS_Edge deg;
    B.MakeEdge(deg);
    B.Add(deg, apex_v.Oriented(TopAbs_FORWARD));
    B.Add(deg, apex_v.Oriented(TopAbs_REVERSED));
    B.Degenerated(deg, true);
    B.UpdateEdge(deg, new Geom2d_Line(gp_Pnt2d(0, v_apex), gp_Dir2d(1, 0)), F, tol);
    B.Range(deg, u0, u0 + 2 * M_PI);
    // Counterclockwise in (u, v): up the right side, back along the top, down the left.
    const TopoDS_Edge up   = TopoDS::Edge(seam.Oriented(s_slope > 0 ? TopAbs_FORWARD : TopAbs_REVERSED));
    const TopoDS_Edge down = TopoDS::Edge(up.Reversed());
    TopoDS_Wire       W;
    B.MakeWire(W);
    auto arc_along = [](const Arc& x, bool rising) { return TopoDS::Edge(x.e.Oriented((x.slope > 0) == rising ? TopAbs_FORWARD : TopAbs_REVERSED)); };
    if (v_apex > v_base)
    {
      for (const Arc& x : arcs)
        B.Add(W, arc_along(x, true));
      B.Add(W, up);
      B.Add(W, deg.Oriented(TopAbs_REVERSED));
      B.Add(W, down);
    }
    else
    {
      B.Add(W, deg.Oriented(TopAbs_FORWARD));
      B.Add(W, up);
      for (auto x = arcs.rbegin(); x != arcs.rend(); ++x)
        B.Add(W, arc_along(*x, false));
      B.Add(W, down);
    }
    W.Closed(true);
    B.Add(F, W);
    if (!BRepCheck_Analyzer(F).IsValid())
      continue;
    occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
    rs->Replace(a.Oriented(TopAbs_FORWARD), F);
    rs->Remove(b);
    const TopoDS_Shape out = rs->Apply(c);
    if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
      continue;
    GProp_GProps g0, g1;
    BRepGProp::VolumeProperties(s, g0);
    BRepGProp::VolumeProperties(out, g1);
    if (std::abs(g1.Mass() - g0.Mass()) > 1e-9 * std::abs(g0.Mass()))
      continue;
    if (hist.IsNull())
      hist = new BRepTools_History();
    hist->Merge(new BRepTools_History(to_list(std::vector<TopoDS_Shape>{s}), cp));
    HHist h = rs->History();
    if (h.IsNull())
      h = new BRepTools_History();
    if (h->Modified(a).IsEmpty())
      h->AddModified(a, F);
    h->AddModified(b, F);
    h->AddModified(ha.apex, deg);
    h->AddModified(hb.apex, deg);
    h->Remove(ha.lines[1]);
    hist->Merge(h);
    // One pair at a time: the shape changed, look again.
    return merge_apex_cones(out, hist);
  }
  return s;
}

// Two faces on one cylinder that USD leaves apart, sharing one chain of
// edges (a notched cylinder fused with its mirror: a patch of the mirror's
// wall filling the notch's bay in the other's, qa OCCT-MIRROR-OVERCOUNT; live
// one face): on a copy, the first face's wire with that chain replaced by the
// second's other edges (lines along the axis and circles round it), on the
// first's surface. The shape unchanged when there's no such pair or the
// merge doesn't come out valid with the same volume.
TopoDS_Shape merge_cylinder_patches(const TopoDS_Shape& s, HHist& hist)
{
  ShapeAncMap ef;
  unique_ancestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  auto cyl_of = [](const TopoDS_Face& f, gp_Cylinder& c) {
    TopLoc_Location                      loc;
    occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(f, loc));
    if (cs.IsNull())
      return false;
    c = cs->Cylinder().Transformed(loc.Transformation());
    return true;
  };
  auto one_wire = [](const TopoDS_Face& f, TopoDS_Wire& w) {
    int n = 0;
    for (TopoDS_Iterator it(f); it.More(); it.Next(), ++n)
      w = TopoDS::Wire(it.Value());
    return n == 1;
  };
  // The edges of `f`'s wire in order, oriented as `f` (oriented) runs them.
  auto ordered = [](const TopoDS_Face& f, const TopoDS_Wire& w) {
    std::vector<TopoDS_Edge> l;
    for (BRepTools_WireExplorer we(w, f); we.More(); we.Next())
      l.push_back(we.Current());
    return l;
  };
  // Each pair both ways (the first one's u range must take the second's edges).
  for (int q = 0; q < 2 * ef.Extent(); ++q)
  {
    const int i = q / 2 + 1;
    if (ef(i).Extent() != 2)
      continue;
    const TopoDS_Face a0 = TopoDS::Face(q % 2 == 0 ? ef(i).First() : ef(i).Last()), b0 = TopoDS::Face(q % 2 == 0 ? ef(i).Last() : ef(i).First());
    gp_Cylinder       ca, cb;
    if (a0.IsSame(b0) || !cyl_of(a0, ca) || !cyl_of(b0, cb))
      continue;
    const double r = ca.Radius();
    if (!ca.Axis().Direction().IsParallel(cb.Axis().Direction(), 1e-12) || std::abs(cb.Radius() - r) > 1e-9 * r
        || gp_Lin(ca.Axis()).Distance(cb.Location()) > 1e-9 * (1 + r))
      continue;
    TopoDS_Wire wa0, wb0;
    if (!one_wire(a0, wa0) || !one_wire(b0, wb0))
      continue;
    // On a copy from here (the second's edges get curves on the new face).
    BRepBuilderAPI_Copy cp(s, false, false);
    const TopoDS_Shape  c     = cp.Shape();
    auto                image = [&](const TopoDS_Shape& x) {
      TopoDS_Shape y = cp.ModifiedShape(x);
      return x.Orientation() == y.Orientation() ? y : y.Reversed();
    };
    const TopoDS_Face a = TopoDS::Face(image(a0)), b = TopoDS::Face(image(b0));
    TopoDS_Wire       wa, wb;
    one_wire(a, wa);
    one_wire(b, wb);
    std::vector<TopoDS_Edge> ea = ordered(a, wa), eb = ordered(b, wb);
    auto shared = [&](const TopoDS_Edge& e, const std::vector<TopoDS_Edge>& other) {
      return std::any_of(other.begin(), other.end(), [&](const TopoDS_Edge& x) { return x.IsSame(e); });
    };
    // Each list turned to end with its one run of shared edges.
    auto chain_last = [&](std::vector<TopoDS_Edge>& l, const std::vector<TopoDS_Edge>& other) {
      const size_t n = l.size();
      int          runs = 0;
      size_t       start = 0;
      for (size_t k = 0; k < n; ++k)
        if (!shared(l[k], other) && shared(l[(k + 1) % n], other))
        {
          ++runs;
          start = (k + 1) % n;
        }
      if (runs != 1)
        return false;
      size_t end = start;
      while (shared(l[end % n], other))
        ++end;
      std::rotate(l.begin(), l.begin() + (long)(end % n), l.end());
      return true;
    };
    if (ea.size() < 2 || eb.size() < 2 || !chain_last(ea, eb) || !chain_last(eb, ea))
      continue;
    std::vector<TopoDS_Edge> merged, own_a, from_b;
    for (const TopoDS_Edge& e : ea)
      if (!shared(e, eb))
        own_a.push_back(e);
    for (const TopoDS_Edge& e : eb)
      if (!shared(e, ea))
        from_b.push_back(e);
    if (own_a.empty() || from_b.empty())
      continue;
    // a's surface and its u range; b's edges as lines (u fixed) and circles
    // (v fixed) on it.
    cyl_of(a, ca);
    TopLoc_Location                 la;
    const occ::handle<Geom_Surface> ga = BRep_Tool::Surface(a, la);
    double                          u0, u1, v0, v1;
    BRepTools::UVBounds(a, u0, u1, v0, v1);
    const double tol = std::max(BRep_Tool::Tolerance(a), BRep_Tool::Tolerance(b));
    auto         uv  = [&](const gp_Pnt& p) {
      double u, v;
      ElSLib::Parameters(ca, p, u, v);
      return std::make_pair(u, v);
    };
    // u moved by turns into a's range (false when it can't be).
    auto into = [&](double& u) {
      u = u0 + std::fmod(std::fmod(u - u0, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
      if (u > u1 + 1e-9 && u - 2 * M_PI >= u0 - 1e-9)
        u -= 2 * M_PI;
      return u >= u0 - 1e-9 && u <= u1 + 1e-9;
    };
    TopoDS_Face TF;
    BRep_Builder B;
    B.MakeFace(TF, ga, la, BRep_Tool::Tolerance(a));
    bool ok = true;
    for (const TopoDS_Edge& e : from_b)
    {
      BRepAdaptor_Curve cc(e);
      const double      f = cc.FirstParameter(), l = cc.LastParameter(), m = 0.5 * (f + l);
      auto              pf = uv(cc.Value(f)), pm = uv(cc.Value(m));
      if (ElSLib::Value(pm.first, pm.second, ca).Distance(cc.Value(m)) > tol)
      {
        ok = false;
        break;
      }
      occ::handle<Geom2d_Curve> c2;
      if (cc.GetType() == GeomAbs_Line && std::abs(std::remainder(pm.first - pf.first, 2 * M_PI)) < 1e-9)
      {
        double um = pm.first;
        const double dv = pm.second - pf.second, slope = dv > 0 ? 1.0 : -1.0;
        if (!into(um) || std::abs(std::abs(dv) - (m - f)) > 1e-7)
          ok = false;
        c2 = new Geom2d_Line(gp_Pnt2d(um, pm.second - slope * m), gp_Dir2d(0, slope));
      }
      else if (cc.GetType() == GeomAbs_Circle && std::abs(pm.second - pf.second) < tol)
      {
        double       um = pm.first;
        const double du = std::remainder(pm.first - pf.first, 2 * M_PI), slope = du > 0 ? 1.0 : -1.0;
        if (!into(um) || std::abs(std::abs(du) - (m - f)) > 1e-7 || std::min(um + slope * (f - m), um + slope * (l - m)) < u0 - 1e-9
            || std::max(um + slope * (f - m), um + slope * (l - m)) > u1 + 1e-9)
          ok = false;
        c2 = new Geom2d_Line(gp_Pnt2d(um - slope * m, pm.second), gp_Dir2d(slope, 0));
      }
      else
        ok = false;
      if (!ok)
        break;
      B.UpdateEdge(e, c2, TF, tol);
    }
    if (!ok)
      continue;
    // As a's TShape has them (a's orientation in the shell undone).
    const bool  flip = a.Orientation() == TopAbs_REVERSED;
    TopoDS_Wire W;
    B.MakeWire(W);
    for (const std::vector<TopoDS_Edge>* l : {&own_a, &from_b})
      for (const TopoDS_Edge& e : *l)
        B.Add(W, flip ? e.Reversed() : TopoDS_Shape(e));
    W.Closed(true);
    B.Add(TF, flip ? W.Reversed() : TopoDS_Shape(W));
    if (!BRepCheck_Analyzer(TF).IsValid())
      continue;
    occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
    rs->Replace(a.Oriented(TopAbs_FORWARD), TF);
    rs->Remove(b);
    const TopoDS_Shape out = rs->Apply(c);
    if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
      continue;
    GProp_GProps g0, g1;
    BRepGProp::VolumeProperties(s, g0);
    BRepGProp::VolumeProperties(out, g1);
    if (std::abs(g1.Mass() - g0.Mass()) > 1e-9 * std::abs(g0.Mass()))
      continue;
    if (hist.IsNull())
      hist = new BRepTools_History();
    hist->Merge(new BRepTools_History(to_list(std::vector<TopoDS_Shape>{s}), cp));
    HHist h = rs->History();
    if (h.IsNull())
      h = new BRepTools_History();
    if (h->Modified(a).IsEmpty())
      h->AddModified(a, TF);
    h->AddModified(b, TF);
    for (const TopoDS_Edge& e : ea)
      if (shared(e, eb))
        h->Remove(e);
    hist->Merge(h);
    // One pair at a time: the shape changed, look again.
    return merge_cylinder_patches(out, hist);
  }
  return s;
}

// The unify, looking only in `sc` when given (UnifyScope); `clean` (when
// given) says whether the result has nothing more to merge.
TopoDS_Shape unify_in(const TopoDS_Shape& s0, HHist& hist, const UnifyScope* sc, bool* clean)
{
  if (clean != nullptr)
    *clean = false;
  std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("unify.candidates");
  if (!same_domain_candidates(s0, sc))
  {
    if (sc != nullptr)
      check_scoped_unify(s0, s0);
    if (clean != nullptr)
      *clean = true;
    return s0;
  }
  lap = std::make_unique<KernelTimer>("unify.merge_planar");
  UnifyScope         next;
  const TopoDS_Shape s = merge_planar(s0, hist, sc, &next);
  lap                  = std::make_unique<KernelTimer>("unify.candidates_again");
  if (!same_domain_candidates(s, sc != nullptr ? &next : nullptr))
  {
    if (sc != nullptr)
      check_scoped_unify(s0, s);
    if (clean != nullptr)
      *clean = true;
    return s;
  }
  lap = std::make_unique<KernelTimer>("unify.usd");
  const TopoDS_Shape s1 = merge_apex_cones(merge_cylinder_bands(s, hist), hist);
  if (TopExp_Explorer(s1, TopAbs_SOLID).More() && round_candidates(s1, sc != nullptr ? &next : nullptr))
    return merge_cylinder_patches(usd_kept_volume(s1, hist), hist);
  ShapeUpgrade_UnifySameDomain u(s1, true, true, false);
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

// fuse_trimmed with noshape's own Boolean throughout: each tool cut by the arguments, then the arguments fused with
// what is left of the tools (they only touch now). False (nothing done) where any of them declines.
bool rust_fuse_trimmed(const std::vector<TopoDS_Shape>& a, const std::vector<TopoDS_Shape>& t, const BoolOpts& opts, TopoDS_Shape& out, HHist& hist)
{
  const RustLenient lenient;
  const size_t n = t.size();
  std::vector<TopoDS_Shape> trimmed(n);
  std::vector<HHist>        cuts(n);
  // (In parallel, as fuse_trimmed's cuts: each thread's Boolean is its own.)
  std::vector<char>        failed(n, 0);
  const std::atomic<bool>* flag = t_cancel;
  OSD_Parallel::For(0, static_cast<int>(n), [&](int i) {
    if (cancelled(flag))
    {
      failed[static_cast<size_t>(i)] = 1;
      return;
    }
    try
    {
      const RustLenient                     lenient_here;
      std::vector<TopoDS_Shape>             in{t[static_cast<size_t>(i)]};
      std::vector<const OcShape*>           rs;
      std::vector<std::unique_ptr<OcShape>> own;
      in.insert(in.end(), a.begin(), a.end());
      if (!rust_boolean(1, in, rs, own, trimmed[static_cast<size_t>(i)], cuts[static_cast<size_t>(i)], opts.fuzzy, false))
        failed[static_cast<size_t>(i)] = 1;
    }
    catch (...)
    {
      failed[static_cast<size_t>(i)] = 1;
    }
  });
  fail_if_cancelled();
  for (size_t i = 0; i < n; ++i)
    if (failed[i])
      return false;
  std::vector<TopoDS_Shape> rest;
  for (size_t i = 0; i < n; ++i)
    if (TopExp_Explorer e(trimmed[i], TopAbs_SOLID); e.More())
      rest.push_back(trimmed[i]);
  if (rest.empty())
    return false;
  TopoDS_Shape                          fused;
  HHist                                 h2;
  std::vector<const OcShape*>           rs;
  std::vector<std::unique_ptr<OcShape>> own;
  if (!rust_boolean(0, concat(a, rest), rs, own, fused, h2, opts.fuzzy, true) || !volumes_add_up(concat(a, rest), fused))
    return false;
  // History from the original inputs, as fuse_trimmed's: the arguments as the fuse has them; a tool's sub-shapes
  // through its cut, then through the fuse.
  HHist h    = new BRepTools_History();
  auto  thru = [&](const TopoDS_Shape& s, const TopoDS_Shape& x, bool generated) {
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
  for (size_t i = 0; i < n; ++i)
  {
    const HHist& c = cuts[i];
    for_sub_shapes(t[i], [&](const TopoDS_Shape& s) {
      if (c->IsRemoved(s))
      {
        h->Remove(s);
        return;
      }
      const ShapeList& m1   = c->Modified(s);
      bool             kept = false;
      if (m1.IsEmpty())
        kept = thru(s, s, false);
      for (ShapeList::Iterator it(m1); it.More(); it.Next())
        kept |= thru(s, it.Value(), false);
      for (ShapeList::Iterator it(c->Generated(s)); it.More(); it.Next())
        thru(s, it.Value(), true);
      if (!kept)
        h->Remove(s);
    });
  }
  out  = fused;
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
  sp.SetRunParallel(bop_parallel());
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
  g.SetRunParallel(bop_parallel());
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
  sp.SetRunParallel(bop_parallel());
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
  const std::atomic<bool>*     flag = t_cancel;
  std::unique_ptr<KernelTimer> lap  = std::make_unique<KernelTimer>("slabs.split");
  BRepAlgoAPI_Splitter         sp;
  ShapeList                one;
  one.Append(target);
  sp.SetArguments(one);
  sp.SetTools(planes);
  sp.SetRunParallel(bop_parallel());
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
  lap = std::make_unique<KernelTimer>("slabs.cuts");
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
    // Checked here, a slab at a time in parallel: the whole part's check
    // (its faces with 400 hole loops) cost ~300 ms.
    if (!BRepCheck_Analyzer(c.Shape()).IsValid())
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
  lap                              = std::make_unique<KernelTimer>("slabs.join");
  const TopoDS_Shape        joined = parts.size() < 2 ? TopoDS_Shape() : join_on_shared_faces(parts, shared);
  if (joined.IsNull())
    return false;
  lap      = std::make_unique<KernelTimer>("slabs.history");
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
  lap                  = std::make_unique<KernelTimer>("slabs.unify");
  const TopoDS_Shape r = unify(joined, h);
  lap                  = std::make_unique<KernelTimer>("slabs.check");
  // The slabs were checked; joined on their shared faces and unified, the
  // part must still be closed.
  int solids = 0;
  for (TopExp_Explorer e(r, TopAbs_SOLID); e.More(); e.Next())
    ++solids;
  if (solids != 1 || !closed_and_oriented(r))
    return false;
  out  = r;
  hist = h;
  return true;
}

// Holes straight through a plate (big-plate: 400 Extrude Remove circles):
// each tool a circle prism (a cylinder and two caps square to it) passing
// clean through two opposite planar faces of the target and touching nothing
// else, so the result is the target with each hole's circles as new inner
// loops of those two faces and the tool's cylinder between them as the
// hole's wall. Built directly (no boolean: OCCT's cut of 400 tools in slabs
// spent ~1.7 s of CPU). False when it doesn't apply; the caller cuts as usual.
static bool cut_through_holes(const TopoDS_Shape& target, const std::vector<TopoDS_Shape>& t, TopoDS_Shape& out, HHist& hist)
{
  auto no = [](int why) {
    if (std::getenv("NOSHAPE_BOOL_DEBUG"))
      std::fprintf(stderr, "through holes: no (%d)\n", why);
    return false;
  };
  if (t.size() < 2)
    return no(1);
  TopoDS_Shape solid;
  int          nsolid = 0;
  for (TopExp_Explorer e(target, TopAbs_SOLID); e.More(); e.Next(), ++nsolid)
    solid = e.Current();
  if (nsolid != 1)
    return no(2);
  TopoDS_Shape shell;
  int          nshell = 0;
  for (TopoDS_Iterator it(solid); it.More(); it.Next(), ++nshell)
    shell = it.Value();
  if (nshell != 1 || shell.ShapeType() != TopAbs_SHELL)
    return no(3);
  ShapeIMap faces;
  TopExp::MapShapes(solid, TopAbs_FACE, faces);
  Bnd_Box all;
  BRepBndLib::Add(solid, all, false);
  if (all.IsVoid())
    return no(4);
  const double tol = 1e-7 * std::max(1.0, std::sqrt(all.SquareExtent()));
  std::vector<Bnd_Box> fbox((size_t)faces.Extent());
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    BRepBndLib::Add(faces(i), fbox[(size_t)i - 1], false);
    fbox[(size_t)i - 1].Enlarge(tol);
  }

  struct Hole
  {
    TopoDS_Face                             side;
    std::vector<TopoDS_Shape>               caps;
    occ::handle<Geom_CylindricalSurface>    cyl; // in world coordinates
    int                                     fa = 0, fb = 0; // target faces (indices) it passes through
    double                                  va = 0, vb = 0; // their positions on the cylinder's v
    TopoDS_Face                             wall;
    TopoDS_Edge                             ea, eb; // the wall's circles on fa, fb
  };
  std::vector<Hole> holes(t.size());
  gp_Dir            d;
  for (size_t k = 0; k < t.size(); ++k)
  {
    Hole& h = holes[k];
    int   nf = 0;
    for (TopExp_Explorer e(t[k], TopAbs_FACE); e.More(); e.Next(), ++nf)
    {
      const TopoDS_Face&  f = TopoDS::Face(e.Current());
      BRepAdaptor_Surface s(f, false);
      if (s.GetType() == GeomAbs_Cylinder && h.side.IsNull())
        h.side = f;
      else if (s.GetType() == GeomAbs_Plane)
        h.caps.push_back(f);
      else
        return no(5);
    }
    if (nf != 3 || h.side.IsNull() || h.caps.size() != 2)
      return no(6);
    double u0, u1, v0, v1;
    BRepTools::UVBounds(h.side, u0, u1, v0, v1);
    if (u1 - u0 < 2 * M_PI - 1e-9)
      return no(7);
    TopLoc_Location                      loc;
    occ::handle<Geom_CylindricalSurface> c = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(h.side, loc));
    if (c.IsNull())
      return no(8);
    h.cyl = loc.IsIdentity() ? c : occ::down_cast<Geom_CylindricalSurface>(c->Transformed(loc.Transformation()));
    const gp_Dir z = h.cyl->Position().Direction();
    if (k == 0)
      d = z;
    else if (!z.IsParallel(d, 1e-12))
      return no(9);
    for (const TopoDS_Shape& cap : h.caps)
      if (!BRepAdaptor_Surface(TopoDS::Face(cap), false).Plane().Axis().Direction().IsParallel(z, 1e-12))
        return no(10);
  }

  // The target's faces each hole meets: exactly two, planar, square to the
  // holes, the hole's caps beyond both; nothing else near it.
  NCollection_DataMap<int, std::vector<size_t>> on_face;
  for (size_t k = 0; k < holes.size(); ++k)
  {
    Hole&        h = holes[k];
    const gp_Ax3 ax = h.cyl->Position();
    const double r  = h.cyl->Radius();
    Bnd_Box      tb;
    BRepBndLib::Add(h.side, tb, false);
    tb.Enlarge(tol);
    std::vector<int> met;
    for (int i = 1; i <= faces.Extent(); ++i)
      if (!fbox[(size_t)i - 1].IsOut(tb))
        met.push_back(i);
    if (met.size() != 2)
      return no(11);
    double vs[2];
    for (int j = 0; j < 2; ++j)
    {
      BRepAdaptor_Surface s(TopoDS::Face(faces(met[(size_t)j])), false);
      if (s.GetType() != GeomAbs_Plane || !s.Plane().Axis().Direction().IsParallel(d, 1e-12))
        return no(12);
      vs[j] = gp_Vec(ax.Location(), s.Plane().Location()).Dot(gp_Vec(ax.Direction()));
    }
    if (std::abs(vs[0] - vs[1]) <= 10 * tol)
      return no(13);
    double cv[2];
    for (int j = 0; j < 2; ++j)
      cv[j] = gp_Vec(ax.Location(), BRepAdaptor_Surface(TopoDS::Face(h.caps[(size_t)j]), false).Plane().Location()).Dot(gp_Vec(ax.Direction()));
    const double lo = std::min(vs[0], vs[1]), hi = std::max(vs[0], vs[1]);
    // Caps on or beyond both faces (a sketch on the top face cut through all
    // starts on it).
    if (!(std::min(cv[0], cv[1]) < lo + 10 * tol && std::max(cv[0], cv[1]) > hi - 10 * tol))
      return no(14);
    h.fa = vs[0] < vs[1] ? met[0] : met[1];
    h.fb = vs[0] < vs[1] ? met[1] : met[0];
    h.va = lo;
    h.vb = hi;
    // The circles lie inside their faces, clear of the faces' edges.
    for (int j = 0; j < 2; ++j)
    {
      const int          fi = j == 0 ? h.fa : h.fb;
      const TopoDS_Face& f  = TopoDS::Face(faces(fi));
      const gp_Pnt       c  = ax.Location().Translated(gp_Vec(ax.Direction()) * (j == 0 ? lo : hi));
      Bnd_Box            cb;
      cb.Add(c);
      BRepClass_FaceClassifier fc(f, c, tol);
      if (fc.State() != TopAbs_IN)
        return no(15);
      for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
      {
        Bnd_Box eb;
        BRepBndLib::Add(e.Current(), eb, false);
        if (eb.Distance(cb) > r + 10 * tol)
          continue;
        BRepExtrema_DistShapeShape dist(BRepBuilderAPI_MakeVertex(c).Vertex(), e.Current());
        if (!dist.IsDone() || dist.Value() <= r + 10 * tol)
          return no(16);
      }
    }
    for (int fi : {h.fa, h.fb})
    {
      if (!on_face.IsBound(fi))
        on_face.Bind(fi, {});
      on_face.ChangeFind(fi).push_back(k);
    }
  }
  // Holes apart from each other.
  {
    std::vector<std::pair<double, size_t>> order;
    gp_Dir                                 x = gp_Ax2(gp_Pnt(0, 0, 0), d).XDirection();
    for (size_t k = 0; k < holes.size(); ++k)
      order.emplace_back(gp_Vec(holes[k].cyl->Location().XYZ()).Dot(gp_Vec(x)), k);
    std::sort(order.begin(), order.end());
    for (size_t i = 0; i < order.size(); ++i)
      for (size_t j = i + 1; j < order.size(); ++j)
      {
        const Hole& a = holes[order[i].second];
        const Hole& b = holes[order[j].second];
        if (order[j].first - order[i].first > a.cyl->Radius() + b.cyl->Radius() + 10 * tol)
          break;
        const double dd = gp_Lin(a.cyl->Axis()).Distance(b.cyl->Location());
        if (dd <= a.cyl->Radius() + b.cyl->Radius() + 10 * tol && (a.fa == b.fa || a.fb == b.fb || a.fa == b.fb || a.fb == b.fa))
          return no(17);
      }
  }

  // The walls: the tools' cylinders between the two faces, facing the axis.
  BRep_Builder B;
  for (Hole& h : holes)
  {
    BRepBuilderAPI_MakeFace mf(h.cyl, 0, 2 * M_PI, h.va, h.vb, tol);
    if (!mf.IsDone())
      return no(18);
    TopoDS_Face wall = mf.Face();
    for (TopExp_Explorer e(wall, TopAbs_EDGE); e.More(); e.Next())
    {
      const TopoDS_Edge& ed = TopoDS::Edge(e.Current());
      if (BRep_Tool::IsClosed(ed, wall))
        continue;
      BRepAdaptor_Curve c(ed);
      if (c.GetType() != GeomAbs_Circle)
        return no(19);
      const double v = gp_Vec(h.cyl->Location(), c.Circle().Location()).Dot(gp_Vec(h.cyl->Position().Direction()));
      (std::abs(v - h.va) < std::abs(v - h.vb) ? h.ea : h.eb) = ed;
    }
    if (h.ea.IsNull() || h.eb.IsNull())
      return no(20);
    // Outward from the material: towards the axis.
    BRepAdaptor_Surface s(wall);
    gp_Pnt              p;
    gp_Vec              du, dv;
    s.D1(0.5, 0.5 * (h.va + h.vb), p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (wall.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    if (n.Dot(gp_Vec(gp_Lin(h.cyl->Axis()).Normal(p).Direction())) > 0)
      wall.Reverse();
    h.wall = wall;
  }

  // The two faces of each hole with its circles as inner loops.
  HHist                                                                   h = new BRepTools_History();
  NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher> replaced;
  for (NCollection_DataMap<int, std::vector<size_t>>::Iterator it(on_face); it.More(); it.Next())
  {
    const TopoDS_Face& f   = TopoDS::Face(faces(it.Key()));
    const TopoDS_Face  fwd = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
    TopoDS_Face        nfc = TopoDS::Face(fwd.EmptyCopied());
    for (TopoDS_Iterator w(fwd); w.More(); w.Next())
      B.Add(nfc, w.Value());
    for (size_t k : it.Value())
    {
      Hole&              ho = holes[k];
      const TopoDS_Edge& e  = it.Key() == ho.fa ? ho.ea : ho.eb;
      BRepLib::BuildPCurveForEdgeOnPlane(e, nfc);
      // The loop runs against the wall's use of the circle (one closed,
      // oriented shell), in the face's own (forward) terms.
      TopAbs_Orientation in_wall = TopAbs_FORWARD;
      for (TopExp_Explorer x(ho.wall, TopAbs_EDGE); x.More(); x.Next())
        if (x.Current().IsSame(e))
          in_wall = x.Current().Orientation();
      const TopAbs_Orientation against = TopAbs::Reverse(in_wall);
      TopoDS_Wire              w;
      B.MakeWire(w);
      B.Add(w, e.Oriented(f.Orientation() == TopAbs_REVERSED ? TopAbs::Reverse(against) : against));
      w.Closed(true);
      B.Add(nfc, w);
    }
    nfc.Orientation(f.Orientation());
    replaced.Bind(f, nfc);
    h->AddModified(f, nfc);
  }
  TopoDS_Shell ns;
  B.MakeShell(ns);
  for (TopoDS_Iterator it(shell); it.More(); it.Next())
  {
    const TopoDS_Shape* r = replaced.Seek(it.Value());
    B.Add(ns, r != nullptr ? r->Oriented(it.Value().Orientation()) : it.Value());
  }
  for (const Hole& ho : holes)
  {
    B.Add(ns, ho.wall);
    h->AddModified(ho.side, ho.wall);
    for (const TopoDS_Shape& cap : ho.caps)
      h->Remove(cap);
    h->AddGenerated(TopoDS::Face(faces(ho.fa)), ho.ea);
    h->AddGenerated(TopoDS::Face(faces(ho.fb)), ho.eb);
  }
  ns.Closed(true);
  TopoDS_Solid so;
  B.MakeSolid(so);
  B.Add(so, ns.Oriented(shell.Orientation()));
  so.Orientation(solid.Orientation());
  if (!closed_and_oriented(so))
    return no(21);
  out  = so;
  hist = h;
  return true;
}


// A tool standing on a planar face of the target (a boss extruded from a
// sketch on the face): one of its planar faces lies in the face's plane,
// facing it, inside it and clear of its edges, and the rest of the tool is
// on the face's outer side touching nothing else. The fuse is the target
// with the cap's outline as a new inner loop of the face and the tool's
// other faces added (no boolean: live's 30-feature part spent ~6 ms of CPU
// in each boss's). False when it doesn't apply; the caller fuses as usual.
static bool fuse_on_face(const TopoDS_Shape& target, const OcShape* src, const TopoDS_Shape& tool, TopoDS_Shape& out, HHist& hist)
{
  auto no = [](int why) {
    if (std::getenv("NOSHAPE_BOOL_DEBUG"))
      std::fprintf(stderr, "fuse on face: no (%d)\n", why);
    return false;
  };
  auto only = [](const TopoDS_Shape& s, TopAbs_ShapeEnum k, TopoDS_Shape& one) {
    int n = 0;
    for (TopExp_Explorer e(s, k); e.More(); e.Next(), ++n)
      one = e.Current();
    return n == 1;
  };
  TopoDS_Shape solid, shell, tsolid, tshell;
  if (!only(target, TopAbs_SOLID, solid) || !only(solid, TopAbs_SHELL, shell) || !only(tool, TopAbs_SOLID, tsolid) || !only(tsolid, TopAbs_SHELL, tshell))
    return no(1);
  // The target's faces and their boxes: its OcShape's when it has one.
  if (src != nullptr && !src->shape.IsSame(target))
    src = nullptr;
  ShapeIMap own_faces;
  if (src == nullptr)
    TopExp::MapShapes(solid, TopAbs_FACE, own_faces);
  const ShapeIMap&     faces = src != nullptr ? src->map(TopAbs_FACE) : own_faces;
  std::vector<Bnd_Box> boxes;
  if (src != nullptr)
    boxes = src->face_boxes();
  else
  {
    boxes.resize((size_t)faces.Extent());
    for (int i = 1; i <= faces.Extent(); ++i)
      face_box(faces(i), boxes[(size_t)i - 1]);
  }
  Bnd_Box all;
  for (const Bnd_Box& b : boxes)
    all.Add(b);
  BRepBndLib::Add(tsolid, all, false);
  if (all.IsVoid())
    return no(2);
  const double tol = 1e-7 * std::max(1.0, std::sqrt(all.SquareExtent()));
  auto outward = [](const TopoDS_Face& f, gp_Pln& pl) {
    BRepAdaptor_Surface s(f, false);
    if (s.GetType() != GeomAbs_Plane)
      return false;
    pl       = s.Plane();
    gp_Dir n = pl.Position().Direct() ? pl.Axis().Direction() : pl.Axis().Direction().Reversed();
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    pl = gp_Pln(pl.Location(), n);
    return true;
  };
  // The tool's cap on a face of the target (each target face's plane found
  // once).
  std::vector<int8_t> planar((size_t)faces.Extent(), -1);
  std::vector<gp_Pln> planes((size_t)faces.Extent());
  TopoDS_Face         cap, face;
  gp_Pln              fpl;
  for (TopExp_Explorer e(tsolid, TopAbs_FACE); e.More() && face.IsNull(); e.Next())
  {
    const TopoDS_Face& c = TopoDS::Face(e.Current());
    gp_Pln             cpl;
    if (!outward(c, cpl))
      continue;
    int nw = 0;
    for (TopoDS_Iterator w(c); w.More(); w.Next())
      ++nw;
    if (nw != 1)
      continue;
    Bnd_Box cb;
    BRepBndLib::Add(c, cb, false);
    for (int i = 1; i <= faces.Extent(); ++i)
    {
      const TopoDS_Face& f = TopoDS::Face(faces(i));
      int8_t&            known = planar[(size_t)i - 1];
      if (known < 0)
        known = outward(f, planes[(size_t)i - 1]) ? 1 : 0;
      const gp_Pln& pl = planes[(size_t)i - 1];
      if (known == 1 && pl.Axis().Direction().Dot(cpl.Axis().Direction()) < -1 + 1e-12 && pl.Distance(cpl.Location()) <= tol)
      {
        if (boxes[(size_t)i - 1].IsOut(cb))
          continue;
        cap  = c;
        face = f;
        fpl  = pl;
        break;
      }
    }
  }
  if (face.IsNull())
    return no(3);
  const gp_Dir n(fpl.Axis().Direction());
  // The rest of the tool on the face's outer side.
  for (TopExp_Explorer v(tsolid, TopAbs_VERTEX); v.More(); v.Next())
    if (gp_Vec(fpl.Location(), BRep_Tool::Pnt(TopoDS::Vertex(v.Current()))).Dot(gp_Vec(n)) < -tol)
      return no(4);
  // Nothing of the target but the face near the tool (boxes), the cap inside
  // the face and clear of its edges.
  Bnd_Box tb;
  BRepBndLib::Add(tsolid, tb, false);
  tb.Enlarge(tol);
  for (int i = 1; i <= faces.Extent(); ++i)
    if (!faces(i).IsSame(face) && !boxes[(size_t)i - 1].IsOut(tb))
      return no(5);
  {
    TopExp_Explorer v(cap, TopAbs_VERTEX);
    gp_Pnt          p;
    if (v.More())
      p = BRep_Tool::Pnt(TopoDS::Vertex(v.Current()));
    else
    {
      TopExp_Explorer ed(cap, TopAbs_EDGE);
      if (!ed.More())
        return no(6);
      BRepAdaptor_Curve c(TopoDS::Edge(ed.Current()));
      p = c.Value(c.FirstParameter());
    }
    BRepClass_FaceClassifier fc(face, p, tol);
    if (fc.State() != TopAbs_IN)
      return no(7);
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next())
    {
      Bnd_Box eb;
      BRepBndLib::Add(e.Current(), eb, false);
      if (eb.IsOut(tb))
        continue;
      BRepExtrema_DistShapeShape dist(cap, e.Current());
      if (!dist.IsDone() || dist.Value() <= 10 * tol)
        return no(8);
    }
  }
  // The face with the cap's outline as an inner loop, run against the
  // tool's own use of its edges (the shell stays closed and oriented).
  BRep_Builder      B;
  const TopoDS_Face fwd = TopoDS::Face(face.Oriented(TopAbs_FORWARD));
  TopoDS_Face       nf  = TopoDS::Face(fwd.EmptyCopied());
  for (TopoDS_Iterator w(fwd); w.More(); w.Next())
    B.Add(nf, w.Value());
  TopoDS_Wire hole;
  B.MakeWire(hole);
  for (TopExp_Explorer e(cap, TopAbs_EDGE); e.More(); e.Next())
  {
    // e as the cap uses it; the face uses it as the cap does (the cap and
    // the face face each other), in the face's forward terms.
    const TopoDS_Edge ed = TopoDS::Edge(e.Current());
    BRepLib::BuildPCurveForEdgeOnPlane(ed, nf);
    B.Add(hole, face.Orientation() == TopAbs_REVERSED ? ed.Reversed() : TopoDS_Shape(ed));
  }
  hole.Closed(true);
  B.Add(nf, hole);
  nf.Orientation(face.Orientation());
  TopoDS_Shell ns;
  B.MakeShell(ns);
  for (TopoDS_Iterator it(shell); it.More(); it.Next())
    B.Add(ns, it.Value().IsSame(face) ? nf.Oriented(it.Value().Orientation()) : it.Value());
  for (TopoDS_Iterator it(tshell); it.More(); it.Next())
    if (!it.Value().IsSame(cap))
      B.Add(ns, it.Value());
  ns.Closed(true);
  TopoDS_Solid so;
  B.MakeSolid(so);
  B.Add(so, ns.Oriented(shell.Orientation()));
  so.Orientation(solid.Orientation());
  if (!closed_and_oriented(so))
    return no(9);
  HHist h = new BRepTools_History();
  h->AddModified(face, nf);
  h->Remove(cap);
  out  = so;
  hist = h;
  return true;
}

// A Boolean of one small tool with a large target, done where they meet
// (docs/BOOLEAN-SPEEDUP.md phase 2): the target's faces the tool's box
// reaches and one ring of their neighbours go through OCCT's general fuse
// with the tool; the pieces are kept or dropped by where they lie (a target
// piece against the tool, a tool piece against the target, a piece the two
// share by whether their faces point the same way); every other face stays as
// it is (Kept). OCCT's Boolean walks the whole part several times (~1 ms a
// face: a fin fused to the 400-fin plate's 1600 faces took 1.8 s). False
// when it doesn't apply (NOSHAPE_BOOL_LOCAL=0: never); the caller runs the
// whole Boolean.
static bool local_boolean(const TopoDS_Shape& target, const OcShape* src, const TopoDS_Shape& tool, int op, TopoDS_Shape& out, HHist& hist, std::shared_ptr<const UnifyScope>& scope, std::array<ShapeIMap, 3>& out_maps)
{
  auto no = [](int why) {
    if (std::getenv("NOSHAPE_BOOL_DEBUG"))
      std::fprintf(stderr, "local boolean: no (%d)\n", why);
    return false;
  };
  if (op != 0 && op != 1)
    return no(1);
  {
    const char* off = std::getenv("NOSHAPE_BOOL_LOCAL");
    if (off != nullptr && std::string(off) == "0")
      return no(2);
  }
  auto only = [](const TopoDS_Shape& x, TopAbs_ShapeEnum k, TopoDS_Shape& one) {
    int n = 0;
    for (TopExp_Explorer it(x, k); it.More(); it.Next(), ++n)
      one = it.Current();
    return n == 1;
  };
  TopoDS_Shape solid, shell, tsolid;
  if (!only(target, TopAbs_SOLID, solid) || !only(solid, TopAbs_SHELL, shell) || !only(tool, TopAbs_SOLID, tsolid))
    return no(3);
  std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("local.maps");
  // The target's maps and face boxes: its OcShape's (the regeneration's
  // naming and auto scope made them), else built here.
  if (src != nullptr && !src->shape.IsSame(target))
    src = nullptr;
  ShapeIMap own_faces;
  if (src == nullptr)
    TopExp::MapShapes(solid, TopAbs_FACE, own_faces);
  const ShapeIMap& faces = src != nullptr ? src->map(TopAbs_FACE) : own_faces;
  int tool_faces = 0;
  for (TopExp_Explorer it(tsolid, TopAbs_FACE); it.More(); it.Next())
    ++tool_faces;
  // Worth it only on a part much bigger than the tool.
  if (faces.Extent() < 200 || faces.Extent() < 8 * tool_faces)
    return no(4);
  std::vector<Bnd_Box> boxes;
  if (src != nullptr)
    boxes = src->face_boxes();
  else
  {
    boxes.resize((size_t)faces.Extent());
    for (int i = 1; i <= faces.Extent(); ++i)
      face_box(faces(i), boxes[(size_t)i - 1]);
  }
  Bnd_Box all;
  for (const Bnd_Box& b : boxes)
    all.Add(b);
  const double tol = 1e-7 * std::max(1.0, std::sqrt(all.SquareExtent()));
  Bnd_Box tb;
  BRepBndLib::Add(tsolid, tb, false);
  tb.Enlarge(10 * tol);
  // Edge -> faces of the target.
  ShapeAncMap ef;
  const ShapeIMap* edges = nullptr;
  const std::pair<std::vector<uint32_t>, std::vector<uint32_t>>* csr = nullptr;
  if (src != nullptr)
  {
    edges = &src->map(TopAbs_EDGE);
    csr   = &ancestors_cached(*src, TopAbs_EDGE, TopAbs_FACE);
  }
  else
    unique_ancestors(solid, TopAbs_EDGE, TopAbs_FACE, ef);
  auto faces_of = [&](const TopoDS_Shape& e, auto&& f) {
    if (csr == nullptr)
    {
      if (const ShapeList* fl = ef.Seek(e))
        for (ShapeList::Iterator it(*fl); it.More(); it.Next())
          f(it.Value());
      return;
    }
    const int k = edges->FindIndex(e);
    if (k > 0)
      for (uint32_t j = csr->first[(size_t)k - 1]; j < csr->first[(size_t)k]; ++j)
        f(faces((int)csr->second[j] + 1));
  };
  // The faces the tool reaches, and their neighbours.
  ShapeIMap reached, region;
  for (int i = 1; i <= faces.Extent(); ++i)
    if (!boxes[(size_t)i - 1].IsOut(tb))
      reached.Add(faces(i));
  if (reached.Extent() == 0 || reached.Extent() * 4 > faces.Extent())
    return no(5);
  // Big planar faces the tool only touches stay out of the general fuse
  // (rebuilding the 400-fin plate's 800-edge front and back faces was most
  // of the time, and their ring of neighbours is the whole part): the tool
  // whole box on their inner side, no face of it lying on them over an area. Their
  // edges the fuse splits are put back below.
  // The corners of the tool's box (a curved tool bulges past its vertices).
  std::vector<gp_Pnt> tv;
  {
    double x0, y0, z0, x1, y1, z1;
    tb.Get(x0, y0, z0, x1, y1, z1);
    for (int c = 0; c < 8; ++c)
      tv.emplace_back(c & 1 ? x1 : x0, c & 2 ? y1 : y0, c & 4 ? z1 : z0);
  }
  auto out_of_fuse = [&](const TopoDS_Face& f) {
    int n = 0;
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More() && n < 64; e.Next())
      ++n;
    gp_Pln pl;
    if (n < 64 || !outer_plane(f, pl))
      return false;
    for (const gp_Pnt& p : tv)
      if (gp_Vec(pl.Location(), p).Dot(gp_Vec(pl.Axis().Direction())) > 20 * tol)
        return false;
    Bnd_Box fb;
    BRepBndLib::Add(f, fb, false);
    for (TopExp_Explorer x(tsolid, TopAbs_FACE); x.More(); x.Next())
    {
      gp_Pln tp;
      if (!outer_plane(TopoDS::Face(x.Current()), tp) || !tp.Axis().Direction().IsParallel(pl.Axis().Direction(), 1e-9) || pl.Distance(tp.Location()) > 10 * tol)
        continue;
      // On the plane: overlapping in a line at most.
      Bnd_Box xb;
      BRepBndLib::Add(x.Current(), xb, false);
      double a0, b0, c0, a1, b1, c1, d0, e0, g0, d1, e1, g1;
      fb.Get(a0, b0, c0, a1, b1, c1);
      xb.Get(d0, e0, g0, d1, e1, g1);
      const double ox = std::min(a1, d1) - std::max(a0, d0), oy = std::min(b1, e1) - std::max(b0, e0), oz = std::min(c1, g1) - std::max(c0, g0);
      int thick = 0;
      for (double o : {ox, oy, oz})
        thick += o > 100 * tol ? 1 : 0;
      if (thick >= 2)
        return false;
    }
    return true;
  };
  ShapeIMap left_out;
  auto      in_fuse = [&](const TopoDS_Shape& f) {
    if (left_out.Contains(f))
      return false;
    if (region.Contains(f))
      return true;
    if (out_of_fuse(TopoDS::Face(f)))
    {
      left_out.Add(f);
      return false;
    }
    return true;
  };
  for (int i = 1; i <= reached.Extent(); ++i)
  {
    if (!in_fuse(reached(i)))
      continue;
    region.Add(reached(i));
    for (TopExp_Explorer e(reached(i), TopAbs_EDGE); e.More(); e.Next())
      faces_of(e.Current(), [&](const TopoDS_Shape& x) {
        if (in_fuse(x))
          region.Add(x);
      });
  }
  if (region.Extent() == 0)
    return no(5);
  // The region's boundary: edges it shares with the faces left alone.
  ShapeIMap boundary;
  for (int i = 1; i <= region.Extent(); ++i)
    for (TopExp_Explorer e(region(i), TopAbs_EDGE); e.More(); e.Next())
      faces_of(e.Current(), [&](const TopoDS_Shape& x) {
        if (!region.Contains(x))
          boundary.Add(e.Current());
      });
  lap = std::make_unique<KernelTimer>("local.gf");
  // OCCT's general fuse of the region's faces and the tool.
  BRep_Builder    B;
  TopoDS_Compound rc;
  B.MakeCompound(rc);
  for (int i = 1; i <= region.Extent(); ++i)
    B.Add(rc, region(i));
  BOPAlgo_Builder gf;
  {
    ShapeList args;
    args.Append(rc);
    args.Append(tsolid);
    gf.SetArguments(args);
  }
  gf.SetNonDestructive(true);
  gf.SetRunParallel(false);
  gf.SetToFillHistory(true);
  gf.Perform();
  if (gf.HasErrors())
    return no(6);
  // Outside faces whose boundary edges the fuse split: the pieces in place
  // of the edge (only planar faces kept out above can have them).
  NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher> patched;
  {
    occ::handle<IntTools_Context> ctx = new IntTools_Context();
    ShapeIMap                     touched;
    for (int i = 1; i <= boundary.Extent(); ++i)
      if (!gf.Modified(boundary(i)).IsEmpty())
        faces_of(boundary(i), [&](const TopoDS_Shape& x) {
          if (!region.Contains(x))
            touched.Add(x);
        });
    for (int i = 1; i <= touched.Extent(); ++i)
    {
      const TopoDS_Face& f   = TopoDS::Face(touched(i));
      gp_Pln             pl;
      if (!outer_plane(f, pl))
        return no(7);
      const TopoDS_Face fwd = TopoDS::Face(f.Oriented(TopAbs_FORWARD));
      TopoDS_Face       nf  = TopoDS::Face(fwd.EmptyCopied());
      for (TopoDS_Iterator w(fwd); w.More(); w.Next())
      {
        TopoDS_Wire nw;
        B.MakeWire(nw);
        for (TopoDS_Iterator x(w.Value()); x.More(); x.Next())
        {
          const ShapeList& im = gf.Modified(x.Value());
          if (im.IsEmpty())
          {
            B.Add(nw, x.Value());
            continue;
          }
          for (ShapeList::Iterator it(im); it.More(); it.Next())
          {
            TopoDS_Edge piece = TopoDS::Edge(it.Value());
            piece.Orientation(TopAbs_FORWARD);
            if (BOPTools_AlgoTools::IsSplitToReverse(piece, TopoDS::Edge(x.Value().Oriented(TopAbs_FORWARD)), ctx))
              piece.Reverse();
            if (x.Value().Orientation() == TopAbs_REVERSED)
              piece.Reverse();
            BRepLib::BuildPCurveForEdgeOnPlane(piece, nf);
            B.Add(nw, piece);
          }
        }
        nw.Closed(true);
        B.Add(nf, nw);
      }
      nf.Orientation(f.Orientation());
      patched.Bind(f, nf);
    }
  }
  auto images = [&](const TopoDS_Shape& f) {
    ShapeList l = gf.Modified(f);
    if (l.IsEmpty() && !gf.IsDeleted(f))
      l.Append(f);
    return l;
  };
  lap = std::make_unique<KernelTimer>("local.pieces");
  // Pieces of the tool's faces.
  ShapeIMap of_tool;
  for (TopExp_Explorer x(tsolid, TopAbs_FACE); x.More(); x.Next())
  {
    const ShapeList im = images(x.Current());
    for (ShapeList::Iterator it(im); it.More(); it.Next())
      of_tool.Add(it.Value());
  }
  // Pieces both sides share (coincident faces).
  ShapeIMap of_target;
  for (int i = 1; i <= region.Extent(); ++i)
  {
    const ShapeList im = images(region(i));
    for (ShapeList::Iterator it(im); it.More(); it.Next())
      of_target.Add(it.Value());
  }
  auto normal_of = [](const TopoDS_Face& f, const gp_Pnt& p) {
    BRepAdaptor_Surface s(f);
    double              u, v;
    GeomAPI_ProjectPointOnSurf pr(p, BRep_Tool::Surface(f));
    if (pr.NbPoints() == 0)
      return gp_Vec();
    pr.LowerDistanceParameters(u, v);
    gp_Pnt q;
    gp_Vec du, dv;
    s.D1(u, v, q, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (f.Orientation() == TopAbs_REVERSED)
      n.Reverse();
    return n;
  };
  occ::handle<IntTools_Context> context = new IntTools_Context();
  auto                          inner_point = [&](const TopoDS_Face& f, gp_Pnt& p) {
    gp_Pnt2d uv;
    return BOPTools_AlgoTools3D::PointInFace(f, p, uv, context) == 0;
  };
  BRepClass3d_SolidClassifier in_tool(tsolid);
  // Inside the target: rays (OCCT's solid classifier traced every face of
  // the 1600-face part for each point, 1.6 s; loading all 1600 into its shape
  // intersector, 40 ms).
  RayClassifier target_rays(faces, std::move(boxes), tol, 10 * tol);
  // The target's faces' pieces.
  NCollection_DataMap<TopoDS_Shape, ShapeList, TopTools_ShapeMapHasher> kept;
  ShapeIMap                                                             keep;
  for (int i = 1; i <= region.Extent(); ++i)
  {
    const TopoDS_Face& f = TopoDS::Face(region(i));
    ShapeList          k;
    const ShapeList    im = images(f);
    for (ShapeList::Iterator it(im); it.More(); it.Next())
    {
      const TopoDS_Face piece = TopoDS::Face(it.Value());
      // A piece never crosses the tool's boundary: with a vertex out of the
      // tool's box it lies outside the tool (no point inside a face of 800
      // edges to look for).
      if (!of_tool.Contains(piece))
      {
        bool out_of_box = false;
        for (TopExp_Explorer v(piece, TopAbs_VERTEX); v.More() && !out_of_box; v.Next())
          out_of_box = tb.IsOut(BRep_Tool::Pnt(TopoDS::Vertex(v.Current())));
        if (out_of_box)
        {
          k.Append(piece);
          keep.Add(piece);
          continue;
        }
      }
      gp_Pnt p;
      if (!inner_point(piece, p))
        return no(8);
      bool shared = false, same = false;
      for (TopExp_Explorer x(tsolid, TopAbs_FACE); x.More() && !shared; x.Next())
      {
        const ShapeList tim = images(x.Current());
        for (ShapeList::Iterator jt(tim); jt.More(); jt.Next())
          if (jt.Value().IsSame(piece))
          {
            shared = true;
            same   = normal_of(f, p).Dot(normal_of(TopoDS::Face(x.Current()), p)) > 0;
          }
      }
      bool take;
      if (shared)
        take = op == 0 ? same : !same;
      else
      {
        in_tool.Perform(p, tol);
        if (in_tool.State() == TopAbs_ON)
          return no(9);
        take = in_tool.State() == TopAbs_OUT;
      }
      if (take)
      {
        const TopoDS_Shape oriented = piece;
        k.Append(oriented);
        keep.Add(oriented);
      }
    }
    kept.Bind(f, k);
  }
  lap = std::make_unique<KernelTimer>("local.tool_pieces");
  // The tool's faces' pieces (a cut keeps those inside the target, turned).
  for (TopExp_Explorer x(tsolid, TopAbs_FACE); x.More(); x.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(x.Current());
    ShapeList          k;
    const ShapeList    im = images(f);
    for (ShapeList::Iterator it(im); it.More(); it.Next())
    {
      if (of_target.Contains(it.Value()))
        continue; // shared: decided with the target's
      const TopoDS_Face piece = TopoDS::Face(it.Value());
      gp_Pnt            p;
      if (!inner_point(piece, p))
        return no(10);
      const TopAbs_State st = target_rays.Perform(p);
      if (st != TopAbs_IN && st != TopAbs_OUT)
        return no(11);
      const bool take = op == 0 ? st == TopAbs_OUT : st == TopAbs_IN;
      if (take)
      {
        const TopoDS_Shape oriented = op == 0 ? TopoDS_Shape(piece) : piece.Reversed();
        k.Append(oriented);
        keep.Add(oriented);
      }
    }
    kept.Bind(f, k);
  }
  lap = std::make_unique<KernelTimer>("local.shell");
  // The shell: the faces left alone and the pieces kept.
  TopoDS_Shell ns;
  B.MakeShell(ns);
  for (TopoDS_Iterator it(shell); it.More(); it.Next())
    if (!region.Contains(it.Value()))
    {
      const TopoDS_Shape* p = patched.Seek(it.Value());
      B.Add(ns, p != nullptr ? p->Oriented(it.Value().Orientation()) : it.Value());
    }
  for (int i = 1; i <= keep.Extent(); ++i)
    B.Add(ns, keep(i));
  ns.Closed(true);
  TopoDS_Solid so;
  B.MakeSolid(so);
  B.Add(so, ns.Oriented(shell.Orientation()));
  so.Orientation(solid.Orientation());
  if (!closed_and_oriented(so))
    return no(12);
  // As OCCT's own Boolean: no BRepCheck of the result (the 400-fin plate's
  // front face, 800 edges, took ~1 s).
  lap = std::make_unique<KernelTimer>("local.history");
  // History: OCCT's general fuse's, kept to what the result holds.
  // The result's maps (make_result's history keeps them for the result).
  std::array<ShapeIMap, 3> rm;
  map_fev(so, rm[0], rm[1], rm[2]);
  struct InResult
  {
    const std::array<ShapeIMap, 3>& m;
    bool Contains(const TopoDS_Shape& x) const
    {
      const TopAbs_ShapeEnum t = x.ShapeType();
      return t == TopAbs_FACE ? m[0].Contains(x) : t == TopAbs_EDGE ? m[1].Contains(x) : t == TopAbs_VERTEX && m[2].Contains(x);
    }
  } in_result{rm};
  HHist                     h = new BRepTools_History();
  std::vector<TopoDS_Shape> inputs;
  for (int i = 1; i <= region.Extent(); ++i)
    inputs.push_back(region(i));
  inputs.push_back(tsolid);
  ShapeIMap seen;
  for (const TopoDS_Shape& in : inputs)
    for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
      for (TopExp_Explorer x(in, k); x.More(); x.Next())
      {
        const TopoDS_Shape& s = x.Current();
        if (seen.Contains(s))
          continue;
        seen.Add(s);
        bool any = in_result.Contains(s);
        for (ShapeList::Iterator it(gf.Modified(s)); it.More(); it.Next())
          if (in_result.Contains(it.Value()) && !it.Value().IsSame(s))
          {
            h->AddModified(s, it.Value());
            any = true;
          }
        for (ShapeList::Iterator it(gf.Generated(s)); it.More(); it.Next())
          if (in_result.Contains(it.Value()))
            h->AddGenerated(s, it.Value());
        if (!any)
          h->Remove(s);
      }
  for (NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(patched); it.More(); it.Next())
    h->AddModified(it.Key(), it.Value());
  // As OCCT records it: both input solids replaced by the new one.
  h->Remove(solid);
  h->Remove(tsolid);
  // A target the unify found nothing to merge in: where in the result it
  // can (UnifyScope): the new faces (pieces kept, faces patched), the
  // vertices of the edges made or removed, and the faces at those.
  if (src != nullptr && src->myUnifyClean && unify_local_on())
  {
    lap                            = std::make_unique<KernelTimer>("local.unify_scope");
    auto             sc            = std::make_shared<UnifyScope>();
    const ShapeIMap& old_edges     = src->map(TopAbs_EDGE);
    ShapeIMap        local;
    for (int i = 1; i <= keep.Extent(); ++i)
      local.Add(keep(i));
    for (NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(patched); it.More(); it.Next())
      local.Add(it.Value());
    const int new_faces = local.Extent();
    for (int i = 1; i <= new_faces; ++i)
      for (TopExp_Explorer e(local(i), TopAbs_EDGE); e.More(); e.Next())
        if (!old_edges.Contains(e.Current()))
          for (TopExp_Explorer v(e.Current(), TopAbs_VERTEX); v.More(); v.Next())
            sc->seeds.Add(v.Current());
    auto removed = [&](const TopoDS_Shape& f) {
      for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
        if (!in_result.Contains(e.Current()))
          for (TopExp_Explorer v(e.Current(), TopAbs_VERTEX); v.More(); v.Next())
            sc->seeds.Add(v.Current());
    };
    for (int i = 1; i <= region.Extent(); ++i)
      removed(region(i));
    for (NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(patched); it.More(); it.Next())
      removed(it.Key());
    // A kept piece's old edges now join it to another face than before (a
    // box fused flush to the plate's end: the front face meets the box's
    // across the edge it shared with the end face): the faces across them,
    // and their vertices (two collinear edges may now share both faces).
    const auto& ef_old = ancestors_cached(*src, TopAbs_EDGE, TopAbs_FACE);
    for (int i = 1; i <= keep.Extent(); ++i)
      for (TopExp_Explorer e(keep(i), TopAbs_EDGE); e.More(); e.Next())
      {
        for (TopExp_Explorer v(e.Current(), TopAbs_VERTEX); v.More(); v.Next())
          sc->seeds.Add(v.Current());
        const int k = old_edges.FindIndex(e.Current());
        if (k > 0)
          for (uint32_t j = ef_old.first[(size_t)k - 1]; j < ef_old.first[(size_t)k]; ++j)
          {
            const TopoDS_Shape& f = faces((int)ef_old.second[j] + 1);
            if (!region.Contains(f) && !patched.IsBound(f))
              local.Add(f);
          }
      }
    // The target's faces at the seeds that the result keeps as they were.
    const auto&      vf = ancestors_cached(*src, TopAbs_VERTEX, TopAbs_FACE);
    const ShapeIMap& tv = src->map(TopAbs_VERTEX);
    for (int i = 1; i <= sc->seeds.Extent(); ++i)
    {
      const int k = tv.FindIndex(sc->seeds(i));
      if (k > 0)
        for (uint32_t j = vf.first[(size_t)k - 1]; j < vf.first[(size_t)k]; ++j)
        {
          const TopoDS_Shape& f = faces((int)vf.second[j] + 1);
          if (!region.Contains(f) && !patched.IsBound(f))
            local.Add(f);
        }
    }
    // In the result's order and orientations (as the whole unify would see them).
    BRep_Builder B2;
    B2.MakeCompound(sc->faces);
    ShapeIMap added;
    for (TopExp_Explorer x(so, TopAbs_FACE); x.More(); x.Next())
      if (local.Contains(x.Current()) && !added.Contains(x.Current()))
      {
        added.Add(x.Current());
        B2.Add(sc->faces, x.Current());
      }
    scope = sc;
  }
  out_maps = std::move(rm);
  out      = so;
  hist     = h;
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

// Tools that can't touch each other, gathered into compounds: OCCT never
// intersects the parts of one argument with each other, and most of a cut
// with many tools is spent on tool pairs whose boxes meet but which are apart
// (the user's Diagonal knurl: 74 parallel helical grooves, over 95 % of the
// candidate pairs, 29 s -> 11 s). Decided on mesh copies (the parts keep no
// triangulation): two tools share a group only when no faces of theirs come
// within twice the mesh deflection, so tools that touch or cross never do.
static std::vector<TopoDS_Shape> group_disjoint(const std::vector<TopoDS_Shape>& t)
{
  const int n = (int)t.size();
  if (n < 8)
    return t;
  Bnd_Box all;
  for (const TopoDS_Shape& s : t)
    BRepBndLib::Add(s, all, false);
  if (all.IsVoid())
    return t;
  const double defl = 1e-4 * std::sqrt(all.SquareExtent());
  TopoDS_Compound copies;
  BRep_Builder    bb;
  bb.MakeCompound(copies);
  NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> owner;
  for (int i = 0; i < n; ++i)
  {
    const TopoDS_Shape c = BRepBuilderAPI_Copy(t[(size_t)i], true, false).Shape();
    BRepMesh_IncrementalMesh(c, defl, false, 0.5);
    for (TopExp_Explorer f(c, TopAbs_FACE); f.More(); f.Next())
      owner.Bind(f.Current(), i);
    bb.Add(copies, c);
  }
  std::vector<std::vector<char>> conflict((size_t)n, std::vector<char>((size_t)n, 0));
  try
  {
    BRepExtrema_SelfIntersection si(copies, 2 * defl);
    si.Perform();
    if (!si.IsDone())
      return t;
    for (NCollection_DataMap<int, TColStd_PackedMapOfInteger>::Iterator it(si.OverlapElements()); it.More(); it.Next())
    {
      const int* a = owner.Seek(si.GetSubShape(it.Key()));
      for (TColStd_PackedMapOfInteger::Iterator j(it.Value()); j.More(); j.Next())
      {
        const int* b = owner.Seek(si.GetSubShape(j.Key()));
        if (a != nullptr && b != nullptr && *a != *b)
          conflict[(size_t)*a][(size_t)*b] = conflict[(size_t)*b][(size_t)*a] = 1;
      }
    }
  }
  catch (const Standard_Failure&)
  {
    return t;
  }
  // Greedy colouring: each tool joins the first group it touches nothing in.
  std::vector<std::vector<int>> groups;
  for (int i = 0; i < n; ++i)
  {
    auto g = std::find_if(groups.begin(), groups.end(), [&](const std::vector<int>& m) {
      return std::none_of(m.begin(), m.end(), [&](int j) { return conflict[(size_t)i][(size_t)j] != 0; });
    });
    if (g == groups.end())
      groups.push_back({i});
    else
      g->push_back(i);
  }
  if ((int)groups.size() == n)
    return t;
  std::vector<TopoDS_Shape> out;
  for (const std::vector<int>& g : groups)
  {
    if (g.size() == 1)
    {
      out.push_back(t[(size_t)g[0]]);
      continue;
    }
    TopoDS_Compound c;
    bb.MakeCompound(c);
    for (int i : g)
      bb.Add(c, t[(size_t)i]);
    out.push_back(c);
  }
  return out;
}

extern "C" void noshape_bop_set_pairs_parallel(int on); // OCCT patch 0004

void set_bop_pairs_parallel(bool on) { noshape_bop_set_pairs_parallel(on ? 1 : 0); }

// The default: half the machine's memory, 4 to 16 GB. A flat 4 GB stopped
// a diamond knurl's subtract (5.9 GB at its peak in the model's metres; lead
// 10-30 11:30: "Selected tools and targets do not intersect."), and the 17 GB
// runaway the budget is for still stops on a 16 GB machine (at 8 GB).
void set_memory_budget_mb(uint64_t mb)
{
  const uint64_t half = physical_memory_mb() / 2;
  g_memory_budget_mb.store(mb > 0 ? mb : std::min<uint64_t>(std::max<uint64_t>(half, 4096), 16384));
}

namespace
{
// The way this thread's last op_boolean went (the Boolean corpus records it).
thread_local const char* t_bool_path = "";
} // namespace

rust::String last_boolean_path()
{
  return rust::String(t_bool_path);
}

namespace
{

// The inputs of a Boolean at another scale (the blends' working scale: the
// model's metres in mm). Each TShape is scaled once and its instances keep
// their locations (scaled), so a pattern's instances still share one TShape
// (OCCT patch 0001's congruent pairs); `hist` takes every input's solids,
// faces, edges and vertices to their images. A diamond knurl's subtract of
// 148 instances grew the process by 5.9 GB in metres, 4.0 GB in mm (and
// took 325 s against 260; KNURL-MEMORY).
struct ScaledInputs
{
  struct Entry
  {
    TopoDS_Shape              copy;
    ShapeIMap                 subs;
    std::vector<TopoDS_Shape> images;
  };
  gp_Trsf                                                        S, Sinv;
  NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> index;
  std::vector<Entry>                                             entries;
  HHist                                                          hist = new BRepTools_History();

  explicit ScaledInputs(double k)
  {
    S.SetScale(gp_Pnt(0, 0, 0), k);
    Sinv.SetScale(gp_Pnt(0, 0, 0), 1.0 / k);
  }

  TopoDS_Shape scale(const TopoDS_Shape& x)
  {
    const TopLoc_Location l    = x.Location();
    const TopoDS_Shape    base = x.Located(TopLoc_Location()).Oriented(TopAbs_FORWARD);
    int                   j    = 0;
    if (const int* found = index.Seek(base))
      j = *found;
    else
    {
      BRepBuilderAPI_Transform tr(base, S, true);
      if (!tr.IsDone())
        fail_op("scaling a Boolean input failed");
      Entry e;
      e.copy = tr.Shape();
      for (TopAbs_ShapeEnum k : {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
        TopExp::MapShapes(base, k, e.subs);
      for (int i = 1; i <= e.subs.Extent(); ++i)
        e.images.push_back(tr.ModifiedShape(e.subs(i)));
      entries.push_back(std::move(e));
      j = (int)entries.size() - 1;
      index.Bind(base, j);
    }
    const Entry&          e = entries[(size_t)j];
    const TopLoc_Location k(S * l.Transformation() * Sinv);
    for (TopAbs_ShapeEnum t : {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    {
      ShapeIMap subs;
      TopExp::MapShapes(x, t, subs);
      for (int i = 1; i <= subs.Extent(); ++i)
      {
        const TopoDS_Shape& y = subs(i);
        const int           m = e.subs.FindIndex(y.Located(l.Inverted() * y.Location()));
        if (m == 0)
          continue;
        TopoDS_Shape r = e.images[(size_t)m - 1].Moved(k);
        if (y.Orientation() != e.subs(m).Orientation())
          r.Reverse();
        hist->AddModified(y, r);
      }
    }
    return e.copy.Moved(k).Oriented(x.Orientation());
  }

  // `r` (at the scaled size) back at the inputs', `h` extended to it.
  TopoDS_Shape unscale(const TopoDS_Shape& r, HHist& h) const
  {
    BRepBuilderAPI_Transform tr(r, Sinv, true);
    if (!tr.IsDone())
      fail_op("scaling a Boolean result back failed");
    if (h.IsNull())
      h = new BRepTools_History();
    h->Merge(new BRepTools_History(to_list(std::vector<TopoDS_Shape>{r}), tr));
    return tr.Shape();
  }
};

// `s` with its location put into its geometry: a fresh copy, its sub-shapes
// in the same map order (a history by index holds for both).
TopoDS_Shape bake_location(const TopoDS_Shape& s)
{
  if (s.Location().IsIdentity())
    return s;
  BRepBuilderAPI_Transform tr(s.Located(TopLoc_Location()), s.Location().Transformation(), true);
  return tr.IsDone() ? tr.Shape() : TopoDS_Shape();
}

// Whether a Boolean of solids can have come out with `r`'s volume: a union
// no bigger than its inputs together nor smaller than the biggest, a cut no
// bigger than its target, a common no bigger than either side.
bool volume_possible(uint8_t op, const std::vector<TopoDS_Shape>& a, const std::vector<TopoDS_Shape>& t, const TopoDS_Shape& r)
{
  double va = 0, vt = 0, big = 0;
  for (const TopoDS_Shape& x : a)
  {
    const double v = volume_of(x);
    va += v;
    big = std::max(big, v);
  }
  for (const TopoDS_Shape& x : t)
  {
    const double v = volume_of(x);
    vt += v;
    big = std::max(big, v);
  }
  const double v = volume_of(r), tol = 1e-7 * std::max(va + vt, 1e-12);
  switch (op)
  {
    case 0:
      return v <= va + vt + tol && v >= big - tol;
    case 1:
      return v <= va + tol;
    default:
      return v <= std::min(va, vt) + tol;
  }
}

bool all_solids(const std::vector<TopoDS_Shape>& l)
{
  for (const TopoDS_Shape& x : l)
    if (!TopExp_Explorer(x, TopAbs_SOLID).More())
      return false;
  return true;
}

// Whether `r` is what the Boolean gives for solids that don't overlap: a
// union of their volume together, a cut of the target's, an empty common.
bool no_interaction(uint8_t op, const std::vector<TopoDS_Shape>& a, const std::vector<TopoDS_Shape>& t, const TopoDS_Shape& r)
{
  double va = 0, vt = 0;
  for (const TopoDS_Shape& x : a)
    va += volume_of(x);
  for (const TopoDS_Shape& x : t)
    vt += volume_of(x);
  const double v = volume_of(r), tol = 1e-9 * std::max(va + vt, 1e-12);
  switch (op)
  {
    case 0:
      return std::abs(v - (va + vt)) <= tol;
    case 1:
      return std::abs(v - va) <= tol;
    default:
      return v <= tol;
  }
}

// Whether some solid of `t` reaches into one of `a`: a point just inside a
// face of a tool (at its middle) inside an argument.
bool tools_reach_in(const std::vector<TopoDS_Shape>& a, const std::vector<TopoDS_Shape>& t)
{
  for (const TopoDS_Shape& x : t)
  {
    Bnd_Box bx;
    BRepBndLib::Add(x, bx);
    const double eps = 1e-6 * std::sqrt(bx.SquareExtent());
    for (TopExp_Explorer fx(x, TopAbs_FACE); fx.More(); fx.Next())
    {
      const TopoDS_Face& f = TopoDS::Face(fx.Current());
      double             u0, u1, v0, v1;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      BRepGProp_Face gf(f);
      gp_Pnt         p;
      gp_Vec         n;
      gf.Normal(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, n);
      if (n.Magnitude() < 1e-12)
        continue;
      // Into the tool (BRepGProp_Face's normal is the face's outward one).
      const gp_Pnt q = p.Translated(-n.Normalized() * eps);
      if (BRepClass3d_SolidClassifier(x, q, eps * 1e-3).State() != TopAbs_IN)
        continue;
      for (const TopoDS_Shape& y : a)
        if (BRepClass3d_SolidClassifier(y, q, eps * 1e-3).State() == TopAbs_IN)
          return true;
    }
  }
  return false;
}

} // namespace

// After a Boolean that grew the process by more than 512 MB: what it freed
// handed back to the system (glibc keeps freed memory in its per-thread
// arenas, so each big Boolean started from the last one's peak: the app grew
// to 16.7 GB over a knurl document's regenerations, lead 10-30).
void trim_if_grown(size_t start)
{
#if defined(__linux__) && defined(__GLIBC__)
  const size_t now = resident_bytes();
  if (now > start && now - start > (size_t(512) << 20))
    malloc_trim(0);
#else
  (void)start;
#endif
}

struct TrimAfter
{
  size_t start = resident_bytes();
  TrimAfter() = default;
  TrimAfter(const TrimAfter&)            = delete;
  TrimAfter& operator=(const TrimAfter&) = delete;
  ~TrimAfter() { trim_if_grown(start); }
};

OpOut op_boolean(uint8_t op, const OcShapeList& args, const OcShapeList& tools,
                 const BoolOpts& opts)
{
  const TrimAfter trim_after;
  t_bool_path = "occt";
  // The inputs' OcShapes for the history (their maps are made already).
  std::vector<const OcShape*> srcs = args.sources;
  srcs.resize(args.items.size(), nullptr);
  srcs.insert(srcs.end(), tools.sources.begin(), tools.sources.end());
  if (args.items.empty())
    fail_invalid("boolean needs at least one argument");
  if (tools.items.empty())
    fail_invalid("boolean needs at least one tool");
  check_all(args.items, "boolean argument");
  check_all(tools.items, "boolean tool");
  g_over_budget.store(false);
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
  {
    const KernelTimer timer("boolean.inputs");
    for (const TopoDS_Shape& x : args.items)
      a.push_back(facet_mesh(x, facets));
    for (const TopoDS_Shape& x : tools.items)
      t.push_back(facet_mesh(x, facets));
  }
  // Many touching tools into one solid (patterns): in slabs.
  if (op == 0 && opts.glue == 3 && a.size() == 1 && facets.source.empty())
  {
    TopoDS_Shape out;
    HHist        h;
    const bool   done = fuse_slabs(a[0], t, opts, out, h);
    fail_if_cancelled();
    if (done)
      t_bool_path = "fuse_slabs";
    if (done)
      return make_result(concat(args.items, tools.items), out, h.get(), &srcs);
  }
  // A tool standing on a face of the target (a boss): built directly.
  if (op == 0 && a.size() == 1 && t.size() == 1 && facets.source.empty())
  {
    TopoDS_Shape out;
    HHist        h;
    bool         done = false;
    {
      const KernelTimer timer("boolean.on_face");
      done = fuse_on_face(a[0], args.sources.empty() ? nullptr : args.sources[0], t[0], out, h);
    }
    if (done)
      t_bool_path = "on_face";
    if (done)
      return make_result(concat(args.items, tools.items), out, h.get(), &srcs);
  }
  // One tool where it meets a large target: the faces it reaches only.
  if ((op == 0 || op == 1) && a.size() == 1 && t.size() == 1 && facets.source.empty() && opts.fuzzy <= 0 && opts.glue == 0)
  {
    TopoDS_Shape                      out;
    HHist                             h;
    std::shared_ptr<const UnifyScope> scope;
    std::array<ShapeIMap, 3>          maps;
    bool                              done = false;
    {
      const KernelTimer timer("boolean.local");
      done = local_boolean(a[0], args.sources.empty() ? nullptr : args.sources[0], t[0], op, out, h, scope, maps);
    }
    if (done)
    {
      t_bool_path = "local";
      // Where the unify (here or op_unify's) has to look (UnifyScope).
      bool clean = false;
      if (opts.unify)
        out = unify_in(out, h, scope.get(), &clean);
      std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("boolean.local_result");
      OpOut r = make_result(concat(args.items, tools.items), out, h.get(), &srcs, opts.unify ? nullptr : &maps);
      if (opts.unify)
        r.shape->myUnifyClean = clean;
      else
        r.shape->myUnifyScope = scope;
      if (!args.sources.empty() && args.sources[0] != nullptr)
        inherit_face_boxes(*args.sources[0], *r.shape);
      return r;
    }
  }
  // Holes straight through a plate: built directly.
  if (op == 1 && a.size() == 1 && facets.source.empty())
  {
    TopoDS_Shape out;
    HHist        h;
    bool         done = false;
    {
      const KernelTimer timer("boolean.through_holes");
      done = cut_through_holes(a[0], t, out, h);
    }
    if (done)
      t_bool_path = "through_holes";
    if (done)
      return make_result(concat(args.items, tools.items), out, h.get(), &srcs);
  }
  // noshape's own Boolean (crates/brep) where it takes the inputs.
  if (rust_boolean_on() && (!facets.source.empty() || opts.fuzzy > 1e-6))
    rust_decline("fuzzy or a mesh body");
  else if (rust_boolean_on())
  {
    TopoDS_Shape                          out;
    HHist                                 h;
    std::vector<const OcShape*>           rs = srcs;
    std::vector<std::unique_ptr<OcShape>> own;
    bool                                  done = false;
    {
      const KernelTimer timer("boolean.rust");
      // (A pattern's checked glue: OCCT's result is the plain Boolean's only where the instances just touch; where
      // they reach into the arguments, OCCT's is fuse_trimmed's: so is ours then, noshape's Boolean throughout.)
      const bool checked = op == 0 && opts.glue == 3 && t.size() > 1;
      if (checked)
      {
        {
          const RustLenient lenient;
          done = rust_boolean(op, concat(a, t), rs, own, out, h, opts.fuzzy, true);
        }
        if (!done && rust_last_decline() == "pattern instances overlapping")
        {
          const KernelTimer trim("boolean.rust.trimmed");
          done = rust_fuse_trimmed(a, t, opts, out, h);
        }
        if (!done)
          rust_decline(nullptr);
      }
      else
        done = rust_boolean(op, concat(a, t), rs, own, out, h, opts.fuzzy, false);
    }
    if (done)
    {
      t_bool_path = "rust";
      // A target known to have nothing to merge: the unify (here or op_unify's) looks only where it changed.
      std::shared_ptr<const UnifyScope> scope;
      if (rs[0] != nullptr && rs[0]->myUnifyClean && unify_local_on())
        scope = rust_unify_scope(*rs[0]);
      bool clean = false;
      if (opts.unify)
        out = scope ? unify_in(out, h, scope.get(), &clean) : unify(out, h);
      std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("boolean.rust.result");
      OpOut r = make_result(concat(args.items, tools.items), out, h.get(), &rs);
      lap.reset();
      if (opts.unify && scope)
        r.shape->myUnifyClean = clean;
      if (!opts.unify)
      {
        r.shape->myUnifyScope = scope;
        rust_boolean_keep(*r.shape);
      }
      return r;
    }
  }
  // Many tools apart from each other cut out of one solid (holes): in slabs.
  if (op == 1 && opts.unify && a.size() == 1 && facets.source.empty())
  {
    TopoDS_Shape out;
    HHist        h;
    const bool   done = cut_slabs(a[0], t, opts, out, h);
    fail_if_cancelled();
    if (done)
      t_bool_path = "cut_slabs";
    if (done)
      return make_result(concat(args.items, tools.items), out, h.get(), &srcs);
  }
  // Many tools (not a pattern's checked glue, which handles its own): the
  // ones apart from each other as one argument each group.
  const size_t n_tools = t.size();
  // A cut by many tools at the working scale (KNURL-MEMORY; ScaledInputs).
  std::unique_ptr<ScaledInputs> scaled;
  if (op == 1 && n_tools >= 64 && opts.glue == 0 && facets.source.empty() && working_scale() != 1.0 && std::getenv("NOSHAPE_BOOL_NO_SCALE") == nullptr)
  {
    scaled = std::make_unique<ScaledInputs>(working_scale());
    for (TopoDS_Shape& x : a)
      x = scaled->scale(x);
    for (TopoDS_Shape& x : t)
      x = scaled->scale(x);
  }
  if (opts.glue != 3 && facets.source.empty())
    t = group_disjoint(t);
  b->SetArguments(to_list(a));
  b->SetTools(to_list(t));
  BoolOpts first = opts;
  if (first.glue == 3 && op != 0) // glue is for fusing touching shapes only
    first.glue = 0;
  // Many tools (a knurl's 148 crossing grooves): OCCT's intersection of the
  // pairs runs in parallel.
  first.parallel = first.parallel || n_tools >= 8;
  apply_opts(*b, first);
  // Many tools crossing each other (a diamond knurl's two families of
  // grooves: the groups above touch): the cut is built from the faces'
  // classification. OCCT's split solids of the target and of every tool
  // (each cut up by the crossing ones) took minutes and gigabytes, and a cut
  // keeps none of the tools'.
  const bool by_faces = op == 1 && n_tools >= 8 && t.size() >= 2 && t.size() < n_tools;
  {
    const KernelTimer timer("boolean.build");
    const CutByFaces scope(by_faces);
    build_cancellable(*b);
  }
  fail_if_cancelled();
  if (by_faces && (b->HasErrors() || !b->IsDone()))
  {
    // As usual, with the split solids.
    b = make();
    b->SetArguments(to_list(a));
    b->SetTools(to_list(t));
    apply_opts(*b, first);
    build_cancellable(*b);
    fail_if_cancelled();
  }
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
      t_bool_path = "trimmed";
      res  = remesh_facets(opts.unify ? unify(res, hist) : res, facets, hist);
      return make_result(concat(args.items, tools.items), res, hist.get(), &srcs);
    }
    else
    {
      BoolOpts plain = opts;
      plain.glue     = 0;
      plain.parallel = first.parallel;
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
  {
    const KernelTimer timer("boolean.history");
    hist = new BRepTools_History(to_list(concat(a, t)), *b);
  }
  res = b->Shape();
  // The builder's data (its pave filler: most of a big Boolean's memory)
  // freed before the unify and the checks below, and handed back: a diamond
  // knurl's subtract held 8 GB through its 30 s unify.
  b.reset();
  trim_if_grown(trim_after.start);
  fail_if_cancelled();
  // A located input (a pattern's instance: moved, not copied) that OCCT got
  // wrong: a circular Part pattern x2 of a half plate whose blind hole's drill
  // tip sits on the axis came out 94 mm³ bigger than both halves and invalid;
  // with the rotation put into the copy's geometry it is right. Checked by
  // volume where that is cheap (a few tools); a result that can't be right
  // is built again on copies with their locations baked in.
  std::vector<TopoDS_Shape> inputs = concat(args.items, tools.items);
  auto located = [](const std::vector<TopoDS_Shape>& l) {
    return std::any_of(l.begin(), l.end(), [](const TopoDS_Shape& x) { return !x.Location().IsIdentity(); });
  };
  if (n_tools <= 8 && first.glue == 0 && facets.source.empty() && (located(args.items) || located(tools.items)) && all_solids(inputs)
      && !volume_possible(op, args.items, tools.items, res))
  {
    std::vector<TopoDS_Shape> a2, t2;
    for (const TopoDS_Shape& x : args.items)
      a2.push_back(bake_location(x));
    for (const TopoDS_Shape& x : tools.items)
      t2.push_back(bake_location(x));
    const bool baked = std::none_of(a2.begin(), a2.end(), [](const TopoDS_Shape& x) { return x.IsNull(); })
                       && std::none_of(t2.begin(), t2.end(), [](const TopoDS_Shape& x) { return x.IsNull(); });
    if (baked)
    {
      std::unique_ptr<BRepAlgoAPI_BooleanOperation> b2 = make();
      b2->SetArguments(to_list(a2));
      b2->SetTools(to_list(t2));
      apply_opts(*b2, first);
      build_cancellable(*b2);
      fail_if_cancelled();
      if (!b2->HasErrors() && b2->IsDone() && volume_possible(op, a2, t2, b2->Shape()))
      {
        t_bool_path = "occt_baked";
        inputs      = concat(a2, t2);
        hist        = new BRepTools_History(to_list(inputs), *b2);
        res         = b2->Shape();
      }
    }
  }
  // OCCT finding no interaction between solids that overlap (two cylinders
  // 1 mm apart standing on one plane, qa OCCT-UNION-OFFSET-CYL: the union came
  // back as the two, the cut as the target, the common empty; with their
  // bottoms 1 µm apart, right): again with a fuzzy value, kept when it does
  // interact and can be right.
  if (t_bool_path == std::string("occt") && n_tools <= 8 && first.glue == 0 && first.fuzzy <= 0 && facets.source.empty() && all_solids(inputs)
      && no_interaction(op, args.items, tools.items, res) && (tools_reach_in(args.items, tools.items) || tools_reach_in(tools.items, args.items)))
  {
    for (const double fz : {1e-7, 1e-6})
    {
      BoolOpts fuzzy = first;
      fuzzy.fuzzy    = fz;
      std::unique_ptr<BRepAlgoAPI_BooleanOperation> b2 = make();
      b2->SetArguments(to_list(args.items));
      b2->SetTools(to_list(tools.items));
      apply_opts(*b2, fuzzy);
      build_cancellable(*b2);
      fail_if_cancelled();
      if (b2->HasErrors() || !b2->IsDone() || no_interaction(op, args.items, tools.items, b2->Shape()) || !volume_possible(op, args.items, tools.items, b2->Shape()))
        continue;
      t_bool_path = "occt_fuzzy";
      inputs      = concat(args.items, tools.items);
      hist        = new BRepTools_History(to_list(inputs), *b2);
      res         = b2->Shape();
      break;
    }
  }
  if (opts.unify)
  {
    const KernelTimer timer("boolean.unify");
    res = unify(res, hist);
  }
  if (scaled)
  {
    const KernelTimer timer("boolean.unscale");
    res          = scaled->unscale(res, hist);
    HHist  whole = scaled->hist;
    whole->Merge(hist);
    hist = whole;
  }
  res = remesh_facets(res, facets, hist);
  const KernelTimer timer("boolean.result");
  return make_result(inputs, res, hist.get(), &srcs);
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

namespace
{
thread_local const char* t_unify_path = "";
} // namespace

rust::String last_unify_path() { return rust::String(t_unify_path); }

OpOut op_unify_whole(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  HHist        hist;
  TopoDS_Shape res = unify(s.shape, hist);
  return make_result({s.shape}, res, hist.get());
}

OpOut op_unify(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  // Known to have nothing to merge (an earlier unify found nothing), or
  // where to look (a local Boolean's result: UnifyScope).
  const std::vector<const OcShape*> srcs{&s};
  if (s.myUnifyClean && unify_local_on())
  {
    t_unify_path          = "known";
    OpOut r              = make_result({s.shape}, s.shape, nullptr, &srcs);
    inherit_face_boxes(s, *r.shape);
    r.shape->myUnifyClean = true;
    return r;
  }
  const UnifyScope* sc = unify_local_on() ? s.myUnifyScope.get() : nullptr;
  t_unify_path         = sc != nullptr ? "scoped" : "whole";
  HHist        hist;
  bool         clean = false;
  TopoDS_Shape res   = unify_in(s.shape, hist, sc, &clean);
  std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("unify.result");
  OpOut        r     = make_result({s.shape}, res, hist.get(), &srcs);
  r.shape->myUnifyClean = clean;
  lap = std::make_unique<KernelTimer>("unify.boxes");
  inherit_face_boxes(s, *r.shape);
  // The caller keeps the input when nothing merged: it knows too.
  if (clean && res.IsSame(s.shape))
    s.myUnifyClean = true;
  return r;
}

OpOut op_enclose(const OcShapeList& shapes)
{
  if (shapes.items.empty())
    fail_invalid("enclose needs shapes");
  check_all(shapes.items, "enclose input");
  BOPAlgo_MakerVolume mv;
  mv.SetArguments(to_list(shapes.items));
  mv.SetIntersect(true);
  mv.SetRunParallel(bop_parallel());
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

namespace
{
// A mirror of a shape (its TShape T at location L): the mirrored copy of T,
// made once per T and mirror, at M·L·M⁻¹ (a rigid motion). The mirrored
// instances of a pattern then share one TShape like the pattern's own, so a
// boolean intersects one pair of congruent instances once (OCCT patch
// 0001).
struct MirrorCopy
{
  TopoDS_Shape              base; // T at the identity (keeps T alive)
  gp_Trsf                   m;
  TopoDS_Shape              copy;
  ShapeIMap                 subs;     // base's faces, edges, vertices, solids
  std::vector<TopoDS_Shape> mirrored; // their images in copy
};

bool same_trsf(const gp_Trsf& a, const gp_Trsf& b)
{
  for (int r = 1; r <= 3; ++r)
    for (int c = 1; c <= 4; ++c)
      if (std::abs(a.Value(r, c) - b.Value(r, c)) > 1e-12)
        return false;
  return true;
}

std::shared_ptr<const MirrorCopy> mirror_copy(const TopoDS_Shape& base, const gp_Trsf& m)
{
  static std::mutex                                     mu;
  static std::vector<std::shared_ptr<const MirrorCopy>> cache;
  {
    const LockedSection locked_section_3220;
    std::lock_guard<std::mutex> g(mu);
    for (const auto& c : cache)
      if (c->base.IsEqual(base) && same_trsf(c->m, m))
        return c;
  }
  BRepBuilderAPI_Transform tr(base, m, true);
  if (!tr.IsDone())
    fail_op("transform failed");
  auto c  = std::make_shared<MirrorCopy>();
  c->base = base;
  c->m    = m;
  c->copy = tr.Shape();
  for (TopAbs_ShapeEnum k : {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    TopExp::MapShapes(base, k, c->subs);
  for (int i = 1; i <= c->subs.Extent(); ++i)
    c->mirrored.push_back(tr.ModifiedShape(c->subs(i)));
  const LockedSection locked_section_3236;
  std::lock_guard<std::mutex> g(mu);
  if (cache.size() >= 16)
    cache.erase(cache.begin());
  cache.push_back(c);
  return c;
}

OpOut mirror_instance(const TopoDS_Shape& s, const gp_Trsf& m)
{
  const TopLoc_Location             l = s.Location();
  const std::shared_ptr<const MirrorCopy> c = mirror_copy(s.Located(TopLoc_Location()), m);
  const TopLoc_Location             k(m * l.Transformation() * m.Inverted());
  HHist                             hist = new BRepTools_History();
  for (TopAbs_ShapeEnum t : {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
  {
    ShapeIMap subs;
    TopExp::MapShapes(s, t, subs);
    for (int i = 1; i <= subs.Extent(); ++i)
    {
      const TopoDS_Shape& x = subs(i);
      const int           j = c->subs.FindIndex(x.Located(l.Inverted() * x.Location()));
      if (j == 0)
        continue;
      TopoDS_Shape r = c->mirrored[(size_t)j - 1].Moved(k);
      if (x.Orientation() != c->subs(j).Orientation())
        r.Reverse();
      hist->AddModified(x, r);
    }
  }
  return make_result({s}, c->copy.Moved(k), hist.get());
}
} // namespace

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
  if (!copy && t.IsNegative() && std::abs(std::abs(t.ScaleFactor()) - 1.) <= TopLoc_Location::ScalePrec())
    return mirror_instance(s.shape, t);
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

namespace
{
// set_call_budget's: the CPU seconds a section may take (0: no limit).
thread_local double t_call_budget = 0;
} // namespace

double set_call_budget(double seconds)
{
  const double was = t_call_budget;
  t_call_budget    = seconds > 0 ? seconds : 0;
  return was;
}

static OpOut op_section_unbudgeted(const OcShape& a, const OcShape& b)
{
  require_non_null(a.shape, "first shape");
  require_non_null(b.shape, "second shape");
  BRepAlgoAPI_Section sec(a.shape, b.shape, false);
  sec.Approximation(true);
  sec.ComputePCurveOn1(true);
  sec.ComputePCurveOn2(true);
  sec.SetRunParallel(bop_parallel());
  // Its inputs as they were if a budget stops it (see run_budgeted); stopped
  // through its progress indicator off Linux.
  sec.SetNonDestructive(true);
  build_cancellable(sec);
  if (sec.HasErrors() || !sec.IsDone())
    fail_op("section failed: " + algo_errors(sec));
  std::vector<TopoDS_Shape> args{a.shape, b.shape};
  HHist                     hist = new BRepTools_History(to_list(args), sec);
  return make_result(args, sec.Shape(), hist.get());
}

// Under set_call_budget's limit when one is set (the collision checks of
// std's editing logic: a section of two of a fuzz body's curved faces took
// up to 28 s, qa MIRROR-HANG-402).
OpOut op_section(const OcShape& a, const OcShape& b)
{
  if (t_call_budget <= 0)
    return op_section_unbudgeted(a, b);
  OpOut     out;
  const int r = run_budgeted(t_call_budget, [&] { out = op_section_unbudgeted(a, b); });
  if (r == 1)
    fail_op("section took too long");
  if (r == 2)
    fail_op("cancelled");
  return out;
}

std::unique_ptr<OcShape> section_plane(const OcShape& s, const PlaneF& plane)
{
  require_non_null(s.shape, "shape");
  BRepAlgoAPI_Section sec(s.shape, to_pln(plane), false);
  sec.Approximation(true);
  sec.SetRunParallel(bop_parallel());
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

namespace noshape {
const std::atomic<bool>* thread_cancel_flag()
{
  return t_cancel;
}
} // namespace noshape
