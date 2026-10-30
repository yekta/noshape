// Shapes, lists, topology exploration, validation and history building.
#include "internal.hpp"
#include <mutex>
#include <map>
#include <fstream>
#include <cstdlib>
#include <csignal>
#include <cstring>
#include <OSD_ThreadPool.hxx>
#ifdef __linux__
#include <dlfcn.h>
#include <execinfo.h>
#include <link.h>
#endif
#ifndef _WIN32
#include <unistd.h> // write() in the crash guard's messages (macOS doesn't get it through other headers)
#endif
#ifdef __linux__
  #include <ucontext.h>
// The program's own code (GNU linkers define these).
extern "C" char __executable_start;
extern "C" char etext;
#endif

#include <atomic>
#include <cmath>

#include <BOPAlgo_ArgumentAnalyzer.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <GProp_GProps.hxx>
#include <BRep_Tool.hxx>
#include <Poly_Triangulation.hxx>
#include <Bnd_Box.hxx>
#include <BRepGProp.hxx>
#include <array>
#include <Geom_Surface.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <Precision.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <TopoDS.hxx>
#include <BRep_Builder.hxx>
#include <ShapeFix_Shape.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <OSD_Parallel.hxx>
#include <OSD_ThreadPool.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <NCollection_IncAllocator.hxx>

#include <algorithm>
#include <sstream>

namespace noshape {

// ---------------------------------------------------------------- errors

void fail_invalid(const std::string& msg) { throw std::runtime_error("invalid: " + msg); }
void fail_op(const std::string& msg) { throw std::runtime_error("failed: " + msg); }

namespace {
std::atomic<double> g_working_scale{1.0};
}

double working_scale() { return g_working_scale.load(std::memory_order_relaxed); }

void set_working_scale(double k)
{
  if (!(k > 0) || !std::isfinite(k))
    fail_invalid("working scale must be positive");
  g_working_scale.store(k, std::memory_order_relaxed);
}
void fail_unsupported(const std::string& msg) { throw std::runtime_error("unsupported: " + msg); }
void fail_io(const std::string& msg) { throw std::runtime_error("io: " + msg); }

std::mutex& mesh_mutex()
{
  static std::mutex m;
  return m;
}

// ---------------------------------------------------------------- OcShape

static void walk_fev(const TopoDS_Shape& x, ShapeIMap& f, ShapeIMap& e, ShapeIMap& v)
{
  switch (x.ShapeType())
  {
    case TopAbs_FACE: {
      const int n = f.Extent();
      if (f.Add(x) <= n)
        return; // met before, with its sub-shapes
      break;
    }
    case TopAbs_EDGE: {
      const int n = e.Extent();
      if (e.Add(x) <= n)
        return;
      break;
    }
    case TopAbs_VERTEX:
      v.Add(x);
      return;
    default:
      break;
  }
  for (TopoDS_Iterator it(x); it.More(); it.Next())
    walk_fev(it.Value(), f, e, v);
}

void unique_ancestors(const TopoDS_Shape& s, TopAbs_ShapeEnum kind, TopAbs_ShapeEnum ancestor_kind, ShapeAncMap& m)
{
  // As TopExp::MapShapesAndUniqueAncestors (the same keys, order and lists),
  // the lists' nodes from one bump allocator: half its time was malloc.
  const occ::handle<NCollection_IncAllocator> alloc = new NCollection_IncAllocator();
  const ShapeList                             empty(alloc);
  for (TopExp_Explorer a(s, ancestor_kind); a.More(); a.Next())
  {
    const TopoDS_Shape& anc = a.Current();
    for (TopExp_Explorer x(anc, kind); x.More(); x.Next())
    {
      ShapeList& l    = m(m.Add(x.Current(), empty));
      bool       seen = false;
      for (ShapeList::Iterator it(l); it.More() && !seen; it.Next())
        seen = anc.IsSame(it.Value());
      if (!seen)
        l.Append(anc);
    }
  }
  for (TopExp_Explorer x(s, kind, ancestor_kind); x.More(); x.Next())
    m.Add(x.Current(), empty);
}

void map_fev(const TopoDS_Shape& s, ShapeIMap& f, ShapeIMap& e, ShapeIMap& v)
{
  if (!s.IsNull())
    walk_fev(s, f, e, v);
}

const ShapeIMap& OcShape::map(TopAbs_ShapeEnum kind) const
{
  const int k = static_cast<int>(kind);
  if (kind == TopAbs_FACE || kind == TopAbs_EDGE || kind == TopAbs_VERTEX)
  {
    const LockedSection locked_section_149;
    std::call_once(myFevFlag, [&]() { map_fev(shape, myMaps[TopAbs_FACE], myMaps[TopAbs_EDGE], myMaps[TopAbs_VERTEX]); });
    return myMaps[k];
  }
  const LockedSection locked_section_152;
  std::call_once(myFlags[k], [&]() {
    if (!shape.IsNull())
      TopExp::MapShapes(shape, kind, myMaps[k]);
  });
  return myMaps[k];
}

void face_box(const TopoDS_Shape& f, Bnd_Box& b)
{
  TopLoc_Location loc;
  if (!occ::down_cast<Geom_Plane>(BRep_Tool::Surface(TopoDS::Face(f), loc)).IsNull())
  {
    bool lines = true;
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More() && lines; e.Next())
    {
      const TopoDS_Edge&            ed = TopoDS::Edge(e.Current());
      double                        a, c;
      TopLoc_Location               l;
      const occ::handle<Geom_Curve> cu = BRep_Tool::Curve(ed, l, a, c);
      lines = !cu.IsNull() && cu->IsKind(STANDARD_TYPE(Geom_Line)) && !BRep_Tool::Degenerated(ed);
    }
    if (lines)
    {
      Bnd_Box vb;
      for (TopExp_Explorer v(f, TopAbs_VERTEX); v.More(); v.Next())
      {
        const TopoDS_Vertex& vx = TopoDS::Vertex(v.Current());
        Bnd_Box              one;
        one.Add(BRep_Tool::Pnt(vx));
        one.Enlarge(BRep_Tool::Tolerance(vx));
        vb.Add(one);
      }
      if (!vb.IsVoid())
      {
        b.Add(vb);
        return;
      }
    }
  }
  BRepBndLib::Add(f, b, false);
}

const std::vector<Bnd_Box>& OcShape::face_boxes() const
{
  const LockedSection locked_section_196;
  std::call_once(myBoxesFlag, [&]() {
    const ShapeIMap& f = map(TopAbs_FACE);
    myBoxes.resize((size_t)f.Extent());
    for (int i = 1; i <= f.Extent(); ++i)
      face_box(f(i), myBoxes[(size_t)i - 1]);
    myBoxesReady.store(true);
  });
  return myBoxes;
}

void OcShape::adopt_fev(std::array<ShapeIMap, 3>&& m)
{
  const LockedSection locked_section_208;
  std::call_once(myFevFlag, [&]() {
    myMaps[TopAbs_FACE]   = std::move(m[0]);
    myMaps[TopAbs_EDGE]   = std::move(m[1]);
    myMaps[TopAbs_VERTEX] = std::move(m[2]);
  });
}

void OcShape::adopt_face_boxes(std::vector<Bnd_Box>&& b)
{
  const LockedSection locked_section_217;
  std::call_once(myBoxesFlag, [&]() {
    myBoxes = std::move(b);
    myBoxesReady.store(true);
  });
}

// The faces' boxes of `out` from `from`'s where they are the same faces
// (made once when `from` has them), the others made here.
void inherit_face_boxes(const OcShape& from, OcShape& out)
{
  if (!from.has_face_boxes())
    return;
  const ShapeIMap&            was = from.map(TopAbs_FACE);
  const std::vector<Bnd_Box>& wb  = from.face_boxes();
  const ShapeIMap&            now = out.map(TopAbs_FACE);
  std::vector<Bnd_Box>        b((size_t)now.Extent());
  for (int i = 1; i <= now.Extent(); ++i)
  {
    const int k = was.FindIndex(now(i));
    if (k > 0)
      b[(size_t)i - 1] = wb[(size_t)k - 1];
    else
      face_box(now(i), b[(size_t)i - 1]);
  }
  out.adopt_face_boxes(std::move(b));
}

std::unique_ptr<OcShape> wrap(const TopoDS_Shape& s) { return std::make_unique<OcShape>(s); }

// ---------------------------------------------------------------- conversions

gp_Dir to_dir(const P3& p, const char* what)
{
  const double n = std::sqrt(p.x * p.x + p.y * p.y + p.z * p.z);
  if (!(n > 1e-12))
    fail_invalid(std::string(what) + " must be a non-zero vector");
  return gp_Dir(p.x / n, p.y / n, p.z / n);
}

gp_Ax3 to_ax3(const PlaneF& p)
{
  gp_Dir n = to_dir(p.normal, "plane normal");
  gp_Dir x = to_dir(p.xdir, "plane x direction");
  if (n.IsParallel(x, 1e-9))
    fail_invalid("plane x direction is parallel to its normal");
  // Orthogonalize x against n.
  gp_Vec xv(x.XYZ() - n.XYZ() * x.XYZ().Dot(n.XYZ()));
  return gp_Ax3(to_pnt(p.origin), n, gp_Dir(xv));
}

gp_Ax2 to_ax2(const PlaneF& p)
{
  gp_Ax3 a = to_ax3(p);
  return gp_Ax2(a.Location(), a.Direction(), a.XDirection());
}

gp_Pln to_pln(const PlaneF& p) { return gp_Pln(to_ax3(p)); }

PlaneF from_ax3(const gp_Ax3& a)
{
  return PlaneF{from_pnt(a.Location()), from_dir(a.XDirection()), from_dir(a.Direction())};
}

TopAbs_ShapeEnum to_kind(uint8_t k)
{
  if (k > TopAbs_SHAPE)
    fail_invalid("invalid shape type code");
  return static_cast<TopAbs_ShapeEnum>(k);
}

void require_non_null(const TopoDS_Shape& s, const char* what)
{
  if (s.IsNull())
    fail_invalid(std::string(what) + " is a null shape");
}

const TopoDS_Face& as_face(const TopoDS_Shape& s, const char* what)
{
  require_non_null(s, what);
  if (s.ShapeType() != TopAbs_FACE)
    fail_invalid(std::string(what) + " must be a face");
  return TopoDS::Face(s);
}

const TopoDS_Edge& as_edge(const TopoDS_Shape& s, const char* what)
{
  require_non_null(s, what);
  if (s.ShapeType() != TopAbs_EDGE)
    fail_invalid(std::string(what) + " must be an edge");
  return TopoDS::Edge(s);
}

const TopoDS_Vertex& as_vertex(const TopoDS_Shape& s, const char* what)
{
  require_non_null(s, what);
  if (s.ShapeType() != TopAbs_VERTEX)
    fail_invalid(std::string(what) + " must be a vertex");
  return TopoDS::Vertex(s);
}

