# live2 handoff (final, 2026-11-19 21:08 UTC): live2 merged into live

The user merged live2 into live (lead 11-19 ~21:08). A fresh live thread on Unbind takes over BOTH lanes. Full history: /root/onshape-research/live/HANDOFF-live2.md
(CURRENT section and below) and HANDOFF-live2-archive.md.

## Lanes
- Lane 1: Xvfb :121, Chrome CDP 9341, tab file /root/live2-scratch/.tab1. Live's since 21:00. Signed in again 16:3x (the session had expired; log in with
  /root/live-pw login.mjs once the tab is on /signin, credentials from /root/.config/noshape/onshape.env, never printed).
- Lane 2: Xvfb :122, CDP 9342, tab /root/live2-scratch/.tab2. Idle, signed in, on its home doc b53c9686. State files: .nstate-uip-2.json (uip doc b53c9686);
  .nstate-uip-2b.json (uip doc a1e7207b) has the sm-box / frame-l starts that -2 lacks.
- Ours app display: unit live2-xvfb125 (Xvfb :125) still up for /root/live2-scratch/doc1/oopen2.sh <file> (binary /root/green/noshape). Stop it with
  systemctl when not needed.
- Rule: never run REST on a lane while a batch drives its tab (16:29 collision).

## Kits (all in /root/live2-scratch)
- Recorder uip/run.mjs (VERSION 2026-11-18.1, records camera), via bin/r.mjs. New: a {"do":"js","file":…} step runs a file with page / sleep / STATE / out.
- bin/batch.sh: N LIST SCD OUT RUN FORCE TMO, plus UIPS=<state file>.
- REST: aud/rest-c.sh <script.js> with CDP / TMO env (hdr-rest.js + nrunpg.mjs; K.req / K.add / K.del / K.use / K.B).
  - smc1-3.js: fromSpec(featureType, name, overrides) from /root/live-pw/kern/.specs-cache.json, plus a creator probe (qCreatedBy per feature,
    lastModifyingOperationId).
  - smshot.js: REST shadedviews with a custom viewMatrix.
  - epo.js: extrude part order; ww/s0.js + s1.js: Loft variants on a copy.
- qa39/: atpath.py <names> prints NEW / OLD / OURS at qa's failing path (rows.txt of qa run 1119-1610-eb3b06a8-uipr); out/ holds today's live takes.
- diag/diag.js: real-pointer (xdotool) submenu trials with a rAF log.

## In flight / queued to land (tools/tiers/land.sh --status, NOSHAPE_LAND_QUEUE_HOST=vibe2, from /root/live2-scratch/wt branch live2-rec)
- 60784916 drw-constraint-tangent retake (CHECKING at 21:09). cf70e676 sketch-vertical-key with camera (QUEUED). If either fails: fix forward.
- Landed today: ae7a6d55 trims (Shift+Z; trim-drag twice), 1da77c2a dimension-variable, 2b23e54b GTOL.

## Open threads someone may still ask for
- skasm / qa: sketch-line-chain rule. Live gives VERTICAL (the 11-03 reference holds). Only the chain (−144.866, 65.193) → (−122.366, 65.193) →
  (−122.366, 20.193) gives PERPENDICULAR. Offered skasm a ±1–3 px sweep of the 3rd click (pixel-angle tie-break?). qa wants the verdict when skasm
  settles it. Takes: live/skasm/feel-1119/line-chain/.
- psui: told live has NO safe triangle (live/psui/submenu-diag-1119/). If psui disputes, the diag kit reruns in one go on lane 2.
- studios: Replicate rule sent (copy always on Top's y 30 edge at (81,184)). Pro retakes REQUESTS 21:02 are for live (unclaimed).
- Open REQUESTS held by live2: NONE. Every row claimed by live2 is ticked (last: skasm 17:13, 18:10, 19:37; psui 16:37, 21:04; core 17:07).

## Scratch documents to delete forever when their owners are done (deldoc + purge)
- 30252aa1db52e1cc6e1e8b12 "live2 ww sections 1119" (core wave-washer sections).
- 92c09715ec48b5b2f26bc53b "live2 sm creators 1119" (skasm SM creators + epo A..E studios).
- 774e3a4879cb3bfa271dece1 (asm/skasm/psui captures): the lead said it stays open.
