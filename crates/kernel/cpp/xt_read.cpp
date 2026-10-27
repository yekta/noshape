// Parasolid transmit file reader (.x_t text / .x_b neutral binary), for
// importing any Parasolid part or assembly as Onshape does (its preferred
// import format). Format: "Parasolid XT Format Reference" (kept outside the
// repo, /root/kern-docs/xt). Files are read node by node with each node
// type's field layout: from V14 on, a file carries per node type the edit
// (Copy / Delete / Insert / Append) from the base schema SCH_13006 to its
// own; older files and our own (schema 12006) use the base layouts directly.
// Nodes keep their field names, so the geometry is read by name whatever the
// file's version.

#include "internal.hpp"

#include <BRepBuilderAPI_Transform.hxx>
#include <BRepLib.hxx>
#include <TopExp.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <Extrema_GenLocateExtPS.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <Geom2dAdaptor_Curve.hxx>
#include <Adaptor3d_CurveOnSurface.hxx>
#include <Extrema_LocateExtPC.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Line.hxx>
#include <GeomLib.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Circle.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom_Line.hxx>
#include <Geom_OffsetSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_SurfaceOfLinearExtrusion.hxx>
#include <Geom_SurfaceOfRevolution.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <BRepTools_ReShape.hxx>
#include <OSD_Parallel.hxx>
#include <ShapeFix_Edge.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeFix_Shape.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <math_Gauss.hxx>
#include <math_Matrix.hxx>
#include <math_Vector.hxx>

#include <cstring>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <set>

namespace noshape {

namespace {

// ---------------------------------------------------------------- schema

struct FieldSpec
{
  std::string name;
  char        type = 'd'; // d int, p pointer, f double, v vector, h hvec, i interval, c char, l logical, u byte, n short, t tag, w unicode char
  int         n    = 0;   // 0 scalar, 1 variable length, >1 fixed array
};

struct BaseType
{
  const char*            name;
  std::vector<FieldSpec> fields;
};

#include "xt_schema.inc"

// ---------------------------------------------------------------- nodes

struct Val
{
  long long i    = 0;
  double    x[3] = {0, 0, 0};
  bool      null = false;
};

struct XNode
{
  int                                type = 0;
  const std::vector<FieldSpec>*      layout = nullptr;
  std::vector<std::vector<Val>>      vals; // per field: 1 value, or the array
  const std::vector<Val>*            get(const char* name) const
  {
    for (size_t k = 0; k < layout->size(); ++k)
      if ((*layout)[k].name == name)
        return &vals[k];
    return nullptr;
  }
  long long ptr(const char* name, size_t k = 0) const
  {
    const auto* v = get(name);
    return v && v->size() > k ? (*v)[k].i : 0;
  }
  long long integer(const char* name) const { return ptr(name); }
  char      chr(const char* name) const { return (char)ptr(name); }
  double    real(const char* name, size_t k = 0, double dflt = 0) const
  {
    const auto* v = get(name);
    return v && v->size() > k && !(*v)[k].null ? (*v)[k].x[0] : dflt;
  }
  bool   is_null(const char* name) const
  {
    const auto* v = get(name);
    return !v || v->empty() || (*v)[0].null;
  }
  gp_XYZ vec(const char* name, size_t k = 0) const
  {
    const auto* v = get(name);
    if (!v || v->size() <= k)
      fail_invalid(std::string("parasolid: missing field ") + name);
    return gp_XYZ((*v)[k].x[0], (*v)[k].x[1], (*v)[k].x[2]);
  }
};

class Parser
{
public:
  explicit Parser(std::string data) : d(std::move(data)) {}

  std::map<long long, XNode> nodes;

  void parse()
  {
    // A transmit file has a text header; a bare stream (a partition embedded
    // in a native CAD file) starts with the binary magic.
    if (d.compare(0, 4, std::string("PS\0\0", 4)) == 0)
      at = 0;
    else
    {
      const size_t h = d.find("**END_OF_HEADER");
      if (h == std::string::npos)
        fail_invalid("parasolid: no header");
      at = d.find('\n', h);
      if (at == std::string::npos)
        fail_invalid("parasolid: truncated file");
      ++at;
      // NX writes a '+' before the binary magic of the partitions it embeds.
      if (d.compare(at, 5, std::string("+PS\0\0", 5)) == 0)
        ++at;
    }
    binary = d.compare(at, 4, std::string("PS\0\0", 4)) == 0;
    std::string archive;
    if (binary)
    {
      at += 4;
      // Modeller version (16-bit length) and schema name (32-bit length).
      const int n1 = b16();
      at += (size_t)n1;
      const long long n2 = b32();
      archive      = d.substr(at, (size_t)n2);
      at += (size_t)n2;
    }
    else
    {
      // Text: records joined (lines wrap mid-token at a fixed width).
      std::string text;
      for (size_t p = at; p < d.size();)
      {
        size_t e = d.find('\n', p);
        if (e == std::string::npos)
          e = d.size();
        size_t q = e;
        while (q > p && (d[q - 1] == ' ' || d[q - 1] == '\r'))
          --q;
        text.append(d, p, q - p);
        p = e + 1;
      }
      text += ' ';
      d  = std::move(text);
      at = 0;
      if (d[at++] != 'T')
        fail_invalid("parasolid: not a transmit file");
      at += (size_t)tint();
      const long long n2 = tint();
      archive            = d.substr(at, (size_t)n2);
      at += (size_t)n2;
    }
    // "SCH_<modeller>_<schema>" or, with embedded schemas, "SCH_<modeller>_<schema>_<base>".
    size_t underscores = 0;
    for (char c : archive)
      underscores += c == '_';
    embedded = underscores >= 3;
    // Schema number (e.g. 38101 = version 38.1) of a file without an
    // embedded schema: its node layouts are that version's own.
    if (!embedded && underscores == 2)
    {
      const long long sch = std::atoll(archive.substr(archive.rfind('_') + 1).c_str());
      const int       code = (int)(sch / 1000 * 1000 + sch % 1000 / 100 * 100);
      for (const auto& [c, types] : version_schemas())
        if (c <= code)
          native = &types;
    }
    if (embedded)
      binary ? b16() : tint(); // highest node type
    binary ? b32() : tint();   // user field size
    for (;;)
    {
      const int type = (int)(binary ? b16() : tint());
      if (type == 1)
        break;
      if (type <= 0)
        fail_invalid("parasolid: bad node type");
      const std::vector<FieldSpec>& L = layout_of(type);
      bool var = false;
      for (const auto& f : L)
        var = var || f.n == 1;
      const long long count = var ? (binary ? b32() : tint()) : 0;
      const long long index = binary ? bptr() : tint();
      XNode n;
      n.type   = type;
      n.layout = &L;
      for (const auto& f : L)
      {
        const long long k = f.n == 1 ? count : f.n > 1 ? f.n : 1;
        std::vector<Val> vs;
        if (f.type == 'c' && f.n == 1 && !binary)
        {
          // A variable char array is written as a run of characters.
          for (long long j = 0; j < k; ++j)
          {
            Val v;
            v.i = (unsigned char)d.at(at++);
            vs.push_back(v);
          }
        }
        else
          for (long long j = 0; j < k; ++j)
            vs.push_back(field(f.type));
        n.vals.push_back(std::move(vs));
      }
      nodes[index] = std::move(n);
    }
  }

  const XNode& at_index(long long i) const
  {
    auto it = nodes.find(i);
    if (it == nodes.end())
      fail_invalid("parasolid: dangling pointer " + std::to_string(i));
    return it->second;
  }
  bool has(long long i) const { return i && nodes.count(i); }

private:
  std::string                                     d;
  size_t                                          at       = 0;
  bool                                            binary   = false;
  bool                                            embedded = false;
  const std::map<int, std::vector<FieldSpec>>*    native   = nullptr; // version layouts (no embedded schema)
  std::map<int, std::unique_ptr<std::vector<FieldSpec>>> layouts;

  // ---- text tokens
  long long tint()
  {
    const size_t e = d.find(' ', at);
    if (e == std::string::npos)
      fail_invalid("parasolid: truncated file");
    const long long v = std::stoll(d.substr(at, e - at));
    at                = e + 1;
    return v;
  }
  double treal()
  {
    const size_t e = d.find(' ', at);
    if (e == std::string::npos)
      fail_invalid("parasolid: truncated file");
    const double v = std::strtod(d.substr(at, e - at).c_str(), nullptr);
    at             = e + 1;
    return v;
  }
  char tchar() { return d.at(at++); }
  std::string tstr()
  {
    const long long n = tint();
    std::string     s = d.substr(at, (size_t)n);
    at += (size_t)n;
    return s;
  }

  // ---- binary (big-endian)
  int b8() { return (unsigned char)d.at(at++); }
  int b16()
  {
    const int v = (b8() << 8) | b8();
    return (int16_t)v;
  }
  long long b32()
  {
    uint32_t v = 0;
    for (int k = 0; k < 4; ++k)
      v = (v << 8) | (uint32_t)b8();
    return (int32_t)v;
  }
  double b64()
  {
    uint64_t v = 0;
    for (int k = 0; k < 8; ++k)
      v = (v << 8) | (uint64_t)b8();
    double x;
    std::memcpy(&x, &v, 8);
    return x;
  }
  // Pointer indices and "positive integers": one short, or two when the first is negative.
  long long bptr()
  {
    int q = 0, r = b16();
    if (r < 0)
    {
      q = b16();
      r = -r;
    }
    return (long long)q * 32767 + r - 1;
  }
  std::string bstr()
  {
    const int   n = b8();
    std::string s = d.substr(at, (size_t)n);
    at += (size_t)n;
    return s;
  }

  Val field(char ty)
  {
    Val v;
    if (binary)
    {
      switch (ty)
      {
        case 'd':
        case 't':
          v.i = b32();
          break;
        case 'p':
          v.i = bptr();
          break;
        case 'n':
        case 'w':
          v.i = b16();
          break;
        case 'u':
        case 'c':
        case 'l':
          v.i = b8();
          if (ty == 'l')
            v.i = v.i ? 'T' : 'F';
          break;
        case 'f':
          v.x[0] = b64();
          v.null = v.x[0] == -3.14158e13;
          break;
        case 'i':
          v.x[0] = b64();
          v.x[1] = b64();
          v.null = v.x[0] == -3.14158e13;
          break;
        case 'v':
        case 'h':
          for (int k = 0; k < 3; ++k)
            v.x[k] = b64();
          v.null = v.x[0] == -3.14158e13;
          break;
        default:
          fail_unsupported(std::string("parasolid: field type ") + ty);
      }
      return v;
    }
    if (ty == 'c' || ty == 'l')
    {
      v.i = (unsigned char)tchar();
      return v;
    }
    if (d.at(at) == '?')
    {
      at += 1;
      // A null vector is a single '?'.
      v.null = true;
      return v;
    }
    switch (ty)
    {
      case 'f':
        v.x[0] = treal();
        break;
      case 'i':
        v.x[0] = treal();
        v.x[1] = treal();
        break;
      case 'v':
      case 'h':
        for (int k = 0; k < 3; ++k)
          v.x[k] = treal();
        break;
      default:
        v.i = tint();
    }
    return v;
  }

