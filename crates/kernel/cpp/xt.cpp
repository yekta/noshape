// Parasolid transmit files (XT): writer for text (.x_t) and neutral binary
// (.x_b), schema SCH_1200000_12006, the version the published "Parasolid XT
// Format Reference" asks external writers to use (every later Parasolid
// reads it).
//
// Each shape becomes a body. A solid body has the infinite void region, one
// solid region per OCCT solid and one void region per cavity; every face is a
// back face of its solid region's shell and a front face of the void side's
// shell. Surfaces and curves are written as planes, cylinders, cones,
// spheres, tori, lines, circles and ellipses where they are, otherwise as
// non-periodic B-splines; surfaces of revolution are spun surfaces (periodic
// like Parasolid's own: a revolved spline is one face, Onshape G21), surfaces
// of extrusion and offset are converted first (same parameterisation), and
// faces closed on a B-spline surface are split (an XT B-surface attached to a
// face may not be closed). Seam and degenerate edges of the periodic surfaces
// are dropped: an XT cylinder face is bounded by its two circles alone.
//
// Edges whose curve lies on both faces within Parasolid's precision (1e-8 m)
// are accurate. OCCT's approximated intersections (crossing cylinders: 4e-8)
// get a curve refitted through exact intersection points, as Parasolid's own
// intersection edges are exact (Onshape G21: the tolerant edges cost +0.03 mm³
// on the crossing cylinders' volume). Edges that still miss are tolerant: no
// edge curve, each fin carries a trimmed SP-curve (the OCCT pcurve on the XT
// surface), and their vertices are tolerant too. Accurate edges at tolerant
// vertices carry trimmed curves.
//
// Senses: an edge runs along its curve; the face normal is the outward normal
// (solid) or the OCCT face normal (sheet), and face->sense says whether it
// agrees with the XT surface's natural normal (computed from the XT
// definition: a cone's points away from its axis). Fins follow the OCCT edge
// orientation in the face, which keeps the face on their left.

#include "internal.hpp"

#include <BRepAdaptor_Surface.hxx>
#include <BRepTools_Modifier.hxx>
#include <ShapeCustom_SweptToElementary.hxx>
#include <BRepClass3d.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomConvert.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Circle.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <Geom_BezierCurve.hxx>
#include <Precision.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <math_Gauss.hxx>
#include <math_Matrix.hxx>
#include <math_Vector.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_SurfaceOfRevolution.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepLib.hxx>
#include <BRep_Builder.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <Geom2d_Line.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <ShapeCustom.hxx>
#include <ShapeFix_Face.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Wire.hxx>
#include <ShapeUpgrade_ShapeDivideClosed.hxx>
#include <ShapeUpgrade_ShapeDivideClosedEdges.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <ctime>
#include <functional>
#include <fstream>
#include <sstream>
#include <map>
#include <string>
#include <vector>

namespace noshape {

namespace {

// ---------------------------------------------------------------- nodes

// Field types of the schema: d int, p pointer, f double, v vector, c char,
// l logical, u byte, n short.
struct Field
{
  char      type;
  long long i    = 0;
  double    x[3] = {0, 0, 0};
  bool      null = false;
};

Field fd(long long v)
{
  Field f{'d'};
  f.i = v;
  return f;
}
Field fp(int v)
{
  Field f{'p'};
  f.i = v;
  return f;
}
Field fu(int v)
{
  Field f{'u'};
  f.i = v;
  return f;
}
Field fn(int v)
{
  Field f{'n'};
  f.i = v;
  return f;
}
Field fc(char v)
{
  Field f{'c'};
  f.i = v;
  return f;
}
Field fl(bool v)
{
  Field f{'l'};
  f.i = v ? 1 : 0;
  return f;
}
Field ff(double v)
{
  Field f{'f'};
  f.x[0] = v;
  return f;
}
Field ff_null()
{
  Field f{'f'};
  f.null = true;
  return f;
}
Field fv_null()
{
  Field f{'v'};
  f.null = true;
  return f;
}
Field fv(const gp_XYZ& v)
{
  Field f{'v'};
  f.x[0] = v.X();
  f.x[1] = v.Y();
  f.x[2] = v.Z();
  return f;
}

struct Node
{
  int                type  = 0;
  int                index = 0;
  bool               variable = false; // the last fields form the variable-length array
  int                var_count = 0;
  std::vector<Field> fields;
};

// Node layouts per version (shared with the reader): the base schema and
// the node types each Parasolid version lays out differently.
struct FieldSpec
{
  std::string name;
  char        type = 'd';
  int         n    = 0;
};
struct BaseType
{
  const char*            name;
  std::vector<FieldSpec> fields;
};
#include "xt_schema.inc"

// The Export dialog's Parasolid versions (live G36: Onshape's file of each
// version names the body's schema in its first line; the 12.0 schema is
// the one the "Parasolid XT Format Reference" documents).
struct XtVersion
{
  const char* name;
  const char* schema;
};

// The writing modeller's id (major minor build), the same in every
// version's file (live G36: "modeller version 3801231", FRU 38.1.231).
const char* const kModeller = "3801231";
const XtVersion kVersions[] = {
  {"38.1", "SCH_3801231_38101"}, {"37.1", "SCH_3701000_37102"}, {"37.0", "SCH_3700000_36001"},
  {"36.0", "SCH_3600000_36001"}, {"35.1", "SCH_3501000_35102"}, {"34.1", "SCH_3401000_34101"},
  {"34.0", "SCH_3400000_34001"}, {"33.1", "SCH_3301000_33103"}, {"33.0", "SCH_3300000_32001"},
  {"32.1", "SCH_3201000_32001"}, {"32.0", "SCH_3200000_32001"}, {"31.1", "SCH_3101000_31100"},
  {"31.0", "SCH_3100000_31001"}, {"30.1", "SCH_3001000_30100"}, {"30.0", "SCH_3000000_30000"},
  {"29.1", "SCH_2901000_28101"}, {"29.0", "SCH_2900000_28101"}, {"28.1", "SCH_2801000_28101"},
  {"28.0", "SCH_2800000_28002"}, {"27.1", "SCH_2701000_26105"}, {"27.0", "SCH_2700000_26105"},
  {"26.1", "SCH_2601000_26105"}, {"26.0", "SCH_2600000_25001"}, {"25.1", "SCH_2501000_25001"},
  {"25.0", "SCH_2500000_25001"}, {"12.0", "SCH_1200000_12006"},
};

XtVersion xt_version(const std::string& name)
{
  for (const XtVersion& v : kVersions)
    if (name == v.name)
      return v;
  if (name.empty())
    return kVersions[0];
  fail_invalid("unknown Parasolid version " + name);
  return kVersions[0];
}

// The layout code of a schema (as the reader computes it: 38101 -> 38100).
int schema_code(const XtVersion& v)
{
  const std::string s  = v.schema;
  const long long   sc = std::atoll(s.substr(s.rfind('_') + 1).c_str());
  return (int)(sc / 1000 * 1000 + sc % 1000 / 100 * 100);
}

// Our nodes (base-schema layouts) laid out as version `v` lays them out:
// fields by name, the ones the version adds 0 / null (as Onshape's files
// have them, live G36), the ones it drops left out.
std::vector<Node> to_version(std::vector<Node> nodes, const XtVersion& v)
{
  const int code = schema_code(v);
  if (code <= 13006)
    return nodes;
  std::map<int, const std::vector<FieldSpec>*> layouts;
  for (const auto& [c, types] : version_schemas())
    if (c <= code)
      for (const auto& [t, f] : types)
        layouts[t] = &f;
  const auto& base = base_schema();
  for (Node& n : nodes)
  {
    auto lt = layouts.find(n.type);
    auto bt = base.find(n.type);
    if (lt == layouts.end() || bt == base.end())
      continue;
    const std::vector<FieldSpec>& bf = bt->second.fields;
    // Each base field's values (a variable-length one takes the rest).
    std::map<std::string, std::vector<Field>> by_name;
    size_t                                    at = 0;
    for (const FieldSpec& f : bf)
    {
      const size_t k = f.n == 1 ? n.fields.size() - std::min(at, n.fields.size()) : (size_t)std::max(1, f.n);
      std::vector<Field> vals(n.fields.begin() + (long)std::min(at, n.fields.size()), n.fields.begin() + (long)std::min(at + k, n.fields.size()));
      by_name[f.name] = std::move(vals);
      at += k;
    }
    std::vector<Field> out;
    for (const FieldSpec& f : *lt->second)
    {
      auto it = by_name.find(f.name);
      if (it != by_name.end())
      {
        out.insert(out.end(), it->second.begin(), it->second.end());
        continue;
      }
      const int k = std::max(1, f.n == 1 ? 0 : f.n);
      for (int i = 0; i < k && f.n != 1; ++i)
        switch (f.type)
        {
          case 'f':
            out.push_back(ff_null());
            break;
          case 'v':
            out.push_back(fv_null());
            break;
          case 'c':
            out.push_back(fc('?'));
            break;
          case 'l':
            out.push_back(fl(false));
            break;
          case 'u':
            out.push_back(fu(0));
            break;
          case 'p':
            out.push_back(fp(0));
            break;
          default:
            out.push_back(fd(0));
        }
    }
    n.fields = std::move(out);
  }
  return nodes;
}

std::string num(double v)
{
  if (v == 0)
    return "0";
  char t[40];
  for (int prec = 1; prec <= 17; ++prec)
  {
    std::snprintf(t, sizeof t, "%.*g", prec, v);
    if (std::strtod(t, nullptr) == v)
      break;
  }
  // Exponents as Parasolid writes them: 1e3, 1e-8.
  std::string r = t;
  const size_t e = r.find('e');
  if (e != std::string::npos)
  {
    std::string m = r.substr(0, e), x = r.substr(e + 1);
    const bool  neg = x[0] == '-';
    if (x[0] == '-' || x[0] == '+')
      x.erase(0, 1);
    x.erase(0, std::min(x.find_first_not_of('0'), x.size() - 1));
    r = m + "e" + (neg ? "-" : "") + x;
  }
  return r;
}

std::string text_body(const std::vector<Node>& nodes, const XtVersion& v)
{
  const std::string kVersion = std::string(": TRANSMIT FILE created by modeller version ") + kModeller;
  const std::string kSchema  = v.schema;
  std::string       s        = "T";
  auto        put = [&](const std::string& t) {
    s += t;
    s += ' ';
  };
  put(std::to_string(kVersion.size()));
  s += kVersion;
  put(std::to_string(kSchema.size()));
  s += kSchema;
  put("0");
  for (const Node& n : nodes)
  {
    put(std::to_string(n.type));
    if (n.variable)
      put(std::to_string(n.var_count));
    put(std::to_string(n.index));
    for (const Field& f : n.fields)
    {
      if (f.null)
      {
        s += '?';
        continue;
      }
      switch (f.type)
      {
        case 'c':
          s += (char)f.i;
          break;
        case 'l':
          s += f.i ? 'T' : 'F';
          break;
        case 'f':
          put(num(f.x[0]));
          break;
        case 'v':
          for (int k = 0; k < 3; ++k)
            put(num(f.x[k]));
          break;
        default:
          put(std::to_string(f.i));
      }
    }
  }
  put("1");
  put("0");
  // Records of about 80 characters, broken before a separating space (a
  // record's trailing spaces are ignored on reading, leading ones are not).
  std::string out;
  size_t      line = 0;
  for (char ch : s)
  {
    if (ch == ' ' && line >= 72)
    {
      out += '\n';
      line = 0;
    }
    out += ch;
    ++line;
  }
  while (!out.empty() && out.back() == ' ')
    out.pop_back();
  out += '\n';
  return out;
}

struct Bin
{
  std::string s;
  void        u8(int v) { s += static_cast<char>(v & 0xff); }
  void        i16(int v)
  {
    u8(v >> 8);
    u8(v);
  }
  void i32(long long v)
  {
    for (int k = 3; k >= 0; --k)
      u8((int)(v >> (8 * k)));
  }
  void f64(double v)
  {
    uint64_t b;
    std::memcpy(&b, &v, 8);
    for (int k = 7; k >= 0; --k)
      u8((int)(b >> (8 * k)));
  }
  void ptr(long long index)
  {
    if (index < 32767)
      i16((int)index + 1);
    else
    {
      i16(-(int)(index % 32767 + 1));
      i16((int)(index / 32767));
    }
  }
};

std::string binary_body(const std::vector<Node>& nodes, const XtVersion& v)
{
  const std::string kVersion  = std::string(": TRANSMIT FILE created by modeller version ") + kModeller;
  const std::string kSchema   = v.schema;
  const double      kNullReal = -3.14158e13;
  Bin               b;
  b.s = std::string("PS\0\0", 4);
  b.i16((int)kVersion.size());
  b.s += kVersion;
  b.i32((long long)kSchema.size());
  b.s += kSchema;
  b.i32(0);
  for (const Node& n : nodes)
  {
    b.i16(n.type);
    if (n.variable)
      b.i32(n.var_count);
    b.ptr(n.index);
    for (const Field& f : n.fields)
      switch (f.type)
      {
        case 'd':
          b.i32(f.i);
          break;
        case 'p':
          b.ptr(f.i);
          break;
        case 'n':
          b.i16((int)f.i);
          break;
        case 'u':
        case 'c':
        case 'l':
          b.u8((int)f.i);
          break;
        case 'f':
          b.f64(f.null ? kNullReal : f.x[0]);
          break;
        case 'v':
          for (int k = 0; k < 3; ++k)
            b.f64(f.null ? kNullReal : f.x[k]);
          break;
      }
  }
  b.i16(1);
  b.ptr(0);
  return b.s;
}

// The header as Onshape's exports have it (live G25/G36: one key per line,
// unknown machine fields, the file name, a UTC date; part 2 names the
// writing modeller's schema whatever the body's version).
std::string header(bool binary, const std::string& path, const std::string& application)
{
  const size_t      slash = path.find_last_of("/\\");
  const std::string file  = slash == std::string::npos ? path : path.substr(slash + 1);
  char              date[40];
  {
    const std::time_t now = std::time(nullptr);
    std::tm           tm{};
#ifdef _WIN32
    gmtime_s(&tm, &now);
#else
    gmtime_r(&now, &tm);
#endif
    std::strftime(date, sizeof date, "%Y-%m-%dT%H:%M:%S (UTC)", &tm);
  }
  std::string h;
  h += "**ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz**************************\n";
  h += "**PARASOLID !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~0123456789**************************\n";
  h += "**PART1;\nMC=unknown;\nMC_MODEL=unknown;\nMC_ID=unknown;\nOS=unknown;\nOS_RELEASE=unknown;\n";
  h += "FRU=" + application + ";\nAPPL=" + application + ";\nSITE=unknown;\nUSER=unknown;\n";
  h += std::string("FORMAT=") + (binary ? "binary" : "text") + ";\nGUISE=transmit;\n";
  h += "KEY=" + file + ";\nFILE=" + file + ";\nDATE=" + date + ";\n";
  h += std::string("**PART2;\nSCH=") + kVersions[0].schema + ";\nUSFLD_SIZE=0;\n";
  h += "**PART3;\n";
  h += "**END_OF_HEADER*****************************************************************\n";
  return h;
}

// ---------------------------------------------------------------- geometry

// An XT surface: its node, how OCCT (u, v) map to XT (u, v) (u' = su u + ou,
// v' = sv v; swapped first for spun surfaces, whose XT u runs along the
// profile), and its natural normal at a point.
struct XSurface
{
  int                                       index = 0;
  double                                    su = 1, ou = 0, sv = 1;
  bool                                      swap = false;
  std::function<gp_Vec(const gp_Pnt&, double, double)> natural; // point, OCCT u, v
};

struct Writer
{
  std::vector<Node>     nodes;
  std::map<int, size_t> pos; // node index -> position in `nodes`
  int                   next_index = 1;
  int                   next_id    = 1;

