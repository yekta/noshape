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
#include <BRepBuilderAPI_Sewing.hxx>
#include <unordered_map>
#include <BRepBuilderAPI_ModifyShape.hxx>
#include <BRepTools_TrsfModification.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2dConvert.hxx>
#include <GeomConvert.hxx>
#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
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
#include <BRepFeat_SplitShape.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <BRepTools_History.hxx>
#include <BRepTools_Modification.hxx>
#include <BRepTools_Modifier.hxx>
#include <Geom_SurfaceOfRevolution.hxx>
#include <ShapeCustom_SweptToElementary.hxx>
#include <GeomConvert_SurfToAnaSurf.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <ShapeFix_Face.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_BezierSurface.hxx>
#include <BRepTools.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepTools_ReShape.hxx>
#include <Geom_Circle.hxx>
#include <Geom_SurfaceOfLinearExtrusion.hxx>
#include <gp_Ax2d.hxx>
#include <gp_Trsf2d.hxx>
#include <gp_Vec2d.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2dAdaptor_Curve.hxx>
#include <math.hxx>
#include <math_Vector.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <ShapeFix_Edge.hxx>
#include <Geom_Ellipse.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <BRep_Builder.hxx>
#include <TColStd_PackedMapOfInteger.hxx>
#include <NCollection_DataMap.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <deque>
#include <array>
#include <mutex>
#include <condition_variable>
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
#include <numeric>
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
    if (work_step() || budget_spent() || cancelled(myFlag) || myOver)
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
// The deadline's clock where it is the launching thread's CPU time: that clock and the steady clock when the job
// started (`cpu0`, `wall0`), the job's time counted as the launcher's, which waits for it without using any CPU
// (SECTIONS-AHEAD-HANG: OCCT's sections ahead ran on, the deadline never reached).
struct PoolContext
{
  const std::atomic<bool>* cancel;
  Deadline*                deadline;
  std::atomic<bool>*       spent;
  double                   cpu0, wall0;
  const PoolContext*       clock;
};
thread_local PoolContext        t_pool_context{nullptr, nullptr, nullptr, 0, 0, nullptr};
thread_local const PoolContext* t_pool_clock = nullptr; // the job this thread is running a part of
} // namespace

extern "C" void* noshape_pool_capture()
{
  if (t_cancel == nullptr && t_deadline == nullptr && t_spent == nullptr)
    return nullptr;
  t_pool_context = {t_cancel, t_deadline, t_spent, t_deadline != nullptr ? deadline_now(*t_deadline) : 0, steady_seconds(), nullptr};
  return &t_pool_context;
}

extern "C" void* noshape_pool_enter(void* context)
{
  if (context == nullptr)
    return nullptr;
  PoolContext* previous = new PoolContext{t_cancel, t_deadline, t_spent, 0, 0, t_pool_clock};
  const PoolContext* c  = static_cast<const PoolContext*>(context);
  t_cancel              = c->cancel;
  t_deadline            = c->deadline;
  t_spent               = c->spent;
  t_pool_clock          = c;
  return previous;
}

