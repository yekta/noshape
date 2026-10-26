//! Raw `cxx` bridge to the C++ OCCT wrapper in `cpp/`.
//!
//! Every C++ function returns `Result`, so cxx wraps it in a try/catch that
//! converts any C++ exception into `Err(cxx::Exception)`. The safe API in the
//! rest of the crate maps those into [`crate::KernelError`].
//!
//! Shape-type codes are `TopAbs_ShapeEnum` values (0 = Compound ... 7 = Vertex).

#[allow(clippy::too_many_arguments, clippy::module_inception)]
#[cxx::bridge(namespace = "noshape")]
pub(crate) mod ffi {
    /// 3D point / vector.
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    struct P3 {
        x: f64,
        y: f64,
        z: f64,
    }

    /// Plane / axis system: origin, X direction, normal (Z).
    #[derive(Debug, Clone, Copy)]
    struct PlaneF {
        origin: P3,
        xdir: P3,
        normal: P3,
    }

    #[derive(Debug, Clone, Copy)]
    struct PlaneQ {
        found: bool,
        plane: PlaneF,
    }

    /// One history record (see `crate::history`).
    #[derive(Debug, Clone, Copy)]
    struct HistRec {
        arg: u32,
        in_kind: u8,
        in_index: u32,
        relation: u8,
        out_kind: u8,
        out_index: u32,
    }

    struct OpOut {
        shape: UniquePtr<OcShape>,
        history: Vec<HistRec>,
    }

    /// Surface description. kind: 0 plane, 1 cylinder, 2 cone, 3 sphere,
    /// 4 torus, 5 bezier, 6 bspline, 7 revolution, 8 extrusion, 9 offset, 10 other.
    #[derive(Debug, Clone, Copy)]
    struct SurfaceInfo {
        kind: u8,
        origin: P3,
        axis: P3,
        xdir: P3,
        r1: f64,
        r2: f64,
        angle: f64,
        u_degree: i32,
        v_degree: i32,
        nb_u_poles: i32,
        nb_v_poles: i32,
        u_periodic: bool,
        v_periodic: bool,
        rational: bool,
        reversed: bool,
    }

    /// Curve description. kind: 0 line, 1 circle, 2 ellipse, 3 hyperbola,
    /// 4 parabola, 5 bezier, 6 bspline, 7 offset, 8 other.
    #[derive(Debug, Clone, Copy)]
    struct CurveInfo {
        kind: u8,
        origin: P3,
        axis: P3,
        xdir: P3,
        r1: f64,
        r2: f64,
        degree: i32,
        nb_poles: i32,
        rational: bool,
        periodic: bool,
        closed: bool,
        degenerated: bool,
        first: f64,
        last: f64,
        start: P3,
        end: P3,
        length: f64,
    }

    /// B-spline data of an edge curve (poles flattened xyz; `weights` empty
    /// when non-rational).
    #[derive(Debug, Clone)]
    struct BSplineF {
        poles: Vec<f64>,
        weights: Vec<f64>,
        knots: Vec<f64>,
        mults: Vec<i32>,
        degree: i32,
        periodic: bool,
    }

    /// A B-spline surface as FeatureScript's BSplineSurface gives it: poles
    /// `nu` rows (u) by `nv` columns (v), flattened xyz row by row; `weights`
    /// empty when non-rational; full knot vectors (repeats written out); a
    /// periodic direction with its `degree` overlapping rows/columns.
    #[derive(Debug, Clone)]
    struct BSplineSurfF {
        poles: Vec<f64>,
        weights: Vec<f64>,
        nu: u32,
        nv: u32,
        u_knots: Vec<f64>,
        v_knots: Vec<f64>,
        u_degree: i32,
        v_degree: i32,
        u_periodic: bool,
        v_periodic: bool,
    }