TopoDS_Wire as_wire(const TopoDS_Shape& s, const char* what)
{
  require_non_null(s, what);
  if (s.ShapeType() == TopAbs_WIRE)
    return TopoDS::Wire(s);
  BRep_Builder b;
  TopoDS_Wire  w;
  b.MakeWire(w);
  if (s.ShapeType() == TopAbs_EDGE)
  {
    b.Add(w, s);
    return w;
  }
  if (s.ShapeType() == TopAbs_COMPOUND)
  {
    // A compound holding exactly one wire, or a set of edges already in order.
    int nw = 0;
    TopoDS_Wire one;
    for (TopExp_Explorer e(s, TopAbs_WIRE); e.More(); e.Next())
    {
      ++nw;
      one = TopoDS::Wire(e.Current());
    }
    if (nw == 1)
      return one;
  }
  fail_invalid(std::string(what) + " must be a wire or an edge");
}

std::vector<TopoDS_Shape> to_vec(const OcShapeList& l) { return l.items; }

ShapeList to_list(const std::vector<TopoDS_Shape>& v)
{
  ShapeList l;
  for (const auto& s : v)
    l.Append(s);
  return l;
}

TopoDS_Shape compound_of(const std::vector<TopoDS_Shape>& v)
{
  BRep_Builder    b;
  TopoDS_Compound c;
  b.MakeCompound(c);
  for (const auto& s : v)
    if (!s.IsNull())
      b.Add(c, s);
  return c;
}

TopoDS_Shape simplify_compound(const TopoDS_Shape& s)
{
  if (s.IsNull() || s.ShapeType() != TopAbs_COMPOUND)
    return s;
  TopoDS_Iterator it(s);
  if (!it.More())
    return s;
  TopoDS_Shape first = it.Value();
  it.Next();
  if (it.More())
    return s;
  return first;
}

// ---------------------------------------------------------------- history

static const TopAbs_ShapeEnum kHistKinds[] = {TopAbs_SOLID, TopAbs_FACE, TopAbs_EDGE,
                                              TopAbs_VERTEX};

HistoryBuilder::HistoryBuilder(const std::vector<TopoDS_Shape>& args, const TopoDS_Shape& output, const std::vector<const OcShape*>* sources, std::array<ShapeIMap, 3>* out_maps)
    : myArgs(args),
      myOut(std::make_unique<OcShape>(output))
{
  if (out_maps != nullptr)
  {
    myOut->adopt_fev(std::move(*out_maps));
  }
  for (size_t i = 0; i < myArgs.size(); ++i)
  {
    const OcShape* src = sources != nullptr && i < sources->size() ? (*sources)[i] : nullptr;
    // The same shape, orientation too (MapShapes keeps the first occurrence's).
    if (src != nullptr && src->shape.IsEqual(myArgs[i]))
      myArgMaps.push_back(src);
    else
    {
      myOwnArgMaps.push_back(std::make_unique<OcShape>(myArgs[i]));
      myArgMaps.push_back(myOwnArgMaps.back().get());
    }
  }
}

void HistoryBuilder::push(uint32_t arg, uint8_t in_kind, uint32_t in_index, uint8_t rel,
                          const TopoDS_Shape& out)
{
  HistRec r;
  r.arg      = arg;
  r.in_kind  = in_kind;
  r.in_index = in_index;
  r.relation = rel;
  if (rel == REL_DELETED || out.IsNull())
  {
    r.out_kind  = 8;
    r.out_index = UINT32_MAX;
    myRecs.push_back(r);
    return;
  }
  const TopAbs_ShapeEnum ok = out.ShapeType();
  if (ok == TopAbs_COMPOUND || ok == TopAbs_SHELL || ok == TopAbs_WIRE)
  {
    // Expand containers into their faces / edges.
    TopAbs_ShapeEnum sub = ok == TopAbs_WIRE ? TopAbs_EDGE : TopAbs_FACE;
    for (TopExp_Explorer e(out, sub); e.More(); e.Next())
      push(arg, in_kind, in_index, rel, e.Current());
    if (ok == TopAbs_COMPOUND)
      for (TopExp_Explorer e(out, TopAbs_EDGE, TopAbs_FACE); e.More(); e.Next())
        push(arg, in_kind, in_index, rel, e.Current());
    return;
  }
  const int idx = myOut->map(ok).FindIndex(out);
  if (idx <= 0)
    return; // not part of the final result
  r.out_kind  = static_cast<uint8_t>(ok);
  r.out_index = static_cast<uint32_t>(idx - 1);
  myRecs.push_back(r);
}

void HistoryBuilder::add_history(const BRepTools_History* h)
{
  for (uint32_t ai = 0; ai < myArgs.size(); ++ai)
  {
    if (myArgs[ai].IsNull())
      continue;
    for (TopAbs_ShapeEnum k : kHistKinds)
    {
      const ShapeIMap& in = myArgMaps[ai]->map(k);
      for (int i = 1; i <= in.Extent(); ++i)
      {
        const TopoDS_Shape& s     = in(i);
        bool                found = false;
        if (h != nullptr)
        {
          if (h->IsRemoved(s))
          {
            push(ai, (uint8_t)k, (uint32_t)(i - 1), REL_DELETED, TopoDS_Shape());
            found = true;
          }
          for (NCollection_List<TopoDS_Shape>::Iterator it(h->Modified(s)); it.More(); it.Next())
          {
            push(ai, (uint8_t)k, (uint32_t)(i - 1), REL_MODIFIED, it.Value());
            found = true;
          }
          for (NCollection_List<TopoDS_Shape>::Iterator it(h->Generated(s)); it.More(); it.Next())
            push(ai, (uint8_t)k, (uint32_t)(i - 1), REL_GENERATED, it.Value());
        }
        if (!found)
        {
          const int oi = myOut->map(k).FindIndex(s);
          if (oi > 0)
          {
            HistRec r{ai, (uint8_t)k, (uint32_t)(i - 1), REL_KEPT, (uint8_t)k, (uint32_t)(oi - 1)};
            myRecs.push_back(r);
          }
        }
      }
    }
  }
}

void HistoryBuilder::add_kept() { add_history(nullptr); }

ShapeList images_of(const occ::handle<BRepTools_History>& h, const TopoDS_Shape& s)
{
  ShapeList r;
  if (s.IsNull())
    return r;
  if (h.IsNull())
  {
    r.Append(s);
    return r;
  }
  if (h->IsRemoved(s))
    return r;
  const ShapeList& m = h->Modified(s);
  if (m.IsEmpty())
    r.Append(s);
  else
    r = m;
  return r;
}

