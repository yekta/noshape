// PVZ (PTC Creo View) export, laid out like Onshape's (pvz_writer.cpp: the
// format). Each shape becomes a part: closed faces split in halves (as
// Onshape does), every face with its exact NURBS form (OCCT's NURBS
// conversion, pcurves in that parameter space, both normalised to [0, 1] as
// in Onshape's files) plus the analytic record for planes, cylinders and
// spheres; edges as NURBS curves over [0, 1] (points at poles); a per-face
// triangle mesh. Everything in metres.

#include "internal.hpp"
#include "pvz_reader.hpp"
#include "pvz_writer.hpp"

#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_NurbsConvert.hxx>
#include <BRepGProp.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BSplCLib.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomConvert.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <Poly_Triangulation.hxx>
#include <ShapeUpgrade_ShapeDivideClosed.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>

#include <algorithm>
#include <fstream>
#include <iterator>

namespace noshape {

namespace {

pvz::Vec3 v3(const gp_XYZ& p)
{
  return {p.X(), p.Y(), p.Z()};
}

// Knots and multiplicities of an OCCT array pair, mapped affinely from [a, b] to [0, 1].
template <class Knots, class Mults>
void unit_knots(const Knots& k, const Mults& m, double a, double b, std::vector<double>& ko, std::vector<int>& mo)
{
  for (int i = k.Lower(); i <= k.Upper(); ++i)
  {
    ko.push_back((k(i) - a) / (b - a));
    mo.push_back(m(i));
  }
}

pvz::NurbsCurve curve3d(occ::handle<Geom_BSplineCurve> c)
{
  if (c->IsPeriodic())
    c->SetNotPeriodic();
  pvz::NurbsCurve n;
  n.degree = c->Degree();
  unit_knots(c->Knots(), c->Multiplicities(), c->FirstParameter(), c->LastParameter(), n.knots, n.mults);
  for (int i = 1; i <= c->NbPoles(); ++i)
  {
    const gp_Pnt p = c->Pole(i);
    n.poles.insert(n.poles.end(), {p.X(), p.Y(), p.Z()});
    if (c->IsRational())
      n.weights.push_back(c->Weight(i));
  }
  return n;
}

struct Face
{
  occ::handle<Geom_BSplineSurface> s; // in global coordinates
  double                           u0, u1, v0, v1;
};

pvz::NurbsCurve pcurve(occ::handle<Geom2d_BSplineCurve> c, const Face& f)
{
  if (c->IsPeriodic())
    c->SetNotPeriodic();
  pvz::NurbsCurve n;
  n.degree = c->Degree();
  for (int i = c->FirstUKnotIndex(); i <= c->LastUKnotIndex(); ++i)
  {
    n.knots.push_back(c->Knot(i));
    n.mults.push_back(c->Multiplicity(i));
  }
  for (int i = 1; i <= c->NbPoles(); ++i)
  {
    const gp_Pnt2d p = c->Pole(i);
    n.poles.push_back((p.X() - f.u0) / (f.u1 - f.u0));
    n.poles.push_back((p.Y() - f.v0) / (f.v1 - f.v0));
    if (c->IsRational())
      n.weights.push_back(c->Weight(i));
  }
  return n;
}

pvz::NurbsSurface nurbs(const Face& f)
{
  const occ::handle<Geom_BSplineSurface>& s = f.s;
  pvz::NurbsSurface                       n;
  n.udegree = s->UDegree();
  n.vdegree = s->VDegree();
  unit_knots(s->UKnots(), s->UMultiplicities(), f.u0, f.u1, n.uknots, n.umults);
  unit_knots(s->VKnots(), s->VMultiplicities(), f.v0, f.v1, n.vknots, n.vmults);
  n.nu             = s->NbUPoles();
  n.nv             = s->NbVPoles();
  const bool rat   = s->IsURational() || s->IsVRational();
  for (int i = 1; i <= n.nu; ++i)
    for (int j = 1; j <= n.nv; ++j)
    {
      const gp_Pnt p = s->Pole(i, j);
      n.poles.insert(n.poles.end(), {p.X(), p.Y(), p.Z()});
      if (rat)
        n.weights.push_back(s->Weight(i, j));
    }
  return n;
}

// The analytic record of the original face's surface (plane, cylinder, sphere), else NURBS only.
pvz::Surface surface(const TopoDS_Face& original, const Face& f)
{
  TopLoc_Location           loc;
  occ::handle<Geom_Surface> s = BRep_Tool::Surface(original, loc);
  while (auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(s))
    s = t->BasisSurface();
  if (!loc.IsIdentity())
    s = occ::down_cast<Geom_Surface>(s->Transformed(loc.Transformation()));
  double u0, u1, v0, v1;
  BRepTools::UVBounds(original, u0, u1, v0, v1);
  if (auto p = occ::down_cast<Geom_Plane>(s))
  {
    const gp_Ax3 a = p->Position();
    return pvz::makePlane(v3(a.Location().XYZ()), v3(a.XDirection().XYZ()), v3(a.YDirection().XYZ()), {u0, u1}, {v0, v1}, nurbs(f));
  }
  if (auto c = occ::down_cast<Geom_CylindricalSurface>(s))
  {
    const gp_Ax3 a = c->Position();
    return pvz::makeCylinder(v3(a.Location().XYZ()), v3(a.XDirection().XYZ()), v3(a.YDirection().XYZ()), v3(a.Direction().XYZ()), c->Radius(), {u0, u1}, {v0, v1},
                             nurbs(f));
  }
  if (auto sp = occ::down_cast<Geom_SphericalSurface>(s))
  {
    const gp_Ax3 a = sp->Position();
    return pvz::makeSphere(v3(a.Location().XYZ()), v3(a.XDirection().XYZ()), v3(a.YDirection().XYZ()), v3(a.Direction().XYZ()), sp->Radius(), {u0, u1}, {v0, v1},
                           nurbs(f));
  }
  pvz::Surface out;
  out.kind  = pvz::SurfaceKind::Nurbs;
  out.nurbs = nurbs(f);
  return out;
}

pvz::Part pvz_part(const TopoDS_Shape& shape, const std::string& name)
{
  // Closed faces in halves (as Onshape writes them), on a copy (meshing must not touch the model).
  ShapeUpgrade_ShapeDivideClosed split(BRepBuilderAPI_Copy(shape, true, false).Shape());
  split.SetNbSplitPoints(1);
  split.Perform();
  const TopoDS_Shape          original = split.Result();
  BRepBuilderAPI_NurbsConvert conv(original, true);
  if (!conv.IsDone())
    fail_op("pvz: NURBS conversion failed");
  const TopoDS_Shape s = conv.Shape();
  Bnd_Box            box;
  BRepBndLib::Add(s, box);
  const double diag = box.IsVoid() ? 1.0 : std::sqrt(box.SquareExtent());
  BRepMesh_IncrementalMesh(s, 5e-4 * diag, false, 0.35, true);

  pvz::Part part;
  part.name = name.empty() ? "Part 1" : name;
  double xmin, ymin, zmin, xmax, ymax, zmax;
  box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
  part.bbox_min = {xmin, ymin, zmin};
  part.bbox_max = {xmax, ymax, zmax};
  GProp_GProps area;
  BRepGProp::SurfaceProperties(s, area);
  part.area_m2   = area.Mass();
  part.volume_m3 = volume_props(s).Mass();
  part.has_volume = TopExp_Explorer(s, TopAbs_SOLID).More();

  // Edges: every distinct edge, its curve over [0, 1] (a point at a pole).
  ShapeIMap edges;
  TopExp::MapShapes(s, TopAbs_EDGE, edges);
  for (int i = 1; i <= edges.Extent(); ++i)
  {
    TopoDS_Edge e = TopoDS::Edge(edges(i));
    e.Orientation(TopAbs_FORWARD);
    pvz::Edge pe;
    if (BRep_Tool::Degenerated(e))
    {
      pe.is_point = true;
      pe.point    = v3(BRep_Tool::Pnt(TopExp::FirstVertex(e)).XYZ());
    }
    else
    {
      double                  f, l;
      occ::handle<Geom_Curve> c = BRep_Tool::Curve(e, f, l);
      if (c.IsNull())
        fail_op("pvz: an edge without a 3D curve");
      pe.curve = curve3d(GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(c, f, l)));
    }
    part.edges.push_back(pe);
  }