    /// One component of a native assembly (flattened tree; `parent` -1 =
    /// top level), transform row-vector 4x4 as the file stores it.
    #[derive(Debug, Clone)]
    struct AsmNodeF {
        parent: i32,
        name: String,
        file: String,
        configuration: String,
        config_id: i32,
        number: i32,
        transform: Vec<f64>,
        suppressed: bool,
        hidden: bool,
        exclude_bom: bool,
        assembly: bool,
    }

    /// A 2D B-spline curve (FeatureScript's BSplineCurve of dimension 2):
    /// poles flattened xy, `weights` empty when non-rational, full knot
    /// vector, overlapping poles when periodic.
    #[derive(Debug, Clone)]
    struct BSpline2dF {
        poles: Vec<f64>,
        weights: Vec<f64>,
        knots: Vec<f64>,
        degree: i32,
        periodic: bool,
    }

    /// 2D sketch segment. kind: 0 line (p0..p3 = ax ay bx by),
    /// 1 circular arc (cx cy r a0 a1), 2 full circle (cx cy r),
    /// 3 elliptic arc (cx cy rmaj rmin rot a0 a1), 4 full ellipse (cx cy rmaj rmin rot),
    /// 5 bspline (poles/weights/knots/mults/degree/periodic), 6 arc through 3 points
    /// (ax ay mx my bx by), 7 interpolated spline through `poles` points.
    #[derive(Debug, Clone)]
    struct Seg2d {
        kind: u8,
        p: [f64; 8],
        poles: Vec<f64>,
        weights: Vec<f64>,
        knots: Vec<f64>,
        mults: Vec<i32>,
        degree: i32,
        periodic: bool,
    }

    #[derive(Debug, Clone, Copy)]
    struct MeshCounts {
        nodes: usize,
        indices: usize,
        faces: usize,
        edge_points: usize,
        edges: usize,
        vertices: usize,
    }

    #[derive(Debug, Clone, Copy)]
    struct PropsF {
        mass: f64,
        center: P3,
        inertia: [f64; 9],
    }

    #[derive(Debug, Clone, Copy)]
    struct DistanceF {
        distance: f64,
        p1: P3,
        p2: P3,
        count: i32,
    }

    #[derive(Debug, Clone, Copy)]
    struct ProjPoint {
        point: P3,
        u: f64,
        v: f64,
        distance: f64,
    }

    /// Compressed sparse rows: row i = values[offsets[i]..offsets[i+1]].
    struct Csr {
        offsets: Vec<u32>,
        values: Vec<u32>,
    }

    struct HlrEdgeF {
        visible: bool,
        kind: u8,
        source: i32,
        start: u32,
        count: u32,
    }

    struct HlrOut {
        edges: Vec<HlrEdgeF>,
        points: Vec<f64>,
    }

    /// A placed part instance of a .pvz product tree: the part (index into
    /// read_pvz_parts), the instance name, the tree's root name, and the
    /// placement (row-major 3x4, metres).
    struct PvzInstance {
        part: u32,
        name: String,
        root: String,
        transform: Vec<f64>,
    }

    struct StepImport {
        shapes: UniquePtr<OcShapeList>,
        names: Vec<String>,
        colors: Vec<f32>,
        has_color: Vec<bool>,
        /// Opacity per shape (1 = opaque; may be empty).
        opacity: Vec<f32>,
        /// Face colours: (shape index, face index, sRGB) per entry.
        face_part: Vec<u32>,
        face_index: Vec<u32>,
        face_colors: Vec<f32>,
    }

    /// A STEP assembly: its distinct parts (own coordinates, OCCT mm) and
    /// the placed leaf instances (`PvzInstance` layout, metres); no
    /// instances when the file's root is not one assembly.
    struct StepTree {
        root: String,
        parts: StepImport,
        instances: Vec<PvzInstance>,
    }

    #[derive(Debug, Clone, Copy)]
    struct BoolOpts {
        fuzzy: f64,
        /// 0 off, 1 shift, 2 full, 3 shift checked by volume (else off)
        glue: u8,
        unify: bool,
        parallel: bool,
        obb: bool,
    }

