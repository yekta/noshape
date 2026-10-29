// Internal helpers shared by the bridge implementation files (not seen by cxx).
#pragma once

#include "noshape-kernel/cpp/noshape_occt.hpp"
#include "noshape-kernel/src/ffi.rs.h"

#include <BRepTools_History.hxx>
#include <Bnd_Box.hxx>
#include <IntCurvesFace_Intersector.hxx>
#include <TopAbs_State.hxx>
#include <memory>
#include <vector>
#include <GProp_GProps.hxx>
#include <Geom_Surface.hxx>
#include <TopExp.hxx>
#include <gp_Trsf.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Dir.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>
#include <gp_Trsf.hxx>
#include <gp_GTrsf.hxx>

#include <array>
#include <Poly_Triangulation.hxx>
#include <map>
#include <functional>
#include <string>
#include <sstream>

namespace noshape {

inline gp_Pnt to_pnt(const P3& p) { return gp_Pnt(p.x, p.y, p.z); }
inline gp_Vec to_vec(const P3& p) { return gp_Vec(p.x, p.y, p.z); }
gp_Dir        to_dir(const P3& p, const char* what);
inline P3     from_xyz(const gp_XYZ& p) { return P3{p.X(), p.Y(), p.Z()}; }
inline P3     from_pnt(const gp_Pnt& p) { return P3{p.X(), p.Y(), p.Z()}; }
inline P3     from_vec(const gp_Vec& p) { return P3{p.X(), p.Y(), p.Z()}; }
inline P3     from_dir(const gp_Dir& p) { return P3{p.X(), p.Y(), p.Z()}; }

// Right-handed axis system from a PlaneF (origin, normal = Z, xdir = X).
gp_Ax3 to_ax3(const PlaneF& p);
gp_Ax2 to_ax2(const PlaneF& p);
gp_Pln to_pln(const PlaneF& p);
PlaneF from_ax3(const gp_Ax3& a);

TopAbs_ShapeEnum to_kind(uint8_t k);
void             require_non_null(const TopoDS_Shape& s, const char* what);
const TopoDS_Face&   as_face(const TopoDS_Shape& s, const char* what);
const TopoDS_Edge&   as_edge(const TopoDS_Shape& s, const char* what);
const TopoDS_Vertex& as_vertex(const TopoDS_Shape& s, const char* what);
TopoDS_Wire          as_wire(const TopoDS_Shape& s, const char* what); // edge -> wire allowed

std::vector<TopoDS_Shape> to_vec(const OcShapeList& l);
ShapeList                 to_list(const std::vector<TopoDS_Shape>& v);
TopoDS_Shape              compound_of(const std::vector<TopoDS_Shape>& v);
// If the shape is a compound with a single child, return the child.
TopoDS_Shape simplify_compound(const TopoDS_Shape& s);

// Relation codes (must match crate::history::Relation).
enum Relation : uint8_t
{
  REL_MODIFIED  = 0,
  REL_GENERATED = 1,
  REL_DELETED   = 2,
  REL_KEPT      = 3,
  REL_FIRST     = 4,
  REL_LAST      = 5,
};

// Collects history records mapping sub-shapes of the input arguments to
// sub-shapes of the final output, using TopExp::MapShapes indices on both
// sides.
// Writes an op's input for a kernel test when NOSHAPE_DUMP_OPS is set (common.cpp).
void dump_op(const char* name, const TopoDS_Shape& s, const std::vector<TopoDS_Shape>& subs, const std::string& params);

// Whether OCCT's booleans may run in parallel: only with NOSHAPE_BOP_PARALLEL=1
// (qa BOOL-THREAD-CRASH, common.cpp).
bool bop_parallel();
// `r` valid where it differs from `input`: its faces not the input's in full,
// the whole shape for closure and orientation (features.cpp).
bool valid_changed(const TopoDS_Shape& input, const TopoDS_Shape& r);
// The face, edge and vertex maps of `s` in one walk, as three
// TopExp::MapShapes make them (each first occurrence, depth first; a
// sub-shape met again isn't walked again) (common.cpp).
void map_fev(const TopoDS_Shape& s, ShapeIMap& f, ShapeIMap& e, ShapeIMap& v);
// TopExp::MapShapesAndUniqueAncestors, quicker (common.cpp).
void unique_ancestors(const TopoDS_Shape& s, TopAbs_ShapeEnum kind, TopAbs_ShapeEnum ancestor_kind, ShapeAncMap& m);
// A face's box: a planar face bounded by straight edges only, its corners'
// (with their tolerances; the face lies in their hull), any other face
// BRepBndLib's without triangulation (common.cpp). OcShape::face_boxes's.
void face_box(const TopoDS_Shape& f, Bnd_Box& b);
// `out`'s face boxes from `from`'s for the faces they share (common.cpp).
void inherit_face_boxes(const OcShape& from, OcShape& out);
// The OcShape's ancestor map (CSR offsets, values; ancestor_map's), made
// once and kept (common.cpp).
const std::pair<std::vector<uint32_t>, std::vector<uint32_t>>& ancestors_cached(const OcShape& s, TopAbs_ShapeEnum kind, TopAbs_ShapeEnum ancestor_kind);
// Every edge used twice, once each way (features.cpp).
bool closed_and_oriented(const TopoDS_Shape& r);

// Where in a shape the unify can find something to merge, when the shape
// is a local Boolean's result on a target that had nothing (a candidate
// pair of faces has a face the Boolean made, a candidate vertex is one whose
// edges or their faces it changed): `faces`, the new faces, the faces across
// the edges of the pieces kept, and every face at a `seeds` vertex, in the
// shape's own order and orientations; `seeds`, the vertices of the edges made
// or removed and of the pieces' edges (boolean.cpp). The unify of the
// 1600-face fin plate after each fin walked the whole part three times.
struct UnifyScope
{
  TopoDS_Compound faces;
  ShapeIMap       seeds;
};

// A point against a solid by the parity of a ray's crossings, each ray
// intersected only with the faces whose box it passes (OCCT's solid
// classifier traces every face of a 1600-face part for each point, ~60 ms)
// (geom.cpp). IN / OUT from a ray that crosses face interiors only; ON when
// a crossing is within `tol` of the point; UNKNOWN when no ray was clean (a
// crossing within `band` of the point, through an edge, or tangent): the
// caller asks OCCT's classifier.
class RayClassifier
{
public:
  RayClassifier(const TopoDS_Shape& solid, double tol, double band);
  // With the solid's faces and their boxes (as BRepBndLib makes them) at hand.
  RayClassifier(const ShapeIMap& faces, std::vector<Bnd_Box> boxes, double tol, double band);
  ~RayClassifier();
  TopAbs_State Perform(const gp_Pnt& p);

private:
  ShapeIMap                                               myFaces;
  std::vector<Bnd_Box>                                    myBoxes;
  std::vector<std::unique_ptr<IntCurvesFace_Intersector>> myRays;
  std::vector<int8_t>                                     myKinds; // -1 not looked at, 1 planar, 0 not
  std::vector<gp_Pln>                                     myPlanes;
  double                                                  myTol, myBand;
};

// noshape's own Boolean (crates/brep; rust_boolean.cpp), off with NOSHAPE_BOOL_RUST=0: the Boolean `op` of
// `args` (the object first) when it takes them (with `touching_only`, only when they merely touch), OCCT's
// fuzzy value `fuzzy` added to every tolerance. `srcs`
// gets an OcShape for each argument (made in `own` where it has none), `out` the result, `hist` its history.
bool rust_boolean_on();
// noshape's own display mesher (mesh_rust.cpp): on unless NOSHAPE_MESH_RUST=0.
bool rust_mesh_on();
// Meshes the `fresh` faces it takes (analytic ones): each is marked in `taken` with its triangulation in `tris`
// (in the face's own frame; per face index), and their edges' polylines go to `polys` (per edge index, the
// FORWARD edge's way). Returns how many.
int rust_mesh_faces(const ShapeIMap& faces, const ShapeIMap& edges, const ShapeIMap& vertices, const std::vector<char>& fresh, double lin, double ang,
                    std::vector<char>& taken, std::vector<occ::handle<Poly_Triangulation>>& tris, std::vector<std::vector<std::array<float, 3>>>& polys);
// Notes why it wasn't tried (false; the replay's strict mode fails).
bool rust_decline(const char* why);
bool rust_boolean(uint8_t op, const std::vector<TopoDS_Shape>& args, std::vector<const OcShape*>& srcs,
                  std::vector<std::unique_ptr<OcShape>>& own, TopoDS_Shape& out, occ::handle<BRepTools_History>& hist,
                  double fuzzy = 0, bool touching_only = false);
// Keeps this thread's last result's flat body on `out` (its OcShape), for the next Boolean on it.
void rust_boolean_keep(const OcShape& out);
// The kernel's own Booleans outside op_boolean (Shell, slabs): `op` (0 fuse, 1 cut, 2 common) of `args` by
// `tools`, noshape's where it takes them, else OCCT's (in parallel with `parallel`); recorded with
// NOSHAPE_RECORD_BOOLEANS. False when OCCT's fails.
bool quick_boolean(uint8_t op, const ShapeList& args, const ShapeList& tools, bool parallel, TopoDS_Shape& out,
                   occ::handle<BRepTools_History>& hist);

class HistoryBuilder
{
public:
  // `sources[i]`, when given and holding args[i], lends its maps (the
  // caller's OcShapes: their maps are made already).
  // `out_maps`: the output's face, edge and vertex maps, made already (taken).
  HistoryBuilder(const std::vector<TopoDS_Shape>& args, const TopoDS_Shape& output, const std::vector<const OcShape*>* sources = nullptr, std::array<ShapeIMap, 3>* out_maps = nullptr);

