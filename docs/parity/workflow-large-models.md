# Workflow: large models (open, orbit, select, edit an early feature, measure)

qa5, 2026-10-21 pass 3, Xvfb :120 (llvmpipe, software Vulkan), debug builds (/tmp/qa5/noshape-1448, CLI 14:36).
Live comparison pending ("qa5 big plate"); live's own machine renders with SwiftShader (10.5 fps on a small model,
behaviours.md "Frame rate"), so fps numbers below are for the perf owner, not a parity verdict.

## 1. NIST MTC CRADA assembly (tests/perf fixture, 36 parts, 670010 triangles; copy qa5-nist.noshape)

| Step | Ours | Notes |
|---|---|---|
| Open (NOSHAPE_PERF=1) | regen[preview] **6.1 s** (123232 tris), then regen[refine] **25.3 s** ("display: bodies 25202 ms"); each fine socket-head screw tessellates in **2.3–3.0 s**, flat-head screws 0.3–0.6 s | perf: fine tessellation of threaded screws dominates |
| Orbit bench (NOSHAPE_PERF_ORBIT=300) | never finished in 3 min: **≈2 viewport renders/s**, gpui.present mean 80–118 ms, "GPU frame still running after 10s" warnings | llvmpipe; perf |
| RSS | 1.42 GB | |

## 2. 400-hole plate (built with /root/qa5-scratch/pass3/mkbig2.py → qa5-big2.noshape)

| Step | Ours (CLI debug) | Status | Verified |
|---|---|---|---|
| 400 circles in one sketch (batch) | 54.7 s for 400 sketch_draw calls | perf |  |
| Extrude Remove Through all, 400 regions | OK, **132 s** | ✗ **PERF1** (live request "qa5 big plate") |  |
| Chamfer 0.5 on 400 top edges | OK, **133 s**; 1397629.073 mm³, 806 faces | ✗ PERF1 |  |
| Edit Extrude 1 10 → 12 | OK, **150 s**; 1677416.687 (by hand ≈ 1677416.7) | ✗ PERF1 |  |
| Open in the app | ~10 min until the model showed (spinner; plane outlines missing, only their labels, while loading) | ✗ PERF1 / ? |  |
| Click on the plate top | readout after ~0.8 s; it selected a **Point** (254.5, 110, 12) instead of the face | ? (point pick priority, cf. ADV 1.7) |  |
| Right-drag 180 px | frame unchanged 0.75 s after mouse-up, updated by 3.6 s (≈ 120° rotation, KB4) | perf |  |

## 3. Side findings while building it

| # | Finding | Repro | Owner |
|---|---|---|---|
| ADV10 | **Feature linear pattern ignores the second direction** (hasSecondDir ☑, directionTwo = Y edge, 20 × 20 mm): only the 20 first-direction instances, status Ok | qa5-big.noshape "Linear pattern 1" | psb |
| ADV11 | Feature pattern of [Extrude Remove + Linear pattern 1] → Warning "19 pattern instance(s) failed: Select parts in the merge scope" | qa5-big.noshape "Linear pattern 2" | psb |

## Update from live's answer (behaviours.md "qa5 big plate", 10-21)

| Step | Live (Onshape server) | Ours (CLI debug, 14:36) | Verdict |
|---|---|---|---|
| 400 holes, Extrude 2 Remove | 1398938.07017026, 406 faces; **total 617 ms** (Extrude 2 491 ms, Sketch 2 116 ms) | 1398938.07 (computed), **132 s** | ✗ PERF1 confirmed (≈200× slower; re-check on a release build) |
| + Chamfer 0.5 × 400 | 1397629.07323126, 806 faces; total **1.02 s** (Chamfer 1 407 ms) | 1397629.073, 806 faces; **133 s** | ✅ geometry / ✗ PERF1 |
| Extrude 1 10 → 12 | **1677416.68726529**, 806; total 1.03 s | 1677416.687, 806; **150 s** | ✅ geometry / ✗ PERF1 |
| Regen time format | "617ms", and "**1.02s**" (2 decimals) at ≥ 1 s, in the Performance panel and the dialog footer | "438.4 ms" style (one decimal + space) | ✗ REG1 (format) |
| Orbit (software rendering on both) | bracket 25.4 fps, big plate 1.3–1.5 fps (SwiftShader); WebGL context lost twice → "Onshape's WebGL rendering context has been lost. Refresh the browser…" | big plate: frame unchanged 0.75 s after mouse-up | – (not comparable) |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Re-run pass 21 (qa5, 2026-10-23, main 40e23871 app)
| REG1 | times read "617ms" / "1.02s" (ui::regen_time_text), Regeneration times dialog as live | ✅ fixed |
| PERF1 | not re-measured (debug build; perf's area) | perf |

## Re-measured by perf (2026-10-26, release builds on the macOS CI runners, Apple Paravirtual Metal; macOS 14 / 15)

Runs: perf.yml 40275443374 (NIST, main 5226055b+), 40249720571 / 40259986505 / 40267041238 (big plate fixture),
40277627211 (model job: big_plate.rs). The runners' presented fps is capped by their timer (33-34 fps fast state,
19 fps slow; see docs/handoff/viewport-performance.md), so frame-time rows give the GPU time and the
frame-driven delivered rate.

