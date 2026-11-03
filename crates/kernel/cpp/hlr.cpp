// Hidden line removal for drawings: exact (HLRBRep_Algo) and polygonal
// (HLRBRep_PolyAlgo). Results are polylines in view coordinates with
// visibility, class and the index of the source edge when known.
#include "internal.hpp"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <HLRAlgo_EdgeIterator.hxx>
#include <HLRAlgo_EdgeStatus.hxx>
#include <HLRAlgo_Projector.hxx>
#include <HLRBRep.hxx>
#include <HLRBRep_Algo.hxx>
#include <HLRBRep_Data.hxx>
#include <HLRBRep_EdgeData.hxx>
#include <HLRBRep_FaceData.hxx>
#include <HLRBRep_FaceIterator.hxx>
#include <HLRBRep_PolyAlgo.hxx>
#include <Bnd_Box.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>

#include <algorithm>
#include <atomic>
#include <numeric>
#include <thread>
#include <unordered_map>

namespace noshape {

namespace {

// Edge classes (must match crate::hlr::EdgeClass).
enum : uint8_t
{
  CLS_SHARP   = 0,
  CLS_SMOOTH  = 1,
  CLS_OUTLINE = 2,
  CLS_SEAM    = 3,
};

// Source edges by their 3D curve. HLR's outliner splits edges where a
// silhouette touches them (the rims of a slightly tilted hole); the pieces
// are new edges that keep the source edge's curve.
using CurveIndex = std::unordered_multimap<const Geom_Curve*, int>;

struct Emitter
{
  HlrOut&           out;
  const ShapeIMap&  srcEdges;
  const CurveIndex& curves;
  double            defl;
  // Source edges that are a face's seam (by source index).
  const std::vector<char>* seams = nullptr;

  bool seam(int src) const { return seams && src >= 0 && (size_t)src < seams->size() && (*seams)[src]; }

  void emit_points(const std::vector<gp_Pnt2d>& pts, bool visible, uint8_t cls, int source)
  {
    if (pts.size() < 2)
      return;
    HlrEdgeF e;
    e.visible = visible;
    e.kind    = cls;
    e.source  = source;
    e.start   = (uint32_t)(out.points.size() / 2);
    e.count   = (uint32_t)pts.size();
    for (const auto& p : pts)
    {
      out.points.push_back(p.X());
      out.points.push_back(p.Y());
    }
    out.edges.push_back(e);
  }

