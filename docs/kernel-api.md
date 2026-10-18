# noshape-kernel public API

Crate `noshape-kernel` (`crates/kernel`). Everything is re-exported at the crate
root (`use noshape_kernel::*;`). Math types are glam (`DVec2`, `DVec3`, `DMat3`,
`DMat4`, re-exported). Build: `OCCT_ROOT=/root/occt cargo build -p noshape-kernel`
(see docs/BUILDING.md).

## Conventions

* **Indices**: sub-shapes are always identified by their index in
  `TopExp::MapShapes` order of the owning shape, i.e. the order of
  `shape.sub_shapes(kind)` / `faces()` / `edges()` / `vertices()`. History
  records, tessellation face/edge ranges and HLR `source_edge` all use these.
* **Errors**: every fallible fn returns `Result<T, KernelError>`. All OCCT
  exceptions are caught in C++; nothing panics on modeling failures.
  `KernelError::{InvalidInput, OperationFailed, Occt, Unsupported, Io}(String)`.
* **Selections** (edges to fillet, faces to remove, ...) are passed as `&[Shape]`
  sub-shapes of the operated shape (`shape.sub_shape(ShapeType::Edge, i)?`).
* Units: the kernel is unitless; only STEP/IGES I/O takes a `LengthUnit`.

## Shape and topology (`shape`)

```rust
#[derive(Clone)] pub struct Shape;           // Arc-backed, cheap clone, Send + Sync
// Eq/Hash use OCCT IsSame (TShape + location, orientation ignored) -> usable as HashMap key
pub enum ShapeType { Compound, CompSolid, Solid, Shell, Face, Wire, Edge, Vertex, Unknown }
pub enum Orientation { Forward, Reversed, Internal, External }

impl Shape {
    fn null() -> Shape;                     fn is_null(&self) -> bool;
    fn shape_type(&self) -> ShapeType;      fn orientation(&self) -> Orientation;
    fn oriented(&self, Orientation) -> Result<Shape>;  fn reversed(&self) -> Result<Shape>;
    fn is_same(&self, &Shape) -> bool;  fn is_equal(..) -> bool;  fn is_partner(..) -> bool;
    fn hash_code(&self) -> u64;
    fn count(&self, ShapeType) -> usize;
    fn sub_shapes(&self, ShapeType) -> Vec<Shape>;          // MapShapes order
    fn sub_shape(&self, ShapeType, index: usize) -> Result<Shape>;
    fn faces/edges/vertices/solids/wires/shells(&self) -> Vec<Shape>;
    fn index_of(&self, sub: &Shape, ShapeType) -> Option<usize>;   // O(1), maps cached
    fn ancestor_map(&self, kind, ancestor_kind) -> Result<AncestorMap>; // CSR; .get(i) -> &[u32], .to_vecs()
    fn ancestors(&self, kind, index, ancestor_kind) -> Result<Vec<usize>>;
    fn edge_faces(&self) -> Result<Vec<Vec<u32>>>;     // edge i -> face indices
    fn vertex_faces(&self) -> Result<Vec<Vec<u32>>>;   // vertex i -> face indices
    fn vertex_edges(&self) -> Result<Vec<Vec<u32>>>;   // vertex i -> edge indices
    fn adjacent_faces(&self, face_index: usize) -> Result<Vec<usize>>;
    fn children(&self) -> Vec<Shape>;  fn compound(&[Shape]) -> Shape;
    fn is_valid(&self) -> bool;        fn fixed(&self, tolerance: f64) -> Result<Shape>;
    fn dump(&self) -> String;
}
```

## Geometry queries (`geom`)

