// Booleans, split, unify, enclose, transforms, sections, projections.
#include <thread>
#include <tuple>
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
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2dConvert.hxx>
#include <GeomConvert.hxx>
#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepGProp.hxx>
#include <OSD_Parallel.hxx>
#include <BRep_Tool.hxx>
#include <Geom2d_Curve.hxx>
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
#include <GeomLib_IsPlanarSurface.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BezierCurve.hxx>
#include <Geom_ElementarySurface.hxx>
#include <Geom_SweptSurface.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_GTrsf.hxx>
#include <gp_Trsf.hxx>

#include <BRepAdaptor_Curve.hxx>
#include <NCollection_Array1.hxx>
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
#include <GeomProjLib.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <BRepTools_History.hxx>
#include <BRepTools_Modification.hxx>
#include <BRepTools_Modifier.hxx>
#include <BRepTools.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepTools_ReShape.hxx>
#include <Geom_Circle.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2dAdaptor_Curve.hxx>
#include <math.hxx>
#include <math_Vector.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRep_Builder.hxx>
#include <TColStd_PackedMapOfInteger.hxx>
#include <NCollection_DataMap.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <deque>
#include <mutex>
#include <memory>
#include <optional>
#include <algorithm>
#include <limits>
#include <ElCLib.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <TopoDS_Compound.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
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
  // (With NOSHAPE_KERNEL_PROFILE: the time to here goes to the step OCCT last showed.)
  ~CancelIndicator() override { Show(nullptr); }
  bool UserBreak() override
  {
    if (cancelled(myFlag) || myOver)
      return true;
    const auto now = std::chrono::steady_clock::now();
    // (A CPU clock is a system call: every 5 ms.)
    if (myDeadline != nullptr && now - myDeadlineChecked >= std::chrono::milliseconds(5))
    {
      myDeadlineChecked = now;
      if (deadline_now(*myDeadline) > myDeadline->at)
      {
        myDeadline->hit.store(true);
        return true;
      }
    }
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
  // With NOSHAPE_KERNEL_PROFILE: each of OCCT's named steps' time (from one progress report to the next, to the
  // innermost named step shown), as "occt: <step>".
  void Show(const Message_ProgressScope& scope, const bool) override { Show(&scope); }

private:
  void Show(const Message_ProgressScope* scope)
  {
    if (!kernel_profile_on())
      return;
    const char* name = nullptr;
    for (const Message_ProgressScope* s = scope; s != nullptr && name == nullptr; s = s->Parent())
      name = s->Name();
    const std::lock_guard<std::mutex> g(myStepLock);
    const auto                        now = kernel_profile_now();
    if (!myStep.empty())
      kernel_profile_add_named(("occt: " + myStep).c_str(), now - myStepStart);
    myStepStart = now;
    myStep      = name != nullptr ? name : "";
  }

  const std::atomic<bool>*              myFlag;
  Deadline*                             myDeadline = t_deadline;
  size_t                                myStart;
  bool                                  myOver = false;
  std::chrono::steady_clock::time_point myChecked = std::chrono::steady_clock::now();
  std::mutex                            myStepLock;
  std::string                           myStep;
  std::chrono::steady_clock::duration   myStepStart{};
  std::chrono::steady_clock::time_point myDeadlineChecked{};
};

} // namespace

// A progress indicator for an OCCT build that takes one: stopped by the
// thread's cancel flag, the memory budget and a budget's deadline.
occ::handle<Message_ProgressIndicator> new_cancel_indicator()
{
  return new CancelIndicator(t_cancel);
}

// OCCT patch 0007's hook (blend walking, the fillet builder's stripes and
// vertices, Draft's faces; they take no progress indicator): the deadline of
// a budget off Linux has passed, or the thread's cancel flag is set.
ScaledInputs::ScaledInputs(double k)
{
  S.SetScale(gp_Pnt(0, 0, 0), k);
  Sinv.SetScale(gp_Pnt(0, 0, 0), 1.0 / k);
}

TopoDS_Shape ScaledInputs::scale(const TopoDS_Shape& x)
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

TopoDS_Shape ScaledInputs::unscale(const TopoDS_Shape& r, occ::handle<BRepTools_History>& h) const
{
  BRepBuilderAPI_Transform tr(r, Sinv, true);
  if (!tr.IsDone())
    fail_op("scaling a Boolean result back failed");
  if (h.IsNull())
    h = new BRepTools_History();
  h->Merge(new BRepTools_History(to_list(std::vector<TopoDS_Shape>{r}), tr));
  return tr.Shape();
}

namespace {
// What a pool job's workers take from the thread that launched it (OCCT patch 0010): its cancel flag and CPU
// deadline, so the break hooks stop OCCT's parallel face / face intersections of a cancelled Boolean on the
// pool's threads too.
struct PoolContext
{
  const std::atomic<bool>* cancel;
  Deadline*                deadline;
};
thread_local PoolContext t_pool_context{nullptr, nullptr};
} // namespace

extern "C" void* noshape_pool_capture()
{
  if (t_cancel == nullptr && t_deadline == nullptr)
    return nullptr;
  t_pool_context = {t_cancel, t_deadline};
  return &t_pool_context;
}

extern "C" void* noshape_pool_enter(void* context)
{
  if (context == nullptr)
    return nullptr;
  PoolContext* previous = new PoolContext{t_cancel, t_deadline};
  const PoolContext* c  = static_cast<const PoolContext*>(context);
  t_cancel              = c->cancel;
  t_deadline            = c->deadline;
  return previous;
}

extern "C" void noshape_pool_leave(void* previous)
{
  if (previous == nullptr)
    return;
  const PoolContext* p = static_cast<const PoolContext*>(previous);
  t_cancel             = p->cancel;
  t_deadline           = p->deadline;
  delete p;
}

namespace {
// Where the process's resident memory stood when this thread's noshape Boolean started (0: none running), and
// whether it has grown past the budget since.
thread_local size_t t_rust_rss_start = 0;
thread_local bool   t_rust_over      = false;
} // namespace

RustMemoryWatch::RustMemoryWatch() : myPrevious(t_rust_rss_start)
{
  t_rust_rss_start = std::max<size_t>(resident_bytes(), 1);
  t_rust_over      = false;
}

RustMemoryWatch::~RustMemoryWatch() { t_rust_rss_start = myPrevious; }

bool RustMemoryWatch::over() { return t_rust_over; }

size_t RustMemoryWatch::budget_mb() { return memory_budget() >> 20; }

extern "C" int noshape_should_break()
{
  Deadline* const d = t_deadline;
  if (cancelled(t_cancel))
    return 1;
  if (t_rust_rss_start != 0)
  {
    // (A process's size is a file read on Linux: every 50 ms.)
    thread_local double next_rss = 0;
    const double        now      = steady_seconds();
    if (!t_rust_over && now >= next_rss)
    {
      next_rss         = now + 0.05;
      const size_t rss = resident_bytes();
      t_rust_over      = rss > t_rust_rss_start && rss - t_rust_rss_start > memory_budget();
    }
    if (t_rust_over)
      return 1;
  }
  if (d == nullptr)
    return 0;
  if (d->hit.load(std::memory_order_relaxed))
    return 1;
  // Called at every math solve (patch 0008): the deadline's CPU clock is a
  // system call, so read once a millisecond (the steady clock is cheap).
  thread_local double next = 0;
  const double        now  = steady_seconds();
  if (now < next)
    return 0;
  next = now + 1e-3;
  if (deadline_now(*d) > d->at)
  {
    d->hit.store(true);
    return 1;
  }
  return 0;
}

namespace {

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

// A cancelled Boolean's builder (its pave filler: most of a big Boolean's memory) freed on a thread of its own, so
// the worker is free at once (freeing the 45 / 45 knurl's took up to a second after the user's cancel).
template <class P> void free_elsewhere(P& p)
{
  try
  {
    std::thread([q = std::move(p)]() mutable { [[maybe_unused]] P r = std::move(q); }).detach();
  }
  catch (...)
  {
    // (No thread: freed by the caller as before.)
  }
}

// Hands what it holds to free_elsewhere when its scope is left after a cancel (the unwinding freed a whole knurl's
// copy, history and unify maps on the worker: half a second after the cancel). Declared after them.
template <class... P> struct FreeIfCancelled
{
  std::tuple<P&...> held;
  explicit FreeIfCancelled(P&... p) : held(p...) {}
  ~FreeIfCancelled()
  {
    if (cancelled(t_cancel))
      std::apply([](auto&... p) { (free_elsewhere(p), ...); }, held);
  }
};

} // namespace

void fail_if_cancelled()
{
  if (cancelled(t_cancel))
    fail_op("cancelled");
  if (g_over_budget.load())
    fail_op("operation stopped: it needed more than " + std::to_string(memory_budget() >> 20) + " MB of memory");
}

namespace {
// BRepTools_History's constructor from an algorithm, stopping on a cancel: on the 45 / 45 knurl's cut it ran
// for seconds over the 50 k argument shapes after the build (each AddGenerated also searches the shape's list
// of modifications), past a cancel.
template <class Algo>
HHist cancellable_history(const std::vector<TopoDS_Shape>& args, Algo& algo)
{
  ShapeIMap all;
  for (const TopoDS_Shape& x : args)
    if (!x.IsNull())
      TopExp::MapShapes(x, all);
  HHist h = new BRepTools_History();
  for (int i = 1; i <= all.Extent(); ++i)
  {
    if ((i & 0xFF) == 0)
      fail_if_cancelled();
    const TopoDS_Shape& s = all(i);
    if (!BRepTools_History::IsSupportedType(s))
      continue;
    if (algo.IsDeleted(s))
      h->Remove(s);
    int k = 0;
    for (const TopoDS_Shape& m : algo.Modified(s))
    {
      if ((++k & 0xFF) == 0)
        fail_if_cancelled();
      h->AddModified(s, m);
    }
    for (const TopoDS_Shape& g : algo.Generated(s))
    {
      if ((++k & 0xFF) == 0)
        fail_if_cancelled();
      h->AddGenerated(s, g);
    }
  }
  return h;
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
  NCollection_DataMap<const Standard_Transient*, bool> planarity;
  auto planar = [&](const occ::handle<Geom_Surface>& g) {
    if (const bool* known = planarity.Seek(g.get()))
      return *known;
    const bool r = GeomLib_IsPlanarSurface(g, Precision::Confusion()).IsPlanar();
    planarity.Bind(g.get(), r);
    return r;
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
    // Two curved faces: the same kind of surface may be one domain. Free-form ones (B-splines, ...) on different
    // surfaces only if both are planar: USD's IsSameDomain (the knurl's 10k B-spline faces made every unify of it
    // run a USD that merges nothing).
    if (ga->DynamicType() == gb->DynamicType())
    {
      if (ga->IsKind(STANDARD_TYPE(Geom_ElementarySurface)) || ga->IsKind(STANDARD_TYPE(Geom_SweptSurface)) ||
          ga->IsKind(STANDARD_TYPE(Geom_RectangularTrimmedSurface)))
        return true;
      if (planar(ga) && planar(gb))
        return true;
    }
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
      // USD joins circles of one circle and straight B-splines / Beziers (two poles) only.
      auto straight = [](const occ::handle<Geom_Curve>& c) {
        const occ::handle<Geom_BSplineCurve> b = occ::down_cast<Geom_BSplineCurve>(c);
        const occ::handle<Geom_BezierCurve>  z = occ::down_cast<Geom_BezierCurve>(c);
        return b.IsNull() && z.IsNull() ? true : (!b.IsNull() ? b->NbPoles() : z->NbPoles()) == 2;
      };
      if (c1->DynamicType() == c2->DynamicType() && straight(c1) && straight(c2))
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
  {
    if ((i & 0xFF) == 0)
      fail_if_cancelled();
    planar[(size_t)i] = outer_plane(TopoDS::Face(faces(i)), pl[(size_t)i]);
  }
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
  fail_if_cancelled();
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
    fail_if_cancelled();
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
  fail_if_cancelled();
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
    B.Add(so, ns); // (the faces as the shell's iterator gives them: already in the solid's orientation)
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
  fail_if_cancelled();
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

// The volume of faces `fs`, each as its solid uses it: a solid's volume is the sum of its faces' shares (one
// integral per face), so two solids sharing all but a few faces differ by those faces' shares, the ones of one
// reversed (in one call: OCCT integrates about the shape's rough centre, so separate calls don't add up).
double faces_volume(const std::vector<TopoDS_Shape>& fs)
{
  if (fs.empty())
    return 0;
  TopoDS_Compound c;
  BRep_Builder    B;
  B.MakeCompound(c);
  for (const TopoDS_Shape& f : fs)
    B.Add(c, f);
  GProp_GProps g;
  BRepGProp::VolumeProperties(c, g);
  return g.Mass();
}

// What a face's volume share depends on, as a value: its orientation, surface and, for each edge use in order, the
// edge, its orientation, range and curve on the face at both ends. USD edits its input in place (a notched cylinder
// fused with its mirror: the same face, the same edges, another share), so a face of its result is the input's own
// only if this didn't change.
struct FaceShare
{
  TopAbs_Orientation    orientation = TopAbs_FORWARD;
  const Standard_Transient* surface = nullptr;
  std::vector<double>       uses;
  std::vector<const void*>  edges;
  bool operator==(const FaceShare& o) const
  {
    return orientation == o.orientation && surface == o.surface && uses == o.uses && edges == o.edges;
  }
};

FaceShare face_share(const TopoDS_Shape& f)
{
  FaceShare       r;
  const TopoDS_Face face = TopoDS::Face(f);
  TopLoc_Location l;
  r.orientation = f.Orientation();
  r.surface     = BRep_Tool::Surface(face, l).get();
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(x.Current());
    r.edges.push_back(e.TShape().get());
    r.uses.push_back((double)e.Orientation());
    double a, b;
    const occ::handle<Geom2d_Curve> c = BRep_Tool::CurveOnSurface(e, face, a, b);
    r.uses.push_back(a);
    r.uses.push_back(b);
    if (c.IsNull())
      continue;
    for (double t : {a, b})
    {
      const gp_Pnt2d p = c->Value(t);
      r.uses.push_back(p.X());
      r.uses.push_back(p.Y());
    }
  }
  return r;
}

// Twice the signed area edge `e` (forward) sweeps about `o` in the plane (x, y), the boundary term of Green's
// theorem; null on a curve this doesn't integrate.
std::optional<double> swept_area(const TopoDS_Edge& e, const gp_Pnt& o, const gp_Dir& x, const gp_Dir& y)
{
  const BRepAdaptor_Curve c(e);
  const GeomAbs_CurveType t = c.GetType();
  if (t != GeomAbs_Line && t != GeomAbs_Circle && t != GeomAbs_Ellipse && t != GeomAbs_BSplineCurve && t != GeomAbs_BezierCurve)
    return std::nullopt;
  const int n = c.NbIntervals(GeomAbs_CN);
  NCollection_Array1<double> knots(1, n + 1);
  c.Intervals(knots, GeomAbs_CN);
  math_Vector gp(1, 10), gw(1, 10);
  math::GaussPoints(10, gp);
  math::GaussWeights(10, gw);
  double r = 0;
  for (int i = 1; i <= n; ++i)
  {
    const double a = knots(i), b = knots(i + 1);
    const int    pieces = (t == GeomAbs_Circle || t == GeomAbs_Ellipse) ? std::max(1, (int)std::ceil((b - a) / (M_PI / 4))) : 1;
    for (int k = 0; k < pieces; ++k)
    {
      const double u0 = a + (b - a) * k / pieces, u1 = a + (b - a) * (k + 1) / pieces;
      const double m = (u0 + u1) / 2, h = (u1 - u0) / 2;
      for (int j = 1; j <= 10; ++j)
      {
        gp_Pnt p;
        gp_Vec v;
        c.D1(m + h * gp(j), p, v);
        const gp_Vec d(o, p);
        r += gw(j) * h * (d.Dot(gp_Vec(x)) * v.Dot(gp_Vec(y)) - d.Dot(gp_Vec(y)) * v.Dot(gp_Vec(x)));
      }
    }
  }
  return r;
}

// `fs` without the faces on one plane whose volume shares cancel. A plane face's share about the integrals' centre
// is its plane's distance from it times its signed area over 3, and the area a sum over its edge uses (Green), in
// which an edge's uses both ways cancel: a face USD only re-edged against its old self differs by its changed
// edges only (cf-spur-gear's Boolean 2: the gear's caps, 261 edges, 30 ms an integral, four integrals a try).
std::vector<TopoDS_Shape> without_cancelling_planes(const std::vector<TopoDS_Shape>& fs, double diag)
{
  struct Plane
  {
    const Standard_Transient* surface;
    TopLoc_Location           loc;
    std::vector<size_t>       faces;
  };
  std::vector<Plane> planes;
  for (size_t i = 0; i < fs.size(); ++i)
  {
    TopLoc_Location                 l;
    const occ::handle<Geom_Surface> g = BRep_Tool::Surface(TopoDS::Face(fs[i]), l);
    if (g.IsNull() || g->DynamicType() != STANDARD_TYPE(Geom_Plane))
      continue;
    auto p = std::find_if(planes.begin(), planes.end(), [&](const Plane& q) { return q.surface == g.get() && q.loc.IsEqual(l); });
    if (p == planes.end())
      planes.push_back({g.get(), l, {i}});
    else
      p->faces.push_back(i);
  }
  std::vector<bool> drop(fs.size(), false);
  for (const Plane& plane : planes)
  {
    if (plane.faces.size() < 2)
      continue;
    struct Use
    {
      const Standard_Transient* curve;
      TopLoc_Location           loc;
      double                    a, b;
      int                       net;
      TopoDS_Edge               edge;
    };
    std::vector<Use> uses;
    bool             ok = true;
    for (size_t i : plane.faces)
      for (TopExp_Explorer x(fs[i], TopAbs_EDGE); ok && x.More(); x.Next())
      {
        const TopoDS_Edge& e = TopoDS::Edge(x.Current());
        if (BRep_Tool::Degenerated(e))
          continue;
        const int net = e.Orientation() == TopAbs_FORWARD ? 1 : e.Orientation() == TopAbs_REVERSED ? -1 : 0;
        TopLoc_Location l;
        double          a, b;
        const occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, l, a, b);
        if (c.IsNull())
        {
          ok = false;
          break;
        }
        auto u = std::find_if(uses.begin(), uses.end(), [&](const Use& q) { return q.curve == c.get() && q.a == a && q.b == b && q.loc.IsEqual(l); });
        if (u == uses.end())
          uses.push_back({c.get(), l, a, b, net, TopoDS::Edge(e.Oriented(TopAbs_FORWARD))});
        else
          u->net += net;
      }
    if (!ok)
      continue;
    const gp_Ax3    frame = BRepAdaptor_Surface(TopoDS::Face(fs[plane.faces[0]]), false).Plane().Position();
    double          area  = 0;
    for (const Use& u : uses)
    {
      if (u.net == 0)
        continue;
      const std::optional<double> a = swept_area(u.edge, frame.Location(), frame.XDirection(), frame.YDirection());
      if (!a)
      {
        ok = false;
        break;
      }
      area += u.net * *a;
    }
    if (!ok || std::abs(area) > 1e-13 * diag * diag)
      continue;
    for (size_t i : plane.faces)
      drop[i] = true;
  }
  std::vector<TopoDS_Shape> r;
  for (size_t i = 0; i < fs.size(); ++i)
    if (!drop[i])
      r.push_back(fs[i]);
  return r;
}

// How much USD's result `out` changed the volume of `s`, on the faces it changed only (the knurl: 14 of its
// 10,376 faces merged, two whole-solid integrals of B-spline faces were most of op_boolean's unify): the faces
// of `s` whose images in its input it removed or changed, against the faces of `out` that took their place.
// `image`: a face of `s` in USD's input as the input uses it (null: not found); `share`: its face_share before
// USD ran. Null when a face of `s` has no image: the whole volumes then.
// What a USD try changed, as a value: the faces of `s` it changed (their indices) and, for each face that took their
// place, its surface, orientation, edge count and where its vertices are. Two tries of one key changed the volume
// alike (cf-spur-gear's and pipeflange's rejected merges: the same merge from three wire starts).
struct ChangeKey
{
  std::vector<size_t>      old;
  std::vector<std::string> made;
  bool operator==(const ChangeKey& o) const { return old == o.old && made == o.made; }
};

std::optional<double> usd_volume_change(const std::vector<TopoDS_Shape>& sf, const std::vector<TopoDS_Shape>& image,
                                        const std::vector<FaceShare>& share, const TopoDS_Shape& out, double diag,
                                        std::optional<std::pair<ChangeKey, double>>* memo = nullptr)
{
  ShapeIMap outf;
  for (TopExp_Explorer x(out, TopAbs_FACE); x.More(); x.Next())
    outf.Add(x.Current());
  std::vector<TopoDS_Shape> before, after;
  ShapeIMap                 kept;
  for (size_t i = 0; i < sf.size(); ++i)
  {
    if (image[i].IsNull())
      return std::nullopt;
    const int k = outf.FindIndex(image[i]);
    if (k > 0 && face_share(outf.FindKey(k)) == share[i])
      kept.Add(image[i]);
    else
      before.push_back(sf[i]);
  }
  for (TopExp_Explorer x(out, TopAbs_FACE); x.More(); x.Next())
    if (!kept.Contains(x.Current()))
      after.push_back(x.Current());
  ChangeKey key;
  if (memo != nullptr)
  {
    for (size_t i = 0; i < sf.size(); ++i)
      if (!kept.Contains(image[i]))
        key.old.push_back(i);
    for (const TopoDS_Shape& f : after)
    {
      TopLoc_Location l;
      char            buf[160];
      double          sx = 0, sy = 0, sz = 0;
      int             nv = 0, ne = 0;
      for (TopExp_Explorer v(f, TopAbs_VERTEX); v.More(); v.Next(), ++nv)
      {
        const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(v.Current()));
        sx += p.X();
        sy += p.Y();
        sz += p.Z();
      }
      for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
        ++ne;
      // (The vertices' sum to 1e-9 of the part: the tries' vertices differ in their last bits.)
      const double q = std::max(1e-9 * diag, 1e-15);
      std::snprintf(buf, sizeof buf, "%p %d %d %d %lld %lld %lld", (const void*)BRep_Tool::Surface(TopoDS::Face(f), l).get(), (int)f.Orientation(), ne, nv, std::llround(sx / q), std::llround(sy / q), std::llround(sz / q));
      key.made.emplace_back(buf);
    }
    std::sort(key.made.begin(), key.made.end());
    if (*memo && (*memo)->first == key)
      return (*memo)->second;
  }
  for (const TopoDS_Shape& f : before)
    after.push_back(f.Reversed());
  const double change = faces_volume(without_cancelling_planes(after, diag));
  if (memo != nullptr)
    *memo = std::make_pair(key, change);
  return change;
}