  int alloc() { return next_index++; }

  Node& add(int type, int index)
  {
    pos[index] = nodes.size();
    nodes.push_back(Node{type, index});
    return nodes.back();
  }

  Node& node(int index) { return nodes[pos.at(index)]; }

  // Common leading fields of curves and surfaces.
  static void geom_head(Node& n, int id, int owner, char sense)
  {
    n.fields = {fd(id), fp(0), fp(owner), fp(0), fp(0), fp(0), fc(sense)};
  }

  int knot_nodes(const NCollection_Array1<double>& knots, const NCollection_Array1<int>& mults, int& mult_index)
  {
    mult_index = alloc();
    Node& m    = add(127, mult_index);
    m.variable = true;
    m.var_count = mults.Length();
    for (int i = mults.Lower(); i <= mults.Upper(); ++i)
      m.fields.push_back(fn(mults(i)));
    const int k = alloc();
    Node&     kn = add(128, k);
    kn.variable  = true;
    kn.var_count = knots.Length();
    for (int i = knots.Lower(); i <= knots.Upper(); ++i)
      kn.fields.push_back(ff(knots(i)));
    return k;
  }

  int vertices_node(const std::vector<double>& v)
  {
    const int i = alloc();
    Node&     n = add(45, i);
    n.variable  = true;
    n.var_count = (int)v.size();
    for (double x : v)
      n.fields.push_back(ff(x));
    return i;
  }

  // B_CURVE + NURBS_CURVE from a non-periodic B-spline (3D or 2D poles).
  int b_curve(int owner, const std::vector<double>& poles, int dim, bool rational, int degree, int npoles,
              const NCollection_Array1<double>& knots, const NCollection_Array1<int>& mults, bool closed)
  {
    const int idx = alloc(), id = next_id++;
    const int nurbs = alloc();
    const int verts = vertices_node(poles);
    int       mult  = 0;
    const int kn    = knot_nodes(knots, mults, mult);
    Node&     nc    = add(136, nurbs);
    nc.fields       = {fn(degree), fd(npoles), fn(dim), fd(knots.Length()), fu(1), fl(false), fl(closed), fl(rational),
                       fu(1),      fp(verts),  fp(mult), fp(kn)};
    Node& b = add(134, idx);
    geom_head(b, id, owner, '+');
    b.fields.push_back(fp(nurbs));
    b.fields.push_back(fp(0));
    return idx;
  }