```rust
pub struct Plane { origin: DVec3, x_dir: DVec3, normal: DVec3 }  // also used as placement frame
  Plane::new(origin, normal, x_dir), xy(), yz(), xz(), from_normal(o, n),
  unit_x/unit_y/unit_normal, to_world(DVec2), to_local(DVec3), signed_distance, offset(d), flipped()
pub struct BoundingBox { min, max }  // center(), size(), diagonal()
pub enum Surface { Plane(Plane), Cylinder{origin,axis,x_dir,radius}, Cone{origin,axis,x_dir,ref_radius,half_angle,apex},
                   Sphere{center,axis,radius}, Torus{center,axis,major_radius,minor_radius},
                   Bezier{..}, BSpline{..}, Revolution{axis_origin,axis_dir}, Extrusion{direction}, Offset{offset}, Other }
pub enum Curve { Line{origin,direction}, Circle{center,axis,x_dir,radius}, Ellipse{..}, Hyperbola, Parabola,
                 Bezier{..}, BSpline{..}, Offset, Other }
pub struct EdgeInfo { curve, start, end, first_param, last_param, length, closed, degenerated }
pub struct BSplineCurveData { poles: Vec<DVec3>, weights: Option<Vec<f64>>, knots: Vec<f64>, mults: Vec<u32>, degree: u32, periodic: bool }
pub struct FaceProjection { point, u, v, distance }

impl Shape {
    fn face_surface(&self) -> Result<Surface>;
    fn face_plane(&self) -> Result<Option<Plane>>;       // normal = outward (orientation applied)
    fn find_plane(&self, tol) -> Result<Option<Plane>>;  fn is_planar(&self) -> bool;
    fn normal_at_uv(&self, u, v) -> Result<DVec3>;       fn normal_at_point(&self, p) -> Result<DVec3>;
    fn point_at_uv(&self, u, v) -> Result<DVec3>;        fn project_point(&self, p) -> Result<FaceProjection>;
    fn uv_bounds(&self) -> Result<[f64; 4]>;
    fn area(&self) -> Result<f64>;  fn centroid(&self) -> Result<DVec3>;   // faces (area centroid)
    fn edge_info(&self) -> Result<EdgeInfo>;  fn edge_curve(&self) -> Result<Curve>;
    fn edge_bspline(&self) -> Result<BSplineCurveData>;  // exact B-spline of the edge (range + orientation applied)
    fn endpoints(&self) -> Result<(DVec3, DVec3)>;  fn length(&self) -> Result<f64>;
    fn point_at(&self, t) -> Result<DVec3>;  fn tangent_at(&self, t) -> Result<DVec3>;
    fn is_closed(&self) -> Result<bool>;     fn vertex_point(&self) -> Result<DVec3>;
    fn bounding_box(&self, optimal: bool) -> Result<BoundingBox>;
    fn classify_point(&self, p, tol) -> Result<PointClass>;   // Inside/Outside/OnBoundary/Unknown
    fn volume(&self) -> Result<f64>;
}
```

## Construction (`construct`)

```rust
pub enum Segment2d {            // plane coordinates, radians CCW around plane normal
    Line{a,b}, Arc{center,radius,start_angle,end_angle}, Arc3{a,mid,b}, Circle{center,radius},
    Ellipse{center,major_radius,minor_radius,rotation}, EllipseArc{..,start_angle,end_angle},
    BSpline{poles,weights:Option<Vec<f64>>,knots,mults,degree,periodic}, Interpolated{points,periodic} }
pub struct TrackedProfile { shape: Shape, segment_edges: Vec<u32> } // segment i -> edge index in shape.edges()

fn make_edge_2d(&Plane, &Segment2d) -> Result<Shape>;
fn make_edges_2d(&Plane, &[Segment2d]) -> Result<Vec<Shape>>;      // input order
fn make_wire_2d(&Plane, &[Segment2d]) -> Result<Shape>;
fn make_wire_2d_tracked(&Plane, &[Segment2d]) -> Result<TrackedProfile>;
fn make_face_2d(&Plane, outer: &[Segment2d], holes: &[Vec<Segment2d>]) -> Result<Shape>;
fn make_face_2d_tracked(&Plane, outer, holes) -> Result<TrackedProfile>; // outer segs first, then holes
fn make_wire(edges: &[Shape]) -> Result<Shape>;
fn make_face(&Plane, outer_wire: &Shape, holes: &[Shape]) -> Result<Shape>;  // orientation auto-fixed
fn make_face_from_wire(&Shape, only_plane: bool) -> Result<Shape>;
fn make_plane_face(&Plane, umin, umax, vmin, vmax) -> Result<Shape>;
fn make_vertex(DVec3); make_line(a,b); make_polyline(&[DVec3], closed); make_circle(c, axis, r);
fn make_arc(c, axis, x_dir, r, a0, a1); make_arc_3pts(a,m,b);
fn make_interpolated_spline(&[DVec3], tangents: &[DVec3], periodic); make_bspline(poles, weights, knots, mults, degree, periodic);
   // tangents: none, [start, end] or one per point (zero = free); derivative vectors w.r.t. the
   // chord-length parameter: magnitudes are kept (unit = natural speed, longer = pulls further)
fn make_bspline_edge(&BSplineCurveData) -> Result<Shape>;   // e.g. after editing edge_bspline() poles/weights
fn make_helix(frame: &Plane, radius, pitch, height, left_handed, taper_angle) -> Result<Shape>;  // wire
fn offset_wire_2d(&Shape, distance, JoinType, open_result) -> Result<Shape>;
fn edges_to_wires(&Shape, tol) -> Result<Vec<Shape>>;
// primitives (frame = placement; normal = local Z)
fn make_box(&Plane, dx, dy, dz); make_box_corners(a, b); make_cylinder(&Plane, r, h, angle);
fn make_cone(&Plane, r1, r2, h, angle); make_sphere(&Plane, r, angle); make_torus(&Plane, R, r, angle);
fn make_wedge(&Plane, dx, dy, dz, ltx);
pub enum JoinType { Arc, Intersection }
```

