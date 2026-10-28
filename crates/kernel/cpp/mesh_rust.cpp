// The display mesh of analytic faces by noshape's own mesher (crates/brep `mesh`, through `brep_mesh`): the
// faces it takes get their triangulation and their edges' polygons here, as BRepMesh would leave them, and
// BRepMesh meshes only the rest (mesh.cpp). NOSHAPE_MESH_RUST=0: off.
#include "internal.hpp"

#include <BRepMesh_ShapeTool.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Geom_Circle.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangulation.hxx>
#include <TopoDS_Iterator.hxx>

#include <cstdlib>
#include <string>
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

void push3(rust::Vec<double>& v, const gp_XYZ& p)
{
  v.push_back(p.X());
  v.push_back(p.Y());
  v.push_back(p.Z());
}

void push0(rust::Vec<double>& v, int n)
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
  return on;
}

int rust_mesh_faces(const ShapeIMap& faces, const ShapeIMap& edges, const ShapeIMap& vertices, const std::vector<char>& fresh, double lin, double ang,
                    std::vector<char>& taken)
{
  const KernelTimer timer("mesh.rust");
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
    const occ::handle<Geom_Plane>   pl = s.IsNull() || is_mesh_face(f) ? occ::handle<Geom_Plane>() : occ::down_cast<Geom_Plane>(s);
    bool                            ok = !pl.IsNull() && (f.Orientation() == TopAbs_FORWARD || f.Orientation() == TopAbs_REVERSED);
    std::vector<uint32_t>           uses, ends;
    std::vector<int>&               ue = use_edges[(size_t)i];
    for (TopoDS_Iterator wi(f); wi.More() && ok; wi.Next())
    {
      ok = wi.Value().ShapeType() == TopAbs_WIRE;
      for (TopoDS_Iterator ei(wi.Value()); ei.More() && ok; ei.Next())
      {
        const int k = edges.FindIndex(ei.Value());
        ok          = k > 0 && (ei.Value().Orientation() == TopAbs_FORWARD || ei.Value().Orientation() == TopAbs_REVERSED);
        if (ok)
        {
          uses.push_back(static_cast<uint32_t>(2 * (k - 1) + (ei.Value().Orientation() == TopAbs_REVERSED ? 1 : 0)));
          ue.push_back(k);
        }
      }
      ends.push_back(static_cast<uint32_t>(uses.size()));
    }
    if (ok)
    {
      const gp_Ax3 a = pl->Position().Transformed(loc.Transformation());
      in.surfaces.push_back(0);
      push3(in.surfaces, a.Location().XYZ());
      push3(in.surfaces, a.XDirection().Crossed(a.YDirection()).XYZ());
      push3(in.surfaces, a.XDirection().XYZ());
      push0(in.surfaces, 3);
      const uint32_t base = static_cast<uint32_t>(in.uses.size());
      for (uint32_t u : uses)
        in.uses.push_back(u);
      for (uint32_t e : ends)
        in.loop_uses.push_back(base + e);
    }
    else
    {
      // Not meshed here: no loops.
      in.surfaces.push_back(255);
      push0(in.surfaces, 12);
    }
    in.face_loops.push_back(static_cast<uint32_t>(in.loop_uses.size() - 1));
    face_of.push_back(i);
  }
  if (face_of.empty())
    return 0;
  MeshOut out;
  if (!brep_mesh(in, out))
    return 0;
  // Per meshed face: its triangulation (in the face's own frame), its edges' polygons.
  std::vector<occ::handle<Poly_Triangulation>> tris(faces.Extent() + 1);
  std::vector<size_t>                          use_at(faces.Extent() + 1, 0); // its first entry in out.use_nodes
  BRep_Builder                                 B;
  int                                          n = 0;
  size_t                                       un = 0;
  for (size_t j = 0; j < face_of.size(); ++j)
  {
    const int i = face_of[j];
    if (!out.face_ok[j])
      continue; // no use_nodes for a declined face
    const TopoDS_Face& f   = TopoDS::Face(faces(i));
    const gp_Trsf      inv = f.Location().Transformation().Inverted();
    const uint32_t     n0 = out.face_nodes[j], n1 = out.face_nodes[j + 1], t0 = out.face_tris[j], t1 = out.face_tris[j + 1];
    occ::handle<Poly_Triangulation> tri = new Poly_Triangulation((int)(n1 - n0), (int)(t1 - t0), false);
    tri->AddNormals();
    for (uint32_t k = n0; k < n1; ++k)
    {
      gp_Pnt p(out.nodes[3 * k], out.nodes[3 * k + 1], out.nodes[3 * k + 2]);
      gp_Dir d(out.normals[3 * k], out.normals[3 * k + 1], out.normals[3 * k + 2]);
      if (!f.Location().IsIdentity())
      {
        p.Transform(inv);
        d.Transform(inv);
      }
      tri->SetNode((int)(k - n0 + 1), p);
      tri->SetNormal((int)(k - n0 + 1), d);
    }
    for (uint32_t t = t0; t < t1; ++t)
      tri->SetTriangle((int)(t - t0 + 1), Poly_Triangle((int)out.tris[3 * t] + 1, (int)out.tris[3 * t + 1] + 1, (int)out.tris[3 * t + 2] + 1));
    tri->Deflection(lin);
    B.UpdateFace(f, tri);
    tris[(size_t)i]   = tri;
    use_at[(size_t)i] = un;
    // Skip to the next face's uses: per use a count and that many node indices.
    for (uint32_t l = in.face_loops[j]; l < in.face_loops[j + 1]; ++l)
      for (uint32_t u = in.loop_uses[l]; u < in.loop_uses[l + 1]; ++u)
        un += 1 + out.use_nodes[un];
    taken[(size_t)i] = 1;
    ++n;
  }
  // Polygons of every taken face's edges (a pattern's instances with the first's nodes), as BRepMesh's
  // post-processing leaves them: the edge's nodes from its start to its end, their parameters.
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    if (!fresh[(size_t)i] || first[(size_t)i] == 0 || tris[(size_t)first[(size_t)i]].IsNull())
      continue;
    const int          i0  = first[(size_t)i];
    const TopoDS_Face& f   = TopoDS::Face(faces(i));
    size_t             at  = use_at[(size_t)i0];
    size_t             nuse = 0;
    if (i != i0)
    {
      // Same TFace: the triangulation is on it already.
      taken[(size_t)i] = 1;
      ++n;
    }
    for (TopoDS_Iterator wi(f); wi.More(); wi.Next())
      for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
      {
        const uint32_t count = out.use_nodes[at];
        // A pattern's instance: its edge's parameters are the first face's edge's at the same use.
        const int      ke    = use_edges[(size_t)i0][(size_t)(nuse++)];
        NCollection_Array1<int>    nodes(1, (int)count);
        NCollection_Array1<double> params(1, (int)count);
        const uint32_t             p0 = out.edge_params[(size_t)(ke - 1)];
        if (out.edge_params[(size_t)ke] - p0 != count)
        {
          at += 1 + count;
          continue;
        }
        for (uint32_t c = 0; c < count; ++c)
        {
          nodes((int)c + 1)  = (int)out.use_nodes[at + 1 + c] + 1;
          params((int)c + 1) = out.params[p0 + c];
        }
        occ::handle<Poly_PolygonOnTriangulation> poly = new Poly_PolygonOnTriangulation(nodes, params);
        poly->Deflection(lin);
        BRepMesh_ShapeTool::UpdateEdge(TopoDS::Edge(ei.Value()), poly, tris[(size_t)i0], f.Location());
        at += 1 + count;
      }
  }
  return n;
}

} // namespace noshape