TopoDS_Shape usd_kept_volume(const TopoDS_Shape& s, HHist& hist)
{
  // The volume of `s` only when the check needs it (one integral of every face: 14 s of the knurl's unify):
  // a change past 1e-9 of the box's volume is past 1e-9 of the solid's, one under 1e-12 of it is the integrals'
  // noise (the knurl's 14 merged faces: 1.0e-14 of its box).
  std::unique_ptr<KernelTimer> lap = std::make_unique<KernelTimer>("usd.box");
  Bnd_Box                      box;
  BRepBndLib::Add(s, box, false);
  const double diag = box.IsVoid() ? 0 : std::sqrt(box.SquareExtent());
  const double bv   = diag * diag * diag;
  // A bound on the solids' volume: a change past 1e-9 of it is past 1e-9 of theirs (cf-spur-gear's flat gear, its
  // rejected merges: 6.7e-8 of its volume, 1.1e-10 of its box's diagonal cubed; the volume then cost 128 ms).
  double most = std::numeric_limits<double>::infinity();
  if (!box.IsVoid() && TopExp_Explorer(s, TopAbs_SOLID).More())
  {
    double x0, y0, z0, x1, y1, z1;
    box.Get(x0, y0, z0, x1, y1, z1);
    int solids = 0;
    for (TopExp_Explorer x(s, TopAbs_SOLID); x.More(); x.Next())
      ++solids;
    most = solids * (x1 - x0) * (y1 - y0) * (z1 - z0);
  }
  lap.reset();
  std::optional<double> v0;
  auto volume0 = [&] {
    if (!v0)
    {
      const KernelTimer timer("usd.volume");
      GProp_GProps      g0;
      BRepGProp::VolumeProperties(s, g0);
      v0 = g0.Mass();
    }
    return *v0;
  };
  std::vector<TopoDS_Shape> sf;
  for (TopExp_Explorer x(s, TopAbs_FACE); x.More(); x.Next())
    sf.push_back(x.Current());
  // Each face of `s` in the copy USD gets (through `rotated`, the wires' rotation, when given) as `in` uses it,
  // and its face_share.
  auto images = [&](const BRepBuilderAPI_Copy& cp, const HHist& rotated, const TopoDS_Shape& in,
                    std::vector<TopoDS_Shape>& image, std::vector<FaceShare>& share) {
    ShapeIMap inf;
    for (TopExp_Explorer x(in, TopAbs_FACE); x.More(); x.Next())
      inf.Add(x.Current());
    image.assign(sf.size(), TopoDS_Shape());
    share.assign(sf.size(), FaceShare());
    for (size_t i = 0; i < sf.size(); ++i)
    {
      TopoDS_Shape f = cp.ModifiedShape(sf[i]);
      if (!rotated.IsNull() && !f.IsNull())
      {
        const ShapeList& m = rotated->Modified(f);
        if (!m.IsEmpty())
          f = m.First();
      }
      const int k = f.IsNull() ? 0 : inf.FindIndex(f);
      if (k == 0)
        continue;
      image[i] = inf.FindKey(k);
      share[i] = face_share(image[i]);
    }
  };
  auto keeps_volume = [&](const std::optional<double>& change, const TopoDS_Shape& out) {
    double d;
    if (change)
    {
      d = *change;
      if (std::abs(d) > 1e-9 * bv)
        return false;
      if (std::abs(d) <= 1e-12 * bv)
        return true;
      if (std::abs(d) > 1e-9 * most)
        return false;
    }
    else
    {
      GProp_GProps g1;
      BRepGProp::VolumeProperties(out, g1);
      d = g1.Mass() - volume0();
    }
    return std::abs(d) <= 1e-9 * std::abs(volume0());
  };
  // USD's result depends on the order of the faces too (a notched cylinder
  // fused with its mirror, qa OCCT-MIRROR-OVERCOUNT: one order of the same
  // faces merged to 4, the other gave 4/3 the volume from every start).
  std::optional<std::pair<int, double>> last_reject;
  std::optional<std::pair<ChangeKey, double>> memo;
  int                                   repeats = 0;
  for (int k = 0; k < 16; ++k)
  {
    fail_if_cancelled();
    lap = std::make_unique<KernelTimer>("usd.copy");
    auto                 cpp = std::make_unique<BRepBuilderAPI_Copy>(s, false, false);
    BRepBuilderAPI_Copy& cp  = *cpp;
    TopoDS_Shape         in  = cp.Shape();
    HHist                hc;
    HHist                hr;
    const FreeIfCancelled<std::unique_ptr<BRepBuilderAPI_Copy>, TopoDS_Shape, HHist, HHist> held(cpp, in, hc, hr);
    hc = cancellable_history(std::vector<TopoDS_Shape>{s}, cp);
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
    fail_if_cancelled();
    lap = std::make_unique<KernelTimer>("usd.shares");
    std::vector<TopoDS_Shape> image;
    std::vector<FaceShare>    share;
    images(cp, hr, in, image, share);
    fail_if_cancelled();
    lap = std::make_unique<KernelTimer>("usd.build");
    auto                          up = std::make_unique<ShapeUpgrade_UnifySameDomain>(in, true, true, false);
    ShapeUpgrade_UnifySameDomain& u  = *up;
    const FreeIfCancelled<std::unique_ptr<ShapeUpgrade_UnifySameDomain>> held_u(up);
    u.Build();
    fail_if_cancelled();
    lap = std::make_unique<KernelTimer>("usd.check");
    const std::optional<double> change = usd_volume_change(sf, image, share, u.Shape(), diag, &memo);
    if (!keeps_volume(change, u.Shape()))
    {
      // (The same merge rejected three times running: the faces' order doesn't change it here, so neither will the
      // rest: cf-spur-gear's Boolean 2 tried eight, 86 ms each. A notched cylinder fused with its mirror is kept at
      // its third.)
      int nf = 0;
      for (TopExp_Explorer x(u.Shape(), TopAbs_FACE); x.More(); x.Next())
        ++nf;
      const bool same = change && last_reject && last_reject->first == nf && std::abs(last_reject->second - *change) <= 1e-12 * bv;
      repeats         = same ? repeats + 1 : 1;
      if (repeats >= 3)
        break;
      last_reject = change ? std::optional<std::pair<int, double>>({nf, *change}) : std::nullopt;
      continue;
    }
    fail_if_cancelled();
    lap = std::make_unique<KernelTimer>("usd.history");
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
    std::vector<TopoDS_Shape> image;
    std::vector<FaceShare>    share;
    images(cp, HHist(), in, image, share);
    u.Build();
    if (!keeps_volume(usd_volume_change(sf, image, share, u.Shape(), diag), u.Shape()))
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

// A face bounded otherwise (its seam moved) covers what it did: only the integration's noise (a hole the wrong way
// round would add its area). One face's area, not the solid's volume twice (motor-mount-jack's pattern: 41 ms).
// A face's parameter-domain area, ∮ u dv over its edges' curves on it (Green), each edge by a 16-point Gauss rule per
// knot span: on faces whose edges' curves are the same up to whole turns, equal to rounding however coarse the rule.
static double uv_area(const TopoDS_Face& f)
{
  math_Vector gp(1, 16), gw(1, 16);
  math::GaussPoints(16, gp);
  math::GaussWeights(16, gw);
  double sum = 0;
  for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
  {
    const TopoDS_Edge&              e = TopoDS::Edge(x.Current());
    double                          a, b;
    const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, f, a, b);
    if (pc.IsNull())
      return std::numeric_limits<double>::quiet_NaN();
    Geom2dAdaptor_Curve ac(pc, a, b);
    // (Every knot span on its own: a rule across spans of a C2 spline isn't exact, and the faces' period shifts then
    // don't cancel: cf-straight-spline's shaft kept its seam.)
    const int           n = ac.NbIntervals(GeomAbs_CN);
    NCollection_Array1<double> cuts(1, n + 1);
    ac.Intervals(cuts, GeomAbs_CN);
    double edge = 0;
    for (int i = 1; i <= n; ++i)
    {
      const double t0 = cuts(i), t1 = cuts(i + 1), h = 0.5 * (t1 - t0);
      for (int k = 1; k <= 16; ++k)
      {
        gp_Pnt2d p;
        gp_Vec2d d;
        pc->D1(t0 + h * (gp(k) + 1), p, d);
        edge += gw(k) * h * p.X() * d.Y();
      }
    }
    sum += e.Orientation() == TopAbs_REVERSED ? -edge : edge;
  }
  return sum;
}

static bool same_area(const TopoDS_Face& a, const TopoDS_Face& f)
{
  // Cylinders: r times the domain's area, to accept quickly (a tube's face with holes bounded by B-spline rims: 8 ms a
  // face adaptively).
  TopLoc_Location l0, l1;
  if (!occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(a, l0)).IsNull() && !occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(f, l1)).IsNull())
  {
    const double d0 = std::abs(uv_area(a)), d1 = std::abs(uv_area(f));
    // (Equal there: the same. Else measured: a move rebuilds some edges' curves a little, cf-straight-spline's shafts.)
    if (std::isfinite(d0) && std::isfinite(d1) && std::abs(d1 - d0) <= 1e-9 * d0)
      return true;
  }
  // (Adaptively, to 1e-7: the fixed rule was 1e-3 off on a tube's face with holes bounded by joined B-spline rims.)
  GProp_GProps g0, g1;
  BRepGProp::SurfaceProperties(a, g0, 1e-7);
  BRepGProp::SurfaceProperties(f, g1, 1e-7);
  return std::abs(g1.Mass() - g0.Mass()) <= 1e-5 * std::abs(g0.Mass());
}

// Whether the faces of `out` that use any of `edges` are valid (a face and edges replaced in a valid solid: the other
// faces are as they were; the whole solid's check was 5 ms a move on motor-mount-jack's tube).
static bool valid_around(const TopoDS_Shape& out, const std::vector<TopoDS_Shape>& edges)
{
  for (TopExp_Explorer fx(out, TopAbs_FACE); fx.More(); fx.Next())
  {
    bool uses = false;
    for (TopExp_Explorer ex(fx.Current(), TopAbs_EDGE); ex.More() && !uses; ex.Next())
      for (const TopoDS_Shape& e : edges)
        uses = uses || ex.Current().IsSame(e);
    if (uses && !BRepCheck_Analyzer(fx.Current()).IsValid())
      return false;
  }
  return true;
}

// `S` (a cylinder, cone or torus, located) with its frame turned about its axis so that its u = 0 lies towards `p`.
occ::handle<Geom_ElementarySurface> turned_to(const occ::handle<Geom_Surface>& S, const gp_Pnt& p)
{
  const occ::handle<Geom_ElementarySurface> el = occ::down_cast<Geom_ElementarySurface>(S->Copy());
  if (el.IsNull())
    return el;
  const gp_Ax3 a = el->Position();
  gp_Vec       d(a.Location(), p);
  d -= gp_Vec(a.Direction()) * d.Dot(gp_Vec(a.Direction()));
  if (d.Magnitude() < 1e-12)
    return {};
  gp_Ax3 b(a.Location(), a.Direction(), gp_Dir(d));
  if (!a.Direct())
    b.YReverse();
  el->SetPosition(b);
  return el;
}

// (u, v) of `p` on a cylinder, cone or torus.
gp_Pnt2d elementary_uv(const occ::handle<Geom_ElementarySurface>& S, const gp_Pnt& p)
{
  double u = 0, v = 0;
  if (auto c = occ::down_cast<Geom_CylindricalSurface>(S); !c.IsNull())
    ElSLib::Parameters(c->Cylinder(), p, u, v);
  else if (auto t = occ::down_cast<Geom_ToroidalSurface>(S); !t.IsNull())
    ElSLib::Parameters(t->Torus(), p, u, v);
  else if (auto k = occ::down_cast<Geom_ConicalSurface>(S); !k.IsNull())
    ElSLib::Parameters(k->Cone(), p, u, v);
  return gp_Pnt2d(u, v);
}