  void emit_edge(const TopoDS_Edge& e, bool visible, uint8_t cls, int source)
  {
    if (e.IsNull())
      return;
    std::vector<gp_Pnt2d> pts;
    try
    {
      BRepAdaptor_Curve c(e);
      if (c.GetType() == GeomAbs_Line)
      {
        gp_Pnt a = c.Value(c.FirstParameter()), b = c.Value(c.LastParameter());
        pts.emplace_back(a.X(), a.Y());
        pts.emplace_back(b.X(), b.Y());
      }
      else
      {
        GCPnts_TangentialDeflection d(c, 0.1, defl);
        for (int k = 1; k <= d.NbPoints(); ++k)
        {
          gp_Pnt p = d.Value(k);
          pts.emplace_back(p.X(), p.Y());
        }
      }
    }
    catch (const Standard_Failure&)
    {
      return;
    }
    emit_points(pts, visible, cls, source);
  }
};

CurveIndex curve_index(const ShapeIMap& src)
{
  CurveIndex m;
  for (int i = 1; i <= src.Extent(); ++i)
  {
    TopLoc_Location               loc;
    double                        f, l;
    const occ::handle<Geom_Curve> c = BRep_Tool::Curve(TopoDS::Edge(src.FindKey(i)), loc, f, l);
    if (!c.IsNull())
      m.emplace(c.get(), i - 1);
  }
  return m;
}

// The source edge of `s`: itself, else (a piece of a split edge) the source
// edge with the same curve and location. Copies sharing a curve (patterned
// instances) are told apart by location; still ambiguous gives none.
// The source edges that are the seam of a periodic face (used twice by it):
// not edges in Onshape's topology, so drawn only where they are the outline.
// A revolve's seam has no continuity flag, so HLR takes it as a sharp edge
// (qa DWG-REVOLVE-SEAM: a line at a revolved tube's axis in side views).
// Whether two faces lie on one surface: the same surface object, or the same
// plane, cylinder, cone, sphere or torus.
static bool same_surface(const TopoDS_Face& a, const TopoDS_Face& b)
{
  TopLoc_Location la, lb;
  if (BRep_Tool::Surface(a, la).get() == BRep_Tool::Surface(b, lb).get() && la.IsEqual(lb))
    return true;
  BRepAdaptor_Surface sa(a, false), sb(b, false);
  if (sa.GetType() != sb.GetType())
    return false;
  const double tol = 1e-7, ang = 1e-9;
  auto same_axis = [&](const gp_Ax1& x, const gp_Ax1& y) {
    return x.Direction().IsParallel(y.Direction(), ang) && gp_Lin(x).Distance(y.Location()) < tol;
  };
  switch (sa.GetType())
  {
  case GeomAbs_Plane:
  {
    const gp_Pln p = sa.Plane(), q = sb.Plane();
    return p.Axis().Direction().IsParallel(q.Axis().Direction(), ang) && p.Distance(q.Location()) < tol;
  }
  case GeomAbs_Cylinder:
    return same_axis(sa.Cylinder().Axis(), sb.Cylinder().Axis()) && std::abs(sa.Cylinder().Radius() - sb.Cylinder().Radius()) < tol;
  case GeomAbs_Cone:
    return same_axis(sa.Cone().Axis(), sb.Cone().Axis()) && sa.Cone().Apex().Distance(sb.Cone().Apex()) < tol &&
           std::abs(std::abs(sa.Cone().SemiAngle()) - std::abs(sb.Cone().SemiAngle())) < ang;
  case GeomAbs_Sphere:
    return sa.Sphere().Location().Distance(sb.Sphere().Location()) < tol && std::abs(sa.Sphere().Radius() - sb.Sphere().Radius()) < tol;
  case GeomAbs_Torus:
    return same_axis(sa.Torus().Axis(), sb.Torus().Axis()) && sa.Torus().Location().Distance(sb.Torus().Location()) < tol &&
           std::abs(sa.Torus().MajorRadius() - sb.Torus().MajorRadius()) < tol && std::abs(sa.Torus().MinorRadius() - sb.Torus().MinorRadius()) < tol;
  default:
    return false;
  }
}

// Edges that aren't edges in Onshape's topology: a face's own seam, and an
// edge between two faces of one surface (Parasolid keeps such a face whole;
// live 1857: no line across pipeflange-steven's fillets).
std::vector<char> seam_edges(const TopoDS_Shape& shape, const ShapeIMap& src)
{
  TopTools_IndexedDataMapOfShapeListOfShape ef;
  TopExp::MapShapesAndAncestors(shape, TopAbs_EDGE, TopAbs_FACE, ef);
  std::vector<char> out((size_t)src.Extent(), 0);
  for (int i = 1; i <= src.Extent(); ++i)
  {
    const int k = ef.FindIndex(src(i));
    if (k == 0)
      continue;
    const TopoDS_Edge& e = TopoDS::Edge(src(i));
    std::vector<TopoDS_Face> faces;
    for (const TopoDS_Shape& f : ef(k))
    {
      if (BRep_Tool::IsClosed(e, TopoDS::Face(f)))
        out[(size_t)i - 1] = 1;
      faces.push_back(TopoDS::Face(f));
    }
    if (faces.size() == 2 && !faces[0].IsSame(faces[1]) && same_surface(faces[0], faces[1]))
      out[(size_t)i - 1] = 1;
  }
  return out;
}

int source_index(const ShapeIMap& src, const CurveIndex& curves, const TopoDS_Shape& s)
{
  if (s.IsNull() || s.ShapeType() != TopAbs_EDGE)
    return -1;
  const int i = src.FindIndex(s);
  if (i > 0)
    return i - 1;
  TopLoc_Location               loc;
  double                        f, l;
  const occ::handle<Geom_Curve> c = BRep_Tool::Curve(TopoDS::Edge(s), loc, f, l);
  if (c.IsNull())
    return -1;
  const auto range = curves.equal_range(c.get());
  int        only  = -1, n = 0;
  for (auto it = range.first; it != range.second; ++it)
  {
    TopLoc_Location l2;
    double          f2, e2;
    BRep_Tool::Curve(TopoDS::Edge(src.FindKey(it->second + 1)), l2, f2, e2);
    if (l2.IsEqual(loc))
      return it->second;
    only = it->second;
    ++n;
  }
  return n == 1 ? only : -1;
}

void draw_edge_data(Emitter& em, HLRBRep_EdgeData& ed, bool visible, uint8_t cls, int source)
{
  double               sta, end;
  float                tolsta, tolend;
  HLRAlgo_EdgeIterator it;
  if (visible)
  {
    for (it.InitVisible(ed.Status()); it.MoreVisible(); it.NextVisible())
    {
      it.Visible(sta, tolsta, end, tolend);
      em.emit_edge(HLRBRep::MakeEdge(ed.Geometry(), sta, end), true, cls, source);
    }
  }
  else
  {
    for (it.InitHidden(ed.Status()); it.MoreHidden(); it.NextHidden())
    {
      it.Hidden(sta, tolsta, end, tolend);
      em.emit_edge(HLRBRep::MakeEdge(ed.Geometry(), sta, end), false, cls, source);
    }
  }
}

// Mirrors HLRBRep_HLRToShape::InternalCompound/DrawFace/DrawEdge for the
// sharp (5), outline (2), smooth (3) and sewn/seam (4) categories, recording
// the source edge.
void exact_pass(Emitter& em, const occ::handle<HLRBRep_Data>& DS, int typ, bool visible)
{
  const int ne = DS->NbEdges();
  const int nf = DS->NbFaces();
  for (int ie = 1; ie <= ne; ++ie)
  {
    HLRBRep_EdgeData& ed = DS->EDataArray().ChangeValue(ie);
    if (ed.Selected() && !ed.Vertical())
    {
      ed.Used(false);
      ed.HideCount(0);
    }
    else
      ed.Used(true);
  }
  auto src_of = [&](int ie) {
    if (ie < 1 || ie > DS->EdgeMap().Extent())
      return -1;
    return source_index(em.srcEdges, em.curves, DS->EdgeMap().FindKey(ie));
  };
  for (int iface = 1; iface <= nf; ++iface)
  {
    HLRBRep_FaceIterator itf;
    for (itf.InitEdge(DS->FDataArray().ChangeValue(iface)); itf.MoreEdge(); itf.NextEdge())
    {
      const int         ie  = itf.Edge();
      HLRBRep_EdgeData& edf = DS->EDataArray().ChangeValue(ie);
      if (edf.Used())
        continue;
      // A face's seam counts as smooth even without its continuity flag.
      const int  src    = src_of(ie);
      const bool smooth = edf.Rg1Line() || em.seam(src);
      bool       todraw;
      if (typ == 2)
        todraw = itf.Internal(); // silhouettes computed by the outliner
      else if (typ == 3)
        // A face's own seam isn't a tangent edge (Parasolid's periodic faces
        // have none; live 1857 draws no line there).
        todraw = edf.Rg1Line() && !edf.RgNLine() && !itf.OutLine() && !em.seam(src);
      else if (typ == 4)
        todraw = edf.RgNLine() && !itf.OutLine();
      else
        todraw = !itf.IsoLine() && !itf.Internal() && (!smooth || itf.OutLine());
      if (todraw)
      {
        uint8_t cls = typ == 2 ? CLS_OUTLINE : typ == 3 ? CLS_SMOOTH : (typ == 4 ? CLS_SEAM : CLS_SHARP);
        // Real edges lying on the silhouette (e.g. a cylinder seam seen
        // from the side) are outlines too but keep their source edge.
        if (typ == 5 && itf.OutLine() && smooth)
          cls = CLS_OUTLINE;
        draw_edge_data(em, edf, visible, cls, src);
        edf.Used(true);
      }
      else
      {
        if ((typ > 4 || typ == 2) && (smooth && !itf.OutLine()))
        {
          int hc = edf.HideCount();
          if (hc > 0)
            edf.Used(true);
          else
            edf.HideCount(hc + 1);
        }
        else
          edf.Used(true);
      }
    }
  }
  if (typ < 3)
    return;
  // Free edges (not on faces).
  for (int ie = 1; ie <= ne; ++ie)
  {
    HLRBRep_EdgeData& ed = DS->EDataArray().ChangeValue(ie);
    if (ed.Used())
      continue;
    bool todraw;
    if (typ == 3)
      todraw = ed.Rg1Line() && !ed.RgNLine();
    else if (typ == 4)
      todraw = ed.RgNLine();
    else
      todraw = !ed.Rg1Line();
    if (todraw)
    {
      uint8_t cls = typ == 3 ? CLS_SMOOTH : (typ == 4 ? CLS_SEAM : CLS_SHARP);
      draw_edge_data(em, ed, visible, cls, src_of(ie));
    }
    ed.Used(true);
  }
}

// Exact HLR of many bodies (a big assembly: one algo takes seconds to load
// them all): chunks of bodies near each other on the sheet, each hidden in
// its own algo, in parallel, by itself and by the bodies whose projected
// boxes overlap its own; a chunk draws its own bodies' edges. False if a
// chunk failed.
constexpr size_t kChunkedBodies = 64, kChunkBodies = 32;

bool exact_chunked(Emitter& em, const std::vector<TopoDS_Shape>& bodies, const HLRAlgo_Projector& proj, const gp_Ax2& ax)
{
  // Projected box; z toward the viewer.
  struct Box2
  {
    double x0 = 1e300, y0 = 1e300, z0 = 1e300, x1 = -1e300, y1 = -1e300, z1 = -1e300;
    bool   overlaps(const Box2& o) const { return x0 <= o.x1 && o.x0 <= x1 && y0 <= o.y1 && o.y0 <= y1; }
    // Whether o can hide part of this: overlapping, not wholly behind it.
    bool hidden_by(const Box2& o) const { return overlaps(o) && o.z1 >= z0; }
    void add(const Box2& o) { *this = {std::min(x0, o.x0), std::min(y0, o.y0), std::min(z0, o.z0), std::max(x1, o.x1), std::max(y1, o.y1), std::max(z1, o.z1)}; }
  };
  const size_t      n = bodies.size();
  std::vector<Box2> box(n);
  Box2              all;
  for (size_t i = 0; i < n; ++i)
  {
    Bnd_Box b;
    BRepBndLib::Add(bodies[i], b, false);
    if (b.IsVoid())
      continue;
    double c[6];
    b.Get(c[0], c[1], c[2], c[3], c[4], c[5]);
    for (int k = 0; k < 8; ++k)
    {
      const gp_XYZ p(c[(k & 1) ? 3 : 0], c[(k & 2) ? 4 : 1], c[(k & 4) ? 5 : 2]);
      const double x = p.Dot(ax.XDirection().XYZ()), y = p.Dot(ax.YDirection().XYZ()), z = p.Dot(ax.Direction().XYZ());
      box[i].add({x, y, z, x, y, z});
    }
    all.add(box[i]);
  }
  // Z-order of the box centres: chunks of neighbours.
  auto morton = [&](size_t i) {
    if (box[i].x0 > box[i].x1)
      return uint32_t(0);
    auto q = [](double v, double lo, double hi) { return (uint32_t)std::clamp((v - lo) / std::max(hi - lo, 1e-300) * 65535.0, 0.0, 65535.0); };
    const uint32_t x = q((box[i].x0 + box[i].x1) / 2, all.x0, all.x1), y = q((box[i].y0 + box[i].y1) / 2, all.y0, all.y1);
    uint32_t       m = 0;
    for (int b = 0; b < 16; ++b)
      m |= ((x >> b) & 1u) << (2 * b) | ((y >> b) & 1u) << (2 * b + 1);
    return m;
  };
  std::vector<uint32_t> key(n);
  for (size_t i = 0; i < n; ++i)
    key[i] = morton(i);
  std::vector<size_t> order(n);
  std::iota(order.begin(), order.end(), 0);
  std::stable_sort(order.begin(), order.end(), [&](size_t a, size_t b) { return key[a] < key[b]; });
  const size_t        hw     = std::max(1u, std::thread::hardware_concurrency());
  const size_t        chunks = std::clamp<size_t>(n / kChunkBodies, 1, 4 * hw);
  std::vector<HlrOut> outs(chunks);
  std::atomic<bool>   failed{false};
  // Own threads: OCCT's pool runs a nested or concurrent For (the other
  // views of a sheet, computed at the same time) on the calling thread.
  auto hide_chunk = [&](size_t c) {
    const std::vector<size_t> members(order.begin() + (ptrdiff_t)(c * n / chunks), order.begin() + (ptrdiff_t)((c + 1) * n / chunks));
    Box2                      cb;
    for (size_t m : members)
      cb.add(box[m]);
    std::vector<char> mine(n, 0);
    for (size_t m : members)
      mine[m] = 1;
    std::vector<size_t> others;
    for (size_t j = 0; j < n; ++j)
      if (!mine[j] && cb.hidden_by(box[j]) && std::any_of(members.begin(), members.end(), [&](size_t m) { return box[m].hidden_by(box[j]); }))
        others.push_back(j);
    try
    {
      BRep_Builder    bb;
      TopoDS_Compound own, around;
      bb.MakeCompound(own);
      bb.MakeCompound(around);
      for (size_t m : members)
        bb.Add(own, bodies[m]);
      for (size_t o : others)
        bb.Add(around, bodies[o]);
      occ::handle<HLRBRep_Algo> algo = new HLRBRep_Algo();
      algo->Add(own);
      if (!others.empty())
        algo->Add(around);
      algo->Projector(proj);
      algo->Update();
      algo->Hide(1);
      if (!others.empty())
        algo->Hide(1, 2);
      occ::handle<HLRBRep_Data> DS = algo->DataStructure();
      if (DS.IsNull())
        return;
      // Draw the chunk's own edges only.
      algo->Select(1);
      Emitter cem{outs[c], em.srcEdges, em.curves, em.defl, em.seams};
      DS->Projector().Scaled(true);
      for (bool visible : {true, false})
        for (int typ : {5, 2, 3, 4})
          exact_pass(cem, DS, typ, visible);
      DS->Projector().Scaled(false);
    }
    catch (const Standard_Failure&)
    {
      failed = true;
    }
  };
  std::atomic<size_t>      next{0};
  std::vector<std::thread> workers;
  for (size_t t = 0; t < std::min(chunks, hw); ++t)
    workers.emplace_back([&]() {
      for (size_t c = next++; c < chunks; c = next++)
        hide_chunk(c);
    });
  for (std::thread& t : workers)
    t.join();
  if (failed)
    return false;
  for (const HlrOut& o : outs)
  {
    const uint32_t base = (uint32_t)(em.out.points.size() / 2);
    for (double v : o.points)
      em.out.points.push_back(v);
    for (HlrEdgeF e : o.edges)
    {
      e.start += base;
      em.out.edges.push_back(e);
    }
  }
  return true;
}

} // namespace

HlrOut project_hlr(const OcShape& s, P3 dir, P3 up, bool exact, double deflection)
{
  require_non_null(s.shape, "shape");
  gp_Dir vd  = to_dir(dir, "view direction");
  gp_Dir upd = to_dir(up, "up direction");
  gp_Dir z   = vd.Reversed(); // toward the viewer
  if (z.IsParallel(upd, 1e-9))
    fail_invalid("up direction is parallel to the view direction");
  gp_Vec yv(upd.XYZ() - z.XYZ() * upd.XYZ().Dot(z.XYZ()));
  gp_Dir y(yv);
  gp_Dir x = y.Crossed(z);
  gp_Ax2 ax(gp_Pnt(0, 0, 0), z, x);
  HLRAlgo_Projector proj(ax);
  const double      defl = deflection > 0 ? deflection : 0.01;

  HlrOut    out;
  ShapeIMap srcEdges;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, srcEdges);
  const CurveIndex        curves = curve_index(srcEdges);
  const std::vector<char> seams  = seam_edges(s.shape, srcEdges);
  Emitter                 em{out, srcEdges, curves, defl, &seams};

