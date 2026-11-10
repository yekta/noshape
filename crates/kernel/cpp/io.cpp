// I/O: BREP (text, binary), STEP (with XCAF names/colors), IGES.
//
// All STEP/IGES exchange happens in OCCT's default session unit (millimeter);
// unit conversion is done by the Rust layer.
#include "internal.hpp"
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Precision.hxx>
#include <ShapeFix_Edge.hxx>
#include <TopExp_Explorer.hxx>
#include <algorithm>
#include <array>
#include <iterator>
#include <regex>

#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BinTools.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
#include <IGESControl_Reader.hxx>
#include <IGESCAFControl_Reader.hxx>
#include <Transfer_Binder.hxx>
#include <Transfer_TransientProcess.hxx>
#include <XSControl_TransferReader.hxx>
#include <XSControl_WorkSession.hxx>
#include <IGESControl_Writer.hxx>
#include <IGESData_GlobalSection.hxx>
#include <IGESData_IGESEntity.hxx>
#include <Interface_InterfaceModel.hxx>
#include <IGESData_IGESModel.hxx>
#include <IGESBasic_Name.hxx>
#include <IGESBasic_SingularSubfigure.hxx>
#include <IGESBasic_SubfigureDef.hxx>
#include <IGESGeom_TransformationMatrix.hxx>
#include <IGESGraph_Color.hxx>
#include <IGESSolid_EdgeList.hxx>
#include <IGESSolid_Face.hxx>
#include <IGESSolid_Loop.hxx>
#include <IGESSolid_ManifoldSolid.hxx>
#include <IGESSolid_Shell.hxx>
#include <IGESSolid_VertexList.hxx>
#include <TransferBRep.hxx>
#include <TransferBRep_ShapeMapper.hxx>
#include <Transfer_FinderProcess.hxx>
#include <TCollection_HAsciiString.hxx>
#include <ctime>
#include <Quantity_Color.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <chrono>
#include <cstdlib>
#include <DESTEP_Parameters.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_Writer.hxx>
#include <TDF_Label.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopoDS_Iterator.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDoc_ColorTool.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <XCAFDoc_DimTolTool.hxx>
#include <XCAFDoc_Dimension.hxx>
#include <XCAFDoc_Datum.hxx>
#include <XCAFDimTolObjects_DimensionObject.hxx>
#include <XCAFDimTolObjects_DatumObject.hxx>
#include <TCollection_HAsciiString.hxx>
#include <gp_Trsf.hxx>

#include <fstream>
#include <sstream>

namespace noshape {

namespace {

// Data exchange uses process-wide static parameters and a singleton XCAF
// application; serialize it.
std::mutex& xde_mutex()
{
  static std::mutex m;
  return m;
}

// OCCT data exchange prints progress/statistics to stdout through the default
// messenger; a library must not write to stdout, so remove its printers once.
void quiet_messenger()
{
  static std::once_flag once;
  const LockedSection locked_section_59;
  std::call_once(once, []() {
    const occ::handle<Message_Messenger>& m = Message::DefaultMessenger();
    if (!m.IsNull())
      m->RemovePrinters(STANDARD_TYPE(Message_PrinterOStream));
  });
}

std::string path_str(rust::Str p) { return std::string(p.data(), p.size()); }

std::string label_name(const TDF_Label& l)
{
  occ::handle<TDataStd_Name> n;
  if (!l.FindAttribute(TDataStd_Name::GetID(), n))
    return std::string();
  const TCollection_ExtendedString& es = n->Get();
  TCollection_AsciiString           as(es, '?');
  return std::string(as.ToCString());
}

bool label_color(const occ::handle<XCAFDoc_ColorTool>& ct, const TDF_Label& l,
                 const TopoDS_Shape& s, Quantity_Color& c)
{
  if (ct->GetColor(l, XCAFDoc_ColorSurf, c) || ct->GetColor(l, XCAFDoc_ColorGen, c))
    return true;
  if (!s.IsNull() && (ct->GetColor(s, XCAFDoc_ColorSurf, c) || ct->GetColor(s, XCAFDoc_ColorGen, c)))
    return true;
  return false;
}

struct Leaf
{
  TopoDS_Shape   shape;
  std::string    name;
  bool           hasColor = false;
  Quantity_Color color;
  // Faces with their own colour (TopExp::MapShapes order).
  std::vector<std::pair<int, Quantity_Color>> faceColors;
};

// The faces of `s` (TopExp::MapShapes order) with a colour of their own,
// from its label's sub-shape labels. (Looking each face up by shape searched
// the whole document: quadratic in the parts, 2.9 s of 4.3 s for 2000.)
std::vector<std::pair<int, Quantity_Color>> face_colors(const occ::handle<XCAFDoc_ColorTool>& ct, const TDF_Label& ref,
                                                        const TopoDS_Shape& s)
{
  ShapeIMap faces;
  TopExp::MapShapes(s, TopAbs_FACE, faces);
  NCollection_Sequence<TDF_Label> subs;
  XCAFDoc_ShapeTool::GetSubShapes(ref, subs);
  std::vector<std::pair<int, Quantity_Color>> found;
  for (int k = 1; k <= subs.Length(); ++k)
  {
    Quantity_Color fc;
    if (!(ct->GetColor(subs(k), XCAFDoc_ColorSurf, fc) || ct->GetColor(subs(k), XCAFDoc_ColorGen, fc)))
      continue;
    // A sub-shape label holds one face or several (a shell, a compound).
    for (TopExp_Explorer e(XCAFDoc_ShapeTool::GetShape(subs(k)), TopAbs_FACE); e.More(); e.Next())
    {
      const int i = faces.FindIndex(e.Current());
      if (i > 0)
        found.emplace_back(i - 1, fc);
    }
  }
  std::stable_sort(found.begin(), found.end(), [](const auto& x, const auto& y) { return x.first < y.first; });
  found.erase(std::unique(found.begin(), found.end(), [](const auto& x, const auto& y) { return x.first == y.first; }), found.end());
  return found;
}

void collect(const occ::handle<XCAFDoc_ShapeTool>& st, const occ::handle<XCAFDoc_ColorTool>& ct,
             const TDF_Label& label, const TopLoc_Location& loc, const std::string& inheritedName,
             bool inheritedHasColor, const Quantity_Color& inheritedColor, std::vector<Leaf>& out,
             int depth)
{
  if (depth > 64)
    return;
  TDF_Label ref = label;
  TopLoc_Location here = loc;
  if (XCAFDoc_ShapeTool::IsReference(label))
  {
    XCAFDoc_ShapeTool::GetReferredShape(label, ref);
    here = loc * XCAFDoc_ShapeTool::GetLocation(label);
  }
  std::string name = label_name(label);
  if (name.empty())
    name = label_name(ref);
  if (name.empty())
    name = inheritedName;
  Quantity_Color c        = inheritedColor;
  bool           hasColor = inheritedHasColor;
  {
    Quantity_Color c2;
    if (label_color(ct, label, TopoDS_Shape(), c2) || label_color(ct, ref, TopoDS_Shape(), c2))
    {
      c        = c2;
      hasColor = true;
    }
  }
  if (XCAFDoc_ShapeTool::IsAssembly(ref))
  {
    NCollection_Sequence<TDF_Label> comps;
    XCAFDoc_ShapeTool::GetComponents(ref, comps, false);
    for (int i = 1; i <= comps.Length(); ++i)
      collect(st, ct, comps(i), here, name, hasColor, c, out, depth + 1);
    return;
  }
  TopoDS_Shape s = XCAFDoc_ShapeTool::GetShape(ref);
  if (s.IsNull())
    return;
  Leaf leaf;
  leaf.shape = here.IsIdentity() ? s : s.Moved(here);
  leaf.name  = name;
  // The part's own label (`ref`) is its shape's label: no search by shape
  // (that walks every shape of the document, per part).
  Quantity_Color c3;
  if (label_color(ct, ref, TopoDS_Shape(), c3))
  {
    c        = c3;
    hasColor = true;
  }
  leaf.hasColor = hasColor;
  leaf.color    = c;
  // Face colours sit on the prototype's sub-shape labels; a placed copy
  // keeps their order.
  leaf.faceColors = face_colors(ct, ref, s);
  out.push_back(leaf);
}

// The largest distance between an edge and its pcurve on `f` (sampled).
double pcurve_deviation(const TopoDS_Edge& e, const TopoDS_Face& f)
{
  double                          a, b;
  const occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, f, a, b);
  if (pc.IsNull())
    return -1;
  const BRepAdaptor_Curve   c(e);
  const BRepAdaptor_Surface s(f, false);
  double                    m = 0;
  for (int k = 0; k <= 64; ++k)
  {
    const double   t = a + (b - a) * k / 64;
    const gp_Pnt2d q = pc->Value(t);
    m                = std::max(m, s.Value(q.X(), q.Y()).Distance(c.Value(t)));
  }
  return m;
}

// The B-spline pcurve moved onto the boundary iso of `S` it runs along (all
// its poles within a thousandth of the parameter range of it), or null.
occ::handle<Geom2d_Curve> snapped_to_iso(const occ::handle<Geom2d_Curve>& c, const occ::handle<Geom_Surface>& S)
{
  occ::handle<Geom2d_Curve> basis = c;
  while (auto tc = occ::down_cast<Geom2d_TrimmedCurve>(basis))
    basis = tc->BasisCurve();
  const auto bs = occ::down_cast<Geom2d_BSplineCurve>(basis);
  if (bs.IsNull())
    return {};
  double u0, u1, v0, v1;
  S->Bounds(u0, u1, v0, v1);
  for (int axis = 0; axis < 2; ++axis)
    for (const double iso : axis ? std::array<double, 2>{v0, v1} : std::array<double, 2>{u0, u1})
    {
      if (Precision::IsInfinite(iso))
        continue;
      const double eps      = 1e-3 * (axis ? v1 - v0 : u1 - u0);
      bool         near_iso = true, already = true;
      for (int k = 1; k <= bs->NbPoles() && near_iso; ++k)
      {
        const double x = axis ? bs->Pole(k).Y() : bs->Pole(k).X();
        near_iso       = std::abs(x - iso) < eps;
        already        = already && x == iso;
      }
      if (!near_iso || already)
        continue;
      const auto out = occ::down_cast<Geom2d_BSplineCurve>(bs->Copy());
      for (int k = 1; k <= out->NbPoles(); ++k)
      {
        gp_Pnt2d p = out->Pole(k);
        if (axis)
          p.SetY(iso);
        else
          p.SetX(iso);
        out->SetPole(k, p);
      }
      return occ::handle<Geom2d_Curve>(out);
    }
  return {};
}

// A STEP file without p-curves (Onshape's own, ours): the reader projects each edge onto its face loosely, 3.6 µm
// off on cf-spur-gear's involute flanks (surfaces of extrusion), and the faces bounded by them lose area (the
// 32-tooth gear -0.3 mm², live's file -1.0). On free-form faces an edge whose p-curve strays from it is projected
// again, a thousand times tighter; kept when twice as close.
void refine_freeform_pcurves(const std::vector<TopoDS_Shape>& shapes)
{
  ShapeIMap faces;
  for (const TopoDS_Shape& s : shapes)
    TopExp::MapShapes(s, TopAbs_FACE, faces);
  BRep_Builder B;
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    const TopoDS_Face f = TopoDS::Face(faces(i).Oriented(TopAbs_FORWARD));
    switch (BRepAdaptor_Surface(f, false).GetType())
    {
      case GeomAbs_BSplineSurface:
      case GeomAbs_BezierSurface:
      case GeomAbs_SurfaceOfExtrusion:
      case GeomAbs_SurfaceOfRevolution:
      case GeomAbs_OffsetSurface:
        break;
      default:
        continue;
    }
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> S = BRep_Tool::Surface(f, loc);
    if (S.IsNull())
      continue;
    // (A small face: projecting onto NIST's healed cap screw's 1300-pole thread surfaces takes 0.15-0.25 s an edge
    // and brings nothing closer; the gear's surfaces have up to 155.)
    if (const auto bs = occ::down_cast<Geom_BSplineSurface>(S); !bs.IsNull() && bs->NbUPoles() * bs->NbVPoles() > 200)
      continue;
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
    {
      const TopoDS_Edge ed = TopoDS::Edge(e.Current());
      TopLoc_Location   cl;
      double            c0, c1;
      if (BRep_Tool::Degenerated(ed) || BRep_Tool::IsClosed(ed, f) || BRep_Tool::Curve(ed, cl, c0, c1).IsNull())
        continue;
      // (Not a long spline: projecting NIST's healed cap screw's 848-pole thread edges again takes 0.1-0.3 s each and
      // doesn't bring them closer; the gear's involutes have 67 poles.)
      if (const auto bs = occ::down_cast<Geom_BSplineCurve>(BRep_Tool::Curve(ed, cl, c0, c1)); !bs.IsNull() && bs->NbPoles() > 200)
        continue;
      const double before = pcurve_deviation(ed, f);
      if (!(before > 1e-7))
        continue;
      double                          a, b;
      const occ::handle<Geom2d_Curve> old = BRep_Tool::CurveOnSurface(ed, f, a, b);
      B.UpdateEdge(ed, occ::handle<Geom2d_Curve>(), S, loc, 0.);
      ShapeFix_Edge fx;
      if (!fx.FixAddPCurve(ed, f, false, before / 1000) || !(pcurve_deviation(ed, f) < before / 2))
      {
        B.UpdateEdge(ed, old, S, loc, 0.);
        B.Range(ed, S, loc, a, b);
      }
    }
  }
}

