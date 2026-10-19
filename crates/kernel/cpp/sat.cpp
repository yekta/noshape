// ACIS save files (.sat, text), version 7.0: every ACIS-based and
// translator-based reader accepts it. Record layouts follow the ACIS 7.0 "SAT
// Save File Format" chapter and the class reference's restore descriptions
// (entity: name, attribute pointer, tag, pattern pointer, data, '#').
//
// One body per shape: a lump per solid, its shells, faces (single sided,
// sense against the surface normal so the face normal points out of the
// solid), loops of coedges (face on the left), edges with their curve
// parameter range, vertices and points. Planes, cylinders/cones (cone-surface
// with sine/cosine of the half angle: cosine > 0 = normal away from the
// axis), spheres, tori, straight lines and ellipses are written as such,
// everything else as exact B-splines (exactsur / exactcur, "nubs"/"nurbs";
// ACIS knot vectors leave out the outermost knot at each end, so end
// multiplicities are the degree). Faces get no seams on periodic analytic
// surfaces, like Parasolid's (shared preparation: xt.cpp). Lengths are
// written in millimetres.

#include "internal.hpp"

#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <BRepClass3d.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepLProp_SLProps.hxx>
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
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>

#include <cmath>
#include <cstdio>
#include <ctime>
#include <fstream>
#include <functional>
#include <map>
#include <string>
#include <vector>

namespace noshape {

namespace {

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
  return t;
}

std::string vec(const gp_XYZ& v)
{
  return num(v.X()) + " " + num(v.Y()) + " " + num(v.Z());
}

std::string ptr(int i)
{
  return "$" + std::to_string(i);
}

// Discontinuity lists of a knot vector: knots where the curve is only C0, C1, C2.
std::string discontinuities(const NCollection_Array1<double>& knots, const NCollection_Array1<int>& mults, int degree)
{
  std::vector<double> c[3];
  for (int i = knots.Lower() + 1; i < knots.Upper(); ++i)
  {
    const int continuity = degree - mults(i);
    if (continuity >= 0 && continuity < 3)
      c[continuity].push_back(knots(i));
  }
  std::string s;
  for (auto& l : c)
  {
    s += std::to_string(l.size()) + " ";
    for (double k : l)
      s += num(k) + " ";
  }
  return s;
}

// ACIS knots: distinct values with multiplicities, the outermost knots at most `degree` deep.
std::string acis_knots(const NCollection_Array1<double>& knots, const NCollection_Array1<int>& mults, int degree)
{
  std::string s;
  for (int i = knots.Lower(); i <= knots.Upper(); ++i)
    s += num(knots(i)) + " " + std::to_string(std::min(mults(i), degree)) + " ";
  return s;
}

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

struct Sat
{
  std::vector<std::string> records;

  int reserve()
  {
    records.emplace_back();
    return (int)records.size() - 1;
  }
  void set(int i, const std::string& type, const std::string& data)
  {
    records[i] = type + " $-1 -1 $-1 " + data + "#";
  }
  int add(const std::string& type, const std::string& data)
  {
    const int i = reserve();
    set(i, type, data);
    return i;
  }

  // The ACIS surface of a face and its outward test: whether the ACIS
  // surface normal at a point agrees with a given direction.
  struct Surf
  {
    int                                                    index;
    std::function<gp_Vec(const gp_Pnt&, double, double)> normal; // point, OCCT (u, v)
  };