  for (TopExp_Explorer ef(s, TopAbs_FACE); ef.More(); ef.Next())
  {
    const TopoDS_Face&        F = TopoDS::Face(ef.Current());
    TopLoc_Location           loc;
    occ::handle<Geom_Surface> gs = BRep_Tool::Surface(F, loc);
    while (auto t = occ::down_cast<Geom_RectangularTrimmedSurface>(gs))
      gs = t->BasisSurface();
    auto bs = occ::down_cast<Geom_BSplineSurface>(gs);
    if (bs.IsNull())
      fail_op("pvz: a face isn't a B-spline after conversion");
    Face f{occ::down_cast<Geom_BSplineSurface>(bs->Copy()), 0, 0, 0, 0};
    if (!loc.IsIdentity())
      f.s->Transform(loc.Transformation());
    if (f.s->IsUPeriodic())
      f.s->SetUNotPeriodic();
    if (f.s->IsVPeriodic())
      f.s->SetVNotPeriodic();
    f.s->Bounds(f.u0, f.u1, f.v0, f.v1);

    // The original (split, unconverted) face for the analytic record.
    TopoDS_Face original_face = F;
    for (TopExp_Explorer eo(original, TopAbs_FACE); eo.More(); eo.Next())
      if (conv.ModifiedShape(eo.Current()).IsSame(F))
        original_face = TopoDS::Face(eo.Current());

    pvz::Face pf;
    pf.surface    = surface(original_face, f);
    pf.same_sense = F.Orientation() != TopAbs_REVERSED;
    // Loops: the outer one first, coedges in loop order (as used in the face).
    const TopoDS_Wire        outer = BRepTools::OuterWire(F);
    std::vector<TopoDS_Wire> wires;
    if (!outer.IsNull())
      wires.push_back(outer);
    for (TopExp_Explorer ew(F, TopAbs_WIRE); ew.More(); ew.Next())
      if (outer.IsNull() || !ew.Current().IsSame(outer))
        wires.push_back(TopoDS::Wire(ew.Current()));
    for (const TopoDS_Wire& W : wires)
    {
      pvz::Loop loop;
      for (BRepTools_WireExplorer we(W, F); we.More(); we.Next())
      {
        const TopoDS_Edge& e = we.Current();
        TopoDS_Edge        key = e;
        key.Orientation(TopAbs_FORWARD);
        pvz::Coedge c;
        c.edge            = edges.FindIndex(key) - 1;
        c.forward         = e.Orientation() == TopAbs_FORWARD;
        c.pcurve_reversed = !c.forward;
        double                    pf_, pl_;
        occ::handle<Geom2d_Curve> pc = BRep_Tool::CurveOnSurface(e, F, pf_, pl_);
        if (pc.IsNull())
          fail_op("pvz: an edge without a curve on its face");
        c.pcurve.nurbs = pcurve(Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(pc, pf_, pl_)), f);
        loop.coedges.push_back(c);
      }
      if (!loop.coedges.empty())
        pf.loops.push_back(loop);
    }
    // Mesh: per-face vertices, outward normals, triangles counter-clockwise from outside.
    occ::handle<Poly_Triangulation> T = BRep_Tool::Triangulation(F, loc);
    if (!T.IsNull())
    {
      if (!T->HasNormals())
        BRepLib_ToolTriangulatedShape::ComputeNormals(F, T);
      const gp_Trsf tr  = loc.Transformation();
      const bool    rev = F.Orientation() == TopAbs_REVERSED;
      for (int i = 1; i <= T->NbNodes(); ++i)
      {
        const gp_Pnt p = T->Node(i).Transformed(tr);
        gp_Dir       d = T->Normal(i).Transformed(tr);
        if (rev)
          d.Reverse();
        pf.mesh.positions.insert(pf.mesh.positions.end(), {(float)p.X(), (float)p.Y(), (float)p.Z()});
        pf.mesh.normals.insert(pf.mesh.normals.end(), {(float)d.X(), (float)d.Y(), (float)d.Z()});
      }
      for (int t = 1; t <= T->NbTriangles(); ++t)
      {
        int n[3];
        T->Triangle(t).Get(n[0], n[1], n[2]);
        if (rev)
          std::swap(n[1], n[2]);
        for (int k : n)
          pf.mesh.triangles.push_back((uint32_t)(k - 1));
      }
    }
    part.faces.push_back(pf);
  }
  return part;
}

} // namespace