// STEP files carry no pcurves for most edges and OCCT's projection goes
// wrong next to a surface's pole: on I1's blends (rational B-spline
// patches that close to a point at one u) the edges along v = 0 / 1 got
// pcurves drifting to v = 0.9958 towards the pole, the pole's degenerate
// edge followed, and each face lost a sliver (13.18188 mm² of live's
// 13.20006). On faces with a degenerate edge, the edges meeting it are
// projected again when they're off, moved onto the boundary iso they run
// along, and the degenerate edges then span their neighbours' ends.
void fix_pole_pcurves(const std::vector<TopoDS_Shape>& shapes)
{
  ShapeIMap faces;
  for (const TopoDS_Shape& s : shapes)
    TopExp::MapShapes(s, TopAbs_FACE, faces);
  BRep_Builder B;
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    const TopoDS_Face f = TopoDS::Face(faces(i).Oriented(TopAbs_FORWARD));
    ShapeIMap         poles;
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
      if (BRep_Tool::Degenerated(TopoDS::Edge(e.Current())))
        poles.Add(TopExp::FirstVertex(TopoDS::Edge(e.Current())));
    if (poles.IsEmpty())
      continue;
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> S = BRep_Tool::Surface(f, loc);
    if (S.IsNull())
      continue;
    bool changed = false;
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
    {
      const TopoDS_Edge ed = TopoDS::Edge(e.Current());
      TopLoc_Location cl;
      double          c0, c1;
      if (BRep_Tool::Degenerated(ed) || BRep_Tool::IsClosed(ed, f) || BRep_Tool::Curve(ed, cl, c0, c1).IsNull())
        continue;
      if (!poles.Contains(TopExp::FirstVertex(ed)) && !poles.Contains(TopExp::LastVertex(ed)))
        continue;
      double                          a, b;
      const occ::handle<Geom2d_Curve> old = BRep_Tool::CurveOnSurface(ed, f, a, b);
      if (old.IsNull())
        continue;
      const auto put = [&](const occ::handle<Geom2d_Curve>& c, double first, double last) {
        B.UpdateEdge(ed, c, S, loc, 0.);
        B.Range(ed, S, loc, first, last);
      };
      const double before = pcurve_deviation(ed, f);
      if (before > 1e-5)
      {
        B.UpdateEdge(ed, occ::handle<Geom2d_Curve>(), S, loc, 0.);
        ShapeFix_Edge fx;
        if (!fx.FixAddPCurve(ed, f, false, 1e-7) || !(pcurve_deviation(ed, f) < before / 2))
          put(old, a, b);
      }
      double                          a2, b2;
      const occ::handle<Geom2d_Curve> c    = BRep_Tool::CurveOnSurface(ed, f, a2, b2);
      const double                    good = pcurve_deviation(ed, f);
      if (const occ::handle<Geom2d_Curve> iso = snapped_to_iso(c, S))
      {
        put(iso, a2, b2);
        if (!(pcurve_deviation(ed, f) < std::max(2 * good, 1e-6)))
          put(c, a2, b2);
      }
      double a3, b3;
      if (BRep_Tool::CurveOnSurface(ed, f, a3, b3) == old)
        continue;
      changed = true;
      // Tolerances only grow: the pcurve on the edge's other face may need
      // the old one.
      const double tol = 1.1 * pcurve_deviation(ed, f);
      if (tol > BRep_Tool::Tolerance(ed))
      {
        B.UpdateEdge(ed, tol);
        for (TopoDS_Iterator v(ed); v.More(); v.Next())
          B.UpdateVertex(TopoDS::Vertex(v.Value()), tol);
      }
    }
    if (!changed)
      continue;
    std::vector<gp_Pnt2d> ends; // the other edges' (u, v) at a pole
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
    {
      const TopoDS_Edge& ed = TopoDS::Edge(e.Current());
      double             a, b;
      const occ::handle<Geom2d_Curve> c = BRep_Tool::Degenerated(ed) ? nullptr : BRep_Tool::CurveOnSurface(ed, f, a, b);
      if (c.IsNull())
        continue;
      if (poles.Contains(TopExp::FirstVertex(ed)))
        ends.push_back(c->Value(a));
      if (poles.Contains(TopExp::LastVertex(ed)))
        ends.push_back(c->Value(b));
    }
    const auto nearest = [&](const gp_Pnt2d& p) {
      gp_Pnt2d best = ends.front();
      for (const gp_Pnt2d& q : ends)
        if (q.SquareDistance(p) < best.SquareDistance(p))
          best = q;
      return best;
    };
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next())
    {
      const TopoDS_Edge& d = TopoDS::Edge(e.Current());
      double             a, b;
      const occ::handle<Geom2d_Curve> c = BRep_Tool::Degenerated(d) ? BRep_Tool::CurveOnSurface(d, f, a, b) : nullptr;
      if (c.IsNull() || ends.empty())
        continue;
      const gp_Pnt2d p = nearest(c->Value(a)), q = nearest(c->Value(b));
      if (p.Distance(q) < 1e-12)
        continue;
      B.UpdateEdge(d, new Geom2d_Line(p, gp_Dir2d(gp_Vec2d(p, q))), f, BRep_Tool::Tolerance(d));
      B.Range(d, 0, p.Distance(q));
    }
  }
}

} // namespace

