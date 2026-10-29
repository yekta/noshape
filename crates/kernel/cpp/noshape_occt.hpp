// noshape-kernel C++ bridge to OpenCascade (OCCT 8).
//
// Every function exposed to Rust is declared here and implemented in one of the
// cpp/*.cpp files. All bridge functions are declared as returning `Result` on
// the Rust side, so cxx wraps each call in `rust::behavior::trycatch` (see
// below), which converts any C++ exception (Standard_Failure, std::exception,
// or anything else) into a Rust `Err` carrying the message.
#pragma once

#include "rust/cxx.h"

#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <Standard_Failure.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_List.hxx>
#include <TopoDS_Shape.hxx>

class IntCurvesFace_ShapeIntersector; // geom.cpp line_crossings
class BRepClass3d_SolidClassifier;    // geom.cpp classify_point
namespace noshape {
class RayClassifier; // internal.hpp, classify_point
struct UnifyScope;   // internal.hpp, the unify
}

#include <chrono>
#include <typeinfo>
#include <array>
#include <map>
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <exception>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>

// ---------------------------------------------------------------------------
// Exception -> Rust error conversion (overrides cxx's default which only
// catches std::exception). OCCT 8's Standard_Failure derives from
// std::exception, but we handle it first to include the exception type name.
// ---------------------------------------------------------------------------
#ifndef _WIN32
  #include <csetjmp>
  #include <csignal>
namespace noshape {
// Crash guard (qa DRAFT-SEGV-DOC / DOC-POISON): a SIGSEGV, SIGBUS, SIGFPE,
// SIGILL or SIGABRT (std::terminate: OCCT 8 throws from noexcept code, e.g.
// gp_Dir of a zero vector) raised on a thread while it runs a bridge call
// jumps back to that
// call, which fails with an error, so one bad feature fails instead of
// killing the process (and the document stays openable). Signals anywhere
// else go to the handler that was there before (Rust's). OCCT's own
// OSD::SetSignal isn't used: it also takes SIGINT and friends.
extern thread_local sigjmp_buf* crash_guard;
extern thread_local int         crash_signal;
void                            install_crash_guard();
} // namespace noshape
#endif
#if defined(_WIN32) && defined(_MSC_VER)
namespace noshape {
// Windows crash guard (cpp/seh.cpp, WIN-CRASH-GUARD): runs fn(ctx) under
// __try / __except; 0, or the code of the structured exception (an access
// violation, a division by zero, std::terminate...) that ended it.
unsigned long seh_call(void (*fn)(void*), void* ctx) noexcept;
template <class F>
unsigned long seh_run(F& f) noexcept
{
  return seh_call([](void* c) { (*static_cast<F*>(c))(); }, &f);
}
} // namespace noshape
#endif

