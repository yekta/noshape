// Tessellation: one BRepMesh call, then flat buffers filled in a single pass.
#include "internal.hpp"

#include <BRepAdaptor_Curve.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_Context.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <IMeshTools_Parameters.hxx>
#include <Precision.hxx>

#include <BRep_Tool.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <Poly_Polygon3D.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangle.hxx>
#include <Poly_Triangulation.hxx>
#include <TopLoc_Location.hxx>

#include <algorithm>

namespace noshape {

namespace {
struct FaceTri
{
  occ::handle<Poly_Triangulation> tri;
  gp_Trsf                         trsf;
  bool                            identity = true;
  bool                            reversed = false;
};
} // namespace

struct OcMeshFaces
{
  std::vector<FaceTri> faces;
};

std::unique_ptr<OcMesh> mesh_prepare(const OcShape& s, double lin, double ang, bool relative,
                                     bool parallel)
{
  require_non_null(s.shape, "shape");
  if (!(lin > 0) || !std::isfinite(lin))
    fail_invalid("linear deflection must be positive");
  if (!(ang > 0) || !std::isfinite(ang))
    fail_invalid("angular deflection must be positive");
  auto m   = std::make_unique<OcMesh>();
  m->shape = s.shape;
  TopExp::MapShapes(s.shape, TopAbs_FACE, m->faces);
  TopExp::MapShapes(s.shape, TopAbs_EDGE, m->edges);
  TopExp::MapShapes(s.shape, TopAbs_VERTEX, m->vertices);
  auto side = std::make_shared<OcMeshFaces>();
  side->faces.resize(m->faces.Extent());

  ShapeAncMap edgeFaces;
  TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, edgeFaces);
  {
    std::lock_guard<std::mutex> lock(mesh_mutex());
    IMeshTools_Parameters prm;
    prm.Deflection               = lin;
    prm.Angle                    = ang;
    prm.Relative                 = relative;
    prm.InParallel               = parallel;
    prm.MinSize                  = Precision::Confusion();
    // Faces whose stored triangulation already satisfies the deflection are
    // skipped by BRepMesh (triangulations are reused).
    // Every surface through BRepMesh's own algorithms. Not Delabella for
    // planar faces: it dereferences null on some of them on macOS arm64
    // (features_a pattern_instance_references_survive_upstream_edit,
    // v0.4.6 CI run 39827462890).
    occ::handle<BRepMesh_Context> ctx = new BRepMesh_Context();
    BRepMesh_IncrementalMesh mesher;
    mesher.SetShape(s.shape);
    mesher.ChangeParameters() = prm;
    mesher.Perform(ctx);
    for (int i = 1; i <= m->faces.Extent(); ++i)
    {
      const TopoDS_Face& f = TopoDS::Face(m->faces(i));
      TopLoc_Location    loc;
      const occ::handle<Poly_Triangulation>& tri = BRep_Tool::Triangulation(f, loc);
      FaceTri&                               ft  = side->faces[i - 1];
      ft.reversed                                = f.Orientation() == TopAbs_REVERSED;
      if (tri.IsNull())
        continue;
      if (is_mesh_face(f))
      {
        // A mesh body's face: drawn faceted (a normal per triangle), like Onshape's mesh parts.
        occ::handle<Poly_Triangulation> flat = new Poly_Triangulation(3 * tri->NbTriangles(), tri->NbTriangles(), false);
        flat->AddNormals();
        for (int k = 1; k <= tri->NbTriangles(); ++k)
        {
          int a, b, c;
          tri->Triangle(k).Get(a, b, c);
          const gp_Pnt pa = tri->Node(a), pb = tri->Node(b), pc = tri->Node(c);
          gp_Vec       n  = gp_Vec(pa, pb).Crossed(gp_Vec(pa, pc));
          if (n.Magnitude() > 0)
            n.Normalize();
          for (int j = 0; j < 3; ++j)
          {
            flat->SetNode(3 * k - 2 + j, j == 0 ? pa : j == 1 ? pb : pc);
            flat->SetNormal(3 * k - 2 + j, gp_Dir(n.Magnitude() > 0 ? n : gp_Vec(0, 0, 1)));
          }
          flat->SetTriangle(k, Poly_Triangle(3 * k - 2, 3 * k - 1, 3 * k));
        }
        ft.tri      = flat;
        ft.identity = loc.IsIdentity();
        if (!ft.identity)
          ft.trsf = loc.Transformation();
        m->nodes += flat->NbNodes();
        m->indices += 3 * (size_t)flat->NbTriangles();
        continue;
      }
      if (!tri->HasNormals())
        BRepLib_ToolTriangulatedShape::ComputeNormals(f, tri);
      ft.tri      = tri;
      ft.identity = loc.IsIdentity();
      if (!ft.identity)
        ft.trsf = loc.Transformation();
      m->nodes += tri->NbNodes();
      m->indices += 3 * (size_t)tri->NbTriangles();
    }

    // Edge polylines (inside the lock: reads polygons stored on edges).
    m->edge_polys.resize(m->edges.Extent());
    for (int i = 1; i <= m->edges.Extent(); ++i)
    {
      const TopoDS_Edge& e    = TopoDS::Edge(m->edges(i));
      auto&              poly = m->edge_polys[i - 1];
      if (BRep_Tool::Degenerated(e))
        continue;
      bool                                done = false;
      const NCollection_List<TopoDS_Shape>* fl   = edgeFaces.Seek(e);
      if (fl != nullptr)
      {
        for (NCollection_List<TopoDS_Shape>::Iterator it(*fl); it.More() && !done; it.Next())
        {
          const TopoDS_Face& f = TopoDS::Face(it.Value());
          TopLoc_Location    floc;
          const occ::handle<Poly_Triangulation>& tri = BRep_Tool::Triangulation(f, floc);
          if (tri.IsNull())
            continue;
          TopLoc_Location eloc;
          const occ::handle<Poly_PolygonOnTriangulation>& pot =
            BRep_Tool::PolygonOnTriangulation(e, tri, eloc);
          if (pot.IsNull())
            continue;
          const bool ident = eloc.IsIdentity();
          gp_Trsf    t     = ident ? gp_Trsf() : eloc.Transformation();
          poly.reserve(pot->NbNodes());
          for (int k = 1; k <= pot->NbNodes(); ++k)
          {
            const int ni = pot->Node(k);
            if (ni < 1 || ni > tri->NbNodes())
              continue;
            gp_Pnt p = tri->Node(ni);
            if (!ident)
              p.Transform(t);
            poly.push_back({(float)p.X(), (float)p.Y(), (float)p.Z()});
          }
          done = !poly.empty();
        }
      }
      if (!done)
      {
        TopLoc_Location                    l3;
        const occ::handle<Poly_Polygon3D>& p3 = BRep_Tool::Polygon3D(e, l3);
        if (!p3.IsNull())
        {
          gp_Trsf t = l3.IsIdentity() ? gp_Trsf() : l3.Transformation();
          for (int k = p3->Nodes().Lower(); k <= p3->Nodes().Upper(); ++k)
          {
            gp_Pnt p = p3->Nodes().Value(k);
            if (!l3.IsIdentity())
              p.Transform(t);
            poly.push_back({(float)p.X(), (float)p.Y(), (float)p.Z()});
          }
          done = !poly.empty();
        }
      }
      if (!done)
      {
        try
        {
          BRepAdaptor_Curve           c(e);
          GCPnts_TangentialDeflection d(c, ang, relative ? lin * 0.01 : lin);
          for (int k = 1; k <= d.NbPoints(); ++k)
          {
            gp_Pnt p = d.Value(k);
            poly.push_back({(float)p.X(), (float)p.Y(), (float)p.Z()});
          }
        }
        catch (const Standard_Failure&)
        {
          poly.clear();
        }
      }
      if (e.Orientation() == TopAbs_REVERSED)
        std::reverse(poly.begin(), poly.end());
      m->edge_points += poly.size();
    }
  }
  m->faceData = side;
  return m;
}

