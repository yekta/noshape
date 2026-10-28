// Hidden line removal for drawings: exact (HLRBRep_Algo) and polygonal
// (HLRBRep_PolyAlgo). Results are polylines in view coordinates with
// visibility, class and the index of the source edge when known.
#include "internal.hpp"

#include <BRepAdaptor_Curve.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
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
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>

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
    for (const TopoDS_Shape& f : ef(k))
      if (BRep_Tool::IsClosed(TopoDS::Edge(src(i)), TopoDS::Face(f)))
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
        todraw = edf.Rg1Line() && !edf.RgNLine() && !itf.OutLine();
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