  int b_curve_3d(int owner, const occ::handle<Geom_BSplineCurve>& c)
  {
    const bool          rat = c->IsRational();
    std::vector<double> p;
    for (int i = 1; i <= c->NbPoles(); ++i)
    {
      const gp_Pnt q = c->Pole(i);
      const double w = rat ? c->Weight(i) : 1.0;
      p.insert(p.end(), {q.X() * w, q.Y() * w, q.Z() * w});
      if (rat)
        p.push_back(w);
    }
    return b_curve(owner, p, rat ? 4 : 3, rat, c->Degree(), c->NbPoles(), c->Knots(), c->Multiplicities(), c->IsClosed());
  }
};

occ::handle<Geom_BSplineCurve> to_bspline(const occ::handle<Geom_Curve>& c, double f, double l)
{
  occ::handle<Geom_BSplineCurve> b = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(c, f, l));
  if (b->IsPeriodic())
  {
    b->SetNotPeriodic();
    b->Segment(f, l);
  }
  return b;
}

occ::handle<Geom_Surface> basis_surface(occ::handle<Geom_Surface> s)
{
  while (auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
    s = t->BasisSurface();
  return s;
}

occ::handle<Geom_Curve> basis_curve(occ::handle<Geom_Curve> c)
{
  while (auto t = occ::down_cast<Geom_TrimmedCurve>(c))
    c = t->BasisCurve();
  return c;
}

// A surface of revolution written as an XT spun surface: its profile is a
// curve the writer keeps with its parameterisation.
bool spun(const occ::handle<Geom_Surface>& s)
{
  auto r = occ::down_cast<Geom_SurfaceOfRevolution>(s);
  if (r.IsNull())
    return false;
  const occ::handle<Geom_Curve> c = basis_curve(r->BasisCurve());
  return !occ::down_cast<Geom_Line>(c).IsNull() || !occ::down_cast<Geom_Circle>(c).IsNull() || !occ::down_cast<Geom_Ellipse>(c).IsNull() ||
         !occ::down_cast<Geom_BSplineCurve>(c).IsNull() || !occ::down_cast<Geom_BezierCurve>(c).IsNull();
}

bool periodic_analytic(const occ::handle<Geom_Surface>& s)
{
  return !occ::down_cast<Geom_CylindricalSurface>(s).IsNull() || !occ::down_cast<Geom_ConicalSurface>(s).IsNull() ||
         !occ::down_cast<Geom_SphericalSurface>(s).IsNull() || !occ::down_cast<Geom_ToroidalSurface>(s).IsNull() || spun(s);
}

// ---------------------------------------------------------------- body

constexpr double kPrecision = 1e-8; // Parasolid linear precision (metres)

struct XFin
{
  int  index = 0, loop = 0, forward = 0, backward = 0, vertex = 0, other = 0, edge = 0, curve = 0, next_at_vx = 0;
  bool positive = true;
  bool dummy    = false;
  bool isolated = false; // the single fin of an isolated loop (a cone's apex)
  int  face     = -1;    // XFace index
  TopoDS_Edge occ;    // as used in the face
};

struct BodyWriter
{
  Writer& w;
  int     body;
  bool    solid_body;

  ShapeIMap faces, edges, vertices;
  std::vector<int>       face_idx, face_surface, face_shell, face_front;
  std::vector<XSurface>  surfaces;
  std::vector<char>      face_sense;
  std::vector<std::vector<int>> face_loops;
  std::vector<XFin>      fins;
  std::vector<std::vector<int>> edge_fins; // per edge: fin indices into `fins`
  std::vector<int>       edge_idx, vertex_idx, vertex_point;
  std::vector<double>    edge_tol, vertex_tol; // 0 = accurate
  std::vector<int>       edge_curve;
  std::vector<occ::handle<Geom_Curve>> edge_fit;          // refitted curves (same range), or null
  std::map<int, gp_XYZ>                vertex_fit;        // vertex index -> point on refitted ends
  std::vector<int>       chain_surface, chain_curve, chain_point; // boundary chains (node indices)

  BodyWriter(Writer& w_, int body_, bool solid_) : w(w_), body(body_), solid_body(solid_) {}

  Node& node(int index) { return w.node(index); }
  Node& add(int type, int index) { return w.add(type, index); }

  // ---- surfaces

  XSurface surface_of(const TopoDS_Face& f, int face_index)
  {
    TopLoc_Location           loc;
    occ::handle<Geom_Surface> s = basis_surface(BRep_Tool::Surface(f, loc));
    if (!loc.IsIdentity())
      s = occ::down_cast<Geom_Surface>(s->Transformed(loc.Transformation()));
    XSurface  xs;
    const int idx = w.alloc(), id = w.next_id++;
    xs.index      = idx;
    chain_surface.push_back(idx);
    auto head = [&](int type) -> Node& {
      Node& n = add(type, idx);
      Writer::geom_head(n, id, face_index, '+');
      return n;
    };
    if (auto p = occ::down_cast<Geom_Plane>(s))
    {
      const gp_Ax3 a = p->Position();
      const gp_XYZ n = a.XDirection().XYZ().Crossed(a.YDirection().XYZ());
      Node&        nd = head(50);
      nd.fields.insert(nd.fields.end(), {fv(a.Location().XYZ()), fv(n), fv(a.XDirection().XYZ())});
      xs.natural = [n](const gp_Pnt&, double, double) { return gp_Vec(n); };
      return xs;
    }
    if (auto c = occ::down_cast<Geom_CylindricalSurface>(s))
    {
      const gp_Ax3 a = c->Position();
      const gp_XYZ ax = a.XDirection().XYZ().Crossed(a.YDirection().XYZ());
      if (!a.Direct())
        xs.sv = -1; // the XT axis is the other way
      Node& nd = head(51);
      nd.fields.insert(nd.fields.end(), {fv(a.Location().XYZ()), fv(ax), ff(c->Radius()), fv(a.XDirection().XYZ())});
      const gp_Pnt o = a.Location();
      xs.natural     = [o, ax](const gp_Pnt& q, double, double) {
        gp_Vec r(o, q);
        return r - gp_Vec(ax) * r.Dot(gp_Vec(ax));
      };
      return xs;
    }
    if (auto c = occ::down_cast<Geom_ConicalSurface>(s))
    {
      // XT: R = P + v A + (X cos u + Y sin u)(r + v tan a), Y = A x X, A the
      // way the cone widens (as Onshape and NX write cones; the XT
      // reference's formula has the opposite sign).
      const gp_Ax3 a     = c->Position();
      const double alpha = c->SemiAngle();
      const gp_XYZ z     = a.Direction().XYZ();
      const gp_XYZ A     = alpha > 0 ? z : z.Reversed();
      const gp_XYZ X     = a.XDirection().XYZ();
      // XT u turns about A; OCCT's about Z (or -Z when indirect).
      const bool same_turn = (A.Dot(a.XDirection().XYZ().Crossed(a.YDirection().XYZ())) > 0);
      xs.su                = same_turn ? 1 : -1;
      xs.sv                = alpha > 0 ? std::cos(alpha) : -std::cos(alpha);
      const double h       = std::abs(alpha);
      Node&        nd      = head(52);
      nd.fields.insert(nd.fields.end(), {fv(a.Location().XYZ()), fv(A), ff(c->RefRadius()), ff(std::sin(h)), ff(std::cos(h)), fv(X)});
      const gp_Pnt o = a.Location();
      const double t = std::tan(h);
      xs.natural     = [o, A, t](const gp_Pnt& q, double, double) {
        gp_Vec r(o, q);
        gp_Vec radial = r - gp_Vec(A) * r.Dot(gp_Vec(A));
        if (radial.Magnitude() > 0)
          radial.Normalize();
        return radial - gp_Vec(A) * t;
      };
      return xs;
    }
    if (auto sp = occ::down_cast<Geom_SphericalSurface>(s))
    {
      const gp_Ax3 a = sp->Position();
      const gp_XYZ A = a.XDirection().XYZ().Crossed(a.YDirection().XYZ());
      if (!a.Direct())
        xs.sv = -1;
      Node& nd = head(53);
      nd.fields.insert(nd.fields.end(), {fv(a.Location().XYZ()), ff(sp->Radius()), fv(A), fv(a.XDirection().XYZ())});
      const gp_Pnt o = a.Location();
      xs.natural     = [o](const gp_Pnt& q, double, double) { return gp_Vec(o, q); };
      return xs;
    }
    if (auto t = occ::down_cast<Geom_ToroidalSurface>(s))
    {
      const gp_Ax3 a = t->Position();
      const gp_XYZ A = a.XDirection().XYZ().Crossed(a.YDirection().XYZ());
      if (!a.Direct())
        xs.sv = -1;
      Node& nd = head(54);
      nd.fields.insert(nd.fields.end(), {fv(a.Location().XYZ()), fv(A), ff(t->MajorRadius()), ff(t->MinorRadius()), fv(a.XDirection().XYZ())});
      const gp_Pnt o = a.Location();
      const double R = t->MajorRadius();
      xs.natural     = [o, A, R](const gp_Pnt& q, double, double) {
        gp_Vec r(o, q);
        gp_Vec radial = r - gp_Vec(A) * r.Dot(gp_Vec(A));
        if (radial.Magnitude() > 0)
          radial.Normalize();
        return gp_Vec(gp_Pnt(o.XYZ() + radial.XYZ() * R), q);
      };
      return xs;
    }
    if (spun(s))
    {
      // R(u, v) = Z(u) + (C(u) - Z(u)) cos v + A x (C(u) - Z(u)) sin v: OCCT's
      // revolution with u and v swapped (same turning sense about A).
      auto                          r = occ::down_cast<Geom_SurfaceOfRevolution>(s);
      const gp_Ax1                  a = r->Axis();
      const occ::handle<Geom_Curve> c = basis_curve(r->BasisCurve());
      const gp_XYZ                  A = a.Direction().XYZ(), O = a.Location().XYZ();
      double                        u0, u1, v0, v1;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      const double cf = c->IsPeriodic() || Precision::IsInfinite(c->FirstParameter()) ? v0 : c->FirstParameter();
      const double cl = c->IsPeriodic() || Precision::IsInfinite(c->LastParameter()) ? v1 : c->LastParameter();
      bool         closed = false;
      const int    prof   = curve_node(body, c, cf, cl, closed);
      auto radial = [&](const gp_XYZ& q) { return q - O - A * (q - O).Dot(A); };
      // Degeneracies where the profile meets the axis at an end.
      const gp_XYZ pf = c->Value(cf).XYZ(), pl = c->Value(cl).XYZ();
      const bool   df = radial(pf).Modulus() < kPrecision, dl = radial(pl).Modulus() < kPrecision;
      // x_axis when the profile is planar in a plane through the axis.
      gp_XYZ x;
      for (int i = 0; i <= 8 && x.Modulus() < 1e-12; ++i)
      {
        const gp_XYZ rq = radial(c->Value(cf + (cl - cf) * (0.5 + i / 17.0)).XYZ());
        if (rq.Modulus() > kPrecision)
          x = rq.Normalized();
      }
      bool planar = x.Modulus() > 0.5;
      for (int i = 0; i <= 32 && planar; ++i)
        planar = std::abs((c->Value(cf + (cl - cf) * i / 32.0).XYZ() - O).Dot(A.Crossed(x))) < kPrecision;
      Node& nd = head(68);
      nd.fields.insert(nd.fields.end(), {fp(prof), fv(O), fv(A), df ? fv(pf) : fv_null(), dl ? fv(pl) : fv_null(), df ? ff(cf) : ff_null(),
                                         dl ? ff(cl) : ff_null(), planar ? fv(x) : fv_null(), ff_null()});
      depend(prof, idx);
      xs.swap    = true;
      xs.natural = [r](const gp_Pnt&, double u, double v) {
        gp_Pnt q;
        gp_Vec du, dv;
        r->D1(u, v, q, du, dv);
        return dv.Crossed(du);
      };
      return xs;
    }
    // Anything else: a B-spline surface over the face's range (not closed:
    // closed faces were split beforehand).
    occ::handle<Geom_BSplineSurface> b = occ::down_cast<Geom_BSplineSurface>(s);
    if (b.IsNull() || b->IsUPeriodic() || b->IsVPeriodic())
    {
      double u0, u1, v0, v1;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      b = GeomConvert::SurfaceToBSplineSurface(new Geom_RectangularTrimmedSurface(s, u0, u1, v0, v1));
      if (b->IsUPeriodic())
      {
        b->SetUNotPeriodic();
        b->Segment(u0, u1, v0, v1);
      }
      if (b->IsVPeriodic())
      {
        b->SetVNotPeriodic();
        b->Segment(u0, u1, v0, v1);
      }
    }
    const bool          rat = b->IsURational() || b->IsVRational();
    std::vector<double> p;
    for (int i = 1; i <= b->NbUPoles(); ++i)
      for (int j = 1; j <= b->NbVPoles(); ++j) // v varies fastest
      {
        const gp_Pnt q  = b->Pole(i, j);
        const double wt = rat ? b->Weight(i, j) : 1.0;
        p.insert(p.end(), {q.X() * wt, q.Y() * wt, q.Z() * wt});
        if (rat)
          p.push_back(wt);
      }
    const int nurbs = w.alloc();
    const int verts = w.vertices_node(p);
    int       um = 0, vm = 0;
    const int uk = w.knot_nodes(b->UKnots(), b->UMultiplicities(), um);
    const int vk = w.knot_nodes(b->VKnots(), b->VMultiplicities(), vm);
    Node&     ns = w.add(126, nurbs);
    ns.fields    = {fl(false), fl(false), fn(b->UDegree()), fn(b->VDegree()), fd(b->NbUPoles()), fd(b->NbVPoles()), fu(1), fu(1),
                    fd(b->NbUKnots()), fd(b->NbVKnots()), fl(rat), fl(b->IsUClosed()), fl(b->IsVClosed()), fu(1),
                    fn(rat ? 4 : 3), fp(verts), fp(um), fp(vm), fp(uk), fp(vk)};
    Node& nd = head(124);
    nd.fields.push_back(fp(nurbs));
    nd.fields.push_back(fp(0));
    xs.natural = [b](const gp_Pnt&, double u, double v) {
      gp_Pnt q;
      gp_Vec du, dv;
      b->D1(u, v, q, du, dv);
      return du.Crossed(dv);
    };
    return xs;
  }

  // ---- curves

  // A vertex's point (moved onto refitted edge ends).
  gp_Pnt vertex_pnt(int vi) const
  {
    auto it = vertex_fit.find(vi);
    return it != vertex_fit.end() ? gp_Pnt(it->second) : BRep_Tool::Pnt(TopoDS::Vertex(vertices(vi + 1)));
  }

  // Edge i's curve: the refitted one, or the edge's own.
  occ::handle<Geom_Curve> curve_of(const TopoDS_Edge& e, int i, double& f, double& l) const
  {
    if (!edge_fit[i].IsNull())
    {
      f = edge_fit[i]->FirstParameter();
      l = edge_fit[i]->LastParameter();
      return edge_fit[i];
    }
    return curve_of(e, f, l);
  }

  // The edge's 3D curve (location applied, trims removed) and its range.
  static occ::handle<Geom_Curve> curve_of(const TopoDS_Edge& e, double& f, double& l)
  {
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
    while (auto t = occ::down_cast<Geom_TrimmedCurve>(c))
      c = t->BasisCurve();
    return c;
  }

  // Curve node for an accurate edge's curve; `closed` set if a B-spline had to
  // be closed (it then needs a trimmed curve to sit on the edge).
  int curve_node(int owner, const occ::handle<Geom_Curve>& c, double f, double l, bool& closed)
  {
    closed = false;
    auto head = [&](int type, int idx) -> Node& {
      Node& n = add(type, idx);
      Writer::geom_head(n, w.next_id++, owner, '+');
      return n;
    };
    int idx = 0;
    if (auto ln = occ::down_cast<Geom_Line>(c))
    {
      idx      = w.alloc();
      Node& nd = head(30, idx);
      nd.fields.insert(nd.fields.end(), {fv(ln->Position().Location().XYZ()), fv(ln->Position().Direction().XYZ())});
    }
    else if (auto ci = occ::down_cast<Geom_Circle>(c))
    {
      idx            = w.alloc();
      const gp_Ax2 a = ci->Position();
      Node&        nd = head(31, idx);
      nd.fields.insert(nd.fields.end(), {fv(a.Location().XYZ()), fv(a.Direction().XYZ()), fv(a.XDirection().XYZ()), ff(ci->Radius())});
    }
    else if (auto el = occ::down_cast<Geom_Ellipse>(c))
    {
      idx            = w.alloc();
      const gp_Ax2 a = el->Position();
      Node&        nd = head(32, idx);
      nd.fields.insert(nd.fields.end(), {fv(a.Location().XYZ()), fv(a.Direction().XYZ()), fv(a.XDirection().XYZ()), ff(el->MajorRadius()), ff(el->MinorRadius())});
    }
    else
    {
      occ::handle<Geom_BSplineCurve> b = to_bspline(c, f, l);
      closed                           = b->IsClosed();
      idx                              = w.b_curve_3d(owner, b);
    }
    chain_curve.push_back(idx);
    return idx;
  }

  int geometric_owner(int owner, int shared)
  {
    const int g = w.alloc();
    Node&     n = add(141, g);
    n.fields    = {fp(owner), fp(g), fp(g), fp(shared)};
    return g;
  }

  // Add a dependant to `shared`'s ring of geometric owners.
  void depend(int shared, int dependant)
  {
    const int g    = geometric_owner(dependant, shared);
    Field&    head = node(shared).fields[5];
    if (head.i == 0)
    {
      head.i = g;
      return;
    }
    // Insert g after the head in the ring.
    const int h    = (int)head.i;
    const int next = (int)node(h).fields[1].i;
    node(g).fields[1].i    = next;
    node(g).fields[2].i    = h;
    node(h).fields[1].i    = g;
    node(next).fields[2].i = g;
  }

  int trimmed(int owner, int basis, const gp_Pnt& p1, const gp_Pnt& p2, double t1, double t2)
  {
    const int idx = w.alloc();
    Node&     n   = add(133, idx);
    Writer::geom_head(n, w.next_id++, owner, '+');
    n.fields.insert(n.fields.end(), {fp(basis), fv(p1.XYZ()), fv(p2.XYZ()), ff(t1), ff(t2)});
    chain_curve.push_back(idx);
    depend(basis, idx);
    return idx;
  }

  // Trimmed SP-curve of a tolerant edge's fin: the OCCT pcurve on the face's
  // XT surface, run along the fin (Parasolid's convention, as NX writes
  // them): on a negative fin, a B-curve of negative sense trimmed from its
  // end back to its start.
  int fin_curve(int fin_node, const TopoDS_Edge& e, int face, bool positive)
  {
    const TopoDS_Face& F = TopoDS::Face(faces(face + 1));
    double             f, l;
    TopoDS_Edge        ef = e;
    ef.Orientation(TopAbs_FORWARD);
    occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(ef, F, f, l);
    if (pc.IsNull())
      fail_op("parasolid: edge without a curve on its face");
    occ::handle<Geom2d_BSplineCurve> b = Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(pc, f, l));
    if (b->IsPeriodic())
    {
      b->SetNotPeriodic();
      b->Segment(f, l);
    }
    const XSurface&     xs  = surfaces[face];
    const bool          rat = b->IsRational();
    std::vector<double> p;
    for (int i = 1; i <= b->NbPoles(); ++i)
    {
      const gp_Pnt2d q  = b->Pole(i);
      const double   wt = rat ? b->Weight(i) : 1.0;
      const double   qu = xs.swap ? q.Y() : q.X(), qv = xs.swap ? q.X() : q.Y();
      const double   u = xs.su * qu + xs.ou, v = xs.sv * qv;
      p.insert(p.end(), {u * wt, v * wt});
      if (rat)
        p.push_back(wt);
    }
    const int bc = w.b_curve(0, p, rat ? 3 : 2, rat, b->Degree(), b->NbPoles(), b->Knots(), b->Multiplicities(), false);
    const int sp = w.alloc();
    Node&     n  = add(137, sp);
    Writer::geom_head(n, w.next_id++, body, positive ? '+' : '-');
    n.fields.insert(n.fields.end(), {fp(xs.index), fp(bc), fp(0), ff_null()});
    chain_curve.push_back(sp);
    depend(xs.index, sp);
    TopLoc_Location           loc;
    occ::handle<Geom_Surface> surf = BRep_Tool::Surface(F, loc);
    auto at = [&](double t) { return surf->Value(pc->Value(t).X(), pc->Value(t).Y()).Transformed(loc.Transformation()); };
    return positive ? trimmed(fin_node, sp, at(f), at(l), f, l) : trimmed(fin_node, sp, at(l), at(f), l, f);
  }

  // The point on both faces' surfaces in the plane through x0 normal to t
  // (Newton from the pcurves' (u, v)).
  static bool on_both(const BRepAdaptor_Surface& a1, const BRepAdaptor_Surface& a2, gp_Pnt2d& p1, gp_Pnt2d& p2, const gp_Pnt& x0,
                      const gp_Vec& t, gp_Pnt& out)
  {
    double u1 = p1.X(), v1 = p1.Y(), u2 = p2.X(), v2 = p2.Y();
    for (int it = 0; it < 40; ++it)
    {
      gp_Pnt q1, q2;
      gp_Vec s1u, s1v, s2u, s2v;
      a1.D1(u1, v1, q1, s1u, s1v);
      a2.D1(u2, v2, q2, s2u, s2v);
      const gp_Vec d(q2, q1);
      const double g = t.Dot(gp_Vec(x0, q1));
      math_Matrix  J(1, 4, 1, 4);
      math_Vector  F(1, 4), X(1, 4);
      for (int k = 1; k <= 3; ++k)
      {
        J(k, 1) = s1u.Coord(k);
        J(k, 2) = s1v.Coord(k);
        J(k, 3) = -s2u.Coord(k);
        J(k, 4) = -s2v.Coord(k);
        F(k)    = -d.Coord(k);
      }
      J(4, 1) = t.Dot(s1u);
      J(4, 2) = t.Dot(s1v);
      J(4, 3) = J(4, 4) = 0;
      F(4)              = -g;
      math_Gauss G(J);
      if (!G.IsDone())
        return false;
      G.Solve(F, X);
      u1 += X(1), v1 += X(2), u2 += X(3), v2 += X(4);
      if (d.Magnitude() < 1e-14 && std::abs(g) < 1e-14 && X.Norm() < 1e-12)
      {
        out = a1.Value(u1, v1);
        p1.SetCoord(u1, v1);
        p2.SetCoord(u2, v2);
        return true;
      }
    }
    return false;
  }

  // An edge's curve through exact intersection points of its two faces (same
  // parameter range; ends in the normal planes through its vertices), or null
  // when that doesn't come within a tenth of Parasolid's precision.
  static occ::handle<Geom_Curve> refit(const TopoDS_Edge& e, const TopoDS_Face& F1, const TopoDS_Face& F2)
  {
    double                    f, l, f1, l1, f2, l2;
    occ::handle<Geom_Curve>   c  = BRep_Tool::Curve(e, f, l); // location applied
    occ::handle<Geom2d_Curve> q1 = BRep_Tool::CurveOnSurface(e, F1, f1, l1), q2 = BRep_Tool::CurveOnSurface(e, F2, f2, l2);
    if (c.IsNull() || q1.IsNull() || q2.IsNull() || F1.IsSame(F2) || std::abs(f1 - f) + std::abs(l1 - l) + std::abs(f2 - f) + std::abs(l2 - l) > 1e-9 * (1 + std::abs(l - f)))
      return {};
    const BRepAdaptor_Surface a1(F1, false), a2(F2, false);
    const gp_Pnt              vf = BRep_Tool::Pnt(TopExp::FirstVertex(e)), vl = BRep_Tool::Pnt(TopExp::LastVertex(e));
    // Exact point at t, from x0 (the curve's point, or a vertex at the ends).
    auto exact = [&](double t, const gp_Pnt* x0, gp_Pnt& out) {
      gp_Pnt p;
      gp_Vec d;
      c->D1(t, p, d);
      if (d.Magnitude() < 1e-12)
        return false;
      gp_Pnt2d p1 = q1->Value(t), p2 = q2->Value(t);
      return on_both(a1, a2, p1, p2, x0 ? *x0 : p, d.Normalized(), out);
    };
    for (int n = 64; n <= 1024; n *= 2)
    {
      occ::handle<NCollection_HArray1<gp_Pnt>> P = new NCollection_HArray1<gp_Pnt>(1, n + 1);
      occ::handle<NCollection_HArray1<double>> T = new NCollection_HArray1<double>(1, n + 1);
      for (int j = 0; j <= n; ++j)
      {
        const double t = j == n ? l : f + (l - f) * j / n;
        gp_Pnt       o;
        if (!exact(t, j == 0 ? &vf : j == n ? &vl : nullptr, o))
          return {};
        P->SetValue(j + 1, o);
        T->SetValue(j + 1, t);
      }
      GeomAPI_Interpolate in(P, T, false, 1e-12);
      in.Perform();
      if (!in.IsDone())
        return {};
      occ::handle<Geom_BSplineCurve> b = in.Curve();
      double                         dev = 0;
      for (int j = 0; j < 2 * n && dev <= kPrecision / 10; ++j)
      {
        const double t = f + (l - f) * (j + 0.5) / (2 * n);
        gp_Pnt       o;
        const gp_Pnt x0 = b->Value(t);
        if (!exact(t, &x0, o))
          return {};
        dev = std::max(dev, o.Distance(x0));
      }
      if (dev <= kPrecision / 10)
        return b;
    }
    return {};
  }

  // Largest distance between the edge's curve and its faces (same parameter).
  static double deviation(const TopoDS_Edge& e, const std::vector<TopoDS_Face>& fs)
  {
    double                  f, l;
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
    if (c.IsNull())
      return 1e9;
    double dev = 0;
    for (const TopoDS_Face& F : fs)
    {
      double                    pf, pl;
      occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, F, pf, pl);
      if (pc.IsNull())
        return 1e9;
      TopLoc_Location           loc;
      occ::handle<Geom_Surface> s = BRep_Tool::Surface(F, loc);
      for (int i = 0; i <= 20; ++i)
      {
        const double   t  = f + (l - f) * i / 20.0;
        const gp_Pnt2d uv = pc->Value(pf + (pl - pf) * i / 20.0);
        dev               = std::max(dev, c->Value(t).Distance(s->Value(uv.X(), uv.Y()).Transformed(loc.Transformation())));
      }
    }
    return dev;
  }

