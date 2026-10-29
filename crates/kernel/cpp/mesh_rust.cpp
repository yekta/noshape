// The display mesh of analytic faces by noshape's own mesher (crates/brep `mesh`, through `brep_mesh`): the
// faces it takes get their triangulation and their edges' polygons here, as BRepMesh would leave them, and
// BRepMesh meshes only the rest (mesh.cpp). NOSHAPE_MESH_RUST=0: off.
#include "internal.hpp"

#include <BRepMesh_ShapeTool.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Geom_Circle.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangulation.hxx>
#include <TopoDS_Iterator.hxx>

#include <cstdlib>
#include <string>
#include <algorithm>
#include <tuple>
#include <unordered_map>

namespace noshape {

namespace {

occ::handle<Geom_Curve> basis(occ::handle<Geom_Curve> c)
{
  while (true)
  {
    const occ::handle<Geom_TrimmedCurve> t = occ::down_cast<Geom_TrimmedCurve>(c);
    if (t.IsNull())
      return c;
    c = t->BasisCurve();
  }
}

// The input of `brep_mesh` (its slices).
struct MeshIn
{
  std::vector<double>   vertices, curves, surfaces;
  std::vector<uint32_t> edges, face_loops, loop_uses, uses;
  double                linear = 0, angular = 0;
};

void push3(std::vector<double>& v, const gp_XYZ& p)
{
  v.push_back(p.X());
  v.push_back(p.Y());
  v.push_back(p.Z());
}

void push0(std::vector<double>& v, int n)
{
  for (int k = 0; k < n; ++k)
    v.push_back(0);
}

} // namespace

bool rust_mesh_on()
{
  static const bool on = [] {
    const char* v = std::getenv("NOSHAPE_MESH_RUST");
    return v == nullptr || std::string(v) != "0";
  }();
  return on && brep_mesh_on();
}

int rust_mesh_faces(const ShapeIMap& faces, const ShapeIMap& edges, const ShapeIMap& vertices, const std::vector<char>& fresh, double lin, double ang,
                    std::vector<char>& taken, std::vector<occ::handle<Poly_Triangulation>>& tris, std::vector<std::vector<std::array<float, 3>>>& polys)
{
  const KernelTimer timer("mesh.rust");
  auto              lap = std::make_unique<KernelTimer>("mesh.rust.in");
  MeshIn            in;
  in.linear  = lin;
  in.angular = ang;
  in.vertices.reserve(3 * (size_t)vertices.Extent());
  for (int i = 1; i <= vertices.Extent(); ++i)
    push3(in.vertices, BRep_Tool::Pnt(TopoDS::Vertex(vertices(i))).XYZ());
  // Edges: kind (0 line, 1 circle, 3 another curve or none), centre, normal, x direction, radius, parameter
  // range (13 numbers); start and end vertex.
  for (int i = 1; i <= edges.Extent(); ++i)
  {
    const TopoDS_Edge e = TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     v1, v2;
    TopExp::Vertices(e, v1, v2);
    const int a = v1.IsNull() ? 0 : vertices.FindIndex(v1), b = v2.IsNull() ? 0 : vertices.FindIndex(v2);
    in.edges.push_back(static_cast<uint32_t>(a > 0 ? a - 1 : 0));
    in.edges.push_back(static_cast<uint32_t>(b > 0 ? b - 1 : 0));
    TopLoc_Location               loc;
    double                        f = 0, l = 0;
    const occ::handle<Geom_Curve> c = BRep_Tool::Degenerated(e) || a <= 0 || b <= 0 ? occ::handle<Geom_Curve>() : BRep_Tool::Curve(e, loc, f, l);
    const occ::handle<Geom_Curve> bc = c.IsNull() ? c : basis(c);
    if (!bc.IsNull() && !occ::down_cast<Geom_Line>(bc).IsNull())
    {
      in.curves.push_back(0);
      push0(in.curves, 10);
    }
    else if (const occ::handle<Geom_Circle> circ = bc.IsNull() ? occ::handle<Geom_Circle>() : occ::down_cast<Geom_Circle>(bc); !circ.IsNull())
    {
      const gp_Circ g = circ->Circ().Transformed(loc.Transformation());
      in.curves.push_back(1);
      push3(in.curves, g.Location().XYZ());
      push3(in.curves, g.Axis().Direction().XYZ());
      push3(in.curves, g.XAxis().Direction().XYZ());
      in.curves.push_back(g.Radius());
    }
    else
    {
      in.curves.push_back(3);
      push0(in.curves, 10);
    }
    in.curves.push_back(f);
    in.curves.push_back(l);
  }
  // Faces: kind (0 plane, 255 not meshed here), origin, normal (the surface's own: x × y), x direction (9
  // numbers after the kind, 3 spare); their wires' edge uses. One face per TFace: the others (a pattern's
  // instances) take its triangulation.
  std::vector<int>                       first(faces.Extent() + 1, 0);
  std::vector<std::vector<int>>          use_edges(faces.Extent() + 1); // per first face: its uses' edges
  std::unordered_map<const void*, int>   by_tface;
  in.face_loops.push_back(0);
  in.loop_uses.push_back(0);
  std::vector<int> face_of; // input face -> faces index
  std::vector<int> input_of(faces.Extent() + 1, -1); // faces index -> input face
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    if (!fresh[(size_t)i])
      continue;
    const TopoDS_Face& f = TopoDS::Face(faces(i));
    if (auto it = by_tface.find(f.TShape().get()); it != by_tface.end())
    {
      first[(size_t)i] = it->second;
      continue;
    }
    by_tface.emplace(f.TShape().get(), i);
    first[(size_t)i] = i;
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> s  = BRep_Tool::Surface(f, loc);
    const bool                      mf = s.IsNull() || is_mesh_face(f);
    const occ::handle<Geom_Plane>   pl = mf ? occ::handle<Geom_Plane>() : occ::down_cast<Geom_Plane>(s);
    const occ::handle<Geom_CylindricalSurface> cyl = mf ? occ::handle<Geom_CylindricalSurface>() : occ::down_cast<Geom_CylindricalSurface>(s);
    const occ::handle<Geom_ConicalSurface>     con = mf ? occ::handle<Geom_ConicalSurface>() : occ::down_cast<Geom_ConicalSurface>(s);
    bool ok = (!pl.IsNull() || !cyl.IsNull() || !con.IsNull()) && (f.Orientation() == TopAbs_FORWARD || f.Orientation() == TopAbs_REVERSED);
    // Its loops, meshed here or not (a face BRepMesh meshes tells which edges it splits its own way).
    std::vector<int>& ue   = use_edges[(size_t)i];
    for (TopoDS_Iterator wi(f); wi.More(); wi.Next())
    {
      if (wi.Value().ShapeType() != TopAbs_WIRE)
      {
        ok = false;
        continue;
      }
      for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
      {
        const int k = edges.FindIndex(ei.Value());
        if (k <= 0 || (ei.Value().Orientation() != TopAbs_FORWARD && ei.Value().Orientation() != TopAbs_REVERSED))
        {
          ok = false;
          continue;
        }
        in.uses.push_back(static_cast<uint32_t>(2 * (k - 1) + (ei.Value().Orientation() == TopAbs_REVERSED ? 1 : 0)));
        ue.push_back(k);
      }
      in.loop_uses.push_back(static_cast<uint32_t>(in.uses.size()));
    }
    if (ok && !pl.IsNull())
    {
      const gp_Ax3 a = pl->Position().Transformed(loc.Transformation());
      in.surfaces.push_back(0);
      push3(in.surfaces, a.Location().XYZ());
      push3(in.surfaces, a.XDirection().Crossed(a.YDirection()).XYZ());
      push3(in.surfaces, a.XDirection().XYZ());
      push0(in.surfaces, 3);
    }
    else if (ok)
    {
      // Kind, origin, axis, x direction, radius, half-angle, whether the frame is right-handed.
      const gp_Ax3 a = (cyl.IsNull() ? con->Position() : cyl->Position()).Transformed(loc.Transformation());
      in.surfaces.push_back(cyl.IsNull() ? 2 : 1);
      push3(in.surfaces, a.Location().XYZ());
      push3(in.surfaces, a.Direction().XYZ());
      push3(in.surfaces, a.XDirection().XYZ());
      in.surfaces.push_back(cyl.IsNull() ? con->RefRadius() : cyl->Radius());
      in.surfaces.push_back(cyl.IsNull() ? con->SemiAngle() : 0.0);
      in.surfaces.push_back(a.Direct() ? 1.0 : 0.0);
    }
    else
    {
      // Meshed by BRepMesh.
      in.surfaces.push_back(255);
      push0(in.surfaces, 12);
    }
    in.face_loops.push_back(static_cast<uint32_t>(in.loop_uses.size() - 1));
    input_of[(size_t)i] = (int)face_of.size();
    face_of.push_back(i);
  }
  if (face_of.empty())
    return 0;
  MeshOut out;
  lap = std::make_unique<KernelTimer>("mesh.rust.mesh");
  auto f64s = [](const std::vector<double>& v) { return rust::Slice<const double>(v.data(), v.size()); };
  auto u32s = [](const std::vector<uint32_t>& v) { return rust::Slice<const uint32_t>(v.data(), v.size()); };
  if (!brep_mesh(f64s(in.vertices), u32s(in.edges), f64s(in.curves), f64s(in.surfaces), u32s(in.face_loops), u32s(in.loop_uses), u32s(in.uses), in.linear, in.angular, out))
    return 0;
  // The output through plain pointers (a rust::Vec's [] asks Rust for its data each time).
  const uint8_t*  face_ok    = out.face_ok.data();
  const uint32_t* face_nodes = out.face_nodes.data();
  const uint32_t* face_tris  = out.face_tris.data();
  const double*   nodes      = out.nodes.data();
  const double*   normals    = out.normals.data();
  const uint32_t* otris      = out.tris.data();
  const uint32_t* use_nodes  = out.use_nodes.data();
  lap = std::make_unique<KernelTimer>("mesh.rust.out");
  // Per meshed face: its triangulation (in the face's own frame); per edge of one: its polyline (in the
  // map's FORWARD direction, as mesh.cpp's edges loop wants it).
  std::vector<size_t> use_at(faces.Extent() + 1, 0); // its first entry in out.use_nodes
  int                 n  = 0;
  size_t              un = 0;
  for (size_t j = 0; j < face_of.size(); ++j)
  {
    const int i = face_of[j];
    if (!face_ok[j])
      continue; // no use_nodes for a declined face
    const TopoDS_Face& f   = TopoDS::Face(faces(i));
    const gp_Trsf      inv = f.Location().Transformation().Inverted();
    const uint32_t     n0 = face_nodes[j], n1 = face_nodes[j + 1], t0 = face_tris[j], t1 = face_tris[j + 1];
    occ::handle<Poly_Triangulation> tri = new Poly_Triangulation((int)(n1 - n0), (int)(t1 - t0), false);
    tri->AddNormals();
    for (uint32_t k = n0; k < n1; ++k)
    {
      gp_Pnt p(nodes[3 * k], nodes[3 * k + 1], nodes[3 * k + 2]);
      gp_Dir d(normals[3 * k], normals[3 * k + 1], normals[3 * k + 2]);
      if (!f.Location().IsIdentity())
      {
        p.Transform(inv);
        d.Transform(inv);
      }
      tri->SetNode((int)(k - n0 + 1), p);
      tri->SetNormal((int)(k - n0 + 1), d);
    }
    for (uint32_t t = t0; t < t1; ++t)
      tri->SetTriangle((int)(t - t0 + 1), Poly_Triangle((int)otris[3 * t] + 1, (int)otris[3 * t + 1] + 1, (int)otris[3 * t + 2] + 1));
    tri->Deflection(lin);
    tris[(size_t)i]   = tri;
    use_at[(size_t)i] = un;
    for (uint32_t l = in.face_loops[j]; l < in.face_loops[j + 1]; ++l)
      for (uint32_t u = in.loop_uses[l]; u < in.loop_uses[l + 1]; ++u)
        un += 1 + use_nodes[un];
  }
  // Edge polylines of the meshed faces' edges (a pattern's instances: the first's, moved), and the instances'
  // triangulations (the first's).
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    if (!fresh[(size_t)i] || first[(size_t)i] == 0 || tris[(size_t)first[(size_t)i]].IsNull())
      continue;
    const int          i0 = first[(size_t)i];
    const TopoDS_Face& f  = TopoDS::Face(faces(i));
    tris[(size_t)i]       = tris[(size_t)i0];
    taken[(size_t)i]      = 1;
    ++n;
    // From the first face's nodes (global) to this face's place.
    const bool    moved = i != i0 && !f.Location().IsEqual(faces(i0).Location());
    const gp_Trsf to    = moved ? f.Location().Transformation() * faces(i0).Location().Transformation().Inverted() : gp_Trsf();
    // The first face's j: its nodes start at face_nodes[j].
    const uint32_t nbase = face_nodes[(size_t)input_of[(size_t)i0]];
    size_t         at    = use_at[(size_t)i0];
    for (TopoDS_Iterator wi(f); wi.More(); wi.Next())
      for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
      {
        const uint32_t count = use_nodes[at];
        const int      k     = edges.FindIndex(ei.Value());
        if (k > 0 && polys[(size_t)k - 1].empty())
        {
          auto& poly = polys[(size_t)k - 1];
          poly.reserve(count);
          for (uint32_t c = 0; c < count; ++c)
          {
            const uint32_t node = nbase + use_nodes[at + 1 + c];
            gp_Pnt         p(nodes[3 * node], nodes[3 * node + 1], nodes[3 * node + 2]);
            if (moved)
              p.Transform(to);
            poly.push_back({(float)p.X(), (float)p.Y(), (float)p.Z()});
          }
        }
        at += 1 + count;
      }
  }
  return n;
}

} // namespace noshape