// ---------------------------------------------------------------- BREP

void write_brep_file(const OcShape& s, rust::Str path)
{
  require_non_null(s.shape, "shape");
  const LockedSection locked_section_171;
  std::lock_guard<std::mutex> lock(mesh_mutex());
  if (!BRepTools::Write(s.shape, path_str(path).c_str(), false, false,
                        TopTools_FormatVersion_CURRENT))
    fail_io("could not write BREP file " + path_str(path));
}

std::unique_ptr<OcShape> read_brep_file(rust::Str path)
{
  TopoDS_Shape s;
  BRep_Builder b;
  if (!BRepTools::Read(s, path_str(path).c_str(), b) || s.IsNull())
    fail_io("could not read BREP file " + path_str(path));
  return wrap(s);
}

rust::Vec<uint8_t> brep_to_bytes(const OcShape& s)
{
  require_non_null(s.shape, "shape");
  std::ostringstream os;
  {
    const LockedSection locked_section_191;
    std::lock_guard<std::mutex> lock(mesh_mutex());
    BRepTools::Write(s.shape, os, false, false, TopTools_FormatVersion_CURRENT);
  }
  const std::string  str = os.str();
  rust::Vec<uint8_t> v;
  v.reserve(str.size());
  for (char c : str)
    v.push_back(static_cast<uint8_t>(c));
  return v;
}

std::unique_ptr<OcShape> brep_from_bytes(rust::Slice<const uint8_t> bytes)
{
  std::string        str(reinterpret_cast<const char*>(bytes.data()), bytes.size());
  std::istringstream is(str);
  TopoDS_Shape       s;
  BRep_Builder       b;
  BRepTools::Read(s, is, b);
  if (s.IsNull())
    fail_io("could not parse BREP data");
  return wrap(s);
}

rust::Vec<uint8_t> bin_to_bytes(const OcShape& s, bool with_triangles)
{
  require_non_null(s.shape, "shape");
  std::ostringstream os(std::ios::out | std::ios::binary);
  {
    const LockedSection locked_section_219;
    std::lock_guard<std::mutex> lock(mesh_mutex());
    BinTools::Write(s.shape, os, with_triangles, false, BinTools_FormatVersion_CURRENT);
  }
  const std::string  str = os.str();
  rust::Vec<uint8_t> v;
  v.reserve(str.size());
  for (char c : str)
    v.push_back(static_cast<uint8_t>(c));
  return v;
}

std::unique_ptr<OcShape> bin_from_bytes(rust::Slice<const uint8_t> bytes)
{
  std::string        str(reinterpret_cast<const char*>(bytes.data()), bytes.size());
  std::istringstream is(str, std::ios::in | std::ios::binary);
  TopoDS_Shape       s;
  BinTools::Read(s, is);
  if (s.IsNull())
    fail_io("could not parse binary BREP data");
  return wrap(s);
}

// ---------------------------------------------------------------- STEP

namespace
{
// NOSHAPE_IMPORT_TIMING: a reader stage's time on stderr.
struct StageClock
{
  bool                                  on = std::getenv("NOSHAPE_IMPORT_TIMING") != nullptr;
  std::chrono::steady_clock::time_point t  = std::chrono::steady_clock::now();
  void                                  lap(const char* what)
  {
    if (!on)
      return;
    const auto now = std::chrono::steady_clock::now();
    std::fprintf(stderr, "  occt %s: %.1f ms\n", what, std::chrono::duration<double, std::milli>(now - t).count());
    t = now;
  }
};
} // namespace

StepImport read_step(rust::Str path)
{
  const LockedSection locked_section_245;
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  StageClock                  clock;
  const std::string           p = path_str(path);
  StepImport                  out;
  out.shapes = std::make_unique<OcShapeList>();

  occ::handle<XCAFApp_Application> app = XCAFApp_Application::GetApplication();
  occ::handle<TDocStd_Document>    doc;
  app->NewDocument("MDTV-XCAF", doc);
  bool cafOk = false;
  if (!doc.IsNull())
  {
    STEPCAFControl_Reader r;
    r.SetColorMode(true);
    r.SetNameMode(true);
    r.SetLayerMode(false);
    if (r.ReadFile(p.c_str()) != IFSelect_RetDone)
    {
      app->Close(doc);
      fail_io("could not read STEP file " + p);
    }
    clock.lap("step ReadFile");
    const bool transferred = r.Transfer(doc);
    clock.lap("step Transfer");
    if (transferred)
    {
      occ::handle<XCAFDoc_ShapeTool> st = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
      occ::handle<XCAFDoc_ColorTool> ct = XCAFDoc_DocumentTool::ColorTool(doc->Main());
      NCollection_Sequence<TDF_Label> roots;
      st->GetFreeShapes(roots);
      std::vector<Leaf> leaves;
      for (int i = 1; i <= roots.Length(); ++i)
        collect(st, ct, roots(i), TopLoc_Location(), std::string(), false, Quantity_Color(),
                leaves, 0);
      for (const auto& l : leaves)
      {
        out.shapes->items.push_back(l.shape);
        out.names.push_back(rust::String(l.name));
        double r_, g_, b_;
        l.color.Values(r_, g_, b_, Quantity_TOC_sRGB);
        out.colors.push_back((float)r_);
        out.colors.push_back((float)g_);
        out.colors.push_back((float)b_);
        out.has_color.push_back(l.hasColor);
        for (const auto& [face, fc] : l.faceColors)
        {
          double fr, fg, fb;
          fc.Values(fr, fg, fb, Quantity_TOC_sRGB);
          out.face_part.push_back((uint32_t)(out.names.size() - 1));
          out.face_index.push_back((uint32_t)face);
          out.face_colors.push_back((float)fr);
          out.face_colors.push_back((float)fg);
          out.face_colors.push_back((float)fb);
        }
      }
      cafOk = !leaves.empty();
      clock.lap("step collect");
    }
    app->Close(doc);
  }
  if (!cafOk)
  {
    STEPControl_Reader r;
    if (r.ReadFile(p.c_str()) != IFSelect_RetDone)
      fail_io("could not read STEP file " + p);
    r.TransferRoots();
    for (int i = 1; i <= r.NbShapes(); ++i)
    {
      TopoDS_Shape s = r.Shape(i);
      if (s.IsNull())
        continue;
      out.shapes->items.push_back(s);
      out.names.push_back(rust::String());
      for (int k = 0; k < 3; ++k)
        out.colors.push_back(0.0f);
      out.has_color.push_back(false);
    }
  }
  if (out.shapes->items.empty())
    fail_io("STEP file contains no shapes: " + p);
  fix_pole_pcurves(out.shapes->items);
  refine_freeform_pcurves(out.shapes->items);
  clock.lap("step pole pcurves");
  return out;
}