  // ---- topology

  void write(const TopoDS_Shape& shape)
  {
    // Shells: (OCCT shell, solid region or 0, void region, inner?) per shell.
    struct ShellIn
    {
      TopoDS_Shape shell;
      int          solid_region = 0, void_region = 0, back = 0, front = 0;
    };
    std::vector<ShellIn> shells;
    std::vector<int>     regions; // in chain order, the infinite void first
    std::vector<char>    region_type;
    const int            infinite = w.alloc();
    regions.push_back(infinite);
    region_type.push_back('V');
    if (solid_body)
    {
      std::vector<int> cavities;
      for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next())
      {
        const TopoDS_Solid& so    = TopoDS::Solid(ex.Current());
        const TopoDS_Shell  outer = BRepClass3d::OuterShell(so);
        const int           r     = w.alloc();
        regions.push_back(r);
        region_type.push_back('S');
        for (TopExp_Explorer es(so, TopAbs_SHELL); es.More(); es.Next())
        {
          ShellIn si{es.Current(), r, infinite};
          if (!outer.IsNull() && !es.Current().IsSame(outer))
          {
            si.void_region = w.alloc();
            cavities.push_back(si.void_region);
          }
          shells.push_back(si);
        }
      }
      for (int c : cavities)
      {
        regions.push_back(c);
        region_type.push_back('V');
      }
    }
    else
    {
      for (TopExp_Explorer es(shape, TopAbs_SHELL); es.More(); es.Next())
        shells.push_back({es.Current(), 0, infinite});
      for (TopExp_Explorer ef(shape, TopAbs_FACE, TopAbs_SHELL); ef.More(); ef.Next())
        shells.push_back({ef.Current(), 0, infinite});
    }
    if (shells.empty())
      fail_invalid("parasolid: the shape has no faces");
    for (auto& si : shells)
    {
      si.back  = w.alloc();
      si.front = solid_body ? w.alloc() : si.back;
    }

