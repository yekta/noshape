| AS-01 | Assembly tab and assembly data model | P0 | App data model (instance tree, TopoDS_Shape + TopLoc_Location); XCAF mirror for exchange |
| AS-02 | Instances list (instances + Mate Features list) | P0 | App UI/data model; filter commands, folders, icons; BRepMesh_IncrementalMesh for tessellation setting |
| AS-03 | Insert parts and assemblies | P0 | App model; shared shape + TopLoc_Location per instance; reference-graph cycle check |
| AS-04 | Rigid Part Studio instance (insert as rigid, Edit) | P1 | App model; single solver body with N shapes |
| AS-05 | Copy / paste and duplicate instances | P1 | App model (clone instances + internal mates/MCs) |
| AS-06 | Fix / Unfix / Unfix all | P0 | Custom solver: remove body variables; rigid detection |
| AS-07 | Hide / Show instances and mates | P0 | Renderer visibility flags |
| AS-08 | Suppress / Unsuppress and dynamic suppression | P0 | App model + expression engine; exclude from solver/BOM |
| AS-09 | Rename instance / instance properties | P1 | App model / metadata |
| AS-10 | Isolate / Make transparent | P1 | Renderer; Bnd_Box / BRepExtrema_DistShapeShape for distance expansion; mate graph BFS for connectivity |
| AS-11 | Delete instance (cascade) | P0 | App dependency graph |
| AS-12 | Replace instance | P1 | Persistent-naming remap of MC references; BRepAdaptor_Surface/Curve geometric matching fallback |
| AS-13 | Change configuration / Change to version of an instance | P1 | Regenerate source Part Studio per configuration; cache |
| AS-14 | Free drag and drag-solve | P0 | Custom solver with soft drag target (null-space projection) |
| AS-15 | Triad manipulator (move / rotate with typed values) | P0 | Custom GPUI/wgpu widget; gp_Trsf composition; AIS_Manipulator as reference |
| AS-16 | Snap mode | P1 | UI + solver; auto Fastened mate creation |
| AS-17 | Mate connector concept (explicit vs implicit, ownership) | P0 | gp_Ax3 local frame + persistent topology reference per occurrence |
| AS-18 | Implicit MC inference points (hover, Shift lock, cycling) | P0 | BRepGProp centroid, BRepLProp_SLProps normal, BRepAdaptor_Curve/Surface (circle center, axis points, cone apex), GCPnts_AbscissaPoint midpoint |
| AS-19 | Mate connector feature (assembly and Part Studio) | P0 | gp_Ax3 construction + gp_Trsf local move/rotate; realign from face normal / edge direction |
| AS-20 | Mate feature: common dialog and workflow | P0 | Custom constraint solver (OCCT has none); app feature model |
| AS-21 | Mate offsets | P0 | gp_Trsf offset applied to first connector frame in solver |
| AS-22 | Mate limits | P1 | Solver inequality constraints (active set / bounded LM) |
| AS-23 | Mate value display and direct value entry | P1 | Solver DOF value functions (B*^-1 A decomposition) |
| AS-24 | Fastened mate | P0 | Solver: 6 equations; union-find rigid clustering |
| AS-25 | Revolute mate | P0 | Solver: 5 equations (origin coincidence + Z parallel) |
| AS-26 | Slider mate | P0 | Solver: 5 equations (origin on Z line + orientation lock) |
| AS-27 | Planar mate | P0 | Solver: 3 equations (point on plane + Z parallel) |
| AS-28 | Cylindrical mate | P0 | Solver: 4 equations (origin on Z line + Z parallel) |
| AS-29 | Pin slot mate | P1 | Solver: 4 equations (pin origin on slot X line + Z parallel) |
| AS-30 | Ball mate | P0 | Solver: 3 equations (origin coincidence); ZY'Z' Euler for values/cone limit |
| AS-31 | Parallel mate | P1 | Solver: 2 equations (Z parallel) |
| AS-32 | Tangent mate | P1 | Solver: analytic surface distance functions (GeomAbs types via BRepAdaptor_Surface); Extrema_ExtSS / BRepExtrema_DistShapeShape fallback |
| AS-33 | Width mate | P1 | Solver: center-plane symmetry equations |
| AS-34 | Mate context menu and list operations | P1 | App UI; home-position transform cache for Reset |
| AS-35 | Animate mate | P1 | Solver continuation loop with driving equality + render timer |
| AS-36 | Mate indicators, states and errors | P1 | Renderer glyphs; solver rank/residual diagnostics |
| AS-37 | Show mates mode and mate visibility | P2 | Renderer / UI mode |
| AS-38 | Group (rigid set) vs Fastened | P0 | Solver: union-find merge into one rigid body |
| AS-39 | Relations (common behavior) | P1 | Solver coupling equations on DOF value functions |
| AS-40 | Gear relation | P1 | Solver: dtheta2 = s*k*dtheta1 |
| AS-41 | Rack and pinion relation | P1 | Solver: dx = s*(L/2pi)*dtheta |
| AS-42 | Screw relation | P1 | Solver: dz = s*(p/2pi)*dtheta on one cylindrical mate |
| AS-43 | Linear relation | P1 | Solver: dx2 = s*k*dx1 |
| AS-44 | Replicate | P1 | Geometric signature matching (BRepAdaptor_Surface/Curve, TopExp adjacency) + re-inference of MCs |
| AS-45 | Assembly linear pattern | P1 | gp_Trsf::SetTranslation per instance; direction from BRepAdaptor |
| AS-46 | Assembly circular pattern | P1 | gp_Trsf::SetRotation(gp_Ax1); axis from BRepAdaptor_Surface cylinder/cone |
| AS-47 | Assembly mirror | P1 | gp_Trsf::SetMirror; proper transform M*T*S for symmetric parts; BRepBuilderAPI_Transform(copy) for derived mirror; BRepAlgoAPI_Common symmetry test |
| AS-48 | Subassembly restructuring (move to new, create empty, dissolve, drag in/out) | P0 | App model tree edits + mate re-parenting |
| AS-49 | Subassembly solve behavior (flexible vs rigid, lock/follow position) | P0 | Solver: rigid/locked = one body, flexible = flattened per occurrence |
| AS-50 | Modeling in context (Edit in context, assembly contexts, update context) | P1 | Context snapshot of shapes (BRepBuilderAPI_Copy / shared shapes) in primary-instance frame; manual update |
| AS-51 | Create Part Studio in context | P1 | As AS-50; new Part Studio frame = chosen MC |
| AS-52 | Updating references (linked documents, Reference manager) | P1 | App/PDM version data model |
| AS-53 | Exploded views | P1 | Display-only per-step gp_Trsf stack; polyline explode lines from BRepGProp centroids |
| AS-54 | Named positions | P1 | Solver driving equalities from stored DOF values + absolute transforms |
| AS-55 | Display states | P2 | App visibility map per occurrence path |
| AS-56 | Interference detection (and drag collision) | P1 | BVH broad phase; BRepExtrema_ShapeProximity / mesh tri-tri; BRepAlgoAPI_Common + BRepGProp volume + Bnd_Box dims; BRepExtrema_DistShapeShape |
| AS-57 | Bill of materials (assembly panel) | P1 | App data model: tree traversal with grouping key; CSV export |
| AS-58 | Assembly configurations and variable table | P1 | App expression engine + configuration tables |
| AS-59 | Standard content (fasteners) | P2 | Custom parametric fastener generator (BRepPrimAPI_MakeCylinder/MakePrism) from standard tables; auto-size via cylinder radius |
| AS-60 | Assembly keyboard shortcuts | P1 | UI keymap |
| AS-61 | Assembly constraint solver (design suggestion) | P0 | Custom Rust solver: 6-DOF bodies, mate residuals, LM/dogleg, active-set limits, coupling relations, SVD/QR DOF analysis (OCCT has no solver) |
| AS-62 | Assembly structure persistence and STEP/XCAF export (design suggestion) | P1 | XCAFDoc_ShapeTool (AddShape/AddComponent), XCAFDoc_ColorTool, TDataStd_Name, STEPCAFControl_Writer/Reader, RWGltf_CafWriter |