namespace noshape {
// The calling thread's regeneration cancel flag (set_thread_cancel), or null.
const std::atomic<bool>* thread_cancel_flag();
// Budget of a blend (qa FILLET-HANG: OCCT's blend approximation never
// returned): `f` runs with a CPU-time timer on this thread (Linux) that
// ticks every 100 ms; once `cpu_seconds` are used or the regeneration is
// cancelled, the tick jumps back here. 0 done, 1 over budget, 2 cancelled.
// Elsewhere `f` just runs.
#ifdef __linux__
extern thread_local sigjmp_buf*             budget_jmp;
extern thread_local double                  budget_limit;
extern thread_local int                     budget_reason;
extern thread_local const std::atomic<bool>* budget_cancel;
extern thread_local int                     budget_hold;
// The running budget's timer (run_budgeted's), for the crash guard to stop
// when it recovers past that budget's frame.
extern thread_local void*                   budget_timer_live;
// The budget as a crash guard found it; restored when the guard recovers
// from a fault past run_budgeted's frame (qa CLI-SUPPRESS-SEGV: its jump
// buffer and timer outlived it, and the next tick or budgeted op jumped into
// the dead frame).
void budget_timer_stop(void* timer);
struct BudgetSnapshot
{
  sigjmp_buf* jmp   = budget_jmp;
  void*       timer = budget_timer_live;
  void        restore() const
  {
    if (budget_timer_live != timer && budget_timer_live != nullptr)
      budget_timer_stop(budget_timer_live);
    budget_timer_live = timer;
    budget_jmp        = jmp;
  }
};
bool                                        budget_timer_start(void*& timer);
void                                        budget_timer_stop(void* timer);
double                                      thread_cpu_seconds();
#endif
// CPU seconds a blend, draft or shell may take (30; NOSHAPE_BLEND_BUDGET).
double blend_budget_seconds();
// While one lives on a budgeted thread the budget doesn't jump (it does at
// the next tick after): around work shared with OCCT's thread pool, whose
// workers would go on using the abandoned stack frames. Linux only (the
// budget's timer); a no-op elsewhere.
struct BudgetHold
{
#ifdef __linux__
  BudgetHold() { ++budget_hold; std::atomic_signal_fence(std::memory_order_seq_cst); }
  ~BudgetHold() { std::atomic_signal_fence(std::memory_order_seq_cst); --budget_hold; }
#else
  BudgetHold() {}
#endif
  BudgetHold(const BudgetHold&)            = delete;
  BudgetHold& operator=(const BudgetHold&) = delete;
};
template <class F>
int run_budgeted(double cpu_seconds, F&& f)
{
#ifdef __linux__
  if (budget_jmp != nullptr)
  {
    f();
    return 0;
  }
  void* timer = nullptr;
  if (!budget_timer_start(timer))
  {
    f();
    return 0;
  }
  sigjmp_buf        env;
  sigjmp_buf* const crash_outer = crash_guard;
  budget_timer_live             = timer;
  if (sigsetjmp(env, 1) != 0)
  {
    budget_timer_stop(timer);
    budget_timer_live = nullptr;
    crash_guard       = crash_outer;
    return budget_reason;
  }
  budget_limit  = thread_cpu_seconds() + cpu_seconds;
  budget_cancel = thread_cancel_flag();
  budget_jmp    = &env;
  // Seen by the signal handler before `f` runs (else a store before an
  // endless loop can be dropped).
  std::atomic_signal_fence(std::memory_order_seq_cst);
  try
  {
    f();
  }
  catch (...)
  {
    budget_jmp = nullptr;
    budget_timer_stop(timer);
    budget_timer_live = nullptr;
    throw;
  }
  budget_jmp = nullptr;
  budget_timer_stop(timer);
  budget_timer_live = nullptr;
#else
  (void)cpu_seconds;
  f();
#endif
  return 0;
}

bool kernel_profile_on();
// The profile's clock: wall time, or this thread's CPU time with
// NOSHAPE_KERNEL_PROFILE=cpu (comparisons on a loaded machine; Linux), =proc for the process (pool threads too).
std::chrono::steady_clock::duration kernel_profile_now();
void kernel_profile_add(const char* what, std::chrono::steady_clock::duration took);
void kernel_profile_add_named(const char* name, std::chrono::steady_clock::duration took);
// A step inside an op in the NOSHAPE_KERNEL_PROFILE table ("op.step").
struct KernelTimer
{
  const char*                           what;
  std::chrono::steady_clock::duration    t0;
  explicit KernelTimer(const char* w)
      : what(w),
        t0(kernel_profile_on() ? kernel_profile_now() : std::chrono::steady_clock::duration{})
  {
  }
  ~KernelTimer()
  {
    if (kernel_profile_on())
      kernel_profile_add_named(what, kernel_profile_now() - t0);
  }
};

// Runs `f` under the crash guard: false when it throws or crashes (a
// fallback inside an op that may crash OCCT, so the op can go on).
template <class F>
bool run_guarded(F&& f) noexcept
{
  bool ok      = true;
  auto guarded = [&] {
    try
    {
      f();
    }
    catch (...)
    {
      ok = false;
    }
  };
#if defined(_WIN32) && defined(_MSC_VER)
  return seh_run(guarded) == 0 && ok;
#elif defined(_WIN32)
  guarded();
  return ok;
#else
  install_crash_guard();
  sigjmp_buf        env;
  sigjmp_buf* const outer = crash_guard;
#ifdef __linux__
  const BudgetSnapshot budget;
#endif
  if (sigsetjmp(env, 1) != 0)
  {
    crash_guard = outer;
#ifdef __linux__
    budget.restore();
#endif
    return false;
  }
  crash_guard = &env;
  std::atomic_signal_fence(std::memory_order_seq_cst);
  guarded();
  crash_guard = outer;
  return ok;
#endif
}
} // namespace noshape