  if (exact)
  {
    std::vector<TopoDS_Shape> bodies;
    if (s.shape.ShapeType() == TopAbs_COMPOUND)
      for (TopoDS_Iterator it(s.shape); it.More(); it.Next())
        bodies.push_back(it.Value());
    if (bodies.size() >= kChunkedBodies && exact_chunked(em, bodies, proj, ax))
      return out;
    occ::handle<HLRBRep_Algo> algo = new HLRBRep_Algo();
    algo->Add(s.shape);
    algo->Projector(proj);
    algo->Update();
    algo->Hide();
    occ::handle<HLRBRep_Data> DS = algo->DataStructure();
    if (DS.IsNull())
      return out;
    DS->Projector().Scaled(true);
    for (bool visible : {true, false})
      for (int typ : {5, 2, 3, 4})
        exact_pass(em, DS, typ, visible);
    DS->Projector().Scaled(false);
    return out;
  }

  // Polygonal HLR on the triangulation.
  {
    const LockedSection locked_section_328;
    std::lock_guard<std::mutex> lock(mesh_mutex());
    BRepMesh_IncrementalMesh    mesher(s.shape, defl, false, 0.25, true);
    (void)mesher;
  }
  occ::handle<HLRBRep_PolyAlgo> pa = new HLRBRep_PolyAlgo();
  pa->Load(s.shape);
  pa->Projector(proj);
  pa->Update();
  const gp_Trsf& T = pa->Projector().Transformation();