void HistoryBuilder::add_mapped(uint32_t arg, uint8_t relation,
                                const std::function<ShapeList(const TopoDS_Shape&)>& fn)
{
  if (arg >= myArgs.size())
    return;
  for (TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
  {
    const ShapeIMap& in = myArgMaps[arg]->map(k);
    for (int i = 1; i <= in.Extent(); ++i)
    {
      ShapeList r;
      try
      {
        r = fn(in(i));
      }
      catch (const Standard_Failure&)
      {
        continue;
      }
      for (ShapeList::Iterator it(r); it.More(); it.Next())
        if (!it.Value().IsNull())
          push(arg, (uint8_t)k, (uint32_t)(i - 1), relation, it.Value());
    }
  }
}

void HistoryBuilder::add_cap(uint32_t arg, uint8_t relation, const TopoDS_Shape& cap)
{
  if (arg >= myArgs.size() || cap.IsNull())
    return;
  uint8_t  in_kind  = TopAbs_FACE;
  uint32_t in_index = 0;
  if (myArgMaps[arg]->map(TopAbs_FACE).Extent() == 0)
  {
    in_kind = TopAbs_WIRE;
    if (myArgMaps[arg]->map(TopAbs_WIRE).Extent() == 0)
      in_kind = TopAbs_EDGE;
  }
  for (TopExp_Explorer e(cap, TopAbs_FACE); e.More(); e.Next())
    push(arg, in_kind, in_index, relation, e.Current());
}

OpOut HistoryBuilder::finish()
{
  // The result's OcShape with the maps the records needed (its first
  // queries would make them again).
  OpOut o;
  o.shape   = std::move(myOut);
  o.history = std::move(myRecs);
  return o;
}

OpOut make_result(const std::vector<TopoDS_Shape>& args, const TopoDS_Shape& out,
                  const BRepTools_History* h, const std::vector<const OcShape*>* sources,
                  std::array<ShapeIMap, 3>* out_maps)
{
  HistoryBuilder hb(args, out, sources, out_maps);
  hb.add_history(h);
  return hb.finish();
}

// ---------------------------------------------------------------- shape basics

std::unique_ptr<OcShape> shape_null() { return std::make_unique<OcShape>(); }

std::unique_ptr<OcShape> shape_clone(const OcShape& s) { return wrap(s.shape); }

uint8_t shape_type(const OcShape& s)
{
  if (s.shape.IsNull())
    return TopAbs_SHAPE;
  return static_cast<uint8_t>(s.shape.ShapeType());
}

bool shape_is_null(const OcShape& s) { return s.shape.IsNull(); }

uint8_t shape_orientation(const OcShape& s)
{
  if (s.shape.IsNull())
    return 0;
  return static_cast<uint8_t>(s.shape.Orientation());
}

std::unique_ptr<OcShape> shape_oriented(const OcShape& s, uint8_t orientation)
{
  if (orientation > TopAbs_EXTERNAL)
    fail_invalid("invalid orientation code");
  require_non_null(s.shape, "shape");
  return wrap(s.shape.Oriented(static_cast<TopAbs_Orientation>(orientation)));
}

uint64_t shape_hash(const OcShape& s)
{
  if (s.shape.IsNull())
    return 0;
  return static_cast<uint64_t>(std::hash<TopoDS_Shape>{}(s.shape));
}

uint64_t shape_partner_hash(const OcShape& s)
{
  return s.shape.IsNull() ? 0 : static_cast<uint64_t>(reinterpret_cast<uintptr_t>(s.shape.TShape().get()));
}

rust::Vec<double> shape_location(const OcShape& s)
{
  const gp_Trsf     t = s.shape.Location().Transformation();
  rust::Vec<double> m;
  m.reserve(12);
  for (int r = 1; r <= 3; ++r)
    for (int c = 1; c <= 4; ++c)
      m.push_back(t.Value(r, c));
  return m;
}

std::unique_ptr<OcShape> shape_unlocated(const OcShape& s)
{
  return wrap(s.shape.Located(TopLoc_Location()));
}

bool shape_is_same(const OcShape& a, const OcShape& b) { return a.shape.IsSame(b.shape); }
bool shape_is_equal(const OcShape& a, const OcShape& b) { return a.shape.IsEqual(b.shape); }
bool shape_is_partner(const OcShape& a, const OcShape& b) { return a.shape.IsPartner(b.shape); }

uint32_t sub_count(const OcShape& s, uint8_t kind)
{
  return static_cast<uint32_t>(s.map(to_kind(kind)).Extent());
}

std::unique_ptr<OcShape> sub_shape(const OcShape& s, uint8_t kind, uint32_t index)
{
  const ShapeIMap& m = s.map(to_kind(kind));
  if (index >= static_cast<uint32_t>(m.Extent()))
    fail_invalid("sub-shape index out of range");
  return wrap(m(static_cast<int>(index) + 1));
}

std::unique_ptr<OcShapeList> sub_shapes(const OcShape& s, uint8_t kind)
{
  const ShapeIMap& m = s.map(to_kind(kind));
  auto             l = std::make_unique<OcShapeList>();
  l->items.reserve(m.Extent());
  for (int i = 1; i <= m.Extent(); ++i)
    l->items.push_back(m(i));
  return l;
}

int64_t index_of(const OcShape& s, const OcShape& sub, uint8_t kind)
{
  if (sub.shape.IsNull())
    return -1;
  const int i = s.map(to_kind(kind)).FindIndex(sub.shape);
  return i > 0 ? i - 1 : -1;
}


Csr ancestor_map(const OcShape& s, uint8_t kind, uint8_t ancestor_kind)
{
  const std::pair<std::vector<uint32_t>, std::vector<uint32_t>>& m = ancestors_cached(s, to_kind(kind), to_kind(ancestor_kind));
  Csr out;
  out.offsets.reserve(m.first.size());
  for (uint32_t v : m.first)
    out.offsets.push_back(v);
  out.values.reserve(m.second.size());
  for (uint32_t v : m.second)
    out.values.push_back(v);
  return out;
}

// For every ancestor, the sub-shapes of `kind` in it (as
// TopExp::MapShapesAndAncestors finds them), as (sub-shape, ancestor) index
// pairs sorted: the rows of the map, each ancestor once, in order.
static std::pair<std::vector<uint32_t>, std::vector<uint32_t>> ancestor_map_uncached(const OcShape& s, TopAbs_ShapeEnum k, TopAbs_ShapeEnum ak)
{
  const KernelTimer timer(k == TopAbs_EDGE && ak == TopAbs_FACE     ? "ancestors.edge_face"
                          : k == TopAbs_VERTEX && ak == TopAbs_FACE ? "ancestors.vertex_face"
                          : k == TopAbs_VERTEX && ak == TopAbs_EDGE ? "ancestors.vertex_edge"
                                                                    : "ancestors.other");
  const ShapeIMap&                           km = s.map(k);
  const ShapeIMap&                           am = s.map(ak);
  std::vector<std::pair<uint32_t, uint32_t>> pairs;
  for (int a = 1; a <= am.Extent(); ++a)
    for (TopExp_Explorer x(am(a), k); x.More(); x.Next())
    {
      const int i = km.FindIndex(x.Current());
      if (i > 0)
        pairs.emplace_back((uint32_t)i - 1, (uint32_t)a - 1);
    }
  std::sort(pairs.begin(), pairs.end());
  pairs.erase(std::unique(pairs.begin(), pairs.end()), pairs.end());
  std::pair<std::vector<uint32_t>, std::vector<uint32_t>> out;
  out.first.assign((size_t)km.Extent() + 1, 0);
  out.second.reserve(pairs.size());
  for (const auto& [i, a] : pairs)
  {
    ++out.first[(size_t)i + 1];
    out.second.push_back(a);
  }
  for (size_t i = 1; i < out.first.size(); ++i)
    out.first[i] += out.first[i - 1];
  return out;
}

using Csr32 = std::pair<std::vector<uint32_t>, std::vector<uint32_t>>;

static Csr32 csr_of(std::vector<std::pair<uint32_t, uint32_t>>& pairs, int rows)
{
  std::sort(pairs.begin(), pairs.end());
  pairs.erase(std::unique(pairs.begin(), pairs.end()), pairs.end());
  Csr32 out;
  out.first.assign((size_t)rows + 1, 0);
  out.second.reserve(pairs.size());
  for (const auto& [i, a] : pairs)
  {
    ++out.first[(size_t)i + 1];
    out.second.push_back(a);
  }
  for (size_t i = 1; i < out.first.size(); ++i)
    out.first[i] += out.first[i - 1];
  return out;
}

// Edge -> faces and vertex -> faces together (the naming asks for both): one
// walk of each face's edges and their vertices, the rows as
// ancestor_map_uncached makes them.
static std::pair<Csr32, Csr32> edge_and_vertex_faces(const OcShape& s)
{
  const KernelTimer                          timer("ancestors.edge_vertex_face");
  const ShapeIMap&                           fm = s.map(TopAbs_FACE);
  const ShapeIMap&                           em = s.map(TopAbs_EDGE);
  const ShapeIMap&                           vm = s.map(TopAbs_VERTEX);
  std::vector<std::pair<uint32_t, uint32_t>> pe, pv;
  uint32_t                                   a = 0;
  std::function<void(const TopoDS_Shape&)>   visit = [&](const TopoDS_Shape& x) {
    if (x.ShapeType() == TopAbs_VERTEX)
    {
      const int i = vm.FindIndex(x);
      if (i > 0)
        pv.emplace_back((uint32_t)i - 1, a);
      return;
    }
    if (x.ShapeType() == TopAbs_EDGE)
    {
      const int i = em.FindIndex(x);
      if (i > 0)
        pe.emplace_back((uint32_t)i - 1, a);
    }
    for (TopoDS_Iterator it(x); it.More(); it.Next())
      visit(it.Value());
  };
  for (int f = 1; f <= fm.Extent(); ++f)
  {
    a = (uint32_t)f - 1;
    for (TopoDS_Iterator it(fm(f)); it.More(); it.Next())
      visit(it.Value());
  }
  return {csr_of(pe, em.Extent()), csr_of(pv, vm.Extent())};
}

const std::pair<std::vector<uint32_t>, std::vector<uint32_t>>& ancestors_cached(const OcShape& s, TopAbs_ShapeEnum kind, TopAbs_ShapeEnum ancestor_kind)
{
  const int key = (int)kind * 16 + (int)ancestor_kind;
  {
    const LockedSection locked_section_769;
    std::lock_guard<std::mutex> g(s.myAncestorsLock);
    auto                        it = s.myAncestors.find(key);
    if (it != s.myAncestors.end())
      return it->second;
  }
  if (ancestor_kind == TopAbs_FACE && (kind == TopAbs_EDGE || kind == TopAbs_VERTEX))
  {
    auto                        both = edge_and_vertex_faces(s);
    const LockedSection locked_section_777;
    std::lock_guard<std::mutex> g(s.myAncestorsLock);
    s.myAncestors.emplace((int)TopAbs_EDGE * 16 + (int)TopAbs_FACE, std::move(both.first));
    s.myAncestors.emplace((int)TopAbs_VERTEX * 16 + (int)TopAbs_FACE, std::move(both.second));
    return s.myAncestors.find(key)->second;
  }
  auto                        m = ancestor_map_uncached(s, kind, ancestor_kind);
  const LockedSection locked_section_783;
  std::lock_guard<std::mutex> g(s.myAncestorsLock);
  // std::map's entries stay where they are: the reference outlives the lock.
  return s.myAncestors.emplace(key, std::move(m)).first->second;
}

// Whether an edge is an OCCT seam: it closes every face it bounds on itself
// (twice in the face's wire, BRep_Tool::IsClosed). Parasolid has no such edge.
bool is_seam(const TopoDS_Edge& e, const NCollection_List<TopoDS_Shape>* faces)
{
  if (faces == nullptr || faces->IsEmpty())
    return false;
  for (NCollection_List<TopoDS_Shape>::Iterator it(*faces); it.More(); it.Next())
    if (!BRep_Tool::IsClosed(e, TopoDS::Face(it.Value())))
      return false;
  return true;
}

// Onshape's (Parasolid's) edges and vertices among OCCT's: seams and
// degenerated edges (a sphere's poles) aren't edges there; a vertex is one if
// it ends a shown edge that isn't closed, has no edge (a point), or is a
// cone's apex (live: a drill tip is a vertex; a cylinder's circles, a ring's
// and a sphere have none). Flags in MapShapes order, the edges' then the
// vertices'.
// Made once per shape from its cached ancestor maps (the naming made the
// edge / vertex -> face ones): the display asked twice a regeneration, each
// time mapping the 1600-face fin plate's ancestors three times (~70 ms).
rust::Vec<uint8_t> shown_topology(const OcShape& s)
{
  const LockedSection locked_section_811;
  std::call_once(s.myShownFlag, [&]() {
    const KernelTimer timer("shown_topology.make");
    const ShapeIMap&  em = s.map(TopAbs_EDGE);
    const ShapeIMap&  vm = s.map(TopAbs_VERTEX);
    const ShapeIMap&  fm = s.map(TopAbs_FACE);
    const auto&       ef = ancestors_cached(s, TopAbs_EDGE, TopAbs_FACE);
    const auto&       ve = ancestors_cached(s, TopAbs_VERTEX, TopAbs_EDGE);
    const auto&       vf = ancestors_cached(s, TopAbs_VERTEX, TopAbs_FACE);
    std::vector<uint8_t>& out = s.myShown;
    out.reserve((size_t)(em.Extent() + vm.Extent()));
    std::vector<bool> open(em.Extent() + 1, false);
    for (int i = 1; i <= em.Extent(); ++i)
    {
      const TopoDS_Edge& e = TopoDS::Edge(em(i));
      // A seam closes every face it bounds on itself.
      bool seam = ef.first[(size_t)i] > ef.first[(size_t)i - 1];
      for (uint32_t k = ef.first[(size_t)i - 1]; k < ef.first[(size_t)i] && seam; ++k)
        seam = BRep_Tool::IsClosed(e, TopoDS::Face(fm((int)ef.second[k] + 1)));
      const bool    shown = !BRep_Tool::Degenerated(e) && !seam;
      TopoDS_Vertex a, b;
      TopExp::Vertices(e, a, b);
      open[i] = shown && !a.IsSame(b);
      out.push_back(shown ? 1 : 0);
    }
    for (int i = 1; i <= vm.Extent(); ++i)
    {
      const uint32_t e0 = ve.first[(size_t)i - 1], e1 = ve.first[(size_t)i];
      bool           shown = e0 == e1, anyEdge = false;
      for (uint32_t k = e0; k < e1 && !shown; ++k)
      {
        const int j = (int)ve.second[k] + 1;
        shown       = open[j];
        anyEdge     = anyEdge || out[(size_t)j - 1] != 0;
      }
      if (!shown && !anyEdge)
        for (uint32_t k = vf.first[(size_t)i - 1]; k < vf.first[(size_t)i] && !shown; ++k)
          shown = BRepAdaptor_Surface(TopoDS::Face(fm((int)vf.second[k] + 1)), false).GetType() == GeomAbs_Cone;
      out.push_back(shown ? 1 : 0);
    }
  });
  rust::Vec<uint8_t> out;
  out.reserve(s.myShown.size());
  for (uint8_t v : s.myShown)
    out.push_back(v);
  return out;
}

rust::Vec<uint32_t> adjacent_faces(const OcShape& s, uint32_t face_index)
{
  const ShapeIMap& fm = s.map(TopAbs_FACE);
  if (face_index >= static_cast<uint32_t>(fm.Extent()))
    fail_invalid("face index out of range");
  ShapeAncMap anc;
  TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, anc);
  const TopoDS_Shape&   f = fm(static_cast<int>(face_index) + 1);
  std::vector<uint32_t> res;
  for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
  {
    const NCollection_List<TopoDS_Shape>* l = anc.Seek(e.Current());
    if (l == nullptr)
      continue;
    for (NCollection_List<TopoDS_Shape>::Iterator it(*l); it.More(); it.Next())
    {
      if (it.Value().IsSame(f))
        continue;
      const int ai = fm.FindIndex(it.Value());
      if (ai > 0)
        res.push_back(static_cast<uint32_t>(ai - 1));
    }
  }
  std::sort(res.begin(), res.end());
  res.erase(std::unique(res.begin(), res.end()), res.end());
  rust::Vec<uint32_t> out;
  for (uint32_t v : res)
    out.push_back(v);
  return out;
}