## History (`history`)

```rust
pub enum Relation { Modified, Generated, Deleted, Kept, First, Last }
//  Kept  = input sub-shape present unchanged in output (same TShape+location) ("Same")
//  First/Last = start/end cap copy of an input sub-shape (prism, revolve, sweep, loft, up-to)
pub struct HistoryRecord { arg: u32, input_kind: ShapeType, input_index: u32,
                           relation: Relation, output_kind: ShapeType, output_index: u32 }
//  Deleted: output_kind = Unknown, output_index = u32::MAX
pub struct History { records: Vec<HistoryRecord> }
impl History {
    fn of_input(&self, arg, kind, index) -> impl Iterator<Item=&HistoryRecord>;
    fn of_output(&self, kind, index) -> impl Iterator<Item=&HistoryRecord>;
    fn modified(&self, arg, kind, index) -> Vec<u32>;            // Modified + Kept images
    fn generated(&self, arg, kind, index) -> Vec<(ShapeType, u32)>;
    fn is_deleted(&self, arg, kind, index) -> bool;
    fn kept_faces(&self) -> Vec<u32>;  fn first_faces(&self) -> Vec<u32>;  fn last_faces(&self) -> Vec<u32>;
    fn by_input(&self) -> HashMap<(u32, ShapeType, u32), Vec<HistoryRecord>>;
    fn by_output(&self) -> HashMap<(ShapeType, u32), Vec<HistoryRecord>>;
}
pub struct OpResult { shape: Shape, history: History }
```
History covers Solid/Face/Edge/Vertex inputs. Argument order is documented per
op (booleans/split: objects then tools; extrude: 0 = profile; sweep: profile(s)
then path; up-to extrude: 0 = profile, 1 = target).

## Feature operations (`ops`) — all return `Result<OpResult>`

```rust
fn extrude(profile, vector: DVec3);
fn extrude_between(profile, start: DVec3, end: DVec3, draft_angle: f64);  // offsets, 2-sided, draft
fn extrude_symmetric(profile, vector);
fn extrude_up_to(profile, direction, target: &Shape, start_offset);      // face/shell/solid target
fn extrude_up_to_point(profile, direction, point);
fn revolve(profile, origin, axis, angle);  fn revolve_between(profile, origin, axis, a0, a1);
fn sweep(profile, path, &SweepOptions);  fn sweep_sections(&[Shape], path, &SweepOptions);
fn sweep_pipe(profile, path);
   SweepOptions { mode: SweepMode{CorrectedFrenet, Frenet, Fixed{z,x}, Binormal(DVec3), Discrete},
                  solid, transition: SweepTransition{Transformed, RoundCorner}, scale_end, with_contact, with_correction }
fn loft(&[Shape], &LoftOptions);   LoftOptions { solid, ruled, closed, start_points: Option<Vec<DVec3>> }
fn thicken(shape, t1, t2);   // result solids are always oriented outward (positive volume)
fn shell(solid, remove_faces: &[Shape], thickness, ShellSide{Inside, Outside}, JoinType);
fn fillet(shape, edges, radius);  fn fillet_radii(shape, edges, &[f64]);  fn fillet_variable(shape, edge, &[(t, r)]);
fn chamfer(shape, edges, ChamferSpec{Equal(d), TwoDistances(d1,d2), DistanceAngle(d,angle)}, reference_faces: &[Shape]);
fn draft(shape, faces, pull_direction, neutral: &Plane, angle);
fn delete_faces(shape, faces);
fn offset_faces(shape, faces, distance);  fn offset_faces_each(shape, faces, &[f64]);
fn replace_face(..) -> Err(Unsupported);
fn offset_shape(shape, distance, JoinType);
fn sew(shapes, tolerance, make_solid);
fn fill_surface(&[FillEdge{edge, support: Option<Shape>, continuity: Continuity}], points: &[DVec3]);
fn enclose(shapes);
fn rib(base, profile, plane: &Plane, thickness);   // robust split/extrude/cut/fuse gusset
fn rib_linear_form(base, profile, plane, thickness, fuse: bool); // OCCT BRepFeat_MakeLinearForm
fn revolve_profile_tool(profile: &[DVec2] /* (radius, depth) */, frame: &Plane) -> Result<Shape>;
fn cut_holes(target, profile: &[DVec2], frames: &[Plane]);
```

## Booleans, transforms, sections (`boolean`, `transform`)