// A STEP assembly as its distinct parts (prototypes, in their own
// coordinates) and every placed leaf instance (assembly import, like .pvz).
namespace
{
struct TreeWalk
{
  occ::handle<XCAFDoc_ShapeTool> st;
  occ::handle<XCAFDoc_ColorTool> ct;
  std::vector<TDF_Label>         protos;
  std::vector<Leaf>              parts;
  rust::Vec<PvzInstance>         instances;
  std::string                    root;
  // Subassembly definitions and the tree's direct children (StepTree::nest).
  std::vector<TDF_Label>         subProtos;
  rust::Vec<rust::String>        subNames;
  rust::Vec<StepChild>           nest;
};

// Row-major 3 x 4 of a location, translation in metres (OCCT works in mm).
void push_trsf(rust::Vec<double>& out, const TopLoc_Location& l)
{
  const gp_Trsf t = l.Transformation();
  for (int r = 1; r <= 3; ++r)
    for (int k = 1; k <= 4; ++k)
      out.push_back(k == 4 ? t.Value(r, k) * 1e-3 : t.Value(r, k));
}

// `owner`: the subassembly definition holding `label` (-1 the root);
// `record`: the owner is walked for the first time (its children go in
// `nest` once).
void walk_tree(TreeWalk& w, const TDF_Label& label, const TopLoc_Location& loc,
               const std::string& inheritedName, bool inheritedHasColor,
               const Quantity_Color& inheritedColor, int depth, int owner, bool record)
{
  if (depth > 64)
    return;
  TDF_Label       ref  = label;
  TopLoc_Location here = loc;
  TopLoc_Location local;
  if (XCAFDoc_ShapeTool::IsReference(label))
  {
    XCAFDoc_ShapeTool::GetReferredShape(label, ref);
    local = XCAFDoc_ShapeTool::GetLocation(label);
    here  = loc * local;
  }
  std::string name = label_name(label);
  if (name.empty())
    name = label_name(ref);
  if (name.empty())
    name = inheritedName;
  Quantity_Color c        = inheritedColor;
  bool           hasColor = inheritedHasColor;
  {
    Quantity_Color c2;
    if (label_color(w.ct, label, TopoDS_Shape(), c2) || label_color(w.ct, ref, TopoDS_Shape(), c2))
    {
      c        = c2;
      hasColor = true;
    }
  }
  if (XCAFDoc_ShapeTool::IsAssembly(ref))
  {
    int def = -1;
    for (size_t i = 0; i < w.subProtos.size(); ++i)
      if (w.subProtos[i].IsEqual(ref))
        def = (int)i;
    const bool first = def < 0;
    if (first)
    {
      def = (int)w.subProtos.size();
      w.subProtos.push_back(ref);
      const std::string defName = label_name(ref).empty() ? name : label_name(ref);
      w.subNames.push_back(rust::String(defName));
    }
    if (record)
    {
      StepChild ch;
      ch.owner = owner;
      ch.part  = -1;
      ch.sub   = def;
      ch.name  = rust::String(name);
      push_trsf(ch.transform, local);
      w.nest.push_back(std::move(ch));
    }
    NCollection_Sequence<TDF_Label> comps;
    XCAFDoc_ShapeTool::GetComponents(ref, comps, false);
    for (int i = 1; i <= comps.Length(); ++i)
      walk_tree(w, comps(i), here, name, hasColor, c, depth + 1, def, first);
    return;
  }
  TopoDS_Shape s = XCAFDoc_ShapeTool::GetShape(ref);
  if (s.IsNull())
    return;
  size_t part = w.protos.size();
  for (size_t i = 0; i < w.protos.size(); ++i)
    if (w.protos[i].IsEqual(ref))
      part = i;
  if (part == w.protos.size())
  {
    // The part as its product defines it (no placement), named after it.
    Leaf leaf;
    leaf.shape = s;
    leaf.name  = label_name(ref).empty() ? name : label_name(ref);
    Quantity_Color c3;
    if (label_color(w.ct, ref, TopoDS_Shape(), c3))
    {
      c        = c3;
      hasColor = true;
    }
    leaf.hasColor   = hasColor;
    leaf.color      = c;
    leaf.faceColors = face_colors(w.ct, ref, s);
    w.protos.push_back(ref);
    w.parts.push_back(leaf);
  }
  // Placement: row-major 3 x 4, translation in metres (OCCT works in mm).
  const gp_Trsf t = here.Transformation();
  PvzInstance   inst;
  inst.part = (uint32_t)part;
  inst.name = rust::String(name);
  inst.root = rust::String(w.root);
  for (int r = 1; r <= 3; ++r)
    for (int k = 1; k <= 4; ++k)
      inst.transform.push_back(k == 4 ? t.Value(r, k) * 1e-3 : t.Value(r, k));
  w.instances.push_back(std::move(inst));
  if (record)
  {
    StepChild ch;
    ch.owner = owner;
    ch.part  = (int)part;
    ch.sub   = -1;
    ch.name  = rust::String(name);
    push_trsf(ch.transform, local);
    w.nest.push_back(std::move(ch));
  }
}
} // namespace

StepTree read_step_tree(rust::Str path)
{
  const LockedSection locked_section_421;
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  StageClock        clock;
  const std::string p = path_str(path);
  StepTree          out;
  out.parts.shapes = std::make_unique<OcShapeList>();
  occ::handle<XCAFApp_Application> app = XCAFApp_Application::GetApplication();
  occ::handle<TDocStd_Document>    doc;
  app->NewDocument("MDTV-XCAF", doc);
  if (doc.IsNull())
    fail_io("could not read STEP file " + p);
  STEPCAFControl_Reader r;
  r.SetColorMode(true);
  r.SetNameMode(true);
  r.SetLayerMode(false);
  if (r.ReadFile(p.c_str()) != IFSelect_RetDone)
  {
    app->Close(doc);
    fail_io("could not read STEP file " + p);
  }
  clock.lap("step tree ReadFile");
  if (!r.Transfer(doc))
  {
    app->Close(doc);
    fail_io("could not read STEP file " + p);
  }
  clock.lap("step tree Transfer");
  TreeWalk w;
  w.st = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
  w.ct = XCAFDoc_DocumentTool::ColorTool(doc->Main());
  NCollection_Sequence<TDF_Label> roots;
  w.st->GetFreeShapes(roots);
  // Only a single root product that is an assembly makes an Assembly.
  if (roots.Length() == 1 && XCAFDoc_ShapeTool::IsAssembly(roots(1)))
  {
    out.root = rust::String(label_name(roots(1)));
    w.root   = label_name(roots(1));
    NCollection_Sequence<TDF_Label> comps;
    XCAFDoc_ShapeTool::GetComponents(roots(1), comps, false);
    Quantity_Color c;
    const bool     has = label_color(w.ct, roots(1), TopoDS_Shape(), c);
    for (int i = 1; i <= comps.Length(); ++i)
      walk_tree(w, comps(i), TopLoc_Location(), w.root, has, c, 1, -1, true);
  }
  for (const auto& l : w.parts)
  {
    out.parts.shapes->items.push_back(l.shape);
    out.parts.names.push_back(rust::String(l.name));
    double r_, g_, b_;
    l.color.Values(r_, g_, b_, Quantity_TOC_sRGB);
    out.parts.colors.push_back((float)r_);
    out.parts.colors.push_back((float)g_);
    out.parts.colors.push_back((float)b_);
    out.parts.has_color.push_back(l.hasColor);
    for (const auto& [face, fc] : l.faceColors)
    {
      double fr, fg, fb;
      fc.Values(fr, fg, fb, Quantity_TOC_sRGB);
      out.parts.face_part.push_back((uint32_t)(out.parts.names.size() - 1));
      out.parts.face_index.push_back((uint32_t)face);
      out.parts.face_colors.push_back((float)fr);
      out.parts.face_colors.push_back((float)fg);
      out.parts.face_colors.push_back((float)fb);
    }
  }
  out.instances = std::move(w.instances);
  out.sub_names = std::move(w.subNames);
  out.nest      = std::move(w.nest);
  clock.lap("step tree walk");
  app->Close(doc);
  clock.lap("step tree close");
  fix_pole_pcurves(out.parts.shapes->items);
  refine_freeform_pcurves(out.parts.shapes->items);
  clock.lap("step tree pole pcurves");
  return out;
}