    // Faces as oriented in their shells.
    std::vector<int> face_shell_of; // index into shells
    for (size_t k = 0; k < shells.size(); ++k)
      for (TopExp_Explorer ef(shells[k].shell, TopAbs_FACE); ef.More(); ef.Next())
        if (faces.Add(ef.Current()) > (int)face_shell_of.size())
          face_shell_of.push_back((int)k);
    const int nf = faces.Extent();
    face_idx.resize(nf);
    for (int i = 0; i < nf; ++i)
      face_idx[i] = w.alloc();

    // Fins (seams of periodic analytic surfaces and degenerate edges dropped), in loops.
    face_loops.resize(nf);
    for (int fi = 0; fi < nf; ++fi)
      for (const auto& cycle : exchange_loops(TopoDS::Face(faces(fi + 1))))
      {
        const size_t first = fins.size();
        for (const TopoDS_Edge& e : cycle)
        {
          XFin fin;
          fin.index    = w.alloc();
          fin.positive = e.Orientation() == TopAbs_FORWARD;
          fin.face     = fi;
          fin.occ      = e;
          fins.push_back(fin);
        }
        const int    loop = w.alloc();
        const size_t n    = cycle.size();
        for (size_t k = 0; k < n; ++k)
        {
          XFin& x    = fins[first + k];
          x.loop     = loop;
          x.forward  = fins[first + (k + 1) % n].index;
          x.backward = fins[first + (k + n - 1) % n].index;
        }
        face_loops[fi].push_back(loop);
        face_loops[fi].push_back(fins[first].index); // (loop, first fin) pairs
      }
    // A cone face reaching its apex has the apex as an isolated loop (Parasolid
    // rejects the face otherwise: Onshape G13).
    std::vector<std::pair<int, TopoDS_Vertex>> apexes;
    for (int fi = 0; fi < nf; ++fi)
    {
      const TopoDS_Face& F = TopoDS::Face(faces(fi + 1));
      TopLoc_Location    loc;
      if (occ::down_cast<Geom_ConicalSurface>(basis_surface(BRep_Tool::Surface(F, loc))).IsNull())
        continue;
      for (TopExp_Explorer ee(F, TopAbs_EDGE); ee.More(); ee.Next())
        if (BRep_Tool::Degenerated(TopoDS::Edge(ee.Current())))
          apexes.emplace_back(fi, TopExp::FirstVertex(TopoDS::Edge(ee.Current())));
    }

    // Edges and vertices of the kept fins.
    for (size_t k = 0; k < fins.size(); ++k)
    {
      TopoDS_Edge e = fins[k].occ;
      e.Orientation(TopAbs_FORWARD);
      const int ei = edges.Add(e);
      if (ei > (int)edge_fins.size())
        edge_fins.emplace_back();
      edge_fins[ei - 1].push_back((int)k);
      fins[k].edge = ei;
    }
    const int ne = edges.Extent();
    edge_idx.resize(ne);
    edge_tol.assign(ne, 0.0);
    edge_fit.assign(ne, occ::handle<Geom_Curve>());
    for (int i = 0; i < ne; ++i)
    {
      edge_idx[i] = w.alloc();
      const TopoDS_Edge& e = TopoDS::Edge(edges(i + 1));
      vertices.Add(TopExp::FirstVertex(e));
      vertices.Add(TopExp::LastVertex(e));
      if (edge_fins[i].size() > 2)
        fail_unsupported("parasolid: non-manifold edge");
      if (edge_fins[i].size() == 2 && fins[edge_fins[i][0]].positive == fins[edge_fins[i][1]].positive)
        fail_op("parasolid: an edge is used twice the same way");
      std::vector<TopoDS_Face> fs;
      for (int k : edge_fins[i])
        fs.push_back(TopoDS::Face(faces(fins[k].face + 1)));
      const double dev = deviation(e, fs);
      if (dev > kPrecision && fs.size() == 2 && !(edge_fit[i] = refit(e, fs[0], fs[1])).IsNull())
        for (int end = 0; end < 2; ++end)
        {
          // The vertex moves onto the refitted end (exact points, closer than its tolerance).
          const TopoDS_Vertex v   = end ? TopExp::LastVertex(e) : TopExp::FirstVertex(e);
          const gp_Pnt        p   = edge_fit[i]->Value(end ? edge_fit[i]->LastParameter() : edge_fit[i]->FirstParameter());
          const int           vi  = vertices.FindIndex(v) - 1;
          if (p.Distance(BRep_Tool::Pnt(v)) > BRep_Tool::Tolerance(v))
            edge_fit[i].Nullify();
          else if (!vertex_fit.count(vi))
            vertex_fit[vi] = p.XYZ();
        }
      if (dev > kPrecision && edge_fit[i].IsNull())
        edge_tol[i] = std::max(BRep_Tool::Tolerance(e), 2 * dev);
    }
    for (auto& [fi, v] : apexes)
      vertices.Add(v);
    const int nv = vertices.Extent();
    vertex_idx.resize(nv);
    vertex_tol.assign(nv, 0.0);
    for (int i = 0; i < nv; ++i)
      vertex_idx[i] = w.alloc();
    for (auto& [fi, v] : apexes)
    {
      XFin x;
      x.index    = w.alloc();
      x.loop     = w.alloc();
      x.forward  = x.backward = x.index;
      x.isolated = true;
      x.face     = fi;
      x.vertex   = vertex_idx[vertices.FindIndex(v) - 1];
      x.occ      = TopoDS_Edge();
      face_loops[fi].push_back(x.loop);
      face_loops[fi].push_back(x.index);
      fins.push_back(x);
    }
    // Tolerant vertices: at tolerant edges, or off an accurate edge's curve end.
    for (int i = 0; i < ne; ++i)
    {
      const TopoDS_Edge& e = TopoDS::Edge(edges(i + 1));
      double             f, l;
      occ::handle<Geom_Curve> c = curve_of(e, i, f, l);
      for (int end = 0; end < 2; ++end)
      {
        const TopoDS_Vertex v  = end ? TopExp::LastVertex(e) : TopExp::FirstVertex(e);
        const int           vi = vertices.FindIndex(v) - 1;
        double              gap = c.IsNull() ? 1e9 : c->Value(end ? l : f).Distance(vertex_pnt(vi));
        if (edge_tol[i] > 0 || gap > kPrecision / 2) // accurate vertices: half the resolution
          vertex_tol[vi] = std::max({vertex_tol[vi], BRep_Tool::Tolerance(v), edge_tol[i], 2 * gap});
      }
    }