std::unique_ptr<OcShapeList> children(const OcShape& s)
{
  auto l = std::make_unique<OcShapeList>();
  if (s.shape.IsNull())
    return l;
  for (TopoDS_Iterator it(s.shape); it.More(); it.Next())
    l->items.push_back(it.Value());
  return l;
}

std::unique_ptr<OcShape> make_compound(const OcShapeList& l) { return wrap(compound_of(l.items)); }

std::unique_ptr<OcShapeList> list_new() { return std::make_unique<OcShapeList>(); }
void list_push(OcShapeList& l, const OcShape& s)
{
  l.items.push_back(s.shape);
  l.sources.resize(l.items.size() - 1, nullptr);
  l.sources.push_back(&s);
}
size_t list_len(const OcShapeList& l) { return l.items.size(); }

std::unique_ptr<OcShape> list_get(const OcShapeList& l, size_t i)
{
  if (i >= l.items.size())
    fail_invalid("list index out of range");
  return wrap(l.items[i]);
}

bool is_valid(const OcShape& s)
{
  if (s.shape.IsNull())
    return false;
  if (has_mesh_faces(s.shape))
    return mesh_valid(s.shape);
  BRepCheck_Analyzer a(s.shape);
  return a.IsValid();
}

namespace {

// ---- integrals over faces that wrap a periodic surface more than once
// (a thread's crest between two helices: u spans 16 turns). OCCT's
// integrators take the inner integral of Green's formula across the face's
// whole u range with a fixed number of points and get these wrong (Gauss
// +14 mm³ on a McMaster screw, adaptive ±1); here the inner integral uses
// the period, and every integral is split into short Gauss segments.

// Ten moments: ∫1, ∫x, ∫y, ∫z, ∫x², ∫y², ∫z², ∫xy, ∫yz, ∫zx (over the
// volume, by the divergence theorem, or over the area). The McMaster screw:
// 254.418923 mm³ (Onshape 254.40378; OCCT Gauss 268.69, adaptive 254.1–255.7).
using Moments = std::array<double, 10>;

constexpr double kGaussX[8] = {-0.9602898564975363, -0.7966664774136267, -0.5255324099163290, -0.1834346424956498,
                               0.1834346424956498,  0.5255324099163290,  0.7966664774136267,  0.9602898564975363};
constexpr double kGaussW[8] = {0.1012285362903763, 0.2223810344533745, 0.3137066458778873, 0.3626837833783620,
                               0.3626837833783620, 0.3137066458778873, 0.2223810344533745, 0.1012285362903763};

// A face OCCT's integrators get wrong: wrapping a periodic surface more
// than once (a thread's crest). Such a solid is integrated here whole, its
// many-span flanks too (OCCT's Gauss and adaptive disagree on those).
bool hard_to_integrate(const TopoDS_Face& f)
{
  TopLoc_Location                 l;
  const occ::handle<Geom_Surface> s = BRep_Tool::Surface(f, l);
  if (s.IsNull() || !s->IsUPeriodic())
    return false;
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  return u1 - u0 > 1.01 * s->UPeriod();
}

// g(u, v): the moments' integrands times the area element (volume: F·(Su×Sv)
// with div F the moment's function; area: the function times |Su×Sv|).
void integrand(const BRepAdaptor_Surface& s, double u, double v, bool volume, const gp_XYZ& o, Moments& g)
{
  gp_Pnt p;
  gp_Vec su, sv;
  s.D1(u, v, p, su, sv);
  const gp_Vec n = su.Crossed(sv);
  const double x = p.X() - o.X(), y = p.Y() - o.Y(), z = p.Z() - o.Z();
  if (volume)
  {
    g = {(x * n.X() + y * n.Y() + z * n.Z()) / 3, x * x / 2 * n.X(), y * y / 2 * n.Y(), z * z / 2 * n.Z(), x * x * x / 3 * n.X(),
         y * y * y / 3 * n.Y(), z * z * z / 3 * n.Z(), x * x * y / 2 * n.X(), y * y * z / 2 * n.Y(), z * z * x / 2 * n.Z()};
    return;
  }
  const double a = n.Magnitude();
  g              = {a, x * a, y * a, z * a, x * x * a, y * y * a, z * z * a, x * y * a, y * z * a, z * x * a};
}

// Segments of at most `step` over a length (a hair over a whole number of
// steps is that number: π / (π/8) is 8, not 9).
int segments(double length, double step)
{
  return std::max(1, (int)std::ceil(std::abs(length) / step - 1e-9));
}

// ∫ g(u, v) du over [a, b] in segments of at most `step`, split first at
// `breaks` (the surface's u knots: g is only C0 or C1 there, and a segment
// across one integrates to second order only: a NURBS sphere read back from
// Rhino, +4e-5 of its volume).
Moments inner(const BRepAdaptor_Surface& s, double a, double b, double v, double step, bool volume, const gp_XYZ& o,
              const std::vector<double>& breaks)
{
  Moments             out{};
  std::vector<double> pts{a};
  for (double k : breaks)
    if (k > std::min(a, b) + 1e-12 && k < std::max(a, b) - 1e-12)
      pts.push_back(k);
  pts.push_back(b);
  std::sort(pts.begin() + 1, pts.end() - 1);
  if (b < a)
    std::reverse(pts.begin() + 1, pts.end() - 1);
  for (size_t piece = 0; piece + 1 < pts.size(); ++piece)
  {
    const double pa = pts[piece], pb = pts[piece + 1];
    const int    n  = segments(pb - pa, step);
    for (int k = 0; k < n; ++k)
    {
      const double s0 = pa + (pb - pa) * k / n, h = (pb - pa) / n;
      for (int i = 0; i < 8; ++i)
      {
        Moments g;
        integrand(s, s0 + h * (kGaussX[i] + 1) / 2, v, volume, o, g);
        for (int j = 0; j < 10; ++j)
          out[(size_t)j] += g[(size_t)j] * kGaussW[i] * h / 2;
      }
    }
  }
  return out;
}

// The surface's parameter values where it's less than C2 along u (`along_u`)
// or v (B-spline knots, a sweep's section knots), repeated a period either
// side on a periodic surface.
std::vector<double> surface_breaks(const BRepAdaptor_Surface& s, bool along_u)
{
  std::vector<double> out;
  const int           n = along_u ? s.NbUIntervals(GeomAbs_C2) : s.NbVIntervals(GeomAbs_C2);
  if (n <= 1)
    return out;
  NCollection_Array1<double> t(1, n + 1);
  if (along_u)
    s.UIntervals(t, GeomAbs_C2);
  else
    s.VIntervals(t, GeomAbs_C2);
  const bool   per    = along_u ? s.IsUPeriodic() : s.IsVPeriodic();
  const double period = per ? (along_u ? s.UPeriod() : s.VPeriod()) : 0;
  for (int i = 1; i <= n + 1; ++i)
    for (int k = per ? -1 : 0; k <= (per ? 1 : 0); ++k)
      out.push_back(t(i) + k * period);
  return out;
}

// Gaps in a face's boundary: where an edge's pcurve ends away from where
// the next edge's starts (a tolerant vertex of an import: g50's shell, 0.02
// mm). Parasolid closes them with straight (u, v) segments, so face areas
// and volumes count the sliver between; OCCT's integrators skip them and
// get a result that depends on where u is measured from (F18 of g50:
// -0.0617 mm², live 82.8478429). Each gap: from the end (u, v) of one edge
// to the start of the next, in the face's forward orientation. Gaps that
// are no gap in space (ends a period apart) are not counted.
std::vector<std::pair<gp_Pnt2d, gp_Pnt2d>> uv_gaps(const TopoDS_Face& face)
{
  std::vector<std::pair<gp_Pnt2d, gp_Pnt2d>> out;
  const TopoDS_Face f = TopoDS::Face(face.Oriented(TopAbs_FORWARD));
  TopLoc_Location   loc;
  const occ::handle<Geom_Surface> S = BRep_Tool::Surface(f, loc);
  if (S.IsNull())
    return out;
  const double uper = S->IsUPeriodic() ? S->UPeriod() : 0, vper = S->IsVPeriodic() ? S->VPeriod() : 0;
  for (TopExp_Explorer w(f, TopAbs_WIRE); w.More(); w.Next())
  {
    std::vector<std::pair<gp_Pnt2d, gp_Pnt2d>> ends;
    for (BRepTools_WireExplorer we(TopoDS::Wire(w.Current()), f); we.More(); we.Next())
    {
      const TopoDS_Edge& e = we.Current();
      double             a, b;
      const occ::handle<Geom2d_Curve> c = BRep_Tool::CurveOnSurface(e, f, a, b);
      if (c.IsNull())
        return {};
      if (e.Orientation() == TopAbs_REVERSED)
        std::swap(a, b);
      ends.emplace_back(c->Value(a), c->Value(b));
    }
    for (size_t i = 0; i < ends.size() && ends.size() > 1; ++i)
    {
      const gp_Pnt2d p = ends[i].second, q = ends[(i + 1) % ends.size()].first;
      const double   du = std::abs(q.X() - p.X()), dv = std::abs(q.Y() - p.Y());
      if ((uper > 0 && du > uper / 4) || (vper > 0 && dv > vper / 4))
        continue;
      const double gap = S->Value(p.X(), p.Y()).Distance(S->Value(q.X(), q.Y()));
      if (gap > 1e-9 && gap < 1e-3)
        out.emplace_back(p, q);
    }
  }
  return out;
}

// The face's moments by Green's formula: ∬ g du dv = ∮ G dv, G(u, v) = ∫ g du
// from the face's lowest u (on a u-periodic surface whole periods once, then
// the remainder). Segments: a sixteenth of a period, or half a knot span.
Moments face_moments(const TopoDS_Face& face, bool volume, const gp_XYZ& o = gp_XYZ())
{
  const TopoDS_Face   f = TopoDS::Face(face.Oriented(TopAbs_FORWARD));
  BRepAdaptor_Surface s(f, false);
  double              u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  const bool   uper = s.IsUPeriodic(), vper = s.IsVPeriodic();
  const double period = uper ? s.UPeriod() : 0;
  const auto   step   = [](bool per, double p, int spans, double lo, double hi) {
    if (per)
      return p / 16;
    return std::max(hi - lo, 1e-300) / (2 * std::max(1, spans));
  };
  const int    nus = s.GetType() == GeomAbs_BSplineSurface ? s.NbUIntervals(GeomAbs_C2) : 4;
  const int    nvs = s.GetType() == GeomAbs_BSplineSurface ? s.NbVIntervals(GeomAbs_C2) : 4;
  const double ustep = step(uper, period, nus, s.FirstUParameter(), s.LastUParameter());
  const double vstep = step(vper, vper ? s.VPeriod() : 0, nvs, s.FirstVParameter(), s.LastVParameter());
  const double ust   = std::min(ustep, std::max(u1 - u0, 1e-300) / 4) / 2; // halved: converged to 1e-7 relative on a threaded screw
  const double vst   = std::min(vstep, std::max(v1 - v0, 1e-300) / 4) / 2;
  const std::vector<double> ubreaks = surface_breaks(s, true), vbreaks = surface_breaks(s, false);
  const auto G = [&](double u, double v) {
    const double turns = uper ? std::floor((u - u0) / period) : 0;
    Moments      out   = inner(s, u0, u - turns * period, v, ust, volume, o, ubreaks);
    if (turns != 0)
    {
      const Moments full = inner(s, u0, u0 + period, v, ust, volume, o, ubreaks);
      for (int j = 0; j < 10; ++j)
        out[(size_t)j] += turns * full[(size_t)j];
    }
    return out;
  };
  Moments total{};
  for (TopExp_Explorer ee(f, TopAbs_EDGE); ee.More(); ee.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(ee.Current());
    // Only the boundary counts: an internal edge (a union along a tangent
    // cylinder leaves one in the face; DIN 985 M48's hex flats) bounds
    // nothing and would add its line integral once (-0.2165 mm² a flat).
    if (e.Orientation() == TopAbs_INTERNAL || e.Orientation() == TopAbs_EXTERNAL)
      continue;
    double             a, b;
    const occ::handle<Geom2d_Curve> c = BRep_Tool::CurveOnSurface(e, f, a, b);
    if (c.IsNull())
      continue;
    if (e.Orientation() == TopAbs_REVERSED)
      std::swap(a, b);
    // Pieces between the pcurve's knots (its parameterization may be very
    // uneven), each cut into segments short in both u and v (sampled: an edge
    // may wind and come back).
    std::vector<double> cuts{std::min(a, b), std::max(a, b)};
    {
      occ::handle<Geom2d_Curve> basis = c;
      while (auto tc = occ::down_cast<Geom2d_TrimmedCurve>(basis))
        basis = tc->BasisCurve();
      if (auto bs = occ::down_cast<Geom2d_BSplineCurve>(basis))
        for (int k = 1; k <= bs->NbKnots(); ++k)
          if (bs->Knot(k) > cuts[0] && bs->Knot(k) < cuts[1])
            cuts.push_back(bs->Knot(k));
      // Where the pcurve crosses the surface's knots: the integrand of the
      // boundary integral is only C0 or C1 across them.
      const double lo = cuts[0], hi = cuts[1];
      const int    ns = 64;
      for (int k = 0; k < ns; ++k)
      {
        const double ta = lo + (hi - lo) * k / ns, tb = lo + (hi - lo) * (k + 1) / ns;
        const gp_Pnt2d pa = c->Value(ta), pb = c->Value(tb);
        for (int axis = 0; axis < 2; ++axis)
          for (double kv : axis ? vbreaks : ubreaks)
          {
            const double fa = (axis ? pa.Y() : pa.X()) - kv, fb = (axis ? pb.Y() : pb.X()) - kv;
            if (!(fa * fb < 0))
              continue;
            double x = ta, y = tb, fx = fa;
            for (int it = 0; it < 60; ++it)
            {
              const double m  = (x + y) / 2;
              const double fm = (axis ? c->Value(m).Y() : c->Value(m).X()) - kv;
              if ((fm < 0) == (fx < 0))
              {
                x  = m;
                fx = fm;
              }
              else
                y = m;
            }
            cuts.push_back((x + y) / 2);
          }
      }
      std::sort(cuts.begin(), cuts.end());
      cuts.erase(std::unique(cuts.begin(), cuts.end(), [&](double p, double q) { return q - p <= 1e-12 * std::max(1.0, hi - lo); }), cuts.end());
      if (b < a)
        std::reverse(cuts.begin(), cuts.end());
    }
    for (size_t piece = 0; piece + 1 < cuts.size(); ++piece)
    {
      const double p0 = cuts[piece], p1 = cuts[piece + 1];
      double       du = 0, dv = 0;
      for (int k = 0; k < 16; ++k)
      {
        const gp_Pnt2d p = c->Value(p0 + (p1 - p0) * k / 16), q = c->Value(p0 + (p1 - p0) * (k + 1) / 16);
        du += std::abs(q.X() - p.X());
        dv += std::abs(q.Y() - p.Y());
      }
      const int n = std::min(20000, segments(2 * (du / ust + dv / vst), 1.0));
      for (int k = 0; k < n; ++k)
      {
        const double t0 = p0 + (p1 - p0) * k / n, h = (p1 - p0) / n;
        for (int i = 0; i < 8; ++i)
        {
          gp_Pnt2d     q;
          gp_Vec2d     d;
          const double t = t0 + h * (kGaussX[i] + 1) / 2;
          c->D1(t, q, d);
          const Moments g = G(q.X(), q.Y());
          for (int j = 0; j < 10; ++j)
            total[(size_t)j] += g[(size_t)j] * d.Y() * kGaussW[i] * h / 2;
        }
      }
    }
  }
  // The boundary's gaps, closed with straight (u, v) segments.
  for (const auto& [p, q] : uv_gaps(f))
    for (int i = 0; i < 8; ++i)
    {
      const double  t = (kGaussX[i] + 1) / 2;
      const Moments g = G(p.X() + (q.X() - p.X()) * t, p.Y() + (q.Y() - p.Y()) * t);
      for (int j = 0; j < 10; ++j)
        total[(size_t)j] += g[(size_t)j] * (q.Y() - p.Y()) * kGaussW[i] / 2;
    }
  if (volume && face.Orientation() == TopAbs_REVERSED)
    for (double& m : total)
      m = -m;
  if (!volume && total[0] < 0) // the area is positive whatever the loops' direction
    for (double& m : total)
      m = -m;
  return total;
}

// Moments about `o` as moments about the origin.
Moments unshift(const Moments& m, const gp_XYZ& o)
{
  const double x = o.X(), y = o.Y(), z = o.Z(), v = m[0];
  return {v,
          m[1] + x * v,
          m[2] + y * v,
          m[3] + z * v,
          m[4] + 2 * x * m[1] + x * x * v,
          m[5] + 2 * y * m[2] + y * y * v,
          m[6] + 2 * z * m[3] + z * z * v,
          m[7] + x * m[2] + y * m[1] + x * y * v,
          m[8] + y * m[3] + z * m[2] + y * z * v,
          m[9] + z * m[1] + x * m[3] + z * x * v};
}

// Moments as properties (inertia about the origin, as BRepGProp keeps it).
struct MomentProps : GProp_GProps
{
  explicit MomentProps(const Moments& m) : GProp_GProps(gp::Origin())
  {
    dim     = m[0];
    g       = dim != 0 ? gp_Pnt(m[1] / dim, m[2] / dim, m[3] / dim) : gp::Origin();
    inertia = gp_Mat(gp_XYZ(m[5] + m[6], -m[7], -m[9]), gp_XYZ(-m[7], m[4] + m[6], -m[8]), gp_XYZ(-m[9], -m[8], m[4] + m[5]));
  }
};

// The shape's faces split into those OCCT integrates and those it doesn't.
bool split_wrapping(const TopoDS_Shape& s, TopoDS_Compound& rest, std::vector<TopoDS_Face>& wrapping)
{
  BRep_Builder B;
  B.MakeCompound(rest);
  for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    if (hard_to_integrate(f) || !uv_gaps(f).empty())
      wrapping.push_back(f);
    else
      B.Add(rest, f);
  }
  return !wrapping.empty();
}

} // namespace