namespace rust {
namespace behavior {
template <typename Try, typename Fail>
static void trycatch_guarded(Try&& func, Fail&& fail) noexcept
try
{
  func();
}
catch (const Standard_Failure& e)
{
  std::string msg = "occt: ";
  msg += e.ExceptionType();
  const char* w = e.what();
  if (w != nullptr && *w != '\0')
  {
    msg += ": ";
    msg += w;
  }
  fail(msg);
}
catch (const std::exception& e)
{
  const char* w = e.what();
  fail(w != nullptr ? w : "unknown std::exception");
}
catch (...)
{
  fail("occt: unknown C++ exception");
}

template <typename Try, typename Fail>
static void trycatch_timed(Try&& func, Fail&& fail) noexcept;

template <typename Try, typename Fail>
static void trycatch(Try&& func, Fail&& fail) noexcept
{
  // NOSHAPE_KERNEL_PROFILE: time every bridge call by function (printed at
  // exit, common.cpp).
  if (noshape::kernel_profile_on())
  {
    const auto t0 = noshape::kernel_profile_now();
    trycatch_timed(func, fail);
    noshape::kernel_profile_add(typeid(Try).name(), noshape::kernel_profile_now() - t0);
    return;
  }
  trycatch_timed(func, fail);
}

template <typename Try, typename Fail>
static void trycatch_timed(Try&& func, Fail&& fail) noexcept
{
#if defined(_WIN32) && defined(_MSC_VER)
  auto                guarded = [&] { trycatch_guarded(func, fail); };
  const unsigned long code    = noshape::seh_run(guarded);
  if (code != 0)
  {
    char hex[16];
    std::snprintf(hex, sizeof hex, "%08lx", code);
    fail(std::string("occt: the kernel crashed (exception 0x") + hex + ")");
  }
#elif defined(_WIN32)
  trycatch_guarded(func, fail);
#else
  noshape::install_crash_guard();
  sigjmp_buf        env;
  sigjmp_buf* const outer = noshape::crash_guard;
#ifdef __linux__
  const noshape::BudgetSnapshot budget;
#endif
  if (sigsetjmp(env, 1) != 0)
  {
    noshape::crash_guard = outer;
#ifdef __linux__
    budget.restore();
#endif
    std::string msg = "occt: the kernel crashed (signal ";
    msg += std::to_string(noshape::crash_signal);
    msg += ")";
    fail(msg);
    return;
  }
  noshape::crash_guard = &env;
  std::atomic_signal_fence(std::memory_order_seq_cst);
  trycatch_guarded(func, fail);
  noshape::crash_guard = outer;
#endif
}
} // namespace behavior
} // namespace rust

namespace noshape {

using ShapeIMap = NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher>;
using ShapeList = NCollection_List<TopoDS_Shape>;
using ShapeAncMap =
  NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<TopoDS_Shape>, TopTools_ShapeMapHasher>;

// Error helpers: the message prefix is parsed on the Rust side into the
// corresponding `KernelError` variant.
[[noreturn]] void fail_invalid(const std::string& msg);
[[noreturn]] void fail_op(const std::string& msg);
[[noreturn]] void fail_unsupported(const std::string& msg);
[[noreturn]] void fail_io(const std::string& msg);

struct BrepBody; // a body for noshape's own Boolean (ffi.rs)

// Immutable wrapper around a TopoDS_Shape. Lazily caches the
// TopExp::MapShapes indexed maps per shape type (thread-safe via call_once).
class OcShape
{
public:
  OcShape() = default;
  explicit OcShape(const TopoDS_Shape& s)
      : shape(s)
  {
  }
  OcShape(const OcShape&)            = delete;
  OcShape& operator=(const OcShape&) = delete;

  const ShapeIMap& map(TopAbs_ShapeEnum kind) const;
  // Its faces' boxes (face_box) by map(TopAbs_FACE)
  // index - 1, computed once: the auto scope's classification, fuse_on_face
  // and the local Boolean all box the target's 1600 faces.
  const std::vector<Bnd_Box>& face_boxes() const;
  // Whether face_boxes() has been made.
  bool has_face_boxes() const { return myBoxesReady.load(); }
  // The face, edge and vertex maps / the face boxes someone made already (the
  // history of the op that made the shape; boxes of the faces it kept): taken
  // unless made here first. map() makes the face, edge and vertex maps in one
  // walk (map_fev).
  void adopt_fev(std::array<ShapeIMap, 3>&& m);
  void adopt_face_boxes(std::vector<Bnd_Box>&& b);
  // Volume properties, computed once (`compute` runs the first time):
  // regeneration asks for a body's volume before and after each boolean.
  template <class F>
  const GProp_GProps& volume_props(F&& compute) const
  {
    std::call_once(myVolumeFlag, [&] { myVolume = compute(); });
    return myVolume;
  }