  // Add Modified/Generated/Deleted records from a (composed) history plus
  // Kept records for input sub-shapes present unchanged in the output.
  void add_history(const BRepTools_History* h);
  // Add Kept records only (for input sub-shapes present in the output).
  void add_kept();
  // For each sub-shape (vertex/edge/face) of argument `arg`, call `fn` and
  // record (relation) every non-null result found in the output.
  void add_mapped(uint32_t arg, uint8_t relation,
                  const std::function<ShapeList(const TopoDS_Shape&)>& fn);
  // Record every face (and the shape itself if a face) of `cap` as
  // (relation) of the first face/wire of argument `arg`.
  void add_cap(uint32_t arg, uint8_t relation, const TopoDS_Shape& cap);

  OpOut finish();

  const TopoDS_Shape& output() const { return myOut->shape; }
  // The result's OcShape (finish() hands it on, with the maps made here).
  OcShape& out() { return *myOut; }

private:
  void push(uint32_t arg, uint8_t in_kind, uint32_t in_index, uint8_t rel,
            const TopoDS_Shape& out);

  std::vector<TopoDS_Shape>             myArgs;
  std::vector<std::unique_ptr<OcShape>> myOwnArgMaps;
  std::vector<const OcShape*>           myArgMaps;
  std::unique_ptr<OcShape>              myOut;
  rust::Vec<HistRec>                    myRecs;
};

// Images of `s` through a (possibly null) history: Modified images, or `s`
// itself when unmodified; empty when removed.
ShapeList images_of(const occ::handle<BRepTools_History>& h, const TopoDS_Shape& s);

// Message of a failed BRepAlgoAPI / BOPAlgo algorithm (DumpErrors).
template <class Algo>
std::string algo_errors(const Algo& a)
{
  std::ostringstream os;
  a.DumpErrors(os);
  std::string s = os.str();
  while (!s.empty() && (s.back() == '\n' || s.back() == ' '))
    s.pop_back();
  return s;
}

// Factor applied to model lengths before fragile OCCT algorithms (blends):
// they are tuned for millimetre-sized parts. 1 = none (set_working_scale).
double working_scale();

// Corner faces of a fillet over smaller corner fillets rebuilt as Parasolid's
// rolling-ball vertex blends (vertex_blend.cpp); `s` unchanged (and the
// history null) when there are none or the result wouldn't be valid.
TopoDS_Shape exact_vertex_blends(const TopoDS_Shape& s, occ::handle<BRepTools_History>& history);

// Mesh import (mesh_io.cpp): nodes closer than `weld` merged (0: STL's grid
// merge), degenerate triangles dropped, T-junctions split (weld > 0); `pts`
// the merged nodes, `tris` renumbered.
void mesh_weld(const std::vector<gp_Pnt>& nodes, std::vector<std::array<int, 3>>& tris, double weld, std::vector<gp_Pnt>& pts);

// Mesh bodies (mesh_body.cpp): faces with a triangulation and no surface.
bool is_mesh_face(const TopoDS_Face& f);
bool has_mesh_faces(const TopoDS_Shape& s);
bool mesh_valid(const TopoDS_Shape& s);
TopoDS_Shape transform_mesh(const TopoDS_Shape& s, const gp_Trsf& t, BRepTools_History& h);
TopoDS_Shape gtransform_mesh(const TopoDS_Shape& s, const gp_GTrsf& g, BRepTools_History& h);
// Booleans and splits with mesh bodies (mesh_body.cpp): mesh faces as planar
// facets for OCCT (`source`: each facet's plane -> its mesh face;
// `to_facets`: mesh faces -> facets), then the result's facets back as one
// mesh face per shell (`h`: that step's history).
struct MeshFacets
{
  std::map<const Geom_Surface*, TopoDS_Face> source;
  occ::handle<BRepTools_History>             to_facets = new BRepTools_History;
};
TopoDS_Shape facet_mesh(const TopoDS_Shape& s, MeshFacets& m);
TopoDS_Shape unfacet_mesh(const TopoDS_Shape& s, const MeshFacets& m, occ::handle<BRepTools_History>& h);
// One mesh face per connected piece of the welded triangles, and whether
// the piece is closed (every edge used equally often each way).
struct MeshPiece
{
  TopoDS_Face face;
  bool        closed = false;
};
std::vector<MeshPiece> mesh_bodies(const std::vector<gp_Pnt>& nodes, std::vector<std::array<int, 3>> tris, double weld);
// A body per closed group / open piece: closed pieces in one solid (a shell
// each), each open piece a shell; returned as a compound of them.
TopoDS_Shape mesh_compound(const std::vector<MeshPiece>& pieces);
// 3MF / glTF / GLB triangles of every placed object, in millimetres
// (mesh_formats.cpp); false for other files.
bool read_mesh_format(const std::string& path, std::vector<gp_Pnt>& nodes, std::vector<std::array<int, 3>>& tris);

// Parasolid import (xt_read.cpp): every part of a transmit file (assembly
// instances placed), metres, with its SDL/TYSA name and colour.
struct XtPart
{
  TopoDS_Shape          shape;
  std::string           name;
  std::array<double, 3> colour{0, 0, 0};
  bool                  has_colour = false;
  double                opacity    = 1; // 1 - the file's transparency
  // Colours of single faces: (index in TopExp::MapShapes order, sRGB).
  std::vector<std::pair<int, std::array<double, 3>>> face_colours;
};
std::vector<XtPart> read_xt_parts(const std::string& path);
// The same from a transmit file's bytes (or a bare binary stream "PS\0\0…").
std::vector<XtPart> read_xt_data(std::string data);
// A transmit file's assembly: the top assembly's name, its distinct bodies
// (healed, in their own coordinates) and every placed body instance (index
// into `parts`, placement in metres, the body's name), in file order. No
// instances without an assembly.
struct XtTree
{
  std::string                          root;
  std::vector<XtPart>                  parts;
  std::vector<std::pair<int, gp_Trsf>> instances;
  std::vector<std::string>             names;
};
XtTree read_xt_tree(const std::string& path);
// Pcurves, tolerances, seams and apexes for an imported body (xt_read.cpp),
// shared by the Parasolid and ACIS readers.
TopoDS_Shape heal_import(TopoDS_Shape shape, double tol);
// ACIS import (sat_read.cpp): every body of a .sat/.sab file (or a
// ShapeManager stream), metres, with its name and colour.
std::vector<XtPart> read_sat_parts(const std::string& path);
std::vector<XtPart> read_sat_data(std::string data);
// JT import (jt_read.cpp): the parts of its XT B-rep segments (unplaced).
std::vector<XtPart> read_jt_parts(const std::string& path);
// Native CAD import (native.cpp): the Parasolid geometry a SolidWorks (OLE2),
// Solid Edge or NX part file embeds, read as above.
// A SOLIDWORKS assembly's components (native.cpp): the active configuration's
// tree, flattened; `parent` indexes the list (-1 = top level). Transforms are
// row-vector 4x4 matrices as the file stores them (p' = p M, metres).
struct AsmNode
{
  int                    parent = -1;
  std::string            name, file, configuration;
  int                    config_id = -1, number = 1;
  std::array<double, 16> m{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
  bool                   suppressed = false, hidden = false, exclude_bom = false, assembly = false;
};
std::vector<AsmNode> read_sldasm_tree(const std::string& path);
// An Inventor assembly's occurrence tree; `file`: the referenced document,
// relative to `search_dir` (where the assembly and its documents are).
std::vector<AsmNode> read_iam_tree(const std::string& path, const std::string& search_dir);
// A Solid Edge assembly's occurrence tree (`file`: the document's name).
std::vector<AsmNode> read_se_tree(const std::string& path, const std::string& search_dir);
// Any native assembly: SOLIDWORKS, Inventor or Solid Edge, by extension.
std::vector<AsmNode> read_native_tree(const std::string& path, const std::string& search_dir);
// Extract a zip's top-level files into `dir` (folders are listed, not
// extracted); returns every entry name.
std::vector<std::string> extract_zip(const std::string& zip, const std::string& dir);
// `config`: a SOLIDWORKS configuration id (an assembly's reference), -1 = the default.
std::vector<XtPart> read_native_parts(const std::string& path, int config = -1);
// Rhino import (rhino_read.cpp, openNURBS): every object, placed, metres.
std::vector<XtPart> read_rhino_parts(const std::string& path);

// Volume properties that survive OCCT's integrator weaknesses (see common.cpp).
GProp_GProps volume_props(const TopoDS_Shape& s);
// Area properties likewise (faces wrapping a periodic surface more than once).
GProp_GProps surface_props(const TopoDS_Shape& s);

// B-rep exchange writers (xt.cpp): a shape with elementary surfaces where it
// can, B-splines for swept/offset ones (revolved ones too unless `keep_spun`)
// and closed B-spline faces split; whether a face lies on a cylinder/cone/
// sphere/torus (or a kept surface of revolution); a face's
// boundary as oriented edge cycles without seams of such surfaces and
// without degenerate edges.
TopoDS_Shape                          exchange_prepared(const TopoDS_Shape& s, bool keep_spun);
bool                                  exchange_periodic_analytic(const TopoDS_Face& f);
std::vector<std::vector<TopoDS_Edge>> exchange_loops(const TopoDS_Face& f);

// Mesh import (mesh_io.cpp): a shell (solid when closed) of planar triangle
// faces; nodes closer than `weld` are one vertex and T-junctions are split
// (`weld` 0: STL's grid merge). Meshes with at most `merge_limit` triangles
// get coplanar neighbours merged.
TopoDS_Shape mesh_shape(const std::vector<gp_Pnt>& nodes, const std::vector<std::array<int, 3>>& tris, uint32_t merge_limit, double weld);
// One prepared shape as a Parasolid neutral-binary transmit body (from "PS\0\0",
// no text header: what JT embeds), and its faces in XT order (node ids).
std::string parasolid_binary_body(const TopoDS_Shape& prepared, std::vector<TopoDS_Face>& faces);

// A blend of some of a slab's edges for blend_in_slabs (`edges`: indices into
// the edges given to it; `image` maps the part's sub-shapes into the slab):
// the result and its history from the slab; false if it fails.
using SlabBlend = std::function<bool(const TopoDS_Shape& slab, const std::vector<int>& edges,
                                     const std::function<TopoDS_Shape(const TopoDS_Shape&)>& image,
                                     TopoDS_Shape& out, occ::handle<BRepTools_History>& hist)>;
// Blends of many edges far apart, slab by slab in parallel (boolean.cpp);
// `reach` bounds how far a blend changes the part from its edge. False when
// it doesn't apply or a slab fails.
bool blend_in_slabs(const TopoDS_Shape& target, const std::vector<TopoDS_Edge>& edges, double reach, const SlabBlend& blend,
                    TopoDS_Shape& out, occ::handle<BRepTools_History>& hist);

// An OCCT seam: closes every face it bounds (`faces`, its ancestors) on
// itself; Parasolid has no such edge (common.cpp shown_topology).
bool is_seam(const TopoDS_Edge& e, const NCollection_List<TopoDS_Shape>* faces);

// Convenience: build OpOut from args + output + optional history.
OpOut make_result(const std::vector<TopoDS_Shape>& args, const TopoDS_Shape& out,
                  const BRepTools_History* h, const std::vector<const OcShape*>* sources = nullptr,
                  std::array<ShapeIMap, 3>* out_maps = nullptr);

} // namespace noshape
