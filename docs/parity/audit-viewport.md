# Viewport against live: looks and speed (psui, from 2026-11-02)

User 11-02: rendering parity side by side with live at the same camera (shading, edges and silhouettes, hidden-line and
hidden-edge modes, transparency, section caps, selection and hover colours, appearances and reflections, the view
cube and triad) on live-30, the NIST import, the knurl, a sheet metal flat and a big assembly; then speed budgets
(orbit / pan / zoom fps, first frame after open, selection latency on the 2000-instance assembly and the NIST import).

**Method.** The same B-rep on both sides: our models exported as Parasolid (`noshape-cli export parts`) and imported
into live's "noshape mm test" (tabs `live-30-features`, `knurl-d25x25-45-45`); live's own NIST and big-assembly
files imported into ours. Live: the tab, P (planes off), Shift+7, F, a screenshot of `#canvas` (1673 × 886 at DPR
1; /root/psui-scratch/live/vp.mjs, modes.mjs). Ours: `noshape-cli render image --width 1673 --height 886` (the
app's renderer; /root/psui-scratch/vpr.sh, vpm.sh), compared with `compare -metric AE -fuzz 6%`. Onshape's shaders
and constants are read from its webgl / woolsthorpe bundles (onshape-research/live/dl-bundles/app).

## Found

| # | Area | P | Difference | Status |
|---|---|---|---|---|
| V1 | render_image framing | P2 | the CLI fit padded the box by 10 %, the app's F (live's zoomToFit) 5 %: parts 5 % smaller than live's at the same size | ✅ render_image frames as the app (Camera::fit_box, 1.05) |
| V2 | Ambient occlusion | P1 | ours a home-made 16 px SSAO, barely visible; live a soft halo ≈ 20 px around a boss foot and darker lower walls | ✅ Onshape's: its occlusion GLSL (5 rings × 5 samples, intensity 1, bias 0.1, scale 24 over the scene's depth extent, rings 1 % of the buffer's diagonal) into a half-size buffer, its weighted 5 × 5 filter, `rgb *= pow(1 - occ, 4 × slider)`; none in Hidden line or Translucent (`Scene::no_occlusion`). T1 render offscreen::ambient_occlusion_darkens_a_corner_unless_the_scene_has_none |
| V3 | Translucent | P1 | ours every face at 35 % (two layers over a plate: (191,215,232)); live each part's front faces at 25 % ((227,238,245) = one layer) | ✅ TRANSLUCENT_PASS_ALPHA 0.25, back faces culled (`Instance::front_only`), Part Studio and assemblies; within 3 levels of live's order-independent blend. T1 translucent_parts_blend_their_front_faces_once |
| V4 | Hidden edges visible | P1 | ours dashed grey at alpha 140 over everything; live solid, the edge color mixed 85 % toward the background (HIDDEN_LINE_FADE_AMOUNT): light grey, near-white over faces, visible edges black over them | ✅ solid (217,217,217) drawn only where hidden (`hidden_only`), Part Studio and assemblies |
| V5 | Graphics preferences tooltips | P3 | ours paraphrased ("Soft shading where faces meet…"); live "Controls the …" | ✅ all 19 as live (General from dom/dialog-graphics-preferences.txt, Sketch read 11-02) |

After V1–V4, live-30 at iso differs from live in ≈ 1 % of pixels in every mode (Shaded with edges 0.9 %, Shaded
0.5 %, Translucent 1.3 %, Unshaded 1.1 %, Unshaded + hidden edges 1.1 %, Shaded + hidden edges 1.1 %): edge
anti-aliasing and AO noise.

Other models at iso (ours after V1–V4):
- **Knurl** (our knurl's x_t in live): Shaded with edges 2.0 %, Shaded 1.5 %, Translucent 5.4 %, Unshaded 3.2 %, hidden
  edges 4.1 % (its dense edges' anti-aliasing).
- **NIST** (live's own x_t import, "dx speed nist import"; ours of tests/perf/nist-mtc-crada-assembly.x_t): 3.0 %, the
  same colours per part (#fff2e8, #dde8ff, …), AO and specular noise inside the box.
- **2000-instance assembly** (live "dx big asm", ours of live's dx-big-asm-ap242.step): the same look; render_image of an
  assembly and render_named_view still framed with the 10 % margin (V1 covered Part Studios only): V6.

| # | Area | P | Difference | Status |
|---|---|---|---|---|
| V6 | render_image of an assembly, render_named_view | P2 | framed with the old 10 % margin | ✅ one fit (render.rs `fit_camera`: Camera::fit_box, 1.05) for all three |
| V7 | p (planes) | P2 | ours hid and showed the three default planes only; live (qa4 "Plane z10", 11-02) every plane, a Plane feature's with them | ✅ every plane of the studio (toggle_default_planes over display.planes). T1 UI p_toggles_every_plane |

Section caps (live-30 cut through Boss 1–3's axes: live Shift+X on Boss 2's side, ours `section` plane:front −15 mm):
the same cut, the cap after ✓ hatched in the part colour with a black outline; 3.0 % of pixels differ. Live's lines run
~12 px apart at DPR 1, ours ~5 px in render_image: the pattern is Onshape's 15 × pace device px, and render_image draws
at 2× (a Retina screen, where live is also 7.5 × pace px). The hatch direction follows Onshape's occurrence id hash
(ours our part id): it can't match exactly. Live names Boss 2's section plane "(Front)": the plane holds the cylinder's
reference direction (−X in our x_t), ours had taken the reference direction as its normal ("(Right)"): fixed (A30).

View cube (live-30 iso, the app at 1919 × 992 against live's page): 1.8 % of the cube's pixels differ (its labels'
anti-aliasing). Hover on a face: no fill change on both (live keeps (142,187,214) under the pointer); a selected face's
lit top (229,163,34) on live, ours the same formula (behaviours "ENTITY_HIGHLIGHT_COLOR1"), to compare again on the
matched framing.

Speed (live, this box: Chrome on software WebGL at load 25–56, so not a fair baseline, as behaviours "speed: psui
open a document"): a click on an instance of the 2000-instance assembly took 0.7–6 s to show its selection. Ours is
measured by the new pick benchmark (`NOSHAPE_PERF_BENCH=pick`, bench.sh `PICK`, analyze.py: click to the first frame
with the selection, p50 / p95) on perf.yml's runners.

## Next

The app at DPR 1 against live (AO width / darkness, view cube and triad), selection and hover colours, section caps,
appearances and reflections; the NIST import, the knurl, a sheet metal flat, the big assembly; then the speed budgets.