// A hole's cylinder between two rims, each the edge of a coaxial torus or cone (a rounded rim, a countersink) or of a
// plane, where a torus's or cone's seam ends mid-way on its outer loop and splits it (learning-center-michael's
// Fillet 2: both rims' tori, live's 99 / 54 edges / vertices, ours 103 / 58; pipeflange-steven's Fillet 2: a
// countersink's rim on the top face in two arcs, live's 79 / 40, ours 81 / 42; Parasolid's faces have no seams): the
// seams moved together to an angle where the tori's and cones' outer loops all have a vertex, the rims' vertices with
// them; join_loop_pieces then joins the pieces. The shape as it was where none applies or where the faces don't come
// out valid with their areas.
TopoDS_Shape rim_chain_seams(const TopoDS_Shape& s, HHist& hist)
{
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  for (TopExp_Explorer fx(s, TopAbs_FACE); fx.More(); fx.Next())
  {
    const TopoDS_Face   cf = TopoDS::Face(fx.Current().Oriented(TopAbs_FORWARD));
    const occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(cf));
    if (cs.IsNull())
      continue;
    const gp_Cylinder ca = cs->Cylinder();
    // Its seams and its two rims (closed circles round its axis), nothing else.
    std::vector<TopoDS_Edge> cseams, rims;
    bool                     only = true;
    ShapeIMap                seen;
    for (TopExp_Explorer x(cf, TopAbs_EDGE); x.More() && only; x.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(x.Current());
      if (seen.Contains(e))
        continue;
      seen.Add(e);
      if (BRep_Tool::IsClosed(e, cf))
      {
        cseams.push_back(e);
        continue;
      }
      TopoDS_Vertex v0, v1;
      TopExp::Vertices(e, v0, v1);
      BRepAdaptor_Curve c(e);
      only = !v0.IsNull() && v0.IsSame(v1) && c.GetType() == GeomAbs_Circle && c.Circle().Axis().Direction().IsParallel(ca.Axis().Direction(), 1e-9);
      rims.push_back(e);
    }
    if (!only || cseams.size() != 1 || rims.size() != 2)
      continue;
    // Each rim's other face: a torus or cone about the same axis with a seam, or a plane.
    struct Side
    {
      TopoDS_Edge                         rim;
      TopoDS_Face                         face;
      occ::handle<Geom_ElementarySurface> ts; // null: a plane
      std::vector<TopoDS_Edge>            seams, rest;
      std::vector<std::pair<double, TopoDS_Vertex>> at; // the outer loop's vertices off the seams, by angle
    };
    std::vector<Side> sides;
    for (const TopoDS_Edge& rim : rims)
    {
      const ShapeList* fs = ef.Seek(rim);
      if (fs == nullptr || fs->Extent() != 2)
        break;
      const TopoDS_Face tf = TopoDS::Face((fs->First().IsSame(cf) ? fs->Last() : fs->First()).Oriented(TopAbs_FORWARD));
      const occ::handle<Geom_Surface> gs = BRep_Tool::Surface(tf);
      if (!occ::down_cast<Geom_Plane>(gs).IsNull())
      {
        sides.push_back(Side{rim, tf, {}, {}, {}, {}});
        continue;
      }
      occ::handle<Geom_ElementarySurface> ts = occ::down_cast<Geom_ToroidalSurface>(gs);
      if (ts.IsNull())
        ts = occ::down_cast<Geom_ConicalSurface>(gs);
      if (ts.IsNull() || !ts->Position().Axis().IsParallel(ca.Axis(), 1e-9) || gp_Lin(ca.Axis()).Distance(ts->Position().Location()) > 1e-9 * (1 + ca.Radius()))
        break;
      Side sd{rim, tf, ts, {}, {}, {}};
      for (TopExp_Explorer x(tf, TopAbs_EDGE); x.More(); x.Next())
      {
        const TopoDS_Edge& e = TopoDS::Edge(x.Current());
        if (e.IsSame(rim) || std::any_of(sd.seams.begin(), sd.seams.end(), [&](const TopoDS_Edge& q) { return q.IsSame(e); }) || std::any_of(sd.rest.begin(), sd.rest.end(), [&](const TopoDS_Edge& q) { return q.IsSame(e); }))
          continue;
        (BRep_Tool::IsClosed(e, tf) ? sd.seams : sd.rest).push_back(e);
      }
      if (sd.seams.size() != 1 || sd.rest.empty())
        break;
      ShapeIMap on_seam, rest_v;
      TopExp::MapShapes(sd.seams[0], TopAbs_VERTEX, on_seam);
      for (const TopoDS_Edge& e : sd.rest)
        TopExp::MapShapes(e, TopAbs_VERTEX, rest_v);
      for (int i = 1; i <= rest_v.Extent(); ++i)
        if (!on_seam.Contains(rest_v(i)))
          sd.at.push_back({elementary_uv(cs, BRep_Tool::Pnt(TopoDS::Vertex(rest_v(i)))).X(), TopoDS::Vertex(rest_v(i))});
      sides.push_back(sd);
    }
    if (sides.size() != 2)
      continue;
    // Only where a torus's seam ends between two pieces of one curve (smooth, both on the same other face): what the
    // move undoes.
    auto splits = [&](const Side& sd) {
      TopoDS_Vertex a0, a1;
      TopExp::Vertices(sd.seams[0], a0, a1);
      for (const TopoDS_Vertex& j : {a0, a1})
      {
        std::vector<gp_Vec>       ds;
        std::vector<TopoDS_Shape> others;
        for (const TopoDS_Edge& e : sd.rest)
          for (TopExp_Explorer w(e, TopAbs_VERTEX); w.More(); w.Next())
            if (w.Current().IsSame(j))
            {
              BRepAdaptor_Curve c(e);
              gp_Pnt            p;
              gp_Vec            d;
              c.D1(BRep_Tool::Parameter(TopoDS::Vertex(w.Current()), e), p, d);
              ds.push_back(d);
              if (const ShapeList* fs = ef.Seek(e))
                for (ShapeList::Iterator it(*fs); it.More(); it.Next())
                  if (!it.Value().IsSame(sd.face))
                    others.push_back(it.Value());
              break;
            }
        if (ds.size() == 2 && others.size() == 2 && others[0].IsSame(others[1]) && ds[0].Magnitude() > 1e-12 && ds[1].Magnitude() > 1e-12
            && ds[0].Crossed(ds[1]).Magnitude() <= 1e-6 * ds[0].Magnitude() * ds[1].Magnitude())
          return true;
      }
      return false;
    };
    if (!(sides[0].ts.IsNull() ? false : splits(sides[0])) && !(sides[1].ts.IsNull() ? false : splits(sides[1])))
      continue;
    // The angle the seamed sides' outer loops all have a vertex at, nearest the seam.
    const double old   = elementary_uv(cs, BRep_Tool::Pnt(TopExp::FirstVertex(cseams[0]))).X();
    const int    first = sides[0].ts.IsNull() ? 1 : 0;
    double       theta = 0, best = std::numeric_limits<double>::infinity();
    TopoDS_Vertex V[2];
    for (const auto& [a0, v0] : sides[(size_t)first].at)
    {
      TopoDS_Vertex v1;
      if (!sides[(size_t)(1 - first)].ts.IsNull())
      {
        for (const auto& [a1, x1] : sides[(size_t)(1 - first)].at)
          if (std::abs(std::remainder(a0 - a1, 2 * M_PI)) < 1e-9)
            v1 = x1;
        if (v1.IsNull())
          continue;
      }
      if (std::abs(std::remainder(a0 - old, 2 * M_PI)) < best)
      {
        best          = std::abs(std::remainder(a0 - old, 2 * M_PI));
        theta         = a0;
        V[first]      = v0;
        V[1 - first]  = v1;
      }
    }
    if (V[first].IsNull() || best < 1e-9)
      continue;
    const gp_Pnt toward = cs->Value(theta, 0);
    const occ::handle<Geom_ElementarySurface> cn = turned_to(cs, toward);
    if (cn.IsNull())
      continue;
    BRep_Builder B;
    double       tol = BRep_Tool::Tolerance(cf);
    for (const Side& sd : sides)
      tol = std::max({tol, BRep_Tool::Tolerance(sd.face), BRep_Tool::Tolerance(sd.rim)});
    TopoDS_Face Fc;
    B.MakeFace(Fc, cn, tol);
    // The rims, from the turned cylinder (their parameter its u): a vertex at u = 0.
    double      vc[2];
    TopoDS_Edge R[2];
    TopoDS_Vertex VR[2];
    for (int i = 0; i < 2; ++i)
    {
      vc[i] = elementary_uv(cn, BRepAdaptor_Curve(sides[(size_t)i].rim).Value(0)).Y();
      B.MakeVertex(VR[i], cn->Value(0, vc[i]), tol);
      B.MakeEdge(R[i], cn->VIso(vc[i]), tol);
      B.Add(R[i], VR[i].Oriented(TopAbs_FORWARD));
      B.Add(R[i], VR[i].Oriented(TopAbs_REVERSED));
      B.Range(R[i], 0, 2 * M_PI);
      B.UpdateEdge(R[i], new Geom2d_Line(gp_Pnt2d(0, vc[i]), gp_Dir2d(1, 0)), Fc, tol);
      B.Range(R[i], Fc, 0, 2 * M_PI);
    }
    const int lo = vc[0] < vc[1] ? 0 : 1, hi = 1 - lo;
    TopoDS_Edge cseam;
    B.MakeEdge(cseam, cn->UIso(0), tol);
    B.Add(cseam, VR[lo].Oriented(TopAbs_FORWARD));
    B.Add(cseam, VR[hi].Oriented(TopAbs_REVERSED));
    B.Range(cseam, vc[lo], vc[hi]);
    B.UpdateEdge(cseam, new Geom2d_Line(gp_Pnt2d(2 * M_PI, 0), gp_Dir2d(0, 1)), new Geom2d_Line(gp_Pnt2d(0, 0), gp_Dir2d(0, 1)), Fc, tol);
    B.Range(cseam, Fc, vc[lo], vc[hi]);
    {
      TopoDS_Wire W;
      B.MakeWire(W);
      B.Add(W, R[lo]);
      B.Add(W, cseam.Oriented(TopAbs_FORWARD));
      B.Add(W, R[hi].Reversed());
      B.Add(W, cseam.Oriented(TopAbs_REVERSED));
      W.Closed(true);
      B.Add(Fc, W);
    }
    // Each torus: turned so its u = 0 lies at the vertex; its rim, a seam from the rim's vertex to the outer loop's,
    // the loop's own curves moved by the turn.
    TopoDS_Face Ft[2];
    TopoDS_Edge tseam[2];
    bool        ok = true;
    for (int i = 0; i < 2 && ok; ++i)
    {
      const Side& sd = sides[(size_t)i];
      if (sd.ts.IsNull())
        continue;
      const occ::handle<Geom_ElementarySurface> tn = turned_to(sd.ts, BRep_Tool::Pnt(V[i]));
      if (tn.IsNull())
      {
        ok = false;
        break;
      }
      B.MakeFace(Ft[i], tn, tol);
      // The old curves' u and v: the rim's v and the vertex's (u, v) on the old face as its curves have them.
      double                    f2, l2;
      occ::handle<Geom2d_Curve> rpc = BRep_Tool::CurveOnSurface(sd.rim, sd.face, f2, l2);
      if (rpc.IsNull())
      {
        ok = false;
        break;
      }
      const double v_rim = rpc->Value(f2).Y();
      double       uV = 0, vV = 0;
      bool         found = false;
      for (const TopoDS_Edge& e : sd.rest)
      {
        TopoDS_Vertex a0, a1;
        TopExp::Vertices(e, a0, a1);
        if (!(a0.IsSame(V[i]) || a1.IsSame(V[i])))
          continue;
        const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, sd.face, f2, l2);
        if (pc.IsNull())
          continue;
        const gp_Pnt2d q = pc->Value(BRep_Tool::Parameter(V[i], e));
        uV               = q.X();
        vV               = q.Y();
        found            = true;
        break;
      }
      if (!found)
      {
        ok = false;
        break;
      }
      // The rim on this torus: its parameter (the cylinder's turned u) against the torus's turned u.
      const double t1  = 1.0;
      const double ut1 = std::remainder(elementary_uv(tn, BRepAdaptor_Curve(R[i]).Value(t1)).X(), 2 * M_PI);
      const bool   same = std::abs(ut1 - t1) < 1e-6;
      if (!same && std::abs(ut1 + t1) > 1e-6)
      {
        ok = false;
        break;
      }
      const occ::handle<Geom2d_Line> rl = same ? new Geom2d_Line(gp_Pnt2d(0, v_rim), gp_Dir2d(1, 0)) : new Geom2d_Line(gp_Pnt2d(2 * M_PI, v_rim), gp_Dir2d(-1, 0));
      B.UpdateEdge(R[i], rl, Ft[i], tol);
      B.Range(R[i], Ft[i], 0, 2 * M_PI);
      const TopoDS_Edge plus_u = same ? R[i] : TopoDS::Edge(R[i].Reversed());
      // The seam: the turned torus's meridian at u = 0 (its parameter v), from the lower v to the higher.
      const bool   up = vV > v_rim;
      const double va = std::min(vV, v_rim), vb = std::max(vV, v_rim);
      B.MakeEdge(tseam[i], tn->UIso(0), tol);
      B.Add(tseam[i], (up ? VR[i] : V[i]).Oriented(TopAbs_FORWARD));
      B.Add(tseam[i], (up ? V[i] : VR[i]).Oriented(TopAbs_REVERSED));
      B.Range(tseam[i], va, vb);
      B.UpdateEdge(tseam[i], new Geom2d_Line(gp_Pnt2d(2 * M_PI, 0), gp_Dir2d(0, 1)), new Geom2d_Line(gp_Pnt2d(0, 0), gp_Dir2d(0, 1)), Ft[i], tol);
      B.Range(tseam[i], Ft[i], va, vb);
      // The outer loop's curves, turned and put in [0, 2 pi).
      for (const TopoDS_Edge& e : sd.rest)
      {
        occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, sd.face, f2, l2);
        if (pc.IsNull())
        {
          ok = false;
          break;
        }
        const double um = pc->Value(0.5 * (f2 + l2)).X() - uV;
        const double k  = std::floor(um / (2 * M_PI));
        pc              = occ::down_cast<Geom2d_Curve>(pc->Translated(gp_Vec2d(-uV - 2 * M_PI * k, 0)));
        B.UpdateEdge(e, pc, Ft[i], BRep_Tool::Tolerance(e));
        B.Range(e, Ft[i], f2, l2);
      }
      if (!ok)
        break;
      // The loop from V as the old face ran it.
      std::vector<TopoDS_Edge> uses;
      for (TopoDS_Iterator w(sd.face); w.More(); w.Next())
        for (TopoDS_Iterator x(w.Value()); x.More(); x.Next())
          for (const TopoDS_Edge& e : sd.rest)
            if (x.Value().IsSame(e))
              uses.push_back(TopoDS::Edge(x.Value()));
      std::vector<TopoDS_Edge> chain;
      TopoDS_Vertex            at = V[i];
      std::vector<bool>        used(uses.size(), false);
      do
      {
        size_t next = uses.size();
        for (size_t j = 0; j < uses.size() && next == uses.size(); ++j)
          if (!used[j] && TopExp::FirstVertex(uses[j], true).IsSame(at))
            next = j;
        if (next == uses.size())
        {
          ok = false;
          break;
        }
        used[next] = true;
        chain.push_back(uses[next]);
        at = TopExp::LastVertex(uses[next], true);
      } while (!at.IsSame(V[i]));
      if (!ok || chain.size() != uses.size())
      {
        ok = false;
        break;
      }
      TopoDS_Wire W;
      B.MakeWire(W);
      if (up)
      {
        B.Add(W, plus_u);
        B.Add(W, tseam[i].Oriented(TopAbs_FORWARD));
        for (const TopoDS_Edge& e : chain)
          B.Add(W, e);
        B.Add(W, tseam[i].Oriented(TopAbs_REVERSED));
      }
      else
      {
        B.Add(W, plus_u.Reversed());
        B.Add(W, tseam[i].Oriented(TopAbs_REVERSED));
        for (const TopoDS_Edge& e : chain)
          B.Add(W, e);
        B.Add(W, tseam[i].Oriented(TopAbs_FORWARD));
      }
      W.Closed(true);
      B.Add(Ft[i], W);
      ok = BRepCheck_Analyzer(Ft[i]).IsValid() && same_area(sd.face, Ft[i]);
    }
    if (!ok || !BRepCheck_Analyzer(Fc).IsValid() || !same_area(cf, Fc))
      continue;
    occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
    rs->Replace(cf, Fc);
    std::vector<TopoDS_Shape> around{R[0], R[1], cseam};
    for (int i = 0; i < 2; ++i)
      if (sides[(size_t)i].ts.IsNull())
      {
        // A plane keeps its face, the rim swapped for the new one (the same way round).
        const bool same = BRepAdaptor_Curve(sides[(size_t)i].rim).Circle().Axis().Direction().Dot(BRepAdaptor_Curve(R[i]).Circle().Axis().Direction()) > 0;
        rs->Replace(sides[(size_t)i].rim.Oriented(TopAbs_FORWARD), same ? R[i] : TopoDS::Edge(R[i].Reversed()));
      }
      else
      {
        rs->Replace(sides[(size_t)i].face, Ft[i]);
        around.push_back(tseam[i]);
      }
    const TopoDS_Shape out = rs->Apply(s);
    if (out.IsNull() || !valid_around(out, around))
      continue;
    if (hist.IsNull())
      hist = new BRepTools_History();
    HHist h = new BRepTools_History();
    h->AddModified(cf, Fc);
    for (int i = 0; i < 2; ++i)
    {
      h->AddModified(sides[(size_t)i].rim, R[i]);
      if (sides[(size_t)i].ts.IsNull())
        continue;
      h->AddModified(sides[(size_t)i].face, Ft[i]);
      h->Remove(sides[(size_t)i].seams[0]);
    }
    h->Remove(cseams[0]);
    hist->Merge(h);
    return rim_chain_seams(out, hist);
  }
  return s;
}

// A cylinder face between one end circle and a chain of other edges round the axis whose seam splits the chain (a
// shaft's band below spline slots, a run-out crossing the seam, cf-straight-spline; live's faces have no seams, its
// run-out is one edge): the seam moved to run from the circle to the chain's vertex nearest it, the circle's vertex
// with it; join_loop_pieces then joins what the old seam split. Null where it doesn't apply or doesn't come out valid
// with the same area.
TopoDS_Shape seam_to_vertex(const TopoDS_Shape& s, const TopoDS_Face& a, const gp_Cylinder& ca, const std::vector<TopoDS_Edge>& seams,
                            const TopoDS_Edge& end, const std::vector<TopoDS_Edge>& rest, HHist& hist)
{
  auto uv = [&ca](const gp_Pnt& p) {
    double u, v;
    ElSLib::Parameters(ca, p, u, v);
    return gp_Pnt2d(u, v);
  };
  BRepAdaptor_Curve ec(end);
  const double      v_ring = uv(ec.Value(ec.FirstParameter())).Y();
  ShapeIMap         seam_v, chain_v;
  for (const TopoDS_Edge& e : seams)
    TopExp::MapShapes(e, TopAbs_VERTEX, seam_v);
  for (const TopoDS_Edge& e : rest)
    TopExp::MapShapes(e, TopAbs_VERTEX, chain_v);
  // A joint of the chain: two chain edges on the same other face going on smoothly, pieces of one curve (where OCCT's
  // seam split it; not a corner, nor where a line runs on tangent from an ellipse).
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  auto joint = [&](const TopoDS_Vertex& x) {
    std::vector<gp_Vec>       ds;
    std::vector<TopoDS_Shape> others;
    for (const TopoDS_Edge& e : rest)
      for (TopExp_Explorer w(e, TopAbs_VERTEX); w.More(); w.Next())
        if (w.Current().IsSame(x))
        {
          BRepAdaptor_Curve c(e);
          gp_Pnt            p;
          gp_Vec            d;
          c.D1(BRep_Tool::Parameter(TopoDS::Vertex(w.Current()), e), p, d);
          ds.push_back(d);
          if (const ShapeList* fs = ef.Seek(e))
            for (ShapeList::Iterator it(*fs); it.More(); it.Next())
              if (!it.Value().IsSame(a))
                others.push_back(it.Value());
          break;
        }
    return ds.size() == 2 && others.size() == 2 && others[0].IsSame(others[1]) && ds[0].Magnitude() > 1e-12 && ds[1].Magnitude() > 1e-12
           && ds[0].Crossed(ds[1]).Magnitude() <= 1e-6 * ds[0].Magnitude() * ds[1].Magnitude();
  };
  for (int i = 1; i <= seam_v.Extent(); ++i)
    if (chain_v.Contains(seam_v(i)) && !joint(TopoDS::Vertex(seam_v(i))))
      return {};
  // The loop round the axis: the chain edges joined to where the seam meets them (not a hole's loop, which the seam
  // can't end on: bushing2-0-joshua's window through the wall lies nearer the circle than the notched end).
  ShapeIMap round_v;
  for (int i = 1; i <= seam_v.Extent(); ++i)
    if (chain_v.Contains(seam_v(i)))
      round_v.Add(seam_v(i));
  for (bool grew = true; grew;)
  {
    grew = false;
    for (const TopoDS_Edge& e : rest)
    {
      TopoDS_Vertex e0, e1;
      TopExp::Vertices(e, e0, e1);
      if (e0.IsNull() || e1.IsNull() || round_v.Contains(e0) == round_v.Contains(e1))
        continue;
      round_v.Add(e0);
      round_v.Add(e1);
      grew = true;
    }
  }
  // An edge of the chain along the axis at angle `alpha` (all of it on that line).
  auto along = [&](const TopoDS_Edge& e, double alpha) {
    BRepAdaptor_Curve c(e);
    for (int k = 0; k <= 8; ++k)
      if (std::abs(std::remainder(uv(c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / 8.0)).X() - alpha, 2 * M_PI)) >= 1e-6)
        return false;
    return true;
  };
  // (No chain edge runs at a candidate's angle between its ends: it would sit on the seam, on one side of it or the
  // other; but one along the axis from the candidate itself goes on from the seam, its side known below.)
  auto clear_at = [&](double alpha, const TopoDS_Vertex& x) {
    for (const TopoDS_Edge& e : rest)
    {
      TopoDS_Vertex e0, e1;
      TopExp::Vertices(e, e0, e1);
      if ((x.IsSame(e0) || x.IsSame(e1)) && along(e, alpha))
        continue;
      BRepAdaptor_Curve c(e);
      for (int k = 1; k < 32; ++k)
        if (std::abs(std::remainder(uv(c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / 32.0)).X() - alpha, 2 * M_PI)) < 1e-6)
          return false;
    }
    return true;
  };
  // Nothing of the chain between the circle and x at x's angle: no sample there, nor two samples either side of it
  // (bushing2-0-joshua's window through the wall crosses it between samples).
  auto open_to = [&](const TopoDS_Vertex& x) {
    const gp_Pnt2d px = uv(BRep_Tool::Pnt(x));
    for (const TopoDS_Edge& e : rest)
    {
      BRepAdaptor_Curve c(e);
      double            du0 = 0;
      for (int k = 0; k <= 32; ++k)
      {
        const gp_Pnt2d p  = uv(c.Value(c.FirstParameter() + (c.LastParameter() - c.FirstParameter()) * k / 32.0));
        const double   du = std::remainder(p.X() - px.X(), 2 * M_PI);
        const bool     between = (p.Y() - v_ring) * (px.Y() - p.Y()) > 0;
        if (between && ((k > 0 && k < 32 && std::abs(du) < 1e-3) || (k > 0 && std::abs(du) > 1e-9 && std::abs(du0) > 1e-9 && du * du0 < 0 && std::abs(du - du0) < M_PI)))
          return false;
        du0 = du;
      }
    }
    return true;
  };
  TopoDS_Vertex V;
  double        best = std::numeric_limits<double>::infinity();
  for (int i = 1; i <= chain_v.Extent(); ++i)
  {
    const TopoDS_Vertex& x = TopoDS::Vertex(chain_v(i));
    const double         d = std::abs(uv(BRep_Tool::Pnt(x)).Y() - v_ring);
    if (!seam_v.Contains(x) && round_v.Contains(x) && d < best - 1e-12 && !joint(x) && clear_at(uv(BRep_Tool::Pnt(x)).X(), x) && open_to(x))
    {
      best = d;
      V    = x;
    }
  }
  if (V.IsNull() || best < 1e-9)
    return {};
  const gp_Pnt2d pV    = uv(BRep_Tool::Pnt(V));
  const double   alpha = pV.X();
  const bool     up    = pV.Y() > v_ring;
  const gp_Ax3& A = ca.Position();
  BRep_Builder  B;
  double        tol = std::max(BRep_Tool::Tolerance(a), BRep_Tool::Tolerance(end));
  const gp_Dir  n   = gp_Dir(gp_Vec(A.XDirection()).Crossed(gp_Vec(A.YDirection())));
  // (The end circle as it was, framed so its parameter is the cylinder's u: rebuilt from the cylinder, its radius a
  // nanometre off the Boolean's moved a pin's union by 6e-4 mm³.)
  const gp_Circ oc = ec.Circle();
  const gp_Circ rc(gp_Ax2(oc.Location(), n, A.XDirection()), oc.Radius());
  TopoDS_Vertex vr;
  B.MakeVertex(vr, ElCLib::Value(alpha, rc), tol);
  TopoDS_Edge ring;
  B.MakeEdge(ring, new Geom_Circle(rc), tol);
  B.Add(ring, vr.Oriented(TopAbs_FORWARD));
  B.Add(ring, vr.Oriented(TopAbs_REVERSED));
  B.Range(ring, alpha, alpha + 2 * M_PI);
  TopoDS_Edge seam;
  B.MakeEdge(seam, new Geom_Line(ElCLib::Value(alpha, rc), up ? A.Direction() : A.Direction().Reversed()), tol);
  B.Add(seam, vr.Oriented(TopAbs_FORWARD));
  B.Add(seam, V.Oriented(TopAbs_REVERSED));
  B.Range(seam, 0, std::abs(pV.Y() - v_ring));
  TopoDS_Face F;
  TopLoc_Location la;
  B.MakeFace(F, occ::down_cast<Geom_Surface>(BRep_Tool::Surface(a, la)->Copy()), la, BRep_Tool::Tolerance(a));
  B.UpdateEdge(ring, new Geom2d_Line(gp_Pnt2d(0, v_ring), gp_Dir2d(1, 0)), F, tol);
  B.Range(ring, F, alpha, alpha + 2 * M_PI);
  // Going up the face's material is on the circle's +u side: up the seam at alpha + 2 pi, back down at alpha.
  const occ::handle<Geom2d_Line> hi = new Geom2d_Line(gp_Pnt2d(alpha + 2 * M_PI, v_ring), gp_Dir2d(0, up ? 1 : -1));
  const occ::handle<Geom2d_Line> lo = new Geom2d_Line(gp_Pnt2d(alpha, v_ring), gp_Dir2d(0, up ? 1 : -1));
  B.UpdateEdge(seam, up ? hi : lo, up ? lo : hi, F, tol);
  B.Range(seam, F, 0, std::abs(pV.Y() - v_ring));
  for (const TopoDS_Edge& e : rest)
  {
    double                    f2, l2;
    occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, a, f2, l2);
    if (pc.IsNull())
      return {};
    const double um = pc->Value(0.5 * (f2 + l2)).X();
    const double k  = std::floor((alpha + 2 * M_PI - um) / (2 * M_PI));
    if (k != 0)
      pc = occ::down_cast<Geom2d_Curve>(pc->Translated(gp_Vec2d(2 * M_PI * k, 0)));
    B.UpdateEdge(e, pc, F, BRep_Tool::Tolerance(e));
    B.Range(e, F, f2, l2);
  }
  // The other edges as the face uses them, in loops: the one through V into the outer wire, the rest holes.
  std::vector<TopoDS_Edge> uses;
  for (TopoDS_Iterator w(a); w.More(); w.Next())
    for (TopoDS_Iterator x(w.Value()); x.More(); x.Next())
      for (const TopoDS_Edge& e : rest)
        if (x.Value().IsSame(e))
          uses.push_back(TopoDS::Edge(x.Value()));
  std::vector<bool> used(uses.size(), false);
  auto              loop_from = [&](size_t at, std::vector<TopoDS_Edge>& out) {
    const TopoDS_Vertex start = TopExp::FirstVertex(uses[at], true);
    while (true)
    {
      used[at] = true;
      out.push_back(uses[at]);
      const TopoDS_Vertex last = TopExp::LastVertex(uses[at], true);
      if (last.IsSame(start))
        return true;
      size_t next = uses.size();
      for (size_t j = 0; j < uses.size() && next == uses.size(); ++j)
        if (!used[j] && TopExp::FirstVertex(uses[j], true).IsSame(last))
          next = j;
      if (next == uses.size())
        return false;
      at = next;
    }
  };
  size_t from_v = uses.size();
  for (size_t k = 0; k < uses.size(); ++k)
    if (TopExp::FirstVertex(uses[k], true).IsSame(V))
    {
      if (from_v != uses.size())
        return {};
      from_v = k;
    }
  std::vector<TopoDS_Edge> chain;
  if (from_v == uses.size() || !loop_from(from_v, chain))
    return {};
  // An edge along the axis from V on the new seam's line: on the side the wire leaves V by (from the ring up the seam
  // at alpha + 2 pi, so on at alpha + 2 pi; from a ring above, down at alpha) or comes back to V by (the other).
  for (const TopoDS_Edge* e : {&chain.front(), &chain.back()})
  {
    if (!along(*e, alpha))
      continue;
    double                    f2, l2;
    occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(*e, F, f2, l2);
    if (pc.IsNull())
      return {};
    const double want = (e == &chain.front()) == up ? alpha + 2 * M_PI : alpha;
    pc = occ::down_cast<Geom2d_Curve>(pc->Translated(gp_Vec2d(want - pc->Value(0.5 * (f2 + l2)).X(), 0)));
    B.UpdateEdge(*e, pc, F, BRep_Tool::Tolerance(*e));
    B.Range(*e, F, f2, l2);
  }
  TopoDS_Wire W;
  B.MakeWire(W);
  B.Add(W, up ? ring : TopoDS::Edge(ring.Reversed()));
  B.Add(W, seam.Oriented(TopAbs_FORWARD));
  for (const TopoDS_Edge& e : chain)
    B.Add(W, e);
  B.Add(W, seam.Oriented(TopAbs_REVERSED));
  W.Closed(true);
  B.Add(F, W);
  for (size_t k = 0; k < uses.size(); ++k)
  {
    if (used[k])
      continue;
    std::vector<TopoDS_Edge> loop;
    if (!loop_from(k, loop))
      return {};
    TopoDS_Wire hole;
    B.MakeWire(hole);
    for (const TopoDS_Edge& e : loop)
      B.Add(hole, e);
    hole.Closed(true);
    B.Add(F, hole);
  }
  if (!BRepCheck_Analyzer(F).IsValid())
    return {};
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  rs->Replace(a, F);
  const bool same = BRepAdaptor_Curve(end).Circle().Axis().Direction().Dot(gp_Vec(n)) > 0;
  rs->Replace(end.Oriented(TopAbs_FORWARD), same ? ring : TopoDS::Edge(ring.Reversed()));
  const TopoDS_Shape out = rs->Apply(s);
  if (out.IsNull() || !same_area(a, F) || !valid_around(out, {ring}))
    return {};
  if (hist.IsNull())
    hist = new BRepTools_History();
  HHist h = rs->History();
  if (h.IsNull())
    h = new BRepTools_History();
  if (h->Modified(a).IsEmpty())
    h->AddModified(a, F);
  for (const TopoDS_Edge& e : seams)
    h->Remove(e);
  hist->Merge(h);
  return out;
}