```rust
pub enum BooleanOp { Fuse, Cut, Common }   pub enum Glue { Off, Shift, Full }
pub struct BooleanOptions { fuzzy: f64, glue: Glue, unify: bool, parallel: bool }  // Default: exact, no unify, parallel
fn boolean(op, objects: &[Shape], tools: &[Shape], &BooleanOptions) -> Result<OpResult>;
fn fuse(a, b); cut(a, b); common(a, b); fuse_all(&[Shape], &opts);
fn split(objects, tools, &opts); split_by_plane(shape, &Plane); imprint(shape, curves);
fn unify_same_domain(shape) -> Result<OpResult>;      // history composed through USD
fn section(a, b) -> Result<OpResult>;  section_plane(shape, &Plane) -> Result<Shape>;
fn section_wires(shape, &Plane) -> Result<Vec<Shape>>;  section_caps(shape, &Plane) -> Result<Shape>;
fn project_curves(curves, target, direction: Option<DVec3>) -> Result<Shape>;  // curves: edge, wire or any
   // compound (of edges, wires or faces: every wire and free edge is projected); target: any shape with faces
pub enum Transform { Translate(DVec3), Rotate{origin,axis,angle}, Scale{center,factor},
                     ScaleXyz{center,factors}, Mirror(Plane), Matrix(DMat4) }
  Transform::frame_to_frame(&Plane, &Plane), .matrix()
fn transform(shape, &Transform, copy: bool) -> Result<OpResult>;   // every sub-shape Modified -> image
impl Shape { fn transformed(&self, &Transform) -> Result<Shape>; fn translated(&self, DVec3) -> Result<Shape>; }
```

## Tessellation (`mesh`)

```rust
fn tessellate(shape, linear_deflection, angular_deflection, parallel: bool) -> Result<Tessellation>;
fn tessellate_with(shape, &MeshOptions{linear_deflection, angular_deflection, relative, parallel});
pub struct Tessellation {
    positions: Vec<[f32;3]>, normals: Vec<[f32;3]>,  // unit, outward
    indices: Vec<u32>,                               // CCW from outside
    faces: Vec<FaceRange{first_index, index_count, first_vertex, vertex_count}>,  // face i (MapShapes)
    edge_points: Vec<[f32;3]>, edges: Vec<EdgeRange{first_point, point_count}>,  // edge i (MapShapes)
    vertices: Vec<[f32;3]>,                                                       // vertex i
}  // face_indices(i), edge_polyline(i), triangle_count()
```

## Mass properties and measurement (`props`)

```rust
pub struct MassProperties { mass: f64, center: DVec3, inertia: DMat3 /* about COM, unit density */ }
fn volume_properties(&Shape); surface_properties(&Shape); linear_properties(&Shape) -> Result<MassProperties>;
pub struct Distance { distance, point_a, point_b, solutions }
fn distance(a, b) -> Result<Distance>;  fn angle_between(a, b) -> Result<f64>;
```

## I/O (`io`)

```rust
pub enum LengthUnit { Millimeter, Centimeter, Meter, Inch, Foot }   // unit of the kernel shapes
pub struct ImportedShape { shape, name: Option<String>, color: Option<[f32;3]> }
pub struct ExportShape   { shape, name: Option<String>, color: Option<[f32;3]> }
fn read_step(&Path, LengthUnit) -> Result<Vec<ImportedShape>>;   // XCAF names/colors, assemblies flattened
fn write_step(&Path, &[ExportShape], LengthUnit) -> Result<()>;  fn write_step_shapes(&Path, &[Shape], LengthUnit);
fn read_iges(&Path, LengthUnit) -> Result<Shape>;  fn write_iges(&Path, &[Shape], LengthUnit);
fn read_mesh(&Path, file_unit: LengthUnit, merge_limit: u32) -> Result<Shape>;  // STL/OBJ: triangle faces in one connected shell (solid if closed), coplanar faces merged when <= merge_limit triangles; coordinates in file_unit, returned in meters
fn write_stl(&Shape, &Path, deflection, StlFormat{Binary, Ascii});  fn write_stl_mesh(&Tessellation, &Path, StlFormat);
fn write_obj(&Shape, &Path, deflection);  fn write_obj_mesh(&Tessellation, &Path);
fn write_brep(&Shape, &Path); fn read_brep(&Path) -> Result<Shape>;
impl Shape { fn to_brep_bytes(&self) -> Result<Vec<u8>>; fn from_brep_bytes(&[u8]) -> Result<Shape>;
             fn to_bin_bytes(&self, with_triangles: bool) -> Result<Vec<u8>>; fn from_bin_bytes(&[u8]) -> Result<Shape>; }
```

## HLR for drawings (`hlr`)

```rust
pub enum Visibility { Visible, Hidden }   pub enum EdgeClass { Sharp, Smooth, Outline, Seam }
pub struct ProjectedEdge { visibility, class, polyline: Vec<DVec2> /* x right, y up */, source_edge: Option<u32> }
fn project_hlr(shape, view_dir, up, exact: bool) -> Result<Vec<ProjectedEdge>>;
fn project_hlr_with(shape, view_dir, up, exact, deflection) -> Result<Vec<ProjectedEdge>>;
```
