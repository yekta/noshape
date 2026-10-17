// I/O: BREP (text, binary), STEP (with XCAF names/colors), IGES.
//
// All STEP/IGES exchange happens in OCCT's default session unit (millimeter);
// unit conversion is done by the Rust layer.
#include "internal.hpp"

#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BinTools.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
#include <IGESControl_Reader.hxx>
#include <IGESControl_Writer.hxx>
#include <Quantity_Color.hxx>
#include <STEPCAFControl_Reader.hxx>
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
};

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
  Quantity_Color c3;
  if (label_color(ct, ref, s, c3))
  {
    c        = c3;
    hasColor = true;
  }
  leaf.hasColor = hasColor;
  leaf.color    = c;
  out.push_back(leaf);
}

} // namespace

// ---------------------------------------------------------------- BREP

void write_brep_file(const OcShape& s, rust::Str path)
{
  require_non_null(s.shape, "shape");
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

StepImport read_step(rust::Str path)
{
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
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
    if (r.Transfer(doc))
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
      }
      cafOk = !leaves.empty();
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
  return out;
}

void write_step(const OcShapeList& shapes, rust::Slice<const rust::String> names,
                rust::Slice<const float> colors, rust::Slice<const bool> has_color,
                rust::Str path)
{
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
  for (size_t i = 0; i < shapes.items.size(); ++i)
  {
    const TopoDS_Shape& s = shapes.items[i];
    require_non_null(s, "exported shape");
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
  bool ok = w.Transfer(doc, STEPControl_AsIs);
  if (ok)
    ok = w.Write(p.c_str()) == IFSelect_RetDone;
  app->Close(doc);
  if (!ok)
    fail_io("could not write STEP file " + p);
}

// ---------------------------------------------------------------- IGES

std::unique_ptr<OcShape> read_iges(rust::Str path)
{
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
  return wrap(s);
}

void write_iges(const OcShapeList& shapes, rust::Str path)
{
  std::lock_guard<std::mutex> lock(xde_mutex());
  quiet_messenger();
  const std::string           p = path_str(path);
  if (shapes.items.empty())
    fail_invalid("nothing to export");
  IGESControl_Writer w("MM", 1);
  for (const auto& s : shapes.items)
  {
    require_non_null(s, "exported shape");
    w.AddShape(s);
  }
  w.ComputeModel();
  if (!w.Write(p.c_str()))
    fail_io("could not write IGES file " + p);
}

} // namespace noshape