  // Surface (area) properties, computed once like the volume.
  template <class F>
  const GProp_GProps& surface_props(F&& compute) const
  {
    std::call_once(mySurfaceFlag, [&] { mySurface = compute(); });
    return mySurface;
  }

  // The loaded line intersector of line_crossings (geom.cpp): building it
  // costs more than a crossing, and std's probes cast hundreds of lines at
  // one body. Its Perform keeps results inside: used under the lock.
  mutable std::mutex                                       myLineLock;
  mutable std::shared_ptr<IntCurvesFace_ShapeIntersector> myLineHits;

  // The loaded point classifier of classify_point (geom.cpp), likewise:
  // Extrude's auto scope classifies a tool's vertices against one part.
  mutable std::mutex                                    myClassifyLock;
  mutable std::shared_ptr<BRepClass3d_SolidClassifier> myClassifier;
  // Its rays for a big solid (by the tolerance they were made with).
  mutable std::pair<double, std::shared_ptr<RayClassifier>> myRays{-1.0, nullptr};

  // bounding_box's (geom.cpp) by `optimal`, made once: the regeneration
  // boxes the same part several times a feature (the auto scope, the
  // Boolean's tools, the extent).
  mutable std::mutex                   myBoxLock;
  mutable std::array<double, 6>        myBox[2]{};
  mutable bool                         myBoxMade[2]{false, false};

  // shown_topology's flags (common.cpp), made once.
  mutable std::once_flag       myShownFlag;
  mutable std::vector<uint8_t> myShown;

  // What the unify knows of it (boolean.cpp): nothing to merge in it (a
  // unify found nothing); for a local Boolean's result on such a shape,
  // where to look.
  mutable std::atomic<bool>         myUnifyClean{false};
  std::shared_ptr<const UnifyScope> myUnifyScope;

  // noshape's own Boolean's flat form of it (rust_boolean.cpp), or why it has none, made once: a chain of
  // Booleans converts each body once.
  mutable std::mutex                      myBrepLock;
  mutable std::shared_ptr<const BrepBody> myBrep;
  mutable double                          myBrepTol = 0;
  mutable const char*                     myBrepWhy = nullptr;

  // Ancestor maps (CSR offsets, values) by kind * 16 + ancestor kind,
  // computed once: regeneration asks for the same ones of a body many times.
  mutable std::mutex                                                                 myAncestorsLock;
  mutable std::map<int, std::pair<std::vector<uint32_t>, std::vector<uint32_t>>> myAncestors;