// Whether every face lies on a plane, cylinder, cone, sphere or torus (where
// OCCT's adaptive integrator controls its error; on B-spline surfaces and
// sweeps it can be far off: a curvature fillet's extruded B-spline section
// read +30 mm³).
static bool analytic_only(const TopoDS_Shape& s)
{
  for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
    switch (BRepAdaptor_Surface(TopoDS::Face(e.Current()), false).GetType())
    {
      case GeomAbs_Plane:
      case GeomAbs_Cylinder:
      case GeomAbs_Cone:
      case GeomAbs_Sphere:
      case GeomAbs_Torus:
        break;
      default:
        return false;
    }
  return true;
}

GProp_GProps volume_props(const TopoDS_Shape& s)
{
  TopoDS_Compound          rest;
  std::vector<TopoDS_Face> wrapping;
  // Faces OCCT doesn't integrate (wrapping, with gaps) or integrates loosely
  // (B-spline, extruded and other non-analytic surfaces: a conic fillet
  // +0.0013 mm³, a variable fillet -0.0165, a curvature fillet +0.030 of the
  // exact volume, live oracle F4-F6): every face by the segmented Gauss
  // integral.
  if (!has_mesh_faces(s) && (split_wrapping(s, rest, wrapping) || !analytic_only(s)))
  {
    // Every face by the same divergence field: OCCT's per-face terms use
    // other fields, so they can't be mixed with these. The field is centred
    // on the shape's box: with gaps between faces (a tolerant import) the
    // result depends on that centre, and so doesn't change when the part
    // moves.
    Bnd_Box box;
    BRepBndLib::Add(s, box);
    gp_XYZ o;
    if (!box.IsVoid())
    {
      double a0, b0, c0, a1, b1, c1;
      box.Get(a0, b0, c0, a1, b1, c1);
      o = gp_XYZ((a0 + a1) / 2, (b0 + b1) / 2, (c0 + c1) / 2);
    }
    Moments m{};
    for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
    {
      const Moments f = face_moments(TopoDS::Face(e.Current()), true, o);
      for (int j = 0; j < 10; ++j)
        m[(size_t)j] += f[(size_t)j];
    }
    return MomentProps(unshift(m, o));
  }
  // Mesh faces: OCCT integrates their triangulation (exact for a mesh).
  GProp_GProps out;
  if (has_mesh_faces(s))
  {
    BRepGProp::VolumeProperties(s, out);
    return out;
  }
  // Analytic faces only: the adaptive integrator, whose error is controlled
  // there (a Ø6 hole through a Ø40 cylinder: plain Gauss −0.41 mm³).
  BRepGProp::VolumeProperties(s, out, 1e-7, true);
  return out;
}