// Cylinder band `a` (between the end circles `ends`, its seams `seams`, its other edges `rest`) with its seam moved to
// angle `alpha`, the end circles' vertices with it (`la` its surface's location). Null where it doesn't come out valid
// with the same area.
TopoDS_Shape band_seam_to(const TopoDS_Shape& s, const TopoDS_Face& a, TopLoc_Location la, const gp_Cylinder& ca, const std::vector<TopoDS_Edge>& seams,
                          const std::vector<TopoDS_Edge>& ends, const std::vector<TopoDS_Edge>& rest, double alpha, HHist& hist)
{
  bool ok = true;
  const double  r = ca.Radius();
  const gp_Ax3& A = ca.Position();
  auto          v_of = [&](const TopoDS_Edge& e) {
    BRepAdaptor_Curve c(e);
    double            u, v;
    ElSLib::Parameters(ca, c.Value(c.FirstParameter()), u, v);
    return v;
  };
  const bool   first_low = v_of(ends[0]) < v_of(ends[1]);
  const double lo = v_of(ends[first_low ? 0 : 1]), hi = v_of(ends[first_low ? 1 : 0]);
  BRep_Builder B;
  double       tol = BRep_Tool::Tolerance(a);
  for (const TopoDS_Edge& e : ends)
    tol = std::max(tol, BRep_Tool::Tolerance(e));
  // The end circles again, their vertex at alpha, their parameter the cylinder's u.
  const gp_Dir n = gp_Dir(gp_Vec(A.XDirection()).Crossed(gp_Vec(A.YDirection())));
  auto         ring = [&](double v, TopoDS_Vertex& vx) {
    B.MakeVertex(vx, ElSLib::Value(alpha, v, ca), tol);
    occ::handle<Geom_Circle> circ = new Geom_Circle(gp_Ax2(A.Location().Translated(gp_Vec(A.Direction()) * v), n, A.XDirection()), r);
    TopoDS_Edge              e;
    B.MakeEdge(e, circ, tol);
    B.Add(e, vx.Oriented(TopAbs_FORWARD));
    B.Add(e, vx.Oriented(TopAbs_REVERSED));
    B.Range(e, alpha, alpha + 2 * M_PI);
    return e;
  };
  TopoDS_Vertex     vlo, vhi;
  const TopoDS_Edge low = ring(lo, vlo), top = ring(hi, vhi);
  occ::handle<Geom_Line> line = new Geom_Line(ElSLib::Value(alpha, lo, ca), A.Direction());
  TopoDS_Edge            seam;
  B.MakeEdge(seam, line, tol);
  B.Add(seam, vlo.Oriented(TopAbs_FORWARD));
  B.Add(seam, vhi.Oriented(TopAbs_REVERSED));
  B.Range(seam, 0, hi - lo);
  // (On a copy of the surface: the edges keep their curves on the old face, whatever comes of this.)
  TopoDS_Face F;
  B.MakeFace(F, occ::down_cast<Geom_Surface>(BRep_Tool::Surface(a, la)->Copy()), la, BRep_Tool::Tolerance(a));
  B.UpdateEdge(low, new Geom2d_Line(gp_Pnt2d(0, lo), gp_Dir2d(1, 0)), F, tol);
  B.Range(low, F, alpha, alpha + 2 * M_PI);
  B.UpdateEdge(top, new Geom2d_Line(gp_Pnt2d(0, hi), gp_Dir2d(1, 0)), F, tol);
  B.Range(top, F, alpha, alpha + 2 * M_PI);
  B.UpdateEdge(seam, new Geom2d_Line(gp_Pnt2d(alpha + 2 * M_PI, lo), gp_Dir2d(0, 1)), new Geom2d_Line(gp_Pnt2d(alpha, lo), gp_Dir2d(0, 1)), F, tol);
  B.Range(seam, F, 0, hi - lo);
  TopoDS_Wire W;
  B.MakeWire(W);
  B.Add(W, low);
  B.Add(W, seam.Oriented(TopAbs_FORWARD));
  B.Add(W, top.Reversed());
  B.Add(W, seam.Oriented(TopAbs_REVERSED));
  B.Add(F, W);
  // The other edges, as the face uses them, a whole number of turns into (alpha, alpha + 2 pi), in loops.
  std::vector<TopoDS_Edge> uses;
  for (TopoDS_Iterator w(a); w.More(); w.Next())
    for (TopoDS_Iterator x(w.Value()); x.More(); x.Next())
      for (const TopoDS_Edge& e : rest)
        if (x.Value().IsSame(e))
          uses.push_back(TopoDS::Edge(x.Value()));
  for (const TopoDS_Edge& e : rest)
  {
    double                    f2, l2;
    occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, a, f2, l2);
    const double              um = pc->Value(0.5 * (f2 + l2)).X();
    const double              k  = std::floor((alpha + 2 * M_PI - um) / (2 * M_PI));
    if (k != 0)
      pc = occ::down_cast<Geom2d_Curve>(pc->Translated(gp_Vec2d(2 * M_PI * k, 0)));
    B.UpdateEdge(e, pc, F, BRep_Tool::Tolerance(e));
    B.Range(e, F, f2, l2);
  }
  std::vector<bool> used(uses.size(), false);
  for (size_t k = 0; k < uses.size() && ok; ++k)
  {
    if (used[k])
      continue;
    TopoDS_Wire hole;
    B.MakeWire(hole);
    size_t              at    = k;
    const TopoDS_Vertex start = TopExp::FirstVertex(uses[k], true);
    while (ok)
    {
      used[at] = true;
      B.Add(hole, uses[at]);
      const TopoDS_Vertex end = TopExp::LastVertex(uses[at], true);
      if (end.IsSame(start))
        break;
      size_t next = uses.size();
      for (size_t j = 0; j < uses.size() && next == uses.size(); ++j)
        if (!used[j] && TopExp::FirstVertex(uses[j], true).IsSame(end))
          next = j;
      if (next == uses.size())
        ok = false;
      at = next;
    }
    hole.Closed(true);
    B.Add(F, hole);
  }
  if (!ok || !BRepCheck_Analyzer(F).IsValid())
    return {};
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  rs->Replace(a, F);
  for (size_t k = 0; k < 2; ++k)
  {
    const TopoDS_Edge&  old   = ends[k];
    const TopoDS_Edge&  made  = (k == 0) == first_low ? low : top;
    BRepAdaptor_Curve   old_c(old);
    const bool          same  = old_c.Circle().Axis().Direction().Dot(gp_Vec(n)) > 0;
    rs->Replace(old.Oriented(TopAbs_FORWARD), same ? made : TopoDS::Edge(made.Reversed()));
  }
  const TopoDS_Shape out = rs->Apply(s);
  if (out.IsNull() || !same_area(a, F) || !valid_around(out, {low, top}))
    return {};
  if (hist.IsNull())
    hist = new BRepTools_History();
  HHist h = rs->History();
  if (h.IsNull())
    h = new BRepTools_History();
  if (h->Modified(a).IsEmpty())
    h->AddModified(a, F);
  for (const TopoDS_Edge& e : seams)
    h->Remove(e);
  hist->Merge(h);
  return out;
}

} // namespace

// Face `a` of `s`, a cylinder band between two end circles, with its seam at angle `alpha` (band_seam_to). Null where
// it isn't such a band or the move doesn't come out valid.
TopoDS_Shape band_seam_at(const TopoDS_Shape& s, const TopoDS_Face& face, double alpha, HHist& hist)
{
  const TopoDS_Face                    a = TopoDS::Face(face.Oriented(TopAbs_FORWARD));
  TopLoc_Location                      la;
  occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(a, la));
  if (cs.IsNull())
    return {};
  const gp_Cylinder        ca = cs->Cylinder().Transformed(la.Transformation());
  std::vector<TopoDS_Edge> seams, ends, rest;
  ShapeIMap                seen;
  for (TopExp_Explorer x(a, TopAbs_EDGE); x.More(); x.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(x.Current());
    if (seen.Contains(e))
      continue;
    seen.Add(e);
    if (BRep_Tool::IsClosed(e, a))
    {
      seams.push_back(e);
      continue;
    }
    TopoDS_Vertex v0, v1;
    TopExp::Vertices(e, v0, v1);
    BRepAdaptor_Curve c(e);
    if (!v0.IsNull() && v0.IsSame(v1) && c.GetType() == GeomAbs_Circle && c.Circle().Axis().Direction().IsParallel(ca.Axis().Direction(), 1e-9))
      ends.push_back(e);
    else
      rest.push_back(e);
  }
  if (seams.empty() || ends.size() != 2)
    return {};
  return band_seam_to(s, a, la, ca, seams, ends, rest, alpha, hist);
}

namespace {

// A cylinder band between two end circles whose seam runs into its other edges (a hole through a tube across the
// tube's seam: the hole's loops split where the seam meets them, dx's CUT-CLOSED-EDGES-SPLIT; Parasolid's faces have no
// seams, live's loops are whole): the seam moved to the widest angle no other edge reaches, the end circles' vertices
// with it, the other edges the face's inner wires. One face at a time; the shape unchanged where nothing applies or
// the result isn't valid with the same area.
TopoDS_Shape move_seams_off_holes(const TopoDS_Shape& s, HHist& hist)
{
  for (TopExp_Explorer fx(s, TopAbs_FACE); fx.More(); fx.Next())
  {
    const TopoDS_Face                    a = TopoDS::Face(fx.Current().Oriented(TopAbs_FORWARD));
    TopLoc_Location                      la;
    occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(a, la));
    if (cs.IsNull())
      continue;
    const gp_Cylinder ca = cs->Cylinder().Transformed(la.Transformation());
    // Its seams, end circles (closed edges round the axis) and the rest.
    std::vector<TopoDS_Edge> seams, ends, rest;
    ShapeIMap                seen;
    bool                     ok = true;
    for (TopExp_Explorer x(a, TopAbs_EDGE); x.More() && ok; x.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(x.Current());
      if (seen.Contains(e))
        continue;
      seen.Add(e);
      if (BRep_Tool::IsClosed(e, a))
      {
        seams.push_back(e);
        continue;
      }
      TopoDS_Vertex v0, v1;
      TopExp::Vertices(e, v0, v1);
      BRepAdaptor_Curve c(e);
      if (!v0.IsNull() && v0.IsSame(v1) && c.GetType() == GeomAbs_Circle && c.Circle().Axis().Direction().IsParallel(ca.Axis().Direction(), 1e-9))
        ends.push_back(e);
      else
        rest.push_back(e);
    }
    if (seams.empty() || ends.empty() || ends.size() > 2 || rest.empty())
      continue;
    // Only where a seam meets the other edges.
    ShapeIMap rest_v;
    for (const TopoDS_Edge& e : rest)
      for (TopExp_Explorer v(e, TopAbs_VERTEX); v.More(); v.Next())
        rest_v.Add(v.Current());
    bool touches = false;
    for (const TopoDS_Edge& e : seams)
      for (TopExp_Explorer v(e, TopAbs_VERTEX); v.More() && !touches; v.Next())
        touches = rest_v.Contains(v.Current());
    if (!touches)
      continue;
    double u0, u1, v0f, v1f;
    BRepTools::UVBounds(a, u0, u1, v0f, v1f);
    if (std::abs(u1 - u0 - 2 * M_PI) > 1e-9)
      continue;
    if (ends.size() == 1)
    {
      const TopoDS_Shape out = seam_to_vertex(s, a, ca, seams, ends[0], rest, hist);
      if (out.IsNull())
        continue;
      return move_seams_off_holes(out, hist);
    }
    // The angles the other edges cover (their curves on the face, sampled): the new seam in the widest gap.
    std::vector<double> us;
    for (const TopoDS_Edge& e : rest)
    {
      double                          f2, l2;
      const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, a, f2, l2);
      if (pc.IsNull())
      {
        ok = false;
        break;
      }
      for (int k = 0; k <= 32; ++k)
        us.push_back(u0 + std::fmod(std::fmod(pc->Value(f2 + (l2 - f2) * k / 32.0).X() - u0, 2 * M_PI) + 2 * M_PI, 2 * M_PI));
    }
    if (!ok || us.empty())
      continue;
    std::sort(us.begin(), us.end());
    double gap = us.front() + 2 * M_PI - us.back(), alpha = us.back() + gap / 2;
    for (size_t k = 0; k + 1 < us.size(); ++k)
      if (us[k + 1] - us[k] > gap)
      {
        gap   = us[k + 1] - us[k];
        alpha = us[k] + gap / 2;
      }
    if (gap < 1e-3)
      continue;
    const TopoDS_Shape out = band_seam_to(s, a, la, ca, seams, ends, rest, alpha, hist);
    if (out.IsNull())
      continue;
    return move_seams_off_holes(out, hist);
  }
  return s;
}

// BRepCheck's verdict on a whole part, in parallel (cf-spur-gear's Boolean 2: 160 ms a check, two of them in its
// unify), but on this thread for a part with an edge or vertex looser than a hundredth of it: BRepCheck_Face's
// classification of such a face segfaults on the pool's threads (CRASH-3388).
bool valid_whole(const TopoDS_Shape& s)
{
  Bnd_Box box;
  BRepBndLib::Add(s, box);
  const double loose = box.IsVoid() ? 0 : 1e-2 * std::sqrt(box.SquareExtent());
  bool         tight = loose > 0;
  for (TopExp_Explorer x(s, TopAbs_EDGE); tight && x.More(); x.Next())
    tight = BRep_Tool::Tolerance(TopoDS::Edge(x.Current())) <= loose;
  for (TopExp_Explorer x(s, TopAbs_VERTEX); tight && x.More(); x.Next())
    tight = BRep_Tool::Tolerance(TopoDS::Vertex(x.Current())) <= loose;
  BudgetHold hold;
  return BRepCheck_Analyzer(s, true, tight).IsValid();
}