namespace
{
void write_step_impl(const OcShapeList& shapes, rust::Slice<const rust::String> names,
                     rust::Slice<const float> colors, rust::Slice<const bool> has_color,
                     rust::Str path, uint8_t schema, uint8_t file_unit);
} // namespace

void write_step(const OcShapeList& shapes, rust::Slice<const rust::String> names,
                rust::Slice<const float> colors, rust::Slice<const bool> has_color,
                rust::Str path, uint8_t schema, uint8_t file_unit)
{
  write_step_impl(shapes, names, colors, has_color, path, schema, file_unit);
}

rust::Vec<rust::String> read_step_pmi(rust::Str path)
{
  const LockedSection locked_section_pmi;
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  const std::string                p = path_str(path);
  occ::handle<XCAFApp_Application> app = XCAFApp_Application::GetApplication();
  occ::handle<TDocStd_Document>    doc;
  app->NewDocument("MDTV-XCAF", doc);
  STEPCAFControl_Reader r;
  r.SetGDTMode(true);
  r.SetNameMode(true);
  if (r.ReadFile(p.c_str()) != IFSelect_RetDone || !r.Transfer(doc))
  {
    app->Close(doc);
    fail_io("could not read STEP file " + p);
  }
  rust::Vec<rust::String>         out;
  occ::handle<XCAFDoc_DimTolTool> dt = XCAFDoc_DocumentTool::DimTolTool(doc->Main());
  NCollection_Sequence<TDF_Label> ls;
  dt->GetDatumLabels(ls);
  for (int i = 1; i <= ls.Length(); ++i)
  {
    occ::handle<XCAFDoc_Datum> a;
    std::string                name;
    if (ls(i).FindAttribute(XCAFDoc_Datum::GetID(), a) && !a->GetObject().IsNull() && !a->GetObject()->GetName().IsNull())
      name = a->GetObject()->GetName()->ToCString();
    NCollection_Sequence<TDF_Label> f, s2;
    XCAFDoc_DimTolTool::GetRefShapeLabel(ls(i), f, s2);
    out.push_back(rust::String("datum " + name + " faces " + std::to_string(f.Length())));
  }
  ls.Clear();
  dt->GetDimensionLabels(ls);
  for (int i = 1; i <= ls.Length(); ++i)
  {
    occ::handle<XCAFDoc_Dimension> a;
    if (!ls(i).FindAttribute(XCAFDoc_Dimension::GetID(), a) || a->GetObject().IsNull())
      continue;
    occ::handle<XCAFDimTolObjects_DimensionObject> o = a->GetObject();
    NCollection_Sequence<TDF_Label>               f, s2;
    XCAFDoc_DimTolTool::GetRefShapeLabel(ls(i), f, s2);
    char buf[256];
    std::snprintf(buf, sizeof buf, "dim %d %.6g tol %.6g %.6g faces %d+%d", (int)o->GetType(), o->GetValue(), o->GetLowerTolValue(), o->GetUpperTolValue(), f.Length(), s2.Length());
    out.push_back(rust::String(buf));
  }
  app->Close(doc);
  return out;
}

namespace
{
// An edge using a small part of a long B-spline writes only that part. Healing an
// imported thread splits its helix edges, and each piece keeps the whole
// 848-pole curve: NIST's cap screw wrote 4 MB of STEP (40 such curves), and
// reading it back took 17 s, OCCT re-projecting every copy. The shape is
// copied first when an edge qualifies (exported shapes may share the
// document's geometry).
TopoDS_Shape compact_bspline_edges(const TopoDS_Shape& in)
{
  auto long_spline = [](const TopoDS_Edge& e, double& f, double& l) -> occ::handle<Geom_BSplineCurve> {
    TopLoc_Location loc;
    occ::handle<Geom_BSplineCurve> bs = occ::down_cast<Geom_BSplineCurve>(BRep_Tool::Curve(e, loc, f, l));
    if (bs.IsNull() || bs->NbPoles() < 64 || l - f > 0.75 * (bs->LastParameter() - bs->FirstParameter()))
      return {};
    return bs;
  };
  bool any = false;
  for (TopExp_Explorer ex(in, TopAbs_EDGE); ex.More() && !any; ex.Next())
  {
    double f, l;
    any = !long_spline(TopoDS::Edge(ex.Current()), f, l).IsNull();
  }
  if (!any)
    return in;
  TopoDS_Shape s = BRepBuilderAPI_Copy(in, true, false).Shape();
  BRep_Builder b;
  for (TopExp_Explorer ex(s, TopAbs_EDGE); ex.More(); ex.Next())
  {
    const TopoDS_Edge e = TopoDS::Edge(ex.Current());
    double            f, l;
    occ::handle<Geom_BSplineCurve> bs = long_spline(e, f, l);
    if (bs.IsNull())
      continue;
    occ::handle<Geom_BSplineCurve> seg = occ::down_cast<Geom_BSplineCurve>(bs->Copy());
    try
    {
      // Same parameterization: the edge's range and p-curves stay valid.
      seg->Segment(f, l);
    }
    catch (const Standard_Failure&)
    {
      continue;
    }
    TopLoc_Location loc;
    BRep_Tool::Curve(e, loc, f, l);
    b.UpdateEdge(e, seg, loc, BRep_Tool::Tolerance(e));
    b.Range(e, f, l, true);
  }
  return s;
}

} // namespace

void clamp_edge_ranges(const TopoDS_Shape& s)
{
  BRep_Builder b;
  for (TopExp_Explorer ex(s, TopAbs_EDGE); ex.More(); ex.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(ex.Current());
    if (BRep_Tool::Degenerated(e))
      continue;
    double                  f, l;
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
    if (c.IsNull() || c->IsPeriodic())
      continue;
    const double nf = std::max(f, c->FirstParameter()), nl = std::min(l, c->LastParameter());
    if ((nf != f || nl != l) && nl > nf)
      b.Range(e, nf, nl);
  }
}