// For the mass properties shown (centre of mass, inertia): OCCT's adaptive
// integrator controls the error of the mass only, and at volume_props'
// 1e-7 a revolved tube's Ixy read -870 mm⁵ for 0 and its Ixx / Iyy differed
// by 1.7e-5 (qa MASS-INERTIA-APPROX); at 1e-12 they are exact to 1e-10. Not
// for every volume: 1e-12 costs ~0.85 ms more a face (1e-10 on big parts).
GProp_GProps volume_props_precise(const TopoDS_Shape& s)
{
  TopoDS_Compound          rest;
  std::vector<TopoDS_Face> wrapping;
  if (has_mesh_faces(s) || split_wrapping(s, rest, wrapping) || !analytic_only(s))
    return volume_props(s);
  int faces = 0;
  for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
    ++faces;
  GProp_GProps out;
  BRepGProp::VolumeProperties(s, out, faces <= 1000 ? 1e-12 : 1e-10, true);
  return out;
}

GProp_GProps surface_props(const TopoDS_Shape& s)
{
  GProp_GProps out;
  if (has_mesh_faces(s))
  {
    BRepGProp::SurfaceProperties(s, out); // the triangulation's area
    return out;
  }
  TopoDS_Compound          rest;
  std::vector<TopoDS_Face> wrapping;
  split_wrapping(s, rest, wrapping);
  // OCCT's adaptive integrator controls its error on analytic faces only: on
  // extruded B-spline sections it is far off (a conic fillet, rho 0.7:
  // -0.235 mm² of 8747.53; a curvature fillet: -0.915 mm²; live's bounds are
  // ±0.2). Those faces take the segmented Gauss integral too.
  TopoDS_Compound analytic;
  BRep_Builder    B;
  B.MakeCompound(analytic);
  for (TopoDS_Iterator it(rest); it.More(); it.Next())
    if (analytic_only(it.Value()))
      B.Add(analytic, it.Value());
    else
      wrapping.push_back(TopoDS::Face(it.Value()));
  out = GProp_GProps(gp::Origin());
  if (analytic.NbChildren())
    BRepGProp::SurfaceProperties(analytic, out, 1e-7, true);
  for (const TopoDS_Face& f : wrapping)
    out.Add(MomentProps(face_moments(f, false)));
  return out;
}

bool self_intersects(const OcShape& s)
{
  if (s.shape.IsNull())
    return false;
  BOPAlgo_ArgumentAnalyzer a;
  a.SetShape1(s.shape);
  a.ArgumentTypeMode()   = false;
  a.SelfInterMode()      = true;
  a.SmallEdgeMode()      = false;
  a.RebuildFaceMode()    = false;
  a.TangentMode()        = false;
  a.MergeVertexMode()    = false;
  a.MergeEdgeMode()      = false;
  a.ContinuityMode()     = false;
  a.CurveOnSurfaceMode() = false;
  a.Perform();
  return a.HasFaulty();
}

std::unique_ptr<OcShape> fix_shape(const OcShape& s, double tolerance)
{
  require_non_null(s.shape, "shape");
  occ::handle<ShapeFix_Shape> fix = new ShapeFix_Shape(s.shape);
  if (tolerance > 0)
  {
    fix->SetPrecision(tolerance);
    fix->SetMaxTolerance(std::max(tolerance * 10.0, 1e-3));
  }
  fix->Perform();
  return wrap(fix->Shape());
}

rust::String dump_shape(const OcShape& s)
{
  std::ostringstream os;
  if (s.shape.IsNull())
    os << "<null shape>";
  else
    BRepTools::Dump(s.shape, os);
  return rust::String(os.str());
}

} // namespace noshape

namespace noshape {
thread_local int locked_sections = 0;
thread_local int mutex_holds     = 0;
} // namespace noshape

#if defined(__SANITIZE_ADDRESS__) && defined(__linux__) && defined(__x86_64__)
  #include <sanitizer/asan_interface.h>
namespace noshape {
// AddressSanitizer builds (the GUARD-SEGV-MALLOC-DEADLOCK hunt): a guard or
// budget jump abandons frames whose stack poisoning ASan can't clear on a
// big thread stack (the regen thread's 256 MB: "ignoring requested
// __asan_handle_no_return"), and their reuse then reads as a false
// stack-use-after-scope. Unpoisoned from the interrupted stack pointer up.
void asan_unpoison_abandoned(const void* uctx)
{
  const ucontext_t* uc = static_cast<const ucontext_t*>(uctx);
  if (uc == nullptr)
    return;
  const uintptr_t sp = (uintptr_t)uc->uc_mcontext.gregs[REG_RSP];
  pthread_attr_t  a;
  if (pthread_getattr_np(pthread_self(), &a) != 0)
    return;
  void*  base = nullptr;
  size_t size = 0;
  pthread_attr_getstack(&a, &base, &size);
  pthread_attr_destroy(&a);
  const uintptr_t hi = (uintptr_t)base + size;
  if (sp >= (uintptr_t)base && sp < hi)
    __asan_unpoison_memory_region((void*)sp, hi - sp);
}
} // namespace noshape
#else
namespace noshape {
inline void asan_unpoison_abandoned(const void*) {}
} // namespace noshape
#endif

#ifdef __linux__
// Every pthread mutex the program's own code (OCCT, the bridge, std::mutex)
// takes is counted per thread, and every pthread_once it runs: while one is
// held the CPU budget doesn't jump and the crash guard doesn't resume (qa
// GUARD-SEGV-MALLOC-DEADLOCK: the v0.4.13 CLI waited on a mutex it owned
// itself). These definitions take the executable's references; the C
// library's are called through dlsym(RTLD_NEXT). Shared libraries keep
// theirs.
namespace {
using MutexFn = int (*)(pthread_mutex_t*);
using TimedFn = int (*)(pthread_mutex_t*, const struct timespec*);
using ClockFn = int (*)(pthread_mutex_t*, clockid_t, const struct timespec*);
using OnceFn  = int (*)(pthread_once_t*, void (*)());
std::atomic<MutexFn> real_lock{nullptr}, real_trylock{nullptr}, real_unlock{nullptr};
std::atomic<TimedFn> real_timedlock{nullptr};
std::atomic<ClockFn> real_clocklock{nullptr};
std::atomic<OnceFn>  real_once{nullptr};

template <class F>
F next_symbol(std::atomic<F>& slot, const char* name)
{
  F f = slot.load(std::memory_order_acquire);
  if (f == nullptr)
  {
    f = reinterpret_cast<F>(dlsym(RTLD_NEXT, name));
    slot.store(f, std::memory_order_release);
  }
  return f;
}

__attribute__((constructor(101))) void resolve_mutex_functions()
{
  next_symbol(real_lock, "pthread_mutex_lock");
  next_symbol(real_trylock, "pthread_mutex_trylock");
  next_symbol(real_unlock, "pthread_mutex_unlock");
  next_symbol(real_timedlock, "pthread_mutex_timedlock");
  next_symbol(real_clocklock, "pthread_mutex_clocklock");
  next_symbol(real_once, "pthread_once");
}
} // namespace

extern "C" int pthread_mutex_lock(pthread_mutex_t* m)
{
  const int r = next_symbol(real_lock, "pthread_mutex_lock")(m);
  if (r == 0)
    ++noshape::mutex_holds;
  return r;
}

extern "C" int pthread_mutex_trylock(pthread_mutex_t* m)
{
  const int r = next_symbol(real_trylock, "pthread_mutex_trylock")(m);
  if (r == 0)
    ++noshape::mutex_holds;
  return r;
}

extern "C" int pthread_mutex_timedlock(pthread_mutex_t* m, const struct timespec* t)
{
  const int r = next_symbol(real_timedlock, "pthread_mutex_timedlock")(m, t);
  if (r == 0)
    ++noshape::mutex_holds;
  return r;
}

extern "C" int pthread_mutex_clocklock(pthread_mutex_t* m, clockid_t c, const struct timespec* t)
{
  const int r = next_symbol(real_clocklock, "pthread_mutex_clocklock")(m, c, t);
  if (r == 0)
    ++noshape::mutex_holds;
  return r;
}

extern "C" int pthread_mutex_unlock(pthread_mutex_t* m)
{
  const int r = next_symbol(real_unlock, "pthread_mutex_unlock")(m);
  if (r == 0 && noshape::mutex_holds > 0)
    --noshape::mutex_holds;
  return r;
}

extern "C" int pthread_once(pthread_once_t* o, void (*init)())
{
  ++noshape::mutex_holds;
  const int r = next_symbol(real_once, "pthread_once")(o, init);
  --noshape::mutex_holds;
  return r;
}
#endif