  Surf surface(const TopoDS_Face& f)
  {
    TopLoc_Location           loc;
    occ::handle<Geom_Surface> s = BRep_Tool::Surface(f, loc);
    while (auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
      s = t->BasisSurface();
    if (!loc.IsIdentity())
      s = occ::down_cast<Geom_Surface>(s->Transformed(loc.Transformation()));
    if (auto p = occ::down_cast<Geom_Plane>(s))
    {
      const gp_Ax3 a = p->Position();
      const gp_XYZ n = a.XDirection().XYZ().Crossed(a.YDirection().XYZ());
      return {add("plane-surface", vec(a.Location().XYZ()) + " " + vec(n) + " " + vec(a.XDirection().XYZ()) + " forward_v I I I I "),
              [n](const gp_Pnt&, double, double) { return gp_Vec(n); }};
    }
    auto radial = [](const gp_Pnt& o, const gp_Dir& ax) {
      return [o, ax](const gp_Pnt& q, double, double) {
        gp_Vec r(o, q);
        return r - gp_Vec(ax) * r.Dot(gp_Vec(ax));
      };
    };
    if (auto c = occ::down_cast<Geom_CylindricalSurface>(s))
    {
      const gp_Ax3 a = c->Position();
      const double r = c->Radius();
      return {add("cone-surface", vec(a.Location().XYZ()) + " " + vec(a.Direction().XYZ()) + " " + vec(a.XDirection().XYZ() * r) + " 1 I I 0 1 " + num(r) + " forward I I I I "),
              radial(a.Location(), a.Direction())};
    }
    if (auto c = occ::down_cast<Geom_ConicalSurface>(s))
    {
      // Base ellipse where the radius isn't zero: at the middle of the face.
      const gp_Ax3 a = c->Position();
      const double al = c->SemiAngle();
      double       u0, u1, v0, v1;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      double v = 0.5 * (v0 + v1);
      if (std::abs(c->RefRadius() + v * std::sin(al)) < 1e-9)
        v += 1;
      const double r      = c->RefRadius() + v * std::sin(al);
      const gp_Pnt centre = a.Location().Translated(gp_Vec(a.Direction()) * (v * std::cos(al)));
      // Radius growing along the axis (sin, cos same sign) unless r < 0 flipped it.
      const double sn = r > 0 ? std::sin(al) : -std::sin(al);
      return {add("cone-surface", vec(centre.XYZ()) + " " + vec(a.Direction().XYZ()) + " " + vec(a.XDirection().XYZ() * std::abs(r)) + " 1 I I " + num(sn) + " " + num(std::cos(al)) + " " +
                                    num(std::abs(r)) + " forward I I I I "),
              radial(a.Location(), a.Direction())};
    }
    if (auto sp = occ::down_cast<Geom_SphericalSurface>(s))
    {
      const gp_Ax3 a = sp->Position();
      const gp_Pnt o = a.Location();
      return {add("sphere-surface", vec(o.XYZ()) + " " + num(sp->Radius()) + " " + vec(a.XDirection().XYZ()) + " " + vec(a.Direction().XYZ()) + " forward_v I I I I "),
              [o](const gp_Pnt& q, double, double) { return gp_Vec(o, q); }};
    }
    if (auto t = occ::down_cast<Geom_ToroidalSurface>(s))
    {
      const gp_Ax3 a = t->Position();
      const gp_Pnt o = a.Location();
      const gp_Dir ax = a.Direction();
      const double R = t->MajorRadius();
      return {add("torus-surface", vec(o.XYZ()) + " " + vec(ax.XYZ()) + " " + num(R) + " " + num(t->MinorRadius()) + " " + vec(a.XDirection().XYZ()) + " forward_v I I I I "),
              [o, ax, R](const gp_Pnt& q, double, double) {
                gp_Vec r(o, q);
                gp_Vec rad = r - gp_Vec(ax) * r.Dot(gp_Vec(ax));
                if (rad.Magnitude() > 0)
                  rad.Normalize();
                return gp_Vec(gp_Pnt(o.XYZ() + rad.XYZ() * R), q);
              }};
    }
    occ::handle<Geom_BSplineSurface> b = occ::down_cast<Geom_BSplineSurface>(s);
    if (b.IsNull() || b->IsUPeriodic() || b->IsVPeriodic())
    {
      double u0, u1, v0, v1;
      BRepTools::UVBounds(f, u0, u1, v0, v1);
      b = GeomConvert::SurfaceToBSplineSurface(new Geom_RectangularTrimmedSurface(s, u0, u1, v0, v1));
      if (b->IsUPeriodic() || b->IsVPeriodic())
      {
        if (b->IsUPeriodic())
          b->SetUNotPeriodic();
        if (b->IsVPeriodic())
          b->SetVNotPeriodic();
        b->Segment(u0, u1, v0, v1);
      }
    }
    const bool  rat = b->IsURational() || b->IsVRational();
    std::string d   = std::string("forward { exactsur full ") + (rat ? "nurbs " : "nubs ") + std::to_string(b->UDegree()) + " " + std::to_string(b->VDegree()) +
                    (rat ? " both" : "") + " open open none none " + std::to_string(b->NbUKnots()) + " " + std::to_string(b->NbVKnots()) + " " +
                    acis_knots(b->UKnots(), b->UMultiplicities(), b->UDegree()) + acis_knots(b->VKnots(), b->VMultiplicities(), b->VDegree());
    for (int j = 1; j <= b->NbVPoles(); ++j) // u varies fastest
      for (int i = 1; i <= b->NbUPoles(); ++i)
      {
        d += vec(b->Pole(i, j).XYZ()) + " ";
        if (rat)
          d += num(b->Weight(i, j)) + " ";
      }
    d += "0 " + discontinuities(b->UKnots(), b->UMultiplicities(), b->UDegree()) + discontinuities(b->VKnots(), b->VMultiplicities(), b->VDegree()) +
         "F 1 F 0 F 1 F 0 } I I I I ";
    return {add("spline-surface", d), [b](const gp_Pnt&, double u, double v) {
              gp_Pnt q;
              gp_Vec du, dv;
              b->D1(u, v, q, du, dv);
              return du.Crossed(dv);
            }};
  }

  // Curve of an edge (FORWARD) and its range.
  int curve(const TopoDS_Edge& e, double& f, double& l)
  {
    occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
    if (c.IsNull())
      fail_op("acis: an edge without a 3D curve");
    while (auto t = occ::down_cast<Geom_TrimmedCurve>(c))
      c = t->BasisCurve();
    if (auto ln = occ::down_cast<Geom_Line>(c))
      return add("straight-curve", vec(ln->Position().Location().XYZ()) + " " + vec(ln->Position().Direction().XYZ()) + " I I ");
    if (auto ci = occ::down_cast<Geom_Circle>(c))
    {
      const gp_Ax2 a = ci->Position();
      return add("ellipse-curve", vec(a.Location().XYZ()) + " " + vec(a.Direction().XYZ()) + " " + vec(a.XDirection().XYZ() * ci->Radius()) + " 1 I I ");
    }
    if (auto el = occ::down_cast<Geom_Ellipse>(c))
    {
      const gp_Ax2 a = el->Position();
      return add("ellipse-curve", vec(a.Location().XYZ()) + " " + vec(a.Direction().XYZ()) + " " + vec(a.XDirection().XYZ() * el->MajorRadius()) + " " +
                                    num(el->MinorRadius() / el->MajorRadius()) + " I I ");
    }
    occ::handle<Geom_BSplineCurve> b   = to_bspline(c, f, l);
    const bool                     rat = b->IsRational();
    std::string d = std::string("forward { exactcur full ") + (rat ? "nurbs " : "nubs ") + std::to_string(b->Degree()) + " open " + std::to_string(b->NbKnots()) + " " +
                    acis_knots(b->Knots(), b->Multiplicities(), b->Degree());
    for (int i = 1; i <= b->NbPoles(); ++i)
    {
      d += vec(b->Pole(i).XYZ()) + " ";
      if (rat)
        d += num(b->Weight(i)) + " ";
    }
    d += "0 null_surface null_surface nullbs nullbs I I " + discontinuities(b->Knots(), b->Multiplicities(), b->Degree()) + "F 1 F 0 } I I ";
    return add("intcurve-curve", d);
  }

  int body(const TopoDS_Shape& shape, int body_index)
  {
    const bool solid = TopExp_Explorer(shape, TopAbs_SOLID).More();
    // Lumps: a solid each (its shells), or one lump of the sheets' shells.
    std::vector<std::vector<TopoDS_Shape>> lumps;
    if (solid)
      for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next())
      {
        std::vector<TopoDS_Shape> shells;
        const TopoDS_Shell        outer = BRepClass3d::OuterShell(TopoDS::Solid(ex.Current()));
        if (!outer.IsNull())
          shells.push_back(outer);
        for (TopExp_Explorer es(ex.Current(), TopAbs_SHELL); es.More(); es.Next())
          if (outer.IsNull() || !es.Current().IsSame(outer))
            shells.push_back(es.Current());
        lumps.push_back(shells);
      }
    else
    {
      std::vector<TopoDS_Shape> shells;
      for (TopExp_Explorer es(shape, TopAbs_SHELL); es.More(); es.Next())
        shells.push_back(es.Current());
      for (TopExp_Explorer ef(shape, TopAbs_FACE, TopAbs_SHELL); ef.More(); ef.Next())
        shells.push_back(ef.Current());
      lumps.push_back(shells);
    }
    if (lumps.empty() || lumps[0].empty())
      fail_invalid("acis: the shape has no faces");

    ShapeIMap                     edges, vertices;
    std::map<int, int>            edge_rec, vertex_rec;   // OCCT index -> record
    std::map<int, std::vector<int>> edge_coedges;          // OCCT edge index -> coedge records
    std::map<int, bool>           coedge_forward;
    std::map<int, int>            coedge_edge;            // coedge record -> OCCT edge index
    std::vector<int>              lump_recs;
    for (auto& shells : lumps)
    {
      const int        lump = reserve();
      std::vector<int> shell_recs;
      for (auto& sh : shells)
      {
        const int        shell = reserve();
        std::vector<int> face_recs;
        std::vector<std::pair<TopoDS_Face, int>> faces;
        for (TopExp_Explorer ef(sh, TopAbs_FACE); ef.More(); ef.Next())
          faces.emplace_back(TopoDS::Face(ef.Current()), reserve());
        for (size_t fi = 0; fi < faces.size(); ++fi)
        {
          const TopoDS_Face& F    = faces[fi].first;
          const Surf         surf = surface(F);
          // Face sense: the (outward) face normal against the ACIS surface normal.
          double u0, u1, v0, v1;
          BRepTools::UVBounds(F, u0, u1, v0, v1);
          BRepAdaptor_Surface ad(F);
          double              agree = 0;
          for (double t : {0.5, 0.37, 0.71, 0.13})
          {
            const double      u = u0 + (u1 - u0) * t, v = v0 + (v1 - v0) * t;
            BRepLProp_SLProps pr(ad, u, v, 1, 1e-12);
            if (!pr.IsNormalDefined())
              continue;
            gp_Vec n = pr.Normal();
            if (F.Orientation() == TopAbs_REVERSED)
              n.Reverse();
            const gp_Vec m = surf.normal(pr.Value(), u, v);
            if (m.Magnitude() > 0 && std::abs(n.Dot(m.Normalized())) > 1e-3)
            {
              agree = n.Dot(m.Normalized());
              break;
            }
          }
          if (agree == 0)
            fail_op("acis: no surface normal on a face");
          // Loops of coedges.
          std::vector<int> loop_recs;
          for (const auto& cycle : exchange_loops(F))
          {
            const int        loop = reserve();
            std::vector<int> co;
            for (size_t k = 0; k < cycle.size(); ++k)
              co.push_back(reserve());
            for (size_t k = 0; k < cycle.size(); ++k)
            {
              TopoDS_Edge e = cycle[k];
              e.Orientation(TopAbs_FORWARD);
              const int ei = edges.Add(e);
              edge_coedges[ei].push_back(co[k]);
              coedge_forward[co[k]] = cycle[k].Orientation() == TopAbs_FORWARD;
              coedge_edge[co[k]]    = ei;
              // Filled in once partners and edges are known.
              records[co[k]] = std::to_string(loop) + " " + std::to_string(co[(k + 1) % co.size()]) + " " + std::to_string(co[(k + co.size() - 1) % co.size()]);
            }
            loop_recs.push_back(loop);
            records[loop] = std::to_string(co[0]) + " " + std::to_string(faces[fi].second);
          }
          for (size_t k = 0; k < loop_recs.size(); ++k)
          {
            const std::string rest = records[loop_recs[k]];
            const int         first = std::stoi(rest.substr(0, rest.find(' ')));
            set(loop_recs[k], "loop", ptr(k + 1 < loop_recs.size() ? loop_recs[k + 1] : -1) + " " + ptr(first) + " " + ptr(faces[fi].second) + " ");
          }
          set(faces[fi].second, "face",
              ptr(fi + 1 < faces.size() ? faces[fi + 1].second : -1) + " " + ptr(loop_recs.empty() ? -1 : loop_recs[0]) + " " + ptr(shell) + " $-1 " + ptr(surf.index) + " " +
                (agree > 0 ? "forward" : "reversed") + (solid ? " single " : " double out "));
        }
        shell_recs.push_back(shell);
        records[shell] = std::to_string(faces.empty() ? -1 : faces[0].second);
      }
      for (size_t k = 0; k < shell_recs.size(); ++k)
        set(shell_recs[k], "shell", ptr(k + 1 < shell_recs.size() ? shell_recs[k + 1] : -1) + " $-1 " + ptr(std::stoi(records[shell_recs[k]])) + " $-1 " + ptr(lump) + " ");
      lump_recs.push_back(lump);
      records[lump] = std::to_string(shell_recs[0]);
    }
    for (size_t k = 0; k < lump_recs.size(); ++k)
      set(lump_recs[k], "lump", ptr(k + 1 < lump_recs.size() ? lump_recs[k + 1] : -1) + " " + ptr(std::stoi(records[lump_recs[k]])) + " " + ptr(body_index) + " ");
    set(body_index, "body", ptr(lump_recs[0]) + " $-1 $-1 ");

    // Edges, vertices, points; then the coedges.
    for (int ei = 1; ei <= edges.Extent(); ++ei)
    {
      const TopoDS_Edge& e = TopoDS::Edge(edges(ei));
      const int          er = reserve();
      edge_rec[ei]          = er;
      int vr[2];
      for (int end = 0; end < 2; ++end)
      {
        const TopoDS_Vertex v  = end ? TopExp::LastVertex(e) : TopExp::FirstVertex(e);
        const int           vi = vertices.Add(v);
        if (!vertex_rec.count(vi))
        {
          const int rec  = reserve();
          vertex_rec[vi] = rec;
          const int pt   = add("point", vec(BRep_Tool::Pnt(v).XYZ()) + " ");
          set(rec, "vertex", ptr(er) + " " + ptr(pt) + " ");
        }
        vr[end] = vertex_rec[vi];
      }
      double    f, l;
      const int c = curve(e, f, l);
      set(er, "edge", ptr(vr[0]) + " " + num(f) + " " + ptr(vr[1]) + " " + num(l) + " " + ptr(edge_coedges[ei][0]) + " " + ptr(c) + " forward @7 unknown ");
    }
    for (auto& [ei, co] : edge_coedges)
    {
      if (co.size() > 2)
        fail_unsupported("acis: non-manifold edge");
      for (size_t k = 0; k < co.size(); ++k)
      {
        const std::string r = records[co[k]];
        int               loop, next, prev;
        std::sscanf(r.c_str(), "%d %d %d", &loop, &next, &prev);
        const int partner = co.size() == 2 ? co[1 - k] : -1;
        set(co[k], "coedge", ptr(next) + " " + ptr(prev) + " " + ptr(partner) + " " + ptr(edge_rec[ei]) + " " + (coedge_forward[co[k]] ? "forward " : "reversed ") + ptr(loop) + " $-1 ");
      }
    }
    return body_index;
  }
};

std::string ctime_now()
{
  std::time_t t = std::time(nullptr);
  char        buf[32];
  std::strftime(buf, sizeof buf, "%a %b %d %H:%M:%S %Y", std::gmtime(&t));
  return buf;
}

} // namespace

