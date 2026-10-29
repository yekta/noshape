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
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <OSD_Parallel.hxx>
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

void put3(double* v, const gp_XYZ& p)
{
  v[0] = p.X();
  v[1] = p.Y();
  v[2] = p.Z();
}

// f(i) for i in 1..=n, in runs of 64 on OCCT's pool when `parallel` and there are many (a small part's few
// hundred stay here: waking the pool cost more, up to 6 ms on a loaded machine for live-30's 150 edges).
template <class F>
void in_chunks(int n, bool parallel, const F& f)
{
  constexpr int run = 64;
  OSD_Parallel::For(
    0, (n + run - 1) / run,
    [&](int c) {
      for (int i = c * run + 1; i <= std::min(n, (c + 1) * run); ++i)
        f(i);
    },
    !parallel || n < 1024);
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
                    bool parallel, std::vector<char>& taken, std::vector<RustFace>& meshes, std::vector<std::vector<std::array<float, 3>>>& polys)
{
  const KernelTimer timer("mesh.rust");
  auto              lap = std::make_unique<KernelTimer>("mesh.rust.in");
  MeshIn            in;
  in.linear  = lin;
  in.angular = ang;
  auto              sub = std::make_unique<KernelTimer>("mesh.rust.in.vertices");
  in.vertices.reserve(3 * (size_t)vertices.Extent());
  for (int i = 1; i <= vertices.Extent(); ++i)
    push3(in.vertices, BRep_Tool::Pnt(TopoDS::Vertex(vertices(i))).XYZ());
  // Edges: kind (0 line, 1 circle, 3 another curve or none), centre, normal, x direction, radius, parameter
  // range (13 numbers); start and end vertex. Each edge's own slots, read in parallel (OCCT's pool, as
  // BRepMesh's InParallel under the same lock).
  sub = std::make_unique<KernelTimer>("mesh.rust.in.edges");
  const int ne = edges.Extent();
  in.edges.assign(2 * (size_t)ne, 0);
  in.curves.assign(13 * (size_t)ne, 0.0);
  in_chunks(ne, parallel, [&](int i) {
    uint32_t*         ev = &in.edges[2 * (size_t)(i - 1)];
    double*           cv = &in.curves[13 * (size_t)(i - 1)];
    const TopoDS_Edge e  = TopoDS::Edge(edges(i).Oriented(TopAbs_FORWARD));
    TopoDS_Vertex     v1, v2;
    TopExp::Vertices(e, v1, v2);
    const int a = v1.IsNull() ? 0 : vertices.FindIndex(v1), b = v2.IsNull() ? 0 : vertices.FindIndex(v2);
    ev[0]       = static_cast<uint32_t>(a > 0 ? a - 1 : 0);
    ev[1]       = static_cast<uint32_t>(b > 0 ? b - 1 : 0);
    TopLoc_Location               loc;
    double                        f = 0, l = 0;
    const bool                    degenerated = BRep_Tool::Degenerated(e);
    const occ::handle<Geom_Curve> c  = degenerated || a <= 0 || b <= 0 ? occ::handle<Geom_Curve>() : BRep_Tool::Curve(e, loc, f, l);
    const occ::handle<Geom_Curve> bc = c.IsNull() ? c : basis(c);
    if (degenerated && a > 0 && a == b)
    {
      // An apex or pole: its parameters are its face's u.
      BRep_Tool::Range(e, f, l);
      cv[0] = 2;
    }
    else if (!bc.IsNull() && !occ::down_cast<Geom_Line>(bc).IsNull())
      cv[0] = 0;
    else if (const occ::handle<Geom_Circle> circ = bc.IsNull() ? occ::handle<Geom_Circle>() : occ::down_cast<Geom_Circle>(bc); !circ.IsNull())
    {
      const gp_Circ g = circ->Circ().Transformed(loc.Transformation());
      cv[0]           = 1;
      put3(cv + 1, g.Location().XYZ());
      put3(cv + 4, g.Axis().Direction().XYZ());
      put3(cv + 7, g.XAxis().Direction().XYZ());
      cv[10] = g.Radius();
    }
    else
      cv[0] = 3;
    cv[11] = f;
    cv[12] = l;
  });
  // Faces: kind (0 plane, 1 cylinder, 2 cone, 3 torus, 4 sphere, 255 not meshed here) and 12 numbers of geometry (below);
  // their wires' edge uses. One face per TFace: the others (a pattern's instances) take its triangulation.
  sub = std::make_unique<KernelTimer>("mesh.rust.in.faces");
  std::vector<int>                     first(faces.Extent() + 1, 0);
  std::unordered_map<const void*, int> by_tface;
  std::vector<int>                     face_of; // input face -> faces index
  std::vector<int>                     input_of(faces.Extent() + 1, -1); // faces index -> input face
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    if (!fresh[(size_t)i])
      continue;
    const auto [it, added] = by_tface.emplace(faces(i).TShape().get(), i);
    first[(size_t)i]       = it->second;
    if (added)
    {
      input_of[(size_t)i] = (int)face_of.size();
      face_of.push_back(i);
    }
  }
  // Each first face read on its own (in parallel), then joined in order.
  struct FaceIn
  {
    std::array<double, 13> surface{};
    std::vector<uint32_t>  uses, loop_ends;
  };
  std::vector<FaceIn> read(face_of.size());
  in_chunks((int)face_of.size(), parallel, [&](int j1) {
    FaceIn&                         r = read[(size_t)j1 - 1];
    const TopoDS_Face&              f = TopoDS::Face(faces(face_of[(size_t)j1 - 1]));
    TopLoc_Location                 loc;
    const occ::handle<Geom_Surface> s  = BRep_Tool::Surface(f, loc);
    const bool                      mf = s.IsNull() || is_mesh_face(f);
    const occ::handle<Geom_Plane>   pl = mf ? occ::handle<Geom_Plane>() : occ::down_cast<Geom_Plane>(s);
    const occ::handle<Geom_CylindricalSurface> cyl = mf ? occ::handle<Geom_CylindricalSurface>() : occ::down_cast<Geom_CylindricalSurface>(s);
    const occ::handle<Geom_ConicalSurface>     con = mf ? occ::handle<Geom_ConicalSurface>() : occ::down_cast<Geom_ConicalSurface>(s);
    const occ::handle<Geom_ToroidalSurface>    tor = mf ? occ::handle<Geom_ToroidalSurface>() : occ::down_cast<Geom_ToroidalSurface>(s);
    const occ::handle<Geom_SphericalSurface>   sph = mf ? occ::handle<Geom_SphericalSurface>() : occ::down_cast<Geom_SphericalSurface>(s);
    bool ok = (!pl.IsNull() || !cyl.IsNull() || !con.IsNull() || !tor.IsNull() || !sph.IsNull()) && (f.Orientation() == TopAbs_FORWARD || f.Orientation() == TopAbs_REVERSED);
    // Its loops, meshed here or not (a face BRepMesh meshes tells which edges it splits its own way).
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
        r.uses.push_back(static_cast<uint32_t>(2 * (k - 1) + (ei.Value().Orientation() == TopAbs_REVERSED ? 1 : 0)));
      }
      r.loop_ends.push_back(static_cast<uint32_t>(r.uses.size()));
    }
    double* sv = r.surface.data();
    if (ok && !pl.IsNull())
    {
      const gp_Ax3 a = pl->Position().Transformed(loc.Transformation());
      sv[0]          = 0;
      put3(sv + 1, a.Location().XYZ());
      put3(sv + 4, a.XDirection().Crossed(a.YDirection()).XYZ());
      put3(sv + 7, a.XDirection().XYZ());
    }
    else if (ok && !sph.IsNull())
    {
      // Kind, origin, axis, x direction, radius, (spare), whether the frame is right-handed.
      const gp_Ax3 a = sph->Position().Transformed(loc.Transformation());
      sv[0]          = 4;
      put3(sv + 1, a.Location().XYZ());
      put3(sv + 4, a.Direction().XYZ());
      put3(sv + 7, a.XDirection().XYZ());
      sv[10] = sph->Radius();
      sv[12] = a.Direct() ? 1.0 : 0.0;
    }
    else if (ok && !tor.IsNull())
    {
      // Kind, origin, axis, x direction, major and minor radius, whether the frame is right-handed.
      const gp_Ax3 a = tor->Position().Transformed(loc.Transformation());
      sv[0]          = 3;
      put3(sv + 1, a.Location().XYZ());
      put3(sv + 4, a.Direction().XYZ());
      put3(sv + 7, a.XDirection().XYZ());
      sv[10] = tor->MajorRadius();
      sv[11] = tor->MinorRadius();
      sv[12] = a.Direct() ? 1.0 : 0.0;
    }
    else if (ok)
    {
      // Kind, origin, axis, x direction, radius, half-angle, whether the frame is right-handed.
      const gp_Ax3 a = (cyl.IsNull() ? con->Position() : cyl->Position()).Transformed(loc.Transformation());
      sv[0]          = cyl.IsNull() ? 2 : 1;
      put3(sv + 1, a.Location().XYZ());
      put3(sv + 4, a.Direction().XYZ());
      put3(sv + 7, a.XDirection().XYZ());
      sv[10] = cyl.IsNull() ? con->RefRadius() : cyl->Radius();
      sv[11] = cyl.IsNull() ? con->SemiAngle() : 0.0;
      sv[12] = a.Direct() ? 1.0 : 0.0;
    }
    else
      sv[0] = 255; // meshed by BRepMesh
  });
  in.face_loops.push_back(0);
  in.loop_uses.push_back(0);
  for (const FaceIn& r : read)
  {
    const uint32_t u0 = (uint32_t)in.uses.size();
    in.uses.insert(in.uses.end(), r.uses.begin(), r.uses.end());
    for (const uint32_t e : r.loop_ends)
      in.loop_uses.push_back(u0 + e);
    in.surfaces.insert(in.surfaces.end(), r.surface.begin(), r.surface.end());
    in.face_loops.push_back(static_cast<uint32_t>(in.loop_uses.size() - 1));
  }
  if (face_of.empty())
    return 0;
  auto out = std::make_shared<MeshOut>();
  sub.reset();
  lap = std::make_unique<KernelTimer>("mesh.rust.mesh");
  auto f64s = [](const std::vector<double>& v) { return rust::Slice<const double>(v.data(), v.size()); };
  auto u32s = [](const std::vector<uint32_t>& v) { return rust::Slice<const uint32_t>(v.data(), v.size()); };
  if (!brep_mesh(f64s(in.vertices), u32s(in.edges), f64s(in.curves), f64s(in.surfaces), u32s(in.face_loops), u32s(in.loop_uses), u32s(in.uses), in.linear, in.angular, *out))
    return 0;
  // The output through plain pointers (a rust::Vec's [] asks Rust for its data each time).
  const uint8_t*  face_ok    = out->face_ok.data();
  const uint32_t* face_nodes = out->face_nodes.data();
  const uint32_t* face_tris  = out->face_tris.data();
  const double*   nodes      = out->nodes.data();
  const uint32_t* use_nodes  = out->use_nodes.data();
  lap = std::make_unique<KernelTimer>("mesh.rust.out");
  // Per meshed face: its ranges of the output; per edge of one: its polyline (in the map's FORWARD direction,
  // as mesh.cpp's edges loop wants it).
  std::vector<size_t> use_at(faces.Extent() + 1, 0); // its first entry in out.use_nodes
  int                 n  = 0;
  size_t              un = 0;
  for (size_t j = 0; j < face_of.size(); ++j)
  {
    const int i = face_of[j];
    if (!face_ok[j])
      continue; // no use_nodes for a declined face
    RustFace& r = meshes[(size_t)i];
    r.out       = out;
    r.n0        = face_nodes[j];
    r.n1        = face_nodes[j + 1];
    r.t0        = face_tris[j];
    r.t1        = face_tris[j + 1];
    use_at[(size_t)i] = un;
    for (uint32_t l = in.face_loops[j]; l < in.face_loops[j + 1]; ++l)
      for (uint32_t u = in.loop_uses[l]; u < in.loop_uses[l + 1]; ++u)
        un += 1 + use_nodes[un];
  }
  // Edge polylines of the meshed faces' edges (a pattern's instances: the first's, moved), and the instances'
  // meshes (the first's, moved).
  for (int i = 1; i <= faces.Extent(); ++i)
  {
    if (!fresh[(size_t)i] || first[(size_t)i] == 0 || !meshes[(size_t)first[(size_t)i]].out)
      continue;
    const int          i0 = first[(size_t)i];
    const TopoDS_Face& f  = TopoDS::Face(faces(i));
    taken[(size_t)i]      = 1;
    ++n;
    // From the first face's nodes (global) to this face's place.
    const bool    moved = i != i0 && !f.Location().IsEqual(faces(i0).Location());
    const gp_Trsf to    = moved ? f.Location().Transformation() * faces(i0).Location().Transformation().Inverted() : gp_Trsf();
    if (i != i0)
    {
      meshes[(size_t)i]       = meshes[(size_t)i0];
      meshes[(size_t)i].moved = moved;
      meshes[(size_t)i].to    = to;
    }
    // The first face's j: its nodes start at face_nodes[j].
    const size_t   j0    = (size_t)input_of[(size_t)i0];
    const uint32_t nbase = face_nodes[j0];
    size_t         at    = use_at[(size_t)i0];
    // Its uses' edges: the first face's read above, an instance's found in the map.
    std::vector<int> ks;
    if (i == i0)
      for (uint32_t u = in.loop_uses[in.face_loops[j0]]; u < in.loop_uses[in.face_loops[j0 + 1]]; ++u)
        ks.push_back((int)(in.uses[u] >> 1) + 1);
    else
      for (TopoDS_Iterator wi(f); wi.More(); wi.Next())
        for (TopoDS_Iterator ei(wi.Value()); ei.More(); ei.Next())
          ks.push_back(edges.FindIndex(ei.Value()));
    for (const int k : ks)
    {
      const uint32_t count = use_nodes[at];
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