#ifndef _WIN32
namespace noshape {

thread_local sigjmp_buf* crash_guard  = nullptr;
thread_local int         crash_signal = 0;

namespace {
struct sigaction g_previous[32];

// Set by the terminate handler just before its abort().
thread_local volatile bool terminating = false;
std::terminate_handler     g_previous_terminate = nullptr;

// The C library's code (where malloc / free run), found at install.
uintptr_t g_libc_lo = 0, g_libc_hi = 0;
bool      g_guard_trace = false;

// The faulting instruction (Linux; 0 elsewhere: no C library check there).
uintptr_t fault_pc([[maybe_unused]] const void* ctx)
{
#if defined(__linux__) && (defined(__x86_64__) || defined(__aarch64__))
  const ucontext_t* uc = static_cast<const ucontext_t*>(ctx);
  if (uc == nullptr)
    return 0;
  #if defined(__x86_64__)
  return (uintptr_t)uc->uc_mcontext.gregs[REG_RIP];
  #else
  return (uintptr_t)uc->uc_mcontext.pc;
  #endif
#else
  return 0;
#endif
}

void on_crash(int sig, siginfo_t* info, void* ctx)
{
  if (sigjmp_buf* env = crash_guard)
  {
    // A fault inside the C library may hold malloc's arena lock (or its heap
    // is corrupt: glibc's own abort): after a jump the next allocation waits
    // forever (qa GUARD-SEGV-MALLOC-DEADLOCK). Not recovered: the process
    // fails as without the guard, at once rather than frozen.
    // (A signal raised on purpose, raise() / abort(), si_code <= 0, is
    // delivered in the C library too: still recovered.)
    // Nor one while this thread holds a kernel lock (LockedSection).
    const uintptr_t pc   = fault_pc(ctx);
    const bool      libc = info != nullptr && info->si_code > 0 && pc != 0 && pc >= g_libc_lo && pc < g_libc_hi;
    // An abort() neither: glibc's own heap checks ("free(): invalid
    // pointer", ...) abort from inside malloc, possibly with its lock held.
    // (std::terminate's abort, an OCCT exception through a noexcept frame,
    // stays recovered: its handler marks it.)
    const bool abort_raw = sig == SIGABRT && !terminating;
    terminating          = false;
    if (libc || abort_raw || locked_sections > 0 || mutex_holds > 0)
    {
      static const char in_libc[]  = "noshape: the kernel crashed inside the C library; not recoverable\n";
      static const char in_lock[]  = "noshape: the kernel crashed holding a lock; not recoverable\n";
      static const char aborted[]  = "noshape: the kernel aborted; not recoverable\n";
      [[maybe_unused]] const ssize_t w = libc               ? write(2, in_libc, sizeof in_libc - 1)
                                         : abort_raw      ? write(2, aborted, sizeof aborted - 1)
                                                          : write(2, in_lock, sizeof in_lock - 1);
      crash_guard = nullptr;
#ifdef __linux__
      // NOSHAPE_GUARD_TRACE=1: where (diagnosis; not async-signal-safe, the
      // process is ending anyway).
      if (g_guard_trace)
      {
        void*     frames[64];
        const int n = backtrace(frames, 64);
        backtrace_symbols_fd(frames, n, 2);
      }
#endif
      signal(sig, SIG_DFL);
      raise(sig);
      return;
    }
    crash_guard  = nullptr;
    crash_signal = sig;
    asan_unpoison_abandoned(ctx);
#ifdef __linux__
    // NOSHAPE_GUARD_TRACE=1: the recovered ones too (where the kernel faults).
    if (g_guard_trace)
    {
      static const char head[] = "noshape: the kernel crashed; recovered:\n";
      [[maybe_unused]] const ssize_t w = write(2, head, sizeof head - 1);
      void*     frames[64];
      const int n = backtrace(frames, 64);
      backtrace_symbols_fd(frames, n, 2);
    }
#endif
    siglongjmp(*env, 1);
  }
  // Not in a kernel call: the previous handler, or the default action
  // (returning re-raises the fault).
  const struct sigaction& prev = g_previous[sig];
  if (prev.sa_flags & SA_SIGINFO)
  {
    if (prev.sa_sigaction != nullptr)
    {
      prev.sa_sigaction(sig, info, ctx);
      return;
    }
  }
  else if (prev.sa_handler == SIG_IGN)
    return;
  else if (prev.sa_handler != SIG_DFL && prev.sa_handler != nullptr)
  {
    prev.sa_handler(sig);
    return;
  }
  signal(sig, SIG_DFL);
}
} // namespace

void install_crash_guard()
{
  static std::once_flag once;
  const LockedSection locked_section_1492;
  std::call_once(once, [] {
    if (std::getenv("NOSHAPE_NO_CRASH_GUARD") != nullptr)
      return;
    g_guard_trace = std::getenv("NOSHAPE_GUARD_TRACE") != nullptr;
#ifdef __linux__
    Dl_info di{};
    if (dladdr(reinterpret_cast<void*>(&malloc), &di) != 0 && di.dli_fbase != nullptr)
    {
      struct Find
      {
        const void* base;
        uintptr_t   lo, hi;
      } find{di.dli_fbase, 0, 0};
      dl_iterate_phdr(
        [](dl_phdr_info* info, size_t, void* data) {
          Find* f = static_cast<Find*>(data);
          if ((const void*)info->dlpi_addr != f->base)
            return 0;
          for (int k = 0; k < info->dlpi_phnum; ++k)
          {
            const ElfW(Phdr)& ph = info->dlpi_phdr[k];
            if (ph.p_type == PT_LOAD && (ph.p_flags & PF_X))
            {
              f->lo = info->dlpi_addr + ph.p_vaddr;
              f->hi = f->lo + ph.p_memsz;
            }
          }
          return 1;
        },
        &find);
      g_libc_lo = find.lo;
      g_libc_hi = find.hi;
    }
#endif
    g_previous_terminate = std::set_terminate([] {
      terminating = true;
      if (g_previous_terminate != nullptr)
        g_previous_terminate();
      std::abort();
    });
    for (int sig : {SIGSEGV, SIGBUS, SIGFPE, SIGILL, SIGABRT})
    {
      struct sigaction sa;
      std::memset(&sa, 0, sizeof sa);
      sa.sa_sigaction = on_crash;
      sa.sa_flags     = SA_SIGINFO | SA_ONSTACK | SA_NODEFER;
      sigemptyset(&sa.sa_mask);
      sigaction(sig, &sa, &g_previous[sig]);
    }
  });
}

} // namespace noshape
#endif

namespace noshape {
void crash_inside_kernel(uint8_t kind)
{
#ifdef _WIN32
  // Real faults: Windows' raise() goes to the C runtime's signals, not to
  // structured exceptions.
  volatile int  zero = 0;
  volatile int* null = nullptr;
  if (kind == 1)
    zero = 1 / zero;
  else if (kind == 0)
    *null = 1;
#endif
  if (kind == 1)
    std::raise(SIGFPE);
  else if (kind == 2)
    // An OCCT exception through a noexcept frame (OCCT 8's gp_Dir of a zero
    // vector): std::terminate, abort.
    [](double x) noexcept { (void)gp_Dir(x, 0, 0); }(0.0);
  else if (kind == 8)
  {
    // A fault inside a budgeted op (qa CLI-SUPPRESS-SEGV): the crash guard
    // takes it past run_budgeted's frame.
    run_budgeted(5.0, [] { std::raise(SIGSEGV); });
  }
  else if (kind == 3)
  {
    // A loop that never ends, under a 0.3 s budget (run_budgeted).
    volatile double x = 0;
    if (run_budgeted(0.3, [&] {
          for (;;)
            x = x + 1;
        }) == 1)
      fail_op("spin took too long");
  }
  else if (kind == 4)
  {
    // Endless allocation under short budgets (qa BUDGET-LONGJMP): a jump
    // out of malloc / free would leave the heap locked; every budget must
    // end with the heap usable.
    for (int i = 0; i < 10; ++i)
      run_budgeted(0.05, [] {
        for (;;)
        {
          std::vector<double>* v = new std::vector<double>(64 + (size_t)std::rand() % 4096, 1.0);
          delete v;
        }
      });
    std::vector<double> after(1 << 16, 2.0);
    if (after.back() == 2.0)
      fail_op("allocation took too long");
  }
#ifdef __linux__
  else if (kind == 5)
  {
    // A budget running out inside an OCCT thread pool loop (qa
    // BOP-PAIRS-GPF): it stops the work after the loop, never inside it.
    // A pool of its own: the default one may have no thread free (other
    // tests' parallel Booleans), and a job of this thread alone is a serial
    // loop the budget does stop (kind 7).
    static const occ::handle<OSD_ThreadPool> pool = new OSD_ThreadPool(4);
    std::atomic<int>                         done{0};
    const int                                n = 4096;
    OSD_ThreadPool::Launcher                 launcher(*pool, 4);
    const int                                reason = run_budgeted(0.05, [&] {
      launcher.Perform(0, n, [&](int, int i) {
        // Arithmetic only (a clock read would put the pc in the vDSO, where
        // the budget never jumps): a few ms each.
        volatile double x = i;
        for (int k = 0; k < 1000000; ++k)
          x = x * 1.0000001 + 1e-9;
        ++done;
      });
      volatile double y = 0;
      for (long k = 0; k < 40000000000L; ++k)
        y = y * 1.0000001 + 1e-9;
    });
    fail_op("budget " + std::to_string(reason) + " after " + std::to_string(done.load()) + " of " + std::to_string(n));
  }
  else if (kind == 10)
  {
    // A fault holding a kernel lock: not recovered either.
    run_guarded([&] {
      const LockedSection held;
      std::raise(SIGSEGV);
    });
    fail_op("survived a fault holding a lock");
  }
  else if (kind == 12)
  {
    // The guard and malloc (qa GUARD-SEGV-MALLOC-DEADLOCK): a heap chunk's
    // header overwritten, then freed inside a guarded call: glibc aborts from
    // inside free(). Recovered, the next allocation could wait forever on the
    // arena lock; not recovered, the process dies of SIGABRT.
    run_guarded([&] {
      void* const              p    = std::malloc(64);
      volatile const uintptr_t head = reinterpret_cast<uintptr_t>(p) - sizeof(size_t);
      *reinterpret_cast<volatile size_t*>(head) = 0x12345;
      std::free(p);
    });
    fail_op("survived freeing a corrupt chunk");
  }
  else if (kind == 11)
  {
    // A fault holding a plain mutex (as OCCT's own): not recovered.
    static std::mutex m;
    run_guarded([&] {
      std::lock_guard<std::mutex> g(m);
      std::raise(SIGSEGV);
    });
    fail_op("survived a fault holding a mutex");
  }
  else if (kind == 9)
  {
    // A fault inside the C library (memset through a null pointer): not
    // recovered, the process dies (GUARD-SEGV-MALLOC-DEADLOCK).
    void* (*volatile set)(void*, int, size_t) = &memset;
    run_guarded([&] { set(nullptr, 0, 64); });
    fail_op("survived a fault inside the C library");
  }
  else if (kind == 7)
  {
    // A pool job of this thread alone (as OCCT's serial BOPTools_Parallel
    // loops are): the budget still stops it inside.
    std::atomic<int>         done{0};
    OSD_ThreadPool::Launcher alone(*OSD_ThreadPool::DefaultPool(), 1);
    const int                reason = run_budgeted(0.05, [&] {
      alone.Perform(0, 4, [&](int, int i) {
        volatile double x = i;
        for (long k = 0; k < 2000000000L; ++k)
          x = x * 1.0000001 + 1e-9;
        ++done;
      });
    });
    fail_op("budget " + std::to_string(reason) + " after " + std::to_string(done.load()) + " of 4");
  }
#endif
  else
    std::raise(SIGSEGV);
}
} // namespace noshape

namespace noshape {
// NOSHAPE_DUMP_OPS=<dir>: each dumped op writes <dir>/<name>-<n>.brep (its
// input shape) and .txt (the indices of its sub-shape arguments in the
// shape's face / edge maps, then its parameters), so a crash found in a
// document becomes a kernel test (crates/kernel/tests/crash_inputs).
void dump_op(const char* name, const TopoDS_Shape& s, const std::vector<TopoDS_Shape>& subs, const std::string& params)
{
  const char* dir = std::getenv("NOSHAPE_DUMP_OPS");
  if (dir == nullptr || s.IsNull())
    return;
  static std::atomic<int> seq{0};
  const std::string       base = std::string(dir) + "/" + name + "-" + std::to_string(seq++);
  BRepTools::Write(s, (base + ".brep").c_str());
  ShapeIMap faces, edges;
  TopExp::MapShapes(s, TopAbs_FACE, faces);
  TopExp::MapShapes(s, TopAbs_EDGE, edges);
  std::ostringstream out;
  out.precision(17);
  for (const TopoDS_Shape& x : subs)
  {
    if (x.ShapeType() == TopAbs_FACE)
      out << "face " << faces.FindIndex(x) - 1 << "\n";
    else if (x.ShapeType() == TopAbs_EDGE)
      out << "edge " << edges.FindIndex(x) - 1 << "\n";
  }
  out << params << "\n";
  std::ofstream((base + ".txt").c_str()) << out.str();
}
} // namespace noshape