  TopoDS_Shape shape;

private:
  mutable std::once_flag myFlags[TopAbs_SHAPE + 1];
  mutable std::once_flag myFevFlag;
  mutable ShapeIMap      myMaps[TopAbs_SHAPE + 1];
  mutable std::once_flag myVolumeFlag;
  mutable GProp_GProps   myVolume;
  mutable std::once_flag mySurfaceFlag;
  mutable GProp_GProps   mySurface;
  mutable std::once_flag       myBoxesFlag;
  mutable std::vector<Bnd_Box> myBoxes;
  mutable std::atomic<bool>    myBoxesReady{false};
};

// Growable list of shapes used to pass shape arrays across the bridge.
class OcShapeList
{
public:
  std::vector<TopoDS_Shape> items;
  // The OcShapes pushed (list_push; null for items added otherwise), for
  // their caches: valid only during the call the list is passed to.
  std::vector<const OcShape*> sources;
};

struct OcMeshFaces; // per-face triangulation handles (mesh.cpp)

// Result of meshing: holds the meshed shape and pre-computed buffer sizes.
class OcMesh
{
public:
  std::shared_ptr<OcMeshFaces> faceData;
  TopoDS_Shape shape;
  ShapeIMap    faces;
  ShapeIMap    edges;
  ShapeIMap    vertices;
  size_t       nodes       = 0;
  size_t       indices     = 0;
  size_t       edge_points = 0;
  std::vector<std::vector<std::array<float, 3>>> edge_polys; // per edge
  bool                                           cleaned = false; // the shape's triangulations dropped
  size_t planar_fallbacks = 0; // planar faces BRepMesh's algorithm meshed
  size_t rust_faces       = 0; // faces noshape's own mesher meshed (mesh_rust.cpp)
};

std::unique_ptr<OcShape> wrap(const TopoDS_Shape& s);

// Global lock serializing operations that write or read triangulations stored
// on shared TShapes (BRepMesh writes into BRep_TFace).
std::mutex& mesh_mutex();

// Shared structs generated by cxx (defined in ffi.rs.h).
struct P3;
struct HistRec;
struct OpOut;
struct SurfaceInfo;
struct CurveInfo;
struct BSplineF;
struct BSplineSurfF;
struct BSpline2dF;
struct Seg2d;
struct PlaneF;
struct MeshCounts;
struct PropsF;
struct DistanceF;
struct PlaneQ;
struct Csr;
struct HlrOut;
struct StepImport;
struct AsmNodeF;
struct PvzInstance;
struct StepTree;
struct BoolOpts;
struct SweepOpts;
struct FillOpts;
struct ProjPoint;
struct DevSurf;

// ---- shapes & lists (common.cpp)
std::unique_ptr<OcShape> shape_null();
std::unique_ptr<OcShape> shape_clone(const OcShape& s);
uint8_t                  shape_type(const OcShape& s);
bool                     shape_is_null(const OcShape& s);
uint8_t                  shape_orientation(const OcShape& s);
std::unique_ptr<OcShape> shape_oriented(const OcShape& s, uint8_t orientation);
uint64_t                 shape_hash(const OcShape& s);
bool                     shape_is_same(const OcShape& a, const OcShape& b);
bool                     shape_is_equal(const OcShape& a, const OcShape& b);
bool                     shape_is_partner(const OcShape& a, const OcShape& b);
// TShape identity (equal for partners), location as 3x4 row-major, shape at the identity location.
uint64_t                 shape_partner_hash(const OcShape& s);
rust::Vec<double>        shape_location(const OcShape& s);
std::unique_ptr<OcShape> shape_unlocated(const OcShape& s);
std::unique_ptr<OcShape> op_frame_cylinders_at(const OcShape& s, P3 origin, P3 axis);
OpOut                    op_divide_closed(const OcShape& s);
uint32_t                 sub_count(const OcShape& s, uint8_t kind);
std::unique_ptr<OcShape> sub_shape(const OcShape& s, uint8_t kind, uint32_t index);
std::unique_ptr<OcShapeList> sub_shapes(const OcShape& s, uint8_t kind);
int64_t                      index_of(const OcShape& s, const OcShape& sub, uint8_t kind);
Csr                          ancestor_map(const OcShape& s, uint8_t kind, uint8_t ancestor_kind);
rust::Vec<uint32_t>          adjacent_faces(const OcShape& s, uint32_t face_index);
rust::Vec<uint8_t>           shown_topology(const OcShape& s);
std::unique_ptr<OcShapeList> children(const OcShape& s);
std::unique_ptr<OcShape>     make_compound(const OcShapeList& l);
std::unique_ptr<OcShapeList> list_new();
void                         list_push(OcShapeList& l, const OcShape& s);
size_t                       list_len(const OcShapeList& l);
std::unique_ptr<OcShape>     list_get(const OcShapeList& l, size_t i);
bool                         is_valid(const OcShape& s);
bool                         self_intersects(const OcShape& s);
std::unique_ptr<OcShape>     fix_shape(const OcShape& s, double tolerance);
rust::String                 dump_shape(const OcShape& s);

// ---- geometry queries (geom.cpp)
SurfaceInfo              surface_info(const OcShape& face);
CurveInfo                curve_info(const OcShape& edge);
BSplineF                 edge_bspline(const OcShape& edge);
BSplineSurfF             face_bspline(const OcShape& face);
P3    face_normal_at_uv(const OcShape& face, double u, double v);
P3    face_point_at_uv(const OcShape& face, double u, double v);
ProjPoint                face_project_point(const OcShape& face, P3 p);
rust::Vec<double>        face_uv_bounds(const OcShape& face);
P3    edge_point_at(const OcShape& edge, double t);
P3    edge_tangent_at(const OcShape& edge, double t);
P3    vertex_point(const OcShape& v);
rust::Vec<double>        bounding_box(const OcShape& s, bool optimal);
PlaneQ                   find_plane(const OcShape& s, double tolerance);
PropsF                   props_volume(const OcShape& s);
PropsF                   props_surface(const OcShape& s);
PropsF                   props_linear(const OcShape& s);
DistanceF                min_distance(const OcShape& a, const OcShape& b);
double                   angle_between(const OcShape& a, const OcShape& b);
int8_t                   classify_point(const OcShape& solid, P3 p, double tol);
rust::Vec<double>        line_crossings(const OcShape& s, P3 p, P3 dir, double len);

// ---- construction (build.cpp)
std::unique_ptr<OcShape> make_vertex(P3 p);
std::unique_ptr<OcShape> make_edge_2d(const PlaneF& plane, const Seg2d& seg);
std::unique_ptr<OcShape> make_wire_2d(const PlaneF& plane, rust::Slice<const Seg2d> segs);
std::unique_ptr<OcShape> make_wire(const OcShapeList& edges);
std::unique_ptr<OcShape> wire_with_edge_order(const OcShape& wire, rust::Slice<const uint32_t> order);
std::unique_ptr<OcShape> make_face_on_plane(const PlaneF& plane, const OcShape& outer,
                                            const OcShapeList& holes);
std::unique_ptr<OcShape> make_face_from_wire(const OcShape& wire, bool only_plane);
std::unique_ptr<OcShape> make_plane_face(const PlaneF& plane, double umin, double umax,
                                         double vmin, double vmax);
std::unique_ptr<OcShape> make_polyline(rust::Slice<const double> pts, bool closed);
std::unique_ptr<OcShape> make_line(P3 a, P3 b);
std::unique_ptr<OcShape> make_circle(P3 c, P3 axis,
                                     P3 xdir, double r, double a0,
                                     double a1, bool full);
std::unique_ptr<OcShape> make_arc_3pts(P3 a, P3 m,
                                       P3 b);
std::unique_ptr<OcShape> make_interpolated(rust::Slice<const double> pts,
                                           rust::Slice<const double> tangents, bool periodic);
std::unique_ptr<OcShape> make_bspline_3d(rust::Slice<const double> poles,
                                         rust::Slice<const double> weights,
                                         rust::Slice<const double> knots,
                                         rust::Slice<const int32_t> mults, int32_t degree,
                                         bool periodic);
std::unique_ptr<OcShape> make_helix(const PlaneF& frame, double radius, double pitch,
                                    double height, bool left_handed, double taper_angle);
std::unique_ptr<OcShape> offset_wire_2d(const OcShape& wire_or_face, double distance,
                                        uint8_t join, bool open_result);
std::unique_ptr<OcShape> make_box(const PlaneF& frame, double dx, double dy, double dz);
std::unique_ptr<OcShape> make_cylinder(const PlaneF& frame, double r, double h, double angle);
std::unique_ptr<OcShape> make_cone(const PlaneF& frame, double r1, double r2, double h,
                                   double angle);
std::unique_ptr<OcShape> make_sphere(const PlaneF& frame, double r, double angle);
std::unique_ptr<OcShape> make_torus(const PlaneF& frame, double r1, double r2, double angle);
std::unique_ptr<OcShape> make_wedge(const PlaneF& frame, double dx, double dy, double dz,
                                    double ltx);
std::unique_ptr<OcShapeList> edges_to_wires(const OcShape& s, double tol);

// ---- features (features.cpp)
OpOut op_prism(const OcShape& profile, P3 start, P3 end,
               double draft_angle);
OpOut op_prism_until(const OcShape& profile, P3 dir, const OcShape& until,
                     double start_offset);
OpOut op_revol(const OcShape& profile, P3 origin, P3 axis,
               double a0, double a1);
OpOut op_pipe(const OcShape& profile, const OcShape& spine);
OpOut op_pipe_shell(const OcShapeList& profiles, const OcShape& spine, const SweepOpts& opts,
                    const OcShape& aux);
OpOut op_loft(const OcShapeList& sections, bool solid, bool ruled, bool closed,
              rust::Slice<const double> start_points, rust::Slice<const double> conditions, const OcShape& path,
              int32_t path_count);
void  set_working_scale(double k);
// Raises a signal inside a bridge call (tests of the crash guard): 0 SIGSEGV, 1 SIGFPE,
// 2 std::terminate, 3 a loop under a 0.3 s budget (Linux).
void  crash_inside_kernel(uint8_t kind);
OpOut op_thicken(const OcShape& s, double t1, double t2);
OpOut op_shell(const OcShape& solid, const OcShapeList& faces, double thickness, uint8_t join);
OpOut op_shell_solid(const OcShape& solid, const OcShapeList& faces, double thickness, bool outward,
                     uint8_t method);
OpOut op_fillet(const OcShape& s, const OcShapeList& edges, rust::Slice<const double> radii,
                bool checked);
OpOut op_fillet_variable(const OcShape& s, const OcShape& edge, rust::Slice<const double> params,
                         rust::Slice<const double> radii, bool checked);
OpOut op_chamfer(const OcShape& s, const OcShapeList& edges, const OcShapeList& faces,
                 uint8_t mode, double d1, double d2, bool checked);
OpOut op_draft_wedges(const OcShape& solid, const OcShapeList& faces, P3 pull, const PlaneF& neutral,
                      double angle);
std::unique_ptr<OcShape> loft_guided_curved(const OcShape& profile_a, const OcShape& profile_b, const OcShapeList& guides,
                                            bool make_solid);
std::unique_ptr<OcShape> loft_guided_open(const OcShape& profile_a, const OcShape& profile_b, const OcShapeList& guides,
                                          bool trim_guides, bool trim_profiles);
OpOut op_draft_parting_curve(const OcShape& solid, const OcShapeList& faces, const OcShapeList& edges, P3 pull,
                             rust::Slice<const double> angles);
OpOut op_draft(const OcShape& s, const OcShapeList& faces, P3 dir,
               const PlaneF& neutral, double angle);
OpOut op_defeature(const OcShape& s, const OcShapeList& faces);
OpOut op_offset_faces(const OcShape& s, const OcShapeList& faces,
                      rust::Slice<const double> offsets);
OpOut op_offset_shape(const OcShape& s, double offset, uint8_t join);
OpOut op_sew(const OcShapeList& shapes, double tol, bool make_solid);
OpOut op_fill(const OcShapeList& edges, const OcShapeList& supports,
              rust::Slice<const int32_t> orders, rust::Slice<const double> points,
              const OcShapeList& guides);
OpOut op_rib(const OcShape& base, const OcShape& wire, const PlaneF& plane,
             P3 d1, P3 d2, bool fuse);

// ---- booleans, split, transform, section (boolean.cpp)
void                     set_memory_budget_mb(uint64_t mb);
void                     set_bop_parallel(bool on);
rust::String             last_unify_path();
OpOut                    op_unify_whole(const OcShape& s);
rust::String             last_boolean_path();
void                     kernel_profile_mark(rust::Str label);
void                     set_occt_threads(int32_t n);
OpOut op_boolean(uint8_t op, const OcShapeList& args, const OcShapeList& tools,
                 const BoolOpts& opts);
OpOut op_split(const OcShapeList& args, const OcShapeList& tools, const BoolOpts& opts);
// Cancel flag of this thread's booleans (address of a Rust AtomicBool, 0 = none).
void set_thread_cancel(size_t flag);
OpOut op_unify(const OcShape& s);
OpOut op_enclose(const OcShapeList& shapes);
OpOut op_transform(const OcShape& s, rust::Slice<const double> m, bool copy);
OpOut op_gtransform(const OcShape& s, rust::Slice<const double> m);
OpOut op_section(const OcShape& a, const OcShape& b);
std::unique_ptr<OcShape> section_plane(const OcShape& s, const PlaneF& plane);
std::unique_ptr<OcShape> section_caps(const OcShape& s, const PlaneF& plane);
std::unique_ptr<OcShape> project_along(const OcShape& curves, const OcShape& target,
                                       P3 dir);
std::unique_ptr<OcShape> project_normal(const OcShape& curves, const OcShape& target);

// ---- tessellation (mesh.cpp)
std::unique_ptr<OcMesh> mesh_prepare(const OcShape& s, double lin, double ang, bool relative,
                                     bool parallel);
MeshCounts              mesh_counts(const OcMesh& m);
void mesh_fill(const OcMesh& m, rust::Slice<float> positions, rust::Slice<float> normals,
               rust::Slice<uint32_t> indices, rust::Slice<uint32_t> face_ranges,
               rust::Slice<float> edge_points, rust::Slice<uint32_t> edge_ranges,
               rust::Slice<float> vertices);

// ---- I/O (io.cpp)
void                     write_brep_file(const OcShape& s, rust::Str path);
std::unique_ptr<OcShape> read_brep_file(rust::Str path);
rust::Vec<uint8_t>       brep_to_bytes(const OcShape& s);
std::unique_ptr<OcShape> brep_from_bytes(rust::Slice<const uint8_t> bytes);
rust::Vec<uint8_t>       bin_to_bytes(const OcShape& s, bool with_triangles);
std::unique_ptr<OcShape> bin_from_bytes(rust::Slice<const uint8_t> bytes);
StepImport               read_step(rust::Str path);
StepTree                 read_step_tree(rust::Str path);
void write_step(const OcShapeList& shapes, rust::Slice<const rust::String> names,
                rust::Slice<const float> colors, rust::Slice<const bool> has_color,
                rust::Str path, uint8_t schema, uint8_t file_unit);
std::unique_ptr<OcShape> read_iges(rust::Str path);
std::unique_ptr<OcShape> read_mesh(rust::Str path, uint32_t merge_limit);
void                     write_iges(const OcShapeList& shapes, rust::Str path, rust::Str system, rust::Str version);
void write_parasolid(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Slice<const double> colours, rust::Str path, bool binary,
                     rust::Str version, rust::Str application);
void write_parasolid_assembly(const OcShapeList& parts, rust::Slice<const rust::String> names, rust::Slice<const double> colours, rust::Str root_name,
                              rust::Slice<const uint32_t> inst_part, rust::Slice<const double> transforms, rust::Str path, rust::Str version,
                              rust::Str application);
std::unique_ptr<OcShape> read_parasolid(rust::Str path);
StepImport               read_parasolid_parts(rust::Str path);
StepTree                 read_parasolid_tree(rust::Str path);
StepImport               read_rhino_parts_ffi(rust::Str path);
StepImport               read_native_parts_ffi(rust::Str path);
StepImport               read_native_parts_config_ffi(rust::Str path, int32_t config);
rust::Vec<AsmNodeF>      read_native_tree_ffi(rust::Str path, rust::Str dir);
rust::Vec<rust::String>  extract_zip_ffi(rust::Str zip, rust::Str dir);
StepImport               read_sat_parts_ffi(rust::Str path);
StepImport               read_jt_parts_ffi(rust::Str path);
void                     write_acis(const OcShapeList& shapes, rust::Str path);
void     write_rhino(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Str path, uint32_t version);
uint32_t rhino_valid_breps(rust::Str path);
void     write_jt(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Str path);
void     write_pvz(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Str path);
std::unique_ptr<OcShape> read_pvz(rust::Str path);
StepImport               read_pvz_parts(rust::Str path);
rust::Vec<PvzInstance>   read_pvz_instances(rust::Str path);
std::unique_ptr<OcShape> read_mesh_bodies(rust::Str path);
bool                     is_mesh(const OcShape& s);
bool                     has_triangulation(const OcShape& s);

// ---- HLR (hlr.cpp)
HlrOut project_hlr(const OcShape& s, P3 dir, P3 up,
                   bool exact, double deflection);

// ---- surfacing: wrap, isocline, surface fitting (surfacing.cpp)
OpOut op_wrap_faces(const OcShapeList& faces, const DevSurf& src, const DevSurf& dst,
                    rust::Slice<const double> xf, double tol);
std::unique_ptr<OcShapeList> isocline_edges(const OcShape& face, P3 dir, double angle, bool extend);
std::unique_ptr<OcShape> bspline_surface_face(const BSplineSurfF& surface, rust::Slice<const BSpline2dF> boundary, bool make_solid);
std::unique_ptr<OcShape> bspline_grid_face(const PlaneF& frame, double x0, double dx, double y0, double dy,
                                           rust::Slice<const double> z, uint32_t nx, uint32_t ny,
                                           int32_t deg_max, double tol, rust::Slice<const double> bounds);
double face_max_deviation(const OcShape& face, rust::Slice<const double> pts);

} // namespace noshape
