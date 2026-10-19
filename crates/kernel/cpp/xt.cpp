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
// non-periodic B-splines; surfaces of extrusion, revolution and offset are
// converted first (same parameterisation), and faces closed on a B-spline
// surface are split (an XT B-surface attached to a face may not be closed).
// Seam and degenerate edges of the periodic analytic surfaces are dropped: an
// XT cylinder face is bounded by its two circles alone.
//
// Edges whose curve lies on both faces within Parasolid's precision (1e-8 m)
// are accurate; the others (approximated intersections) are tolerant: no edge
// curve, each fin carries a trimmed SP-curve (the OCCT pcurve on the XT
// surface), and their vertices are tolerant too. Accurate edges at tolerant
// vertices carry trimmed curves.
//
// Senses: an edge runs along its curve; the face normal is the outward normal
// (solid) or the OCCT face normal (sheet), and face->sense says whether it
// agrees with the XT surface's natural normal (computed from the XT
// definition: a cone's points toward its axis). Fins follow the OCCT edge
// orientation in the face, which keeps the face on their left.

#include "internal.hpp"

#include <BRepAdaptor_Surface.hxx>
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
#include <Geom_RectangularTrimmedSurface.hxx>
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

const char* kVersion = ": TRANSMIT FILE created by modeller version 1200000";
const char* kSchema  = "SCH_1200000_12006";