    #[derive(Debug, Clone, Copy)]
    struct SweepOpts {
        /// 0 corrected frenet, 1 frenet, 2 fixed (trihedron from `dir`/`xdir`),
        /// 3 constant binormal (`dir`), 4 auxiliary spine (aux shape), 5 discrete,
        /// 6 spine support surface (aux shape)
        mode: u8,
        dir: P3,
        xdir: P3,
        make_solid: bool,
        /// 0 modified, 1 transformed, 2 round corner
        transition: u8,
        scale_end: f64,
        with_contact: bool,
        with_correction: bool,
        /// Total twist of the profile about the path (radians, right-handed about
        /// the path direction), spread evenly along its length; 0 = none.
        twist: f64,
    }

    /// Developable surface for wrap / unwrap (see `crate::surfacing`).
    /// kind: 0 plane, 1 cylinder, 2 cone.
    #[derive(Debug, Clone, Copy)]
    struct DevSurf {
        kind: u8,
        origin: P3,
        axis: P3,
        xdir: P3,
        radius: f64,
        half_angle: f64,
        ref_angle: f64,
    }

    unsafe extern "C++" {
        include!("noshape-kernel/cpp/noshape_occt.hpp");

        type OcShape;
        type OcShapeList;
        type OcMesh;

        // ---- shapes & lists
        fn shape_null() -> Result<UniquePtr<OcShape>>;
        fn shape_clone(s: &OcShape) -> Result<UniquePtr<OcShape>>;
        fn shape_type(s: &OcShape) -> Result<u8>;
        fn shape_is_null(s: &OcShape) -> Result<bool>;
        fn shape_orientation(s: &OcShape) -> Result<u8>;
        fn shape_oriented(s: &OcShape, orientation: u8) -> Result<UniquePtr<OcShape>>;
        fn shape_hash(s: &OcShape) -> Result<u64>;
        fn shape_is_same(a: &OcShape, b: &OcShape) -> Result<bool>;
        fn shape_is_equal(a: &OcShape, b: &OcShape) -> Result<bool>;
        fn shape_is_partner(a: &OcShape, b: &OcShape) -> Result<bool>;
        fn shape_partner_hash(s: &OcShape) -> Result<u64>;
        fn shape_location(s: &OcShape) -> Result<Vec<f64>>;
        fn shape_unlocated(s: &OcShape) -> Result<UniquePtr<OcShape>>;
        fn sub_count(s: &OcShape, kind: u8) -> Result<u32>;
        fn sub_shape(s: &OcShape, kind: u8, index: u32) -> Result<UniquePtr<OcShape>>;
        fn sub_shapes(s: &OcShape, kind: u8) -> Result<UniquePtr<OcShapeList>>;
        fn index_of(s: &OcShape, sub: &OcShape, kind: u8) -> Result<i64>;
        fn ancestor_map(s: &OcShape, kind: u8, ancestor_kind: u8) -> Result<Csr>;
        fn adjacent_faces(s: &OcShape, face_index: u32) -> Result<Vec<u32>>;
        fn shown_topology(s: &OcShape) -> Result<Vec<u8>>;
        fn children(s: &OcShape) -> Result<UniquePtr<OcShapeList>>;
        fn make_compound(l: &OcShapeList) -> Result<UniquePtr<OcShape>>;
        fn list_new() -> Result<UniquePtr<OcShapeList>>;
        fn list_push(l: Pin<&mut OcShapeList>, s: &OcShape) -> Result<()>;
        fn list_len(l: &OcShapeList) -> Result<usize>;
        fn list_get(l: &OcShapeList, i: usize) -> Result<UniquePtr<OcShape>>;
        fn is_valid(s: &OcShape) -> Result<bool>;
        fn self_intersects(s: &OcShape) -> Result<bool>;
        fn fix_shape(s: &OcShape, tolerance: f64) -> Result<UniquePtr<OcShape>>;
        fn dump_shape(s: &OcShape) -> Result<String>;

        // ---- geometry
        fn surface_info(face: &OcShape) -> Result<SurfaceInfo>;
        fn curve_info(edge: &OcShape) -> Result<CurveInfo>;
        fn edge_bspline(edge: &OcShape) -> Result<BSplineF>;
        fn face_bspline(face: &OcShape) -> Result<BSplineSurfF>;
        fn face_normal_at_uv(face: &OcShape, u: f64, v: f64) -> Result<P3>;
        fn face_point_at_uv(face: &OcShape, u: f64, v: f64) -> Result<P3>;
        fn face_project_point(face: &OcShape, p: P3) -> Result<ProjPoint>;
        fn face_uv_bounds(face: &OcShape) -> Result<Vec<f64>>;
        fn edge_point_at(edge: &OcShape, t: f64) -> Result<P3>;
        fn edge_tangent_at(edge: &OcShape, t: f64) -> Result<P3>;
        fn vertex_point(v: &OcShape) -> Result<P3>;
        fn bounding_box(s: &OcShape, optimal: bool) -> Result<Vec<f64>>;
        fn find_plane(s: &OcShape, tolerance: f64) -> Result<PlaneQ>;
        fn props_volume(s: &OcShape) -> Result<PropsF>;
        fn props_surface(s: &OcShape) -> Result<PropsF>;
        fn props_linear(s: &OcShape) -> Result<PropsF>;
        fn min_distance(a: &OcShape, b: &OcShape) -> Result<DistanceF>;
        fn angle_between(a: &OcShape, b: &OcShape) -> Result<f64>;
        fn classify_point(solid: &OcShape, p: P3, tol: f64) -> Result<i8>;
        fn line_crossings(s: &OcShape, p: P3, dir: P3, len: f64) -> Result<Vec<f64>>;

        // ---- construction
        fn make_vertex(p: P3) -> Result<UniquePtr<OcShape>>;
        fn make_edge_2d(plane: &PlaneF, seg: &Seg2d) -> Result<UniquePtr<OcShape>>;
        fn make_wire_2d(plane: &PlaneF, segs: &[Seg2d]) -> Result<UniquePtr<OcShape>>;
        fn make_wire(edges: &OcShapeList) -> Result<UniquePtr<OcShape>>;
        fn wire_with_edge_order(wire: &OcShape, order: &[u32]) -> Result<UniquePtr<OcShape>>;
        fn make_face_on_plane(
            plane: &PlaneF,
            outer: &OcShape,
            holes: &OcShapeList,
        ) -> Result<UniquePtr<OcShape>>;
        fn make_face_from_wire(wire: &OcShape, only_plane: bool) -> Result<UniquePtr<OcShape>>;
        fn make_plane_face(
            plane: &PlaneF,
            umin: f64,
            umax: f64,
            vmin: f64,
            vmax: f64,
        ) -> Result<UniquePtr<OcShape>>;
        fn make_polyline(pts: &[f64], closed: bool) -> Result<UniquePtr<OcShape>>;
        fn make_line(a: P3, b: P3) -> Result<UniquePtr<OcShape>>;
        fn make_circle(
            c: P3,
            axis: P3,
            xdir: P3,
            r: f64,
            a0: f64,
            a1: f64,
            full: bool,
        ) -> Result<UniquePtr<OcShape>>;
        fn make_arc_3pts(a: P3, m: P3, b: P3) -> Result<UniquePtr<OcShape>>;
        fn make_interpolated(
            pts: &[f64],
            tangents: &[f64],
            periodic: bool,
        ) -> Result<UniquePtr<OcShape>>;
        fn make_bspline_3d(
            poles: &[f64],
            weights: &[f64],
            knots: &[f64],
            mults: &[i32],
            degree: i32,
            periodic: bool,
        ) -> Result<UniquePtr<OcShape>>;
        fn make_helix(
            frame: &PlaneF,
            radius: f64,
            pitch: f64,
            height: f64,
            left_handed: bool,
            taper_angle: f64,
        ) -> Result<UniquePtr<OcShape>>;
        fn offset_wire_2d(
            wire_or_face: &OcShape,
            distance: f64,
            join: u8,
            open_result: bool,
        ) -> Result<UniquePtr<OcShape>>;
        fn make_box(frame: &PlaneF, dx: f64, dy: f64, dz: f64) -> Result<UniquePtr<OcShape>>;
        fn make_cylinder(frame: &PlaneF, r: f64, h: f64, angle: f64)
        -> Result<UniquePtr<OcShape>>;
        fn make_cone(
            frame: &PlaneF,
            r1: f64,
            r2: f64,
            h: f64,
            angle: f64,
        ) -> Result<UniquePtr<OcShape>>;
        fn make_sphere(frame: &PlaneF, r: f64, angle: f64) -> Result<UniquePtr<OcShape>>;
        fn make_torus(frame: &PlaneF, r1: f64, r2: f64, angle: f64) -> Result<UniquePtr<OcShape>>;
        fn make_wedge(
            frame: &PlaneF,
            dx: f64,
            dy: f64,
            dz: f64,
            ltx: f64,
        ) -> Result<UniquePtr<OcShape>>;
        fn edges_to_wires(s: &OcShape, tol: f64) -> Result<UniquePtr<OcShapeList>>;

        // ---- features
        fn op_prism(profile: &OcShape, start: P3, end: P3, draft_angle: f64) -> Result<OpOut>;
        fn op_prism_until(
            profile: &OcShape,
            dir: P3,
            until: &OcShape,
            start_offset: f64,
        ) -> Result<OpOut>;
        fn op_revol(profile: &OcShape, origin: P3, axis: P3, a0: f64, a1: f64) -> Result<OpOut>;
        fn op_pipe(profile: &OcShape, spine: &OcShape) -> Result<OpOut>;
        fn op_pipe_shell(
            profiles: &OcShapeList,
            spine: &OcShape,
            opts: &SweepOpts,
            aux: &OcShape,
        ) -> Result<OpOut>;
        fn op_loft(
            sections: &OcShapeList,
            solid: bool,
            ruled: bool,
            closed: bool,
            start_points: &[f64],
            conditions: &[f64],
            path: &OcShape,
            path_count: i32,
        ) -> Result<OpOut>;
        fn set_working_scale(k: f64) -> Result<()>;
        fn op_thicken(s: &OcShape, t1: f64, t2: f64) -> Result<OpOut>;
        fn op_shell(solid: &OcShape, faces: &OcShapeList, thickness: f64, join: u8)
        -> Result<OpOut>;
        fn op_shell_solid(solid: &OcShape, faces: &OcShapeList, thickness: f64, outward: bool, method: u8) -> Result<OpOut>;
        fn op_fillet(s: &OcShape, edges: &OcShapeList, radii: &[f64], checked: bool) -> Result<OpOut>;
        fn op_fillet_variable(
            s: &OcShape,
            edge: &OcShape,
            params: &[f64],
            radii: &[f64],
            checked: bool,
        ) -> Result<OpOut>;
        fn op_chamfer(
            s: &OcShape,
            edges: &OcShapeList,
            faces: &OcShapeList,
            mode: u8,
            d1: f64,
            d2: f64,
            checked: bool,
        ) -> Result<OpOut>;
        fn op_draft_wedges(solid: &OcShape, faces: &OcShapeList, pull: P3, neutral: &PlaneF, angle: f64) -> Result<OpOut>;
        fn op_draft_parting_curve(solid: &OcShape, faces: &OcShapeList, edges: &OcShapeList, pull: P3, angles: &[f64]) -> Result<OpOut>;
        fn op_draft(
            s: &OcShape,
            faces: &OcShapeList,
            dir: P3,
            neutral: &PlaneF,
            angle: f64,
        ) -> Result<OpOut>;
        fn op_defeature(s: &OcShape, faces: &OcShapeList) -> Result<OpOut>;
        fn op_offset_faces(s: &OcShape, faces: &OcShapeList, offsets: &[f64]) -> Result<OpOut>;
        fn op_offset_shape(s: &OcShape, offset: f64, join: u8) -> Result<OpOut>;
        fn op_sew(shapes: &OcShapeList, tol: f64, make_solid: bool) -> Result<OpOut>;
        fn op_fill(
            edges: &OcShapeList,
            supports: &OcShapeList,
            orders: &[i32],
            points: &[f64],
            guides: &OcShapeList,
        ) -> Result<OpOut>;
        fn op_rib(
            base: &OcShape,
            wire: &OcShape,
            plane: &PlaneF,
            d1: P3,
            d2: P3,
            fuse: bool,
        ) -> Result<OpOut>;

        // ---- booleans etc.
        fn set_memory_budget_mb(mb: u64);
        fn op_boolean(
            op: u8,
            args: &OcShapeList,
            tools: &OcShapeList,
            opts: &BoolOpts,
        ) -> Result<OpOut>;
        fn op_split(args: &OcShapeList, tools: &OcShapeList, opts: &BoolOpts) -> Result<OpOut>;
        /// Cancel flag of this thread's booleans (an `AtomicBool` address, 0 = none).
        fn set_thread_cancel(flag: usize);
        fn op_unify(s: &OcShape) -> Result<OpOut>;
        fn op_enclose(shapes: &OcShapeList) -> Result<OpOut>;
        fn op_transform(s: &OcShape, m: &[f64], copy: bool) -> Result<OpOut>;
        fn op_gtransform(s: &OcShape, m: &[f64]) -> Result<OpOut>;
        fn op_section(a: &OcShape, b: &OcShape) -> Result<OpOut>;
        fn section_plane(s: &OcShape, plane: &PlaneF) -> Result<UniquePtr<OcShape>>;
        fn section_caps(s: &OcShape, plane: &PlaneF) -> Result<UniquePtr<OcShape>>;
        fn project_along(curves: &OcShape, target: &OcShape, dir: P3)
        -> Result<UniquePtr<OcShape>>;
        fn project_normal(curves: &OcShape, target: &OcShape) -> Result<UniquePtr<OcShape>>;

        // ---- tessellation
        fn mesh_prepare(
            s: &OcShape,
            lin: f64,
            ang: f64,
            relative: bool,
            parallel: bool,
        ) -> Result<UniquePtr<OcMesh>>;
        fn mesh_counts(m: &OcMesh) -> Result<MeshCounts>;
        fn mesh_fill(
            m: &OcMesh,
            positions: &mut [f32],
            normals: &mut [f32],
            indices: &mut [u32],
            face_ranges: &mut [u32],
            edge_points: &mut [f32],
            edge_ranges: &mut [u32],
            vertices: &mut [f32],
        ) -> Result<()>;

        // ---- I/O
        fn write_brep_file(s: &OcShape, path: &str) -> Result<()>;
        fn read_brep_file(path: &str) -> Result<UniquePtr<OcShape>>;
        fn brep_to_bytes(s: &OcShape) -> Result<Vec<u8>>;
        fn brep_from_bytes(bytes: &[u8]) -> Result<UniquePtr<OcShape>>;
        fn bin_to_bytes(s: &OcShape, with_triangles: bool) -> Result<Vec<u8>>;
        fn bin_from_bytes(bytes: &[u8]) -> Result<UniquePtr<OcShape>>;
        fn read_step(path: &str) -> Result<StepImport>;
        fn read_step_tree(path: &str) -> Result<StepTree>;
        fn write_step(
            shapes: &OcShapeList,
            names: &[String],
            colors: &[f32],
            has_color: &[bool],
            path: &str,
            schema: u8,
            file_unit: u8,
        ) -> Result<()>;
        fn read_iges(path: &str) -> Result<UniquePtr<OcShape>>;
        fn read_mesh(path: &str, merge_limit: u32) -> Result<UniquePtr<OcShape>>;
        fn write_iges(shapes: &OcShapeList, path: &str, system: &str, version: &str) -> Result<()>;
        fn write_parasolid(shapes: &OcShapeList, path: &str, binary: bool, version: &str, application: &str) -> Result<()>;
        fn read_parasolid(path: &str) -> Result<UniquePtr<OcShape>>;
        fn read_parasolid_parts(path: &str) -> Result<StepImport>;
        fn read_rhino_parts_ffi(path: &str) -> Result<StepImport>;
        fn read_native_parts_ffi(path: &str) -> Result<StepImport>;
        fn read_native_parts_config_ffi(path: &str, config: i32) -> Result<StepImport>;
        fn read_native_tree_ffi(path: &str, dir: &str) -> Result<Vec<AsmNodeF>>;
        fn extract_zip_ffi(zip: &str, dir: &str) -> Result<Vec<String>>;
        fn read_sat_parts_ffi(path: &str) -> Result<StepImport>;
        fn read_jt_parts_ffi(path: &str) -> Result<StepImport>;
        fn write_acis(shapes: &OcShapeList, path: &str) -> Result<()>;
        fn write_rhino(shapes: &OcShapeList, names: &[String], path: &str, version: u32) -> Result<()>;
        fn rhino_valid_breps(path: &str) -> Result<u32>;
        fn write_jt(shapes: &OcShapeList, names: &[String], path: &str) -> Result<()>;
        fn write_pvz(shapes: &OcShapeList, names: &[String], path: &str) -> Result<()>;
        fn read_pvz(path: &str) -> Result<UniquePtr<OcShape>>;
        fn read_pvz_parts(path: &str) -> Result<StepImport>;
        fn read_pvz_instances(path: &str) -> Result<Vec<PvzInstance>>;
        fn read_mesh_bodies(path: &str) -> Result<UniquePtr<OcShape>>;
        fn is_mesh(s: &OcShape) -> bool;
        fn has_triangulation(s: &OcShape) -> bool;

        // ---- surfacing (wrap, isocline, surface fitting)
        fn op_wrap_faces(faces: &OcShapeList, src: &DevSurf, dst: &DevSurf, xf: &[f64], tol: f64) -> Result<OpOut>;
        fn isocline_edges(face: &OcShape, dir: P3, angle: f64, extend: bool) -> Result<UniquePtr<OcShapeList>>;
        fn bspline_surface_face(surface: &BSplineSurfF, boundary: &[BSpline2dF], make_solid: bool) -> Result<UniquePtr<OcShape>>;
        fn bspline_grid_face(
            frame: &PlaneF,
            x0: f64,
            dx: f64,
            y0: f64,
            dy: f64,
            z: &[f64],
            nx: u32,
            ny: u32,
            deg_max: i32,
            tol: f64,
            bounds: &[f64],
        ) -> Result<UniquePtr<OcShape>>;
        fn face_max_deviation(face: &OcShape, pts: &[f64]) -> Result<f64>;

        // ---- HLR
        fn project_hlr(s: &OcShape, dir: P3, up: P3, exact: bool, deflection: f64)
        -> Result<HlrOut>;
    }
}

// SAFETY: OcShape wraps an immutable TopoDS_Shape (handles with atomic
// reference counts); its lazily-built maps are guarded by std::call_once.
// OcShapeList/OcMesh are plain owned C++ containers that are not shared.
unsafe impl Send for ffi::OcShape {}
unsafe impl Sync for ffi::OcShape {}
unsafe impl Send for ffi::OcShapeList {}
unsafe impl Send for ffi::OcMesh {}
unsafe impl Sync for ffi::OcMesh {}