#if defined(__GNUC__) || defined(__clang__)
  #include <cxxabi.h>
#endif
namespace noshape {
namespace {
struct KernelProfile
{
  std::mutex                                                  lock;
  std::map<std::string, std::pair<long, double>>              calls; // name -> (count, ms)
  void print(const std::string& label)
  {
    std::vector<std::pair<std::string, std::pair<long, double>>> rows(calls.begin(), calls.end());
    std::sort(rows.begin(), rows.end(), [](const auto& a, const auto& b) { return a.second.second > b.second.second; });
    std::fprintf(stderr, "kernel profile (NOSHAPE_KERNEL_PROFILE)%s:\n", label.c_str());
    for (const auto& [name, v] : rows)
      std::fprintf(stderr, "  %9.1f ms %7ld  %s\n", v.second, v.first, name.c_str());
  }
  ~KernelProfile() { print(""); }
};
KernelProfile& kernel_profile()
{
  static KernelProfile p;
  return p;
}
// The bridge function's name from its lambda's type ("...$op_fillet(...)::<lambda()>").
std::string bridge_name(const char* mangled)
{
#if defined(__GNUC__) || defined(__clang__)
  int   st  = 0;
  char* dem = abi::__cxa_demangle(mangled, nullptr, nullptr, &st);
  std::string n = dem != nullptr ? dem : mangled;
  std::free(dem);
#else
  // MSVC's type names are already readable (no <cxxabi.h> there).
  std::string n = mangled;
#endif
  const size_t paren = n.find('(');
  size_t       end   = paren == std::string::npos ? n.size() : paren;
  size_t       start = n.rfind('$', end);
  return start == std::string::npos ? n.substr(0, end) : n.substr(start + 1, end - start - 1);
}
} // namespace

bool kernel_profile_on()
{
  static const bool on = std::getenv("NOSHAPE_KERNEL_PROFILE") != nullptr;
  return on;
}

std::chrono::steady_clock::duration kernel_profile_now()
{
#ifdef __linux__
  // cpu: this thread's CPU time; proc: the process's (OCCT's pool threads
  // too; run one test at a time).
  static const int clock = [] {
    const char* v = std::getenv("NOSHAPE_KERNEL_PROFILE");
    const std::string m = v != nullptr ? v : "";
    return m == "cpu" ? (int)CLOCK_THREAD_CPUTIME_ID : m == "proc" ? (int)CLOCK_PROCESS_CPUTIME_ID : -1;
  }();
  if (clock >= 0)
  {
    timespec ts{};
    clock_gettime((clockid_t)clock, &ts);
    return std::chrono::seconds(ts.tv_sec) + std::chrono::nanoseconds(ts.tv_nsec);
  }
#endif
  return std::chrono::steady_clock::now().time_since_epoch();
}

void kernel_profile_add_named(const char* name, std::chrono::steady_clock::duration took)
{
  KernelProfile&              p = kernel_profile();
  const LockedSection locked_section_1751;
  std::lock_guard<std::mutex> g(p.lock);
  auto&                       v = p.calls[std::string("  ") + name];
  v.first += 1;
  v.second += std::chrono::duration<double, std::milli>(took).count();
}

void kernel_profile_mark(rust::Str label)
{
  if (!kernel_profile_on())
    return;
  KernelProfile&              p = kernel_profile();
  const LockedSection locked_section_1762;
  std::lock_guard<std::mutex> g(p.lock);
  p.print(" " + std::string(label));
  p.calls.clear();
}

void kernel_profile_add(const char* what, std::chrono::steady_clock::duration took)
{
  KernelProfile&              p = kernel_profile();
  const LockedSection locked_section_1770;
  std::lock_guard<std::mutex> g(p.lock);
  auto&                       v = p.calls[bridge_name(what)];
  v.first += 1;
  v.second += std::chrono::duration<double, std::milli>(took).count();
}
} // namespace noshape

#ifdef __linux__
  #include <ctime>
  #include <unistd.h>
  #include <sys/syscall.h>
namespace noshape {
thread_local sigjmp_buf*              budget_jmp    = nullptr;
thread_local double                   budget_limit  = 0;
thread_local int                      budget_reason = 0;
thread_local const std::atomic<bool>* budget_cancel = nullptr;
thread_local int                      budget_hold   = 0;
thread_local void*                    budget_timer_live = nullptr;

double thread_cpu_seconds()
{
  timespec ts{};
  clock_gettime(CLOCK_THREAD_CPUTIME_ID, &ts);
  return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}

namespace {
const int budget_signal = SIGRTMIN + 3;

// Whether the interrupted instruction is the program's own code (OCCT and
// the bridge are linked in statically), not a shared library's: the C
// library's malloc / free (their arena lock), libstdc++, libm, the dynamic
// linker. A jump out of those left the arena locked (qa BUDGET-LONGJMP: a
// later free() waited forever) or the heap half updated (likely
// BOOL-THREAD-CRASH's late "silent" deaths); the next tick, 100 ms of CPU
// later, finds the thread back in OCCT.
extern "C" int noshape_osd_pool_depth(); // OCCT patch 0005 (OSD_ThreadPool.cxx)

bool in_own_code(const void* uctx)
{
  const ucontext_t* uc = static_cast<const ucontext_t*>(uctx);
  if (uc == nullptr)
    return false;
  #if defined(__x86_64__)
  const uintptr_t pc = (uintptr_t)uc->uc_mcontext.gregs[REG_RIP];
  #elif defined(__aarch64__)
  const uintptr_t pc = (uintptr_t)uc->uc_mcontext.pc;
  #else
  const uintptr_t pc = 0;
  #endif
  return pc >= (uintptr_t)&__executable_start && pc < (uintptr_t)&etext;
}

void on_budget_tick(int, siginfo_t*, void* uctx)
{
  sigjmp_buf* env = budget_jmp;
  // Not while this thread runs an OCCT thread pool job (its workers would go
  // on using the abandoned frames: qa BOP-PAIRS-GPF, BOOL-THREAD-CRASH):
  // OCCT patch 0005 counts them. The next tick after.
  if (env == nullptr || budget_hold > 0 || mutex_holds > 0 || noshape_osd_pool_depth() > 0 || !in_own_code(uctx))
    return;
  const bool over   = thread_cpu_seconds() > budget_limit;
  const bool cancel = budget_cancel != nullptr && budget_cancel->load(std::memory_order_relaxed);
  if (!over && !cancel)
    return;
  budget_jmp    = nullptr;
  budget_reason = over ? 1 : 2;
  asan_unpoison_abandoned(uctx);
  siglongjmp(*env, 1);
}
} // namespace

bool budget_timer_start(void*& timer)
{
  static std::once_flag once;
  const LockedSection locked_section_1844;
  std::call_once(once, [] {
    struct sigaction sa;
    std::memset(&sa, 0, sizeof sa);
    sa.sa_sigaction = on_budget_tick;
    sa.sa_flags     = SA_SIGINFO | SA_ONSTACK | SA_RESTART;
    sigemptyset(&sa.sa_mask);
    sigaction(budget_signal, &sa, nullptr);
  });
  sigevent sev;
  std::memset(&sev, 0, sizeof sev);
  sev.sigev_notify          = SIGEV_THREAD_ID;
  sev.sigev_signo           = budget_signal;
  sev._sigev_un._tid        = (pid_t)syscall(SYS_gettid);
  timer_t t;
  if (timer_create(CLOCK_THREAD_CPUTIME_ID, &sev, &t) != 0)
    return false;
  itimerspec its;
  std::memset(&its, 0, sizeof its);
  its.it_value.tv_nsec    = 100000000;
  its.it_interval.tv_nsec = 100000000;
  if (timer_settime(t, 0, &its, nullptr) != 0)
  {
    timer_delete(t);
    return false;
  }
  static_assert(sizeof(timer_t) <= sizeof(void*), "timer_t fits a pointer");
  std::memcpy(&timer, &t, sizeof t);
  return true;
}

void budget_timer_stop(void* timer)
{
  timer_t t;
  std::memcpy(&t, &timer, sizeof t);
  timer_delete(t);
}
} // namespace noshape
#endif

namespace noshape {
namespace {
std::atomic<bool> g_bop_parallel{std::getenv("NOSHAPE_BOP_PARALLEL") != nullptr};
}

void set_bop_parallel(bool on)
{
  g_bop_parallel.store(on);
}

void set_occt_threads(int32_t n)
{
  // Once, before any parallel work (the pool can't be resized under load).
  static std::once_flag once;
  const LockedSection locked_section_1897;
  std::call_once(once, [n] {
    if (n > 0)
      OSD_ThreadPool::DefaultPool()->Init(n);
  });
}

bool bop_parallel()
{
  // Serial unless NOSHAPE_BOP_PARALLEL=1 (qa BOOL-THREAD-CRASH: the seed-72
  // replay died in BOPAlgo_PaveFiller::ForceInterfEF on a pool thread in 1
  // of 3 runs in parallel, 0 of 3 serial; ~35% slower on that replay). Also
  // passed from Rust (`boolean`): on Windows the C runtime's getenv doesn't
  // see a variable set in the process later (boolean_budget's set_var).
  return g_bop_parallel.load();
}
} // namespace noshape

namespace noshape {
OwnCopy::OwnCopy(const OcShape& s) : in(s)
{
  if (s.shape.IsNull())
    return;
  BRepBuilderAPI_Copy cp(s.shape, false, false);
  if (!cp.IsDone())
    return;
  auto c = std::make_unique<OcShape>(cp.Shape());
  for (const TopAbs_ShapeEnum k : {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX})
    if (c->map(k).Extent() != s.map(k).Extent())
      return;
  copy = std::move(c);
}

OcShapeList OwnCopy::subs(const OcShapeList& l) const
{
  OcShapeList out;
  for (const TopoDS_Shape& x : l.items)
  {
    TopoDS_Shape y = x;
    if (copy && !x.IsNull())
    {
      const TopAbs_ShapeEnum k = x.ShapeType();
      if (k == TopAbs_FACE || k == TopAbs_EDGE || k == TopAbs_VERTEX)
      {
        const int i = in.map(k).FindIndex(x);
        if (i > 0)
        {
          y = copy->map(k)(i);
          y.Orientation(x.Orientation());
        }
      }
    }
    out.items.push_back(y);
    out.sources.push_back(nullptr);
  }
  return out;
}

} // namespace noshape
