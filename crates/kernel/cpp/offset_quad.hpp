#pragma once
// Offset surfaces and their intersections (by_quadrics' machinery, shell.cpp), shared with offset_solid.
#include <memory>
#include <vector>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomAbs_SurfaceType.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Surface.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Ax3.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_XYZ.hxx>

namespace noshape::quad {
struct Surf
{
  GeomAbs_SurfaceType       kind;
  occ::handle<Geom_Surface> off;    // the offset (world coordinates)
  gp_Ax3                    ax;     // the offset's frame (cylinders, cones)
  double                    r = 0;  // the offset's radius (cylinder, sphere) / reference radius (cone) / minor radius (torus)
  double                    ang = 0; // a cone's semi-angle / a torus's major radius
  gp_XYZ                    n;      // planes: outward normal, n.x = c
  double                    c = 0;
  // Free-form faces (B-spline, ...): the face's own surface, its offset distance along the natural normal (0 for a
  // removed face) and the foot point of the last point evaluated (the next projection starts there).
  bool                                  general = false;
  occ::handle<Geom_Surface>             base;
  occ::handle<ShapeAnalysis_Surface>    sas;
  double                                d = 0;
  mutable gp_Pnt2d                      hint;
  mutable bool                          has_hint = false;
  // (The nearest foot over the whole surface, its search grid built once: solve3_damped.)
  mutable std::shared_ptr<GeomAPI_ProjectPointOnSurf> nearest;
  double                                              own[4] = {0, 0, 0, 0}; // the face's own parameters' box
  // (Points over the face's own patch, made when first asked for: the start of a foot with no last one.)
  mutable std::shared_ptr<const std::vector<std::pair<gp_XYZ, gp_Pnt2d>>> grid;
};

struct OffsetRun
{
  occ::handle<Geom_BSplineCurve>   c3;
  occ::handle<Geom2d_BSplineCurve> pc[2];
  double                           s0 = 0, s1 = 0;
  bool                             done = false;
};

bool     newton_foot(const occ::handle<Geom_Surface>& S, const gp_XYZ& P, gp_Pnt2d& uv, bool* past = nullptr);
bool     bounded_foot(const occ::handle<Geom_Surface>& S, const gp_XYZ& P, gp_Pnt2d& uv, bool newton_tried = false);
gp_Pnt2d foot(const Surf& s, const gp_XYZ& x);
double   eval(const Surf& s, const gp_XYZ& x);
double   eval_grad(const Surf& s, const gp_XYZ& x, gp_XYZ& g);
bool     solve3_grad(const Surf* s[3], gp_XYZ& x, double scale);
bool     offset_d1(const Surf& s, double u, double v, gp_XYZ& P, gp_XYZ& Pu, gp_XYZ& Pv);
bool     solve_small(int n, double A[4][4], double b[4], double x[4]);
bool     march_offsets(const Surf* S[2], const TopoDS_Edge& e, const TopoDS_Face F[2], const gp_Pnt& A, const gp_Pnt& B, double size, double fit_tol, OffsetRun& out);
double   eval_grad_near(const Surf& q, const gp_XYZ& p, gp_XYZ& g, double scale, bool global = true);
bool     solve3_damped(const Surf* s[3], gp_XYZ& x, double scale);
bool     solve3(const Surf* s[3], gp_XYZ& x, double scale);
bool     solve_all(const std::vector<const Surf*>& s, gp_XYZ& x, double scale);
bool     solve_line(const Surf& s, const gp_XYZ& o, const gp_XYZ& d, gp_XYZ& x, double scale);
// A B-spline surface run on past all four sides by `len`, in its own parameters: by its end spans' polynomials
// (continued) or along its tangents (linear_extension); null where a side can't be.
occ::handle<Geom_BSplineSurface> continued_u(const occ::handle<Geom_BSplineSurface>& bs, double len, bool at_end);
occ::handle<Geom_BSplineSurface> continued(const occ::handle<Geom_BSplineSurface>& bs, double len);
occ::handle<Geom_BSplineSurface> linear_u(const occ::handle<Geom_BSplineSurface>& bs, double len, bool at_end);
occ::handle<Geom_BSplineSurface> linear_extension(const occ::handle<Geom_BSplineSurface>& bs, double len);
} // namespace noshape::quad