  struct Seg
  {
    gp_Pnt2d a, b;
    bool     visible;
    uint8_t  cls;
    int      source;
  };
  std::vector<Seg> segs;
  HLRAlgo_EdgeStatus status;
  TopoDS_Shape       S;
  bool               reg1, regn, outl, intl;
  for (pa->InitHide(); pa->MoreHide(); pa->NextHide())
  {
    HLRAlgo_BiPoint::PointsT& pts = pa->Hide(status, S, reg1, regn, outl, intl);
    // "Internal" poly lines are the silhouettes of the triangulation
    // (HLRBRep_PolyHLRToShape::OutLineVCompound).
    gp_XYZ a = pts.Pnt1, b = pts.Pnt2;
    T.Transforms(a);
    T.Transforms(b);
    gp_XY a2(a.X(), a.Y()), b2(b.X(), b.Y()), d = b2 - a2;
    if (d.Modulus() <= 1e-10)
      continue;
    // Segments of real edges keep their source; silhouettes have none.
    int     src = source_index(srcEdges, curves, S);
    uint8_t cls = CLS_SHARP;
    if (intl)
    {
      cls = CLS_OUTLINE;
      src = -1;
    }
    else if (src < 0)
      cls = outl ? CLS_OUTLINE : CLS_SHARP;
    else if ((reg1 || em.seam(src)) && outl)
      cls = CLS_OUTLINE;
    else if (regn || em.seam(src))
      cls = CLS_SEAM;
    else if (reg1)
      cls = CLS_SMOOTH;
    HLRAlgo_EdgeIterator it;
    double               sta, end;
    float                ts, te;
    for (it.InitVisible(status); it.MoreVisible(); it.NextVisible())
    {
      it.Visible(sta, ts, end, te);
      segs.push_back({gp_Pnt2d(a2 + sta * d), gp_Pnt2d(a2 + end * d), true, cls, src});
    }
    for (it.InitHidden(status); it.MoreHidden(); it.NextHidden())
    {
      it.Hidden(sta, ts, end, te);
      segs.push_back({gp_Pnt2d(a2 + sta * d), gp_Pnt2d(a2 + end * d), false, cls, src});
    }
  }
  // Chain consecutive segments of the same edge/visibility/class.
  const double          tol2 = 1e-18 + (defl * 1e-3) * (defl * 1e-3);
  std::vector<gp_Pnt2d> chain;
  Seg                   cur{};
  for (const Seg& sg : segs)
  {
    if (sg.a.SquareDistance(sg.b) <= 1e-20)
      continue;
    if (!chain.empty())
    {
      if (cur.visible == sg.visible && cur.cls == sg.cls && cur.source == sg.source &&
          chain.back().SquareDistance(sg.a) <= tol2)
      {
        chain.push_back(sg.b);
        continue;
      }
      em.emit_points(chain, cur.visible, cur.cls, cur.source);
      chain.clear();
    }
    cur = sg;
    chain.push_back(sg.a);
    chain.push_back(sg.b);
  }
  if (!chain.empty())
    em.emit_points(chain, cur.visible, cur.cls, cur.source);
  return out;
}

} // namespace noshape