// Faces on one cylinder whose frames differ (seams apart, either hand: a hole through a boss in a mirrored plate,
// the plate's two half walls and the boss's), or any such faces with `any_frame` (after USD: those it left apart),
// sharing edges, the group's edges used once two closed chains round it or one loop on a part of it: OCCT's unify
// merges them by the first face's frame and leaves an open wire (38b2cd00) or stops at two faces (live 1767: each
// hole's wall one face). Merged here, on a copy: one face on the first one's surface, its curves on it the
// boundary's (projected, a turn from the seam), round it one new seam where both chains have a vertex at one angle.
// The shape unchanged where there's no such group or the merge doesn't come out valid with the same volume.
TopoDS_Shape merge_cylinder_groups(const TopoDS_Shape& s0, HHist& hist, bool any_frame)
{
  auto cyl_of = [](const TopoDS_Face& f, gp_Cylinder& c) {
    TopLoc_Location                      l;
    occ::handle<Geom_CylindricalSurface> cs = occ::down_cast<Geom_CylindricalSurface>(BRep_Tool::Surface(f, l));
    if (cs.IsNull())
      return false;
    c = cs->Cylinder().Transformed(l.Transformation());
    return true;
  };
  auto same_cyl = [](const gp_Cylinder& a, const gp_Cylinder& b) {
    const double r = a.Radius();
    return a.Axis().Direction().IsParallel(b.Axis().Direction(), 1e-12) && std::abs(a.Radius() - b.Radius()) <= 1e-9 * r
           && gp_Lin(a.Axis()).Distance(b.Location()) <= 1e-9 * (1 + r);
  };
  // An edge's curve on a group's first surface `ga` (its range within a bounded curve's: a B-spline edge can reach a
  // rounding past its curve's end, and a trimmed curve there throws).
  occ::handle<Geom_Surface> ga;
  auto                      project = [&ga](const occ::handle<Geom_Curve>& c3, double f, double l) -> occ::handle<Geom2d_Curve> {
    if (c3.IsNull())
      return nullptr;
    if (!c3->IsPeriodic())
    {
      f = std::max(f, c3->FirstParameter());
      l = std::min(l, c3->LastParameter());
    }
    if (!(l > f))
      return nullptr;
    return GeomProjLib::Curve2d(new Geom_TrimmedCurve(c3, f, l), f, l, ga);
  };
  auto same_frame = [](const gp_Cylinder& a, const gp_Cylinder& b) {
    const gp_Ax3 &x = a.Position(), &y = b.Position();
    return x.Direction().IsEqual(y.Direction(), 1e-12) && x.XDirection().IsEqual(y.XDirection(), 1e-12) && x.Direct() == y.Direct();
  };
  // The groups (in `s`), first: none whose frames differ, nothing to do (no copy).
  auto groups_of = [&](const TopoDS_Shape& s, const ShapeAncMap& ef, const ShapeIMap& fm) {
    std::vector<std::vector<int>> out;
    std::vector<char>             seen((size_t)fm.Extent() + 1, 0);
    for (int i = 1; i <= fm.Extent(); ++i)
    {
      gp_Cylinder c0;
      if (seen[(size_t)i] || !cyl_of(TopoDS::Face(fm(i)), c0))
        continue;
      std::vector<int> g{i};
      seen[(size_t)i] = 1;
      bool apart = false;
      for (size_t k = 0; k < g.size(); ++k)
        for (TopExp_Explorer x(fm(g[k]), TopAbs_EDGE); x.More(); x.Next())
          if (const ShapeList* fs = ef.Seek(x.Current()))
            for (const TopoDS_Shape& f : *fs)
            {
              const int   j = fm.FindIndex(f);
              gp_Cylinder cj;
              if (j == 0 || seen[(size_t)j] || !cyl_of(TopoDS::Face(f), cj) || !same_cyl(c0, cj))
                continue;
              seen[(size_t)j] = 1;
              apart           = apart || !same_frame(c0, cj);
              g.push_back(j);
            }
      if (g.size() > 1 && (apart || any_frame))
        out.push_back(g);
    }
    (void)s;
    return out;
  };
  {
    ShapeAncMap ef;
    unique_ancestors(s0, TopAbs_EDGE, TopAbs_FACE, ef);
    ShapeIMap fm;
    TopExp::MapShapes(s0, TopAbs_FACE, fm);
    if (groups_of(s0, ef, fm).empty())
      return s0;
  }
  BRepBuilderAPI_Copy cp(s0, true, false);
  TopoDS_Shape        s     = cp.Shape();
  HHist               local = new BRepTools_History(to_list(std::vector<TopoDS_Shape>{s0}), cp);
  bool                any   = false;
  for (int round = 0; round < 64; ++round)
  {
    ShapeAncMap ef;
    unique_ancestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
    ShapeIMap fm;
    TopExp::MapShapes(s, TopAbs_FACE, fm);
    bool merged = false;
    for (const std::vector<int>& g : groups_of(s, ef, fm))
    {
      gp_Cylinder C;
      const TopoDS_Face a = TopoDS::Face(fm(g[0]));
      cyl_of(a, C);
      // The boundary: the group's edges in one face of it only, not a seam of it.
      std::map<const void*, int> uses;
      for (int j : g)
      {
        ShapeIMap em;
        TopExp::MapShapes(fm(j), TopAbs_EDGE, em);
        for (int k = 1; k <= em.Extent(); ++k)
          uses[em(k).TShape().get()] += BRep_Tool::IsClosed(TopoDS::Edge(em(k)), TopoDS::Face(fm(j))) ? 2 : 1;
      }
      std::vector<TopoDS_Edge> bnd;
      for (int j : g)
      {
        ShapeIMap em;
        TopExp::MapShapes(fm(j), TopAbs_EDGE, em);
        for (int k = 1; k <= em.Extent(); ++k)
          if (uses[em(k).TShape().get()] == 1)
            bnd.push_back(TopoDS::Edge(em(k).Oriented(TopAbs_FORWARD)));
      }
      // Closed chains of them.
      auto ends = [](const TopoDS_Edge& e, TopoDS_Vertex& p, TopoDS_Vertex& q) { TopExp::Vertices(e, p, q); };
      std::vector<std::vector<int>> chains;
      std::vector<char>             used(bnd.size(), 0);
      bool                          closed = true;
      for (size_t k = 0; k < bnd.size(); ++k)
      {
        if (used[k])
          continue;
        std::vector<int> ch{(int)k};
        used[k] = 1;
        TopoDS_Vertex v0, end;
        ends(bnd[k], v0, end);
        while (!end.IsSame(v0))
        {
          int next = -1;
          for (size_t m = 0; m < bnd.size() && next < 0; ++m)
          {
            TopoDS_Vertex p, q;
            ends(bnd[m], p, q);
            if (used[m])
              continue;
            if (p.IsSame(end))
            {
              next = (int)m;
              end  = q;
            }
            else if (q.IsSame(end))
            {
              next = (int)m;
              end  = p;
            }
          }
          if (next < 0)
            break;
          used[(size_t)next] = 1;
          ch.push_back(next);
        }
        closed = closed && end.IsSame(v0);
        chains.push_back(ch);
      }
      if (!closed || chains.empty() || chains.size() > 2)
        continue;
      TopLoc_Location la;
      ga         = BRep_Tool::Surface(a, la);
      double tol = 1e-7;
      for (const TopoDS_Edge& e : bnd)
        tol = std::max(tol, BRep_Tool::Tolerance(e));
      // The merged face in place of the group's, kept where valid with the same volume.
      auto replace_by = [&](const TopoDS_Face& F) {
        if (!BRepCheck_Analyzer(F).IsValid())
          return false;
        occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
        rs->Replace(a.Oriented(TopAbs_FORWARD), F);
        for (size_t k = 1; k < g.size(); ++k)
          rs->Remove(fm(g[k]));
        const TopoDS_Shape out = rs->Apply(s);
        if (out.IsNull() || !valid_whole(out))
          return false;
        // The volume kept: the new face's share of it against the group's faces' (one integral of each side; the
        // whole solid's integral is noisier than the change: 2.9e-7 of it on a part fused with its mirror, 11ed9bd7,
        // where the faces' shares agree to 1e-11 mm³), within 1e-9 of the box's volume.
        std::vector<TopoDS_Shape> before;
        for (int j : g)
          before.push_back(fm(j));
        Bnd_Box box;
        BRepBndLib::Add(s, box, false);
        const double bv = box.IsVoid() ? 0 : std::pow(std::sqrt(box.SquareExtent()), 3);
        if (std::abs(faces_volume({F.Oriented(a.Orientation())}) - faces_volume(before)) > 1e-9 * bv)
          return false;
        HHist h = rs->History();
        if (h.IsNull())
          h = new BRepTools_History();
        for (size_t k = 1; k < g.size(); ++k)
          h->AddModified(fm(g[k]), F);
        local->Merge(h);
        s = out;
        return true;
      };
      if (chains.size() == 1)
      {
        // One loop: a part of the cylinder (a half hole's wall in two inputs' faces). Its curves with u continuous
        // round it, counter-clockwise; none where it goes round the cylinder.
        std::vector<TopoDS_Edge> loop;
        {
          const std::vector<int>& ch = chains[0];
          std::vector<char>       done(ch.size(), 0);
          TopoDS_Vertex           v0, at;
          ends(bnd[(size_t)ch[0]], v0, at);
          loop.push_back(bnd[(size_t)ch[0]]);
          done[0] = 1;
          for (size_t n = 1; n < ch.size(); ++n)
            for (size_t k = 0; k < ch.size(); ++k)
            {
              if (done[k])
                continue;
              TopoDS_Vertex p, q;
              ends(bnd[(size_t)ch[k]], p, q);
              if (p.IsSame(at) || q.IsSame(at))
              {
                loop.push_back(TopoDS::Edge(bnd[(size_t)ch[k]].Oriented(p.IsSame(at) ? TopAbs_FORWARD : TopAbs_REVERSED)));
                at      = p.IsSame(at) ? q : p;
                done[k] = 1;
                break;
              }
            }
          if (loop.size() != ch.size())
            continue;
        }
        std::map<const void*, occ::handle<Geom2d_Curve>> pc;
        gp_Pnt2d                                         prev, first;
        bool                                             ok = true;
        for (size_t n = 0; n < loop.size() && ok; ++n)
        {
          const TopoDS_Edge             e = TopoDS::Edge(loop[n].Oriented(TopAbs_FORWARD));
          double                        f, l;
          const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(e, f, l);
          occ::handle<Geom2d_Curve>     c2 = project(c3, f, l);
          if (c2.IsNull())
          {
            ok = false;
            break;
          }
          const bool fwd = loop[n].Orientation() != TopAbs_REVERSED;
          if (n > 0)
          {
            const double k = std::round((prev.X() - c2->Value(fwd ? f : l).X()) / (2 * M_PI));
            if (k != 0)
              c2->Translate(gp_Vec2d(k * 2 * M_PI, 0));
            ok = c2->Value(fwd ? f : l).Distance(prev) <= 1e-6;
          }
          else
            first = c2->Value(fwd ? f : l);
          prev                 = c2->Value(fwd ? l : f);
          pc[e.TShape().get()] = c2;
        }
        if (!ok || first.Distance(prev) > 1e-6)
          continue;
        // Its area's sign in (u, v): counter-clockwise for a face on the surface's side.
        double area = 0;
        for (const TopoDS_Edge& e : loop)
        {
          double f, l;
          BRep_Tool::Range(e, f, l);
          const occ::handle<Geom2d_Curve>& c2 = pc[e.TShape().get()];
          for (int k = 0; k < 32; ++k)
          {
            const bool     fwd = e.Orientation() != TopAbs_REVERSED;
            const double   t0 = fwd ? f + (l - f) * k / 32 : l - (l - f) * k / 32, t1 = fwd ? f + (l - f) * (k + 1) / 32 : l - (l - f) * (k + 1) / 32;
            const gp_Pnt2d p = c2->Value(t0), q = c2->Value(t1);
            area += p.X() * q.Y() - q.X() * p.Y();
          }
        }
        BRep_Builder B;
        TopoDS_Face  F;
        B.MakeFace(F, ga, la, BRep_Tool::Tolerance(a));
        for (const TopoDS_Edge& e : loop)
          B.UpdateEdge(TopoDS::Edge(e.Oriented(TopAbs_FORWARD)), pc[e.TShape().get()], F, tol);
        TopoDS_Wire W;
        B.MakeWire(W);
        if (area > 0)
          for (const TopoDS_Edge& e : loop)
            B.Add(W, e);
        else
          for (auto it = loop.rbegin(); it != loop.rend(); ++it)
            B.Add(W, it->Reversed());
        B.Add(F, W);
        if (!replace_by(F))
          continue;
        merged = any = true;
        break;
      }
      auto uv = [&](const gp_Pnt& p) {
        double u, v;
        ElSLib::Parameters(C, p, u, v);
        return gp_Pnt2d(u, v);
      };
      // A vertex of each chain at one angle: the seam's ends (of several such pairs, the one whose lower vertex comes
      // first by its point: the same seam whichever face's frame is the reference, OCCT's result's or ours).
      TopoDS_Vertex                               vb, vt;
      std::tuple<long long, long long, long long> best;
      auto key = [](const gp_Pnt& r) { return std::make_tuple(std::llround(r.X() * 1e9), std::llround(r.Y() * 1e9), std::llround(r.Z() * 1e9)); };
      for (int k : chains[0])
        for (TopExp_Explorer x(bnd[(size_t)k], TopAbs_VERTEX); x.More(); x.Next())
          for (int m : chains[1])
            for (TopExp_Explorer y(bnd[(size_t)m], TopAbs_VERTEX); y.More(); y.Next())
            {
              const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(x.Current())), q = BRep_Tool::Pnt(TopoDS::Vertex(y.Current()));
              if (std::abs(std::remainder(uv(p).X() - uv(q).X(), 2 * M_PI)) >= 1e-9)
                continue;
              const auto kk = key(uv(p).Y() < uv(q).Y() ? p : q);
              if (vb.IsNull() || kk < best)
              {
                best = kk;
                vb   = TopoDS::Vertex(x.Current());
                vt   = TopoDS::Vertex(y.Current());
              }
            }
      if (vb.IsNull())
        continue;
      gp_Pnt2d         qb = uv(BRep_Tool::Pnt(vb)), qt = uv(BRep_Tool::Pnt(vt));
      std::vector<int> lowc = chains[0], upc = chains[1];
      if (qb.Y() > qt.Y())
      {
        std::swap(lowc, upc);
        std::swap(vb, vt);
        std::swap(qb, qt);
      }
      const double u0 = qb.X();
      // Each boundary edge's curve on the new face, its u within a turn from the seam by its middle, its ends checked.
      std::map<const void*, occ::handle<Geom2d_Curve>> pc;
      bool                                             projected = true;
      for (const TopoDS_Edge& e : bnd)
      {
        double                        f, l;
        const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(e, f, l);
        occ::handle<Geom2d_Curve>     c2 = project(c3, f, l);
        if (c2.IsNull())
        {
          projected = false;
          break;
        }
        const double k = std::floor((c2->Value(0.5 * (f + l)).X() - u0) / (2 * M_PI));
        if (k != 0)
          c2->Translate(gp_Vec2d(-k * 2 * M_PI, 0));
        for (const double t : {f, l})
        {
          const gp_Pnt2d q = c2->Value(t);
          projected        = projected && ga->Value(q.X(), q.Y()).Transformed(la.Transformation()).Distance(c3->Value(t)) <= tol;
        }
        pc[e.TShape().get()] = c2;
      }
      if (!projected)
        continue;
      // A chain from its seam vertex, the way u goes (up: growing).
      auto chain_from = [&](const std::vector<int>& ch, const TopoDS_Vertex& v, bool up, std::vector<TopoDS_Edge>& out) {
        std::vector<std::pair<int, bool>> starts;
        for (size_t m = 0; m < ch.size(); ++m)
        {
          TopoDS_Vertex p, q;
          ends(bnd[(size_t)ch[m]], p, q);
          if (p.IsSame(v))
            starts.push_back({(int)m, true});
          if (q.IsSame(v))
            starts.push_back({(int)m, false});
        }
        for (const auto& [m0, f0] : starts)
        {
          out.clear();
          std::vector<char> done(ch.size(), 0);
          int               m   = m0;
          bool              fwd = f0, ok = true;
          TopoDS_Vertex     at  = v;
          for (size_t n = 0; n < ch.size(); ++n)
          {
            done[(size_t)m]     = 1;
            const TopoDS_Edge e = bnd[(size_t)ch[(size_t)m]];
            TopoDS_Vertex     p, q;
            ends(e, p, q);
            out.push_back(TopoDS::Edge(e.Oriented(fwd ? TopAbs_FORWARD : TopAbs_REVERSED)));
            at = fwd ? q : p;
            if (n + 1 == ch.size())
              break;
            int next = -1;
            for (size_t k = 0; k < ch.size() && next < 0; ++k)
            {
              if (done[k])
                continue;
              TopoDS_Vertex a2, b2;
              ends(bnd[(size_t)ch[k]], a2, b2);
              if (a2.IsSame(at))
              {
                next = (int)k;
                fwd  = true;
              }
              else if (b2.IsSame(at))
              {
                next = (int)k;
                fwd  = false;
              }
            }
            if (next < 0)
            {
              ok = false;
              break;
            }
            m = next;
          }
          if (!ok || !at.IsSame(v))
            continue;
          double f, l;
          BRep_Tool::Range(out[0], f, l);
          const occ::handle<Geom2d_Curve>& c2 = pc[out[0].TShape().get()];
          double                           d  = c2->Value(0.75 * l + 0.25 * f).X() - c2->Value(0.25 * l + 0.75 * f).X();
          if (out[0].Orientation() == TopAbs_REVERSED)
            d = -d;
          if ((d > 0) == up)
            return true;
        }
        return false;
      };
      std::vector<TopoDS_Edge> low, upper;
      if (!chain_from(lowc, vb, true, low) || !chain_from(upc, vt, false, upper))
        continue;
      // The lower chain from u0, the upper from u0 + 2 pi (its first curve moved a turn if it starts on the other side).
      for (const auto& [es, start] : {std::make_pair(&low, u0), std::make_pair(&upper, u0 + 2 * M_PI)})
      {
        double f, l;
        BRep_Tool::Range((*es)[0], f, l);
        occ::handle<Geom2d_Curve>& c2 = pc[(*es)[0].TShape().get()];
        const double               u  = c2->Value((*es)[0].Orientation() != TopAbs_REVERSED ? f : l).X();
        if (std::abs(u - start) > 1e-9)
          c2->Translate(gp_Vec2d(start - u, 0));
      }
      BRep_Builder B;
      TopoDS_Face  F;
      B.MakeFace(F, ga, la, BRep_Tool::Tolerance(a));
      for (const TopoDS_Edge& e : bnd)
        B.UpdateEdge(e, pc[e.TShape().get()], F, tol);
      // The seam: up the axis (a's v) from the lower vertex.
      const double len = qt.Y() - qb.Y();
      TopoDS_Edge  seam;
      B.MakeEdge(seam, new Geom_Line(BRep_Tool::Pnt(vb), C.Axis().Direction()), tol);
      B.Add(seam, vb.Oriented(TopAbs_FORWARD));
      B.Add(seam, vt.Oriented(TopAbs_REVERSED));
      B.Range(seam, 0, len);
      B.UpdateEdge(seam, new Geom2d_Line(gp_Pnt2d(u0 + 2 * M_PI, qb.Y()), gp_Dir2d(0, 1)), new Geom2d_Line(gp_Pnt2d(u0, qb.Y()), gp_Dir2d(0, 1)), F, tol);
      B.Range(seam, F, 0, len);
      TopoDS_Wire W;
      B.MakeWire(W);
      for (const TopoDS_Edge& e : low)
        B.Add(W, e);
      B.Add(W, seam.Oriented(TopAbs_FORWARD));
      for (const TopoDS_Edge& e : upper)
        B.Add(W, e);
      B.Add(W, seam.Oriented(TopAbs_REVERSED));
      B.Add(F, W);
      if (!replace_by(F))
        continue;
      merged = any = true;
      break;
    }
    if (!merged)
      break;
  }
  if (!any)
    return s0;
  if (hist.IsNull())
    hist = new BRepTools_History();
  hist->Merge(local);
  return s;
}