    // Dummy fins for edges with one face (sheets), then each edge's two fins.
    for (int i = 0; i < ne; ++i)
    {
      if (edge_fins[i].size() == 1)
      {
        XFin d;
        d.index    = w.alloc();
        d.dummy    = true;
        d.positive = !fins[edge_fins[i][0]].positive;
        d.edge     = i + 1;
        d.occ      = fins[edge_fins[i][0]].occ;
        edge_fins[i].push_back((int)fins.size());
        fins.push_back(d);
      }
      XFin& a = fins[edge_fins[i][0]];
      XFin& b = fins[edge_fins[i][1]];
      a.other = b.index;
      b.other = a.index;
      const TopoDS_Edge& e = TopoDS::Edge(edges(i + 1));
      for (int k : edge_fins[i])
      {
        XFin& x = fins[k];
        // The fin's forward vertex: the edge's end for a positive fin.
        const TopoDS_Vertex v = x.positive ? TopExp::LastVertex(e) : TopExp::FirstVertex(e);
        x.vertex              = vertex_idx[vertices.FindIndex(v) - 1];
      }
    }
    std::vector<int> vertex_fin(nv, 0);
    for (auto& x : fins)
    {
      if (x.dummy)
        continue;
      if (x.isolated)
      {
        const int vi   = (int)(std::find(vertex_idx.begin(), vertex_idx.end(), x.vertex) - vertex_idx.begin());
        x.next_at_vx   = vertex_fin[vi];
        vertex_fin[vi] = x.index;
        continue;
      }
      const TopoDS_Edge& e  = TopoDS::Edge(edges(x.edge));
      const int          vi = vertices.FindIndex(x.positive ? TopExp::LastVertex(e) : TopExp::FirstVertex(e)) - 1;
      x.next_at_vx          = vertex_fin[vi];
      vertex_fin[vi]        = x.index;
    }

    // Surfaces and face senses.
    face_sense.resize(nf);
    for (int fi = 0; fi < nf; ++fi)
    {
      const TopoDS_Face& F = TopoDS::Face(faces(fi + 1));
      surfaces.push_back(surface_of(F, face_idx[fi]));
      double u0, u1, v0, v1;
      BRepTools::UVBounds(F, u0, u1, v0, v1);
      BRepAdaptor_Surface ad(F);
      double              best = 0;
      for (double s : {0.5, 0.37, 0.71, 0.13})
      {
        const double      u = u0 + (u1 - u0) * s, v = v0 + (v1 - v0) * s;
        BRepLProp_SLProps pr(ad, u, v, 1, 1e-12);
        if (!pr.IsNormalDefined())
          continue;
        gp_Vec n = pr.Normal();
        if (F.Orientation() == TopAbs_REVERSED)
          n.Reverse();
        const gp_Vec nat = surfaces.back().natural(pr.Value(), u, v);
        if (nat.Magnitude() > 0)
        {
          best = n.Dot(nat.Normalized());
          if (std::abs(best) > 1e-3)
            break;
        }
      }
      if (std::abs(best) < 1e-3)
        fail_op("parasolid: no surface normal on a face");
      face_sense[fi] = best > 0 ? '+' : '-';
    }

    // Curves: accurate edges carry theirs (trimmed at tolerant vertices or
    // when closed), tolerant edges' fins carry trimmed SP-curves.
    edge_curve.assign(ne, 0);
    for (int i = 0; i < ne; ++i)
    {
      const TopoDS_Edge& e = TopoDS::Edge(edges(i + 1));
      if (edge_tol[i] > 0)
      {
        for (int k : edge_fins[i])
          if (!fins[k].dummy)
            fins[k].curve = fin_curve(fins[k].index, e, fins[k].face, fins[k].positive);
        continue;
      }
      double                  f, l;
      occ::handle<Geom_Curve> c = curve_of(e, i, f, l);
      const int               vf = vertices.FindIndex(TopExp::FirstVertex(e)) - 1;
      const int               vl = vertices.FindIndex(TopExp::LastVertex(e)) - 1;
      const bool              trim_ends = vertex_tol[vf] > 0 || vertex_tol[vl] > 0;
      bool                    closed    = false;
      const int               cn        = curve_node(trim_ends ? body : edge_idx[i], c, f, l, closed);
      if (!trim_ends && !closed)
      {
        edge_curve[i] = cn;
        continue;
      }
      if (c->IsPeriodic())
      {
        // parm_1 in the base range [0, period).
        const double period = c->Period(), shift = std::floor(f / period) * period;
        edge_curve[i]       = trimmed(edge_idx[i], cn, c->Value(f), c->Value(l), f - shift, l - shift);
      }
      else
        edge_curve[i] = trimmed(edge_idx[i], cn, c->Value(f), c->Value(l), f, l);
    }

    // ---- nodes
    const int hi_id_placeholder = 0;
    (void)hi_id_placeholder;
    for (size_t r = 0; r < regions.size(); ++r)
    {
      int first_shell = 0, last = 0;
      for (auto& si : shells)
      {
        int sh = 0;
        if (si.solid_region == regions[r])
          sh = si.back;
        else if (si.void_region == regions[r])
          sh = si.front;
        if (!sh || sh == last)
          continue;
        if (!first_shell)
          first_shell = sh;
        last = sh;
      }
      Node& n  = add(19, regions[r]);
      n.fields = {fd(w.next_id++), fp(0), fp(body), fp(r + 1 < regions.size() ? regions[r + 1] : 0), fp(r ? regions[r - 1] : 0), fp(first_shell), fc(region_type[r])};
    }
    // Shells: chained per region in `shells` order.
    std::map<int, std::vector<int>> region_shells;
    for (auto& si : shells)
    {
      if (si.solid_region)
        region_shells[si.solid_region].push_back(si.back);
      if (std::find(region_shells[si.void_region].begin(), region_shells[si.void_region].end(), si.front) == region_shells[si.void_region].end())
        region_shells[si.void_region].push_back(si.front);
    }
    auto next_in = [&](int region, int sh) {
      const auto& v = region_shells[region];
      auto        it = std::find(v.begin(), v.end(), sh);
      return (it != v.end() && it + 1 != v.end()) ? *(it + 1) : 0;
    };
    // Faces of each XT shell (back and front chains).
    std::map<int, std::vector<int>> back_faces, front_faces;
    for (int fi = 0; fi < nf; ++fi)
    {
      const ShellIn& si = shells[face_shell_of[fi]];
      back_faces[si.back].push_back(fi);
      front_faces[si.front].push_back(fi);
    }
    for (auto& si : shells)
    {
      const std::vector<int> ids = solid_body ? std::vector<int>{si.back, si.front} : std::vector<int>{si.back};
      for (int sh : ids)
      {
        const bool  back_side = sh == si.back && solid_body;
        const int   region    = back_side ? si.solid_region : si.void_region;
        const auto& bf        = back_faces[sh];
        const auto& ff_       = front_faces[sh];
        Node&       n         = add(13, sh);
        const int   shell_body = (!solid_body || back_side) ? body : 0;
        n.fields = {fd(w.next_id++), fp(0), fp(shell_body), fp(next_in(region, sh)), fp(bf.empty() ? 0 : face_idx[bf[0]]), fp(0), fp(0), fp(region), fp(ff_.empty() ? 0 : face_idx[ff_[0]])};
      }
    }
    for (int fi = 0; fi < nf; ++fi)
    {
      const ShellIn& si   = shells[face_shell_of[fi]];
      const auto&    bf   = back_faces[si.back];
      const auto&    frf  = front_faces[si.front];
      auto           pos  = std::find(bf.begin(), bf.end(), fi) - bf.begin();
      auto           fpos = std::find(frf.begin(), frf.end(), fi) - frf.begin();
      const int      next = pos + 1 < (long)bf.size() ? face_idx[bf[pos + 1]] : 0;
      const int      prev = pos > 0 ? face_idx[bf[pos - 1]] : 0;
      const int      nfr  = fpos + 1 < (long)frf.size() ? face_idx[frf[fpos + 1]] : 0;
      const int      pfr  = fpos > 0 ? face_idx[frf[fpos - 1]] : 0;
      const auto&    lp   = face_loops[fi];
      Node&          n    = add(14, face_idx[fi]);
      n.fields = {fd(w.next_id++), fp(0), ff_null(), fp(next), fp(prev), fp(lp.empty() ? 0 : lp[0]), fp(si.back), fp(surfaces[fi].index), fc(face_sense[fi]), fp(0), fp(0), fp(nfr), fp(pfr), fp(si.front)};
      for (size_t k = 0; k < lp.size(); k += 2)
      {
        Node& L  = add(15, lp[k]);
        L.fields = {fd(w.next_id++), fp(0), fp(lp[k + 1]), fp(face_idx[fi]), fp(k + 2 < lp.size() ? lp[k + 2] : 0)};
      }
    }
    for (const XFin& x : fins)
    {
      Node& n  = add(17, x.index);
      // An isolated fin's sense is null ('?'), as Parasolid writes it (Onshape G13d: '+' is rejected).
      n.fields = {fp(0), fp(x.loop), fp(x.forward), fp(x.backward), fp(x.vertex), fp(x.other), fp(x.isolated ? 0 : edge_idx[x.edge - 1]), fp(x.curve), fp(x.next_at_vx), fc(x.isolated ? '?' : x.positive ? '+' : '-')};
    }
    for (int i = 0; i < ne; ++i)
    {
      const XFin& a  = fins[edge_fins[i][0]];
      const XFin& b  = fins[edge_fins[i][1]];
      Node&       n  = add(16, edge_idx[i]);
      n.fields       = {fd(w.next_id++), fp(0), edge_tol[i] > 0 ? ff(edge_tol[i]) : ff_null(), fp(a.positive ? a.index : b.index), fp(i ? edge_idx[i - 1] : 0), fp(i + 1 < ne ? edge_idx[i + 1] : 0), fp(edge_curve[i]), fp(0), fp(0), fp(body)};
    }
    for (int i = 0; i < nv; ++i)
    {
      const int pt = w.alloc();
      Node&     p  = add(29, pt);
      p.fields     = {fd(w.next_id++), fp(0), fp(vertex_idx[i]), fp(0), fp(0), fv(vertex_pnt(i).XYZ())};
      chain_point.push_back(pt);
      Node& n  = add(18, vertex_idx[i]);
      n.fields = {fd(w.next_id++), fp(0), fp(vertex_fin[i]), fp(i ? vertex_idx[i - 1] : 0), fp(i + 1 < nv ? vertex_idx[i + 1] : 0), fp(pt), vertex_tol[i] > 0 ? ff(vertex_tol[i]) : ff_null(), fp(body)};
    }
    // Boundary geometry chains (next, previous: fields 3 and 4).
    auto chain = [&](const std::vector<int>& v) {
      for (size_t k = 0; k < v.size(); ++k)
      {
        node(v[k]).fields[3].i = k + 1 < v.size() ? v[k + 1] : 0;
        node(v[k]).fields[4].i = k ? v[k - 1] : 0;
      }
      return v.empty() ? 0 : v[0];
    };
    const int bs = chain(chain_surface), bc = chain(chain_curve), bp = chain(chain_point);
    int first_solid_shell = 0;
    for (auto& si : shells)
      if (!first_solid_shell)
        first_solid_shell = si.back;
    Node& n  = add(12, body);
    n.fields = {fd(w.next_id - 1), fp(0), fp(0), fp(0), fp(0), fp(0), fp(0), ff(1000), ff(1e-8), fp(0), fp(0), fp(0), fu(1), fp(0), fu(solid_body ? 1 : 3), fu(1),
                fp(first_solid_shell), fp(bs), fp(bc), fp(bp), fp(infinite), fp(ne ? edge_idx[0] : 0), fp(nv ? vertex_idx[0] : 0)};
  }
};

