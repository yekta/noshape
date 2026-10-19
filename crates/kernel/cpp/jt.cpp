// JT export (JT 9.5, as Onshape writes it): per part a triangle mesh (one
// group per B-rep face, in the Parasolid face order JT requires) and the
// exact B-rep as an embedded Parasolid neutral-binary body. The file layout
// and encodings are in jt_writer.cpp. Shapes come in metres; the mesh is
// written in millimetres (the file's unit), the XT body in metres (Parasolid's).

#include "internal.hpp"
#include "jt_writer.hpp"

#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <Poly_Triangulation.hxx>
#include <TopoDS.hxx>

#include <cmath>
#include <fstream>
#include <map>

namespace noshape {

namespace {

jt::Part jt_part(const TopoDS_Shape& shape, const std::string& name)
{
  // A copy: meshing stores triangulations on the faces, which must not touch the model's.
  const TopoDS_Shape prepared = BRepBuilderAPI_Copy(exchange_prepared(shape), true, false).Shape();
  jt::Part           part;
  part.name = name.empty() ? "Part" : name;
  std::vector<TopoDS_Face> faces;
  part.xt         = parasolid_binary_body(prepared, faces);
  part.xt_version = {12, 0, 0}; // our transmit files: modeller version 1200000
  Bnd_Box box;
  BRepBndLib::Add(prepared, box);
  const double diag = box.IsVoid() ? 1.0 : std::sqrt(box.SquareExtent());
  BRepMesh_IncrementalMesh(prepared, 5e-4 * diag, false, 0.35, true);
  // Nodes shared along edges are merged (exact positions: BRepMesh shares edge nodes).
  std::map<std::array<double, 3>, int> index;
  auto vertex = [&](const gp_Pnt& p) {
    const std::array<double, 3> k{p.X() * 1000, p.Y() * 1000, p.Z() * 1000};
    auto it = index.find(k);
    if (it != index.end())
      return it->second;
    part.positions.push_back(k);
    return index[k] = (int)part.positions.size() - 1;
  };
  for (size_t fi = 0; fi < faces.size(); ++fi)
  {
    const TopoDS_Face&               F = faces[fi];
    TopLoc_Location                  loc;
    occ::handle<Poly_Triangulation> T = BRep_Tool::Triangulation(F, loc);
    if (T.IsNull())
      continue;
    if (!T->HasNormals())
      BRepLib_ToolTriangulatedShape::ComputeNormals(F, T);
    const gp_Trsf tr  = loc.Transformation();
    const bool    rev = F.Orientation() == TopAbs_REVERSED;
    std::vector<int> ids(T->NbNodes() + 1);
    for (int i = 1; i <= T->NbNodes(); ++i)
      ids[i] = vertex(T->Node(i).Transformed(tr));
    for (int t = 1; t <= T->NbTriangles(); ++t)
    {
      int n[3];
      T->Triangle(t).Get(n[0], n[1], n[2]);
      if (rev)
        std::swap(n[1], n[2]);
      const std::array<int, 3> tri{ids[n[0]], ids[n[1]], ids[n[2]]};
      if (tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2])
        continue; // collapsed by the merge (a pole)
      std::array<std::array<float, 3>, 3> normals;
      for (int k = 0; k < 3; ++k)
      {
        gp_Dir d = T->Normal(n[k]).Transformed(tr);
        if (rev)
          d.Reverse();
        normals[k] = {(float)d.X(), (float)d.Y(), (float)d.Z()};
      }
      part.tris.push_back(tri);
      part.groups.push_back((int)fi);
      part.corner_normals.push_back(normals);
    }
  }
  if (part.tris.empty())
    fail_invalid("jt: a part has no faces to mesh");
  return part;
}

} // namespace

void write_jt(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Str path)
{
  std::vector<jt::Part> parts;
  for (size_t i = 0; i < shapes.items.size(); ++i)
  {
    require_non_null(shapes.items[i], "shape");
    parts.push_back(jt_part(shapes.items[i], i < names.size() ? std::string(names[i]) : std::string()));
  }
  std::string data;
  try
  {
    data = jt::write(parts, std::string(path));
  }
  catch (const std::invalid_argument& e)
  {
    fail_invalid(std::string("jt: ") + e.what());
  }
  std::ofstream out(std::string(path), std::ios::binary);
  if (!out)
    fail_invalid("cannot write " + std::string(path));
  out.write(data.data(), (std::streamsize)data.size());
  if (!out)
    fail_op("writing " + std::string(path) + " failed");
}

} // namespace noshape