void write_pvz(const OcShapeList& shapes, rust::Slice<const rust::String> names, rust::Str path)
{
  std::vector<pvz::Part> parts;
  for (size_t i = 0; i < shapes.items.size(); ++i)
  {
    require_non_null(shapes.items[i], "shape");
    parts.push_back(pvz_part(shapes.items[i], i < names.size() ? std::string(names[i]) : std::string()));
  }
  std::string data;
  try
  {
    data = pvz::write(pvz::makePartStudioModel(parts, "noshape"));
  }
  catch (const std::invalid_argument& e)
  {
    fail_invalid(std::string("pvz: ") + e.what());
  }
  std::ofstream out(std::string(path), std::ios::binary);
  if (!out)
    fail_invalid("cannot write " + std::string(path));
  out.write(data.data(), (std::streamsize)data.size());
  if (!out)
    fail_op("writing " + std::string(path) + " failed");
}

// Onshape imports a PVZ as meshes only (live G22, G27): each part instance's
// tessellation, placed, nodes within Parasolid's precision (1e-8 m) welded,
// as mesh bodies (mesh_body.cpp): a compound per instance of its closed
// solid and open pieces (a loft whose faces' meshes leave gaps). Metres.
namespace
{
// A .pvz file's instances (and the product tree's root name).
std::vector<pvz::MeshInstance> pvz_instances(rust::Str path, std::string* root_name = nullptr)
{
  const std::string p(path.data(), path.size());
  std::ifstream     in(p, std::ios::binary);
  if (!in)
    fail_io("could not open " + p);
  const std::string bytes((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
  try
  {
    return pvz::read_meshes(bytes, root_name);
  }
  catch (const std::exception& e)
  {
    fail_io(std::string(e.what()) + " (" + p + ")");
  }
  return {};
}

// An instance's mesh bodies (placed when `place`); null without triangles.
TopoDS_Shape instance_shape(const pvz::MeshInstance& m, bool place)
{
  const std::array<double, 12> I{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0};
  const auto&         T = place ? m.placement : I;
  std::vector<gp_Pnt> nodes;
  for (size_t i = 0; i + 2 < m.positions.size(); i += 3)
  {
    const double x = m.positions[i], y = m.positions[i + 1], z = m.positions[i + 2];
    nodes.emplace_back(T[0] * x + T[1] * y + T[2] * z + T[3], T[4] * x + T[5] * y + T[6] * z + T[7], T[8] * x + T[9] * y + T[10] * z + T[11]);
  }
  std::vector<std::array<int, 3>> tris;
  for (size_t i = 0; i + 2 < m.triangles.size(); i += 3)
    tris.push_back({(int)m.triangles[i], (int)m.triangles[i + 1], (int)m.triangles[i + 2]});
  if (tris.empty())
    return {};
  return mesh_compound(mesh_bodies(nodes, tris, 1e-8));
}

// The number Creo View appends to a part file ("Torus_56.ol" -> 56; -1 if none).
long ol_number(const std::string& ol)
{
  const size_t dot = ol.rfind('.'), us = ol.rfind('_');
  if (us == std::string::npos || dot == std::string::npos || dot <= us + 1)
    return -1;
  const std::string n = ol.substr(us + 1, dot - us - 1);
  return n.find_first_not_of("0123456789") == std::string::npos ? std::atol(n.c_str()) : -1;
}

// The file's distinct parts (part files) in Onshape's order: by the part
// file's number, highest first (live qa5 zip import: pvz-shapes' Cylinder_57,
// Torus_56, Loft_55, Cone_54 come in as Cylinder, Torus, Cone + the Loft's
// composite), each with its first instance.
std::vector<const pvz::MeshInstance*> distinct_parts(const std::vector<pvz::MeshInstance>& all)
{
  std::vector<const pvz::MeshInstance*> parts;
  for (const auto& m : all)
    if (std::none_of(parts.begin(), parts.end(), [&](const pvz::MeshInstance* p) { return p->ol_file == m.ol_file; }))
      parts.push_back(&m);
  std::stable_sort(parts.begin(), parts.end(), [](const pvz::MeshInstance* a, const pvz::MeshInstance* b) { return ol_number(a->ol_file) > ol_number(b->ol_file); });
  return parts;
}
} // namespace

std::unique_ptr<OcShape> read_pvz(rust::Str path)
{
  const std::vector<pvz::MeshInstance> meshes = pvz_instances(path);
  BRep_Builder    b;
  TopoDS_Compound c;
  b.MakeCompound(c);
  int n = 0;
  for (const auto& m : meshes)
  {
    const TopoDS_Shape s = instance_shape(m, true);
    if (s.IsNull())
      continue;
    b.Add(c, s);
    ++n;
  }
  if (n == 0)
    fail_io("no meshes in " + std::string(path.data(), path.size()));
  return wrap(c);
}

StepImport read_pvz_parts(rust::Str path)
{
  const std::vector<pvz::MeshInstance> all = pvz_instances(path);
  StepImport out;
  out.shapes = std::make_unique<OcShapeList>();
  for (const pvz::MeshInstance* m : distinct_parts(all))
  {
    const TopoDS_Shape s = instance_shape(*m, false);
    if (s.IsNull())
      continue;
    out.shapes->items.push_back(s);
    out.names.push_back(rust::String(m->name));
    for (int k = 0; k < 3; ++k)
      out.colors.push_back(0);
    out.has_color.push_back(false);
    out.opacity.push_back(1);
  }
  if (out.shapes->items.empty())
    fail_io("no meshes in " + std::string(path.data(), path.size()));
  return out;
}

rust::Vec<PvzInstance> read_pvz_instances(rust::Str path)
{
  std::string                          root;
  const std::vector<pvz::MeshInstance> all   = pvz_instances(path, &root);
  const auto                           parts = distinct_parts(all);
  rust::Vec<PvzInstance>               out;
  // Index of each part among the parts read_pvz_parts returns (meshless ones dropped).
  std::vector<std::string> kept;
  for (const pvz::MeshInstance* p : parts)
    if (!p->triangles.empty())
      kept.push_back(p->ol_file);
  for (const auto& m : all)
  {
    const auto it = std::find(kept.begin(), kept.end(), m.ol_file);
    if (it == kept.end())
      continue;
    PvzInstance f;
    f.part = (uint32_t)(it - kept.begin());
    f.name = rust::String(m.name);
    f.root = rust::String(root);
    for (double v : m.placement)
      f.transform.push_back(v);
    out.push_back(std::move(f));
  }
  return out;
}

} // namespace noshape
