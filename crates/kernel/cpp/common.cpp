// Shapes, lists, topology exploration, validation and history building.
#include "internal.hpp"
#include <mutex>
#include <map>
#include <fstream>
#include <cstdlib>
#include <csignal>
#include <cstring>

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

const ShapeIMap& OcShape::map(TopAbs_ShapeEnum kind) const
{
  const int k = static_cast<int>(kind);
  std::call_once(myFlags[k], [&]() {
    if (!shape.IsNull())
      TopExp::MapShapes(shape, kind, myMaps[k]);
  });
  return myMaps[k];
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

HistoryBuilder::HistoryBuilder(const std::vector<TopoDS_Shape>& args, const TopoDS_Shape& output)
    : myArgs(args),
      myOut(output)
{
  for (const auto& a : myArgs)
    myArgMaps.push_back(std::make_unique<OcShape>(a));
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
  const int idx = myOut.map(ok).FindIndex(out);
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
          const int oi = myOut.map(k).FindIndex(s);
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
  OpOut o;
  o.shape   = wrap(myOut.shape);
  o.history = std::move(myRecs);
  return o;
}

OpOut make_result(const std::vector<TopoDS_Shape>& args, const TopoDS_Shape& out,
                  const BRepTools_History* h)
{
  HistoryBuilder hb(args, out);
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
  const TopAbs_ShapeEnum k  = to_kind(kind);
  const TopAbs_ShapeEnum ak = to_kind(ancestor_kind);
  const ShapeIMap&       km = s.map(k);
  const ShapeIMap&       am = s.map(ak);
  ShapeAncMap            anc;
  if (!s.shape.IsNull())
    TopExp::MapShapesAndAncestors(s.shape, k, ak, anc);
  Csr out;
  out.offsets.reserve(km.Extent() + 1);
  out.offsets.push_back(0);
  std::vector<uint32_t> row;
  for (int i = 1; i <= km.Extent(); ++i)
  {
    row.clear();
    const NCollection_List<TopoDS_Shape>* l = anc.Seek(km(i));
    if (l != nullptr)
    {
      for (NCollection_List<TopoDS_Shape>::Iterator it(*l); it.More(); it.Next())
      {
        const int ai = am.FindIndex(it.Value());
        if (ai > 0)
          row.push_back(static_cast<uint32_t>(ai - 1));
      }
    }
    std::sort(row.begin(), row.end());
    row.erase(std::unique(row.begin(), row.end()), row.end());
    for (uint32_t v : row)
      out.values.push_back(v);
    out.offsets.push_back(static_cast<uint32_t>(out.values.size()));
  }
  return out;
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
rust::Vec<uint8_t> shown_topology(const OcShape& s)
{
  const ShapeIMap& em = s.map(TopAbs_EDGE);
  const ShapeIMap& vm = s.map(TopAbs_VERTEX);
  ShapeAncMap      ef, ve, vf;
  if (!s.shape.IsNull())
  {
    TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, ef);
    TopExp::MapShapesAndAncestors(s.shape, TopAbs_VERTEX, TopAbs_EDGE, ve);
    TopExp::MapShapesAndAncestors(s.shape, TopAbs_VERTEX, TopAbs_FACE, vf);
  }
  rust::Vec<uint8_t> out;
  out.reserve((size_t)(em.Extent() + vm.Extent()));
  std::vector<bool> open(em.Extent() + 1, false);
  for (int i = 1; i <= em.Extent(); ++i)
  {
    const TopoDS_Edge& e     = TopoDS::Edge(em(i));
    const bool         shown = !BRep_Tool::Degenerated(e) && !is_seam(e, ef.Seek(e));
    TopoDS_Vertex      a, b;
    TopExp::Vertices(e, a, b);
    open[i] = shown && !a.IsSame(b);
    out.push_back(shown ? 1 : 0);
  }
  for (int i = 1; i <= vm.Extent(); ++i)
  {
    const NCollection_List<TopoDS_Shape>* es      = ve.Seek(vm(i));
    bool                                  shown   = es == nullptr || es->IsEmpty();
    bool                                  anyEdge = false;
    if (es != nullptr)
      for (NCollection_List<TopoDS_Shape>::Iterator it(*es); it.More() && !shown; it.Next())
      {
        const int j = em.FindIndex(it.Value());
        shown       = j > 0 && open[j];
        anyEdge     = anyEdge || (j > 0 && out[(size_t)j - 1] != 0);
      }
    if (!shown && !anyEdge)
      if (const NCollection_List<TopoDS_Shape>* fs = vf.Seek(vm(i)))
        for (NCollection_List<TopoDS_Shape>::Iterator it(*fs); it.More() && !shown; it.Next())
          shown = BRepAdaptor_Surface(TopoDS::Face(it.Value()), false).GetType() == GeomAbs_Cone;
    out.push_back(shown ? 1 : 0);
  }
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
void   list_push(OcShapeList& l, const OcShape& s) { l.items.push_back(s.shape); }
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

#ifndef _WIN32
namespace noshape {

thread_local sigjmp_buf* crash_guard  = nullptr;
thread_local int         crash_signal = 0;

namespace {
struct sigaction g_previous[32];

void on_crash(int sig, siginfo_t* info, void* ctx)
{
  if (sigjmp_buf* env = crash_guard)
  {
    crash_guard  = nullptr;
    crash_signal = sig;
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
  std::call_once(once, [] {
    if (std::getenv("NOSHAPE_NO_CRASH_GUARD") != nullptr)
      return;
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
  if (kind == 1)
    std::raise(SIGFPE);
  else if (kind == 2)
    // An OCCT exception through a noexcept frame (OCCT 8's gp_Dir of a zero
    // vector): std::terminate, abort.
    [](double x) noexcept { (void)gp_Dir(x, 0, 0); }(0.0);
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

#include <cxxabi.h>
namespace noshape {
namespace {
struct KernelProfile
{
  std::mutex                                                  lock;
  std::map<std::string, std::pair<long, double>>              calls; // name -> (count, ms)
  ~KernelProfile()
  {
    std::vector<std::pair<std::string, std::pair<long, double>>> rows(calls.begin(), calls.end());
    std::sort(rows.begin(), rows.end(), [](const auto& a, const auto& b) { return a.second.second > b.second.second; });
    std::fprintf(stderr, "kernel profile (NOSHAPE_KERNEL_PROFILE):\n");
    for (const auto& [name, v] : rows)
      std::fprintf(stderr, "  %9.1f ms %7ld  %s\n", v.second, v.first, name.c_str());
  }
};
KernelProfile& kernel_profile()
{
  static KernelProfile p;
  return p;
}
// The bridge function's name from its lambda's type ("...$op_fillet(...)::<lambda()>").
std::string bridge_name(const char* mangled)
{
  int   st  = 0;
  char* dem = abi::__cxa_demangle(mangled, nullptr, nullptr, &st);
  std::string n = dem != nullptr ? dem : mangled;
  std::free(dem);
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

void kernel_profile_add_named(const char* name, std::chrono::steady_clock::duration took)
{
  KernelProfile&              p = kernel_profile();
  std::lock_guard<std::mutex> g(p.lock);
  auto&                       v = p.calls[std::string("  ") + name];
  v.first += 1;
  v.second += std::chrono::duration<double, std::milli>(took).count();
}

void kernel_profile_add(const char* what, std::chrono::steady_clock::duration took)
{
  KernelProfile&              p = kernel_profile();
  std::lock_guard<std::mutex> g(p.lock);
  auto&                       v = p.calls[bridge_name(what)];
  v.first += 1;
  v.second += std::chrono::duration<double, std::milli>(took).count();
}
} // namespace noshape