extern "C" void noshape_pool_leave(void* previous)
{
  if (previous == nullptr)
    return;
  const PoolContext* p = static_cast<const PoolContext*>(previous);
  t_cancel             = p->cancel;
  t_deadline           = p->deadline;
  t_spent              = p->spent;
  t_pool_clock         = p->clock;
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
  if (work_step() || budget_spent())
    return 1;
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
  const PoolContext* const job = t_pool_clock;
  const double             at  = job != nullptr && job->deadline == d ? std::max(deadline_now(*d), job->cpu0 + (now - job->wall0)) : deadline_now(*d);
  if (at > d->at)
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
    // (Their curves as stored and their locations once: the located curve taken from BRep_Tool::Curve(e, f, l) moved
    // again by the location put an edge in a located shape off its line, c2-sm-louver's Extrude 3, whose sheet's edge
    // then stayed in three. Through trims too.)
    double                  f1, l1, f2, l2;
    TopLoc_Location         l1o, l2o;
    occ::handle<Geom_Curve> c1 = BRep_Tool::Curve(TopoDS::Edge(es.First()), l1o, f1, l1);
    occ::handle<Geom_Curve> c2 = BRep_Tool::Curve(TopoDS::Edge(es.Last()), l2o, f2, l2);
    if (c1.IsNull() || c2.IsNull())
      continue;
    while (const auto t = occ::down_cast<Geom_TrimmedCurve>(c1))
      c1 = t->BasisCurve();
    while (const auto t = occ::down_cast<Geom_TrimmedCurve>(c2))
      c2 = t->BasisCurve();
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
    if (fs.Extent() != 2 || kept_edge(ef.FindKey(i)))
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
      char            buf[256];
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
      // (Its surface by kind and the point at the middle of its parameters, not by handle: each try's merged faces get
      // surfaces of their own, so the same merge never matched (cf-surface-pattern's Surface Pattern 3: 13 merged faces,
      // three tries of each of two unifies, 175 ms a check).)
      const occ::handle<Geom_Surface> g = BRep_Tool::Surface(TopoDS::Face(f), l);
      double                          u0 = 0, u1 = 0, v0 = 0, v1 = 0;
      BRepTools::UVBounds(TopoDS::Face(f), u0, u1, v0, v1);
      const gp_Pnt mid = g.IsNull() ? gp_Pnt() : g->Value(0.5 * (u0 + u1), 0.5 * (v0 + v1)).Transformed(l.Transformation());
      const double qm  = std::max(1e-7 * diag, 1e-15);
      std::snprintf(buf, sizeof buf, "%s %d %d %d %lld %lld %lld %lld %lld %lld", g.IsNull() ? "-" : g->DynamicType()->Name(), (int)f.Orientation(), ne, nv, std::llround(sx / q), std::llround(sy / q),
                    std::llround(sz / q), std::llround(mid.X() / qm), std::llround(mid.Y() / qm), std::llround(mid.Z() / qm));
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
    carry_kept_marks(s, [&](const TopoDS_Shape& x) { return std::vector<TopoDS_Shape>{cp.ModifiedShape(x)}; });
    if (k >= 8)
      in = faces_reversed(in);
    if (k % 8 > 0)
    {
      const TopoDS_Shape before_rotation = in;
      in = rotate_round_wires(in, k % 8, hr);
      if (!in.IsNull() && !hr.IsNull())
        carry_kept_marks(before_rotation, [&](const TopoDS_Shape& x) {
          std::vector<TopoDS_Shape> out;
          for (const TopoDS_Shape& m : hr->Modified(x))
            out.push_back(m);
          return out;
        });
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
    keep_marked_edges(u, in);
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
    carry_kept_marks(in, u);
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
    carry_kept_marks(s, [&](const TopoDS_Shape& x) { return std::vector<TopoDS_Shape>{cp.ModifiedShape(x)}; });
    ShapeUpgrade_UnifySameDomain u(in, true, true, false);
    keep_marked_edges(u, in);
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
    carry_kept_marks(in, u);
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
        if (kept_edge(e))
          return false;
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
    if (ef(i).Extent() != 2 || kept_edge(ef.FindKey(i)))
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
    // b's far circle in a's frame: its parameter is a's u, its vertex on a's seam. A cap that isn't a plane (a round,
    // an imported stud's torus) keeps its own vertex on it too, its seam's end: the circle in two there.
    TopoDS_Face cap;
    if (const ShapeList* caps = ef.Seek(bb.end))
      for (ShapeList::Iterator it(*caps); it.More(); it.Next())
        if (!it.Value().IsSame(b) && BRepAdaptor_Surface(TopoDS::Face(it.Value()), false).GetType() != GeomAbs_Plane)
          cap = TopoDS::Face(it.Value());
    TopoDS_Vertex b_v0, b_v1;
    TopExp::Vertices(bb.end, b_v0, b_v1);
    double bu, bv;
    ElSLib::Parameters(ca, BRep_Tool::Pnt(b_v0), bu, bv);
    bu = u0 + std::fmod(std::fmod(bu - u0, 2 * M_PI) + 2 * M_PI, 2 * M_PI);
    const bool split = !cap.IsNull() && bu - u0 > 1e-7 && u0 + 2 * M_PI - bu > 1e-7;
    TopoDS_Vertex vb;
    if (!cap.IsNull() && !split)
      vb = b_v0;
    else
      B.MakeVertex(vb, ElSLib::Value(u0, v_bf, ca), tol);
    const gp_Dir             n = gp_Dir(gp_Vec(A.XDirection()).Crossed(gp_Vec(A.YDirection())));
    occ::handle<Geom_Circle> circ =
      new Geom_Circle(gp_Ax2(A.Location().Translated(gp_Vec(A.Direction()) * v_bf), n, A.XDirection()), r);
    // Its pieces in u order (one, or two split at b's old vertex).
    std::vector<TopoDS_Edge> b_far;
    std::vector<std::pair<double, double>> spans{{u0, u0 + 2 * M_PI}};
    if (split)
      spans = {{u0, bu}, {bu, u0 + 2 * M_PI}};
    for (const auto& [p0, p1] : spans)
    {
      TopoDS_Edge e;
      B.MakeEdge(e, circ, tol);
      B.Add(e, (p0 == u0 ? vb : b_v0).Oriented(TopAbs_FORWARD));
      B.Add(e, (p1 == u0 + 2 * M_PI ? vb : b_v0).Oriented(TopAbs_REVERSED));
      B.Range(e, p0, p1);
      b_far.push_back(e);
    }
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
    for (size_t k = 0; k < b_far.size(); ++k)
    {
      B.UpdateEdge(b_far[k], new Geom2d_Line(gp_Pnt2d(0, v_bf), gp_Dir2d(1, 0)), F, tol);
      B.Range(b_far[k], F, spans[k].first, spans[k].second);
    }
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
    if (a_low)
      B.Add(W, a_up ? a_far : TopoDS::Edge(a_far.Reversed()));
    else
      for (const TopoDS_Edge& e : b_far)
        B.Add(W, e);
    B.Add(W, seam.Oriented(TopAbs_FORWARD));
    if (a_low)
      for (auto e = b_far.rbegin(); e != b_far.rend(); ++e)
        B.Add(W, e->Reversed());
    else
      B.Add(W, a_up ? TopoDS::Edge(a_far.Reversed()) : a_far);
    B.Add(W, seam.Oriented(TopAbs_REVERSED));
    B.Add(F, W);
    if (!BRepCheck_Analyzer(F).IsValid())
      continue;
    // b's far circle in its cap, with the old one's sense (on a cap that isn't a plane, with its curve on it).
    BRepAdaptor_Curve old_c(bb.end);
    const bool        same = old_c.Circle().Axis().Direction().Dot(gp_Vec(n)) > 0;
    bool              on_cap = true;
    TopoDS_Compound   b_new;
    B.MakeCompound(b_new);
    // The old circle's curve on the cap (a line in its (u, v)) run by the new circle's parameter, over the old one's
    // range (the cap's wire closes through it); else projected.
    auto on_cap_curve = [&](double p0, double p1) -> occ::handle<Geom2d_Curve> {
      double                    t0, t1, c0, c1;
      occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(TopoDS::Edge(bb.end.Oriented(TopAbs_FORWARD)), cap, t0, t1);
      while (const auto t = occ::down_cast<Geom2d_TrimmedCurve>(pc))
        pc = t->BasisCurve();
      occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(bb.end, c0, c1);
      while (const auto t = occ::down_cast<Geom_TrimmedCurve>(c3))
        c3 = t->BasisCurve();
      const auto ln = occ::down_cast<Geom2d_Line>(pc);
      const auto oc = occ::down_cast<Geom_Circle>(c3);
      if (ln.IsNull() || oc.IsNull() || !BRep_Tool::SameParameter(bb.end) || !BRep_Tool::SameRange(bb.end))
        return {};
      const double m  = (p0 + p1) / 2;
      double       tm = ElCLib::Parameter(oc->Circ(), circ->Value(m));
      const double sg = std::remainder(ElCLib::Parameter(oc->Circ(), circ->Value(m + 1e-3)) - tm, 2 * M_PI) > 0 ? 1.0 : -1.0;
      while (tm < t0)
        tm += 2 * M_PI;
      while (tm > t1)
        tm -= 2 * M_PI;
      const gp_Lin2d L = ln->Lin2d();
      return new Geom2d_Line(gp_Pnt2d(L.Location().XY() + L.Direction().XY() * (tm - sg * m)), sg > 0 ? L.Direction() : L.Direction().Reversed());
    };
    for (size_t k = 0; k < b_far.size(); ++k)
    {
      const TopoDS_Edge& e = b_far[k];
      if (!cap.IsNull())
      {
        if (const auto pc = on_cap_curve(spans[k].first, spans[k].second); !pc.IsNull())
        {
          B.UpdateEdge(e, pc, cap, tol);
          B.Range(e, cap, spans[k].first, spans[k].second);
        }
        else
        {
          ShapeFix_Edge fix;
          fix.FixAddPCurve(e, cap, false, tol);
          on_cap = on_cap && !fix.Status(ShapeExtend_FAIL);
        }
      }
      B.Add(b_new, same ? e : TopoDS::Edge(e.Reversed()));
    }
    if (!on_cap)
      continue;
    occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
    rs->Replace(a.Oriented(TopAbs_FORWARD), F);
    rs->Remove(b);
    if (b_far.size() == 1)
      rs->Replace(bb.end.Oriented(TopAbs_FORWARD), same ? b_far[0] : TopoDS::Edge(b_far[0].Reversed()));
    else
      rs->Replace(bb.end.Oriented(TopAbs_FORWARD), b_new);
    const TopoDS_Shape out = rs->Apply(s);
    if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
      continue;
    GProp_GProps g0, g1;
    BRepGProp::VolumeProperties(s, g0);
    BRepGProp::VolumeProperties(out, g1);
    // (Within the integration's noise: a seam moved round a torus cap reads 2e-7 apart, the PEM FH4 stud's.)
    if (std::abs(g1.Mass() - g0.Mass()) > 1e-6 * std::abs(g0.Mass()))
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
    for (const TopoDS_Edge& e : b_far)
      h->AddModified(bb.end, e);
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
  TopLoc_Location           la;
  // (In `ga`'s own frame: a placed face's surface is its untransformed one, imported parts' (c2-gamma-imported).)
  auto project = [&ga, &la](occ::handle<Geom_Curve> c3, double f, double l) -> occ::handle<Geom2d_Curve> {
    if (c3.IsNull())
      return nullptr;
    if (!la.IsIdentity())
      c3 = occ::down_cast<Geom_Curve>(c3->Transformed(la.Transformation().Inverted()));
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
          if (const ShapeList* fs = kept_edge(x.Current()) ? nullptr : ef.Seek(x.Current()))
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
  carry_kept_marks(s0, [&](const TopoDS_Shape& x) { return std::vector<TopoDS_Shape>{cp.ModifiedShape(x)}; });
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
      ga = BRep_Tool::Surface(a, la);
      double tol = 1e-7;
      for (const TopoDS_Edge& e : bnd)
        tol = std::max(tol, BRep_Tool::Tolerance(e));
      // A boundary circle made again with its vertex elsewhere (below), and the one it replaces, everywhere at once.
      TopoDS_Edge swap_from, swap_to;
      // The merged face in place of the group's, kept where valid with the same volume.
      auto replace_by = [&](const TopoDS_Face& F) {
        if (!BRepCheck_Analyzer(F).IsValid())
          return false;
        occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
        if (!swap_from.IsNull())
          rs->Replace(swap_from, swap_to);
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
      // (A whole circle against the other chain with its vertices at other angles: stacked bands of one cylinder from
      // two inputs, their frames a quarter turn apart (c2-gamma-imported's bolt holes, live one face each): the circle made
      // again from the other chain's first vertex (by its point), its curves on its other faces projected, used in its
      // place.)
      const auto whole = [&](int c) {
        if (chains[(size_t)c].size() != 1)
          return false;
        const TopoDS_Edge&            e = bnd[(size_t)chains[(size_t)c][0]];
        double                        f = 0, l = 0;
        const occ::handle<Geom_Curve> cv = BRep_Tool::Curve(e, f, l);
        return BRep_Tool::IsClosed(e) && !occ::down_cast<Geom_Circle>(cv).IsNull() && std::abs(l - f - 2 * M_PI) < 1e-9;
      };
      const int moving = vb.IsNull() ? (whole(1) ? 1 : whole(0) ? 0 : -1) : -1;
      if (moving >= 0)
      {
        const int                                   fixed = 1 - moving;
        const TopoDS_Edge                           e1    = bnd[(size_t)chains[(size_t)moving][0]];
        TopoDS_Vertex                               p0;
        std::tuple<long long, long long, long long> first;
        for (int k : chains[(size_t)fixed])
          for (TopExp_Explorer x(bnd[(size_t)k], TopAbs_VERTEX); x.More(); x.Next())
          {
            const auto kk = key(BRep_Tool::Pnt(TopoDS::Vertex(x.Current())));
            if (p0.IsNull() || kk < first)
            {
              first = kk;
              p0    = TopoDS::Vertex(x.Current());
            }
          }
        double                         f1 = 0, l1 = 0;
        const occ::handle<Geom_Circle> circ = occ::down_cast<Geom_Circle>(BRep_Tool::Curve(e1, f1, l1));
        const double                   t0   = f1 + std::remainder(ElCLib::Parameter(circ->Circ(), BRep_Tool::Pnt(p0)) - f1, 2 * M_PI);
        BRepBuilderAPI_MakeEdge        mk(circ, t0, t0 + 2 * M_PI);
        const ShapeList*               users = ef.Seek(e1);
        bool                           ok    = !p0.IsNull() && mk.IsDone() && users != nullptr;
        TopoDS_Edge                    moved = ok ? mk.Edge() : TopoDS_Edge();
        if (ok)
        {
          BRep_Builder bb;
          bb.UpdateEdge(moved, BRep_Tool::Tolerance(e1));
          for (const TopoDS_Shape& u : *users)
          {
            if (std::find_if(g.begin(), g.end(), [&](int j) { return fm(j).IsSame(u); }) != g.end())
              continue;
            const TopoDS_Face               uf = TopoDS::Face(u);
            TopLoc_Location                 lu;
            const occ::handle<Geom_Surface> su  = BRep_Tool::Surface(uf, lu);
            const occ::handle<Geom_Curve>   c3  = occ::down_cast<Geom_Curve>(circ->Transformed(lu.Transformation().Inverted()));
            const occ::handle<Geom2d_Curve> pc2 = c3.IsNull() ? nullptr : GeomProjLib::Curve2d(c3, t0, t0 + 2 * M_PI, su);
            if (pc2.IsNull())
            {
              ok = false;
              break;
            }
            bb.UpdateEdge(moved, pc2, uf, BRep_Tool::Tolerance(e1));
          }
        }
        if (ok)
        {
          BRepLib::SameParameter(moved, BRep_Tool::Tolerance(e1));
          TopoDS_Vertex p1, q1;
          TopExp::Vertices(moved, p1, q1);
          swap_from                              = e1;
          swap_to                                = moved;
          bnd[(size_t)chains[(size_t)moving][0]] = moved;
          vb                                     = fixed == 0 ? p0 : p1;
          vt                                     = fixed == 0 ? p1 : p0;
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

// Adjacent faces on two surfaces of linear extrusion that are one surface: one's (u, v) the other's (u, k v + c), k
// = ±1 (a mirrored half's: the curve in the mirror plane extruded the other way, c2-hunting-knife's edge strip, live one
// face). The second rebuilt on the first's surface (its curves mapped; for k = -1 its wire and side turned over, the
// material where it was), so OCCT's unify merges them. The shape unchanged when there's no such pair or the result
// isn't valid.
TopoDS_Shape share_mirrored_extrusions(const TopoDS_Shape& s, HHist& hist)
{
  ShapeAncMap ef;
  unique_ancestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  occ::handle<BRepTools_ReShape>                         rs = new BRepTools_ReShape;
  NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> moved;
  BRep_Builder                                           B;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const ShapeList& fs = ef(i);
    if (fs.Extent() != 2 || fs.First().IsSame(fs.Last()) || kept_edge(ef.FindKey(i)))
      continue;
    const TopoDS_Face a = TopoDS::Face(fs.First()), b = TopoDS::Face(fs.Last());
    if (moved.Contains(a) || moved.Contains(b))
      continue;
    // (Compared where they are: a mirror's image placed by a location, c2-hunting-knife's Mirror 1; the face is made
    // again on the first one's surface and location.)
    TopLoc_Location                 la, lb;
    const occ::handle<Geom_Surface> raw_a = BRep_Tool::Surface(a, la), raw_b = BRep_Tool::Surface(b, lb);
    if (raw_a.IsNull() || raw_b.IsNull() || (raw_a == raw_b && la.IsEqual(lb)))
      continue;
    const auto ea = occ::down_cast<Geom_SurfaceOfLinearExtrusion>(BRep_Tool::Surface(a));
    const auto eb = occ::down_cast<Geom_SurfaceOfLinearExtrusion>(BRep_Tool::Surface(b));
    if (ea.IsNull() || eb.IsNull() || !ea->Direction().IsParallel(eb->Direction(), 1e-12))
      continue;
    const double k = ea->Direction().Dot(eb->Direction()) > 0 ? 1.0 : -1.0;
    double       u0, u1, v0, v1;
    BRepTools::UVBounds(b, u0, u1, v0, v1);
    const double c = gp_Vec(ea->BasisCurve()->Value((u0 + u1) / 2), eb->BasisCurve()->Value((u0 + u1) / 2)).Dot(gp_Vec(ea->Direction()));
    Bnd_Box box;
    BRepBndLib::Add(b, box);
    const double tol  = 1e-9 * std::max(1.0, std::sqrt(box.SquareExtent()));
    bool         same = true;
    for (int iu = 0; iu <= 8 && same; ++iu)
      for (int iv = 0; iv <= 2 && same; ++iv)
      {
        const double u = u0 + (u1 - u0) * iu / 8, v = v0 + (v1 - v0) * iv / 2;
        same = eb->Value(u, v).Distance(ea->Value(u, k * v + c)) <= tol;
      }
    bool closed = false;
    for (TopExp_Explorer x(b, TopAbs_EDGE); x.More() && !closed; x.Next())
      closed = BRep_Tool::IsClosed(TopoDS::Edge(x.Current()), b);
    if (!same || closed)
      continue;
    gp_Trsf2d t;
    if (k > 0)
      t.SetTranslation(gp_Vec2d(0, c));
    else
      t.SetMirror(gp_Ax2d(gp_Pnt2d(0, c / 2), gp_Dir2d(1, 0)));
    const occ::handle<Geom_Surface>& ga = raw_a;
    // (In b's own frame: the reshape puts it where b's occurrence is.)
    const TopLoc_Location            to_b = b.Location().Inverted();
    TopoDS_Face                      F;
    B.MakeFace(F, ga, to_b * la, BRep_Tool::Tolerance(b));
    const TopoDS_Face bf = TopoDS::Face(b.Oriented(TopAbs_FORWARD));
    bool              ok = true;
    for (TopoDS_Iterator w(bf); w.More() && ok; w.Next())
    {
      std::vector<TopoDS_Edge> es;
      for (TopExp_Explorer x(w.Value(), TopAbs_EDGE); x.More(); x.Next())
      {
        const TopoDS_Edge e = TopoDS::Edge(x.Current());
        double            f0, f1;
        const auto        pc = BRep_Tool::CurveOnSurface(e, bf, f0, f1);
        if (pc.IsNull())
        {
          ok = false;
          break;
        }
        double g0, g1;
        if (BRep_Tool::CurveOnSurface(e, ga, la, g0, g1).IsNull())
        {
          B.UpdateEdge(e, occ::down_cast<Geom2d_Curve>(pc->Transformed(t)), ga, la, BRep_Tool::Tolerance(e));
          B.Range(e, ga, la, f0, f1);
        }
        const TopoDS_Edge here = TopoDS::Edge(e.Moved(to_b));
        es.push_back(k > 0 ? here : TopoDS::Edge(here.Reversed()));
      }
      if (k < 0)
        std::reverse(es.begin(), es.end());
      TopoDS_Wire W;
      B.MakeWire(W);
      for (const TopoDS_Edge& e : es)
        B.Add(W, e);
      W.Closed(BRep_Tool::IsClosed(W));
      B.Add(F, W);
    }
    if (!ok)
      continue;
    rs->Replace(bf, k > 0 ? TopoDS_Shape(F) : F.Reversed());
    moved.Add(a);
    moved.Add(b);
  }
  if (moved.IsEmpty())
    return s;
  const TopoDS_Shape out = rs->Apply(s);
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    return s;
  if (hist.IsNull())
    hist = new BRepTools_History();
  if (const HHist h = rs->History(); !h.IsNull())
    hist->Merge(h);
  return out;
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
    if (fs.Extent() != 2 || fs.First().IsSame(fs.Last()) || kept_edge(ef.FindKey(i)))
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
  carry_kept_marks(s, [&](const TopoDS_Shape& x) { return std::vector<TopoDS_Shape>{cp.ModifiedShape(x)}; });
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
        if (t != GeomAbs_Line || kept_edge(e))
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
    carry_kept_marks(s, [&](const TopoDS_Shape& x) { return std::vector<TopoDS_Shape>{cp.ModifiedShape(x)}; });
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
TopoDS_Shape merge_cylinder_patches(const TopoDS_Shape& s, HHist& hist, bool valid_known = false)
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
    if (ef(i).Extent() != 2 || kept_edge(ef.FindKey(i)))
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
    carry_kept_marks(s, [&](const TopoDS_Shape& x) { return std::vector<TopoDS_Shape>{cp.ModifiedShape(x)}; });
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
    // (An invalid input can't come out valid: no merge, at the cost of one check, not one per pair (c2-sm-louver's
    // invalid 1382-face fuse: 304 checks, 121 s).)
    if (!valid_known)
    {
      if (!BRepCheck_Analyzer(s).IsValid())
        return s;
      valid_known = true;
    }
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
    // One pair at a time: the shape changed, look again (it is valid).
    return merge_cylinder_patches(out, hist, true);
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

// `refit`: the joined edge's curves on its faces checked against its 3D curve, its tolerance what they need (an
// imported STEP's pieces: their curves on the faces don't follow the 3D curve's parameter exactly).
TopoDS_Shape join_loop_pieces(const TopoDS_Shape& s, HHist& hist, bool refit = false)
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
    if (refit)
    {
      B.SameParameter(E, false);
      BRepLib::SameParameter(E, tol);
    }
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
// Pairs of faces still meeting across an edge (not a seam) on one cylinder or cone: USD's result depends on the faces'
// order. `all`: every pair, else the first.
std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> faces_apart(const TopoDS_Shape& s, bool all)
{
  std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> out;
  // (Cones too: a valve's conical seat cut in two by a tool, its pieces left apart by OCCT's unify in our faces'
  // order where OCCT's own result's merged.)
  ShapeIMap faces;
  TopExp::MapShapes(s, TopAbs_FACE, faces);
  std::vector<std::optional<gp_Cylinder>> cyl(faces.Extent() + 1);
  std::vector<std::optional<gp_Cone>>     cone(faces.Extent() + 1);
  bool                                    any = false;
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    BRepAdaptor_Surface a(TopoDS::Face(faces(i)), false);
    if (a.GetType() == GeomAbs_Cylinder)
    {
      cyl[i] = a.Cylinder();
      any    = true;
    }
    else if (a.GetType() == GeomAbs_Cone)
    {
      cone[i] = a.Cone();
      any     = true;
    }
  }
  if (!any)
    return out;
  ShapeAncMap ef;
  TopExp::MapShapesAndUniqueAncestors(s, TopAbs_EDGE, TopAbs_FACE, ef);
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const auto& fs = ef(i);
    if (fs.Extent() != 2)
      continue;
    const int  a = faces.FindIndex(fs.First()), b = faces.FindIndex(fs.Last());
    if (cyl[a] && cyl[b])
    {
      const gp_Lin la(cyl[a]->Axis()), lb(cyl[b]->Axis());
      const double tol = 1e-9 * std::max(1.0, cyl[a]->Radius());
      if (std::abs(cyl[a]->Radius() - cyl[b]->Radius()) <= tol && la.Direction().IsParallel(lb.Direction(), 1e-9) && la.Distance(lb.Location()) <= tol)
      {
        out.emplace_back(fs.First(), fs.Last());
        if (!all)
          return out;
        continue;
      }
    }
    if (cone[a] && cone[b])
    {
      const double tol = 1e-9 * std::max(1.0, cone[a]->RefRadius());
      if (std::abs(cone[a]->SemiAngle() - cone[b]->SemiAngle()) <= 1e-12 && cone[a]->Apex().Distance(cone[b]->Apex()) <= tol &&
          cone[a]->Axis().Direction().IsParallel(cone[b]->Axis().Direction(), 1e-9))
      {
        out.emplace_back(fs.First(), fs.Last());
        if (!all)
          return out;
      }
    }
  }
  return out;
}

// Two faces of one cylinder or cone meeting only along straight lines of it (along the axis, through a cone's apex)
// whose turns about the axis add up past a full one: joined they'd wrap more than once, which no face can.
bool wraps_past_a_turn(const TopoDS_Face& a, const TopoDS_Face& b)
{
  BRepAdaptor_Surface sa(a, false);
  if (sa.GetType() != GeomAbs_Cylinder && sa.GetType() != GeomAbs_Cone)
    return false;
  const gp_Ax1 axis = sa.GetType() == GeomAbs_Cylinder ? sa.Cylinder().Axis() : sa.Cone().Axis();
  int shared = 0;
  for (TopExp_Explorer x(a, TopAbs_EDGE); x.More(); x.Next())
  {
    bool both = false;
    for (TopExp_Explorer y(b, TopAbs_EDGE); y.More() && !both; y.Next())
      both = x.Current().IsSame(y.Current());
    if (!both)
      continue;
    ++shared;
    BRepAdaptor_Curve c(TopoDS::Edge(x.Current()));
    if (c.GetType() != GeomAbs_Line)
      return false;
    const gp_Lin l = c.Line();
    if (sa.GetType() == GeomAbs_Cylinder ? !l.Direction().IsParallel(axis.Direction(), 1e-9) : l.Distance(sa.Cone().Apex()) > 1e-9 * std::max(1.0, sa.Cone().RefRadius()))
      return false;
  }
  if (shared == 0)
    return false;
  double u0, u1, v0, v1, w0, w1, z0, z1;
  BRepTools::UVBounds(a, u0, u1, v0, v1);
  BRepTools::UVBounds(b, w0, w1, z0, z1);
  // (Exactly a full turn may still merge: c2-valve-config's conical seat, a 3/4 and a 1/4 turn, closes with a seam.)
  return (u1 - u0) + (w1 - w0) > 2 * M_PI + 1e-9;
}

bool cylinder_faces_apart(const TopoDS_Shape& s)
{
  return !faces_apart(s, false).empty();
}

// Pairs of faces the reordered unify left apart in every order it tried (unify_in): a later result whose apart pairs
// are all such gets no tries, which would merge nothing there (c2-housing-draft: 3 tries, 110 ms, on each Boolean of
// its part). Known by their faces' boxes and areas, since the faces come back rebuilt on every Boolean; bounded.
struct StuckPairs
{
  using Key = std::array<double, 14>;
  std::mutex      lock;
  std::deque<Key> keys;
  static constexpr size_t cap = 512;
  static std::array<double, 7> face_key(const TopoDS_Shape& f)
  {
    Bnd_Box b;
    BRepBndLib::Add(f, b, false);
    GProp_GProps g;
    BRepGProp::SurfaceProperties(f, g);
    double x0 = 0, y0 = 0, z0 = 0, x1 = 0, y1 = 0, z1 = 0;
    if (!b.IsVoid())
      b.Get(x0, y0, z0, x1, y1, z1);
    return {x0, y0, z0, x1, y1, z1, g.Mass()};
  }
  static Key key(const std::pair<TopoDS_Shape, TopoDS_Shape>& p)
  {
    std::array<double, 7> a = face_key(p.first), b = face_key(p.second);
    if (b < a)
      std::swap(a, b);
    Key k;
    std::copy(a.begin(), a.end(), k.begin());
    std::copy(b.begin(), b.end(), k.begin() + 7);
    return k;
  }
  static bool same(const Key& a, const Key& b)
  {
    for (size_t i = 0; i < a.size(); ++i)
      if (std::abs(a[i] - b[i]) > 1e-9 * (1e-3 + std::abs(a[i])))
        return false;
    return true;
  }
  bool known(const Key& k) const
  {
    return std::any_of(keys.begin(), keys.end(), [&](const Key& q) { return same(q, k); });
  }
  bool all_known(const std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>>& ps)
  {
    std::vector<Key> ks;
    for (const auto& p : ps)
      ks.push_back(key(p));
    const std::lock_guard<std::mutex> g(lock);
    return !ks.empty() && std::all_of(ks.begin(), ks.end(), [&](const Key& k) { return known(k); });
  }
  void add(const std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>>& ps)
  {
    std::vector<Key> ks;
    for (const auto& p : ps)
      ks.push_back(key(p));
    const std::lock_guard<std::mutex> g(lock);
    for (const Key& k : ks)
      if (!known(k))
      {
        keys.push_back(k);
        if (keys.size() > cap)
          keys.pop_front();
      }
  }
};
StuckPairs& stuck_pairs()
{
  static StuckPairs s;
  return s;
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
  // (Not for an invalid input (c2-sm-louver's fuse): a reordered try is kept only valid, and it gives none, after
  // three more passes.)
  std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> apart = may && !r.IsSame(s0) ? faces_apart(r, true) : std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>>();
  // (Not the pairs no order can merge: two faces meeting only along lines of their surface, whose turns about the axis
  // add up past a full one: c2-helical-sweep's thread strips, a turn each, 77 ms of retries.)
  // (NOSHAPE_UNIFY_RETRY_ALL=1: every pair retried, as before, to compare.)
  static const bool retry_all = std::getenv("NOSHAPE_UNIFY_RETRY_ALL") != nullptr;
  if (!retry_all)
    apart.erase(std::remove_if(apart.begin(), apart.end(), [](const std::pair<TopoDS_Shape, TopoDS_Shape>& p) { return wraps_past_a_turn(TopoDS::Face(p.first), TopoDS::Face(p.second)); }), apart.end());
  if (!apart.empty() && !stuck_pairs().all_known(apart) && BRepCheck_Analyzer(s0).IsValid())
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
    // What no order merged.
    stuck_pairs().add(faces_apart(r, true));
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
      keep_marked_edges(u, s4);
      u.Build();
      carry_kept_marks(s4, u);
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
    keep_marked_edges(u, s1);
    u.Build();
    carry_kept_marks(s1, u);
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
  const TopoDS_Shape shared = share_equal_surfaces(share_mirrored_extrusions(s0b, hs), hs);
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

// Whether the solids a glued fuse left apart don't touch: glue that merged nothing (tools overlapping each other,
// c2-rings-loft's chamfered tips) leaves them all apart, whose volumes add up all the same.
static bool apart_solids_dont_touch(const TopoDS_Shape& result)
{
  std::vector<TopoDS_Shape> solids;
  for (TopExp_Explorer e(result, TopAbs_SOLID); e.More(); e.Next())
    solids.push_back(e.Current());
  if (solids.size() < 2)
    return true;
  std::vector<Bnd_Box> boxes(solids.size());
  for (size_t i = 0; i < solids.size(); ++i)
  {
    BRepBndLib::Add(solids[i], boxes[i], false);
    boxes[i].Enlarge(Precision::Confusion());
  }
  for (size_t i = 0; i < solids.size(); ++i)
    for (size_t j = i + 1; j < solids.size(); ++j)
    {
      if (boxes[i].IsOut(boxes[j]))
        continue;
      BRepExtrema_DistShapeShape d(solids[i], solids[j]);
      if (!d.IsDone() || d.Value() <= Precision::Confusion())
        return false;
    }
  return true;
}

bool volumes_add_up(const std::vector<TopoDS_Shape>& inputs, const TopoDS_Shape& result)
{
  double sum = 0;
  for (const TopoDS_Shape& x : inputs)
    sum += volume_of(x);
  return std::abs(volume_of(result) - sum) <= 1e-7 * std::max(sum, 1e-12) && apart_solids_dont_touch(result);
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

// Whether some two of `tools` meet (their boxes) somewhere outside `target`'s box: big cutters of a pattern reaching
// past the part into each other (c2-valve-config's handles, 2 m long cones meeting below it). A cut by them all at once
// has their crossings out there to build; one at a time (rust_cut_one_by_one) has none.
bool tools_meet_outside(const TopoDS_Shape& target, const std::vector<TopoDS_Shape>& tools)
{
  Bnd_Box tb;
  BRepBndLib::Add(target, tb, false);
  if (tb.IsVoid())
    return false;
  std::vector<Bnd_Box> boxes(tools.size());
  for (size_t i = 0; i < tools.size(); ++i)
    BRepBndLib::Add(tools[i], boxes[i], false);
  double x0, y0, z0, x1, y1, z1;
  tb.Get(x0, y0, z0, x1, y1, z1);
  for (size_t i = 0; i < tools.size(); ++i)
    for (size_t j = i + 1; j < tools.size(); ++j)
    {
      if (boxes[i].IsVoid() || boxes[j].IsVoid() || boxes[i].IsOut(boxes[j]))
        continue;
      double a0, b0, c0, a1, b1, c1, d0, e0, f0, d1, e1, f1;
      boxes[i].Get(a0, b0, c0, a1, b1, c1);
      boxes[j].Get(d0, e0, f0, d1, e1, f1);
      const double lo[3] = {std::max(a0, d0), std::max(b0, e0), std::max(c0, f0)};
      const double hi[3] = {std::min(a1, d1), std::min(b1, e1), std::min(c1, f1)};
      if (lo[0] < x0 || lo[1] < y0 || lo[2] < z0 || hi[0] > x1 || hi[1] > y1 || hi[2] > z1)
        return true;
    }
  return false;
}

// A cut by several tools as one cut each with noshape's Boolean, one after another, where their pieces inside the
// target keep apart: each step taken by it, no step touching (removing or changing) a face, edge or vertex an earlier
// tool left, and each tool's faces left in the part (its walls) clear of the earlier ones' boxes. Then the result is
// the cut by them all at once, and the history (composed through the steps) is too. False (nothing done) otherwise.
bool rust_cut_one_by_one(const TopoDS_Shape& target, const std::vector<TopoDS_Shape>& tools, const BoolOpts& opts, TopoDS_Shape& out, HHist& hist)
{
  const RustLenient               lenient;
  TopoDS_Shape                    cur = target;
  std::vector<std::vector<Stage>> stages;
  std::vector<Bnd_Box>            walls;
  ShapeIMap                       left; // what the earlier tools left in the part: faces, their edges and vertices
  for (const TopoDS_Shape& tool : tools)
  {
    fail_if_cancelled();
    const std::vector<TopoDS_Shape>       in{cur, tool};
    std::vector<const OcShape*>           rs;
    std::vector<std::unique_ptr<OcShape>> own;
    TopoDS_Shape                          next;
    HHist                                 h;
    if (!rust_boolean(1, in, rs, own, next, h, opts.fuzzy, false))
      return false;
    for (int k = 1; k <= left.Extent(); ++k)
      if (h->IsRemoved(left(k)) || !h->Modified(left(k)).IsEmpty())
        return rust_decline("tools meeting inside the target");
    // The tool's faces in the part: its walls.
    ShapeIMap mine;
    for (TopExp_Explorer f(tool, TopAbs_FACE); f.More(); f.Next())
    {
      if (h->IsRemoved(f.Current()))
        continue;
      const ShapeList& m = h->Modified(f.Current());
      if (m.IsEmpty())
        mine.Add(f.Current());
      for (ShapeList::Iterator it(m); it.More(); it.Next())
        mine.Add(it.Value());
    }
    Bnd_Box wall;
    for (int k = 1; k <= mine.Extent(); ++k)
    {
      BRepBndLib::Add(mine(k), wall, false);
      for (TopAbs_ShapeEnum t : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
        for (TopExp_Explorer x(mine(k), t); x.More(); x.Next())
          left.Add(x.Current());
    }
    if (!wall.IsVoid())
    {
      wall.Enlarge(1e-6);
      for (const Bnd_Box& w : walls)
        if (!w.IsOut(wall))
          return rust_decline("tools meeting inside the target");
      walls.push_back(wall);
    }
    stages.push_back({stage_of(in, h)});
    cur = next;
  }
  std::vector<TopoDS_Shape> all{target};
  all.insert(all.end(), tools.begin(), tools.end());
  out  = cur;
  hist = compose(all, stages);
  // (compose has faces, edges and vertices: the solids through the steps too, from the one that takes each in, as
  // OCCT's cut by them all says: the target's Modified, a tool's Deleted.)
  auto solids = [&](const TopoDS_Shape& in, size_t from) {
    for (TopExp_Explorer x(in, TopAbs_SOLID); x.More(); x.Next())
    {
      std::vector<TopoDS_Shape> cur{x.Current()};
      for (size_t k = from; k < stages.size() && !cur.empty(); ++k)
      {
        std::vector<TopoDS_Shape> next;
        for (const TopoDS_Shape& y : cur)
        {
          const HHist& sh = stages[k][0].hist;
          if (sh->IsRemoved(y))
            continue;
          const ShapeList& m = sh->Modified(y);
          if (m.IsEmpty())
            next.push_back(y);
          for (ShapeList::Iterator it(m); it.More(); it.Next())
            next.push_back(it.Value());
        }
        cur = std::move(next);
      }
      if (cur.empty())
        hist->Remove(x.Current());
      for (const TopoDS_Shape& y : cur)
        if (!y.IsSame(x.Current()))
          hist->AddModified(x.Current(), y);
    }
  };
  solids(target, 0);
  for (size_t i = 0; i < tools.size(); ++i)
    solids(tools[i], i);
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


// Two solids meeting on one planar face each, the two faces' outlines the same edge for edge and each solid on its own
// side of their plane (a part split on a plane, its halves drafted about it and fused again: cf-part-draft's Part
// Draft 3, two 153-face halves on z 0, 1.9 s of OCCT's). The fuse is the two shells sewn along that outline, without
// the two faces. False when it doesn't apply; the caller fuses as usual.
static bool fuse_glued(const TopoDS_Shape& target, const TopoDS_Shape& tool, TopoDS_Shape& out, HHist& hist)
{
  auto only = [](const TopoDS_Shape& s, TopAbs_ShapeEnum k, TopoDS_Shape& one) {
    int n = 0;
    for (TopExp_Explorer e(s, k); e.More(); e.Next(), ++n)
      one = e.Current();
    return n == 1;
  };
  TopoDS_Shape sa, sb, sha, shb;
  if (!only(target, TopAbs_SOLID, sa) || !only(tool, TopAbs_SOLID, sb) || !only(sa, TopAbs_SHELL, sha) || !only(sb, TopAbs_SHELL, shb))
    return false;
  // (Not two that share a vertex already: OCCT keeps or rebuilds those as its own rules say.)
  {
    ShapeIMap va;
    TopExp::MapShapes(sa, TopAbs_VERTEX, va);
    for (TopExp_Explorer x(sb, TopAbs_VERTEX); x.More(); x.Next())
      if (va.Contains(x.Current()))
        return false;
  }
  // Each planar face's plane, its normal outward.
  struct Flat
  {
    TopoDS_Face face;
    gp_Pln      pln;
    gp_Dir      n;
  };
  auto flats = [](const TopoDS_Shape& s) {
    std::vector<Flat> v;
    for (TopExp_Explorer x(s, TopAbs_FACE); x.More(); x.Next())
    {
      const TopoDS_Face&  f = TopoDS::Face(x.Current());
      BRepAdaptor_Surface su(f, false);
      if (su.GetType() != GeomAbs_Plane)
        continue;
      const gp_Pln pl = su.Plane();
      gp_Dir       n  = pl.Axis().Direction();
      if (f.Orientation() == TopAbs_REVERSED)
        n.Reverse();
      v.push_back({f, pl, n});
    }
    return v;
  };
  const std::vector<Flat> fa = flats(sa), fb = flats(sb);
  double tol = 1e-7;
  for (const TopoDS_Shape* sh : {&sa, &sb})
    for (TopExp_Explorer x(*sh, TopAbs_EDGE); x.More(); x.Next())
      tol = std::max(tol, BRep_Tool::Tolerance(TopoDS::Edge(x.Current())));
  // The one pair facing each other on one plane; no other planar face of either on it.
  const Flat* pa = nullptr;
  const Flat* pb = nullptr;
  for (const Flat& x : fa)
    for (const Flat& y : fb)
      if (x.n.Dot(y.n) < -1 + 1e-12 && x.pln.Distance(y.pln.Location()) <= tol)
      {
        if (pa != nullptr)
          return false;
        pa = &x;
        pb = &y;
      }
  if (pa == nullptr)
    return false;
  const gp_Pln& pl  = pa->pln;
  auto          on = [&](const std::vector<Flat>& v, const Flat* keep) {
    return std::any_of(v.begin(), v.end(), [&](const Flat& x) { return &x != keep && x.pln.Axis().Direction().IsParallel(pl.Axis().Direction(), 1e-12) && pl.Distance(x.pln.Location()) <= tol; });
  };
  if (on(fa, pa) || on(fb, pb))
    return false;
  // The same outline: each of b's edges one of a's, by its ends and its middle.
  ShapeIMap ea, eb;
  TopExp::MapShapes(pa->face, TopAbs_EDGE, ea);
  TopExp::MapShapes(pb->face, TopAbs_EDGE, eb);
  if (ea.Extent() != eb.Extent() || ea.Extent() == 0)
    return false;
  auto mid = [](const TopoDS_Edge& e) {
    BRepAdaptor_Curve c(e);
    return c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
  };
  std::vector<char> used((size_t)ea.Extent() + 1);
  for (int j = 1; j <= eb.Extent(); ++j)
  {
    const TopoDS_Edge& e  = TopoDS::Edge(eb(j));
    const gp_Pnt       b0 = BRep_Tool::Pnt(TopExp::FirstVertex(e)), b1 = BRep_Tool::Pnt(TopExp::LastVertex(e)), bm = mid(e);
    bool               found = false;
    for (int i = 1; i <= ea.Extent() && !found; ++i)
    {
      if (used[(size_t)i])
        continue;
      const TopoDS_Edge& d  = TopoDS::Edge(ea(i));
      const double       t  = 2 * tol;
      const gp_Pnt       a0 = BRep_Tool::Pnt(TopExp::FirstVertex(d)), a1 = BRep_Tool::Pnt(TopExp::LastVertex(d));
      if (((a0.Distance(b0) <= t && a1.Distance(b1) <= t) || (a0.Distance(b1) <= t && a1.Distance(b0) <= t)) && mid(d).Distance(bm) <= t)
        found = used[(size_t)i] = 1;
    }
    if (!found)
      return false;
  }
  // Each on its own side: its box in the plane's frame, and its vertices on the plane the face's.
  gp_Trsf to_plane;
  to_plane.SetTransformation(gp_Ax3(pl.Location(), pa->n));
  auto side = [&](const TopoDS_Shape& s, const TopoDS_Face& f, bool below) {
    Bnd_Box box;
    BRepBndLib::Add(s.Moved(TopLoc_Location(to_plane)), box, false);
    double x0, y0, z0, x1, y1, z1;
    box.Get(x0, y0, z0, x1, y1, z1);
    if (below ? z1 > 10 * tol + box.GetGap() : z0 < -10 * tol - box.GetGap())
      return false;
    ShapeIMap fv;
    TopExp::MapShapes(f, TopAbs_VERTEX, fv);
    for (TopExp_Explorer x(s, TopAbs_VERTEX); x.More(); x.Next())
      if (!fv.Contains(x.Current()) && pl.Distance(BRep_Tool::Pnt(TopoDS::Vertex(x.Current()))) <= 2 * tol)
        return false;
    return true;
  };
  if (!side(sa, pa->face, true) || !side(sb, pb->face, false))
    return false;
  // Sewn: each shell without its face, joined along the outline.
  BRep_Builder b;
  TopoDS_Shell s1, s2;
  b.MakeShell(s1);
  b.MakeShell(s2);
  for (TopoDS_Iterator x(sha); x.More(); x.Next())
    if (!x.Value().IsSame(pa->face))
      b.Add(s1, x.Value());
  for (TopoDS_Iterator x(shb); x.More(); x.Next())
    if (!x.Value().IsSame(pb->face))
      b.Add(s2, x.Value());
  BRepBuilderAPI_Sewing sew(2 * tol, true, false, false, false);
  sew.Add(s1);
  sew.Add(s2);
  sew.Perform();
  if (sew.NbFreeEdges() != 0 || sew.NbMultipleEdges() != 0)
    return false;
  TopoDS_Shape shell;
  if (!only(sew.SewedShape(), TopAbs_SHELL, shell))
    return false;
  TopoDS_Solid sol;
  b.MakeSolid(sol);
  TopoDS_Shell closed = TopoDS::Shell(shell);
  closed.Closed(true);
  b.Add(sol, closed);
  BRepLib::OrientClosedSolid(sol);
  if (!BRepCheck_Analyzer(sol).IsValid())
    return false;
  // (As OCCT's fuse records it: both solids deleted, and what touches the outline (its vertices, the edges at them,
  // the faces of those edges) modified even where the sewing kept it.)
  hist = new BRepTools_History();
  hist->Remove(sa);
  hist->Remove(sb);
  ShapeIMap touch;
  for (const TopoDS_Face* f : {&pa->face, &pb->face})
    TopExp::MapShapes(*f, TopAbs_VERTEX, touch);
  for (const TopoDS_Shape* sh : {&sa, &sb})
  {
    ShapeAncMap ve, ef;
    TopExp::MapShapesAndUniqueAncestors(*sh, TopAbs_VERTEX, TopAbs_EDGE, ve);
    TopExp::MapShapesAndUniqueAncestors(*sh, TopAbs_EDGE, TopAbs_FACE, ef);
    for (int i = 1, n = touch.Extent(); i <= n; ++i)
      if (const ShapeList* es = touch(i).ShapeType() == TopAbs_VERTEX ? ve.Seek(touch(i)) : nullptr)
        for (ShapeList::Iterator e(*es); e.More(); e.Next())
        {
          touch.Add(e.Value());
          for (ShapeList::Iterator f(ef.FindFromKey(e.Value())); f.More(); f.Next())
            touch.Add(f.Value());
        }
  }
  for (const TopoDS_Shape* sh : {&sa, &sb})
    for (const TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    {
      ShapeIMap m;
      TopExp::MapShapes(*sh, k, m);
      for (int i = 1; i <= m.Extent(); ++i)
      {
        const TopoDS_Shape& x = m(i);
        const TopoDS_Shape  y = sew.IsModifiedSubShape(x) ? sew.ModifiedSubShape(x) : x;
        if (x.IsSame(pa->face) || x.IsSame(pb->face))
          hist->Remove(x);
        else if (!y.IsSame(x) || touch.Contains(x))
          hist->AddModified(x, y);
      }
    }
  out = sol;
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
  // (Sheets: their meshes' check misses two flat faces sharing an edge, which grouped stayed apart in the fuse, both
  // kept, the edge twice: a parity document's 12 sheets, 47 edges where one fuse of them all gives 16.)
  for (const TopoDS_Shape& s : t)
    if (!TopExp_Explorer(s, TopAbs_SOLID).More())
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
  // A pattern's instances share one shape, placed: meshed once (a mesh is kept in the shape's own frame), each
  // instance its placed copy (cf-surface-pattern's 85 spheres: their grouping 4.7 s -> 1.7 s).
  NCollection_DataMap<occ::handle<TopoDS_TShape>, TopoDS_Shape> meshed;
  for (int i = 0; i < n; ++i)
  {
    if (!close[(size_t)i])
      continue;
    fail_if_cancelled();
    const TopoDS_Shape& x    = t[(size_t)i];
    const TopoDS_Shape* base = meshed.Seek(x.TShape());
    if (base == nullptr)
    {
      const TopoDS_Shape m = BRepBuilderAPI_Copy(x.Located(TopLoc_Location()), true, false).Shape();
      BRepMesh_IncrementalMesh(m, defl, false, 0.5);
      base = meshed.Bound(x.TShape(), m);
    }
    const TopoDS_Shape c = base->Located(x.Location()).Oriented(x.Orientation());
    for (TopExp_Explorer f(c, TopAbs_FACE); f.More(); f.Next())
      owner.Bind(f.Current(), i);
    bb.Add(copies, c);
  }
  std::vector<std::vector<char>> conflict((size_t)n, std::vector<char>((size_t)n, 0));
  // (None meshed: no two can meet, one group.)
  if (!owner.IsEmpty())
  {
    // On a thread of its own, so a cancel needn't wait for it (one OCCT call over every tool's mesh without a break
    // hook: cf-surface-pattern's 85 tools, 0.6 s): the worker leaves it to finish alone.
    struct Check
    {
      std::mutex                                    m;
      std::condition_variable                       cv;
      bool                                          done = false, ok = false;
      std::shared_ptr<BRepExtrema_SelfIntersection> si;
    };
    auto job = std::make_shared<Check>();
    try
    {
      job->si = std::make_shared<BRepExtrema_SelfIntersection>(copies, 2 * defl);
    }
    catch (const Standard_Failure&)
    {
      return t;
    }
    std::thread([job] {
      bool ok = false;
      try
      {
        job->si->Perform();
        ok = job->si->IsDone();
      }
      catch (...)
      {
      }
      const std::lock_guard<std::mutex> g(job->m);
      job->ok   = ok;
      job->done = true;
      job->cv.notify_all();
    }).detach();
    {
      std::unique_lock<std::mutex> l(job->m);
      while (!job->cv.wait_for(l, std::chrono::milliseconds(5), [&] { return job->done; }))
      {
        l.unlock();
        fail_if_cancelled();
        l.lock();
      }
    }
    if (!job->ok)
      return t;
    try
    {
      const BRepExtrema_SelfIntersection& si = *job->si;
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

// `r`'s volume less `a`'s and `t`'s, from the faces they don't share only (a face in both, its place and side the
// same, adds as much to each): those faces close up, so they are measured about one point (BRepGProp's own sum).
double volume_change(const std::vector<TopoDS_Shape>& a, const std::vector<TopoDS_Shape>& t, const TopoDS_Shape& r)
{
  NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> fwd, rvs;
  auto count = [&](const TopoDS_Shape& s, int by) {
    for (TopExp_Explorer x(s, TopAbs_FACE); x.More(); x.Next())
    {
      const TopAbs_Orientation o = x.Current().Orientation();
      if (o != TopAbs_FORWARD && o != TopAbs_REVERSED)
        continue;
      auto& m = o == TopAbs_FORWARD ? fwd : rvs;
      if (int* n = m.ChangeSeek(x.Current()))
        *n += by;
      else
        m.Bind(x.Current(), by);
    }
  };
  for (const TopoDS_Shape& x : a)
    count(x, -1);
  for (const TopoDS_Shape& x : t)
    count(x, -1);
  count(r, 1);
  // (In the shapes' own order, each face as often as it is in one and not the other: the inputs' turned over.)
  TopoDS_Compound c;
  BRep_Builder    bb;
  bb.MakeCompound(c);
  auto add = [&](const TopoDS_Shape& s, bool input) {
    for (TopExp_Explorer x(s, TopAbs_FACE); x.More(); x.Next())
    {
      const TopAbs_Orientation o = x.Current().Orientation();
      if (o != TopAbs_FORWARD && o != TopAbs_REVERSED)
        continue;
      int* n = (o == TopAbs_FORWARD ? fwd : rvs).ChangeSeek(x.Current());
      if (n == nullptr || (input ? *n >= 0 : *n <= 0))
        continue;
      *n += input ? 1 : -1;
      bb.Add(c, input ? x.Current().Reversed() : x.Current());
    }
  };
  add(r, false);
  for (const TopoDS_Shape& x : a)
    add(x, true);
  for (const TopoDS_Shape& x : t)
    add(x, true);
  GProp_GProps g;
  BRepGProp::VolumeProperties(c, g);
  return g.Mass();
}

// The sum of `xs`' box volumes: no more than their volumes'.
double box_volumes(const std::vector<TopoDS_Shape>& xs)
{
  double v = 0;
  for (const TopoDS_Shape& x : xs)
  {
    Bnd_Box b;
    BRepBndLib::Add(x, b);
    if (b.IsVoid())
      continue;
    double x0 = 0, y0 = 0, z0 = 0, x1 = 0, y1 = 0, z1 = 0;
    b.Get(x0, y0, z0, x1, y1, z1);
    v += (x1 - x0) * (y1 - y0) * (z1 - z0);
  }
  return v;
}

// Whether `r` is what the Boolean gives for solids that don't overlap: a
// union of their volume together, a cut of the target's, an empty common.
bool no_interaction(uint8_t op, const std::vector<TopoDS_Shape>& a, const std::vector<TopoDS_Shape>& t, const TopoDS_Shape& r)
{
  // (First the volume the faces that changed make, against ten times the tolerance below at the inputs' boxes: a
  // 1355-face part's whole volume took 0.7 s after its 0.3 s cut, a private document's.)
  const double most = 1e-8 * std::max(box_volumes(a) + box_volumes(t), 1e-12);
  if (std::abs(op == 2 ? volume_of(r) : volume_change(a, op == 0 ? t : std::vector<TopoDS_Shape>(), r)) > most)
    return false;
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
  // (Each solid's classifier is built once, and only for points in an argument's box: cf-snap-hook's cuts that miss
  // took 0.4 s each in classifiers rebuilt per face.)
  std::vector<Bnd_Box>                                     boxes(a.size());
  std::vector<std::unique_ptr<BRepClass3d_SolidClassifier>> in_a(a.size());
  for (size_t i = 0; i < a.size(); ++i)
    BRepBndLib::Add(a[i], boxes[i]);
  for (const TopoDS_Shape& x : t)
  {
    Bnd_Box bx;
    BRepBndLib::Add(x, bx);
    const double eps = 1e-6 * std::sqrt(bx.SquareExtent());
    if (std::all_of(boxes.begin(), boxes.end(), [&](const Bnd_Box& b) { return b.IsOut(bx); }))
      continue;
    std::unique_ptr<BRepClass3d_SolidClassifier> in_x;
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
      std::vector<size_t> inside;
      for (size_t i = 0; i < a.size(); ++i)
        if (!boxes[i].IsOut(q))
          inside.push_back(i);
      if (inside.empty())
        continue;
      if (!in_x)
        in_x = std::make_unique<BRepClass3d_SolidClassifier>(x);
      in_x->Perform(q, eps * 1e-3);
      if (in_x->State() != TopAbs_IN)
        continue;
      for (const size_t i : inside)
      {
        if (!in_a[i])
          in_a[i] = std::make_unique<BRepClass3d_SolidClassifier>(a[i]);
        in_a[i]->Perform(q, eps * 1e-3);
        if (in_a[i]->State() == TopAbs_IN)
          return true;
      }
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

static OpOut op_boolean_built(uint8_t op, const OcShapeList& args, const OcShapeList& tools, const BoolOpts& opts);

OpOut op_boolean(uint8_t op, const OcShapeList& args, const OcShapeList& tools,
                 const BoolOpts& opts)
{
  OpOut r = op_boolean_built(op, args, tools, opts);
  if (r.shape)
  {
    ShapeIMap inputs;
    for (const OcShapeList* l : {&args, &tools})
      for (const TopoDS_Shape& x : l->items)
        TopExp::MapShapes(x, TopAbs_EDGE, inputs);
    circles_on_revolutions(r.shape->shape, inputs);
  }
  return r;
}

namespace
{
// An argument's B-spline face lying on another argument's cylinder (a thread swept on its shaft, its inner face on the
// shaft's: asm-hand-drill-pinslot's Sweep 1; OCCT's fuse of the two cut the shaft in three, live's keeps one).
bool on_anothers_cylinder(const std::vector<TopoDS_Shape>& all)
{
  // A face's outward normal at its parameters' middle.
  auto normal_at = [](const TopoDS_Face& f, gp_Pnt& p, gp_Vec& n) {
    double u0 = 0, u1 = 0, v0 = 0, v1 = 0;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    BRepGProp_Face gf(f);
    gf.Normal(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, n);
    return n.Magnitude() > 1e-12;
  };
  for (size_t i = 0; i < all.size(); ++i)
  {
    std::vector<TopoDS_Face> others;
    for (size_t j = 0; j < all.size(); ++j)
      if (j != i)
        for (TopExp_Explorer e(all[j], TopAbs_FACE); e.More(); e.Next())
          if (BRepAdaptor_Surface(TopoDS::Face(e.Current())).GetType() == GeomAbs_Cylinder)
            others.push_back(TopoDS::Face(e.Current()));
    if (others.empty())
      continue;
    for (TopExp_Explorer e(all[i], TopAbs_FACE); e.More(); e.Next())
    {
      const TopoDS_Face&                         f = TopoDS::Face(e.Current());
      TopLoc_Location                            loc;
      const occ::handle<Geom_Surface>            s = BRep_Tool::Surface(f, loc);
      const occ::handle<Geom_CylindricalSurface> c = occ::down_cast<Geom_CylindricalSurface>(as_cylinder(s, f));
      if (c.IsNull())
        continue;
      const gp_Cylinder g = c->Cylinder().Transformed(loc.Transformation());
      gp_Pnt            p;
      gp_Vec            n;
      if (!normal_at(f, p, n))
        continue;
      Bnd_Box fb;
      BRepBndLib::Add(f, fb);
      // (On it, over it and facing it: touching it from the other side, as a thread its shaft.)
      for (const TopoDS_Face& o : others)
      {
        const gp_Cylinder oc = BRepAdaptor_Surface(o).Cylinder();
        if (std::abs(oc.Radius() - g.Radius()) > Precision::Confusion() || !oc.Axis().Direction().IsParallel(g.Axis().Direction(), 1e-6)
            || gp_Lin(oc.Axis()).Distance(g.Location()) > Precision::Confusion())
          continue;
        Bnd_Box ob;
        BRepBndLib::Add(o, ob);
        if (fb.IsOut(ob))
          continue;
        gp_Pnt q;
        gp_Vec m;
        if (!normal_at(o, q, m))
          continue;
        // (The two normals radial at their points: opposite ways round the axis.)
        const gp_Dir ax = oc.Axis().Direction();
        const gp_Vec rp = gp_Vec(oc.Location(), p) - gp_Vec(ax) * gp_Vec(oc.Location(), p).Dot(gp_Vec(ax));
        const gp_Vec rq = gp_Vec(oc.Location(), q) - gp_Vec(ax) * gp_Vec(oc.Location(), q).Dot(gp_Vec(ax));
        if (n.Dot(rp) * m.Dot(rq) < 0)
          return true;
      }
    }
  }
  return false;
}
} // namespace

static OpOut op_boolean_built(uint8_t op, const OcShapeList& args, const OcShapeList& tools, const BoolOpts& opts)
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
  // A face pattern's checked cut by tools that are compounds of solids (an instance's separate pieces) cuts by their
  // solids: the grouping of tools apart and the cut from the faces' classification go by tools
  // (FACE-PATTERN-30-CUT-SLOW: 29 compounds of 3 took 117 s, their 87 solids 1.9 s). The history still names the
  // tools' own sub-shapes. (Other cuts keep their tools: a compound of 900 holes has its own paths.)
  if (op == 1 && opts.glue == 4 && facets.source.empty())
  {
    std::vector<TopoDS_Shape> flat;
    for (const TopoDS_Shape& x : t)
    {
      std::vector<TopoDS_Shape> solids;
      int                       others = 0;
      if (x.ShapeType() == TopAbs_COMPOUND)
        for (TopoDS_Iterator it(x); it.More(); it.Next())
          (it.Value().ShapeType() == TopAbs_SOLID ? solids.push_back(it.Value()) : (void)++others);
      if (solids.size() >= 2 && others == 0)
        flat.insert(flat.end(), solids.begin(), solids.end());
      else
        flat.push_back(x);
    }
    t = std::move(flat);
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
  // Two solids meeting on a whole planar face each (fuse_glued): first where either has free-form faces (noshape's
  // Boolean's slow part: cf-part-draft's halves 1.2 s), else once it has declined (it is quicker on the rest).
  const bool glue_case = op == 0 && a.size() == 1 && t.size() == 1 && facets.source.empty() && opts.fuzzy <= 0 && !opts.unify;
  auto       free_form = [](const TopoDS_Shape& x) {
    for (TopExp_Explorer f(x, TopAbs_FACE); f.More(); f.Next())
      switch (BRepAdaptor_Surface(TopoDS::Face(f.Current()), false).GetType())
      {
        case GeomAbs_Plane:
        case GeomAbs_Cylinder:
        case GeomAbs_Cone:
        case GeomAbs_Sphere:
        case GeomAbs_Torus:
          break;
        default:
          return true;
      }
    return false;
  };
  const bool glue_first = glue_case && (free_form(a[0]) || free_form(t[0]));
  auto       glued      = [&]() -> std::optional<OpOut> {
    TopoDS_Shape out;
    HHist        h;
    bool         done = false;
    {
      const KernelTimer timer("boolean.glued");
      done = fuse_glued(a[0], t[0], out, h);
    }
    if (!done)
      return std::nullopt;
    t_bool_path = "glued";
    return make_result(concat(args.items, tools.items), out, h.get(), &srcs);
  };
  if (glue_first)
    if (std::optional<OpOut> r = glued())
      return std::move(*r);
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
    bool                                  done       = false;
    bool                                  one_by_one = false;
    // (Asked to cut scaled copies, OCCT's path below would: its history has the object's entities all Modified.)
    const bool scaled_inputs = opts.scaled && working_scale() != 1.0 && std::getenv("NOSHAPE_BOOL_NO_SCALE") == nullptr;
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
          // (Instances overlapping one another too, so the trimmed ones don't just touch: OCCT's fuse_trimmed fails
          // there (their volumes don't add up) and OCCT runs the plain fuse; so does ours (c2-rings-loft's
          // Circular pattern 1: 8 rings, 222 ms against OCCT's 472).)
          if (!done)
            done = rust_boolean(op, concat(a, t), rs, own, out, h, opts.fuzzy, false);
        }
        if (!done)
          rust_decline(nullptr);
      }
      else
      {
        // (Big cutters meeting each other past the part: one at a time, where their pieces in it keep apart.)
        if (op == 1 && a.size() == 1 && t.size() >= 2 && t.size() <= 16 && opts.glue == 0 && tools_meet_outside(a[0], t))
        {
          const KernelTimer one("boolean.rust.one_by_one");
          done       = rust_cut_one_by_one(a[0], t, opts, out, h);
          one_by_one = done;
          if (done)
            rs = srcs;
        }
        if (!done)
          done = rust_boolean(op, concat(a, t), rs, own, out, h, opts.fuzzy, false, scaled_inputs);
      }
    }
    if (done)
    {
      t_bool_path = "rust";
      // A target known to have nothing to merge: the unify (here or op_unify's) looks only where it changed.
      std::shared_ptr<const UnifyScope> scope;
      if (!one_by_one && rs[0] != nullptr && rs[0]->myUnifyClean && unify_local_on())
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
  if (glue_case && !glue_first)
    if (std::optional<OpOut> r = glued())
      return std::move(*r);
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
  if ((opts.scaled || (op == 1 && n_tools >= 64 && opts.glue == 0)) && facets.source.empty() && working_scale() != 1.0 && std::getenv("NOSHAPE_BOOL_NO_SCALE") == nullptr)
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
    // Again from the faces' classification when the first cut was (FACE-PATTERN-30-CUT-SLOW: without it OCCT's split
    // solids of 87 pattern tools took 141 s, with it 1.1 s).
    {
      const CutByFaces scope(by_faces);
      build_cancellable(*b);
    }
    if (by_faces && (b->HasErrors() || !b->IsDone()))
    {
      by_faces = false;
      b        = make();
      b->SetArguments(to_list(a));
      b->SetTools(to_list(t));
      apply_opts(*b, plain);
      build_cancellable(*b);
    }
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
  // A fuse come out invalid where an argument's free-form face lies on another's cylinder, touching it from outside
  // (asm-hand-drill-pinslot's thread on its shaft: OCCT's fuse cut the shaft in three): again with OCCT's glue for
  // shapes that touch, kept when valid and the volumes add up (one shaft, as live's).
  if (op == 0 && !scaled && t_bool_path == std::string("occt") && first.glue == 0 && first.fuzzy <= 0 && facets.source.empty() && all_solids(inputs)
      && on_anothers_cylinder(inputs) && !BRepCheck_Analyzer(res).IsValid())
  {
    const KernelTimer timer("boolean.glued_on_cylinder");
    BoolOpts          glued = first;
    glued.glue              = 1;
    std::unique_ptr<BRepAlgoAPI_BooleanOperation> b2 = make();
    b2->SetArguments(to_list(args.items));
    b2->SetTools(to_list(tools.items));
    apply_opts(*b2, glued);
    build_cancellable(*b2);
    fail_if_cancelled();
    if (!b2->HasErrors() && b2->IsDone() && volumes_add_up(inputs, b2->Shape()) && BRepCheck_Analyzer(b2->Shape()).IsValid())
    {
      t_bool_path = "occt_glued";
      inputs      = concat(args.items, tools.items);
      hist        = cancellable_history(inputs, *b2);
      res         = b2->Shape();
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

namespace
{
// One solid split by one planar face covering its whole section (a convex polygon holding the projection of the
// solid's box): the solid's common with the half-space on the face's side and its cut by it, by noshape's Boolean, with
// OCCT's Splitter's history (the tool face Modified into the section faces). False where it doesn't apply or declines.
bool split_by_plane_ours(const TopoDS_Shape& a, const TopoDS_Shape& tool, TopoDS_Shape& out, HHist& hist)
{
  if (!rust_boolean_on() || a.ShapeType() != TopAbs_SOLID || tool.ShapeType() != TopAbs_FACE)
    return false;
  const TopoDS_Face   f = TopoDS::Face(tool);
  BRepAdaptor_Surface su(f, false);
  if (su.GetType() != GeomAbs_Plane)
    return false;
  // A convex polygon: one wire of straight edges turning one way.
  ShapeIMap wires;
  TopExp::MapShapes(f, TopAbs_WIRE, wires);
  if (wires.Extent() != 1)
    return false;
  const gp_Pln        pl = su.Plane();
  const gp_Dir        n  = pl.Axis().Direction();
  std::vector<gp_Pnt> poly;
  for (BRepTools_WireExplorer w(TopoDS::Wire(wires(1)), f); w.More(); w.Next())
  {
    if (BRepAdaptor_Curve(w.Current()).GetType() != GeomAbs_Line)
      return false;
    poly.push_back(BRep_Tool::Pnt(w.CurrentVertex()));
  }
  if (poly.size() < 3)
    return false;
  int turn = 0;
  for (size_t i = 0; i < poly.size(); ++i)
  {
    const gp_Vec u(poly[i], poly[(i + 1) % poly.size()]), v(poly[(i + 1) % poly.size()], poly[(i + 2) % poly.size()]);
    const double c = u.Crossed(v).Dot(gp_Vec(n));
    if (std::abs(c) <= 1e-18)
      continue;
    const int sg = c > 0 ? 1 : -1;
    if (turn != 0 && sg != turn)
      return false;
    turn = sg;
  }
  // The solid's box, its corners on the plane inside the polygon.
  Bnd_Box bb;
  BRepBndLib::Add(a, bb, false);
  if (bb.IsVoid())
    return false;
  double x0, y0, z0, x1, y1, z1;
  bb.Get(x0, y0, z0, x1, y1, z1);
  for (int k = 0; k < 8; ++k)
  {
    const gp_Pnt c((k & 1) ? x1 : x0, (k & 2) ? y1 : y0, (k & 4) ? z1 : z0);
    const gp_Pnt q = c.Translated(gp_Vec(n) * -gp_Vec(pl.Location(), c).Dot(gp_Vec(n)));
    BRepClass_FaceClassifier fc(f, q, Precision::Confusion());
    if (fc.State() != TopAbs_IN && fc.State() != TopAbs_ON)
      return false;
  }
  // The half-space: a square on the plane past the box, swept along the normal past it.
  const double size = 2 * std::sqrt(bb.SquareExtent()) + 1;
  const gp_Pnt mid  = gp_Pnt((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2);
  const gp_Pnt o    = mid.Translated(gp_Vec(n) * -gp_Vec(pl.Location(), mid).Dot(gp_Vec(n)));
  const gp_Pln sq(gp_Ax3(o, n, pl.Position().XDirection()));
  const TopoDS_Face base = BRepBuilderAPI_MakeFace(sq, -size, size, -size, size).Face();
  BRepPrimAPI_MakePrism hp(base, gp_Vec(n) * size, false, true);
  if (!hp.IsDone())
    return false;
  const TopoDS_Shape h = hp.Shape();
  TopoDS_Face        hbase;
  for (TopExp_Explorer e(h, TopAbs_FACE); e.More(); e.Next())
  {
    BRepAdaptor_Surface hs(TopoDS::Face(e.Current()), false);
    if (hs.GetType() == GeomAbs_Plane && std::abs(gp_Vec(hs.Plane().Location(), o).Dot(gp_Vec(n))) <= 1e-12 * size &&
        hs.Plane().Axis().Direction().IsParallel(n, 1e-12))
      hbase = TopoDS::Face(e.Current());
  }
  if (hbase.IsNull())
    return false;
  TopoDS_Shape                          parts[2];
  HHist                                 hs[2];
  for (int k = 0; k < 2; ++k)
  {
    std::vector<const OcShape*>           srcs;
    std::vector<std::unique_ptr<OcShape>> own;
    if (!rust_boolean(k == 0 ? 2 : 1, {a, h}, srcs, own, parts[k], hs[k], 0.0, false))
      return false;
    if (!TopExp_Explorer(parts[k], TopAbs_SOLID).More())
      return false;
  }
  // The halves share their cut faces, edges and vertices, as OCCT's Splitter's (the display meshes faces shared between
  // bodies once, and names go by them): the cut's replaced by the common's, matched by place.
  occ::handle<BRepTools_History> moved;
  {
    if (hs[0].IsNull() || hs[1].IsNull())
      return false;
    std::vector<TopoDS_Face> cuts[2];
    for (int k = 0; k < 2; ++k)
      for (ShapeList::Iterator it(hs[k]->Modified(hbase)); it.More(); it.Next())
        cuts[k].push_back(TopoDS::Face(it.Value()));
    // (Each of the common's matched in the cut's; the cut's others are the solid's own faces on the plane (a board's top
    // around the parts standing on it: cf-fix-pcb), not shared.)
    if (cuts[0].empty() || cuts[0].size() > cuts[1].size())
      return false;
    auto props = [](const TopoDS_Face& x) {
      GProp_GProps g;
      BRepGProp::SurfaceProperties(x, g);
      return std::make_pair(g.Mass(), g.CentreOfMass());
    };
    auto tol_of = [](const TopoDS_Shape& x, const TopoDS_Shape& y) { return 2 * (BRep_Tool::MaxTolerance(x, TopAbs_VERTEX) + BRep_Tool::MaxTolerance(y, TopAbs_VERTEX)) + 1e-9; };
    occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
    std::vector<std::pair<double, gp_Pnt>> p2;
    for (const TopoDS_Face& x : cuts[1])
      p2.push_back(props(x));
    std::vector<char> used(cuts[1].size(), 0);
    for (const TopoDS_Face& f1 : cuts[0])
    {
      const auto [a1, c1] = props(f1);
      size_t     found = cuts[1].size();
      for (size_t i = 0; i < cuts[1].size() && found == cuts[1].size(); ++i)
        if (!used[i] && std::abs(p2[i].first - a1) <= 1e-9 * std::max(a1, p2[i].first) && p2[i].second.Distance(c1) <= tol_of(f1, cuts[1][i]))
          found = i;
      if (found == cuts[1].size())
        return false;
      used[found]           = 1;
      const TopoDS_Face& f2 = cuts[1][found];
      const double       tol = tol_of(f1, f2);
      ShapeIMap          v1s, e1s;
      TopExp::MapShapes(f1, TopAbs_VERTEX, v1s);
      TopExp::MapShapes(f1, TopAbs_EDGE, e1s);
      NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher> vmap;
      for (TopExp_Explorer x(f2, TopAbs_VERTEX); x.More(); x.Next())
      {
        if (vmap.IsBound(x.Current()))
          continue;
        const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(x.Current()));
        int          found = 0;
        for (int i = 1; i <= v1s.Extent() && found == 0; ++i)
          if (BRep_Tool::Pnt(TopoDS::Vertex(v1s(i))).Distance(p) <= tol)
            found = i;
        if (found == 0)
          return false;
        vmap.Bind(x.Current(), v1s(found));
        rs->Replace(x.Current().Oriented(TopAbs_FORWARD), v1s(found).Oriented(TopAbs_FORWARD));
      }
      for (TopExp_Explorer x(f2, TopAbs_EDGE); x.More(); x.Next())
      {
        const TopoDS_Edge e2 = TopoDS::Edge(x.Current().Oriented(TopAbs_FORWARD));
        if (rs->IsRecorded(e2))
          continue;
        TopoDS_Vertex a2v, b2v;
        TopExp::Vertices(e2, a2v, b2v);
        BRepAdaptor_Curve ca(e2);
        const gp_Pnt      m2 = ca.Value(0.5 * (ca.FirstParameter() + ca.LastParameter()));
        int               found = 0;
        bool              same  = true;
        for (int i = 1; i <= e1s.Extent() && found == 0; ++i)
        {
          const TopoDS_Edge e1 = TopoDS::Edge(e1s(i).Oriented(TopAbs_FORWARD));
          TopoDS_Vertex     a1v, b1v;
          TopExp::Vertices(e1, a1v, b1v);
          BRepAdaptor_Curve cb(e1);
          if (cb.Value(0.5 * (cb.FirstParameter() + cb.LastParameter())).Distance(m2) > tol)
            continue;
          if (a1v.IsSame(vmap.Find(a2v)) && b1v.IsSame(vmap.Find(b2v)))
            found = i, same = true;
          else if (a1v.IsSame(vmap.Find(b2v)) && b1v.IsSame(vmap.Find(a2v)))
            found = i, same = false;
        }
        if (found == 0)
          return false;
        rs->Replace(e2, e1s(found).Oriented(same ? TopAbs_FORWARD : TopAbs_REVERSED));
      }
      // (Its normal the way the cut's own face's was.)
      BRepAdaptor_Surface sa(f1, false), sb(f2, false);
      const bool alike = sa.Plane().Axis().Direction().Dot(sb.Plane().Axis().Direction()) > 0;
      rs->Replace(f2.Oriented(TopAbs_FORWARD), f1.Oriented(alike ? TopAbs_FORWARD : TopAbs_REVERSED));
    }
    parts[1] = rs->Apply(parts[1]);
    moved    = rs->History();
  }
  // Where the cut's own history's images went in the sharing.
  auto shared = [&](const TopoDS_Shape& x, const std::function<void(const TopoDS_Shape&)>& add) {
    if (moved.IsNull() || !moved->IsSupportedType(x) || (moved->Modified(x).IsEmpty() && !moved->IsRemoved(x)))
    {
      add(x);
      return;
    }
    for (ShapeList::Iterator it(moved->Modified(x)); it.More(); it.Next())
      add(it.Value());
  };
  // The history: an argument's sub-shape kept where either kept it, its images in both, removed where both removed it;
  // the tool face into the half-space's base face's images; its edges and vertices gone.
  hist = new BRepTools_History();
  for (TopAbs_ShapeEnum kind : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
  {
    ShapeIMap m;
    TopExp::MapShapes(a, kind, m);
    for (int i = 1; i <= m.Extent(); ++i)
    {
      bool removed = true;
      for (int k = 0; k < 2; ++k)
      {
        const HHist& x = hs[k];
        if (x.IsNull())
        {
          removed = false;
          continue;
        }
        if (!x->IsRemoved(m(i)))
          removed = false;
        for (ShapeList::Iterator it(x->Modified(m(i))); it.More(); it.Next())
          shared(it.Value(), [&](const TopoDS_Shape& y) { hist->AddModified(m(i), y); });
        for (ShapeList::Iterator it(x->Generated(m(i))); it.More(); it.Next())
          shared(it.Value(), [&](const TopoDS_Shape& y) { hist->AddGenerated(m(i), y); });
      }
      // (Kept by the cut and rebuilt by the sharing: its new self.)
      if (!hs[1]->IsRemoved(m(i)) && hs[1]->Modified(m(i)).IsEmpty() && !moved.IsNull() && moved->IsSupportedType(m(i)) && !moved->Modified(m(i)).IsEmpty())
        for (ShapeList::Iterator it(moved->Modified(m(i))); it.More(); it.Next())
          hist->AddModified(m(i), it.Value());
      if (removed)
        hist->Remove(m(i));
    }
  }
  // (The shared cut faces once, the common's; the cut's own besides.)
  for (ShapeList::Iterator it(hs[0]->Modified(hbase)); it.More(); it.Next())
    hist->AddModified(f, it.Value());
  for (ShapeList::Iterator it(hs[1]->Modified(hbase)); it.More(); it.Next())
    if (moved.IsNull() || !moved->IsRemoved(it.Value()))
      shared(it.Value(), [&](const TopoDS_Shape& y) {
        bool dup = false;
        for (ShapeList::Iterator jt(hist->Modified(f)); jt.More() && !dup; jt.Next())
          dup = jt.Value().IsSame(y);
        if (!dup)
          hist->AddModified(f, y);
      });
  for (int k = 0; k < 2; ++k)
    for (ShapeList::Iterator it(hs[k]->Generated(hbase)); it.More(); it.Next())
      shared(it.Value(), [&](const TopoDS_Shape& y) { hist->AddGenerated(f, y); });
  for (TopAbs_ShapeEnum kind : {TopAbs_EDGE, TopAbs_VERTEX})
    for (TopExp_Explorer e(f, kind); e.More(); e.Next())
      hist->Remove(e.Current());
  BRep_Builder    b;
  TopoDS_Compound c;
  b.MakeCompound(c);
  for (const TopoDS_Shape& p : parts)
    for (TopExp_Explorer e(p, TopAbs_SOLID); e.More(); e.Next())
      b.Add(c, e.Current());
  out = c;
  return true;
}
} // namespace

// `shape`'s faces split along `edges`, each lying on one of them within `tol` (its curve on that face projected) and
// ending on its boundary: in 2D, no curve checked against another face (a blend's touch curves on its walls:
// c2-impeller-loft's Fillet 1, its general split ~270 ms of 9 edge-face intersections).
OpOut op_split_on(const OcShape& shape, const OcShapeList& edges, double tol)
{
  require_non_null(shape.shape, "shape");
  if (edges.items.empty())
    fail_invalid("split needs at least one edge");
  const KernelTimer   timer("split.on");
  ShapeIMap           faces;
  TopExp::MapShapes(shape.shape, TopAbs_FACE, faces);
  BRepFeat_SplitShape        sp(shape.shape);
  BRep_Builder               b;
  std::vector<TopoDS_Vertex> ends;
  for (const TopoDS_Shape& x : edges.items)
  {
    const TopoDS_Edge e = TopoDS::Edge(x);
    double            f, l;
    const occ::handle<Geom_Curve> c3 = BRep_Tool::Curve(e, f, l);
    if (c3.IsNull())
      fail_invalid("an edge without a curve");
    // The face it lies on: its middle on it.
    const TopoDS_Vertex mid = BRepBuilderAPI_MakeVertex(c3->Value(0.5 * (f + l)));
    int                 on  = 0;
    double              best = tol;
    for (int j = 1; j <= faces.Extent(); ++j)
    {
      BRepExtrema_DistShapeShape d(mid, faces(j));
      if (d.IsDone() && d.Value() <= best)
      {
        best = d.Value();
        on   = j;
      }
    }
    if (on == 0)
      fail_op("an edge on none of the faces");
    const TopoDS_Face               face = TopoDS::Face(faces(on));
    double                          reached = 0;
    const occ::handle<Geom2d_Curve> c2 = GeomProjLib::Curve2d(c3, f, l, BRep_Tool::Surface(face), reached);
    if (c2.IsNull())
      fail_op("an edge with no curve on its face");
    // A vertex at `p`: the one already there (two lofts' touch curves meeting mid-face chain into one cut).
    auto vertex_at = [&](const gp_Pnt& p, double vtol) {
      for (const TopoDS_Vertex& w : ends)
        if (BRep_Tool::Pnt(w).Distance(p) <= tol)
          return w;
      TopoDS_Vertex v;
      b.MakeVertex(v, p, std::max(vtol, tol));
      ends.push_back(v);
      return v;
    };
    // Where its curve on a periodic face crosses the face's seam: cut there, the piece past it a period over.
    std::vector<double>             cuts{f, l};
    const occ::handle<Geom_Surface> surf = BRep_Tool::Surface(face);
    double                          u0, u1, v0, v1;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    if (surf->IsUPeriodic())
    {
      const double per = surf->UPeriod();
      auto         out = [&](double t) { return c2->Value(t).X() < u0 - 1e-9 * per || c2->Value(t).X() > u1 + 1e-9 * per; };
      constexpr int N = 64;
      for (int k = 0; k < N; ++k)
      {
        double ta = f + (l - f) * k / N, tb = f + (l - f) * (k + 1) / N;
        if (out(ta) == out(tb))
          continue;
        const bool oa = out(ta);
        for (int it = 0; it < 60; ++it)
        {
          const double tm = 0.5 * (ta + tb);
          (out(tm) == oa ? ta : tb) = tm;
        }
        cuts.push_back(0.5 * (ta + tb));
      }
    }
    std::sort(cuts.begin(), cuts.end());
    const double etol = std::max({BRep_Tool::Tolerance(e), reached, tol * 0.1});
    for (size_t k = 0; k + 1 < cuts.size(); ++k)
    {
      const double t0 = cuts[k], t1 = cuts[k + 1];
      if (t1 - t0 <= 1e-12 * (l - f))
        continue;
      occ::handle<Geom2d_Curve> pc = c2;
      if (surf->IsUPeriodic())
      {
        const double per = surf->UPeriod(), um = c2->Value(0.5 * (t0 + t1)).X();
        const double sh  = um < u0 ? per * std::ceil((u0 - um) / per) : um > u1 ? -per * std::ceil((um - u1) / per) : 0.0;
        if (sh != 0)
          pc = occ::down_cast<Geom2d_Curve>(c2->Translated(gp_Vec2d(sh, 0)));
      }
      TopoDS_Edge piece;
      b.MakeEdge(piece, c3, etol);
      const TopoDS_Vertex a  = vertex_at(c3->Value(t0), etol), z = vertex_at(c3->Value(t1), etol);
      b.Add(piece, a.Oriented(TopAbs_FORWARD));
      b.Add(piece, z.Oriented(TopAbs_REVERSED));
      b.Range(piece, t0, t1);
      b.UpdateEdge(piece, pc, face, etol);
      b.Range(piece, face, t0, t1);
      b.UpdateVertex(a, t0, piece, etol);
      b.UpdateVertex(z, t1, piece, etol);
      sp.Add(piece, face);
    }
  }
  sp.Build();
  if (!sp.IsDone())
    fail_op("split along edges failed");
  HHist hist = new BRepTools_History(to_list({shape.shape}), sp);
  return make_result({shape.shape}, sp.Shape(), hist.get());
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
  // A solid split by one planar face across it: noshape's Boolean (cf-fix-pcb's 328-face board: OCCT's 470 ms).
  if (a.size() == 1 && t.size() == 1 && facets.source.empty() && opts.fuzzy <= 0 && std::getenv("NOSHAPE_SPLIT_OCCT") == nullptr)
  {
    TopoDS_Shape out;
    HHist        h;
    bool         done = false;
    {
      const KernelTimer timer("split.ours");
      done = split_by_plane_ours(a[0], t[0], out, h);
    }
    if (done)
    {
      if (opts.unify)
      {
        const KernelTimer timer("split.ours.unify");
        out = unify(out, h);
      }
      const KernelTimer timer("split.ours.result");
      return make_result(concat(args.items, tools.items), out, h.get());
    }
  }
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
  keep_marked_edges(u, s.shape);
  u.Build();
  carry_kept_marks(s.shape, u);
  fail_if_cancelled();
  if (!BRepCheck_Analyzer(u.Shape()).IsValid())
    return make_result({s.shape}, s.shape, nullptr);
  HHist hist = new BRepTools_History();
  if (!u.History().IsNull())
    hist->Merge(u.History());
  return make_result({s.shape}, u.Shape(), hist.get());
}

// Edges shorter than `len` whose middle is within `len` of one of `pts` collapsed into one vertex at their middle, its
// tolerance reaching both old ends, as Parasolid leaves a blend's sliver end (c2-hunting-knife's Chamfer 1: a strip
// 0.011 mm wide where the chamfer meets the slope ends in one vertex, live 2462's JZQ). Edges between two faces;
// unchanged when the result isn't valid.
// The edges with their middle within `tol` of one of `pts` lying within `tol` of one of `conics` (11 numbers each:
// centre, normal, major axis direction, major and minor radius), rebuilt on it on the same vertices with their
// pcurves anew: a blend's contact curves built as B-splines where they are conics (c2-hunting-knife's Fillet 3: a
// rolling ball between a plane and a cylinder touches each along an ellipse, as live's). The shape unchanged where the
// result isn't valid.
OpOut op_canonical_edges(const OcShape& s, rust::Slice<const double> pts, rust::Slice<const double> conics, double tol)
{
  std::vector<gp_Pnt> at;
  for (size_t i = 0; i + 2 < pts.size(); i += 3)
    at.emplace_back(pts[i], pts[i + 1], pts[i + 2]);
  std::vector<occ::handle<Geom_Curve>> candidates;
  for (size_t i = 0; i + 10 < conics.size(); i += 11)
  {
    const double* q = &conics[i];
    const gp_Ax2  ax(gp_Pnt(q[0], q[1], q[2]), gp_Dir(q[3], q[4], q[5]), gp_Dir(q[6], q[7], q[8]));
    // (Only a true circle: live's tip contact on c2-hunting-knife, 3.5001 x 3.5, is an ellipse.)
    if (q[9] - q[10] <= 1e-9 * q[9])
      candidates.push_back(occ::handle<Geom_Curve>(new Geom_Circle(gp_Circ(ax, q[10]))));
    else
      candidates.push_back(occ::handle<Geom_Curve>(new Geom_Ellipse(gp_Elips(ax, q[9], q[10]))));
  }
  ShapeAncMap ef;
  TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, ef);
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  BRep_Builder                   B;
  bool                           any = false;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const TopoDS_Edge e = TopoDS::Edge(ef.FindKey(i).Oriented(TopAbs_FORWARD));
    if (BRep_Tool::Degenerated(e))
      continue;
    const BRepAdaptor_Curve c(e);
    const GeomAbs_CurveType type = c.GetType();
    if (type == GeomAbs_Line || type == GeomAbs_Circle || type == GeomAbs_Ellipse)
      continue;
    const double f0 = c.FirstParameter(), f1 = c.LastParameter();
    const gp_Pnt mid = c.Value(0.5 * (f0 + f1));
    if (std::none_of(at.begin(), at.end(), [&](const gp_Pnt& p) { return p.Distance(mid) <= tol; }))
      continue;
    bool seam = false;
    for (const TopoDS_Shape& f : ef(i))
      seam = seam || BRep_Tool::IsClosed(e, TopoDS::Face(f));
    if (seam)
      continue;
    occ::handle<Geom_Curve> g;
    for (const occ::handle<Geom_Curve>& cand : candidates)
    {
      bool on = true;
      for (int k = 0; k <= 8 && on; ++k)
      {
        GeomAPI_ProjectPointOnCurve pr(c.Value(f0 + (f1 - f0) * k / 8.0), cand);
        on = pr.NbPoints() > 0 && pr.LowerDistance() <= tol;
      }
      if (on)
      {
        g = occ::down_cast<Geom_Curve>(cand->Copy());
        break;
      }
    }
    if (g.IsNull())
      continue;
    TopoDS_Vertex v0, v1;
    TopExp::Vertices(e, v0, v1);
    GeomAPI_ProjectPointOnCurve p0(BRep_Tool::Pnt(v0), g), p1(BRep_Tool::Pnt(v1), g);
    if (p0.NbPoints() == 0 || p1.NbPoints() == 0)
      continue;
    double a = p0.LowerDistanceParameter(), b = p1.LowerDistanceParameter();
    // Along the old curve.
    gp_Pnt q;
    gp_Vec d, d_old;
    g->D1(a, q, d);
    c.D1(f0, q, d_old);
    if (d.Dot(d_old) < 0)
    {
      a = g->ReversedParameter(a);
      b = g->ReversedParameter(b);
      g = g->Reversed();
    }
    while (b <= a)
      b += g->Period();
    const double gap = std::max(BRep_Tool::Pnt(v0).Distance(g->Value(a)), BRep_Tool::Pnt(v1).Distance(g->Value(b)));
    const double te  = std::max(BRep_Tool::Tolerance(e), 1.5 * gap);
    TopoDS_Edge  ne;
    B.MakeEdge(ne, g, te);
    // The vertices widened to reach the new curve's ends.
    auto vertex = [&](const TopoDS_Vertex& v) {
      const TopoDS_Shape fv = v.Oriented(TopAbs_FORWARD);
      if (rs->IsRecorded(fv))
        return TopoDS::Vertex(rs->Value(fv));
      if (BRep_Tool::Tolerance(v) >= te)
        return v;
      TopoDS_Vertex nv;
      B.MakeVertex(nv, BRep_Tool::Pnt(v), te);
      rs->Replace(fv, nv);
      return nv;
    };
    B.Add(ne, vertex(v0).Oriented(TopAbs_FORWARD));
    B.Add(ne, vertex(v1).Oriented(TopAbs_REVERSED));
    B.Range(ne, a, b);
    ShapeFix_Edge fix;
    for (const TopoDS_Shape& f : ef(i))
      fix.FixAddPCurve(ne, TopoDS::Face(f), false, tol);
    fix.FixSameParameter(ne);
    fix.FixVertexTolerance(ne);
    rs->Replace(e, ne);
    any = true;
  }
  if (!any)
    return make_result({s.shape}, s.shape, nullptr);
  const TopoDS_Shape out = rs->Apply(s.shape);
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    return make_result({s.shape}, s.shape, nullptr);
  return make_result({s.shape}, out, rs->History().get());
}

OpOut op_collapse_short_edges(const OcShape& s, rust::Slice<const double> pts, double len)
{
  if (pts.size() % 3 != 0)
    fail_invalid("points must be triples");
  require_non_null(s.shape, "shape");
  ShapeIMap es;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, es);
  NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<TopoDS_Shape>, TopTools_ShapeMapHasher> ef;
  TopExp::MapShapesAndUniqueAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, ef);
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  BRep_Builder                   bb;
  ShapeIMap                      used;
  int                            n = 0;
  for (int i = 1; i <= es.Extent(); ++i)
  {
    const TopoDS_Edge e = TopoDS::Edge(es(i));
    if (BRep_Tool::Degenerated(e))
      continue;
    GProp_GProps gp;
    BRepGProp::LinearProperties(e, gp);
    const NCollection_List<TopoDS_Shape>* fl = ef.Seek(e);
    if (gp.Mass() >= len || fl == nullptr || fl->Extent() != 2)
      continue;
    TopoDS_Vertex v1, v2;
    TopExp::Vertices(e, v1, v2);
    if (v1.IsNull() || v2.IsNull() || v1.IsSame(v2) || used.Contains(v1) || used.Contains(v2))
      continue;
    const gp_Pnt p1 = BRep_Tool::Pnt(v1), p2 = BRep_Tool::Pnt(v2);
    const gp_Pnt pm(0.5 * (p1.XYZ() + p2.XYZ()));
    bool         asked = false;
    for (size_t j = 0; j + 2 < pts.size() && !asked; j += 3)
      asked = pm.Distance(gp_Pnt(pts[j], pts[j + 1], pts[j + 2])) <= len;
    if (!asked)
      continue;
    TopoDS_Vertex vm;
    bb.MakeVertex(vm, gp_Pnt(0.5 * (p1.XYZ() + p2.XYZ())), 0.5 * p1.Distance(p2) + std::max(BRep_Tool::Tolerance(v1), BRep_Tool::Tolerance(v2)));
    rs->Replace(v1.Oriented(TopAbs_FORWARD), vm.Oriented(TopAbs_FORWARD));
    rs->Replace(v2.Oriented(TopAbs_FORWARD), vm.Oriented(TopAbs_FORWARD));
    rs->Remove(e);
    used.Add(v1);
    used.Add(v2);
    ++n;
  }
  if (n == 0)
    return make_result({s.shape}, s.shape, nullptr);
  try
  {
    const TopoDS_Shape out = rs->Apply(s.shape);
    if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
      return make_result({s.shape}, s.shape, nullptr);
    return make_result({s.shape}, out, rs->History().get());
  }
  catch (const Standard_Failure&)
  {
    return make_result({s.shape}, s.shape, nullptr);
  }
}

// Edges between the same two faces joined where they meet tangentially at a vertex near one of `pts`, their curves
// concatenated (a line and the B-spline section running on from it); every other vertex kept.
OpOut op_join_edges_at(const OcShape& s, rust::Slice<const double> pts, double tol)
{
  require_non_null(s.shape, "shape");
  if (pts.size() % 3 != 0)
    fail_invalid("points must be triples");
  ShapeIMap vs;
  TopExp::MapShapes(s.shape, TopAbs_VERTEX, vs);
  std::vector<bool> free(vs.Extent() + 1, false);
  for (int i = 1; i <= vs.Extent(); ++i)
  {
    const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(vs(i)));
    for (size_t j = 0; j < pts.size() && !free[i]; j += 3)
      free[i] = p.Distance(gp_Pnt(pts[j], pts[j + 1], pts[j + 2])) <= tol;
  }
  // A straight section (an approximation, so not "linear" to OCCT's unify) running on from a line there: a line.
  NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<TopoDS_Shape>, TopTools_ShapeMapHasher> ve, ef;
  TopExp::MapShapesAndUniqueAncestors(s.shape, TopAbs_VERTEX, TopAbs_EDGE, ve);
  TopExp::MapShapesAndUniqueAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, ef);
  occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
  bool                           straightened = false;
  for (int i = 1; i <= vs.Extent(); ++i)
  {
    const NCollection_List<TopoDS_Shape>* es = free[i] ? ve.Seek(vs(i)) : nullptr;
    if (es == nullptr || es->Extent() != 2)
      continue;
    TopoDS_Edge a = TopoDS::Edge(es->First()), b = TopoDS::Edge(es->Last());
    if (BRepAdaptor_Curve(b).GetType() == GeomAbs_Line)
      std::swap(a, b);
    BRepAdaptor_Curve ca(a), cb(b);
    if (ca.GetType() != GeomAbs_Line || cb.GetType() == GeomAbs_Line || BRep_Tool::Degenerated(b))
      continue;
    TopoDS_Vertex b0, b1;
    TopExp::Vertices(b, b0, b1);
    const gp_Pnt p0 = BRep_Tool::Pnt(b0), p1 = BRep_Tool::Pnt(b1);
    if (p0.Distance(p1) <= tol || gp_Vec(p0, p1).Angle(gp_Vec(ca.Line().Direction())) > 1e-4 && gp_Vec(p0, p1).Angle(-gp_Vec(ca.Line().Direction())) > 1e-4)
      continue;
    const gp_Lin chord(p0, gp_Dir(gp_Vec(p0, p1)));
    const double dev = std::max(tol, BRep_Tool::Tolerance(b));
    bool         straight = true;
    for (int k = 1; k < 16 && straight; ++k)
      straight = chord.Distance(cb.Value(cb.FirstParameter() + (cb.LastParameter() - cb.FirstParameter()) * k / 16.0)) <= dev;
    const NCollection_List<TopoDS_Shape>* fs = ef.Seek(b);
    if (!straight || fs == nullptr)
      continue;
    BRepBuilderAPI_MakeEdge mk(TopoDS::Vertex(b0.Oriented(TopAbs_FORWARD)), TopoDS::Vertex(b1.Oriented(TopAbs_FORWARD)));
    if (!mk.IsDone())
      continue;
    TopoDS_Edge line = mk.Edge();
    for (NCollection_List<TopoDS_Shape>::Iterator f(*fs); f.More(); f.Next())
      ShapeFix_Edge().FixAddPCurve(line, TopoDS::Face(f.Value()), false, dev);
    BRep_Builder().UpdateEdge(line, dev);
    rs->Replace(b.Oriented(TopAbs_FORWARD), line);
    straightened = true;
  }
  const TopoDS_Shape in = straightened ? rs->Apply(s.shape) : s.shape;
  ShapeUpgrade_UnifySameDomain u(in, true, false, false);
  for (int i = 1; i <= vs.Extent(); ++i)
    if (!free[i])
      u.KeepShape(vs(i));
  keep_marked_edges(u, in);
  u.Build();
  carry_kept_marks(in, u);
  if (!BRepCheck_Analyzer(u.Shape()).IsValid())
    return make_result({s.shape}, s.shape, nullptr);
  HHist hist = new BRepTools_History();
  if (straightened && !rs->History().IsNull())
    hist->Merge(rs->History());
  if (!u.History().IsNull())
    hist->Merge(u.History());
  return make_result({s.shape}, u.Shape(), hist.get());
}

namespace
{
// Surfaces of revolution that are a torus, sphere, cylinder or cone made those (an IGES writer's revolved faces: a
// torus in quarters, each its own surface, which UnifySameDomain doesn't merge). OCCT's modification for the
// revolutions only, its pcurve copy fixed for faces it doesn't change (it read a null surface for a torus face next
// to a converted one).
class RevolutionToElementary : public ShapeCustom_SweptToElementary
{
public:
  static bool revolution(const occ::handle<Geom_Surface>& s) { return !s.IsNull() && s->IsKind(STANDARD_TYPE(Geom_SurfaceOfRevolution)); }

  bool NewSurface(const TopoDS_Face& F, occ::handle<Geom_Surface>& S, TopLoc_Location& L, double& Tol, bool& RevWires, bool& RevFace) override
  {
    if (!revolution(BRep_Tool::Surface(F)))
      return false;
    return ShapeCustom_SweptToElementary::NewSurface(F, S, L, Tol, RevWires, RevFace);
  }

  bool NewCurve2d(const TopoDS_Edge& E, const TopoDS_Face& F, const TopoDS_Edge& NewE, const TopoDS_Face& NewF, occ::handle<Geom2d_Curve>& C, double& Tol) override
  {
    if (revolution(BRep_Tool::Surface(F)))
      return ShapeCustom_SweptToElementary::NewCurve2d(E, F, NewE, NewF, C, Tol);
    if (E.IsSame(NewE))
      return false;
    double f, l;
    C = BRep_Tool::CurveOnSurface(E, F, f, l);
    if (C.IsNull())
      return false;
    C   = occ::down_cast<Geom2d_Curve>(C->Copy());
    Tol = BRep_Tool::Tolerance(E);
    return true;
  }
};

// B-spline faces that are a plane, cylinder, cone, sphere or torus within `tol` (an IGES writer's rational torus
// quarters, live's o-ring), rebuilt on that surface: their edges kept, pcurves projected. `reshape` maps them.
TopoDS_Shape bspline_faces_analytic(const TopoDS_Shape& shape, double tol, const occ::handle<BRepTools_ReShape>& reshape)
{
  bool any = false;
  for (TopExp_Explorer e(shape, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face&              f = TopoDS::Face(e.Current());
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> s = BRep_Tool::Surface(f, loc);
    if (s.IsNull() || reshape->IsRecorded(f) || !(s->IsKind(STANDARD_TYPE(Geom_BSplineSurface)) || s->IsKind(STANDARD_TYPE(Geom_BezierSurface))))
      continue;
    GeomConvert_SurfToAnaSurf       conv(s);
    const occ::handle<Geom_Surface> a = conv.ConvertToAnalytical(tol);
    if (a.IsNull() || conv.Gap() > tol)
      continue;
    // The same side out: the new surface's normal against the old one's at the face's middle.
    double u0, u1, v0, v1;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    gp_Pnt p;
    gp_Vec du, dv;
    s->D1((u0 + u1) / 2, (v0 + v1) / 2, p, du, dv);
    const occ::handle<ShapeAnalysis_Surface> sa = new ShapeAnalysis_Surface(a);
    const gp_Pnt2d                           uv = sa->ValueOfUV(p, tol);
    gp_Pnt                                   q;
    gp_Vec                                   au, av;
    a->D1(uv.X(), uv.Y(), q, au, av);
    const bool   flip = du.Crossed(dv).Dot(au.Crossed(av)) < 0;
    BRep_Builder b;
    TopoDS_Face  nf;
    b.MakeFace(nf, a, loc, std::max(BRep_Tool::Tolerance(f), conv.Gap()));
    for (TopoDS_Iterator w(f.Oriented(TopAbs_FORWARD)); w.More(); w.Next())
      b.Add(nf, flip ? w.Value().Reversed() : w.Value());
    ShapeFix_Face fix(nf);
    fix.FixOrientationMode() = 0;
    fix.Perform();
    TopoDS_Face out = fix.Face();
    if (flip)
      out.Reverse();
    if (f.Orientation() == TopAbs_REVERSED)
      out.Reverse();
    reshape->Replace(f, out);
    any = true;
  }
  return any ? reshape->Apply(shape) : shape;
}

// Whether two faces lie on one plane, or (`periodic`) one cylinder, cone, sphere or torus (the cheap test before a merge).
// Two copies of one B-spline surface (an IGES writer's pieces of a periodic one, each its own entity).
bool same_bspline_surface(const occ::handle<Geom_Surface>& a, const occ::handle<Geom_Surface>& b, double tol)
{
  const auto p = occ::down_cast<Geom_BSplineSurface>(a), q = occ::down_cast<Geom_BSplineSurface>(b);
  if (p.IsNull() || q.IsNull() || p->UDegree() != q->UDegree() || p->VDegree() != q->VDegree() || p->NbUPoles() != q->NbUPoles() || p->NbVPoles() != q->NbVPoles() ||
      p->NbUKnots() != q->NbUKnots() || p->NbVKnots() != q->NbVKnots())
    return false;
  for (int i = 1; i <= p->NbUKnots(); ++i)
    if (std::abs(p->UKnot(i) - q->UKnot(i)) > 1e-12 || p->UMultiplicity(i) != q->UMultiplicity(i))
      return false;
  for (int i = 1; i <= p->NbVKnots(); ++i)
    if (std::abs(p->VKnot(i) - q->VKnot(i)) > 1e-12 || p->VMultiplicity(i) != q->VMultiplicity(i))
      return false;
  for (int i = 1; i <= p->NbUPoles(); ++i)
    for (int j = 1; j <= p->NbVPoles(); ++j)
      if (p->Pole(i, j).Distance(q->Pole(i, j)) > tol || std::abs(p->Weight(i, j) - q->Weight(i, j)) > 1e-12)
        return false;
  return true;
}

occ::handle<Geom_Surface> untrimmed(occ::handle<Geom_Surface> s)
{
  while (const auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
    s = t->BasisSurface();
  return s;
}

// A cylinder, cone, sphere or torus a face is on through a trim (a STEP reader's): the whole surface, else null.
occ::handle<Geom_Surface> round_under_trim(const occ::handle<Geom_Surface>& s)
{
  const occ::handle<Geom_Surface> b = untrimmed(s);
  return b != s && occ::down_cast<Geom_ElementarySurface>(b) && !occ::down_cast<Geom_Plane>(b) ? b : occ::handle<Geom_Surface>();
}

// Adjacent faces on copies of one B-spline surface put on one of them (UnifySameDomain merges B-spline faces only on
// one surface), and faces on a trimmed cylinder, cone, sphere or torus on the whole one (merge_cylinder_bands takes
// only those): each face's pcurves registered on it, an edge where two pieces meet across the surface's period a seam
// with both. In place: `shape` is a copy of the part's.
bool share_bspline_surfaces(const TopoDS_Shape& shape)
{
  ShapeIMap fmap;
  TopExp::MapShapes(shape, TopAbs_FACE, fmap);
  ShapeAncMap ef;
  TopExp::MapShapesAndAncestors(shape, TopAbs_EDGE, TopAbs_FACE, ef);
  std::vector<int> parent(fmap.Extent() + 1);
  std::iota(parent.begin(), parent.end(), 0);
  std::function<int(int)> root = [&](int x) { return parent[x] == x ? x : parent[x] = root(parent[x]); };
  auto surface = [](const TopoDS_Shape& f, TopLoc_Location& l) { return BRep_Tool::Surface(TopoDS::Face(f), l); };
  bool any = false;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const auto& fs = ef(i);
    if (fs.Extent() != 2 || fs.First().IsSame(fs.Last()))
      continue;
    TopLoc_Location la, lb;
    const auto      ga = surface(fs.First(), la), gb = surface(fs.Last(), lb);
    if (ga == gb || !la.IsEqual(lb) || !same_bspline_surface(ga, gb, 1e-9))
      continue;
    parent[root(fmap.FindIndex(fs.Last()))] = root(fmap.FindIndex(fs.First()));
    any = true;
  }
  std::vector<bool> trimmed(parent.size(), false);
  for (int i = 1; i <= fmap.Extent(); ++i)
  {
    TopLoc_Location l;
    trimmed[(size_t)i] = !round_under_trim(surface(fmap(i), l)).IsNull();
    any                = any || trimmed[(size_t)i];
  }
  if (!any)
    return false;
  BRep_Builder b;
  // Per edge, the pcurve already on the group's surface and the edge's orientation in that face.
  std::map<std::pair<const TopoDS_TShape*, const Geom_Surface*>, std::pair<occ::handle<Geom2d_Curve>, TopAbs_Orientation>> on;
  std::vector<int> size(parent.size(), 0), order;
  for (int i = 1; i <= fmap.Extent(); ++i)
    ++size[(size_t)root(i)];
  // Each group's own face first (its pcurves are on the surface already).
  for (int pass = 0; pass < 2; ++pass)
    for (int i = 1; i <= fmap.Extent(); ++i)
      if ((size[(size_t)root(i)] > 1 || trimmed[(size_t)i]) && (root(i) == i) == (pass == 0))
        order.push_back(i);
  for (const int i : order)
  {
    const int r = root(i);
    TopLoc_Location lr, lf;
    occ::handle<Geom_Surface> rep = surface(fmap(r), lr);
    // (On its own copy of the whole surface: a neighbour on the same one may run its curves from another start.)
    if (const auto whole = round_under_trim(rep); size[(size_t)r] == 1 && !whole.IsNull())
      rep = occ::down_cast<Geom_Surface>(whole->Copy());
    // (Forward: a seam's two curves read as the edge's orientation in it says, as they're put back.)
    const TopoDS_Face               f   = TopoDS::Face(fmap(i).Oriented(TopAbs_FORWARD));
    const occ::handle<Geom_Surface> own = surface(f, lf);
    // This face's pcurves on its own surface, read before it changes.
    std::vector<std::tuple<TopoDS_Edge, occ::handle<Geom2d_Curve>, occ::handle<Geom2d_Curve>, double, double>> curves;
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
    {
      const TopoDS_Edge ed = TopoDS::Edge(e.Current());
      double            f0, f1;
      const auto        c  = BRep_Tool::CurveOnSurface(ed, f, f0, f1);
      const auto        c2 = BRep_Tool::IsClosed(ed, f) ? BRep_Tool::CurveOnSurface(TopoDS::Edge(ed.Reversed()), f, f0, f1) : occ::handle<Geom2d_Curve>();
      if (c.IsNull())
        return false;
      curves.emplace_back(ed, c, c2, f0, f1);
    }
    if (own != rep)
      b.UpdateFace(f, rep, lr, BRep_Tool::Tolerance(f));
    for (const auto& [ed, c, c2, f0, f1] : curves)
    {
      const double tol = BRep_Tool::Tolerance(ed);
      if (!c2.IsNull())
      {
        // A seam of this face already: both on the group's surface.
        b.UpdateEdge(ed, ed.Orientation() == TopAbs_FORWARD ? c : c2, ed.Orientation() == TopAbs_FORWARD ? c2 : c, rep, lr, tol);
        b.Range(ed, rep, lr, f0, f1);
        continue;
      }
      const auto key = std::make_pair(ed.TShape().get(), rep.get());
      auto       it  = on.find(key);
      if (it == on.end())
      {
        // (With its own range: a STEP edge's curve on a face needn't run over its 3D curve's.)
        b.UpdateEdge(ed, c, rep, lr, tol);
        b.Range(ed, rep, lr, f0, f1);
        on[key] = {c, ed.Orientation()};
        continue;
      }
      // Met before from another piece: the same place in (u, v), or across the period (a seam of the merged face).
      const auto& [other, oo] = it->second;
      const double m          = (f0 + f1) / 2;
      if (other->Value(m).Distance(c->Value(m)) <= 1e-9)
        continue;
      const bool first = oo == TopAbs_FORWARD;
      b.UpdateEdge(ed, first ? other : c, first ? c : other, rep, lr, tol);
    }
  }
  return true;
}

bool same_surface(const TopoDS_Face& a, const TopoDS_Face& b, bool periodic)
{
  if (periodic)
  {
    // One surface (pieces of one B-spline put on it: share_bspline_surfaces).
    TopLoc_Location la, lb;
    const auto      ga = BRep_Tool::Surface(a, la), gb = BRep_Tool::Surface(b, lb);
    if (!ga.IsNull() && ga == gb && la.IsEqual(lb))
      return true;
  }
  const BRepAdaptor_Surface sa(a, false), sb(b, false);
  if (sa.GetType() != sb.GetType() || (!periodic && sa.GetType() != GeomAbs_Plane))
    return false;
  constexpr double tol = 1e-7, ang = 1e-9;
  switch (sa.GetType())
  {
    case GeomAbs_Plane:
    {
      const gp_Pln p = sa.Plane(), q = sb.Plane();
      return p.Axis().Direction().IsParallel(q.Axis().Direction(), ang) && p.Distance(q.Location()) < tol;
    }
    case GeomAbs_Cylinder:
    {
      const gp_Cylinder p = sa.Cylinder(), q = sb.Cylinder();
      return std::abs(p.Radius() - q.Radius()) < tol && p.Axis().IsCoaxial(q.Axis(), ang, tol);
    }
    case GeomAbs_Cone:
    {
      const gp_Cone p = sa.Cone(), q = sb.Cone();
      return p.Apex().Distance(q.Apex()) < tol && std::abs(p.SemiAngle() - q.SemiAngle()) < ang && p.Axis().Direction().IsParallel(q.Axis().Direction(), ang);
    }
    case GeomAbs_Sphere:
      return sa.Sphere().Location().Distance(sb.Sphere().Location()) < tol && std::abs(sa.Sphere().Radius() - sb.Sphere().Radius()) < tol;
    case GeomAbs_Torus:
    {
      const gp_Torus p = sa.Torus(), q = sb.Torus();
      return p.Axis().IsCoaxial(q.Axis(), ang, tol) && p.Location().Distance(q.Location()) < tol && std::abs(p.MajorRadius() - q.MajorRadius()) < tol &&
             std::abs(p.MinorRadius() - q.MinorRadius()) < tol;
    }
    default:
      return false;
  }
}
} // namespace

// Every face on a cylinder, cone, sphere or torus bounded, its seams left out, by loops that close (each vertex begins
// as many of its edges as it ends): a Parasolid face has no seams (a cylinder merged across a thread's helix pieces is
// bounded through its seam: NIST's cap screw).
bool closes_without_seams(const TopoDS_Shape& s)
{
  for (TopExp_Explorer fx(s, TopAbs_FACE); fx.More(); fx.Next())
  {
    const TopoDS_Face& f    = TopoDS::Face(fx.Current());
    const auto         type = BRepAdaptor_Surface(f, false).GetType();
    if (type != GeomAbs_Cylinder && type != GeomAbs_Cone && type != GeomAbs_Sphere && type != GeomAbs_Torus)
      continue;
    NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher> balance;
    for (TopExp_Explorer ee(f, TopAbs_EDGE); ee.More(); ee.Next())
    {
      const TopoDS_Edge& e = TopoDS::Edge(ee.Current());
      if (BRep_Tool::Degenerated(e) || BRep_Tool::IsClosed(e, f))
        continue;
      const TopoDS_Vertex a = TopExp::FirstVertex(e, true), b = TopExp::LastVertex(e, true);
      if (a.IsNull() || b.IsNull())
        return false;
      if (!balance.IsBound(a))
        balance.Bind(a, 0);
      if (!balance.IsBound(b))
        balance.Bind(b, 0);
      ++balance.ChangeFind(a);
      --balance.ChangeFind(b);
    }
    for (NCollection_DataMap<TopoDS_Shape, int, TopTools_ShapeMapHasher>::Iterator it(balance); it.More(); it.Next())
      if (it.Value() != 0)
        return false;
  }
  return true;
}

MergedFaces merge_same_surface_faces(const OcShape& s, bool periodic)
{
  require_non_null(s.shape, "shape");
  const ShapeIMap& fm = s.map(TopAbs_FACE);
  auto             as_is = [&] {
    MergedFaces out;
    out.shape = wrap(s.shape);
    for (int i = 0; i < fm.Extent(); ++i)
      out.face_map.push_back((uint32_t)i);
    return out;
  };
  // Revolved faces as the surfaces they are (periodic: IGES); the faces' map goes through it.
  TopoDS_Shape                        shape     = s.shape;
  occ::handle<RevolutionToElementary> rev       = new RevolutionToElementary;
  BRepTools_Modifier                  mod;
  bool                                converted = false;
  if (periodic)
  {
    bool any_rev = false;
    for (int i = 1; i <= fm.Extent() && !any_rev; ++i)
      any_rev = RevolutionToElementary::revolution(BRep_Tool::Surface(TopoDS::Face(fm(i))));
    if (any_rev)
    {
      mod.Init(shape);
      mod.Perform(rev);
      if (mod.IsDone() && BRepCheck_Analyzer(mod.ModifiedShape(shape)).IsValid())
      {
        shape     = mod.ModifiedShape(shape);
        converted = true;
      }
    }
  }
  // B-spline faces on analytic surfaces (IGES too).
  const occ::handle<BRepTools_ReShape> reshape = new BRepTools_ReShape;
  bool                                 rebuilt = false;
  if (periodic)
  {
    const TopoDS_Shape r = bspline_faces_analytic(shape, 1e-7, reshape);
    if (!r.IsSame(shape) && BRepCheck_Analyzer(r).IsValid())
    {
      shape   = r;
      rebuilt = true;
    }
  }
  // Pieces of one B-spline surface on one (on a copy: the part as read stays as it is).
  std::unique_ptr<BRepBuilderAPI_Copy> copy;
  if (periodic)
  {
    auto c = std::make_unique<BRepBuilderAPI_Copy>(shape, false);
    if (share_bspline_surfaces(c->Shape()) && BRepCheck_Analyzer(c->Shape()).IsValid())
    {
      shape = c->Shape();
      copy  = std::move(c);
    }
  }
  auto converted_face = [&](const TopoDS_Shape& f) {
    TopoDS_Shape g = converted ? mod.ModifiedShape(f) : f;
    g              = rebuilt ? reshape->Value(g) : g;
    return copy ? copy->Modified(g).First() : g;
  };
  ShapeAncMap ef;
  TopExp::MapShapesAndAncestors(shape, TopAbs_EDGE, TopAbs_FACE, ef);
  // Only across edges between faces on one analytic surface: the others kept (a healed thread's B-spline faces merged
  // into one whose boundary doesn't close).
  bool                                                   any = false;
  NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> keep;
  for (int i = 1; i <= ef.Extent(); ++i)
  {
    const auto& faces = ef(i);
    const bool  merge = faces.Extent() == 2 && !faces.First().IsSame(faces.Last()) && same_surface(TopoDS::Face(faces.First()), TopoDS::Face(faces.Last()), periodic);
    any               = any || merge;
    if (!merge)
      keep.Add(ef.FindKey(i));
  }
  if (!any && !converted && !rebuilt && !copy)
    return as_is();
  ShapeUpgrade_UnifySameDomain u(shape, true, true, false);
  u.KeepShapes(keep);
  keep_marked_edges(u, shape);
  if (any)
  {
    u.Build();
    carry_kept_marks(shape, u);
  }
  // (A merge that leaves a face it can't close: the part as read.)
  const TopoDS_Shape merged = any ? u.Shape() : shape;
  if (!BRepCheck_Analyzer(merged).IsValid())
    return as_is();
  // A whole torus or sphere merged into a face with no boundary at all: made again with its seams (the volume reads
  // nothing without them), the solid turned outward.
  const occ::handle<BRepTools_ReShape> whole  = new BRepTools_ReShape;
  bool                                 remade = false;
  for (TopExp_Explorer e(merged, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    if (TopoDS_Iterator(f).More() || whole->IsRecorded(f))
      continue;
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> g = BRep_Tool::Surface(f, loc);
    BRepBuilderAPI_MakeFace         mf(g, BRep_Tool::Tolerance(f));
    if (!mf.IsDone())
      return as_is();
    TopoDS_Face nf = mf.Face();
    nf.Location(loc);
    nf.Orientation(f.Orientation());
    whole->Replace(f, nf);
    remade = true;
  }
  TopoDS_Shape result = merged;
  if (remade)
  {
    result = whole->Apply(result);
    for (TopExp_Explorer e(result, TopAbs_SOLID); e.More(); e.Next())
    {
      TopoDS_Solid so = TopoDS::Solid(e.Current());
      BRepLib::OrientClosedSolid(so);
    }
    if (!BRepCheck_Analyzer(result).IsValid())
      return as_is();
  }
  // A loop the merged faces' old seam left in two pieces of one curve: one edge again (c2-fan-blade-sweep's STEP: 120
  // edges for live's 119).
  HHist joined;
  if (periodic && any)
  {
    const TopoDS_Shape j = join_loop_pieces(result, joined);
    if (!j.IsSame(result) && BRepCheck_Analyzer(j).IsValid())
      result = j;
    else
      joined.Nullify();
  }
  // Pieces whose curves on the faces run over other ranges than their 3D curve (a STEP edge's needn't be the same:
  // join_loop_pieces leaves those): on a copy whose edges take their curve's range, joined if that comes out valid
  // with fewer edges (c2-fan-blade-sweep's blade, 120 edges for live's 119). `ranged_face`: a face's copy and join.
  NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher> ranged_face;
  if (periodic && any)
  {
    BRepBuilderAPI_Copy ranged(result, true, false);
    for (TopExp_Explorer e(ranged.Shape(), TopAbs_EDGE); e.More(); e.Next())
    {
      const TopoDS_Edge& x = TopoDS::Edge(e.Current());
      if (BRep_Tool::SameRange(x) && BRep_Tool::SameParameter(x))
        continue;
      BRepLib::SameRange(x);
      BRepLib::SameParameter(x, Precision::Confusion());
    }
    HHist              again;
    const TopoDS_Shape j = join_loop_pieces(ranged.Shape(), again, true);
    ShapeIMap          before, after;
    TopExp::MapShapes(result, TopAbs_EDGE, before);
    TopExp::MapShapes(j, TopAbs_EDGE, after);
    if (!j.IsSame(ranged.Shape()) && after.Extent() < before.Extent() && BRepCheck_Analyzer(j).IsValid())
    {
      for (TopExp_Explorer f(result, TopAbs_FACE); f.More(); f.Next())
      {
        TopoDS_Shape x = ranged.ModifiedShape(f.Current());
        if (!again.IsNull() && !again->Modified(x).IsEmpty())
          x = again->Modified(x).First();
        ranged_face.Bind(f.Current(), x);
      }
      result = j;
    }
  }
  // Two bands of one cylinder meeting at a full ring, their seams apart (a STEP writer's pieces of one face: the PEM
  // FH4 stud's shank, c2-gamma-imported): one face, as live's.
  HHist bands;
  if (periodic)
  {
    const TopoDS_Shape b = merge_cylinder_bands(result, bands);
    if (!b.IsSame(result))
      result = b;
    else
      bands.Nullify();
  }
  // A merged periodic face Parasolid couldn't bound: the part merged across planes only.
  if (periodic && !closes_without_seams(result) && closes_without_seams(s.shape))
    return merge_same_surface_faces(s, false);
  MergedFaces out;
  out.shape = wrap(result);
  ShapeIMap nm;
  TopExp::MapShapes(result, TopAbs_FACE, nm);
  const occ::handle<BRepTools_History> h = any ? u.History() : occ::handle<BRepTools_History>();
  for (int i = 1; i <= fm.Extent(); ++i)
  {
    TopoDS_Shape f = converted_face(fm(i));
    if (!h.IsNull() && !h->Modified(f).IsEmpty())
      f = h->Modified(f).First();
    f = whole->Value(f);
    if (!joined.IsNull() && !joined->Modified(f).IsEmpty())
      f = joined->Modified(f).First();
    if (const TopoDS_Shape* r = ranged_face.Seek(f))
      f = *r;
    if (!bands.IsNull() && !bands->Modified(f).IsEmpty())
      f = bands->Modified(f).First();
    const int k = nm.FindIndex(f);
    out.face_map.push_back(k > 0 ? (uint32_t)(k - 1) : UINT32_MAX);
  }
  return out;
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
  dump_op("unify", s.shape, {}, sc != nullptr ? "unify scoped" : "unify whole");
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

// A transformed copy that makes each surface the original's faces share once (OCCT's makes one per face: a mirrored
// copy's seam pieces of one surface no longer read as one face, c2-fan-blade-sweep's Mirror 3).
class SharedSurfaceTrsf : public BRepTools_TrsfModification
{
public:
  explicit SharedSurfaceTrsf(const gp_Trsf& t)
      : BRepTools_TrsfModification(t)
  {
  }

  bool NewSurface(const TopoDS_Face& F, occ::handle<Geom_Surface>& S, TopLoc_Location& L, double& Tol, bool& RevWires, bool& RevFace) override
  {
    if (!BRepTools_TrsfModification::NewSurface(F, S, L, Tol, RevWires, RevFace))
      return false;
    TopLoc_Location                  fl;
    const occ::handle<Geom_Surface>& orig = BRep_Tool::Surface(F, fl);
    for (const Made& x : myMade[orig.get()])
      if (x.loc.IsEqual(fl))
      {
        S = x.surface;
        return true;
      }
    myMade[orig.get()].push_back({fl, S, orig});
    return true;
  }

private:
  struct Made
  {
    TopLoc_Location           loc;
    occ::handle<Geom_Surface> surface;
    occ::handle<Geom_Surface> orig; // (kept alive: its address is the key)
  };
  std::unordered_map<const Geom_Surface*, std::vector<Made>> myMade;
};

class SharedSurfaceTransform : public BRepBuilderAPI_ModifyShape
{
public:
  SharedSurfaceTransform(const TopoDS_Shape& s, const gp_Trsf& t)
  {
    myModification = new SharedSurfaceTrsf(t);
    DoModif(s);
  }
};

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
  SharedSurfaceTransform tr(base, m);
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
// set_section_work's: the steps of work a section may take (0: no limit), and the last such section's.
thread_local uint64_t t_section_work = 0;
thread_local uint64_t t_last_section_work = 0;
} // namespace

double set_call_budget(double seconds)
{
  const double was = t_call_budget;
  t_call_budget    = seconds > 0 ? seconds : 0;
  return was;
}

uint64_t set_section_work(uint64_t steps)
{
  const uint64_t was = t_section_work;
  t_section_work     = steps;
  return was;
}

uint64_t last_section_work() { return t_last_section_work; }

static OpOut op_section_unbudgeted(const OcShape& a, const OcShape& b, bool parallel = true)
{
  require_non_null(a.shape, "first shape");
  require_non_null(b.shape, "second shape");
  BRepAlgoAPI_Section sec(a.shape, b.shape, false);
  sec.Approximation(true);
  sec.ComputePCurveOn1(true);
  sec.ComputePCurveOn2(true);
  sec.SetRunParallel(parallel && bop_parallel());
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
  // Under set_section_work's bound: serial (every step on this thread, counted), stopped once past it. The same
  // answer on a fast or a busy machine, unlike a CPU budget (core's DRAFT-NONDETERMINISTIC sweep).
  if (t_section_work > 0)
  {
    WorkBudget        w;
    WorkBudget* const outer = t_work;
    w.limit                 = t_section_work;
    t_work                  = &w;
    const int pairs         = noshape_bop_set_pairs_parallel_here(0);
    const auto restore      = [&] {
      noshape_bop_set_pairs_parallel_here(pairs);
      t_work              = outer;
      t_last_section_work = w.used;
    };
    OpOut out;
    try
    {
      out = op_section_unbudgeted(a, b, false);
    }
    catch (...)
    {
      restore();
      if (w.hit)
        fail_op("section took too much work");
      throw;
    }
    restore();
    if (w.hit)
      fail_op("section took too much work");
    return out;
  }
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
