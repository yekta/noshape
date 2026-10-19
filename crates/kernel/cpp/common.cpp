// Shapes, lists, topology exploration, validation and history building.
#include "internal.hpp"

#include <atomic>
#include <cmath>

#include <BOPAlgo_ArgumentAnalyzer.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <GProp_GProps.hxx>
#include <BRep_Tool.hxx>
#include <Poly_Triangulation.hxx>
#include <Bnd_Box.hxx>
#include <BRepGProp.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepTools.hxx>
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
  BRepCheck_Analyzer a(s.shape);
  return a.IsValid();
}

double mesh_volume(const TopoDS_Shape& shape)
{
  TopoDS_Shape s = BRepBuilderAPI_Copy(shape).Shape();
  Bnd_Box      b;
  BRepBndLib::Add(s, b);
  const double size = b.IsVoid() ? 1.0 : std::sqrt(b.SquareExtent());
  BRepMesh_IncrementalMesh mesh(s, size * 2e-5, false, 0.05);
  double v = 0;
  for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    TopLoc_Location    loc;
    const occ::handle<Poly_Triangulation>& t = BRep_Tool::Triangulation(f, loc);
    if (t.IsNull())
      continue;
    for (int i = 1; i <= t->NbTriangles(); ++i)
    {
      int a, bb, c;
      t->Triangle(i).Get(a, bb, c);
      if (f.Orientation() == TopAbs_REVERSED)
        std::swap(bb, c);
      const gp_XYZ p = t->Node(a).Transformed(loc).XYZ(), q = t->Node(bb).Transformed(loc).XYZ(),
                   r = t->Node(c).Transformed(loc).XYZ();
      v += p.Dot(q.Crossed(r)) / 6.0;
    }
  }
  return v;
}

GProp_GProps volume_props(const TopoDS_Shape& s)
{
  // OCCT's two integrators fail on different surfaces: plain Gauss on rational
  // B-splines (a NURBS cylinder: +0.9 %), the adaptive one on strongly
  // rational extrusions (a conic fillet with weight 4: +4.5 % on the tool).
  // When they disagree, a fine mesh decides.
  GProp_GProps gauss, adaptive;
  BRepGProp::VolumeProperties(s, gauss);
  BRepGProp::VolumeProperties(s, adaptive, 1e-7, true);
  const double a = gauss.Mass(), b = adaptive.Mass();
  if (std::abs(a - b) <= 1e-7 * std::max({std::abs(a), std::abs(b), 1e-300}))
    return adaptive;
  const double m = mesh_volume(s);
  return std::abs(a - m) < std::abs(b - m) ? gauss : adaptive;
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