// OCCT 8.0.1's BRepTools_Modifier::CreateOtherVertices asks NewCurve2d with a
// null new face ("does this pcurve change?"), which SweptToElementary then
// reads (a crash on parts of the NIST assembly). Answer as it would: a copied
// pcurve, so changed.
class SafeSweptToElementary : public ShapeCustom_SweptToElementary
{
public:
  bool NewCurve2d(const TopoDS_Edge& E, const TopoDS_Face& F, const TopoDS_Edge& NewE, const TopoDS_Face& NewF, occ::handle<Geom2d_Curve>& C, double& Tol) override
  {
    if (!NewF.IsNull())
      return ShapeCustom_SweptToElementary::NewCurve2d(E, F, NewE, NewF, C, Tol);
    double f, l;
    C   = BRep_Tool::CurveOnSurface(E, F, f, l);
    Tol = BRep_Tool::Tolerance(E);
    return true;
  }
};

TopoDS_Shape swept_to_elementary(const TopoDS_Shape& s)
{
  occ::handle<SafeSweptToElementary>                                       se = new SafeSweptToElementary();
  NCollection_DataMap<TopoDS_Shape, TopoDS_Shape, TopTools_ShapeMapHasher> context;
  BRepTools_Modifier                                                       md;
  return ShapeCustom::ApplyModifier(s, se, context, md);
}

// Prepare a shape for XT: elementary surfaces where swept/revolved ones are,
// B-splines for the other swept and offset surfaces (same parameterisation),
// revolved ones too unless `keep_spun` (and all of them when one has a profile
// the writer can't keep), and faces closed on a surface that isn't periodic split.
TopoDS_Shape prepared(const TopoDS_Shape& s, bool keep_spun)
{
  TopoDS_Shape r = swept_to_elementary(s);
  for (TopExp_Explorer ef(r, TopAbs_FACE); ef.More() && keep_spun; ef.Next())
  {
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> fs = basis_surface(BRep_Tool::Surface(TopoDS::Face(ef.Current()), loc));
    keep_spun = occ::down_cast<Geom_SurfaceOfRevolution>(fs).IsNull() || spun(fs);
    // A profile reaching the axis (a pole: a degenerated edge) isn't kept spun.
    for (TopExp_Explorer ee(ef.Current(), TopAbs_EDGE); ee.More() && keep_spun && !occ::down_cast<Geom_SurfaceOfRevolution>(fs).IsNull(); ee.Next())
      keep_spun = !BRep_Tool::Degenerated(TopoDS::Edge(ee.Current()));
  }
  r = ShapeCustom::ConvertToBSpline(r, true, !keep_spun, true, false);
  bool split     = false;
  for (TopExp_Explorer ef(r, TopAbs_FACE); ef.More() && !split; ef.Next())
  {
    const TopoDS_Face& F = TopoDS::Face(ef.Current());
    TopLoc_Location    loc;
    if (periodic_analytic(basis_surface(BRep_Tool::Surface(F, loc))))
      continue;
    for (TopExp_Explorer ee(F, TopAbs_EDGE); ee.More() && !split; ee.Next())
      split = BRep_Tool::IsClosed(TopoDS::Edge(ee.Current()), F);
  }
  if (split)
  {
    ShapeUpgrade_ShapeDivideClosed d(r);
    d.SetNbSplitPoints(1);
    d.Perform();
    r = d.Result();
  }
  // A closed edge needs a periodic curve (circle, ellipse): an XT trimmed
  // curve on an open basis may not start where it ends (Onshape G13: the
  // closed intersection loops of two cylinders). Split the others.
  bool closed = false;
  for (TopExp_Explorer ee(r, TopAbs_EDGE); ee.More() && !closed; ee.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(ee.Current());
    if (BRep_Tool::Degenerated(e) || !TopExp::FirstVertex(e).IsSame(TopExp::LastVertex(e)))
      continue;
    double                  f, l;
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
    while (auto t = occ::down_cast<Geom_TrimmedCurve>(c))
      c = t->BasisCurve();
    closed = c.IsNull() || (occ::down_cast<Geom_Circle>(c).IsNull() && occ::down_cast<Geom_Ellipse>(c).IsNull());
  }
  if (closed)
  {
    ShapeUpgrade_ShapeDivideClosedEdges d(r);
    d.SetNbSplitPoints(1);
    d.Perform();
    r = d.Result();
  }
  return r;
}

} // namespace

// ---------------------------------------------------------------- shared with other B-rep writers

TopoDS_Shape exchange_prepared(const TopoDS_Shape& s, bool keep_spun)
{
  return prepared(s, keep_spun);
}

bool exchange_periodic_analytic(const TopoDS_Face& f)
{
  TopLoc_Location loc;
  return periodic_analytic(basis_surface(BRep_Tool::Surface(f, loc)));
}

std::vector<std::vector<TopoDS_Edge>> exchange_loops(const TopoDS_Face& F)
{
  const bool               periodic = exchange_periodic_analytic(F);
  std::vector<TopoDS_Edge> mine;
  for (TopExp_Explorer ee(F, TopAbs_EDGE); ee.More(); ee.Next())
  {
    const TopoDS_Edge& e = TopoDS::Edge(ee.Current());
    if (BRep_Tool::Degenerated(e))
      continue;
    if (BRep_Tool::IsClosed(e, F))
    {
      if (periodic)
        continue;
      fail_unsupported("export: a seam on a face that isn't periodic");
    }
    mine.push_back(e);
  }
  // Chain by vertices: an edge runs from its first to its last vertex (as used in the face).
  std::vector<std::vector<TopoDS_Edge>> out;
  std::vector<bool>                     used(mine.size(), false);
  for (size_t a = 0; a < mine.size(); ++a)
  {
    if (used[a])
      continue;
    std::vector<TopoDS_Edge> cycle{mine[a]};
    used[a]                   = true;
    const TopoDS_Vertex start = TopExp::FirstVertex(mine[a], true);
    TopoDS_Vertex       end   = TopExp::LastVertex(mine[a], true);
    size_t              at    = a;
    while (!end.IsSame(start))
    {
      size_t pick = mine.size();
      for (size_t k = 1; k <= mine.size() && pick == mine.size(); ++k)
      {
        const size_t b = (at + k) % mine.size(); // prefer the wire's own order
        if (!used[b] && TopExp::FirstVertex(mine[b], true).IsSame(end))
          pick = b;
      }
      if (pick == mine.size())
        fail_op("export: a face boundary doesn't close");
      used[pick] = true;
      cycle.push_back(mine[pick]);
      end = TopExp::LastVertex(mine[pick], true);
      at  = pick;
    }
    out.push_back(cycle);
  }
  return out;
}

std::string parasolid_binary_body(const TopoDS_Shape& prepared, std::vector<TopoDS_Face>& faces)
{
  Writer    w;
  const int body = w.alloc(); // the root node: index 1
  bool      solid = TopExp_Explorer(prepared, TopAbs_SOLID).More();
  BodyWriter bw(w, body, solid);
  bw.write(prepared);
  faces.clear();
  for (int i = 1; i <= bw.faces.Extent(); ++i)
    faces.push_back(TopoDS::Face(bw.faces(i)));
  // Embedded in other formats: the 12.0 layouts, as before.
  return binary_body(w.nodes, xt_version("12.0"));
}

