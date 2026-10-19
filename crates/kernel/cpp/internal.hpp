// Internal helpers shared by the bridge implementation files (not seen by cxx).
#pragma once

#include "noshape-kernel/cpp/noshape_occt.hpp"
#include "noshape-kernel/src/ffi.rs.h"

#include <BRepTools_History.hxx>
#include <GProp_GProps.hxx>
#include <TopExp.hxx>
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

#include <array>
#include <functional>
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
class HistoryBuilder
{
public:
  HistoryBuilder(const std::vector<TopoDS_Shape>& args, const TopoDS_Shape& output);

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

  const TopoDS_Shape& output() const { return myOut.shape; }

private:
  void push(uint32_t arg, uint8_t in_kind, uint32_t in_index, uint8_t rel,
            const TopoDS_Shape& out);

  std::vector<TopoDS_Shape>             myArgs;
  std::vector<std::unique_ptr<OcShape>> myArgMaps;
  OcShape                               myOut;
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

// Volume properties that survive OCCT's integrator weaknesses (see common.cpp).
GProp_GProps volume_props(const TopoDS_Shape& s);

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

// Convenience: build OpOut from args + output + optional history.
OpOut make_result(const std::vector<TopoDS_Shape>& args, const TopoDS_Shape& out,
                  const BRepTools_History* h);

} // namespace noshape