// Adjacent faces on equal tori or spheres (one frame, the same radii) given one surface, on a copy: OCCT's unify
// tells two tori are one only by intersecting them, which it can't (pulley02's groove: two rounds, each half its
// height, on one torus, live one face); with one surface it merges them. The shape unchanged when there's no such
// pair.
TopoDS_Shape share_equal_surfaces(const TopoDS_Shape& s, HHist& hist)
{
  ShapeAncMap ef;
  unique_ancestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  auto frame_of = [](const TopoDS_Face& f, gp_Ax3& ax, double& r1, double& r2) {
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> g = BRep_Tool::Surface(f, loc);
    if (const occ::handle<Geom_ToroidalSurface> t = occ::down_cast<Geom_ToroidalSurface>(g); !t.IsNull())
    {
      const gp_Torus x = t->Torus().Transformed(loc.Transformation());
      ax = x.Position(), r1 = x.MajorRadius(), r2 = x.MinorRadius();
      return 1;
    }
    if (const occ::handle<Geom_SphericalSurface> p = occ::down_cast<Geom_SphericalSurface>(g); !p.IsNull())
    {
      const gp_Sphere x = p->Sphere().Transformed(loc.Transformation());
      ax = x.Position(), r1 = x.Radius(), r2 = 0;
      return 2;
    }
    return 0;
  };
  const double tol = Precision::Confusion();
  std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> pairs;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const ShapeList& fs = ef(i);
    if (fs.Extent() != 2 || fs.First().IsSame(fs.Last()))
      continue;
    const TopoDS_Face a = TopoDS::Face(fs.First()), b = TopoDS::Face(fs.Last());
    TopLoc_Location   la, lb;
    if (BRep_Tool::Surface(a, la) == BRep_Tool::Surface(b, lb) && la.IsEqual(lb))
      continue;
    gp_Ax3 xa, xb;
    double a1, a2, b1, b2;
    const int ka = frame_of(a, xa, a1, a2), kb = frame_of(b, xb, b1, b2);
    if (ka == 0 || ka != kb || std::abs(a1 - b1) > tol || std::abs(a2 - b2) > tol || xa.Location().Distance(xb.Location()) > tol ||
        !xa.Direction().IsEqual(xb.Direction(), 1e-9) || !xa.XDirection().IsEqual(xb.XDirection(), 1e-9) || xa.Direct() != xb.Direct())
      continue;
    pairs.emplace_back(a, b);
  }
  if (pairs.empty())
    return s;
  BRepBuilderAPI_Copy cp(s, true, false);
  const TopoDS_Shape  out = cp.Shape();
  // Each face's surface: its first partner's, followed through chains of them.
  NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher> to;
  auto root = [&](TopoDS_Shape f) {
    while (const TopoDS_Shape* n = to.Seek(f))
      f = *n;
    return f;
  };
  for (const auto& [a, b] : pairs)
  {
    const TopoDS_Shape ra = root(cp.ModifiedShape(a)), rb = root(cp.ModifiedShape(b));
    if (!ra.IsSame(rb))
      to.Bind(rb, ra);
  }
  BRep_Builder bld;
  for (NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher>::Iterator it(to); it.More(); it.Next())
  {
    const TopoDS_Face f = TopoDS::Face(it.Key()), g = TopoDS::Face(root(it.Key()));
    if (!f.Location().IsIdentity() || !g.Location().IsIdentity())
      continue;
    TopLoc_Location   lg;
    const occ::handle<Geom_Surface> sg = BRep_Tool::Surface(g, lg);
    // Its edges' curves on it, kept by surface: read off the old one, put on the new one (the same parameters).
    struct OnFace
    {
      TopoDS_Edge                e;
      occ::handle<Geom2d_Curve> c1, c2;
      double                     a, b;
    };
    std::vector<OnFace> curves;
    ShapeIMap           seen;
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
    {
      if (seen.Contains(x.Current()))
        continue;
      seen.Add(x.Current());
      OnFace o;
      o.e  = TopoDS::Edge(x.Current().Oriented(TopAbs_FORWARD));
      o.c1 = BRep_Tool::CurveOnSurface(o.e, f, o.a, o.b);
      if (o.c1.IsNull())
        return s;
      if (BRep_Tool::IsClosed(o.e, f))
      {
        double a2, b2;
        o.c2 = BRep_Tool::CurveOnSurface(TopoDS::Edge(o.e.Reversed()), f, a2, b2);
      }
      curves.push_back(o);
    }
    bld.UpdateFace(f, sg, lg, BRep_Tool::Tolerance(f));
    for (const OnFace& o : curves)
    {
      if (o.c2.IsNull())
        bld.UpdateEdge(o.e, o.c1, sg, lg, BRep_Tool::Tolerance(o.e));
      else
        bld.UpdateEdge(o.e, o.c1, o.c2, sg, lg, BRep_Tool::Tolerance(o.e));
      bld.Range(o.e, sg, lg, o.a, o.b);
    }
  }
  if (hist.IsNull())
    hist = new BRepTools_History();
  hist->Merge(new BRepTools_History(to_list({s}), cp));
  return out;
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
TopoDS_Shape unify_faces_in(const TopoDS_Shape& s0, HHist& hist, const UnifyScope* sc, bool* clean);
TopoDS_Shape unify_seams_in(const TopoDS_Shape& s0, HHist& hist, const UnifyScope* sc, bool* clean);

// A loop's pieces left meeting at vertices of just two curved edges between the same two faces (where a seam crossed
// it): one edge, the pieces' curves (3D and on both faces) concatenated exactly, their parameters kept. `s` when
// there are none or the result isn't valid.
// Whether some u-periodic face has a curved edge ending at u = 0 of its surface where the face has no seam: the only
// place join_loop_pieces joins. Linear, no maps (most bodies have none: a hole's circle ends on its seam).
// Edges `a` and `b` arcs of one circle (where a blend kept a vertex between them: stef-radijator's chamfered hole).
bool one_circle(const TopoDS_Edge& a, const TopoDS_Edge& b)
{
  BRepAdaptor_Curve ca(a), cb(b);
  if (ca.GetType() != GeomAbs_Circle || cb.GetType() != GeomAbs_Circle)
    return false;
  const gp_Circ x = ca.Circle(), y = cb.Circle();
  // (At least OCCT's confusion: a blend built at the working scale comes back with 1e-10 m edges, and a full round's
  // two halves, a hair smaller, have centres 1.5e-9 m apart: clamp-guillem's slot.)
  const double tol = std::max({BRep_Tool::Tolerance(a), BRep_Tool::Tolerance(b), Precision::Confusion()});
  return x.Location().Distance(y.Location()) <= tol && std::abs(x.Radius() - y.Radius()) <= tol && x.Axis().Direction().IsParallel(y.Axis().Direction(), 1e-9);
}

// Two B-spline edges meeting smoothly at `v` (OCCT's pieces of one intersection curve, split where it turns: a hole's
// rim through a tube's wall in three, HOLE-RIM-PIECES; Parasolid's is one closed edge).
bool smooth_pieces(const TopoDS_Edge& a, const TopoDS_Edge& b, const TopoDS_Vertex& v)
{
  BRepAdaptor_Curve ca(a), cb(b);
  if (ca.GetType() != GeomAbs_BSplineCurve || cb.GetType() != GeomAbs_BSplineCurve)
    return false;
  gp_Pnt pa, pb;
  gp_Vec ta, tb;
  ca.D1(BRep_Tool::Parameter(v, a), pa, ta);
  cb.D1(BRep_Tool::Parameter(v, b), pb, tb);
  if (ta.Magnitude() < 1e-12 || tb.Magnitude() < 1e-12)
    return false;
  return ta.Crossed(tb).Magnitude() <= 1e-6 * ta.Magnitude() * tb.Magnitude();
}

bool loop_joint_candidates(const TopoDS_Shape& s)
{
  {
    ShapeAncMap ve;
    TopExp::MapShapesAndUniqueAncestors(s, TopAbs_VERTEX, TopAbs_EDGE, ve);
    for (int i = 1; i <= ve.Extent(); ++i)
      if (ve(i).Extent() == 2
          && (one_circle(TopoDS::Edge(ve(i).First()), TopoDS::Edge(ve(i).Last())) || (!legacy_topology() && smooth_pieces(TopoDS::Edge(ve(i).First()), TopoDS::Edge(ve(i).Last()), TopoDS::Vertex(ve.FindKey(i))))))
        return true;
  }
  for (TopExp_Explorer fx(s, TopAbs_FACE); fx.More(); fx.Next())
  {
    const TopoDS_Face&              f = TopoDS::Face(fx.Current());
    const occ::handle<Geom_Surface> g = BRep_Tool::Surface(f);
    if (g.IsNull() || !g->IsUPeriodic())
      continue;
    ShapeIMap seam_v;
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
      if (BRep_Tool::IsClosed(TopoDS::Edge(x.Current()), f))
        for (TopExp_Explorer v(x.Current(), TopAbs_VERTEX); v.More(); v.Next())
          seam_v.Add(v.Current());
    for (TopExp_Explorer x(f, TopAbs_EDGE); x.More(); x.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(x.Current());
      if (BRep_Tool::Degenerated(e) || BRep_Tool::IsClosed(e, f))
        continue;
      TopoDS_Vertex v0, v1;
      TopExp::Vertices(e, v0, v1);
      if (v0.IsNull() || v0.IsSame(v1) || BRepAdaptor_Curve(e).GetType() == GeomAbs_Line)
        continue;
      for (const TopoDS_Vertex& v : {v0, v1})
        if (!seam_v.Contains(v) && std::abs(std::remainder(BRep_Tool::Parameters(v, f).X(), g->UPeriod())) < 1e-7)
          return true;
    }
  }
  return false;
}

TopoDS_Shape join_loop_pieces(const TopoDS_Shape& s, HHist& hist)
{
  {
    const KernelTimer timer("unify.loop_check");
    if (!loop_joint_candidates(s))
      return s;
  }
  const KernelTimer timer("unify.loop_pieces");
  ShapeAncMap       ve, ef;
  unique_ancestors(s, TopAbs_VERTEX, TopAbs_EDGE, ve);
  // The joints: two curved edges, neither a seam, on the same two faces.
  std::vector<int> joints;
  bool             mapped = false;
  for (int i = 1; i <= ve.Extent(); ++i)
  {
    const ShapeList& es = ve(i);
    if (es.Extent() != 2)
      continue;
    const TopoDS_Edge a = TopoDS::Edge(es.First()), b = TopoDS::Edge(es.Last());
    if (a.IsSame(b) || BRep_Tool::Degenerated(a) || BRep_Tool::Degenerated(b) || BRepAdaptor_Curve(a).GetType() == GeomAbs_Line || BRepAdaptor_Curve(b).GetType() == GeomAbs_Line)
      continue;
    if (!mapped)
    {
      unique_ancestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
      mapped = true;
    }
    const ShapeList *fa = ef.Seek(a), *fb = ef.Seek(b);
    if (fa == nullptr || fb == nullptr || fa->Extent() != 2 || fb->Extent() != 2 || fa->First().IsSame(fa->Last()))
      continue;
    if (!((fa->First().IsSame(fb->First()) && fa->Last().IsSame(fb->Last())) || (fa->First().IsSame(fb->Last()) && fa->Last().IsSame(fb->First()))))
      continue;
    if (BRep_Tool::IsClosed(a, TopoDS::Face(fa->First())) || BRep_Tool::IsClosed(a, TopoDS::Face(fa->Last())) || BRep_Tool::IsClosed(b, TopoDS::Face(fa->First())) || BRep_Tool::IsClosed(b, TopoDS::Face(fa->Last())))
      continue;
    // Only where a seam split the loop (at u = 0, a whole period, of a periodic face's surface, OCCT's seams' place),
    // two arcs of one circle meet alone, or two B-spline pieces meet smoothly.
    bool at_seam = false;
    for (const TopoDS_Shape& fx : {fa->First(), fa->Last()})
    {
      const TopoDS_Face&              f = TopoDS::Face(fx);
      const occ::handle<Geom_Surface> g = BRep_Tool::Surface(f);
      if (g.IsNull() || !g->IsUPeriodic())
        continue;
      const gp_Pnt2d uv = BRep_Tool::Parameters(TopoDS::Vertex(ve.FindKey(i)), f);
      at_seam = at_seam || std::abs(std::remainder(uv.X(), g->UPeriod())) < 1e-7;
    }
    if (at_seam || one_circle(a, b) || (!legacy_topology() && smooth_pieces(a, b, TopoDS::Vertex(ve.FindKey(i)))))
      joints.push_back(i);
  }
  if (joints.empty())
    return s;
  ShapeIMap joint_v;
  for (int i : joints)
    joint_v.Add(ve.FindKey(i));
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape();
  BRep_Builder                   B;
  ShapeIMap                      used;
  std::vector<std::pair<std::vector<TopoDS_Edge>, TopoDS_Edge>> made;
  for (int i : joints)
  {
    const TopoDS_Vertex j = TopoDS::Vertex(ve.FindKey(i));
    if (used.Contains(ve(i).First()))
      continue;
    // Back from j to the chain's start (a vertex that isn't a joint, or j again: a closed chain).
    TopoDS_Vertex start = j;
    TopoDS_Edge   prev  = TopoDS::Edge(ve(i).First());
    for (int guard = 0; guard < 1000; ++guard)
    {
      TopoDS_Vertex v0, v1;
      TopExp::Vertices(prev, v0, v1);
      const TopoDS_Vertex other = v0.IsSame(start) ? v1 : v0;
      if (!joint_v.Contains(other) || other.IsSame(j))
      {
        start = joint_v.Contains(other) ? j : other;
        break;
      }
      const ShapeList& es = ve.FindFromKey(other);
      prev  = TopoDS::Edge(es.First().IsSame(prev) ? es.Last() : es.First());
      start = other;
    }
    // Forward from start, by the edge back there (a corner has others on the same faces).
    std::vector<TopoDS_Edge> chain;
    TopoDS_Vertex            at = start;
    TopoDS_Edge              last;
    for (int guard = 0; guard < 1000; ++guard)
    {
      const ShapeList* es = ve.Seek(at);
      if (es == nullptr)
        break;
      TopoDS_Edge next;
      for (ShapeList::Iterator it(*es); it.More() && next.IsNull(); it.Next())
        if (!used.Contains(it.Value()) && (chain.empty() ? it.Value().IsSame(prev) : joint_v.Contains(at)) && !it.Value().IsSame(last))
        {
          const ShapeList* fl = ef.Seek(it.Value());
          const ShapeList* f0 = chain.empty() ? fl : ef.Seek(chain.front());
          if (fl != nullptr && f0 != nullptr && fl->Extent() == 2 && (fl->First().IsSame(f0->First()) || fl->First().IsSame(f0->Last())) && (fl->Last().IsSame(f0->First()) || fl->Last().IsSame(f0->Last())))
            next = TopoDS::Edge(it.Value());
        }
      if (next.IsNull())
        break;
      used.Add(next);
      TopoDS_Edge e = TopoDS::Edge(next.Oriented(TopAbs_FORWARD));
      if (!TopExp::FirstVertex(e).IsSame(at))
        e.Reverse();
      chain.push_back(e);
      last = next;
      at   = TopExp::LastVertex(e, true);
      if (at.IsSame(start) || !joint_v.Contains(at))
        break;
    }
    if (chain.size() < 2)
      continue;
    const TopoDS_Face F[2] = {TopoDS::Face(ef.FindFromKey(chain[0]).First()), TopoDS::Face(ef.FindFromKey(chain[0]).Last())};
    occ::handle<Geom_BSplineCurve>   c3;
    occ::handle<Geom2d_BSplineCurve> c2[2];
    double                                                 tol = 0;
    bool                                                   ok  = true;
    for (const TopoDS_Edge& e : chain)
    {
      tol = std::max(tol, BRep_Tool::Tolerance(e));
      const bool                    rev = e.Orientation() == TopAbs_REVERSED;
      const TopoDS_Edge             ef_ = TopoDS::Edge(e.Oriented(TopAbs_FORWARD));
      double                        a = 0, b = 0;
      TopLoc_Location               el;
      const occ::handle<Geom_Curve> c = BRep_Tool::Curve(ef_, el, a, b);
      if (c.IsNull())
      {
        ok = false;
        break;
      }
      occ::handle<Geom_BSplineCurve> bs = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(occ::down_cast<Geom_Curve>(c->Transformed(el.Transformation())), a, b));
      if (rev)
        bs->Reverse();
      // Each piece's curve on the faces with the same parameter (its 3D one's), a whole number of periods on.
      occ::handle<Geom2d_BSplineCurve> b2[2];
      for (int k = 0; k < 2 && ok; ++k)
      {
        double                          f2, l2;
        const occ::handle<Geom2d_Curve> p = BRep_Tool::CurveOnSurface(ef_, F[k], f2, l2);
        if (p.IsNull() || std::abs(f2 - a) > 1e-9 || std::abs(l2 - b) > 1e-9)
        {
          ok = false;
          break;
        }
        b2[k] = Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(p, f2, l2));
        if (rev)
          b2[k]->Reverse();
        if (!c2[k].IsNull())
        {
          const occ::handle<Geom_Surface> g  = BRep_Tool::Surface(F[k]);
          const gp_Pnt2d                  e0 = c2[k]->EndPoint(), s0 = b2[k]->StartPoint();
          const double pu = g->IsUPeriodic() ? g->UPeriod() : 0, pv = g->IsVPeriodic() ? g->VPeriod() : 0;
          const double ku = pu > 0 ? std::round((e0.X() - s0.X()) / pu) : 0, kv = pv > 0 ? std::round((e0.Y() - s0.Y()) / pv) : 0;
          if (ku != 0 || kv != 0)
            b2[k]->Translate(gp_Vec2d(ku * pu, kv * pv));
          // (Apart in the parameters but not in space: a full round's halves, a hair smaller, have centres 1.5e-9 m apart,
          // 2e-7 rad on a 7.5 mm cylinder.)
          const gp_Pnt2d s1 = b2[k]->StartPoint();
          if (s1.Distance(e0) > 1e-7 && g->Value(s1.X(), s1.Y()).Distance(g->Value(e0.X(), e0.Y())) > std::max(tol, Precision::Confusion()))
            ok = false;
        }
      }
      if (!ok)
        break;
      if (!c3.IsNull() && bs->StartPoint().Distance(c3->EndPoint()) > tol + Precision::Confusion())
      {
        ok = false;
        break;
      }
      c3 = c3.IsNull() ? bs : concat_shifted<Geom_BSplineCurve, gp_Pnt, NCollection_Array1<gp_Pnt>>(c3, bs);
      for (int k = 0; k < 2; ++k)
        c2[k] = c2[k].IsNull() ? b2[k] : concat_shifted<Geom2d_BSplineCurve, gp_Pnt2d, NCollection_Array1<gp_Pnt2d>>(c2[k], b2[k]);
      if (!ok)
        break;
    }
    if (!ok)
      continue;
    occ::handle<Geom_Curve>   curve = c3;
    occ::handle<Geom2d_Curve> on[2] = {c2[0], c2[1]};
    double                    p0 = c3->FirstParameter(), p1 = c3->LastParameter();
    const TopoDS_Vertex vs = TopExp::FirstVertex(chain.front(), true), vl = TopExp::LastVertex(chain.back(), true);
    // Arcs of one circle: one circle again, its curves on the faces projected (a plane's a circle, a cylinder's a
    // line), so the edge stays analytic (the B-spline's integrals moved a pin-and-plate union's volume 3e-7).
    {
      gp_Circ circ;
      bool    one = true;
      double  len = 0;
      for (size_t k = 0; k < chain.size() && one; ++k)
      {
        BRepAdaptor_Curve ck(chain[k]);
        one = ck.GetType() == GeomAbs_Circle;
        if (!one)
          break;
        const gp_Circ c = ck.Circle();
        if (k == 0)
          circ = c;
        else
          one = c.Location().Distance(circ.Location()) <= std::max(tol, Precision::Confusion()) && std::abs(c.Radius() - circ.Radius()) <= std::max(tol, Precision::Confusion()) && c.Axis().Direction().IsParallel(circ.Axis().Direction(), 1e-9);
        len += GCPnts_AbscissaPoint::Length(ck);
      }
      if (one)
      {
        // Along the chain: the first piece's way from vs.
        BRepAdaptor_Curve c0(TopoDS::Edge(chain.front().Oriented(TopAbs_FORWARD)));
        gp_Pnt            q;
        gp_Vec            d;
        const bool        fwd = chain.front().Orientation() != TopAbs_REVERSED;
        c0.D1(fwd ? c0.FirstParameter() : c0.LastParameter(), q, d);
        if (!fwd)
          d.Reverse();
        const gp_Pnt Pv = BRep_Tool::Pnt(vs);
        gp_Pnt       cq;
        gp_Vec       cd;
        ElCLib::D1(ElCLib::Parameter(circ, Pv), circ, cq, cd);
        if (cd.Dot(d) < 0)
          circ = gp_Circ(gp_Ax2(circ.Location(), circ.Axis().Direction().Reversed(), circ.XAxis().Direction()), circ.Radius());
        const double                  a  = ElCLib::Parameter(circ, Pv), b = a + len / circ.Radius();
        const occ::handle<Geom_Curve> cc = new Geom_Circle(circ);
        occ::handle<Geom2d_Curve>     pc[2];
        for (int k = 0; k < 2 && one; ++k)
        {
          const occ::handle<Geom_Surface> g = BRep_Tool::Surface(F[k]);
          pc[k]                            = GeomProjLib::Curve2d(new Geom_TrimmedCurve(cc, a, b), a, b, g);
          one                              = !pc[k].IsNull();
          if (!one)
            break;
          const gp_Pnt2d want = c2[k]->Value(c2[k]->FirstParameter()), got = pc[k]->Value(a);
          const double   du   = g->IsUPeriodic() ? std::round((want.X() - got.X()) / g->UPeriod()) * g->UPeriod() : 0;
          const double   dv   = g->IsVPeriodic() ? std::round((want.Y() - got.Y()) / g->VPeriod()) * g->VPeriod() : 0;
          if (du != 0 || dv != 0)
            pc[k]->Translate(gp_Vec2d(du, dv));
          one = pc[k]->Value(a).Distance(want) < 1e-6 && g->Value(pc[k]->Value(0.5 * (a + b)).X(), pc[k]->Value(0.5 * (a + b)).Y()).Distance(cc->Value(0.5 * (a + b))) <= tol;
        }
        if (one)
        {
          curve = cc;
          on[0] = pc[0];
          on[1] = pc[1];
          p0    = a;
          p1    = b;
        }
      }
    }
    TopoDS_Edge E;
    B.MakeEdge(E, curve, tol);
    B.Add(E, vs.Oriented(TopAbs_FORWARD));
    B.Add(E, vl.Oriented(TopAbs_REVERSED));
    B.Range(E, p0, p1);
    for (int k = 0; k < 2; ++k)
    {
      B.UpdateEdge(E, on[k], F[k], tol);
      B.Range(E, F[k], p0, p1);
    }
    if (!ok)
      continue;
    B.SameRange(E, true);
    B.SameParameter(E, true);
    rs->Replace(chain.front().Oriented(TopAbs_FORWARD), chain.front().Orientation() == TopAbs_REVERSED ? TopoDS::Edge(E.Reversed()) : E);
    for (size_t k = 1; k < chain.size(); ++k)
      rs->Remove(chain[k].Oriented(TopAbs_FORWARD));
    made.emplace_back(chain, E);
  }
  if (made.empty())
    return s;
  const TopoDS_Shape out = rs->Apply(s);
  std::vector<TopoDS_Shape> joined;
  for (const auto& m : made)
    joined.push_back(m.second);
  // (A joint's vertex is the two pieces' alone: only the faces along the joined edges change.)
  if (out.IsNull() || !valid_around(out, joined))
    return s;
  if (hist.IsNull())
    hist = new BRepTools_History();
  HHist h = rs->History();
  if (h.IsNull())
    h = new BRepTools_History();
  for (const auto& [chain, E] : made)
    for (size_t k = 1; k < chain.size(); ++k)
      h->AddModified(chain[k].Oriented(TopAbs_FORWARD), E);
  hist->Merge(h);
  return out;
}