| Row | Then (qa5, llvmpipe, debug) | Now | Status | Verified |
|---|---|---|---|---|
| NIST open | preview 6.1 s, refine 25.3 s | first launch: first frame 1.8 / 3.3 s, regen[preview] 1.0 / 2.3 s (133094 tris); regen[refine] in the background **4.9 / 6.7 s** (911998 tris; was 13.9 / 14.4 s, 991130 tris, before the thread bands, f095bf20, run 40357492793) | ✅ | ea572159 10-28 Linux llvmpipe, load 44: window 2.07 s, regen[preview] 5.16 s (display 3.97 s, 133096 tris), background refine 36.3 s (912140 tris; display 36.1 s), Parts (36) listed; not comparable with the CI runners |
| NIST orbit | ≈2 renders/s | 32.5 / 29.6 fps presented (timer-bound), GPU ~5 ms per frame, UI CPU 0.4 ms | ✅ |  |
| Big plate: 400 holes (Extrude Remove) | 132 s | 1.15 s (macOS-latest; Windows 1.31 s); live 617 ms | ✅ PERF1 under 2 s; 2x live | 77870dd4 10-28 Linux: Extrude 2 (400-hole Remove) 517 ms on open (live 491 ms) |
| Big plate: Chamfer 0.5 × 400 | 133 s | 1.08 s (Windows 1.69 s); live 1.02 s | ✅ | v0.4.12 CLI 10-28 06:50 (load ~34): OK 5.5 s incl. the CLI round trip; 1397629.073 mm³, 806 faces |
| Big plate: Extrude 1 10 → 12 | 150 s | 1.62 s (Windows **2.88 s**, over big_plate.rs's 2 s); live 1.03 s | ✅ macOS / ✗ Windows (kern: these doubled since 10-25's 0.49 / 0.71 / 1.04 s on macOS) | 77870dd4 10-28 Linux: commit regen 447 ms preview / 685 ms refine (live 1.03 s) |
| Big plate: open in the app | ~10 min | first frame 1.9 / 3.4 s (preview 1.1 / 2.4 s, fine mesh 0.24 / 0.36 s after) | ✅ | 77870dd4 10-28 Linux xvfb: preview 0.87 s (Extrude 2 517 ms), refine 1.19 s |
| Big plate: orbit / pan / zoom | frame 0.75-3.6 s behind | frame-driven at 2x Retina: orbit 93 / 102 fps, zoom 110 / 102, pan 86 / 75; 2-14% of frames over 16.7 ms | ✅ 60 fps |  |
| Click on the plate top selects a Point | — | not re-checked (pick priority, ADV 1.7) | ✅ (a click between holes selects the top face, "Area: 139893.807 mm²") | 77870dd4 10-28 |

## Re-run 2026-10-28 06:50 (qa, v0.4.12 CLI = release build of green 1b81b7ca, load ~34; bin/mkbig2_qa.py, Remove with `oppositeDirection: false` per API-REMOVE-DIR)
| Step | Ours | Status | Verified |
|---|---|---|---|
| 400 circles in one sketch (batch) | 5.2 s (was 54.7 s, debug build) | ✅ | v0.4.12 CLI 10-28 |
| Extrude Remove Through all, 400 regions | OK, 8.6 s (was 132 s) | ✅ PERF1 much better (release vs debug build, not like for like) | v0.4.12 CLI 10-28 |
| Chamfer 0.5 on 400 top edges | OK, 5.5 s; 1397629.073 mm³, 806 faces (same numbers as 10-21) | ✅ | v0.4.12 CLI 10-28 |
| Edit Extrude 1 10 → 12 | OK, 4.7 s; 1677416.687 (same) | ✅ | v0.4.12 CLI 10-28 |