MeshCounts mesh_counts(const OcMesh& m)
{
  MeshCounts c{};
  c.nodes       = m.nodes;
  c.indices     = m.indices;
  c.faces       = (size_t)m.faces.Extent();
  c.edge_points = m.edge_points;
  c.edges       = (size_t)m.edges.Extent();
  c.vertices    = (size_t)m.vertices.Extent();
  return c;
}

void mesh_fill(const OcMesh& m, rust::Slice<float> positions, rust::Slice<float> normals,
               rust::Slice<uint32_t> indices, rust::Slice<uint32_t> face_ranges,
               rust::Slice<float> edge_points, rust::Slice<uint32_t> edge_ranges,
               rust::Slice<float> vertices)
{
  const std::shared_ptr<OcMeshFaces>& side = m.faceData;
  if (!side)
    fail_invalid("mesh has no face data");
  const size_t nf = (size_t)m.faces.Extent();
  if (positions.size() != 3 * m.nodes || normals.size() != 3 * m.nodes ||
      indices.size() != m.indices || face_ranges.size() != 4 * nf ||
      edge_points.size() != 3 * m.edge_points || edge_ranges.size() != 2 * m.edge_polys.size() ||
      vertices.size() != 3 * (size_t)m.vertices.Extent())
    fail_invalid("mesh buffer sizes do not match mesh counts");

  size_t vbase = 0, ibase = 0;
  for (size_t fi = 0; fi < nf; ++fi)
  {
    const FaceTri& ft = side->faces[fi];
    face_ranges[4 * fi + 0] = (uint32_t)ibase;
    face_ranges[4 * fi + 2] = (uint32_t)vbase;
    if (ft.tri.IsNull())
    {
      face_ranges[4 * fi + 1] = 0;
      face_ranges[4 * fi + 3] = 0;
      continue;
    }
    const Poly_Triangulation& tri = *ft.tri;
    const int                 nn  = tri.NbNodes();
    const int                 nt  = tri.NbTriangles();
    const bool                hasN = tri.HasNormals();
    const float               sgn  = ft.reversed ? -1.0f : 1.0f;
    for (int k = 1; k <= nn; ++k)
    {
      gp_Pnt p = tri.Node(k);
      if (!ft.identity)
        p.Transform(ft.trsf);
      float* pp = &positions[3 * (vbase + k - 1)];
      pp[0]     = (float)p.X();
      pp[1]     = (float)p.Y();
      pp[2]     = (float)p.Z();
      float* np = &normals[3 * (vbase + k - 1)];
      if (hasN)
      {
        NCollection_Vec3<float> n;
        tri.Normal(k, n);
        if (!ft.identity)
        {
          gp_Dir d(n.x(), n.y(), n.z());
          d.Transform(ft.trsf);
          n = NCollection_Vec3<float>((float)d.X(), (float)d.Y(), (float)d.Z());
        }
        float len = std::sqrt(n.x() * n.x() + n.y() * n.y() + n.z() * n.z());
        if (len > 0)
          len = 1.0f / len;
        np[0] = n.x() * len * sgn;
        np[1] = n.y() * len * sgn;
        np[2] = n.z() * len * sgn;
      }
      else
      {
        np[0] = 0;
        np[1] = 0;
        np[2] = 1;
      }
    }
    for (int t = 1; t <= nt; ++t)
    {
      int n1, n2, n3;
      tri.Triangle(t).Get(n1, n2, n3);
      if (ft.reversed)
        std::swap(n2, n3);
      uint32_t* ip = &indices[ibase + 3 * (t - 1)];
      ip[0]        = (uint32_t)(vbase + n1 - 1);
      ip[1]        = (uint32_t)(vbase + n2 - 1);
      ip[2]        = (uint32_t)(vbase + n3 - 1);
    }
    face_ranges[4 * fi + 1] = (uint32_t)(3 * nt);
    face_ranges[4 * fi + 3] = (uint32_t)nn;
    vbase += nn;
    ibase += 3 * (size_t)nt;
  }

  size_t ep = 0;
  for (size_t ei = 0; ei < m.edge_polys.size(); ++ei)
  {
    const auto& poly        = m.edge_polys[ei];
    edge_ranges[2 * ei + 0] = (uint32_t)ep;
    edge_ranges[2 * ei + 1] = (uint32_t)poly.size();
    for (const auto& p : poly)
    {
      edge_points[3 * ep + 0] = p[0];
      edge_points[3 * ep + 1] = p[1];
      edge_points[3 * ep + 2] = p[2];
      ++ep;
    }
  }
  for (int vi = 1; vi <= m.vertices.Extent(); ++vi)
  {
    gp_Pnt p                   = BRep_Tool::Pnt(TopoDS::Vertex(m.vertices(vi)));
    vertices[3 * (vi - 1) + 0] = (float)p.X();
    vertices[3 * (vi - 1) + 1] = (float)p.Y();
    vertices[3 * (vi - 1) + 2] = (float)p.Z();
  }
}

} // namespace noshape
