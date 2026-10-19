// PTC Creo View PVZ writer (C++17, standard library only).
//
// Writes the layout produced by Onshape's "OnshapeCreoViewWriter 22.1" (format notes:
// /root/kern-docs/pvz/format-notes.md).  A .pvz is a zip holding
//   <id>.pvs            product structure (nodes, instances, transforms, properties)
//   <part>_<n>.ol       one per distinct part: header/colours, per-face triangle mesh,
//                       exact B-rep (analytic + NURBS surfaces, NURBS edges, pcurves)
// Both member types consist of a small binary header followed by gzip streams.
//
// Compression: every deflate stream (zip members and the inner gzip blocks) is written
// with STORED deflate blocks (BTYPE 00).  That is valid deflate for any inflater, but no
// Creo View acceptance test has been possible, see format-notes.md section "Writer".
//
// Units: metres everywhere (geometry, transforms, bbox), square/cubic metres for mass
// properties, radians for angles.  Integers in the files are little-endian.
//
// Everything that the format derives from topology is computed by the writer:
// record order, edge numbering (order of first use), coedge numbering, loop parents,
// COEDGE index slot, curve/pcurve stream order, the ID table, the PVS string table, the
// placement of the SHAPE bbox, CHILD record kinds and the appearance names.
#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace noshape {
namespace pvz {

using Vec3 = std::array<double, 3>;
using Vec2 = std::array<double, 2>;

// ---------------------------------------------------------------------------------
// NURBS (OpenCascade conventions: distinct knots + multiplicities, Euclidean poles,
// separate weights).  The file stores the flattened knot vector and homogeneous poles
// (x*w, y*w, [z*w,] w) as float32; order = degree + 1.
// ---------------------------------------------------------------------------------
struct NurbsCurve {
  int degree = 1;
  std::vector<double> knots;   // distinct knot values, strictly increasing
  std::vector<int> mults;      // multiplicity of each knot; sum(mults) == n_poles + degree + 1
  std::vector<double> poles;   // `dim` values per pole (dim = 3 for edge curves, 2 for pcurves)
  std::vector<double> weights; // one per pole, or empty = non-rational (rational flag 0, w = 1)
};

struct NurbsSurface {
  int udegree = 1, vdegree = 1;
  std::vector<double> uknots; std::vector<int> umults;
  std::vector<double> vknots; std::vector<int> vmults;
  int nu = 0, nv = 0;          // pole grid; sum(umults) == nu + udegree + 1, same for v
  std::vector<double> poles;   // 3 * nu * nv values, pole (i, j) at index (i * nv + j) * 3 (i along u)
  std::vector<double> weights; // nu * nv values (same order), or empty = non-rational
};

// ---------------------------------------------------------------------------------
// Surfaces.  Analytic surfaces are written with their analytic frame AND the NURBS form;
// kind Nurbs writes only the NURBS form (Onshape writes tori and splines this way; there
// is no analytic torus record in the samples, so pass tori as Nurbs).
//
// Analytic parametrisations (axes are NOT normalised: they carry the radii):
//   Plane    P(u,v) = O + u*A1 + v*A2               A1, A2 unit, A3 = unit normal
//   Cylinder P(u,v) = O + cos u*A1 + sin u*A2 + v*A3  |A1| = |A2| = R, A3 unit axis, u = angle
//   Cone     P(u,v) = O + u*(cos v*A1 + sin v*A2 + A3) O = apex, u in [0,1] along the generator,
//                                                      |A1| = |A2| = base radius, A3 = apex->base
//                                                      (axis * height), v = angle
//   Sphere   P(u,v) = O + cos v*(cos u*A1 + sin u*A2) + sin v*A3   |A1| = |A2| = |A3| = R
// urange/vrange are the face's analytic parameter ranges.
//
// The NURBS form must be the exact (or best) NURBS of the same face patch.  Onshape
// normalises its knot vectors to [0,1] for analytic faces; pcurves are expressed in the
// NURBS parameter space (verified on all samples).  For cones Onshape's NURBS runs
// (u' = angle, v' = generator), i.e. swapped w.r.t. the analytic (u, v): set
// nurbs_uv_swapped (written as surface byte 2 = 0, otherwise 1; inferred from 1 sample).
// Periodic faces: Onshape splits every closed face into two halves (separate faces with
// the same det_id); do the same (e.g. split cylinders at the seam into 0..pi, pi..2pi).
// ---------------------------------------------------------------------------------
enum class SurfaceKind : uint8_t { Cone = 1, Cylinder = 2, Nurbs = 4, Plane = 5, Sphere = 8 };

struct Surface {
  SurfaceKind kind = SurfaceKind::Nurbs;
  Vec3 origin{}, axis1{}, axis2{}, axis3{};          // analytic kinds only
  Vec2 urange{}, vrange{};                           // analytic kinds only
  bool nurbs_uv_swapped = false;                     // analytic kinds only (cone: true)
  NurbsSurface nurbs;                                // always required
  std::optional<Vec2> nurbs_interval;                // kind Nurbs only: extra parameter interval
                                                     // (Onshape loft: v knots end at 1.006, interval
                                                     // [0,1]); written as surface byte 2 = 1 + 2 f32
};

Surface makePlane(const Vec3& origin, const Vec3& xdir, const Vec3& ydir, Vec2 urange, Vec2 vrange,
                  NurbsSurface nurbs);
Surface makeCylinder(const Vec3& origin, const Vec3& xdir, const Vec3& ydir, const Vec3& axis, double radius,
                     Vec2 angle_range, Vec2 height_range, NurbsSurface nurbs);
Surface makeCone(const Vec3& apex, const Vec3& xdir, const Vec3& ydir, const Vec3& axis, double base_radius,
                 double height, Vec2 generator_range, Vec2 angle_range, NurbsSurface nurbs,
                 bool nurbs_uv_swapped = true);
Surface makeSphere(const Vec3& centre, const Vec3& xdir, const Vec3& ydir, const Vec3& zdir, double radius,
                   Vec2 longitude_range, Vec2 latitude_range, NurbsSurface nurbs);

// ---------------------------------------------------------------------------------
// Topology.  There are no vertex records in the format: vertices are implied by the end
// points of the edge curves.
// ---------------------------------------------------------------------------------
struct Pcurve {                 // curve in the NURBS parameter space of the face's surface
  bool is_line = false;         // true: 2D line segment record (Onshape uses it for degenerate pole edges)
  Vec2 start{}, end{};          // is_line only
  NurbsCurve nurbs;             // !is_line: dim = 2
};

struct Coedge {
  int edge = -1;                // index into Part::edges (the writer renumbers edges by first use)
  bool forward = true;          // true: coedge runs along the edge curve (start -> end)
  bool pcurve_reversed = false; // true: the pcurve runs opposite to the COEDGE direction
                                //   = (!forward) XOR (pcurve parametrised opposite to the 3D curve)
                                //   With OpenCascade pcurves (same parameter as the 3D curve):
                                //   pcurve_reversed = !forward.
  Pcurve pcurve;
};

struct Loop {                   // loops[0] of a face is the outer loop, the rest are holes
  std::vector<Coedge> coedges;  // in loop order; consecutive coedges must share end points
};

struct Edge {
  bool is_point = false;        // degenerate edge (e.g. sphere pole): a point instead of a curve
  Vec3 point{};                 // is_point only
  NurbsCurve curve;             // !is_point: dim = 3
  double t0 = 0.0, t1 = 1.0;    // EDGE parameter range (always 0..1 in Onshape files)
  std::string det_id;           // optional Onshape-style topology id (ID table), "" = none
};

// Per-face triangle mesh, metres, part coordinates.  Vertices are per face (not shared
// between faces).  Normals: unit, outward.  Triangles counter-clockwise seen from outside.
struct Mesh {
  std::vector<float> positions;   // x y z per vertex
  std::vector<float> normals;     // x y z per vertex
  std::vector<uint32_t> triangles;// 3 indices per triangle; written as one TRILIST record
  struct Primitive {
    uint32_t kind = 0x1b;         // 0x19 fan (i0,ik,ik+1), 0x1a strip (alternating), 0x1b list
    std::vector<uint32_t> indices;
  };
  std::vector<Primitive> primitives; // optional: written verbatim instead of `triangles`
};

struct Face {
  Surface surface;
  bool same_sense = true;       // true when the outward face normal equals the NURBS normal
                                // dS/du' x dS/dv' (verified on all samples incl. the cone)
  std::vector<Loop> loops;
  Mesh mesh;
  uint32_t material = 0;        // index into Part::materials (per-face colour)
  std::string det_id;           // optional topology id, "" = none
};

// MATERIAL record = 14 float32: ambient[3] diffuse[3] specular[3] emissive[3] f12 f13.
// Onshape writes ambient/specular/emissive 0, f12 = 0, f13 = 1 and puts the part colour in
// diffuse.  Onshape does NOT export transparency (the 50 % transparent sample is identical
// to an opaque one), so where opacity belongs is unknown; it is written into f13 only when
// Model::write_opacity is set (guess: f13 = opacity).
struct Material {
  std::array<float, 3> diffuse{0.615686f, 0.811765f, 0.929412f}; // Onshape default part colour
  std::array<float, 3> ambient{0.f, 0.f, 0.f};
  std::array<float, 3> specular{0.f, 0.f, 0.f};
  std::array<float, 3> emissive{0.f, 0.f, 0.f};
  float shininess = 0.f;          // f12 (meaning inferred)
  float opacity = 1.f;            // see above
  std::string appearance_name;    // "" = Onshape style "A<ambient hex>D<diffuse hex>" (e.g. A000000Dff0000)
};

struct Part {
  std::string name;               // used for the default .ol name
  std::string ol_name;            // zip member name; "" = "<name>_<54 + part index>.ol"
  std::vector<Material> materials;// [0] = body colour; empty = one default material
  bool has_area = true, has_volume = true;
  double area_m2 = 0.0;           // written as float32
  double volume_m3 = 0.0;         // written as float32
  Vec3 bbox_min{}, bbox_max{};    // part coordinates, written as float32 (OL BODY and PVS SHAPE)
  double angle_tolerance = 1.3089969389957472; // 75 deg, as Onshape
  double chord_tolerance = -1.0;  // < 0: 0.0005 * bbox diagonal (Onshape's value)
  std::string id_table_name = "BTI/DetId";
  std::string body_det_id;        // optional; ID table + PVS property "BTI/DetId" are up to the caller
  std::vector<Edge> edges;
  std::vector<Face> faces;
  bool write_brep = true;         // false: section 3 omitted (untested whether viewers accept that)
};

// ---------------------------------------------------------------------------------
// Product structure.  The tree is written post-order (children before parents, root
// last), one PVS node per Node.  A node either references a part (leaf) or has children.
// Onshape conventions (see make* helpers):
//   single part Part Studio : root = the part node, props documentId, BTI/DetId, Adapter_*
//   several parts           : root "RootAssembly" with one identity child per part (part
//                             props: BTI/DetId); root props documentId, Adapter_*
//   assembly                : root "<assembly name>_<n>", part instance nodes named after
//                             the part, sub-assembly nodes "<instance name, '<' '>' -> '_'>_<n>",
//                             instance props BTI/InstanceId = '.'-joined occurrence path
// ---------------------------------------------------------------------------------
struct Property { std::string key, value; };

// Row-major 4x4 (m[0..3] = first row).  Child -> parent: p_parent = R * p + t with
// R = upper-left 3x3, t = (m[3], m[7], m[11]) in metres.  Rigid transforms only (the
// file stores t and R; scale/shear are not representable as far as known).
using Transform = std::array<double, 16>;
constexpr Transform kIdentity{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};

struct Node {
  std::string name;
  int part = -1;                  // >= 0: leaf referencing Model::parts[part]
  Transform transform = kIdentity;// placement in the parent (ignored for the root)
  std::vector<Property> properties;
  std::vector<Node> children;
};

struct Model {
  std::vector<Part> parts;
  Node root;
  std::string pvs_name;           // "" = "000000000000000000000000.pvs"-style 24 hex digits (derived from content)
  std::vector<std::string> member_order; // optional zip member order (names); default: .pvs first, then .ol in part order
  bool write_opacity = false;     // see Material
  bool zip_utf8_flag = false;     // set general-purpose bit 11 for non-ASCII names (Onshape does not)
};

// Encodes the model; returns the .pvz file bytes.  Throws std::invalid_argument on
// inconsistent input (bad indices, pole/knot count mismatch, node with part and children...).
std::string write(const Model& model);

// Convenience: Onshape-style Part Studio export (one part -> part node is the root;
// several -> "RootAssembly").  `det_ids` (optional, per part) fill the BTI/DetId properties.
Model makePartStudioModel(std::vector<Part> parts, const std::string& document_id,
                          const std::vector<std::string>& det_ids = {});

// Adapter properties Onshape attaches to the root node (documentId first).
std::vector<Property> onshapeRootProperties(const std::string& document_id);

} // namespace pvz
} // namespace noshape