void write_acis(const OcShapeList& shapes, rust::Str path)
{
  Sat              sat;
  std::vector<int> bodies;
  for (size_t i = 0; i < shapes.items.size(); ++i)
    bodies.push_back(sat.reserve()); // bodies are the first records
  for (size_t i = 0; i < shapes.items.size(); ++i)
  {
    require_non_null(shapes.items[i], "shape");
    sat.body(exchange_prepared(shapes.items[i]), bodies[i]);
  }
  const std::string product = "noshape", version = "ACIS 7.0 NT", date = ctime_now();
  std::string       out = "700 0 " + std::to_string(bodies.size()) + " 0\n";
  out += "@" + std::to_string(product.size()) + " " + product + " @" + std::to_string(version.size()) + " " + version + " @" + std::to_string(date.size()) + " " + date + "\n";
  out += "1 9.9999999999999995e-07 1e-10\n";
  for (const auto& r : sat.records)
    out += r + "\n";
  out += "End-of-ACIS-data\n";
  std::ofstream f(std::string(path), std::ios::binary);
  if (!f)
    fail_invalid("cannot write " + std::string(path));
  f.write(out.data(), (std::streamsize)out.size());
  if (!f)
    fail_op("writing " + std::string(path) + " failed");
}

} // namespace noshape