// `out`'s solids in the order of the input's they came from (by their faces, through `hist`): a unify mustn't reorder
// a Boolean's pieces, later features name them by place (qa MIRROR-PIECE-ORDER-CHANGE: Mirror 3's five pieces another
// way round once a unify that merged only edges was kept). `out` when it has fewer than two solids or keeps the order.
TopoDS_Shape keep_solid_order(const TopoDS_Shape& in, const TopoDS_Shape& out, const HHist& hist)
{
  std::vector<TopoDS_Shape> solids;
  for (TopExp_Explorer x(out, TopAbs_SOLID); x.More(); x.Next())
    solids.push_back(x.Current());
  if (solids.size() < 2 || out.ShapeType() != TopAbs_COMPOUND)
    return out;
  // Each result face: the first input solid one of its sources is in.
  NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> rank;
  int                                                            k = 0;
  for (TopExp_Explorer x(in, TopAbs_SOLID); x.More(); x.Next(), ++k)
    for (TopExp_Explorer f(x.Current(), TopAbs_FACE); f.More(); f.Next())
    {
      ShapeList imgs;
      if (!hist.IsNull() && hist->IsRemoved(f.Current()))
        continue;
      if (!hist.IsNull())
        imgs = hist->Modified(f.Current());
      if (imgs.IsEmpty())
        imgs.Append(f.Current());
      for (ShapeList::Iterator it(imgs); it.More(); it.Next())
        if (!rank.IsBound(it.Value()))
          rank.Bind(it.Value(), k);
    }
  std::vector<std::pair<int, size_t>> order;
  for (size_t i = 0; i < solids.size(); ++i)
  {
    int r = std::numeric_limits<int>::max();
    for (TopExp_Explorer f(solids[i], TopAbs_FACE); f.More(); f.Next())
      if (const int* x = rank.Seek(f.Current()))
        r = std::min(r, *x);
    order.emplace_back(r, i);
  }
  std::stable_sort(order.begin(), order.end());
  bool same = true;
  for (size_t i = 0; i < order.size(); ++i)
    same = same && order[i].second == i;
  if (same)
    return out;
  // (Only the solids' order: anything else in the compound after them.)
  TopoDS_Compound c;
  BRep_Builder    B;
  B.MakeCompound(c);
  for (const auto& [r, i] : order)
    B.Add(c, solids[i]);
  for (TopoDS_Iterator it(out); it.More(); it.Next())
    if (it.Value().ShapeType() != TopAbs_SOLID && it.Value().ShapeType() != TopAbs_COMPSOLID)
      B.Add(c, it.Value());
  return c;
}

namespace {
// Two faces still meeting across an edge (not a seam) on one cylinder: USD's result depends on the faces' order.
bool cylinder_faces_apart(const TopoDS_Shape& s)
{
  ShapeIMap faces;
  TopExp::MapShapes(s, TopAbs_FACE, faces);
  std::vector<std::optional<gp_Cylinder>> cyl(faces.Extent() + 1);
  bool                                    any = false;
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    BRepAdaptor_Surface a(TopoDS::Face(faces(i)), false);
    if (a.GetType() == GeomAbs_Cylinder)
    {
      cyl[i] = a.Cylinder();
      any    = true;
    }
  }
  if (!any)
    return false;
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const auto& fs = ef(i);
    if (fs.Extent() != 2)
      continue;
    const std::optional<gp_Cylinder>&ca = cyl[faces.FindIndex(fs.First())], &cb = cyl[faces.FindIndex(fs.Last())];
    if (!ca || !cb)
      continue;
    const gp_Lin la(ca->Axis()), lb(cb->Axis());
    const double tol = 1e-9 * std::max(1.0, ca->Radius());
    if (std::abs(ca->Radius() - cb->Radius()) <= tol && la.Direction().IsParallel(lb.Direction(), 1e-9) && la.Distance(lb.Location()) <= tol)
      return true;
  }
  return false;
}

// The shape with each shell's faces in another order (`k` 1: reversed; 2, 3: the reversed and the plain order
// turned by half), the same faces; null where it isn't solids.
TopoDS_Shape faces_reordered(const TopoDS_Shape& s0, int k)
{
  BRep_Builder bb;
  auto         solid = [&](const TopoDS_Solid& so) {
    TopoDS_Solid out;
    bb.MakeSolid(out);
    for (TopoDS_Iterator it(so, false, false); it.More(); it.Next())
    {
      if (it.Value().ShapeType() != TopAbs_SHELL)
        return TopoDS_Solid();
      std::vector<TopoDS_Shape> fs;
      for (TopoDS_Iterator f(it.Value(), false, false); f.More(); f.Next())
        fs.push_back(f.Value());
      if (k == 1 || k == 2)
        std::reverse(fs.begin(), fs.end());
      if (k >= 2)
        std::rotate(fs.begin(), fs.begin() + fs.size() / 2, fs.end());
      TopoDS_Shell sh;
      bb.MakeShell(sh);
      for (const TopoDS_Shape& f : fs)
        bb.Add(sh, f);
      sh.Closed(it.Value().Closed());
      sh.Orientation(it.Value().Orientation());
      bb.Add(out, sh);
    }
    out.Orientation(so.Orientation());
    return out;
  };
  if (s0.ShapeType() == TopAbs_SOLID)
    return solid(TopoDS::Solid(s0));
  if (s0.ShapeType() != TopAbs_COMPOUND)
    return TopoDS_Shape();
  TopoDS_Compound c;
  bb.MakeCompound(c);
  for (TopoDS_Iterator it(s0, false, false); it.More(); it.Next())
  {
    if (it.Value().ShapeType() != TopAbs_SOLID)
      return TopoDS_Shape();
    const TopoDS_Solid so = solid(TopoDS::Solid(it.Value()));
    if (so.IsNull())
      return TopoDS_Shape();
    bb.Add(c, so);
  }
  return c;
}

int face_count(const TopoDS_Shape& s)
{
  ShapeIMap m;
  TopExp::MapShapes(s, TopAbs_FACE, m);
  return m.Extent();
}
} // namespace

TopoDS_Shape unify_in(const TopoDS_Shape& s0, HHist& hist, const UnifyScope* sc, bool* clean)
{
  if (legacy_topology())
    return unify_faces_in(s0, hist, sc, clean);
  // (Faces on one cylinder USD left apart: again with the faces in other orders (a part mirrored over a hole on the
  // mirror plane: its wall left in 4 pieces in 20 of the 24 orders of the half's faces, RUST-MIRROR-SPLIT-FACES), the
  // fewest faces kept where valid and of the same volume; each try with the history as it came.)
  auto copy = [](const HHist& h) {
    HHist c;
    if (!h.IsNull())
    {
      c = new BRepTools_History();
      c->Merge(h);
    }
    return c;
  };
  const bool   may    = cylinder_faces_apart(s0);
  const HHist  before = may ? copy(hist) : HHist();
  TopoDS_Shape r      = unify_seams_in(s0, hist, sc, clean);
  if (may && !r.IsSame(s0) && cylinder_faces_apart(r))
  {
    const KernelTimer timer("unify.reordered");
    GProp_GProps      v0;
    BRepGProp::VolumeProperties(r, v0);
    for (int k = 1; k <= 3 && cylinder_faces_apart(r); ++k)
    {
      const TopoDS_Shape sk = faces_reordered(s0, k);
      if (sk.IsNull())
        break;
      HHist              hk = copy(before);
      bool               ck = false;
      const TopoDS_Shape rk = unify_seams_in(sk, hk, sc, clean != nullptr ? &ck : nullptr);
      GProp_GProps       vk;
      BRepGProp::VolumeProperties(rk, vk);
      if (face_count(rk) < face_count(r) && std::abs(vk.Mass() - v0.Mass()) <= 1e-9 * std::abs(v0.Mass()) && BRepCheck_Analyzer(rk).IsValid())
      {
        r    = rk;
        hist = hk;
        if (clean != nullptr)
          *clean = ck;
      }
    }
  }
  return r.IsSame(s0) ? r : keep_solid_order(s0, r, hist);
}

TopoDS_Shape unify_seams_in(const TopoDS_Shape& s0, HHist& hist, const UnifyScope* sc, bool* clean)
{
  const TopoDS_Shape s = unify_faces_in(s0, hist, sc, clean);
  if (!TopExp_Explorer(s, TopAbs_SOLID).More())
    return s;
  TopoDS_Shape moved;
  {
    const KernelTimer timer("unify.seams");
    moved = move_seams_off_holes(s, hist);
  }
  // (Also where an earlier unify moved the seam: the pieces a later cut left.)
  const TopoDS_Shape joined = join_loop_pieces(moved, hist);
  if (clean != nullptr && !joined.IsSame(s))
    *clean = false;
  return joined;
}

TopoDS_Shape unify_faces_in(const TopoDS_Shape& s0, HHist& hist, const UnifyScope* sc, bool* clean)
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
  fail_if_cancelled();
  lap = std::make_unique<KernelTimer>("unify.merge_planar");
  UnifyScope         next;
  const TopoDS_Shape s = merge_planar(s0, hist, sc, &next);
  fail_if_cancelled();
  lap                  = std::make_unique<KernelTimer>("unify.candidates_again");
  if (!same_domain_candidates(s, sc != nullptr ? &next : nullptr))
  {
    if (sc != nullptr)
      check_scoped_unify(s0, s);
    if (clean != nullptr)
      *clean = true;
    return s;
  }
  fail_if_cancelled();
  lap = std::make_unique<KernelTimer>("unify.bands");
  // (A failure inside it (OCCT's projection): the shape as it was; its history is merged only at its end.)
  TopoDS_Shape groups = s;
  try
  {
    groups = merge_cylinder_groups(s, hist, false);
  }
  catch (const Standard_Failure&)
  {
    groups = s;
  }
  const TopoDS_Shape s0b = merge_apex_cones(merge_cylinder_bands(groups, hist), hist);
  // The rest, on `s1` with its own history `hist`.
  auto rest = [&](const TopoDS_Shape& s1, HHist& hist) -> TopoDS_Shape {
    fail_if_cancelled();
    lap                   = std::make_unique<KernelTimer>("unify.usd");
    if (TopExp_Explorer(s1, TopAbs_SOLID).More() && round_candidates(s1, sc != nullptr ? &next : nullptr))
    {
      const TopoDS_Shape s2 = usd_kept_volume(s1, hist);
      fail_if_cancelled();
      lap                   = std::make_unique<KernelTimer>("unify.patches");
      const TopoDS_Shape s3 = merge_cylinder_patches(s2, hist);
      // Faces on one cylinder USD left apart (one frame, its result depending on the faces' order and wires' starts:
      // a Shell fuse's walls, 1b121fc3, a face more on ours than on OCCT's), merged as those whose frames differ.
      fail_if_cancelled();
      // (Its own history, merged only where its result is kept: valid, as USD's edges after it can leave it invalid
      // (a Shell's common with hyperbola edges, 14218a58: OCCT's result 42 faces valid, ours then 41 invalid).)
      TopoDS_Shape s4 = s3;
      HHist        h4;
      try
      {
        s4 = merge_cylinder_groups(s3, h4, true);
      }
      catch (const Standard_Failure&)
      {
        return s3;
      }
      if (s4.IsSame(s3))
        return s3;
      // (Their boundaries' pieces now between one face on each side: joined as USD joins edges.)
      ShapeUpgrade_UnifySameDomain u(s4, true, false, false);
      u.Build();
      fail_if_cancelled();
      if (!valid_whole(u.Shape()))
        return s3;
      if (hist.IsNull())
        hist = new BRepTools_History();
      if (!h4.IsNull())
        hist->Merge(h4);
      if (!u.History().IsNull())
        hist->Merge(u.History());
      return u.Shape();
    }
    fail_if_cancelled();
    ShapeUpgrade_UnifySameDomain u(s1, true, true, false);
    u.Build();
    fail_if_cancelled();
    if (!u.History().IsNull())
    {
      if (hist.IsNull())
        hist = new BRepTools_History();
      hist->Merge(u.History());
    }
    return u.Shape();
  };
  // Equal tori and spheres on one surface first, kept where the result is valid (a Shell's common, 14218a58: USD's
  // merge then left it invalid).
  HHist              hs;
  const TopoDS_Shape shared = share_equal_surfaces(s0b, hs);
  if (!shared.IsSame(s0b))
  {
    HHist              ht;
    const TopoDS_Shape r = rest(shared, ht);
    if (BRepCheck_Analyzer(r).IsValid())
    {
      if (hist.IsNull())
        hist = new BRepTools_History();
      hist->Merge(hs);
      if (!ht.IsNull())
        hist->Merge(ht);
      return r;
    }
  }
  return rest(s0b, hist);
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
// The cut of `tools` lying inside `args` and apart: the result's volume is theirs less the tools' (to a thousandth of
// the smallest tool: the integration's noise with B-spline faces is ~1e-5 of the part).
bool cut_volumes_add_up(const std::vector<TopoDS_Shape>& args, const std::vector<TopoDS_Shape>& tools, const TopoDS_Shape& result)
{
  double va = 0, vt = 0, smallest = std::numeric_limits<double>::infinity();
  for (const TopoDS_Shape& x : args)
    va += volume_of(x);
  for (const TopoDS_Shape& x : tools)
  {
    const double v = volume_of(x);
    vt += v;
    smallest = std::min(smallest, v);
  }
  return !tools.empty() && smallest > 0 && std::abs(volume_of(result) - (va - vt)) <= 1e-3 * smallest;
}

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
  // (Each failed cut's reason: a thread's own, said again on the caller's.)
  std::vector<std::string> whys(n);
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
      {
        failed[static_cast<size_t>(i)] = 1;
        whys[static_cast<size_t>(i)]   = rust_last_decline();
      }
    }
    catch (...)
    {
      failed[static_cast<size_t>(i)] = 1;
      whys[static_cast<size_t>(i)]   = "a cut failed";
    }
  });
  fail_if_cancelled();
  for (size_t i = 0; i < n; ++i)
    if (failed[i])
    {
      rust_decline(whys[i].c_str());
      return false;
    }
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