std::string text_body(const std::vector<Node>& nodes)
{
  std::string s = "T";
  auto        put = [&](const std::string& t) {
    s += t;
    s += ' ';
  };
  put(std::to_string(std::strlen(kVersion)));
  s += kVersion;
  put(std::to_string(std::strlen(kSchema)));
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

std::string binary_body(const std::vector<Node>& nodes)
{
  const double kNullReal = -3.14158e13;
  Bin          b;
  b.s = std::string("PS\0\0", 4);
  b.i16((int)std::strlen(kVersion));
  b.s += kVersion;
  b.i32((long long)std::strlen(kSchema));
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

std::string header(bool binary)
{
  std::string h;
  h += "**ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz**************************\n";
  h += "**PARASOLID !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~0123456789**************************\n";
  h += "**PART1;MC=unknown;APPL=noshape;FORMAT=";
  h += binary ? "binary" : "text";
  h += ";GUISE=transmit;\n";
  h += "**PART2;SCH=SCH_1200000_12006;USFLD_SIZE=0;\n";
  h += "**PART3;\n";
  h += "**END_OF_HEADER*****************************************************************\n";
  return h;
}

// ---------------------------------------------------------------- geometry

// An XT surface: its node, how OCCT (u, v) map to XT (u, v) (u' = su u + ou,
// v' = sv v), and its natural normal at a point.
struct XSurface
{
  int                                       index = 0;
  double                                    su = 1, ou = 0, sv = 1;
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

bool periodic_analytic(const occ::handle<Geom_Surface>& s)
{
  return !occ::down_cast<Geom_CylindricalSurface>(s).IsNull() || !occ::down_cast<Geom_ConicalSurface>(s).IsNull() ||
         !occ::down_cast<Geom_SphericalSurface>(s).IsNull() || !occ::down_cast<Geom_ToroidalSurface>(s).IsNull();
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
      // XT: R = P - v A + (X cos u + Y sin u)(r + v tan a), Y = A x X, A toward the apex.
      const gp_Ax3 a     = c->Position();
      const double alpha = c->SemiAngle();
      const gp_XYZ z     = a.Direction().XYZ();
      const gp_XYZ A     = alpha > 0 ? z.Reversed() : z;
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
        return -radial - gp_Vec(A) * t;
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

  // Trimmed SP-curve of a tolerant edge's fin: the OCCT pcurve on the face's XT surface.
  int fin_curve(int fin_node, const TopoDS_Edge& e, int face)
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
      const double   u = xs.su * q.X() + xs.ou, v = xs.sv * q.Y();
      p.insert(p.end(), {u * wt, v * wt});
      if (rat)
        p.push_back(wt);
    }
    const int bc = w.b_curve(0, p, rat ? 3 : 2, rat, b->Degree(), b->NbPoles(), b->Knots(), b->Multiplicities(), false);
    const int sp = w.alloc();
    Node&     n  = add(137, sp);
    Writer::geom_head(n, w.next_id++, body, '+');
    n.fields.insert(n.fields.end(), {fp(xs.index), fp(bc), fp(0), ff_null()});
    chain_curve.push_back(sp);
    depend(xs.index, sp);
    TopLoc_Location           loc;
    occ::handle<Geom_Surface> surf = BRep_Tool::Surface(F, loc);
    auto at = [&](double t) { return surf->Value(pc->Value(t).X(), pc->Value(t).Y()).Transformed(loc.Transformation()); };
    return trimmed(fin_node, sp, at(f), at(l), f, l);
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
      if (dev > kPrecision)
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
      occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
      for (int end = 0; end < 2; ++end)
      {
        const TopoDS_Vertex v  = end ? TopExp::LastVertex(e) : TopExp::FirstVertex(e);
        const int           vi = vertices.FindIndex(v) - 1;
        double              gap = c.IsNull() ? 1e9 : c->Value(end ? l : f).Distance(BRep_Tool::Pnt(v));
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
            fins[k].curve = fin_curve(fins[k].index, e, fins[k].face);
        continue;
      }
      double                  f, l;
      occ::handle<Geom_Curve> c = curve_of(e, f, l);
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
      p.fields     = {fd(w.next_id++), fp(0), fp(vertex_idx[i]), fp(0), fp(0), fv(BRep_Tool::Pnt(TopoDS::Vertex(vertices(i + 1))).XYZ())};
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

// Prepare a shape for XT: elementary surfaces where swept/revolved ones are,
// B-splines for the other swept, revolved and offset surfaces (same
// parameterisation), and faces closed on a surface that isn't periodic-analytic split.
TopoDS_Shape prepared(const TopoDS_Shape& s)
{
  TopoDS_Shape r = ShapeCustom::SweptToElementary(s);
  r              = ShapeCustom::ConvertToBSpline(r, true, true, true, false);
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

TopoDS_Shape exchange_prepared(const TopoDS_Shape& s)
{
  return prepared(s);
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
  return binary_body(w.nodes);
}

void write_parasolid(const OcShapeList& shapes, rust::Str path, bool binary)
{
  Writer           w;
  std::vector<int> bodies;
  const bool       list = shapes.items.size() != 1;
  const int        root = list ? w.alloc() : 0;
  for (const auto& s : shapes.items)
  {
    require_non_null(s, "shape");
    const TopoDS_Shape p = prepared(s);
    bool               solid = false;
    for (TopExp_Explorer ex(p, TopAbs_SOLID); ex.More(); ex.Next())
      solid = true;
    const int body = w.alloc();
    bodies.push_back(body);
    w.next_id = 1;
    BodyWriter bw(w, body, solid);
    bw.write(p);
  }
  if (list)
  {
    Node& n     = w.add(74, root);
    n.variable  = true;
    n.var_count = (int)bodies.size();
    n.fields    = {fd((long long)bodies.size()), fp(0)};
    for (int b : bodies)
      n.fields.push_back(fp(b));
  }
  const std::string   data = header(binary) + (binary ? binary_body(w.nodes) : text_body(w.nodes));
  std::ofstream       out(std::string(path), std::ios::binary);
  if (!out)
    fail_invalid("cannot write " + std::string(path));
  out.write(data.data(), (std::streamsize)data.size());
  if (!out)
    fail_op("writing " + std::string(path) + " failed");
}

// ---------------------------------------------------------------- reader
//
// Reads the transmit files written above (schema 12006, the node types this
// writer uses) back into OCCT: for round-trip checks, and Parasolid import
// of such files. Orientation comes from the file alone: face senses and fin
// directions are not repaired, so a wrong one shows as an invalid solid.

namespace {

// Field types per node type (schema 12006, the nodes written above).
const std::map<int, std::string>& schema()
{
  static const std::map<int, std::string> s = {
    {12, "dppppppffpppupuuppppppp"}, {13, "dpppppppp"}, {14, "dpfpppppcppppp"}, {15, "dpppp"},
    {16, "dpfppppppp"}, {17, "pppppppppc"}, {18, "dpppppfp"}, {19, "dpppppc"},
    {29, "dppppv"}, {30, "dpppppcvv"}, {31, "dpppppcvvvf"}, {32, "dpppppcvvvff"},
    {45, "F"}, {50, "dpppppcvvv"}, {51, "dpppppcvvfv"}, {52, "dpppppcvvfffv"},
    {53, "dpppppcvfvv"}, {54, "dpppppcvvffv"}, {74, "dpP"}, {124, "dpppppcpp"},
    {126, "llnnddUUddlllunppppp"}, {127, "N"}, {128, "F"}, {133, "dpppppcpvvff"},
    {134, "dpppppcpp"}, {136, "ndndullluppp"}, {137, "dpppppcpppf"}, {141, "pppp"},
  };
  return s;
}

struct PNode
{
  int                type = 0;
  std::vector<Field> f;
};

struct Reader
{
  std::string           data;
  size_t                at = 0;
  bool                  binary = false;
  std::map<int, PNode>  nodes;

  // ---- text
  std::string text; // records joined, trailing spaces removed
  size_t      t = 0;

  long long tint()
  {
    size_t e = text.find(' ', t);
    if (e == std::string::npos)
      fail_invalid("parasolid: truncated file");
    long long v = std::stoll(text.substr(t, e - t));
    t           = e + 1;
    return v;
  }
  Field tfield(char ty)
  {
    Field f{ty};
    if (text[t] == '?' && ty != 'c' && ty != 'l')
    {
      ++t;
      f.null = true;
      return f;
    }
    switch (ty)
    {
      case 'c':
        f.i = text[t++];
        break;
      case 'l':
        f.i = text[t++] == 'T';
        break;
      case 'f':
      case 'v':
        for (int k = 0; k < (ty == 'v' ? 3 : 1); ++k)
        {
          size_t e = text.find(' ', t);
          f.x[k]   = std::strtod(text.substr(t, e - t).c_str(), nullptr);
          t        = e + 1;
        }
        break;
      default:
        f.i = tint();
    }
    return f;
  }

  // ---- binary (big-endian)
  int b8() { return (unsigned char)data.at(at++); }
  int b16()
  {
    int v = (b8() << 8) | b8();
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
    double d;
    std::memcpy(&d, &v, 8);
    return d;
  }
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
  Field bfield(char ty)
  {
    Field f{ty};
    switch (ty)
    {
      case 'd':
        f.i = b32();
        break;
      case 'p':
        f.i = bptr();
        break;
      case 'n':
        f.i = b16();
        break;
      case 'u':
      case 'c':
      case 'l':
        f.i = b8();
        break;
      case 'f':
        f.x[0] = b64();
        f.null = f.x[0] == -3.14158e13;
        break;
      case 'v':
        for (int k = 0; k < 3; ++k)
          f.x[k] = b64();
        f.null = f.x[0] == -3.14158e13;
        break;
    }
    return f;
  }

  void parse()
  {
    const size_t h = data.find("**END_OF_HEADER");
    if (h == std::string::npos)
      fail_invalid("parasolid: no header");
    at = data.find('\n', h);
    if (at == std::string::npos)
      fail_invalid("parasolid: truncated file");
    ++at;
    binary = data.compare(at, 4, std::string("PS\0\0", 4)) == 0;
    auto field = [&](char ty) { return binary ? bfield(ty) : tfield(ty); };
    if (binary)
    {
      at += 4;
      at += b16();
      at += b32();
      b32();
    }
    else
    {
      std::istringstream in(data.substr(at));
      std::string        line;
      while (std::getline(in, line))
      {
        while (!line.empty() && (line.back() == ' ' || line.back() == '\r'))
          line.pop_back();
        text += line;
      }
      text += ' ';
      if (text[t++] != 'T')
        fail_invalid("parasolid: not a transmit file");
      t += tint();
      t += tint();
      tint();
    }
    for (;;)
    {
      const int type = binary ? b16() : (int)tint();
      if (type == 1)
        break;
      auto it = schema().find(type);
      if (it == schema().end())
        fail_unsupported("parasolid: node type " + std::to_string(type) + " is not supported");
      const std::string& spec = it->second;
      const bool variable = std::any_of(spec.begin(), spec.end(), [](char c) { return c == 'F' || c == 'N' || c == 'P'; });
      long long  count    = 0;
      if (variable)
        count = binary ? b32() : tint();
      const int index = (int)(binary ? bptr() : tint());
      PNode     n;
      n.type = type;
      for (char c : spec)
      {
        if (c == 'F' || c == 'N' || c == 'P')
        {
          const char ty = c == 'F' ? 'f' : c == 'N' ? 'n' : 'p';
          for (long long k = 0; k < count; ++k)
            n.f.push_back(field(ty));
        }
        else
          n.f.push_back(field(c == 'U' ? 'u' : c));
      }
      nodes[index] = std::move(n);
    }
  }

  const PNode& at_index(long long i)
  {
    auto it = nodes.find((int)i);
    if (it == nodes.end())
      fail_invalid("parasolid: dangling pointer " + std::to_string(i));
    return it->second;
  }
};

gp_Pnt P(const Field& f)
{
  return gp_Pnt(f.x[0], f.x[1], f.x[2]);
}
gp_Dir D(const Field& f)
{
  return gp_Dir(f.x[0], f.x[1], f.x[2]);
}

struct Builder
{
  Reader&                                   r;
  std::map<int, occ::handle<Geom_Surface>>  surf;
  std::map<int, double>                     cone_cos; // cones: OCCT (u, v) = (-u, v / cos a)
  std::map<int, TopoDS_Vertex>              verts;
  std::map<int, TopoDS_Edge>                edges;
  BRep_Builder                              B;

  explicit Builder(Reader& r_) : r(r_) {}

  void nurbs(const PNode& n, int skip, std::vector<double>& poles, std::vector<double>& knots, std::vector<int>& mults)
  {
    (void)skip;
    const PNode& pv = r.at_index(n.f[n.f.size() - 3].i);
    const PNode& km = r.at_index(n.f[n.f.size() - 2].i);
    const PNode& ks = r.at_index(n.f[n.f.size() - 1].i);
    for (auto& x : pv.f)
      poles.push_back(x.x[0]);
    for (auto& x : km.f)
      mults.push_back((int)x.i);
    for (auto& x : ks.f)
      knots.push_back(x.x[0]);
  }

  static NCollection_Array1<double> arr(const std::vector<double>& v)
  {
    NCollection_Array1<double> a(1, (int)v.size());
    for (size_t i = 0; i < v.size(); ++i)
      a((int)i + 1) = v[i];
    return a;
  }
  static NCollection_Array1<int> arr(const std::vector<int>& v)
  {
    NCollection_Array1<int> a(1, (int)v.size());
    for (size_t i = 0; i < v.size(); ++i)
      a((int)i + 1) = v[i];
    return a;
  }

  occ::handle<Geom_Surface> surface(int idx)
  {
    if (surf.count(idx))
      return surf[idx];
    const PNode&              n = r.at_index(idx);
    const auto&               f = n.f;
    occ::handle<Geom_Surface> s;
    switch (n.type)
    {
      case 50:
        s = new Geom_Plane(gp_Ax3(P(f[7]), D(f[8]), D(f[9])));
        break;
      case 51:
        s = new Geom_CylindricalSurface(gp_Ax3(P(f[7]), D(f[8]), D(f[10])), f[9].x[0]);
        break;
      case 52:
      {
        const double a = std::atan2(f[10].x[0], f[11].x[0]);
        s              = new Geom_ConicalSurface(gp_Ax3(P(f[7]), D(f[8]).Reversed(), D(f[12])), a, f[9].x[0]);
        cone_cos[idx]  = std::cos(a);
        break;
      }
      case 53:
        s = new Geom_SphericalSurface(gp_Ax3(P(f[7]), D(f[9]), D(f[10])), f[8].x[0]);
        break;
      case 54:
        s = new Geom_ToroidalSurface(gp_Ax3(P(f[7]), D(f[8]), D(f[11])), f[9].x[0], f[10].x[0]);
        break;
      case 124:
      {
        const PNode& ns = r.at_index(f[7].i);
        const auto&  g  = ns.f;
        const int    du = (int)g[2].i, dv = (int)g[3].i, nu = (int)g[4].i, nv = (int)g[5].i, dim = (int)g[14].i;
        const bool   rat = g[10].i;
        const PNode& pv  = r.at_index(g[15].i);
        NCollection_Array2<gp_Pnt> poles(1, nu, 1, nv);
        NCollection_Array2<double> w(1, nu, 1, nv);
        for (int i = 0; i < nu; ++i)
          for (int j = 0; j < nv; ++j)
          {
            const double* q  = &pv.f[(size_t)((i * nv + j) * dim)].x[0];
            const double  wt = rat ? pv.f[(size_t)((i * nv + j) * dim + 3)].x[0] : 1.0;
            poles(i + 1, j + 1) = gp_Pnt(pv.f[(size_t)((i * nv + j) * dim)].x[0] / wt, pv.f[(size_t)((i * nv + j) * dim + 1)].x[0] / wt,
                                         pv.f[(size_t)((i * nv + j) * dim + 2)].x[0] / wt);
            w(i + 1, j + 1) = wt;
            (void)q;
          }
        auto ints = [&](long long i) {
          std::vector<int> v;
          for (auto& x : r.at_index(i).f)
            v.push_back((int)x.i);
          return v;
        };
        auto reals = [&](long long i) {
          std::vector<double> v;
          for (auto& x : r.at_index(i).f)
            v.push_back(x.x[0]);
          return v;
        };
        s = rat ? new Geom_BSplineSurface(poles, w, arr(reals(g[18].i)), arr(reals(g[19].i)), arr(ints(g[16].i)), arr(ints(g[17].i)), du, dv)
                : new Geom_BSplineSurface(poles, arr(reals(g[18].i)), arr(reals(g[19].i)), arr(ints(g[16].i)), arr(ints(g[17].i)), du, dv);
        break;
      }
      default:
        fail_unsupported("parasolid: surface type " + std::to_string(n.type));
    }
    return surf[idx] = s;
  }

  // XT natural normal agrees with OCCT's except for cones.
  bool natural_flipped(int idx) { return r.at_index(idx).type == 52; }

  // B_CURVE (3D or 2D) as poles, weights, knots.
  void b_curve(int idx, int& degree, int& dim, bool& rat, std::vector<double>& p, std::vector<double>& knots, std::vector<int>& mults)
  {
    const PNode& b  = r.at_index(idx);
    const PNode& nc = r.at_index(b.f[7].i);
    degree          = (int)nc.f[0].i;
    dim             = (int)nc.f[2].i;
    rat             = nc.f[7].i;
    for (auto& x : r.at_index(nc.f[9].i).f)
      p.push_back(x.x[0]);
    for (auto& x : r.at_index(nc.f[10].i).f)
      mults.push_back((int)x.i);
    for (auto& x : r.at_index(nc.f[11].i).f)
      knots.push_back(x.x[0]);
  }

  occ::handle<Geom_Curve> curve(int idx)
  {
    const PNode& n = r.at_index(idx);
    const auto&  f = n.f;
    switch (n.type)
    {
      case 30:
        return new Geom_Line(P(f[7]), D(f[8]));
      case 31:
        return new Geom_Circle(gp_Ax2(P(f[7]), D(f[8]), D(f[9])), f[10].x[0]);
      case 32:
        return new Geom_Ellipse(gp_Ax2(P(f[7]), D(f[8]), D(f[9])), f[10].x[0], f[11].x[0]);
      case 134:
      {
        int                 deg, dim;
        bool                rat;
        std::vector<double> p, k;
        std::vector<int>    m;
        b_curve(idx, deg, dim, rat, p, k, m);
        const int                  np = (int)p.size() / dim;
        NCollection_Array1<gp_Pnt> poles(1, np);
        NCollection_Array1<double> w(1, np);
        for (int i = 0; i < np; ++i)
        {
          const double wt = rat ? p[(size_t)(i * dim + 3)] : 1.0;
          poles(i + 1)    = gp_Pnt(p[(size_t)(i * dim)] / wt, p[(size_t)(i * dim + 1)] / wt, p[(size_t)(i * dim + 2)] / wt);
          w(i + 1)        = wt;
        }
        if (rat)
          return new Geom_BSplineCurve(poles, w, arr(k), arr(m), deg);
        return new Geom_BSplineCurve(poles, arr(k), arr(m), deg);
      }
      default:
        fail_unsupported("parasolid: curve type " + std::to_string(n.type));
    }
  }

  occ::handle<Geom2d_Curve> curve2d(int idx, int surface)
  {
    int                 deg, dim;
    bool                rat;
    std::vector<double> p, k;
    std::vector<int>    m;
    b_curve(idx, deg, dim, rat, p, k, m);
    const int                    np = (int)p.size() / dim;
    NCollection_Array1<gp_Pnt2d> poles(1, np);
    NCollection_Array1<double>   w(1, np);
    const double                 cc = cone_cos.count(surface) ? cone_cos[surface] : 0;
    for (int i = 0; i < np; ++i)
    {
      const double wt = rat ? p[(size_t)(i * dim + 2)] : 1.0;
      double       u = p[(size_t)(i * dim)] / wt, v = p[(size_t)(i * dim + 1)] / wt;
      if (cc)
        u = -u, v = v / cc;
      poles(i + 1) = gp_Pnt2d(u, v);
      w(i + 1)     = wt;
    }
    if (rat)
      return new Geom2d_BSplineCurve(poles, w, arr(k), arr(m), deg);
    return new Geom2d_BSplineCurve(poles, arr(k), arr(m), deg);
  }

  TopoDS_Vertex vertex(long long idx)
  {
    if (verts.count((int)idx))
      return verts[(int)idx];
    const PNode&  v = r.at_index(idx);
    TopoDS_Vertex V;
    B.MakeVertex(V, P(r.at_index(v.f[5].i).f[5]), v.f[6].null ? 1e-7 : std::max(1e-7, v.f[6].x[0]));
    return verts[(int)idx] = V;
  }

  static double param_on(const occ::handle<Geom_Curve>& c, const gp_Pnt& p)
  {
    GeomAPI_ProjectPointOnCurve pr(p, c);
    if (pr.NbPoints() == 0)
      fail_op("parasolid: a vertex is off its edge's curve");
    return pr.LowerDistanceParameter();
  }

  TopoDS_Edge edge(long long idx)
  {
    if (edges.count((int)idx))
      return edges[(int)idx];
    const PNode&  e   = r.at_index(idx);
    const PNode&  fin = r.at_index(e.f[3].i);
    const PNode&  oth = r.at_index(fin.f[5].i);
    TopoDS_Vertex v1  = vertex(oth.f[4].i), v2 = vertex(fin.f[4].i);
    const double  tol = e.f[2].null ? 1e-7 : std::max(1e-7, e.f[2].x[0]);
    TopoDS_Edge   E;
    if (e.f[6].i)
    {
      const PNode&            cn = r.at_index(e.f[6].i);
      occ::handle<Geom_Curve> c;
      double                  t1, t2;
      if (cn.type == 133)
      {
        c  = curve((int)cn.f[7].i);
        t1 = cn.f[10].x[0];
        t2 = cn.f[11].x[0];
      }
      else
      {
        c  = curve((int)e.f[6].i);
        t1 = param_on(c, BRep_Tool::Pnt(v1));
        t2 = param_on(c, BRep_Tool::Pnt(v2));
        if (c->IsPeriodic())
        {
          while (t2 <= t1 + 1e-12)
            t2 += c->Period();
        }
      }
      B.MakeEdge(E, c, tol);
      B.Range(E, t1, t2);
    }
    else
    {
      // Tolerant: the fins' SP-curves.
      B.MakeEdge(E);
      B.UpdateEdge(E, tol);
      double t1 = 0, t2 = 0;
      for (long long fi : {e.f[3].i, fin.f[5].i})
      {
        const PNode& x = r.at_index(fi);
        if (!x.f[7].i)
          continue;
        const PNode& tr = r.at_index(x.f[7].i);
        const PNode& sp = r.at_index(tr.f[7].i);
        const int    s  = (int)sp.f[7].i;
        B.UpdateEdge(E, curve2d((int)sp.f[8].i, s), surface(s), TopLoc_Location(), tol);
        t1 = tr.f[10].x[0];
        t2 = tr.f[11].x[0];
      }
      B.Range(E, t1, t2);
      BRepLib::BuildCurve3d(E, tol);
    }
    v1.Orientation(TopAbs_FORWARD);
    v2.Orientation(TopAbs_REVERSED);
    B.Add(E, v1);
    B.Add(E, v2);
    return edges[(int)idx] = E;
  }

  TopoDS_Face face(long long idx)
  {
    const PNode&              n  = r.at_index(idx);
    const int                 si = (int)n.f[7].i;
    occ::handle<Geom_Surface> s  = surface(si);
    // XT face normal = natural normal, reversed when exactly one sense is '-'.
    const bool xt_reversed = (n.f[8].i == '-') != (r.at_index(si).f[6].i == '-');
    const bool flip        = xt_reversed != natural_flipped(si);
    TopoDS_Face F;
    B.MakeFace(F, s, 1e-7);
    for (long long l = n.f[5].i; l; l = r.at_index(l).f[4].i)
    {
      if (r.at_index(r.at_index(l).f[2].i).f[6].i == 0)
        continue; // an isolated loop (a cone's apex): OCCT has its degenerate edge added by the fix below
      TopoDS_Wire W;
      B.MakeWire(W);
      const long long first = r.at_index(l).f[2].i;
      long long       fi    = first;
      do
      {
        const PNode& fin = r.at_index(fi);
        TopoDS_Edge  E   = edge(fin.f[6].i);
        const bool   pos = (fin.f[9].i == '+') != flip;
        E.Orientation(pos ? TopAbs_FORWARD : TopAbs_REVERSED);
        B.Add(W, E);
        fi = fin.f[2].i;
      } while (fi != first);
      B.Add(F, W);
    }
    if (flip)
      F.Reverse();
    return F;
  }
};

} // namespace

std::unique_ptr<OcShape> read_parasolid(rust::Str path)
{
  std::ifstream in(std::string(path), std::ios::binary);
  if (!in)
    fail_invalid("cannot read " + std::string(path));
  Reader r;
  r.data.assign(std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>());
  r.parse();
  Builder         b(r);
  std::vector<long long> bodies;
  const PNode&    root = r.at_index(1);
  if (root.type == 12)
    bodies.push_back(1);
  else if (root.type == 74)
    for (size_t k = 2; k < root.f.size(); ++k)
      bodies.push_back(root.f[k].i);
  TopoDS_Compound out;
  b.B.MakeCompound(out);
  occ::handle<ShapeBuild_ReShape> ctx = new ShapeBuild_ReShape;
  for (long long bi : bodies)
  {
    const PNode& body  = r.at_index(bi);
    const bool   solid = body.f[14].i == 1;
    TopoDS_Compound part;
    b.B.MakeCompound(part);
    for (long long reg = body.f[20].i; reg; reg = r.at_index(reg).f[3].i)
    {
      const PNode& R = r.at_index(reg);
      if (solid && R.f[6].i != 'S')
        continue;
      TopoDS_Solid so;
      b.B.MakeSolid(so);
      for (long long sh = R.f[5].i; sh; sh = r.at_index(sh).f[3].i)
      {
        TopoDS_Shell shell;
        b.B.MakeShell(shell);
        for (long long fi = r.at_index(sh).f[4].i; fi; fi = r.at_index(fi).f[3].i)
          b.B.Add(shell, b.face(fi));
        if (solid)
          b.B.Add(so, shell);
        else
          b.B.Add(part, shell);
      }
      if (solid)
        b.B.Add(part, so);
    }
    b.B.Add(out, part);
  }
  // Missing pcurves and the seams OCCT wants on periodic faces; never orientation.
  for (TopExp_Explorer ef(out, TopAbs_FACE); ef.More(); ef.Next())
  {
    ShapeFix_Face fix(TopoDS::Face(ef.Current()));
    fix.SetContext(ctx);
    fix.FixOrientationMode() = 0;
    fix.Perform();
  }
  TopoDS_Shape res = ctx->Apply(out);
  BRepLib::SameParameter(res, 1e-7, true);
  return std::make_unique<OcShape>(res);
}

} // namespace noshape