// A base-schema field's position in a node (the fields before it fixed-size).
size_t base_field(int type, const std::string& name)
{
  const std::vector<FieldSpec>& f = base_schema().at(type).fields;
  for (size_t i = 0; i < f.size(); ++i)
    if (f[i].name == name)
      return i;
  fail_invalid("no field " + name);
  return 0;
}

// One part's BODY, as write_parasolid writes it.
int write_body(Writer& w, const TopoDS_Shape& s)
{
  require_non_null(s, "shape");
  const TopoDS_Shape p = prepared(s, true);
  bool               solid = false;
  for (TopExp_Explorer ex(p, TopAbs_SOLID); ex.More(); ex.Next())
    solid = true;
  const int body = w.alloc();
  w.next_id      = 1;
  BodyWriter bw(w, body, solid);
  bw.write(p);
  return body;
}

void write_file(const std::vector<Node>& nodes, const XtVersion& ver, bool binary, const std::string& path, const std::string& application)
{
  const std::string data = header(binary, path, application) + (binary ? binary_body(nodes, ver) : text_body(nodes, ver));
  std::ofstream     out(path, std::ios::binary);
  if (!out)
    fail_invalid("cannot write " + path);
  out.write(data.data(), (std::streamsize)data.size());
  if (!out)
    fail_op("writing " + path + " failed");
}

void write_parasolid(const OcShapeList& shapes, rust::Str path, bool binary, rust::Str version, rust::Str application)
{
  const XtVersion ver = xt_version(std::string(version));
  Writer           w;
  std::vector<int> bodies;
  const bool       list = shapes.items.size() != 1;
  const int        root = list ? w.alloc() : 0;
  for (const auto& s : shapes.items)
    bodies.push_back(write_body(w, s));
  if (list)
  {
    Node& n     = w.add(74, root);
    n.variable  = true;
    n.var_count = (int)bodies.size();
    n.fields    = {fd((long long)bodies.size()), fp(0)};
    for (int b : bodies)
      n.fields.push_back(fp(b));
  }
  write_file(to_version(std::move(w.nodes), ver), ver, binary, std::string(path), std::string(application));
}

// An assembly as Onshape exports one (38.1, live samples/dx-nist-assembly.x_t;
// qa ASM-PARASOLID-FLAT): the ASSEMBLY first (node 1), each part's BODY once
// (bodies chained next / previous, ref_instance the first of its instances),
// and per placement an INSTANCE (chained in the assembly and per part) with
// its TRANSFORM (`tf`: 12 per instance, the rotation's rows each followed by
// the translation in metres; flag 1 moved, 2 turned).
void write_parasolid_assembly(const OcShapeList& parts, rust::Slice<const uint32_t> inst_part, rust::Slice<const double> tf, rust::Str path, rust::Str version,
                              rust::Str application)
{
  const XtVersion ver = xt_version(std::string(version));
  if (schema_code(ver) < 38100)
    fail_invalid("Parasolid assemblies are written in 38.1");
  if (tf.size() != inst_part.size() * 12)
    fail_invalid("one transform per instance");
  Writer           w;
  const int        root = w.alloc();
  std::vector<int> bodies;
  for (const auto& s : parts.items)
    bodies.push_back(write_body(w, s));
  const size_t                     ni = inst_part.size();
  std::vector<int>                 inst(ni), trs(ni);
  std::vector<std::vector<size_t>> of_part(bodies.size());
  for (size_t k = 0; k < ni; ++k)
  {
    if (inst_part[k] >= bodies.size())
      fail_invalid("an instance of no part");
    inst[k] = w.alloc();
    trs[k]  = w.alloc();
    of_part[inst_part[k]].push_back(k);
  }
  long long id = 1;
  for (size_t k = 0; k < ni; ++k)
  {
    const std::vector<size_t>& same = of_part[inst_part[k]];
    const size_t               j    = (size_t)(std::find(same.begin(), same.end(), k) - same.begin());
    const double*              m    = &tf[k * 12];
    Node&                      I    = w.add(11, inst[k]);
    I.fields = {fd(id++), fp(0), fu(1), fp(bodies[inst_part[k]]), fp(trs[k]), fp(root), fp(k + 1 < ni ? inst[k + 1] : 0), fp(k > 0 ? inst[k - 1] : 0),
                fp(j + 1 < same.size() ? inst[same[j + 1]] : 0), fp(j > 0 ? inst[same[j - 1]] : 0)};
    bool turned = false;
    for (int r = 0; r < 3; ++r)
      for (int c = 0; c < 3; ++c)
        turned |= std::abs(m[r * 4 + c] - (r == c ? 1.0 : 0.0)) > 1e-15;
    const bool moved = m[3] != 0 || m[7] != 0 || m[11] != 0;
    Node&      T     = w.add(100, trs[k]);
    T.fields         = {fd(id++), fp(inst[k]), fp(0), fp(0)};
    for (int r = 0; r < 3; ++r)
      for (int c = 0; c < 3; ++c)
        T.fields.push_back(ff(m[r * 4 + c]));
    T.fields.push_back(fv(gp_XYZ(m[3], m[7], m[11])));
    T.fields.push_back(ff(1));
    T.fields.push_back(fd((moved ? 1 : 0) | (turned ? 2 : 0)));
    T.fields.push_back(fv_null());
  }
  Node& A  = w.add(10, root);
  A.fields = {fd(id), fp(0), fp(0), fp(0), fp(0), fp(0), fp(0), fp(0), ff(1e3), ff(1e-8), fp(0), fp(0), fp(0), fu(1), fp(0), fu(1), fp(ni ? inst[0] : 0)};
  const size_t ref = base_field(12, "ref_instance"), next = base_field(12, "next"), prev = base_field(12, "previous");
  for (size_t b = 0; b < bodies.size(); ++b)
  {
    Node& B = w.node(bodies[b]);
    if (!of_part[b].empty())
      B.fields.at(ref) = fp(inst[of_part[b].front()]);
    B.fields.at(next) = fp(b + 1 < bodies.size() ? bodies[b + 1] : 0);
    B.fields.at(prev) = fp(b > 0 ? bodies[b - 1] : 0);
  }
  std::vector<Node> nodes = std::move(w.nodes);
  std::stable_sort(nodes.begin(), nodes.end(), [](const Node& a, const Node& b) { return a.index < b.index; });
  write_file(to_version(std::move(nodes), ver), ver, false, std::string(path), std::string(application));
}

// ---------------------------------------------------------------- reader
//
// Reading Parasolid files (any version): xt_read.cpp.

std::unique_ptr<OcShape> read_parasolid(rust::Str path)
{
  BRep_Builder    b;
  TopoDS_Compound out;
  b.MakeCompound(out);
  for (const XtPart& p : read_xt_parts(std::string(path)))
    b.Add(out, p.shape);
  return wrap(out);
}

namespace {
StepImport import_of(const std::vector<XtPart>& parts)
{
  StepImport out;
  out.shapes = std::make_unique<OcShapeList>();
  for (const XtPart& p : parts)
  {
    out.shapes->items.push_back(p.shape);
    out.names.push_back(rust::String(p.name));
    for (double c : p.colour)
      out.colors.push_back((float)c);
    out.has_color.push_back(p.has_colour);
    out.opacity.push_back((float)p.opacity);
    for (const auto& [face, c] : p.face_colours)
    {
      out.face_part.push_back((uint32_t)(out.names.size() - 1));
      out.face_index.push_back((uint32_t)face);
      for (double v : c)
        out.face_colors.push_back((float)v);
    }
  }
  return out;
}
} // namespace

StepImport read_parasolid_parts(rust::Str path)
{
  return import_of(read_xt_parts(std::string(path)));
}

StepTree read_parasolid_tree(rust::Str path)
{
  XtTree   t = read_xt_tree(std::string(path));
  StepTree out;
  out.root  = rust::String(t.root);
  out.parts = import_of(t.parts);
  for (size_t i = 0; i < t.instances.size(); ++i)
  {
    PvzInstance inst;
    inst.part = (uint32_t)t.instances[i].first;
    inst.name = rust::String(t.names[i]);
    inst.root = rust::String(t.root);
    const gp_Trsf& m = t.instances[i].second;
    for (int r = 1; r <= 3; ++r)
      for (int k = 1; k <= 4; ++k)
        inst.transform.push_back(m.Value(r, k));
    out.instances.push_back(std::move(inst));
  }
  return out;
}

StepImport read_rhino_parts_ffi(rust::Str path)
{
  return import_of(read_rhino_parts(std::string(path)));
}

StepImport read_native_parts_ffi(rust::Str path)
{
  return import_of(read_native_parts(std::string(path)));
}

StepImport read_native_parts_config_ffi(rust::Str path, int32_t config)
{
  return import_of(read_native_parts(std::string(path), config));
}

rust::Vec<AsmNodeF> read_native_tree_ffi(rust::Str path, rust::Str dir)
{
  rust::Vec<AsmNodeF> out;
  for (const AsmNode& n : read_native_tree(std::string(path), std::string(dir)))
  {
    AsmNodeF f;
    f.parent = n.parent;
    f.name   = rust::String(n.name);
    f.file   = rust::String(n.file);
    f.configuration = rust::String(n.configuration);
    f.config_id     = n.config_id;
    f.number        = n.number;
    for (double v : n.m)
      f.transform.push_back(v);
    f.suppressed  = n.suppressed;
    f.hidden      = n.hidden;
    f.exclude_bom = n.exclude_bom;
    f.assembly    = n.assembly;
    out.push_back(std::move(f));
  }
  return out;
}

rust::Vec<rust::String> extract_zip_ffi(rust::Str zip, rust::Str dir)
{
  rust::Vec<rust::String> out;
  for (const std::string& n : extract_zip(std::string(zip), std::string(dir)))
    out.push_back(rust::String(n));
  return out;
}

StepImport read_sat_parts_ffi(rust::Str path)
{
  return import_of(read_sat_parts(std::string(path)));
}

StepImport read_jt_parts_ffi(rust::Str path)
{
  return import_of(read_jt_parts(std::string(path)));
}

} // namespace noshape