// The union of `all` two at a time, a balanced tree of OCCT fuses, and its history from `all`. False when one fails.
bool fuse_tree(const std::vector<TopoDS_Shape>& all, const BoolOpts& opts, TopoDS_Shape& out, HHist& hist)
{
  std::vector<TopoDS_Shape>       level = all;
  std::vector<std::vector<Stage>> stages;
  while (level.size() > 1)
  {
    std::vector<TopoDS_Shape> next;
    std::vector<Stage>        st;
    for (size_t i = 0; i + 1 < level.size(); i += 2)
    {
      const std::vector<TopoDS_Shape> pair{level[i], level[i + 1]};
      BRepAlgoAPI_Fuse                f;
      f.SetArguments(to_list(std::vector<TopoDS_Shape>{level[i]}));
      f.SetTools(to_list(std::vector<TopoDS_Shape>{level[i + 1]}));
      apply_opts(f, opts);
      build_cancellable(f);
      if (cancelled(t_cancel) || f.HasErrors() || !f.IsDone())
        return false;
      st.push_back(stage_of(pair, new BRepTools_History(to_list(pair), f)));
      next.push_back(f.Shape());
    }
    if (level.size() % 2 == 1)
      next.push_back(level.back());
    stages.push_back(std::move(st));
    level = std::move(next);
  }
  out  = level[0];
  hist = compose(all, stages);
  return true;
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
  B.Add(so, ns); // (the faces as the shell's iterator gives them: already in the solid's orientation)
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
  B.Add(so, ns); // (the faces as the shell's iterator gives them: already in the solid's orientation)
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
  B.Add(so, ns); // (the faces as the shell's iterator gives them: already in the solid's orientation)
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
  // Boxes first: two tools whose boxes, widened by what the mesh and the check reach, don't meet can't overlap there,
  // so only tools with such a neighbour are meshed (the meshing was most of the time: 48 % of the Y Ribs feature's
  // warm regeneration, OCCT's Delaunay on its tori).
  std::vector<Bnd_Box> boxes((size_t)n);
  for (int i = 0; i < n; ++i)
  {
    BRepBndLib::Add(t[(size_t)i], boxes[(size_t)i], false);
    boxes[(size_t)i].Enlarge(3 * defl);
  }
  std::vector<char> close((size_t)n, 0);
  for (int i = 0; i < n; ++i)
    for (int j = i + 1; j < n; ++j)
      if (!boxes[(size_t)i].IsOut(boxes[(size_t)j]))
        close[(size_t)i] = close[(size_t)j] = 1;
  TopoDS_Compound copies;
  BRep_Builder    bb;
  bb.MakeCompound(copies);
  NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> owner;
  for (int i = 0; i < n; ++i)
  {
    if (!close[(size_t)i])
      continue;
    const TopoDS_Shape c = BRepBuilderAPI_Copy(t[(size_t)i], true, false).Shape();
    BRepMesh_IncrementalMesh(c, defl, false, 0.5);
    for (TopExp_Explorer f(c, TopAbs_FACE); f.More(); f.Next())
      owner.Bind(f.Current(), i);
    bb.Add(copies, c);
  }
  std::vector<std::vector<char>> conflict((size_t)n, std::vector<char>((size_t)n, 0));
  // (None meshed: no two can meet, one group.)
  if (!owner.IsEmpty())
  {
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
    fail_if_cancelled();
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

// An edge of `s` bounding more than two faces.
bool non_manifold(const TopoDS_Shape& s)
{
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  for (int i = 1; i <= ef.Extent(); ++i)
    if (ef(i).Extent() > 2)
      return true;
  return false;
}

// Whether `r` is what the Boolean gives for solids that don't overlap: a
// union of their volume together, a cut of the target's, an empty common.
bool no_interaction(uint8_t op, const std::vector<TopoDS_Shape>& a, const std::vector<TopoDS_Shape>& t, const TopoDS_Shape& r)
{
  // (Volumes of big parts take seconds: a cancel is looked at between them.)
  double va = 0, vt = 0;
  for (const TopoDS_Shape& x : a)
  {
    fail_if_cancelled();
    va += volume_of(x);
  }
  for (const TopoDS_Shape& x : t)
  {
    fail_if_cancelled();
    vt += volume_of(x);
  }
  fail_if_cancelled();
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
      // (Each face classifies against whole solids: seconds on a knurl's 10k faces, so a cancel looks here.)
      fail_if_cancelled();
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
size_t process_resident_bytes() { return resident_bytes(); }

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
  // (Not every Boolean: scaling all those of 9+ tools made the corpus 25 %
  // slower and changed 7 of 974; the cases that gain are hard ones.)
  if (op == 1 && n_tools >= 64 && opts.glue == 0 && facets.source.empty() && working_scale() != 1.0 && std::getenv("NOSHAPE_BOOL_NO_SCALE") == nullptr)
  {
    scaled = std::make_unique<ScaledInputs>(working_scale());
    for (TopoDS_Shape& x : a)
      x = scaled->scale(x);
    for (TopoDS_Shape& x : t)
      x = scaled->scale(x);
  }
  // (The tree below fuses the tools one by one: OCCT's fuses of the groups two at a time lost a tool's volume (Shell's
  // 47 slabs, bf976715: 5.755e-5 m³ against 6.154e-5); the groups speed up one Boolean of them all.)
  const std::vector<TopoDS_Shape> each = t;
  if (opts.glue != 3 && facets.source.empty())
    t = group_disjoint(t);
  b->SetArguments(to_list(a));
  b->SetTools(to_list(t));
  BoolOpts first = opts;
  if (first.glue == 3 && op != 0) // glue is for fusing touching shapes only
    first.glue = 0;
  if (first.glue == 4) // (a cut by tools inside the part: checked below)
    first.glue = op == 1 ? 1 : 0;
  // Many tools (a knurl's 148 crossing grooves): OCCT's intersection of the
  // pairs runs in parallel.
  first.parallel = first.parallel || n_tools >= 8;
  apply_opts(*b, first);
  // Many tools crossing each other (a diamond knurl's two families of
  // grooves: the groups above touch): the cut is built from the faces'
  // classification. OCCT's split solids of the target and of every tool
  // (each cut up by the crossing ones) took minutes and gigabytes, and a cut
  // keeps none of the tools'.
  // Big free-form tools overlapping each other fused (qa MIRROR-HANG-402: a Part mirror's Add with Merge with all
  // over 11 overlapping copies of a 38-face free-form part): OCCT's one fuse of them intersects every pair at once
  // (220 s, then BOPAlgo_AlertBuilderFailed); two at a time in a balanced tree gave live's solid in 33 s (a loaded
  // box). Small ones stay one fuse (the tree was up to 7 times slower on the corpus's 3-10 tool fuses).
  TopoDS_Shape tree;
  HHist        tree_hist;
  bool         by_tree = false;
  auto heavy = [&] {
    int  faces = 0;
    bool free  = false;
    for (const TopoDS_Shape& x : concat(a, t))
      for (TopExp_Explorer f(x, TopAbs_FACE); f.More(); f.Next(), ++faces)
        if (!free)
          switch (BRepAdaptor_Surface(TopoDS::Face(f.Current()), false).GetType())
          {
            case GeomAbs_Plane:
            case GeomAbs_Cylinder:
            case GeomAbs_Cone:
            case GeomAbs_Sphere:
            case GeomAbs_Torus:
              break;
            default:
              free = true;
          }
    return free && faces >= 100;
  };
  if (op == 0 && first.glue == 0 && facets.source.empty() && t.size() >= 3 && heavy() && std::getenv("NOSHAPE_BOOL_NO_TREE") == nullptr)
  {
    // At the working scale (in metres OCCT's fuses of those parts failed).
    std::vector<TopoDS_Shape>     all = concat(a, each);
    std::unique_ptr<ScaledInputs> sc;
    if (!scaled && working_scale() != 1.0 && std::getenv("NOSHAPE_BOOL_NO_SCALE") == nullptr)
    {
      sc = std::make_unique<ScaledInputs>(working_scale());
      for (TopoDS_Shape& x : all)
        x = sc->scale(x);
    }
    {
      const KernelTimer timer("boolean.tree");
      by_tree = fuse_tree(all, first, tree, tree_hist);
    }
    fail_if_cancelled();
    if (by_tree)
    {
      t_bool_path = "occt_tree";
      if (sc)
        scaled = std::move(sc);
    }
  }
  bool by_faces = op == 1 && n_tools >= 8 && t.size() >= 2 && t.size() < n_tools;
  if (!by_tree)
  {
    const KernelTimer timer("boolean.build");
    const CutByFaces scope(by_faces);
    build_cancellable(*b);
  }
  if (cancelled(t_cancel))
    free_elsewhere(b);
  fail_if_cancelled();
  if (!by_tree && by_faces && (b->HasErrors() || !b->IsDone()))
  {
    by_faces = false;
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
  HHist        hist = tree_hist;
  TopoDS_Shape res  = tree;
  const FreeIfCancelled<HHist, TopoDS_Shape> held(hist, res);
  if (!by_tree && first.glue == 3 && (b->HasErrors() || !b->IsDone() || !volumes_add_up(concat(a, t), b->Shape())))
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
  // A checked glue cut: kept when the part lost each tool's whole volume (the tools inside it and apart; a glued cut
  // gone wrong leaves some of a tool), else again without glue. (Tools reaching out of the part fail it: slower, right.)
  if (!by_tree && opts.glue == 4 && op == 1 && (b->HasErrors() || !b->IsDone() || !cut_volumes_add_up(a, each, b->Shape())))
  {
    fail_if_cancelled();
    BoolOpts plain = opts;
    plain.glue     = 0;
    plain.parallel = first.parallel;
    b              = make();
    b->SetArguments(to_list(a));
    b->SetTools(to_list(t));
    apply_opts(*b, plain);
    build_cancellable(*b);
  }
  fail_if_cancelled();
  if (!by_tree && (b->HasErrors() || !b->IsDone()))
    fail_op("boolean failed: " + algo_errors(*b));
  if (!by_tree)
  {
    const KernelTimer timer("boolean.history");
    hist = cancellable_history(concat(a, t), *b);
    // Built from the faces (patch 0001's CutByFaces), the result's solids are none of the target's images, and
    // the history says the target was deleted; the usual build (and noshape's Boolean) says Modified, which the
    // Rust Boolean's replay and the naming go by: each target solid Modified to the result's solids holding
    // its faces or their pieces.
    if (by_faces)
      for (const TopoDS_Shape& arg : a)
        for (TopExp_Explorer sx(arg, TopAbs_SOLID); sx.More(); sx.Next())
        {
          const TopoDS_Shape& S = sx.Current();
          if (!hist->IsRemoved(S))
            continue;
          ShapeIMap own;
          for (TopExp_Explorer fx(S, TopAbs_FACE); fx.More(); fx.Next())
          {
            own.Add(fx.Current());
            for (const TopoDS_Shape& m : hist->Modified(fx.Current()))
              own.Add(m);
          }
          for (TopExp_Explorer rx(b->Shape(), TopAbs_SOLID); rx.More(); rx.Next())
          {
            bool holds = false;
            for (TopExp_Explorer fx(rx.Current(), TopAbs_FACE); fx.More() && !holds; fx.Next())
              holds = own.Contains(fx.Current());
            if (holds)
              hist->AddModified(S, rx.Current());
          }
        }
  }
  if (!by_tree)
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
  if (!by_tree && n_tools <= 8 && first.glue == 0 && facets.source.empty() && (located(args.items) || located(tools.items)) && all_solids(inputs)
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
        hist        = cancellable_history(inputs, *b2);
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
      hist        = cancellable_history(inputs, *b2);
      res         = b2->Shape();
      break;
    }
  }
  // A cut by several tools leaving an edge on more than two faces where no input has one (cf-straight-spline: a slot's
  // flat side and its run-out's cone touch along the profile edge, and OCCT keeps the cone's half inside the slot;
  // live's part is manifold): again with the tools fused first, kept when that is manifold.
  if (op == 1 && n_tools > 1 && facets.source.empty() && all_solids(inputs) && non_manifold(res)
      && std::none_of(inputs.begin(), inputs.end(), [](const TopoDS_Shape& x) { return non_manifold(x); }))
  {
    const KernelTimer               timer("boolean.fused_tools");
    const std::vector<TopoDS_Shape> a2(inputs.begin(), inputs.begin() + static_cast<long>(inputs.size() - n_tools));
    const std::vector<TopoDS_Shape> t2(inputs.begin() + static_cast<long>(inputs.size() - n_tools), inputs.end());
    BoolOpts                        plain = first;
    plain.glue                            = 0;
    BRepAlgoAPI_Fuse                fuse;
    fuse.SetArguments(to_list({t2[0]}));
    fuse.SetTools(to_list(std::vector<TopoDS_Shape>(t2.begin() + 1, t2.end())));
    apply_opts(fuse, plain);
    build_cancellable(fuse);
    fail_if_cancelled();
    if (!fuse.HasErrors() && fuse.IsDone())
    {
      BRepAlgoAPI_Cut cut;
      cut.SetArguments(to_list(a2));
      cut.SetTools(to_list({fuse.Shape()}));
      apply_opts(cut, plain);
      build_cancellable(cut);
      fail_if_cancelled();
      if (!cut.HasErrors() && cut.IsDone() && !non_manifold(cut.Shape()) && volume_possible(op, a2, t2, cut.Shape()))
      {
        t_bool_path = "occt_fused_tools";
        HHist h     = cancellable_history(t2, fuse);
        h->Merge(cancellable_history(concat(a2, {fuse.Shape()}), cut));
        hist = h;
        res  = cut.Shape();
      }
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
  build_cancellable(sp);
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

// The kernel's unify of a whole shape (op_unify's), for the kernel's own operations.
TopoDS_Shape rim_chain_seams_joined(const TopoDS_Shape& s, HHist& hist)
{
  const TopoDS_Shape moved = rim_chain_seams(s, hist);
  return moved.IsSame(s) ? s : join_loop_pieces(moved, hist);
}

TopoDS_Shape unify_shape(const TopoDS_Shape& s, HHist& hist)
{
  return unify(s, hist);
}

// Only the pieces of loops joined (join_loop_pieces: where a seam or a blend left a vertex between two pieces of one
// curve), nothing unified; none in a studio saved before (legacy topology).
OpOut op_join_edges(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  HHist              hist;
  const TopoDS_Shape res = legacy_topology() ? s.shape : join_loop_pieces(s.shape, hist);
  return make_result({s.shape}, res, hist.get());
}

OpOut op_unify_edges(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  ShapeUpgrade_UnifySameDomain u(s.shape, true, false, false);
  u.Build();
  fail_if_cancelled();
  if (!BRepCheck_Analyzer(u.Shape()).IsValid())
    return make_result({s.shape}, s.shape, nullptr);
  HHist hist = new BRepTools_History();
  if (!u.History().IsNull())
    hist->Merge(u.History());
  return make_result({s.shape}, u.Shape(), hist.get());
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

namespace
{
// An affine map of a body bounded by planes and straight edges: planes stay planes and lines lines (Onshape's;
// BRepBuilderAPI_GTransform converts everything to B-splines first).
class AffinePlanar : public BRepTools_Modification
{
public:
  explicit AffinePlanar(const gp_GTrsf& g)
      : myG(g)
  {
    const gp_Mat& a = g.VectorialPart();
    for (int c = 1; c <= 3; ++c)
      myScale = std::max(myScale, gp_XYZ(a(1, c), a(2, c), a(3, c)).Modulus());
  }

  static bool applies(const TopoDS_Shape& s, const gp_GTrsf& g)
  {
    if (g.VectorialPart().Determinant() <= 0.0)
      return false;
    for (TopExp_Explorer x(s, TopAbs_FACE); x.More(); x.Next())
      if (plane_of(TopoDS::Face(x.Current())).IsNull())
        return false;
    for (TopExp_Explorer x(s, TopAbs_EDGE); x.More(); x.Next())
      if (line_of(TopoDS::Edge(x.Current())).IsNull())
        return false;
    return true;
  }

  bool NewSurface(const TopoDS_Face& F, occ::handle<Geom_Surface>& S, TopLoc_Location& L, double& Tol, bool& RevWires, bool& RevFace) override
  {
    S        = new Geom_Plane(new_plane(F));
    L        = TopLoc_Location();
    Tol      = BRep_Tool::Tolerance(F) * myScale;
    RevWires = false;
    RevFace  = false;
    return true;
  }

  bool NewCurve(const TopoDS_Edge& E, occ::handle<Geom_Curve>& C, TopLoc_Location& L, double& Tol) override
  {
    const gp_Lin l = new_line(E);
    C              = new Geom_Line(l);
    L              = TopLoc_Location();
    Tol            = BRep_Tool::Tolerance(E) * myScale;
    return true;
  }

  bool NewPoint(const TopoDS_Vertex& V, gp_Pnt& P, double& Tol) override
  {
    gp_XYZ p = BRep_Tool::Pnt(V).XYZ();
    myG.Transforms(p);
    P   = gp_Pnt(p);
    Tol = BRep_Tool::Tolerance(V) * myScale;
    return true;
  }

  bool NewCurve2d(const TopoDS_Edge& E, const TopoDS_Face& F, const TopoDS_Edge&, const TopoDS_Face&, occ::handle<Geom2d_Curve>& C, double& Tol) override
  {
    const gp_Ax3 a = new_plane(F);
    const gp_Lin l = new_line(E);
    double       u, v;
    ElSLib::Parameters(gp_Pln(a), l.Location(), u, v);
    const gp_XYZ d = l.Direction().XYZ();
    C              = new Geom2d_Line(gp_Pnt2d(u, v), gp_Dir2d(d.Dot(a.XDirection().XYZ()), d.Dot(a.YDirection().XYZ())));
    Tol            = BRep_Tool::Tolerance(E) * myScale;
    return true;
  }

  bool NewParameter(const TopoDS_Vertex& V, const TopoDS_Edge& E, double& P, double& Tol) override
  {
    P   = BRep_Tool::Parameter(V, E) * stretch(E);
    Tol = BRep_Tool::Tolerance(V) * myScale;
    return true;
  }

  GeomAbs_Shape Continuity(const TopoDS_Edge& E, const TopoDS_Face& F1, const TopoDS_Face& F2, const TopoDS_Edge&, const TopoDS_Face&, const TopoDS_Face&) override
  {
    return BRep_Tool::Continuity(E, F1, F2);
  }

private:
  static occ::handle<Geom_Plane> plane_of(const TopoDS_Face& f)
  {
    TopLoc_Location           l;
    occ::handle<Geom_Surface> s = BRep_Tool::Surface(f, l);
    if (auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
      s = t->BasisSurface();
    return occ::down_cast<Geom_Plane>(s);
  }

  static occ::handle<Geom_Line> line_of(const TopoDS_Edge& e)
  {
    TopLoc_Location         l;
    double                  f, g;
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, l, f, g);
    if (auto t = occ::down_cast<Geom_TrimmedCurve>(c))
      c = t->BasisCurve();
    return occ::down_cast<Geom_Line>(c);
  }

  gp_XYZ map_vector(const gp_XYZ& d) const
  {
    gp_XYZ v = d;
    v.Multiply(myG.VectorialPart());
    return v;
  }

  gp_XYZ map_point(const gp_XYZ& p) const
  {
    gp_XYZ q = p;
    myG.Transforms(q);
    return q;
  }

  // The face's plane mapped: its origin and axes the images of the original's (the same handedness).
  gp_Ax3 new_plane(const TopoDS_Face& f) const
  {
    TopLoc_Location l;
    BRep_Tool::Surface(f, l);
    const gp_Ax3 a = plane_of(f)->Position().Transformed(l.Transformation());
    const gp_XYZ x = map_vector(a.XDirection().XYZ()), y = map_vector(a.YDirection().XYZ());
    gp_Ax3       n(gp_Pnt(map_point(a.Location().XYZ())), gp_Dir(x.Crossed(y)), gp_Dir(x));
    if (!a.Direct())
      n.ZReverse();
    return n;
  }

  // The edge's line mapped, its parameter t at the image of the original's point at t / stretch.
  gp_Lin new_line(const TopoDS_Edge& e) const
  {
    TopLoc_Location l;
    double          f, g;
    BRep_Tool::Curve(e, l, f, g);
    const gp_Lin a = line_of(e)->Lin().Transformed(l.Transformation());
    return gp_Lin(gp_Pnt(map_point(a.Location().XYZ())), gp_Dir(map_vector(a.Direction().XYZ())));
  }

  double stretch(const TopoDS_Edge& e) const
  {
    TopLoc_Location l;
    double          f, g;
    BRep_Tool::Curve(e, l, f, g);
    return map_vector(line_of(e)->Lin().Transformed(l.Transformation()).Direction().XYZ()).Modulus();
  }

  gp_GTrsf myG;
  double   myScale = 0.0;
};
} // namespace

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
  if (AffinePlanar::applies(s.shape, g))
  {
    BRepTools_Modifier md(s.shape, new AffinePlanar(g));
    if (md.IsDone())
    {
      HHist hist = new BRepTools_History();
      for (TopAbs_ShapeEnum k : {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
      {
        ShapeIMap m;
        TopExp::MapShapes(s.shape, k, m);
        for (int i = 1; i <= m.Extent(); ++i)
        {
          TopoDS_Shape r = md.ModifiedShape(m(i));
          if (!r.IsNull() && !r.IsSame(m(i)) && r.ShapeType() == k)
            hist->AddModified(m(i), r);
        }
      }
      return make_result({s.shape}, md.ModifiedShape(s.shape), hist.get());
    }
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
  build_cancellable(sec);
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