  // A field description in an embedded schema: name, pointer class, element
  // count, type (non-pointers), transmit flag (variable-length only).
  struct Embedded
  {
    FieldSpec spec;
    bool      transmitted = true;
  };
  Embedded embedded_field()
  {
    Embedded e;
    e.spec.name   = binary ? bstr() : tstr();
    const int pc  = (int)(binary ? b16() : tint());
    e.spec.n      = (int)(binary ? bptr() : tint()); // binary: pointer-encoded (0 = 1)
    e.spec.type   = 'p';
    if (pc == 0)
    {
      const std::string t = binary ? bstr() : tstr();
      e.spec.type         = t.empty() ? 'd' : t[0];
    }
    if (e.spec.n == 1)
      e.transmitted = binary ? b8() != 0 : tchar() != 'F';
    if (e.spec.n == 1 && e.spec.type == 'p' && pc == 0)
      e.spec.type = 'p';
    return e;
  }

  const std::vector<FieldSpec>& layout_of(int type)
  {
    auto it = layouts.find(type);
    if (it != layouts.end())
      return *it->second;
    auto        L    = std::make_unique<std::vector<FieldSpec>>();
    const auto& base = base_schema();
    auto        bi   = base.find(type);
    if (!embedded && native && native->count(type))
      *L = native->at(type);
    else if (!embedded)
    {
      if (bi == base.end())
        fail_unsupported("parasolid: node type " + std::to_string(type) + " is not in the base schema");
      *L = bi->second.fields;
    }
    else if (bi == base.end())
    {
      // A node type the base schema doesn't have: its full description.
      const int nf = binary ? b8() : (int)tint();
      binary ? bstr() : tstr(); // name
      binary ? bstr() : tstr(); // description
      for (int k = 0; k < nf; ++k)
      {
        Embedded e = embedded_field();
        if (e.transmitted)
          L->push_back(e.spec);
      }
    }
    else
    {
      const int n = binary ? b8() : (int)tint();
      if (n == 255)
        *L = bi->second.fields;
      else
      {
        const auto& bf = bi->second.fields;
        size_t      b  = 0;
        for (;;)
        {
          const char op = binary ? (char)b8() : tchar();
          if (op == 'Z')
            break;
          if (op == 'C')
          {
            if (b >= bf.size())
              fail_invalid("parasolid: bad embedded schema");
            L->push_back(bf[b++]);
          }
          else if (op == 'D')
            ++b;
          else if (op == 'I' || op == 'A')
          {
            Embedded e = embedded_field();
            if (e.transmitted)
              L->push_back(e.spec);
          }
          else
            fail_invalid(std::string("parasolid: bad schema edit '") + op + "'");
        }
        if ((int)L->size() != n)
        {
          // n counts effective fields, non-transmitted variable ones included;
          // only transmitted ones are read.
        }
      }
    }
    return *(layouts[type] = std::move(L));
  }
};


// ---------------------------------------------------------------- geometry

occ::handle<Geom_BSplineCurve> bspline_through(const std::vector<gp_Pnt>& pts)
{
  occ::handle<NCollection_HArray1<gp_Pnt>> P = new NCollection_HArray1<gp_Pnt>(1, (int)pts.size());
  for (size_t i = 0; i < pts.size(); ++i)
    P->SetValue((int)i + 1, pts[i]);
  GeomAPI_Interpolate in(P, false, 1e-12);
  in.Perform();
  if (!in.IsDone())
    fail_op("parasolid: could not fit an intersection curve");
  return in.Curve();
}

// How far an edge's 3D curve strays from its (first) curve on a surface.
double off_pcurve(const TopoDS_Edge& E)
{
  double                       f, l, pf, pl;
  const occ::handle<Geom_Curve> c = BRep_Tool::Curve(E, f, l);
  occ::handle<Geom2d_Curve>     p;
  occ::handle<Geom_Surface>     s;
  TopLoc_Location               loc;
  BRep_Tool::CurveOnSurface(E, p, s, loc, pf, pl);
  if (c.IsNull() || p.IsNull() || s.IsNull())
    return 1e300;
  double worst = 0;
  for (int k = 0; k <= 400; ++k)
  {
    const gp_Pnt2d uv = p->Value(pf + (pl - pf) * k / 400);
    worst             = std::max(worst, c->Value(f + (l - f) * k / 400).Distance(s->Value(uv.X(), uv.Y()).Transformed(loc.Transformation())));
  }
  return worst;
}

// The point on both surfaces in the plane through x0 normal to t (Newton
// from parameters uv1, uv2), as for the writer's exact intersection edges.
bool on_both(const GeomAdaptor_Surface& a1, const GeomAdaptor_Surface& a2, gp_Pnt2d& p1, gp_Pnt2d& p2, const gp_Pnt& x0, const gp_Vec& t,
             gp_Pnt& out)
{
  double u1 = p1.X(), v1 = p1.Y(), u2 = p2.X(), v2 = p2.Y();
  for (int it = 0; it < 50; ++it)
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

struct Builder
{
  const Parser&                                  r;
  BRep_Builder                                   B;
  std::map<long long, occ::handle<Geom_Surface>> surfs;
  struct Projector
  {
    occ::handle<Geom_Surface>  s;
    GeomAdaptor_Surface        a;
    GeomAPI_ProjectPointOnSurf pj;
    bool                       seeded = false;
    double                     u = 0, v = 0; // the last foot
  };
  std::map<const Geom_Surface*, std::unique_ptr<Projector>> projectors;
  // Attribute nodes by owner (built on first use).
  mutable std::map<long long, std::vector<long long>> attrs_by_owner;
  mutable bool                                        attrs_indexed = false;
  // Second fins' SP-curves to put on their edges' parameter (along_edge),
  // done for all edges at once after the walk (finish_fins).
  struct FinJob
  {
    TopoDS_Edge               E;
    occ::handle<Geom2d_Curve> c;
    double                    a, b;
    occ::handle<Geom_Surface> S;
    double                    tol;
  };
  std::vector<FinJob> fin_jobs;
  // Faces' own colours (SDL/TYSA_COLOUR on the face), by their surface.
  std::map<const Geom_Surface*, std::array<double, 3>> face_colour;
  std::map<long long, occ::handle<Geom_Curve>>   curves;
  // Edges on curves rebuilt here (intersections, SP-curves) rather than
  // read: the last-resort check of a body looks at these only.
  ShapeIMap rebuilt_edges;
  std::map<long long, double>                    cone_cos; // cones: OCCT (u, v) = (u, v / cos a)
  std::map<long long, TopoDS_Vertex>             verts;
  std::map<long long, TopoDS_Edge>               edges;
  double                                         tol = 1e-7;

  explicit Builder(const Parser& p) : r(p) {}

  static gp_Pnt P(const gp_XYZ& x) { return gp_Pnt(x); }
  static gp_Dir D(const gp_XYZ& x) { return gp_Dir(x); }

  std::vector<double> reals(long long idx) const
  {
    std::vector<double> v;
    if (!idx)
      return v;
    for (const auto& x : *r.at_index(idx).get(r.at_index(idx).type == 45 ? "vertices" : "knots"))
      v.push_back(x.x[0]);
    return v;
  }
  std::vector<int> ints(long long idx) const
  {
    std::vector<int> v;
    if (!idx)
      return v;
    for (const auto& x : *r.at_index(idx).get("mult"))
      v.push_back((int)x.i);
    return v;
  }
  template <class T> static NCollection_Array1<T> arr(const std::vector<T>& v)
  {
    NCollection_Array1<T> a(1, (int)v.size());
    for (size_t i = 0; i < v.size(); ++i)
      a((int)i + 1) = v[i];
    return a;
  }

  // NURBS_CURVE of a B_CURVE: degree, dimension, rational, poles (flat), knots, multiplicities.
  void nurbs_curve(long long bcurve, int& deg, int& dim, bool& rat, std::vector<double>& p, std::vector<double>& k, std::vector<int>& m) const
  {
    const XNode& nc = r.at_index(r.at_index(bcurve).ptr("nurbs"));
    deg             = (int)nc.integer("degree");
    dim             = (int)nc.integer("vertex_dim");
    rat             = nc.chr("rational") == 'T';
    p               = reals(nc.ptr("bspline_vertices"));
    k               = reals(nc.ptr("knots"));
    m               = ints(nc.ptr("knot_mult"));
    trim_knots(k, m, nc.integer("n_knots"));
    p.resize(std::min(p.size(), (size_t)(nc.integer("n_vertices") * dim)));
  }

  bool curve_periodic(long long bcurve) const { return r.at_index(r.at_index(bcurve).ptr("nurbs")).chr("periodic") == 'T'; }

  // A periodic B-curve in XT form (unclamped knots over all np poles, the
  // first deg poles repeated at the end) as OCCT's: one period of knots, the
  // end multiplicities equal, np - deg poles. False when not in that form.
  static bool periodic_knots(int deg, int np, const std::vector<double>& k, const std::vector<int>& m, std::vector<double>& pk, std::vector<int>& pm)
  {
    std::vector<double> flat;
    for (size_t i = 0; i < k.size() && i < m.size(); ++i)
      for (int j = 0; j < m[i]; ++j)
        flat.push_back(k[i]);
    if (deg < 1 || np <= deg || (int)flat.size() != np + deg + 1)
      return false;
    const double a = flat[(size_t)deg], b = flat[(size_t)np], eps = 1e-12 * std::max(1.0, b - a);
    pk.clear();
    pm.clear();
    for (size_t i = 0; i < k.size(); ++i)
      if (k[i] >= a - eps && k[i] <= b + eps)
      {
        pk.push_back(k[i]);
        pm.push_back(m[i]);
      }
    if (pk.size() < 2)
      return false;
    pm.back() = pm.front();
    int sum   = 0;
    for (size_t i = 0; i + 1 < pm.size(); ++i)
      sum += pm[i];
    return sum == np - deg;
  }

  // The periodic form of c (made periodic when its XT curve is), if it
  // evaluates the same; c otherwise.
  template <class Curve, class Pnt>
  static occ::handle<Curve> as_periodic(const occ::handle<Curve>& c, int deg, const NCollection_Array1<Pnt>& poles, const NCollection_Array1<double>& w,
                                        bool rat, const std::vector<double>& k, const std::vector<int>& m)
  {
    std::vector<double> pk;
    std::vector<int>    pm;
    const int           np = poles.Length();
    if (!periodic_knots(deg, np, k, m, pk, pm))
      return c;
    try
    {
      NCollection_Array1<Pnt>    pp(1, np - deg);
      NCollection_Array1<double> pw(1, np - deg);
      for (int i = 1; i <= np - deg; ++i)
      {
        pp(i) = poles(i);
        pw(i) = w(i);
      }
      occ::handle<Curve> pc = rat ? new Curve(pp, pw, arr(pk), arr(pm), deg, true) : new Curve(pp, arr(pk), arr(pm), deg, true);
      const double       f = c->FirstParameter(), l = c->LastParameter();
      for (int i = 0; i <= 8; ++i)
      {
        const double t = f + (l - f) * i / 8;
        if (pc->Value(t).Distance(c->Value(t)) > 1e-9)
          return c;
      }
      return pc;
    }
    catch (const Standard_Failure&)
    {
      return c;
    }
  }

  // Knot arrays may be longer than the knot count (NX pads them with null
  // knots of multiplicity 0).
  static void trim_knots(std::vector<double>& k, std::vector<int>& m, long long n)
  {
    if (n > 0 && (size_t)n < k.size())
      k.resize((size_t)n);
    if (n > 0 && (size_t)n < m.size())
      m.resize((size_t)n);
  }

  occ::handle<Geom_Surface> surface(long long idx)
  {
    auto it = surfs.find(idx);
    if (it != surfs.end())
      return it->second;
    const XNode&              n = r.at_index(idx);
    occ::handle<Geom_Surface> s;
    switch (n.type)
    {
      case 50:
        s = new Geom_Plane(gp_Ax3(P(n.vec("pvec")), D(n.vec("normal")), D(n.vec("x_axis"))));
        break;
      case 51:
        s = new Geom_CylindricalSurface(gp_Ax3(P(n.vec("pvec")), D(n.vec("axis")), D(n.vec("x_axis"))), n.real("radius"));
        break;
      case 52:
      {
        const double a = std::atan2(n.real("sin_half_angle"), n.real("cos_half_angle"));
        // The axis points the way the cone widens (Onshape's and NX's cones;
        // the XT reference's formula has the opposite sign):
        // R = P + v A + (X cos u + Y sin u)(r + v tan a), Y = A x X.
        s              = new Geom_ConicalSurface(gp_Ax3(P(n.vec("pvec")), D(n.vec("axis")), D(n.vec("x_axis"))), a, n.real("radius"));
        cone_cos[idx]  = std::cos(a);
        break;
      }
      case 53:
        s = new Geom_SphericalSurface(gp_Ax3(P(n.vec("centre")), D(n.vec("axis")), D(n.vec("x_axis"))), n.real("radius"));
        break;
      case 54:
        s = new Geom_ToroidalSurface(gp_Ax3(P(n.vec("centre")), D(n.vec("axis")), D(n.vec("x_axis"))), n.real("major_radius"), n.real("minor_radius"));
        break;
      case 56:
        s = blend_surface(n);
        break;
      case 60:
        // Parasolid offsets bases OCCT calls not C1 (a blend patch with a
        // double knot): no continuity check.
        s = new Geom_OffsetSurface(surface(n.ptr("surface")), n.real("offset"), true);
        break;
      case 67:
        s = new Geom_SurfaceOfLinearExtrusion(curve(n.ptr("section")), D(n.vec("sweep")));
        break;
      case 68:
        s = new Geom_SurfaceOfRevolution(curve(n.ptr("profile")), gp_Ax1(P(n.vec("base")), D(n.vec("axis"))));
        break;
      case 124:
      {
        const XNode& g   = r.at_index(n.ptr("nurbs"));
        const int    du = (int)g.integer("u_degree"), dv = (int)g.integer("v_degree");
        const int    nu = (int)g.integer("n_u_vertices"), nv = (int)g.integer("n_v_vertices"), dim = (int)g.integer("vertex_dim");
        const bool   rat = g.chr("rational") == 'T';
        const std::vector<double> pv = reals(g.ptr("bspline_vertices"));
        if ((int)pv.size() < nu * nv * dim)
          fail_invalid("parasolid: short B-surface vertex array");
        NCollection_Array2<gp_Pnt> poles(1, nu, 1, nv);
        NCollection_Array2<double> w(1, nu, 1, nv);
        // Vertices with v varying fastest.
        for (int i = 0; i < nu; ++i)
          for (int j = 0; j < nv; ++j)
          {
            const double* q  = &pv[(size_t)((i * nv + j) * dim)];
            const double  wt = rat ? q[dim - 1] : 1.0;
            poles(i + 1, j + 1) = gp_Pnt(q[0] / wt, q[1] / wt, q[2] / wt);
            w(i + 1, j + 1)     = wt;
          }
        auto uk = reals(g.ptr("u_knots")), vk = reals(g.ptr("v_knots"));
        auto um = ints(g.ptr("u_knot_mult")), vm = ints(g.ptr("v_knot_mult"));
        trim_knots(uk, um, g.integer("n_u_knots"));
        trim_knots(vk, vm, g.integer("n_v_knots"));
        s = rat ? new Geom_BSplineSurface(poles, w, arr(uk), arr(vk), arr(um), arr(vm), du, dv)
                : new Geom_BSplineSurface(poles, arr(uk), arr(vk), arr(um), arr(vm), du, dv);
        break;
      }
      default:
        fail_unsupported("parasolid: surface type " + std::to_string(n.type));
    }
    return surfs[idx] = s;
  }

  // A rolling-ball blend (constant radius): the tube of that radius round
  // its spine (the faces' edges trim it); Onshape's vertex blends are these.
  occ::handle<Geom_Surface> blend_surface(const XNode& n)
  {
    const double r0 = n.real("range", 0), r1 = n.real("range", 1, r0);
    if (std::abs(std::abs(r0) - std::abs(r1)) > 1e-9 * std::max(1.0, std::abs(r0)) || std::abs(r0) <= 0)
      fail_unsupported("parasolid: blend surfaces other than constant-radius rolling balls");
    // An intersection spine is marched past its chart: a blend reaching past
    // the spine's edge (a blend on a blend) needs the tube there too.
    const XNode&            spn = r.at_index(n.ptr("spine"));
    occ::handle<Geom_Curve> sp  = spn.type == 38 ? intersection(spn, 3 * std::abs(r0)) : curve(n.ptr("spine"));
    double                  f = sp->FirstParameter(), l = sp->LastParameter();
    if (Precision::IsInfinite(f) || Precision::IsInfinite(l))
      fail_unsupported("parasolid: blend with an unbounded spine");
    return tube(sp, f, l, std::abs(r0));
  }

  // The tube of radius r round c over [f, l] as a NURBS surface: at each
  // station an exact rational circle (v) in a rotation-minimizing frame,
  // the stations' poles interpolated along u (exact at the stations, O(h^4)
  // between them; stations a sixteenth of r or of the spine's radius of
  // curvature apart).
  static occ::handle<Geom_Surface> tube(const occ::handle<Geom_Curve>& c, double f, double l, double r)
  {
    // Station spacing from length and curvature, sampled.
    double len = 0, rho = 1e300;
    gp_Pnt prev = c->Value(f);
    for (int i = 1; i <= 256; ++i)
    {
      const double t = f + (l - f) * i / 256;
      gp_Pnt       p;
      gp_Vec       d1, d2;
      c->D2(t, p, d1, d2);
      len += p.Distance(prev);
      prev = p;
      const double sp = d1.Magnitude();
      if (sp > 0)
      {
        const double k = d1.Crossed(d2).Magnitude() / (sp * sp * sp);
        if (k > 0)
          rho = std::min(rho, 1 / k);
      }
    }
    const double h = std::min(r, rho) / 16;
    const int    n = std::max(16, std::min(4000, (int)std::ceil(len / std::max(h, 1e-12))));
    // Rotation-minimizing frames (double reflection, Wang et al. 2008).
    std::vector<double> ts(n + 1);
    std::vector<gp_Pnt> cs(n + 1);
    std::vector<gp_Vec> xs(n + 1), tg(n + 1);
    for (int i = 0; i <= n; ++i)
    {
      ts[i] = f + (l - f) * i / n;
      gp_Vec d1;
      c->D1(ts[i], cs[i], d1);
      if (d1.Magnitude() <= 0)
        fail_op("parasolid: a blend spine with a singular point");
      tg[i] = d1.Normalized();
    }
    {
      gp_Vec a = std::abs(tg[0].X()) < 0.9 ? gp_Vec(1, 0, 0) : gp_Vec(0, 1, 0);
      xs[0]    = (a - tg[0] * a.Dot(tg[0])).Normalized();
    }
    for (int i = 0; i < n; ++i)
    {
      const gp_Vec v1(cs[i], cs[i + 1]);
      const double c1 = v1.Dot(v1);
      if (c1 <= 0)
      {
        xs[i + 1] = xs[i];
        continue;
      }
      const gp_Vec rl = xs[i] - v1 * (2 / c1 * v1.Dot(xs[i]));
      const gp_Vec tl = tg[i] - v1 * (2 / c1 * v1.Dot(tg[i]));
      const gp_Vec v2 = tg[i + 1] - tl;
      const double c2 = v2.Dot(v2);
      gp_Vec       x  = c2 > 0 ? rl - v2 * (2 / c2 * v2.Dot(rl)) : rl;
      x               = x - tg[i + 1] * x.Dot(tg[i + 1]);
      xs[i + 1]       = x.Normalized();
    }
    // A full circle: 9 poles, weights 1, sqrt(2)/2 alternating, knots 0..4.
    static const double cx[9] = {1, 1, 0, -1, -1, -1, 0, 1, 1}, cy[9] = {0, 1, 1, 1, 0, -1, -1, -1, 0};
    const double        w2    = std::sqrt(0.5);
    std::vector<occ::handle<Geom_BSplineCurve>> rows;
    for (int j = 0; j < 9; ++j)
    {
      occ::handle<NCollection_HArray1<gp_Pnt>> row = new NCollection_HArray1<gp_Pnt>(1, n + 1);
      occ::handle<NCollection_HArray1<double>> par = new NCollection_HArray1<double>(1, n + 1);
      for (int i = 0; i <= n; ++i)
      {
        const gp_Vec y = tg[i].Crossed(xs[i]);
        row->SetValue(i + 1, cs[i].Translated((xs[i] * cx[j] + y * cy[j]) * r));
        par->SetValue(i + 1, ts[i]);
      }
      GeomAPI_Interpolate in(row, par, false, 1e-15);
      in.Perform();
      if (!in.IsDone() || (!rows.empty() && in.Curve()->NbPoles() != rows[0]->NbPoles()))
        fail_op("parasolid: could not build a blend surface");
      rows.push_back(in.Curve());
    }
    // Every row shares the parameters, so the knots.
    const occ::handle<Geom_BSplineCurve>& r0 = rows[0];
    NCollection_Array2<gp_Pnt>            poles(1, r0->NbPoles(), 1, 9);
    NCollection_Array2<double>            wts(1, r0->NbPoles(), 1, 9);
    for (int j = 0; j < 9; ++j)
      for (int i = 1; i <= r0->NbPoles(); ++i)
      {
        poles(i, j + 1) = rows[(size_t)j]->Pole(i);
        wts(i, j + 1)   = j % 2 ? w2 : 1.0;
      }
    NCollection_Array1<double> vk(1, 5);
    NCollection_Array1<int>    vm(1, 5);
    for (int k = 1; k <= 5; ++k)
    {
      vk(k) = (k - 1) * std::acos(-1.0) / 2;
      vm(k) = k == 1 || k == 5 ? 3 : 2;
    }
    return new Geom_BSplineSurface(poles, wts, r0->Knots(), vk, r0->Multiplicities(), vm, 3, 2);
  }

  // The foot of x on a surface (false when there is none). Points come
  // along curves, so Newton from the last foot on the same surface first,
  // kept when it lands on x (within a micron); else the global search, one
  // projector per surface (its sampling grid built once), with a parametric
  // tolerance for a model in metres (OCCT's default, Precision::Confusion,
  // stops short on a blend tube whose u range is a few centimetres).
  bool project(const gp_Pnt& x, const occ::handle<Geom_Surface>& s, gp_Pnt2d& uv, gp_Pnt* foot = nullptr)
  {
    std::unique_ptr<Projector>& p = projectors[s.get()];
    if (!p)
    {
      double u1, u2, v1, v2;
      s->Bounds(u1, u2, v1, v2);
      p    = std::make_unique<Projector>();
      p->s = s;
      p->a.Load(s);
      p->pj.Init(s, u1, u2, v1, v2, Precision::PConfusion());
    }
    if (p->seeded)
    {
      Extrema_GenLocateExtPS loc(p->a);
      loc.Perform(x, p->u, p->v);
      if (loc.IsDone() && loc.SquareDistance() < 1e-12)
      {
        loc.Point().Parameter(p->u, p->v);
        uv = gp_Pnt2d(p->u, p->v);
        if (foot)
          *foot = loc.Point().Value();
        return true;
      }
    }
    p->pj.Perform(x);
    if (p->pj.NbPoints() == 0)
      return false;
    p->pj.LowerDistanceParameters(p->u, p->v);
    p->seeded = true;
    uv        = gp_Pnt2d(p->u, p->v);
    if (foot)
      *foot = p->pj.NearestPoint();
    return true;
  }

  occ::handle<Geom_Curve> curve(long long idx)
  {
    auto it = curves.find(idx);
    if (it != curves.end())
      return it->second;
    const XNode&            n = r.at_index(idx);
    occ::handle<Geom_Curve> c;
    switch (n.type)
    {
      case 30:
        c = new Geom_Line(P(n.vec("pvec")), D(n.vec("direction")));
        break;
      case 31:
        c = new Geom_Circle(gp_Ax2(P(n.vec("centre")), D(n.vec("normal")), D(n.vec("x_axis"))), n.real("radius"));
        break;
      case 32:
        c = new Geom_Ellipse(gp_Ax2(P(n.vec("centre")), D(n.vec("normal")), D(n.vec("x_axis"))), n.real("major_radius"), n.real("minor_radius"));
        break;
      case 38:
        c = intersection(n);
        break;
      case 133:
        c = new Geom_TrimmedCurve(curve(n.ptr("basis_curve")), n.real("parm_1"), n.real("parm_2"));
        break;
      case 134:
      {
        int                 deg, dim;
        bool                rat;
        std::vector<double> p, k;
        std::vector<int>    m;
        nurbs_curve(idx, deg, dim, rat, p, k, m);
        const int                  np = (int)p.size() / dim;
        NCollection_Array1<gp_Pnt> poles(1, np);
        NCollection_Array1<double> w(1, np);
        for (int i = 0; i < np; ++i)
        {
          const double wt = rat ? p[(size_t)(i * dim + dim - 1)] : 1.0;
          poles(i + 1)    = gp_Pnt(p[(size_t)(i * dim)] / wt, p[(size_t)(i * dim + 1)] / wt, dim - (rat ? 1 : 0) > 2 ? p[(size_t)(i * dim + 2)] / wt : 0.0);
          w(i + 1)        = wt;
        }
        occ::handle<Geom_BSplineCurve> bc = rat ? new Geom_BSplineCurve(poles, w, arr(k), arr(m), deg) : new Geom_BSplineCurve(poles, arr(k), arr(m), deg);
        c = curve_periodic(idx) ? as_periodic(bc, deg, poles, w, rat, k, m) : bc;
        break;
      }
      case 137:
      {
        // An SP-curve: its 2D B-curve on the surface, as a 3D curve.
        const long long             sidx = n.ptr("surface");
        occ::handle<Geom_Surface>   s    = surface(sidx);
        occ::handle<Geom2d_Curve>   c2   = curve2d(n.ptr("b_curve"), sidx);
        TopoDS_Edge                 e;
        B.MakeEdge(e);
        B.UpdateEdge(e, c2, s, TopLoc_Location(), tol);
        B.Range(e, c2->FirstParameter(), c2->LastParameter());
        BRepLib::BuildCurve3d(e, tol);
        double f, l;
        c = BRep_Tool::Curve(e, f, l);
        if (c.IsNull())
          fail_op("parasolid: could not build an SP-curve");
        break;
      }
      default:
        fail_unsupported("parasolid: curve type " + std::to_string(n.type));
    }
    return curves[idx] = c;
  }

  // An intersection curve's chart points, with its limit points (exact) at
  // the chart ends they belong to.
  std::vector<gp_Pnt> chart_points(const XNode& n, bool& fixed_first, bool& fixed_last) const
  {
    const XNode& chart = r.at_index(n.ptr("chart"));
    std::vector<gp_Pnt>             pts;
    auto add_limit = [&](long long li) {
      if (!r.has(li))
        return;
      const auto* hv = r.at_index(li).get("hvec");
      if (hv && !hv->empty())
        pts.push_back(gp_Pnt((*hv)[0].x[0], (*hv)[0].x[1], (*hv)[0].x[2]));
    };
    std::vector<gp_Pnt> cp;
    for (const auto& h : *chart.get("Hvec"))
      cp.push_back(gp_Pnt(h.x[0], h.x[1], h.x[2]));
    if (cp.size() < 2)
      fail_invalid("parasolid: intersection curve without a chart");
    // Terminators lie just outside the chart: their exact points extend it.
    std::vector<gp_Pnt> start, end;
    std::swap(pts, start);
    add_limit(n.ptr("start"));
    std::swap(pts, start);
    std::swap(pts, end);
    add_limit(n.ptr("end"));
    std::swap(pts, end);
    // Limits (terminators, arbitrary and spine-boundary limits) end the
    // curve at their first point; help points don't.
    auto ends_at = [&](long long li) {
      const char t = r.at_index(li).chr("type");
      return t == 'T' || t == 'L' || t == 'B';
    };
    // Limit points are exact (and at terminators, where the surfaces are
    // tangent, Newton would jump to another branch): kept as they are.
    // Each limit goes to the chart end it is next to (the chart may run
    // either way along the curve).
    fixed_first = fixed_last = false;
    for (const auto& [pt, li] : {std::make_pair(start, n.ptr("start")), std::make_pair(end, n.ptr("end"))})
    {
      if (pt.empty() || !ends_at(li))
        continue;
      const double df = pt[0].Distance(cp.front()), dl = pt[0].Distance(cp.back());
      if (df <= dl && !fixed_first && df > 1e-12)
      {
        cp.insert(cp.begin(), pt[0]);
        fixed_first = true;
      }
      else if (dl < df && !fixed_last && dl > 1e-12)
      {
        cp.push_back(pt[0]);
        fixed_last = true;
      }
    }
    return cp;
  }

  // An intersection curve: its chart's points refined onto both surfaces,
  // densified until a cubic through them stays on the surfaces.
  // An intersection curve through its chart (refined onto both surfaces),
  // continued `extend` past each end when given.
  occ::handle<Geom_Curve> intersection(const XNode& n, double extend = 0)
  {
    // On a blend boundary surface: the blend's contact curve with a support.
    for (int k = 0; k < 2; ++k)
      if (r.has(n.ptr("surface", (size_t)k)) && r.at_index(n.ptr("surface", (size_t)k)).type == 59)
        return blend_boundary(n, r.at_index(n.ptr("surface", (size_t)k)));
    const occ::handle<Geom_Surface> s1 = surface(n.ptr("surface", 0)), s2 = surface(n.ptr("surface", 1));
    bool                fixed_first = false, fixed_last = false;
    std::vector<gp_Pnt> cp = chart_points(n, fixed_first, fixed_last);
    GeomAdaptor_Surface a1(s1), a2(s2);
    auto refine = [&](const gp_Pnt& x, const gp_Vec& t, gp_Pnt& out) {
      gp_Pnt2d p1, p2;
      if (!project(x, s1, p1) || !project(x, s2, p2))
        return false;
      return t.Magnitude() > 0 && on_both(a1, a2, p1, p2, x, t.Normalized(), out);
    };
    // A chart point lies on the curve to the chart's accuracy: a refined
    // point much farther away came from a wrong projection (onto an offset of
    // a blend patch: a curve 23 km long, qa5 IM4) and the chart point stays.
    double spacing = 0;
    for (size_t i = 0; i + 1 < cp.size(); ++i)
      spacing = std::max(spacing, cp[i].Distance(cp[i + 1]));
    const double near_tol = std::max(100 * r.at_index(n.ptr("chart")).real("chordal_error"), 1e-2 * spacing);
    std::vector<gp_Pnt> exact;
    for (size_t i = 0; i < cp.size(); ++i)
    {
      const gp_Vec t = gp_Vec(cp[i == 0 ? 0 : i - 1], cp[i + 1 < cp.size() ? i + 1 : i]);
      gp_Pnt       q;
      const bool   fixed = (i == 0 && fixed_first) || (i + 1 == cp.size() && fixed_last);
      exact.push_back(!fixed && refine(cp[i], t, q) && q.Distance(cp[i]) < near_tol ? q : cp[i]);
    }
    if (extend > 0 && exact.size() >= 2 && exact.front().Distance(exact.back()) > 1e-7)
    {
      // March along the tangent, each step refined onto both surfaces (stops
      // at a terminator or where the surfaces part).
      double step = extend / 8;
      for (size_t i = 0; i + 1 < exact.size(); ++i)
        step = std::min(step, std::max(exact[i].Distance(exact[i + 1]), extend / 64));
      for (int end = 0; end < 2; ++end)
      {
        std::vector<gp_Pnt> added;
        gp_Pnt              p = end ? exact.back() : exact.front();
        gp_Vec              t(end ? exact[exact.size() - 2] : exact[1], p);
        for (double done = 0; done < extend && t.Magnitude() > 0;)
        {
          gp_Pnt q;
          if (!refine(p.Translated(t.Normalized() * step), t, q) || q.Distance(p) < step / 2 || q.Distance(p) > step * 2)
            break;
          done += q.Distance(p);
          t = gp_Vec(p, q);
          p = q;
          added.push_back(q);
        }
        if (end)
          exact.insert(exact.end(), added.begin(), added.end());
        else
          exact.insert(exact.begin(), added.rbegin(), added.rend());
      }
    }
    occ::handle<Geom_BSplineCurve> c = bspline_through(exact);
    for (int round = 0; round < 4; ++round)
    {
      double              worst = 0;
      std::vector<gp_Pnt> more;
      for (size_t i = 0; i + 1 < exact.size(); ++i)
      {
        more.push_back(exact[i]);
        const gp_Pnt mid((exact[i].XYZ() + exact[i + 1].XYZ()) / 2);
        GeomAPI_ProjectPointOnCurve pc(mid, c);
        const gp_Pnt on = pc.NbPoints() ? pc.NearestPoint() : mid;
        gp_Pnt       q;
        // A point that jumps (to another branch, another turn of a helix)
        // is not this segment's.
        if (refine(on, gp_Vec(exact[i], exact[i + 1]), q) && q.Distance(on) < exact[i].Distance(exact[i + 1]) / 2)
        {
          worst = std::max(worst, q.Distance(on));
          more.push_back(q);
        }
      }
      more.push_back(exact.back());
      if (worst < 1e-10)
        break;
      exact = more;
      c     = bspline_through(exact);
    }
    return c;
  }

  // The curve where a rolling-ball blend touches support 1 - boundary (a
  // BLEND_BOUND has no geometry of its own): its spine's points projected
  // onto that support, through the chart's points.
  occ::handle<Geom_Curve> blend_boundary(const XNode& n, const XNode& bound)
  {
    const XNode&                    blend   = r.at_index(bound.ptr("blend"));
    const int                       side    = 1 - (int)bound.integer("boundary");
    const occ::handle<Geom_Surface> support = surface(blend.ptr("surface", (size_t)std::max(0, std::min(1, side))));
    const occ::handle<Geom_Curve>   spine   = curve(blend.ptr("spine"));
    bool                            fixed_first = false, fixed_last = false;
    const std::vector<gp_Pnt>       cp = chart_points(n, fixed_first, fixed_last);
    GeomAPI_ProjectPointOnCurve pc;
    auto contact = [&](const gp_Pnt& x, gp_Pnt& out) {
      pc.Init(x, spine);
      if (pc.NbPoints() == 0)
        return false;
      gp_Pnt2d uv;
      return project(pc.NearestPoint(), support, uv, &out);
    };
    std::vector<gp_Pnt> exact;
    for (size_t i = 0; i < cp.size(); ++i)
    {
      gp_Pnt     q;
      const bool fixed = (i == 0 && fixed_first) || (i + 1 == cp.size() && fixed_last);
      exact.push_back(!fixed && contact(cp[i], q) ? q : cp[i]);
    }
    occ::handle<Geom_BSplineCurve> c = bspline_through(exact);
    for (int round = 0; round < 4; ++round)
    {
      double              worst = 0;
      std::vector<gp_Pnt> more;
      for (size_t i = 0; i + 1 < exact.size(); ++i)
      {
        more.push_back(exact[i]);
        GeomAPI_ProjectPointOnCurve on(gp_Pnt((exact[i].XYZ() + exact[i + 1].XYZ()) / 2), c);
        gp_Pnt                      q;
        if (on.NbPoints() && contact(on.NearestPoint(), q) && q.Distance(on.NearestPoint()) < exact[i].Distance(exact[i + 1]) / 2)
        {
          worst = std::max(worst, q.Distance(on.NearestPoint()));
          more.push_back(q);
        }
      }
      more.push_back(exact.back());
      if (worst < 1e-10)
        break;
      exact = more;
      c     = bspline_through(exact);
    }
    return c;
  }

  // A 2D B-curve in OCCT's (u, v) of the surface.
  occ::handle<Geom2d_Curve> curve2d(long long bcurve, long long surface_idx)
  {
    int                 deg, dim;
    bool                rat;
    std::vector<double> p, k;
    std::vector<int>    m;
    nurbs_curve(bcurve, deg, dim, rat, p, k, m);
    const int                    np = (int)p.size() / dim;
    NCollection_Array1<gp_Pnt2d> poles(1, np);
    NCollection_Array1<double>   w(1, np);
    surface(surface_idx);
    const double cc   = cone_cos.count(surface_idx) ? cone_cos[surface_idx] : 0;
    const bool   swap = r.at_index(surface_idx).type == 68;
    for (int i = 0; i < np; ++i)
    {
      const double wt = rat ? p[(size_t)(i * dim + dim - 1)] : 1.0;
      double       u = p[(size_t)(i * dim)] / wt, v = p[(size_t)(i * dim + 1)] / wt;
      if (cc)
        v = v / cc;
      if (swap)
        std::swap(u, v);
      poles(i + 1) = gp_Pnt2d(u, v);
      w(i + 1)     = wt;
    }
    occ::handle<Geom2d_BSplineCurve> bc = rat ? new Geom2d_BSplineCurve(poles, w, arr(k), arr(m), deg) : new Geom2d_BSplineCurve(poles, arr(k), arr(m), deg);
    return curve_periodic(bcurve) ? as_periodic(bc, deg, poles, w, rat, k, m) : bc;
  }

  // XT natural normal against OCCT's: spun surfaces (u, v swapped) flip.
  bool natural_flipped(long long idx) const { return r.at_index(idx).type == 68; }

  TopoDS_Vertex vertex(long long idx)
  {
    auto it = verts.find(idx);
    if (it != verts.end())
      return it->second;
    const XNode&  v = r.at_index(idx);
    TopoDS_Vertex V;
    B.MakeVertex(V, P(r.at_index(v.ptr("point")).vec("pvec")), v.is_null("tolerance") ? tol : std::max(tol, v.real("tolerance")));
    return verts[idx] = V;
  }

  static double param_on(const occ::handle<Geom_Curve>& c, const gp_Pnt& p)
  {
    GeomAPI_ProjectPointOnCurve pr(p, c);
    if (pr.NbPoints() > 0)
      return pr.LowerDistanceParameter();
    // Just past an end of a bounded curve: that end.
    const double f = c->FirstParameter(), l = c->LastParameter();
    if (!Precision::IsInfinite(f) && !Precision::IsInfinite(l))
    {
      const double df = c->Value(f).Distance(p), dl = c->Value(l).Distance(p);
      if (std::min(df, dl) < 1e-5)
        return df <= dl ? f : l;
    }
    fail_op("parasolid: a vertex is off its edge's curve");
  }

  TopoDS_Edge edge(long long idx)
  {
    auto it = edges.find(idx);
    if (it != edges.end())
      return it->second;
    const XNode&  e   = r.at_index(idx);
    const XNode&  fin = r.at_index(e.ptr("fin"));
    const long long other = fin.ptr("other");
    // A ring edge (a closed curve with no vertex) has none on its fins.
    const bool    ring = !fin.ptr("vertex");
    TopoDS_Vertex v1, v2;
    if (!ring)
    {
      v2 = vertex(fin.ptr("vertex"));
      v1 = r.has(other) && r.at_index(other).ptr("vertex") ? vertex(r.at_index(other).ptr("vertex")) : v2;
    }
    const double etol = e.is_null("tolerance") ? tol : std::max(tol, e.real("tolerance"));
    TopoDS_Edge  E;
    if (e.ptr("curve"))
    {
      const XNode&            cn = r.at_index(e.ptr("curve"));
      occ::handle<Geom_Curve> c;
      double                  t1, t2;
      const int               ct      = cn.type == 133 ? r.at_index(cn.ptr("basis_curve")).type : cn.type;
      const bool              rebuilt = ct == 38 || ct == 137;
      if (cn.type == 133)
      {
        c  = curve(cn.ptr("basis_curve"));
        t1 = cn.real("parm_1");
        t2 = cn.real("parm_2");
        if (r.at_index(cn.ptr("basis_curve")).chr("sense") == '-')
        {
          // Against a basis curve of negative sense: parm_2 < parm_1.
          c  = c->Reversed();
          t1 = c->ReversedParameter(t1);
          t2 = c->ReversedParameter(t2);
        }
        else if (cn.chr("sense") == '-')
        {
          c  = c->Reversed();
          const double a = c->ReversedParameter(t2), b = c->ReversedParameter(t1);
          t1 = a, t2 = b;
        }
        if (t2 < t1 && c->IsPeriodic())
          t2 += c->Period();
        // A rebuilt basis (an intersection or SP-curve) has its own
        // parameters, not Parasolid's: the vertices give the range (parm_1/2
        // on it ran a shell's edge off its curve, 42 km long, qa5 IM4).
        if (rebuilt && !ring)
        {
          t1 = param_on(c, BRep_Tool::Pnt(v1));
          t2 = param_on(c, BRep_Tool::Pnt(v2));
        }
      }
      else
      {
        c = curve(e.ptr("curve"));
        // A curve with sense '-' runs against its natural direction (and the edge).
        if (cn.chr("sense") == '-')
          c = c->Reversed();
        if (ring)
        {
          t1 = c->FirstParameter();
          t2 = c->LastParameter();
        }
        else
        {
          t1 = param_on(c, BRep_Tool::Pnt(v1));
          t2 = param_on(c, BRep_Tool::Pnt(v2));
          if (c->IsPeriodic())
          {
            const double per = c->Period();
            while (t2 <= t1 + 1e-12)
              t2 += per;
            if (v1.IsSame(v2) && t2 - t1 < 1e-9)
              t2 = t1 + per;
          }
          else if (v1.IsSame(v2) && std::abs(t2 - t1) < 1e-9)
          {
            // A closed edge on a closed, non-periodic curve (an intersection
            // curve's chart that returns to its start): all of it.
            t1 = c->FirstParameter();
            t2 = c->LastParameter();
          }
        }
      }
      B.MakeEdge(E, c, etol);
      B.Range(E, t1, t2);
      if (rebuilt)
        rebuilt_edges.Add(E);
      if (ring)
      {
        // A vertex where the curve starts and ends.
        TopoDS_Vertex V;
        B.MakeVertex(V, c->Value(t1), etol);
        v1 = v2 = V;
      }
    }
    else
    {
      // Tolerant: the fins' SP-curves.
      // The first fin's SP-curve gives the edge its parameter range; another
      // fin's joins it only when trimmed the same way (else its pcurve is
      // projected later).
      B.MakeEdge(E);
      B.UpdateEdge(E, etol);
      struct FinCurve
      {
        occ::handle<Geom2d_Curve> c;
        long long                 s;
        double                    a, b;
      };
      std::vector<FinCurve> fins;
      for (long long fi : {e.ptr("fin"), other})
      {
        if (!r.has(fi))
          continue;
        const XNode& x = r.at_index(fi);
        if (!x.ptr("curve"))
          continue;
        const XNode& tr = r.at_index(x.ptr("curve"));
        const XNode& sp = r.at_index(tr.type == 133 ? tr.ptr("basis_curve") : x.ptr("curve"));
        if (sp.type != 137)
          continue;
        const long long           s  = sp.ptr("surface");
        occ::handle<Geom2d_Curve> c2 = curve2d(sp.ptr("b_curve"), s);
        // The SP-curve runs along its fin from parameter a to b (decreasing
        // against a B-curve of negative sense); the edge runs the fin's way on
        // a '+' fin, the other way on a '-' one.
        double a = c2->FirstParameter(), b = c2->LastParameter();
        if (tr.type == 133)
        {
          a = tr.real("parm_1");
          b = tr.real("parm_2");
        }
        else if (sp.chr("sense") == '-')
          std::swap(a, b);
        if (x.chr("sense") == '-')
          std::swap(a, b);
        if (a > b)
        {
          c2 = c2->Reversed();
          a  = c2->ReversedParameter(a);
          b  = c2->ReversedParameter(b);
        }
        fins.push_back({c2, s, a, b});
      }
      if (fins.empty())
        B.Range(E, 0, 0);
      else
      {
        // The first fin's SP-curve gives the edge its curve and parameter.
        B.UpdateEdge(E, fins[0].c, surface(fins[0].s), TopLoc_Location(), etol);
        B.Range(E, fins[0].a, fins[0].b);
        // The 3D curve from it: OCCT's automatic segment count is too few for
        // a curve winding many times round its surface (a thread's helix: 3 mm
        // off over 16 turns), and it keeps the asked tolerance whatever the
        // fit. Checked, with more segments until it fits.
        for (int segs : {0, 300, 3000})
          if (BRepLib::BuildCurve3d(E, etol, GeomAbs_C1, 14, segs) && off_pcurve(E) <= etol)
            break;
        // The other fin's SP-curve follows the edge only to within its
        // tolerance, not in its parameter: put on the edge's parameter point
        // by point (else projected later), in finish_fins.
        if (fins.size() > 1 && fins[1].s != fins[0].s)
          fin_jobs.push_back({E, fins[1].c, fins[1].a, fins[1].b, surface(fins[1].s), etol});
      }
    }
    v1.Orientation(TopAbs_FORWARD);
    v2.Orientation(TopAbs_REVERSED);
    B.Add(E, v1);
    B.Add(E, v2);
    return edges[idx] = E;
  }

  // The queued second fins (fin_jobs): each one's SP-curve matched to its
  // edge in parallel (along_edge only reads the edge), then set on it.
  void finish_fins()
  {
    std::vector<occ::handle<Geom2d_Curve>> pcs(fin_jobs.size());
    OSD_Parallel::For(0, (int)fin_jobs.size(), [&](int i) {
      const FinJob&             j = fin_jobs[i];
      occ::handle<Geom2d_Curve> pc;
      if (run_guarded([&] { pc = along_edge(j.E, j.c, j.a, j.b, j.S, j.tol); }))
        pcs[i] = pc;
    });
    for (size_t i = 0; i < fin_jobs.size(); ++i)
      if (!pcs[i].IsNull())
      {
        const FinJob& j = fin_jobs[i];
        double        f, l;
        BRep_Tool::Range(j.E, f, l);
        B.UpdateEdge(j.E, pcs[i], j.S, TopLoc_Location(), j.tol);
        B.Range(j.E, j.S, TopLoc_Location(), f, l);
      }
    fin_jobs.clear();
  }

  // A fin's SP-curve c (from a to b, along the edge) in the edge's own
  // parameter: points of the edge's 3D curve matched to it, marching along
  // it, and interpolated. Null when they don't meet within the tolerance.
  static occ::handle<Geom2d_Curve> along_edge(const TopoDS_Edge& E, const occ::handle<Geom2d_Curve>& c, double a, double b,
                                              const occ::handle<Geom_Surface>& S, double tol)
  {
    double                        f, l;
    const occ::handle<Geom_Curve> C = BRep_Tool::Curve(E, f, l);
    if (C.IsNull() || !(l > f) || !(b > a))
      return {};
    int knots = 16;
    {
      occ::handle<Geom2d_Curve> basis = c;
      while (auto t = occ::down_cast<Geom2d_TrimmedCurve>(basis))
        basis = t->BasisCurve();
      if (auto bs = occ::down_cast<Geom2d_BSplineCurve>(basis))
        knots = std::max(knots, bs->NbKnots());
    }
    const int                                     n  = std::min(4000, std::max(200, 16 * knots));
    const occ::handle<Geom2dAdaptor_Curve>        ac = new Geom2dAdaptor_Curve(c, a, b);
    const occ::handle<GeomAdaptor_Surface>        as = new GeomAdaptor_Surface(S);
    const Adaptor3d_CurveOnSurface                on(ac, as);
    occ::handle<NCollection_HArray1<gp_Pnt2d>>    pts = new NCollection_HArray1<gp_Pnt2d>(1, n + 1);
    occ::handle<NCollection_HArray1<double>>      par = new NCollection_HArray1<double>(1, n + 1);
    // Seeds: the points at the same fraction of arc length along both (they
    // trace one path to within the tolerance), refined locally.
    const int           m = 8 * n;
    std::vector<double> lc(m + 1, 0.0), ls(m + 1, 0.0);
    for (int j = 1; j <= m; ++j)
    {
      lc[j] = lc[j - 1] + C->Value(f + (l - f) * j / m).Distance(C->Value(f + (l - f) * (j - 1) / m));
      ls[j] = ls[j - 1] + on.Value(a + (b - a) * j / m).Distance(on.Value(a + (b - a) * (j - 1) / m));
    }
    if (!(lc[m] > 0) || !(ls[m] > 0))
      return {};
    double u = a;
    for (int k = 0; k <= n; ++k)
    {
      const double t = f + (l - f) * k / n;
      const gp_Pnt P = C->Value(t);
      if (k == n)
        u = b;
      else if (k > 0)
      {
        const double want = lc[(size_t)k * 8] / lc[m] * ls[m];
        const size_t j    = (size_t)(std::lower_bound(ls.begin(), ls.end(), want) - ls.begin());
        const double j0   = j == 0 ? 0 : (double)j - 1 + (want - ls[j - 1]) / std::max(ls[j] - ls[j - 1], 1e-300);
        Extrema_LocateExtPC loc(P, on, a + (b - a) * std::min(1.0, j0 / m), 1e-12);
        if (!loc.IsDone() || loc.Point().Parameter() < u - 1e-12)
          return {};
        u = loc.Point().Parameter();
      }
      if (on.Value(u).Distance(P) > 10 * tol)
        return {};
      pts->SetValue(k + 1, c->Value(u));
      par->SetValue(k + 1, t);
    }
    try
    {
      Geom2dAPI_Interpolate in(pts, par, false, 1e-12);
      in.Perform();
      if (in.IsDone())
        return occ::handle<Geom2d_Curve>(in.Curve());
    }
    catch (const Standard_Failure&)
    {
    }
    return {};
  }

  // Outward normal of a rolling-ball blend face at p: away from the ball's
  // centre on a convex blend (negative range), towards it on a concave one.
  bool blend_flipped(long long si, const occ::handle<Geom_Surface>& s, const TopoDS_Face& F)
  {
    const XNode& n = r.at_index(si);
    for (TopExp_Explorer ee(F, TopAbs_EDGE); ee.More(); ee.Next())
    {
      double                        f, l;
      const occ::handle<Geom_Curve> c = BRep_Tool::Curve(TopoDS::Edge(ee.Current()), f, l);
      if (c.IsNull())
        continue;
      gp_Pnt2d uv;
      if (!project(c->Value((f + l) / 2), s, uv))
        continue;
      const double u = uv.X(), v = uv.Y();
      gp_Pnt       q;
      gp_Vec du, dv;
      s->D1(u, v, q, du, dv);
      const gp_Vec nat = du.Crossed(dv);
      GeomAPI_ProjectPointOnCurve pc(q, curve(n.ptr("spine")));
      if (pc.NbPoints() == 0 || nat.Magnitude() == 0)
        continue;
      gp_Vec out(pc.NearestPoint(), q);
      if (n.real("range", 0) > 0)
        out.Reverse();
      return nat.Dot(out) < 0;
    }
    return false;
  }

  TopoDS_Face face(long long idx)
  {
    const XNode&              n  = r.at_index(idx);
    const long long           si = n.ptr("surface");
    occ::handle<Geom_Surface> s  = surface(si);
    {
      std::string           unused;
      std::array<double, 3> c{};
      bool                  has = false;
      attributes(idx, unused, c, has);
      if (has)
        face_colour[s.get()] = c;
    }
    TopoDS_Face               F;
    B.MakeFace(F, s, tol);
    for (long long l = n.ptr("loop"); l; l = r.at_index(l).ptr("next"))
    {
      const XNode& L = r.at_index(l);
      if (!r.has(L.ptr("fin")) || r.at_index(L.ptr("fin")).ptr("edge") == 0)
        continue; // an isolated loop (a cone's apex): the degenerate edge comes from the fix below
      TopoDS_Wire W;
      B.MakeWire(W);
      const long long first = L.ptr("fin");
      long long       fi    = first;
      int             guard = 0;
      do
      {
        const XNode& fin = r.at_index(fi);
        TopoDS_Edge  E   = edge(fin.ptr("edge"));
        E.Orientation(fin.chr("sense") == '+' ? TopAbs_FORWARD : TopAbs_REVERSED);
        B.Add(W, E);
        fi = fin.ptr("forward");
        if (++guard > 1000000)
          fail_invalid("parasolid: a loop doesn't close");
      } while (fi && fi != first);
      B.Add(F, W);
    }
    // XT face normal = natural normal, reversed when exactly one sense is '-'.
    bool flip;
    if (r.at_index(si).type == 56)
      flip = blend_flipped(si, s, F) != (n.chr("sense") == '-');
    else
      flip = ((n.chr("sense") == '-') != (r.at_index(si).chr("sense") == '-')) != natural_flipped(si);
    if (flip)
    {
      // Edges were added along the fins as seen from the XT normal: reverse
      // them on a reversed face.
      TopoDS_Face G;
      B.MakeFace(G, s, tol);
      for (TopoDS_Iterator w(F); w.More(); w.Next())
        B.Add(G, w.Value().Reversed());
      F = G;
      F.Reverse();
    }
    return F;
  }

  // ---- bodies

  // Faces of a shell: its face chain (back faces) and its front faces.
  std::vector<long long> shell_faces(const XNode& sh) const
  {
    std::vector<long long> out;
    for (long long f = sh.ptr("face"); f; f = r.at_index(f).ptr("next"))
      out.push_back(f);
    for (long long f = sh.ptr("front_face"); f; f = r.at_index(f).ptr("next_front"))
      if (std::find(out.begin(), out.end(), f) == out.end())
        out.push_back(f);
    return out;
  }

  // A body as the file has it, before healing (heal_import; bodies share
  // nothing, so read_xt_data heals several at once).
  TopoDS_Shape body_raw(long long bi)
  {
    const XNode&    b    = r.at_index(bi);
    const int       type = (int)b.integer("body_type"); // 1 solid, 2 wire, 3 sheet, 6 general
    TopoDS_Compound part;
    B.MakeCompound(part);
    int nsolids = 0;
    TopoDS_Solid last;
    for (long long reg = b.ptr("region"); reg; reg = r.at_index(reg).ptr("next"))
    {
      const XNode& R      = r.at_index(reg);
      const bool   solid_region = R.chr("type") == 'S';
      if (type == 1 && !solid_region)
        continue;
      TopoDS_Solid so;
      B.MakeSolid(so);
      int nshells = 0;
      for (long long sh = R.ptr("shell"); sh; sh = r.at_index(sh).ptr("next"))
      {
        TopoDS_Shell shell;
        B.MakeShell(shell);
        int nf = 0;
        for (long long fi : shell_faces(r.at_index(sh)))
        {
          B.Add(shell, face(fi));
          ++nf;
        }
        if (nf == 0)
        {
          // A wire / acorn shell: its edges.
          for (long long e = r.at_index(sh).ptr("edge"); e; e = r.at_index(e).ptr("next"))
            B.Add(part, edge(e));
          continue;
        }
        if (type == 1 && solid_region)
        {
          shell.Closed(true);
          B.Add(so, shell);
          ++nshells;
        }
        else
          B.Add(part, shell);
      }
      if (nshells)
      {
        B.Add(part, so);
        last = so;
        ++nsolids;
      }
    }
    TopoDS_Shape out = part;
    if (nsolids == 1 && part.NbChildren() == 1)
      out = last;
    check_rebuilt(out);
    return out;
  }

  // Last resort: a body with a curve rebuilt here (an intersection or
  // SP-curve) running far outside the box of everything else in it fails
  // instead of importing as garbage. Bodies with nothing else to compare
  // against are left alone.
  void check_rebuilt(const TopoDS_Shape& body) const
  {
    Bnd_Box                   ref;
    std::vector<TopoDS_Shape> rebuilt;
    for (TopExp_Explorer v(body, TopAbs_VERTEX); v.More(); v.Next())
      ref.Add(BRep_Tool::Pnt(TopoDS::Vertex(v.Current())));
    for (TopExp_Explorer e(body, TopAbs_EDGE); e.More(); e.Next())
    {
      const TopoDS_Edge& E = TopoDS::Edge(e.Current());
      if (BRep_Tool::Degenerated(E))
        continue;
      if (rebuilt_edges.Contains(E))
        rebuilt.push_back(E);
      else
        BRepBndLib::Add(E, ref, false);
    }
    if (ref.IsVoid() || rebuilt.empty() || ref.SquareExtent() < 1e-18)
      return;
    ref.Enlarge(10 * std::sqrt(ref.SquareExtent()));
    double a0, b0, c0, a1, b1, c1;
    ref.Get(a0, b0, c0, a1, b1, c1);
    for (const TopoDS_Shape& E : rebuilt)
    {
      Bnd_Box eb;
      try
      {
        BRepBndLib::Add(E, eb, false);
      }
      catch (const Standard_Failure&)
      {
        fail_invalid("parasolid: a curve of the file could not be rebuilt");
      }
      if (eb.IsVoid())
        continue;
      double x0, y0, z0, x1, y1, z1;
      eb.Get(x0, y0, z0, x1, y1, z1);
      if (x0 < a0 || y0 < b0 || z0 < c0 || x1 > a1 || y1 > b1 || z1 > c1)
        fail_invalid("parasolid: a curve of the file could not be rebuilt");
    }
  }

  // ---- names and colours (SDL/TYSA_NAME, SDL/TYSA_COLOUR attributes)

  std::string attribute_def_name(long long def) const
  {
    if (!r.has(def))
      return {};
    const long long id = r.at_index(def).ptr("identifier");
    if (!r.has(id))
      return {};
    std::string s;
    for (const auto& c : *r.at_index(id).get("String"))
      s += (char)c.i;
    return s;
  }

  // Attributes owned by node `owner`, by definition name.
  // The part's faces that carry their own colour, matched through their
  // surfaces (healing keeps them; a trimmed copy wraps the same one).
  void face_colours_of(XtPart& part) const
  {
    if (face_colour.empty())
      return;
    ShapeIMap faces;
    TopExp::MapShapes(part.shape, TopAbs_FACE, faces);
    for (int i = 1; i <= faces.Extent(); ++i)
    {
      TopLoc_Location           loc;
      occ::handle<Geom_Surface> s = BRep_Tool::Surface(TopoDS::Face(faces(i)), loc);
      while (auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
        s = t->BasisSurface();
      const auto it = face_colour.find(s.get());
      if (it != face_colour.end())
        part.face_colours.emplace_back(i - 1, it->second);
    }
  }

  void attributes(long long owner, std::string& name, std::array<double, 3>& colour, bool& has_colour) const
  {
    // Indexed once (a file without attributes left it empty: every face
    // rescanned the whole file).
    if (!attrs_indexed)
    {
      attrs_indexed = true;
      for (const auto& [i, n] : r.nodes)
        if (n.type == 81)
          attrs_by_owner[n.ptr("owner")].push_back(i);
    }
    const auto it = attrs_by_owner.find(owner);
    if (it == attrs_by_owner.end())
      return;
    for (long long i : it->second)
    {
      const XNode& n = r.at_index(i);
      const std::string def = attribute_def_name(n.ptr("definition"));
      const auto*       fs  = n.get("fields");
      if (!fs || fs->empty() || !r.has((*fs)[0].i))
        continue;
      const XNode& v = r.at_index((*fs)[0].i);
      if (def == "SDL/TYSA_NAME" || def == "SDL/TYSA_UNAME")
      {
        std::string s;
        for (const auto& c : *v.get("values"))
          if (v.type == 84 || (c.i > 0 && c.i < 128))
            s += (char)c.i;
        if (!s.empty() && (name.empty() || def == "SDL/TYSA_NAME"))
          name = s;
      }
      else if ((def == "SDL/TYSA_COLOUR" || def == "SDL/TYSA_COLOUR_2") && v.type == 83 && v.get("values")->size() >= 3)
      {
        for (int k = 0; k < 3; ++k)
          colour[k] = (*v.get("values"))[(size_t)k].x[0];
        has_colour = true;
      }
    }
  }
};

gp_Trsf transform_of(const XNode& t)
{
  gp_Trsf      tr;
  const auto*  m = t.get("rotation_matrix");
  const gp_XYZ v = t.vec("translation_vector");
  if (m && m->size() == 9)
    tr.SetValues((*m)[0].x[0], (*m)[1].x[0], (*m)[2].x[0], v.X(), (*m)[3].x[0], (*m)[4].x[0], (*m)[5].x[0], v.Y(), (*m)[6].x[0], (*m)[7].x[0],
                 (*m)[8].x[0], v.Z());
  else
    tr.SetTranslation(gp_Vec(v));
  return tr;
}

} // namespace


namespace {

// One loop of a face on a u-periodic surface, its edges in order with
// pcurves made continuous in (u, v) from its first edge on: their uv ends,
// and its net turns round the surface.
struct UnwrappedLoop
{
  std::vector<TopoDS_Edge> edges; // as oriented in the face
  std::vector<gp_Pnt2d>    starts;
  gp_Pnt2d                 end;
  int                      turns = 0;
  bool                     ok    = true;
};

UnwrappedLoop unwrap_loop(const TopoDS_Wire& W, const TopoDS_Face& F, double period)
{
  UnwrappedLoop out;
  BRep_Builder  B;
  bool          have = false;
  gp_Pnt2d      prev;
  for (BRepTools_WireExplorer wx(W, F); wx.More(); wx.Next())
  {
    const TopoDS_Edge& E = wx.Current();
    double             f, l;
    occ::handle<Geom2d_Curve> c = BRep_Tool::CurveOnSurface(E, F, f, l);
    if (c.IsNull() || BRep_Tool::IsClosed(E, F) || BRep_Tool::Degenerated(E))
    {
      out.ok = false;
      return out;
    }
    const bool fwd = E.Orientation() != TopAbs_REVERSED;
    gp_Pnt2d   a = c->Value(fwd ? f : l), b = c->Value(fwd ? l : f);
    if (have)
    {
      const double k = std::round((prev.X() - a.X()) / period);
      if (k != 0)
      {
        occ::handle<Geom2d_Curve> t = occ::down_cast<Geom2d_Curve>(c->Copy());
        t->Translate(gp_Vec2d(k * period, 0));
        B.UpdateEdge(E, t, F, BRep_Tool::Tolerance(E));
        a.SetX(a.X() + k * period);
        b.SetX(b.X() + k * period);
      }
      if (a.Distance(prev) > 1e-6 * std::max(1.0, period))
        out.ok = false; // the loop doesn't close in (u, v): leave it to ShapeFix
    }
    out.edges.push_back(E);
    out.starts.push_back(a);
    prev = b;
    have = true;
  }
  if (!have)
  {
    out.ok = false;
    return out;
  }
  out.end   = prev;
  out.turns = (int)std::round((prev.X() - out.starts.front().X()) / period);
  if (std::abs(prev.Y() - out.starts.front().Y()) > 1e-6 * std::max(1.0, period) || std::abs(prev.X() - out.starts.front().X() - out.turns * period) > 1e-6 * period)
    out.ok = false;
  return out;
}

// A loop rotated to start at edge k, its pcurves re-made continuous.
UnwrappedLoop rotated(const UnwrappedLoop& L, size_t k, const TopoDS_Face& F, double period)
{
  BRep_Builder B;
  TopoDS_Wire  W;
  B.MakeWire(W);
  for (size_t i = 0; i < L.edges.size(); ++i)
    B.Add(W, L.edges[(k + i) % L.edges.size()]);
  return unwrap_loop(W, F, period);
}

// Shift a loop's pcurves by whole periods.
void shift_loop(UnwrappedLoop& L, const TopoDS_Face& F, double du)
{
  if (du == 0)
    return;
  BRep_Builder B;
  for (size_t i = 0; i < L.edges.size(); ++i)
  {
    const TopoDS_Edge& E = L.edges[i];
    double             f, l;
    occ::handle<Geom2d_Curve> c = occ::down_cast<Geom2d_Curve>(BRep_Tool::CurveOnSurface(E, F, f, l)->Copy());
    c->Translate(gp_Vec2d(du, 0));
    B.UpdateEdge(E, c, F, BRep_Tool::Tolerance(E));
    L.starts[i].SetX(L.starts[i].X() + du);
  }
  L.end.SetX(L.end.X() + du);
}

// A ring face (two loops winding once round a periodic surface, opposite
// ways, no seam: Parasolid's periodic faces) as OCCT needs it: one loop
// joined by a seam edge, reusing the face's edges. The seam runs straight in
// (u, v) between the loops' closest vertices (not along a u iso-line: a face
// bounded by a helix over many turns would be crossed by one). Null when
// this doesn't give a valid face.
TopoDS_Face seam_ring(const TopoDS_Face& F, double tol)
{
  TopLoc_Location                 loc;
  const occ::handle<Geom_Surface> S = BRep_Tool::Surface(F, loc);
  if (S.IsNull() || !S->IsUPeriodic())
    return TopoDS_Face();
  const double                period = S->UPeriod();
  std::vector<TopoDS_Wire>    wires;
  for (TopExp_Explorer ew(F, TopAbs_WIRE); ew.More(); ew.Next())
    wires.push_back(TopoDS::Wire(ew.Current()));
  if (wires.size() != 2)
    return TopoDS_Face();
  UnwrappedLoop A = unwrap_loop(wires[0], F, period), Bl = unwrap_loop(wires[1], F, period);
  if (!A.ok || !Bl.ok || std::abs(A.turns) != 1 || A.turns != -Bl.turns)
    return TopoDS_Face();
  // Vertex pairs by distance in (u, v) (B shifted to be closest).
  struct Pair
  {
    double d;
    size_t i, j;
  };
  std::vector<Pair> pairs;
  for (size_t i = 0; i < A.starts.size(); ++i)
    for (size_t j = 0; j < Bl.starts.size(); ++j)
    {
      const double du = Bl.starts[j].X() - A.starts[i].X();
      const double k  = std::round(du / period);
      pairs.push_back({gp_Pnt2d(Bl.starts[j].X() - k * period, Bl.starts[j].Y()).Distance(A.starts[i]), i, j});
    }
  std::sort(pairs.begin(), pairs.end(), [](const Pair& x, const Pair& y) { return x.d < y.d; });
  BRep_Builder B;
  for (size_t t = 0; t < std::min<size_t>(pairs.size(), 6); ++t)
  {
    UnwrappedLoop a = rotated(A, pairs[t].i, F, period), b = rotated(Bl, pairs[t].j, F, period);
    if (!a.ok || !b.ok)
      continue;
    // After A the walk is a turn on: start B closest to there.
    const gp_Pnt2d a2 = a.end;
    shift_loop(b, F, std::round((a2.X() - b.starts.front().X()) / period) * period);
    const gp_Pnt2d b2 = b.starts.front();
    // The seam: forward along a2 -> b2, reversed one turn back.
    occ::handle<Geom2d_Line> line = new Geom2d_Line(a2, gp_Dir2d(gp_Vec2d(a2, b2)));
    const double             len  = a2.Distance(b2);
    if (len <= 1e-12)
      continue;
    occ::handle<Geom2d_TrimmedCurve> c1 = new Geom2d_TrimmedCurve(line, 0, len);
    occ::handle<Geom2d_TrimmedCurve> c2 = occ::down_cast<Geom2d_TrimmedCurve>(c1->Copy());
    c2->Translate(gp_Vec2d(-a.turns * period, 0));
    TopoDS_Vertex va = TopExp::FirstVertex(a.edges.front(), true), vb = TopExp::FirstVertex(b.edges.front(), true);
    TopoDS_Edge   seam;
    B.MakeEdge(seam);
    B.UpdateEdge(seam, c1, c2, S, loc, tol);
    B.Range(seam, 0, len);
    va.Orientation(TopAbs_FORWARD);
    vb.Orientation(TopAbs_REVERSED);
    B.Add(seam, va);
    B.Add(seam, vb);
    try
    {
      BRepLib::BuildCurve3d(seam, tol);
    }
    catch (const Standard_Failure&)
    {
      continue;
    }
    TopoDS_Wire W;
    B.MakeWire(W);
    for (const TopoDS_Edge& e : a.edges)
      B.Add(W, e);
    B.Add(W, seam);
    for (const TopoDS_Edge& e : b.edges)
      B.Add(W, e);
    B.Add(W, seam.Reversed());
    TopoDS_Face G;
    B.MakeFace(G, S, loc, BRep_Tool::Tolerance(F));
    B.Add(G, W);
    G.Orientation(F.Orientation());
    // The pcurves above were set on F's surface: valid for G too.
    if (BRepCheck_Analyzer(G).IsValid())
      return G;
  }
  return TopoDS_Face();
}

} // namespace

// Pcurves an imported body's file doesn't store, tolerances, cone apexes and
// seams (shared by the Parasolid and ACIS readers). One edge or face the
// healing chokes on must not leave the rest unhealed: on failure, edge by
// edge and face by face.
TopoDS_Shape heal_import(TopoDS_Shape out, double tol)
{
  const auto fixed = [tol](const TopoDS_Shape& s) {
    occ::handle<ShapeFix_Shape> fix = new ShapeFix_Shape(s);
    fix->SetPrecision(tol);
    fix->SetMaxTolerance(1e-4);
    fix->Perform();
    return fix->Shape();
  };
  for (TopExp_Explorer ef(out, TopAbs_FACE); ef.More(); ef.Next())
  {
    const TopoDS_Face& F = TopoDS::Face(ef.Current());
    for (TopExp_Explorer ee(F, TopAbs_EDGE); ee.More(); ee.Next())
    {
      const TopoDS_Edge& E = TopoDS::Edge(ee.Current());
      double             f, l;
      if (BRep_Tool::Degenerated(E) || !BRep_Tool::CurveOnSurface(E, F, f, l).IsNull())
        continue;
      try
      {
        ShapeFix_Edge sfe;
        sfe.FixAddPCurve(E, F, BRep_Tool::IsClosed(E, F), tol);
      }
      catch (const Standard_Failure&)
      {
      }
    }
  }
  try
  {
    BRepLib::SameParameter(out, tol, true);
  }
  catch (const Standard_Failure&)
  {
    ShapeIMap es;
    TopExp::MapShapes(out, TopAbs_EDGE, es);
    for (int i = 1; i <= es.Extent(); ++i)
      try
      {
        BRepLib::SameParameter(TopoDS::Edge(es(i)), tol);
      }
      catch (const Standard_Failure&)
      {
      }
  }
  // Ring faces get their seams here; ShapeFix's (along a u iso-line) cuts a
  // face bounded by a long helix into one piece per turn.
  {
    occ::handle<BRepTools_ReShape> rs = new BRepTools_ReShape;
    bool                           any = false;
    for (TopExp_Explorer ef(out, TopAbs_FACE); ef.More(); ef.Next())
      try
      {
        const TopoDS_Face G = seam_ring(TopoDS::Face(ef.Current()), tol);
        if (!G.IsNull())
        {
          rs->Replace(ef.Current(), G);
          any = true;
        }
      }
      catch (const Standard_Failure&)
      {
      }
    if (any)
      out = rs->Apply(out);
  }
  try
  {
    return fixed(out);
  }
  catch (const Standard_Failure&)
  {
  }
  occ::handle<BRepTools_ReShape> re = new BRepTools_ReShape;
  for (TopExp_Explorer ef(out, TopAbs_FACE); ef.More(); ef.Next())
    try
    {
      ShapeFix_Face sf(TopoDS::Face(ef.Current()));
      sf.SetPrecision(tol);
      sf.SetMaxTolerance(1e-4);
      sf.Perform();
      re->Replace(ef.Current(), sf.Face());
    }
    catch (const Standard_Failure&)
    {
    }
  out = re->Apply(out);
  try
  {
    return fixed(out);
  }
  catch (const Standard_Failure&)
  {
    return out;
  }
}

std::vector<XtPart> read_xt_parts(const std::string& path)
{
  std::ifstream in(path, std::ios::binary);
  if (!in)
    fail_io("cannot read " + path);
  return read_xt_data(std::string((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>()));
}

namespace
{
std::vector<XtPart> read_xt_impl(std::string data, XtTree* tree);
}

std::vector<XtPart> read_xt_data(std::string data) { return read_xt_impl(std::move(data), nullptr); }

XtTree read_xt_tree(const std::string& path)
{
  std::ifstream in(path, std::ios::binary);
  if (!in)
    fail_io("cannot read " + path);
  XtTree t;
  read_xt_impl(std::string((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>()), &t);
  return t;
}

namespace
{
std::vector<XtPart> read_xt_impl(std::string data, XtTree* tree)
{
  Parser p(std::move(data));
  {
    KernelTimer t("xt.parse");
    p.parse();
  }
  Builder              b(p);
  std::vector<XtPart>  out;
  std::set<long long>  seen;
  // Each part's body and placement, healed and placed after the walk.
  struct Pending
  {
    long long body;
    gp_Trsf   place;
  };
  std::vector<Pending> pending;
  // Part: a body (with its placement), or an assembly's instances.
  std::function<void(long long, const gp_Trsf&, int)> visit = [&](long long idx, const gp_Trsf& place, int depth) {
    if (depth > 64 || !p.has(idx))
      return;
    const XNode& n = p.at_index(idx);
    if (n.type == 12)
    {
      XtPart part;
      part.shape = b.body_raw(idx);
      b.attributes(idx, part.name, part.colour, part.has_colour);
      pending.push_back({idx, place});
      out.push_back(std::move(part));
    }
    else if (n.type == 10)
    {
      // The top assembly's name (SDL/TYSA_NAME).
      if (tree && depth == 0 && tree->root.empty())
      {
        std::array<double, 3> c{};
        bool                  has = false;
        b.attributes(idx, tree->root, c, has);
        if (tree->root.empty())
          tree->root = "Assembly";
      }
      for (long long inst = n.ptr("sub_instance"); inst && p.has(inst); inst = p.at_index(inst).ptr("next_in_part"))
      {
        const XNode& I = p.at_index(inst);
        gp_Trsf      t = place;
        if (p.has(I.ptr("transform")))
          t = place * transform_of(p.at_index(I.ptr("transform")));
        visit(I.ptr("part"), t, depth + 1);
      }
    }
  };
  // Top-level parts: bodies and assemblies no instance refers to (a part
  // file's root is its body; a partition's is the WORLD).
  std::set<long long> referenced;
  for (const auto& [i, n] : p.nodes)
    if (n.type == 11)
      referenced.insert(n.ptr("part"));
  const XNode& root = p.at_index(1);
  if (root.type == 12 || root.type == 10)
    visit(1, gp_Trsf(), 0);
  else
    for (const auto& [i, n] : p.nodes)
      if ((n.type == 12 || n.type == 10) && !referenced.count(i))
        visit(i, gp_Trsf(), 0);
  // Heal the bodies in parallel (an instanced body once: its instances share
  // its edges), each under its own crash guard (unhealed if it crashes).
  {
    KernelTimer t("xt.fins");
    b.finish_fins();
  }
  KernelTimer                 heal_timer("xt.heal");
  std::map<long long, size_t> first;
  std::vector<size_t>         uniq;
  for (size_t k = 0; k < pending.size(); ++k)
    if (first.emplace(pending[k].body, k).second)
      uniq.push_back(k);
  std::vector<TopoDS_Shape> healed(pending.size());
  const double              tol = b.tol;
  OSD_Parallel::For(0, (int)uniq.size(), [&](int j) {
    const size_t       k   = uniq[j];
    const TopoDS_Shape raw = out[k].shape;
    TopoDS_Shape       h;
    healed[k] = run_guarded([&] { h = heal_import(raw, tol); }) && !h.IsNull() ? h : raw;
  });
  // The assembly tree: each distinct body once, unplaced, then the placed
  // instances (only when the file has an assembly).
  if (tree && !tree->root.empty())
  {
    std::map<long long, int> part_of;
    for (size_t k : uniq)
    {
      XtPart part = out[k];
      part.shape  = healed[k];
      b.face_colours_of(part);
      part_of[pending[k].body] = (int)tree->parts.size();
      tree->parts.push_back(std::move(part));
    }
    for (size_t k = 0; k < pending.size(); ++k)
    {
      tree->instances.emplace_back(part_of[pending[k].body], pending[k].place);
      tree->names.push_back(out[k].name);
    }
  }
  for (size_t k = 0; k < pending.size(); ++k)
  {
    out[k].shape = healed[first[pending[k].body]];
    b.face_colours_of(out[k]);
    if (pending[k].place.Form() != gp_Identity)
      out[k].shape = BRepBuilderAPI_Transform(out[k].shape, pending[k].place, true).Shape();
  }
  if (out.empty())
    fail_invalid("parasolid: no parts in the file");
  return out;
}
} // namespace

} // namespace noshape