namespace
{
void write_step_impl(const OcShapeList& shapes, rust::Slice<const rust::String> names,
                     rust::Slice<const float> colors, rust::Slice<const bool> has_color,
                     rust::Str path, uint8_t schema, uint8_t file_unit)
{
  const LockedSection locked_section_487;
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  const std::string           p = path_str(path);
  if (shapes.items.empty())
    fail_invalid("nothing to export");
  occ::handle<XCAFApp_Application> app = XCAFApp_Application::GetApplication();
  occ::handle<TDocStd_Document>    doc;
  app->NewDocument("MDTV-XCAF", doc);
  if (doc.IsNull())
    fail_op("could not create XCAF document");
  occ::handle<XCAFDoc_ShapeTool> st = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
  occ::handle<XCAFDoc_ColorTool> ct = XCAFDoc_DocumentTool::ColorTool(doc->Main());
  // Yards (file unit 100): OCCT has no such unit; the shapes go in a third
  // the size as feet and the file's foot unit becomes a yard (0.9144 m).
  const bool yard = file_unit == 100;
  gp_Trsf    third;
  third.SetScale(gp_Pnt(0, 0, 0), 1.0 / 3.0);
  for (size_t i = 0; i < shapes.items.size(); ++i)
  {
    require_non_null(shapes.items[i], "exported shape");
    TopoDS_Shape s = shapes.items[i];
    // A located part (a pattern instance sharing its seed's geometry) would
    // become an assembly instance of a "SOLID" product: bake the location.
    if (!s.Location().IsIdentity())
      s = BRepBuilderAPI_Transform(s.Located(TopLoc_Location()), s.Location().Transformation(), true).Shape();
    if (yard)
      s = BRepBuilderAPI_Transform(s, third, true).Shape();
    s = compact_bspline_edges(s);
    TDF_Label l = st->AddShape(s, false);
    if (i < names.size() && !names[i].empty())
    {
      std::string n(names[i].data(), names[i].size());
      TDataStd_Name::Set(l, TCollection_ExtendedString(n.c_str(), true));
    }
    if (i < has_color.size() && has_color[i] && colors.size() >= 3 * (i + 1))
    {
      Quantity_Color c(colors[3 * i], colors[3 * i + 1], colors[3 * i + 2], Quantity_TOC_sRGB);
      ct->SetColor(l, c, XCAFDoc_ColorGen);
    }
  }
  STEPCAFControl_Writer w;
  w.SetColorMode(true);
  w.SetNameMode(true);
  // Shapes are in millimetres here (see io.rs); the file unit and schema are the user's.
  DESTEP_Parameters params;
  params.InitFromStatic();
  // One product per part (Onshape writes no assembly) and no p-curves.
  params.WriteAssembly       = DESTEP_Parameters::WriteMode_Assembly_Off;
  params.WriteSurfaceCurMode = false;
  // The shapes' greatest tolerance as the uncertainty (step_out.rs keeps
  // Onshape's 1e-8 m unless a part's is larger).
  params.WritePrecisionMode = DESTEP_Parameters::WriteMode_PrecisionMode_Greatest;
  switch (schema)
  {
    case 1:
      params.WriteSchema = DESTEP_Parameters::WriteMode_StepSchema_AP242DIS;
      break;
    case 2:
      params.WriteSchema = DESTEP_Parameters::WriteMode_StepSchema_AP214IS;
      break;
    case 3:
      params.WriteSchema = DESTEP_Parameters::WriteMode_StepSchema_AP203;
      break;
    default:
      break; // OCCT's default (AP214 IS)
  }
  if (yard)
    file_unit = UnitsMethods_LengthUnit_Foot;
  if (file_unit != 0)
    params.WriteUnit = static_cast<UnitsMethods_LengthUnit>(file_unit);
  bool ok = w.Perform(doc, p.c_str(), params);
  app->Close(doc);
  if (!ok)
    fail_io("could not write STEP file " + p);
  if (yard)
  {
    std::ifstream in(p, std::ios::binary);
    std::string   text((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
    in.close();
    // ( CONVERSION_BASED_UNIT('FOOT',#n) ... ) with #n = LENGTH_MEASURE_WITH_UNIT(304.8,#mm).
    const std::regex unit("CONVERSION_BASED_UNIT\\('FOOT'"), measure("LENGTH_MEASURE_WITH_UNIT\\((LENGTH_MEASURE\\()?304\\.8[0-9]*");
    if (!std::regex_search(text, unit) || !std::regex_search(text, measure))
      fail_io("could not write the yard unit in " + p);
    text = std::regex_replace(std::regex_replace(text, unit, "CONVERSION_BASED_UNIT('YARD'"), measure, "LENGTH_MEASURE_WITH_UNIT($01914.4");
    std::ofstream out(p, std::ios::binary | std::ios::trunc);
    out << text;
    if (!out)
      fail_io("could not write STEP file " + p);
  }
}

} // namespace

// ---------------------------------------------------------------- IGES

std::unique_ptr<OcShape> read_iges(rust::Str path)
{
  const LockedSection locked_section_578;
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  const std::string           p = path_str(path);
  IGESControl_Reader          r;
  if (r.ReadFile(p.c_str()) != IFSelect_RetDone)
    fail_io("could not read IGES file " + p);
  r.TransferRoots();
  TopoDS_Shape s = r.OneShape();
  if (s.IsNull())
    fail_io("IGES file contains no shapes: " + p);
  refine_freeform_pcurves({s});
  return wrap(s);
}

// An IGES file's parts and placed instances when several of its roots are
// the same shape placed (subfigure instances: live's IGES export of an
// assembly, request 1755, round-trips as an Assembly); no instances
// otherwise. Names and colours through OCCT's XDE reader; the root's name is
// the file's sending product id. Parts in mm, placements in metres.
StepTree read_iges_tree(rust::Str path)
{
  const LockedSection         locked_section_iges_tree;
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  const std::string p = path_str(path);
  StepTree          out;
  out.parts.shapes = std::make_unique<OcShapeList>();
  occ::handle<XCAFApp_Application> app = XCAFApp_Application::GetApplication();
  occ::handle<TDocStd_Document>    doc;
  app->NewDocument("MDTV-XCAF", doc);
  if (doc.IsNull())
    fail_io("could not read IGES file " + p);
  IGESCAFControl_Reader r;
  r.SetColorMode(true);
  r.SetNameMode(true);
  r.SetLayerMode(false);
  if (r.ReadFile(p.c_str()) != IFSelect_RetDone)
  {
    app->Close(doc);
    fail_io("could not read IGES file " + p);
  }
  // Onshape's (HOOPS) labels carry subscript 0, which OCCT would append
  // ("B17-29(0)"); live names the part "B17-29".
  const occ::handle<Interface_InterfaceModel> read = r.Model();
  for (int i = 1; !read.IsNull() && i <= read->NbEntities(); ++i)
    if (const occ::handle<IGESData_IGESEntity> e = occ::down_cast<IGESData_IGESEntity>(read->Value(i)); !e.IsNull() && e->HasShortLabel() && e->SubScriptNumber() == 0)
      e->SetLabel(e->ShortLabel(), -1);
  if (!r.Transfer(doc))
  {
    app->Close(doc);
    fail_io("could not read IGES file " + p);
  }
  // Each solid's name by its transferred shape, as live names it (1818):
  // its 8-character label (not a longer Name property, which XDE prefers),
  // or " DE" and its directory line when it has none. (XDE also leaves an
  // instanced solid unnamed: live's assembly IGES, 1755.)
  std::map<const TopoDS_TShape*, std::string> labelled;
  const occ::handle<Transfer_TransientProcess>& tp = r.WS()->TransferReader()->TransientProcess();
  for (int i = 1; !read.IsNull() && i <= read->NbEntities(); ++i)
    if (const occ::handle<IGESSolid_ManifoldSolid> e = occ::down_cast<IGESSolid_ManifoldSolid>(read->Value(i)); !e.IsNull())
      if (const occ::handle<Transfer_Binder> b = tp->Find(e); !b.IsNull())
        if (const TopoDS_Shape sh = TransferBRep::ShapeResult(b); !sh.IsNull())
        {
          std::string name;
          if (e->HasShortLabel())
          {
            TCollection_AsciiString n = e->ShortLabel()->String();
            n.LeftAdjust();
            n.RightAdjust();
            name = n.ToCString();
          }
          if (name.empty())
          {
            const std::string de = std::to_string(2 * i - 1);
            name = " DE" + std::string(de.size() < 9 ? 9 - de.size() : 0, ' ') + de;
          }
          labelled[sh.TShape().get()] = name;
        }
  occ::handle<XCAFDoc_ShapeTool>  st = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
  occ::handle<XCAFDoc_ColorTool>  ct = XCAFDoc_DocumentTool::ColorTool(doc->Main());
  NCollection_Sequence<TDF_Label> roots;
  st->GetFreeShapes(roots);
  // Roots by their unlocated shape.
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> protos;
  std::vector<int>                                              part_of;
  for (int i = 1; i <= roots.Length(); ++i)
  {
    const TopoDS_Shape s = XCAFDoc_ShapeTool::GetShape(roots(i));
    part_of.push_back(s.IsNull() ? -1 : protos.Add(s.Located(TopLoc_Location())) - 1);
  }
  std::vector<Leaf> parts;
  const bool        shared = protos.Extent() > 0 && protos.Extent() < roots.Length();
  // Separate shapes: each a part, named and coloured (no instances).
  for (int i = 1; i <= roots.Length() && !shared; ++i)
    collect(st, ct, roots(i), TopLoc_Location(), std::string(), false, Quantity_Color(), parts, 0);
  if (shared)
    parts.resize((size_t)protos.Extent());
  std::vector<bool> seen((size_t)protos.Extent(), false);
  for (int i = 1; i <= roots.Length() && shared; ++i)
  {
    const int k = part_of[(size_t)i - 1];
    if (k < 0)
      continue;
    const TopoDS_Shape s = XCAFDoc_ShapeTool::GetShape(roots(i));
    if (!seen[(size_t)k])
    {
      seen[(size_t)k] = true;
      Leaf& leaf      = parts[(size_t)k];
      leaf.shape      = protos(k + 1);
      // The solid's own label (an instance's is XDE's link, "=>[0:1:1:4]").
      TDF_Label ref;
      leaf.name = XCAFDoc_ShapeTool::GetReferredShape(roots(i), ref) ? label_name(ref) : label_name(roots(i));
      leaf.hasColor   = label_color(ct, roots(i), TopoDS_Shape(), leaf.color);
      // Else its solid's (an IGES solid points to its colour definition).
      for (TopExp_Explorer ex(leaf.shape, TopAbs_SOLID); ex.More() && !leaf.hasColor; ex.Next())
        leaf.hasColor = ct->GetColor(ex.Current(), XCAFDoc_ColorGen, leaf.color) || ct->GetColor(ex.Current(), XCAFDoc_ColorSurf, leaf.color);
      leaf.faceColors = face_colors(ct, roots(i), leaf.shape);
    }
    const gp_Trsf t = s.Location().Transformation();
    PvzInstance   inst;
    inst.part = (uint32_t)k;
    inst.name = rust::String(label_name(roots(i)));
    for (int row = 1; row <= 3; ++row)
      for (int c = 1; c <= 4; ++c)
        inst.transform.push_back(c == 4 ? t.Value(row, c) * 1e-3 : t.Value(row, c));
    out.instances.push_back(std::move(inst));
  }
  occ::handle<IGESData_IGESModel> model = occ::down_cast<IGESData_IGESModel>(r.WS()->Model());
  std::string                     root;
  if (!model.IsNull() && !model->GlobalSection().SendName().IsNull())
    root = model->GlobalSection().SendName()->ToCString();
  out.root = rust::String(root);
  for (auto& inst : out.instances)
    inst.root = rust::String(root);
  for (const auto& l : parts)
  {
    // XDE names a solid without a label after its type ("SOLID"; qa
    // IGES-IMPORT-NAME-SOLID) or its link ("=>[0:1:1:4]"): no name.
    bool made_up = false;
    for (const char* t : {"COMPOUND", "COMPSOLID", "SOLID", "SHELL", "FACE", "WIRE", "EDGE", "VERTEX", "SHAPE"})
      made_up = made_up || l.name == t || l.name.rfind("=>", 0) == 0;
    std::string name = made_up ? std::string() : l.name;
    for (TopExp_Explorer ex(l.shape, TopAbs_SOLID); !labelled.empty();)
    {
      // The part itself, else its (first) solid: a subfigure is a compound.
      const TopoDS_Shape& sh = ex.More() ? ex.Current() : l.shape;
      if (const auto at = labelled.find(l.shape.TShape().get()); at != labelled.end())
        name = at->second;
      else if (const auto at2 = labelled.find(sh.TShape().get()); at2 != labelled.end())
        name = at2->second;
      break;
    }
    out.parts.shapes->items.push_back(l.shape);
    out.parts.names.push_back(rust::String(name));
    double r_, g_, b_;
    l.color.Values(r_, g_, b_, Quantity_TOC_sRGB);
    out.parts.colors.push_back((float)r_);
    out.parts.colors.push_back((float)g_);
    out.parts.colors.push_back((float)b_);
    out.parts.has_color.push_back(l.hasColor);
    for (const auto& [face, fc] : l.faceColors)
    {
      double fr, fg, fb;
      fc.Values(fr, fg, fb, Quantity_TOC_sRGB);
      out.parts.face_part.push_back((uint32_t)(out.parts.names.size() - 1));
      out.parts.face_index.push_back((uint32_t)face);
      out.parts.face_colors.push_back((float)fr);
      out.parts.face_colors.push_back((float)fg);
      out.parts.face_colors.push_back((float)fb);
    }
  }
  app->Close(doc);
  refine_freeform_pcurves(out.parts.shapes->items);
  return out;
}

// Each part as Onshape's IGES (HOOPS) names and colours it (live
// qa5-iges-header.igs, dx-big-asm.igs): its name in the 8-character labels of
// its manifold solid (186), edge list (504) and vertex list (502), which
// readers take as the part's name; its colour a Color Definition (314,
// "COLOR", RGB in percent) the solid and its faces point to, one per colour.
// Returns each part's top entity (null where it has none).
std::vector<occ::handle<IGESData_IGESEntity>> name_iges_solids(IGESControl_Writer& w, const std::vector<TopoDS_Shape>& shapes, rust::Slice<const rust::String> names,
                                                              rust::Slice<const double> colours)
{
  const occ::handle<Transfer_FinderProcess>& fp = w.TransferProcess();
  std::map<std::array<double, 3>, occ::handle<IGESGraph_Color>> defined;
  std::vector<occ::handle<IGESData_IGESEntity>>                 out;
  for (size_t i = 0; i < shapes.size(); ++i)
  {
    const auto [name, rgb] = name_colour(names, colours, i);
    out.push_back(occ::down_cast<IGESData_IGESEntity>(fp->FindTransient(TransferBRep::ShapeMapper(fp, shapes[i]))));
    const occ::handle<IGESSolid_ManifoldSolid> solid = occ::down_cast<IGESSolid_ManifoldSolid>(out.back());
    if (solid.IsNull())
      continue;
    const occ::handle<IGESSolid_Shell> shell = solid->Shell();
    if (!name.empty())
    {
      // Right-aligned, subscript 0, as live's ("  Part 1       0").
      const std::string                           cut   = name.substr(0, 8);
      const occ::handle<TCollection_HAsciiString> label = new TCollection_HAsciiString((std::string(8 - cut.size(), ' ') + cut).c_str());
      solid->SetLabel(label, 0);
      // The whole name as a Name property (406 form 15) the solid points
      // to, as live's (readers take the label; live 1818).
      occ::handle<IGESBasic_Name> full = new IGESBasic_Name();
      full->Init(1, new TCollection_HAsciiString(name.c_str()));
      w.Model()->AddEntity(full);
      solid->AddProperty(full);
      if (!shell.IsNull() && shell->NbFaces() > 0 && shell->Face(1)->NbLoops() > 0 && shell->Face(1)->Loop(1)->NbEdges() > 0)
        if (const occ::handle<IGESSolid_EdgeList> edges = occ::down_cast<IGESSolid_EdgeList>(shell->Face(1)->Loop(1)->Edge(1)); !edges.IsNull())
        {
          edges->SetLabel(label, 0);
          if (!edges->StartVertexList(1).IsNull())
            edges->StartVertexList(1)->SetLabel(label, 0);
        }
    }
    if (!rgb)
      continue;
    const std::array<double, 3> key{rgb[0], rgb[1], rgb[2]};
    occ::handle<IGESGraph_Color>& colour = defined[key];
    if (colour.IsNull())
    {
      colour = new IGESGraph_Color();
      colour->Init(100.0 * rgb[0], 100.0 * rgb[1], 100.0 * rgb[2], new TCollection_HAsciiString("COLOR"));
      w.Model()->AddEntity(colour);
    }
    solid->InitColor(colour);
    for (int f = 1; !shell.IsNull() && f <= shell->NbFaces(); ++f)
      shell->Face(f)->InitColor(colour);
  }
  return out;
}

// The parts to an IGES writer in BRep mode, each with its long splines cut
// to the parts its edges use; the shapes written.
std::vector<TopoDS_Shape> add_iges_parts(IGESControl_Writer& w, const OcShapeList& shapes)
{
  if (shapes.items.empty())
    fail_invalid("nothing to export");
  std::vector<TopoDS_Shape> written;
  for (const auto& s : shapes.items)
  {
    require_non_null(s, "exported shape");
    written.push_back(compact_bspline_edges(s));
    w.AddShape(written.back());
  }
  w.ComputeModel();
  return written;
}

// IGES 5.1 laid out as Onshape's (HOOPS Exchange) export (live "qa5 iges
// header"): a Start line "File generated by <system> Version <v>", the
// Global section's product ids = the file name without extension, file name,
// system and version, 32/24/8/56/15 number limits, unit MM (flag 2), line
// weights 1000 / 1., dates YYMMDD.HHMMSS (UTC), resolution 1e-5, max
// coordinate 1e6, no author or organization; sequence numbers right-aligned
// ("S      1", not "S0000001").
void write_iges_file(IGESControl_Writer& w, const std::string& p, rust::Str system, rust::Str version)
{
  const std::string sys(system.data(), system.size()), ver(version.data(), version.size());
  std::string       file = p.substr(p.find_last_of("/\\") == std::string::npos ? 0 : p.find_last_of("/\\") + 1);
  const std::string stem = file.substr(0, file.rfind('.') == std::string::npos ? file.size() : file.rfind('.'));
  auto              h    = [](const std::string& t) { return new TCollection_HAsciiString(t.c_str()); };
  char              date[32];
  {
    const std::time_t now = std::time(nullptr);
    std::tm           tm{};
#ifdef _WIN32
    gmtime_s(&tm, &now);
#else
    gmtime_r(&now, &tm);
#endif
    std::strftime(date, sizeof date, "%y%m%d.%H%M%S", &tm);
  }
  const occ::handle<IGESData_IGESModel>& model = w.Model();
  IGESData_GlobalSection                 g     = model->GlobalSection();
  g.SetSendName(h(stem));
  g.SetFileName(h(file));
  g.SetSystemId(h(sys));
  g.SetInterfaceVersion(h(ver));
  g.SetIntegerBits(32);
  g.SetMaxPower10Single(24);
  g.SetMaxDigitsSingle(8);
  g.SetMaxPower10Double(56);
  g.SetMaxDigitsDouble(15);
  g.SetReceiveName(h(stem));
  g.SetScale(1.0);
  g.SetUnitFlag(2);
  g.SetUnitName(h("MM"));
  g.SetLineWeightGrad(1000);
  g.SetMaxLineWeight(1.0);
  g.SetDate(h(date));
  g.SetResolution(1e-5);
  g.SetMaxCoord(1e6);
  g.SetAuthorName(h(""));
  g.SetCompanyName(h(""));
  g.SetIGESVersion(9);
  g.SetDraftingStandard(0);
  g.SetLastChangeDate(h(date));
  model->SetGlobalSection(g);
  model->ClearStartSection();
  // (OCCT drops empty start lines: a space keeps the blank ones.)
  model->AddStartLine(" ");
  model->AddStartLine(("File generated by " + sys + " Version " + ver).c_str());
  model->AddStartLine(" ");
  std::ostringstream os;
  if (!w.Write(os))
    fail_io("could not write IGES file " + p);
  // The Global section exactly as Onshape's: explicit delimiters, D
  // exponents, 25 parameters (no empty application protocol).
  auto holl = [](const std::string& t) { return std::to_string(t.size()) + "H" + t; };
  const std::string global = "1H,,1H;," + holl(stem) + "," + holl(file) + "," + holl(sys) + "," + holl(ver) + ",32,24,8,56,15," + holl(stem) +
                             ",1.,2,2HMM,1000,1.,13H" + date + ",1D-05,1000000.,,,9,0,13H" + date + ";";
  // Records of 80 columns: sequence numbers right-aligned (as Onshape's),
  // the G section rebuilt, the T counts to match.
  std::istringstream in(os.str());
  std::string        line, out;
  int                counts[5] = {0, 0, 0, 0, 0}; // S G D P T
  auto               record    = [&](const std::string& body, char section) {
    const int i = std::string("SGDPT").find(section);
    const std::string n = std::to_string(++counts[i]);
    std::string       r = body.substr(0, 72);
    r.resize(72, ' ');
    out += r + section + std::string(7 - n.size(), ' ') + n + "\n";
  };
  bool global_done = false;
  while (std::getline(in, line))
  {
    if (!line.empty() && line.back() == '\r')
      line.pop_back();
    if (line.size() < 73)
      continue;
    const char section = line[72];
    if (section == 'G')
    {
      if (!global_done)
        for (size_t at = 0; at < global.size(); at += 72)
          record(global.substr(at, 72), 'G');
      global_done = true;
      continue;
    }
    if (section == 'T')
    {
      auto f = [](char c, int n) { const std::string s = std::to_string(n); return std::string(1, c) + std::string(7 - s.size(), ' ') + s; };
      record(f('S', counts[0]) + f('G', counts[1]) + f('D', counts[2]) + f('P', counts[3]), 'T');
      continue;
    }
    if (section == 'P')
    {
      // The directory back-pointer right-aligned (OCCT pads it with zeros).
      std::string body = line.substr(0, 72);
      for (size_t c = 65; c + 1 < 72 && body[c] == '0'; ++c)
        body[c] = ' ';
      record(body, section);
    }
    else if (section == 'S' || section == 'D')
      record(line.substr(0, 72), section);
  }
  std::ofstream f(p, std::ios::binary);
  f << out;
  if (!f)
    fail_io("could not write IGES file " + p);
}

void write_iges(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Slice<const double> colours, rust::Str path, rust::Str system,
                rust::Str version)
{
  const LockedSection locked_section_600;
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  IGESControl_Writer w("MM", 1);
  name_iges_solids(w, add_iges_parts(w, shapes), names, colours);
  write_iges_file(w, path_str(path), system, version);
}

// An assembly as Onshape's IGES export writes one (live 1755
// dx-big-asm.igs): each part once, its solid in a Subfigure Definition (308)
// with both marked dependent, and each instance a Singular Subfigure
// Instance (408) placed by a Transformation Matrix (124). `tf`: 12 numbers an
// instance (3 rows of rotation and translation, mm).
void write_iges_assembly(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Slice<const double> colours, rust::Slice<const uint32_t> part,
                         rust::Slice<const double> tf, rust::Str path, rust::Str system, rust::Str version)
{
  const LockedSection locked_section_iges_asm;
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  IGESControl_Writer                                   w("MM", 1);
  const occ::handle<IGESData_IGESModel>&               model = w.Model();
  std::vector<occ::handle<IGESBasic_SubfigureDef>>     defs;
  for (const occ::handle<IGESData_IGESEntity>& e : name_iges_solids(w, add_iges_parts(w, shapes), names, colours))
  {
    if (e.IsNull())
      fail_io("could not write a part to IGES");
    e->InitStatus(0, 2, 2, 1);
    occ::handle<NCollection_HArray1<occ::handle<IGESData_IGESEntity>>> members = new NCollection_HArray1<occ::handle<IGESData_IGESEntity>>(1, 1);
    members->SetValue(1, e);
    occ::handle<IGESBasic_SubfigureDef> def = new IGESBasic_SubfigureDef();
    def->Init(1, occ::handle<TCollection_HAsciiString>(), members);
    def->InitStatus(0, 2, 2, 1);
    model->AddEntity(def);
    defs.push_back(def);
  }
  if (tf.size() != 12 * part.size())
    fail_invalid("instance transforms");
  for (size_t i = 0; i < part.size(); ++i)
  {
    if (part[i] >= defs.size())
      fail_invalid("instance part");
    occ::handle<NCollection_HArray2<double>> m = new NCollection_HArray2<double>(1, 3, 1, 4);
    for (int r = 0; r < 3; ++r)
      for (int c = 0; c < 4; ++c)
        m->SetValue(r + 1, c + 1, tf[12 * i + 4 * (size_t)r + (size_t)c]);
    occ::handle<IGESGeom_TransformationMatrix> place = new IGESGeom_TransformationMatrix();
    place->Init(m);
    place->InitStatus(0, 1, 2, 0);
    model->AddEntity(place);
    occ::handle<IGESBasic_SingularSubfigure> inst = new IGESBasic_SingularSubfigure();
    inst->Init(defs[part[i]], gp_XYZ(0., 0., 0.), true, 1.);
    inst->InitTransf(place);
    inst->InitStatus(0, 0, 0, 1);
    model->AddEntity(inst);
  }
  write_iges_file(w, path_str(path), system, version);
}

} // namespace noshape
