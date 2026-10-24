// I/O: BREP (text, binary), STEP (with XCAF names/colors), IGES.
//
// All STEP/IGES exchange happens in OCCT's default session unit (millimeter);
// unit conversion is done by the Rust layer.
#include "internal.hpp"
#include <BRepBuilderAPI_Transform.hxx>
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
#include <IGESControl_Writer.hxx>
#include <IGESData_GlobalSection.hxx>
#include <IGESData_IGESModel.hxx>
#include <TCollection_HAsciiString.hxx>
#include <ctime>
#include <Quantity_Color.hxx>
#include <STEPCAFControl_Reader.hxx>
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
  // Faces with their own colour (TopExp::MapShapes order).
  std::vector<std::pair<int, Quantity_Color>> faceColors;
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
  // Face colours sit on the prototype's sub-shapes; a placed copy keeps
  // their order.
  {
    ShapeIMap faces;
    TopExp::MapShapes(s, TopAbs_FACE, faces);
    for (int i = 1; i <= faces.Extent(); ++i)
    {
      Quantity_Color fc;
      if (ct->GetColor(faces(i), XCAFDoc_ColorSurf, fc) || ct->GetColor(faces(i), XCAFDoc_ColorGen, fc))
        leaf.faceColors.emplace_back(i - 1, fc);
    }
  }
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
                rust::Str path, uint8_t schema, uint8_t file_unit)
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

// IGES 5.1 laid out as Onshape's (HOOPS Exchange) export (live "qa5 iges
// header"): a Start line "File generated by <system> Version <v>", the
// Global section's product ids = the file name without extension, file name,
// system and version, 32/24/8/56/15 number limits, unit MM (flag 2), line
// weights 1000 / 1., dates YYMMDD.HHMMSS (UTC), resolution 1e-5, max
// coordinate 1e6, no author or organization; sequence numbers right-aligned
// ("S      1", not "S0000001").
void write_iges(const OcShapeList& shapes, rust::Str path, rust::Str system, rust::Str version)
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
    if (section == 'S' || section == 'D' || section == 'P')
      record(line.substr(0, 72), section);
  }
  std::ofstream f(p, std::ios::binary);
  f << out;
  if (!f)
    fail_io("could not write IGES file " + p);
}

} // namespace noshape
