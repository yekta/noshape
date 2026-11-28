"""The comparer's rules: `python3 -I tools/ui-parity/test_compare.py` (no output when they hold)."""
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("run", Path(__file__).with_name("run.py"))
run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)

line = {"kind": "line", "points": [[0.0, 0.0], [40.0, 0.0]]}
spline = {"kind": "spline", "points": [[0.0, 50.0], [20.0, 70.0]], "derivatives": [[5.0, 5.0], [5.0, -5.0]]}
live = {"sketch": {"entities": ["line", "spline"], "geometry": [line, spline]}}

# Within 0.01 mm: the same.
ours = {"sketch": {"entities": ["line", "spline"], "geometry": [{**line, "points": [[0.004, 0.0], [40.0, -0.006]]}, spline]}}
assert run.first_difference(live, ours) is None, run.first_difference(live, ours)
# A moved handle: named at its derivative.
moved = {**spline, "derivatives": [[5.0, 5.0], [5.0, -4.0]]}
ours = {"sketch": {"entities": ["line", "spline"], "geometry": [line, moved]}}
assert run.first_difference(live, ours) == (".sketch.geometry[1].derivatives[1][1]", -5.0, -4.0), run.first_difference(live, ours)
assert [d[0] for d in run.all_differences(live, ours)] == [".sketch.geometry[1].derivatives[1][1]"]
# A recording without geometry (live's before UIS-STATE-GEOMETRY) compares the kinds only.
assert run.first_difference({"sketch": {"entities": ["line", "spline"]}}, ours) is None
# Each side's own order: sorted by kind and points before comparing.
ours = {"sketch": {"geometry": [spline, {**line, "points": [[0.0, 0.0], [40.0, 0.0]]}]}}
assert run.first_difference({"sketch": {"geometry": [line, spline]}}, ours) is None

# Live's click pixels (SKETCH-CLICK-ONE-PIXEL): a take from 11-26 14:05 on clicked its `at` (round(px)), per scenario
# step; an older one gave Chrome the fractional pixel (the app floors its own projection).
import json, tempfile
with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
    json.dump({"recorded": "2026-11-26T14:06:45.447Z", "steps": [{"step": 1, "at": [155, 58]}, {"step": 3, "do": "key"}, {"step": 4, "at": [1083, 562]}, {"step": 5, "at": [[1, 2], [3, 4]]}]}, f)
assert run.live_clicks(f.name) == ([[155, 58], None, None, [1083, 562]], True), run.live_clicks(f.name)
with open(f.name, "w") as g:
    json.dump({"recorded": "2026-11-26T13:56:18.350Z", "steps": [{"step": 1, "at": [155, 58]}]}, g)
assert run.live_clicks(f.name) == ([[155, 58]], False)
# Where live's pointer rested before each step: a click's `at`, then a key step's parked `pointer.at` (lead 11-26).
with open(f.name, "w") as g:
    json.dump({"steps": [{"step": 1, "at": [155, 58]}, {"step": 2, "at": [1083, 568]}, {"step": 3, "pointer": {"at": [1889, 932]}}, {"step": 4, "at": [1107, 516]}, {"step": 5, "do": "wait"}]}, g)
assert run.live_pointer_from(f.name) == [None, [155, 58], [1083, 568], [1889, 932], [1107, 516]], run.live_pointer_from(f.name)
Path(f.name).unlink()
# Remembered export settings per format (EXPORT-REMEMBERED-START): STL's own Units while the last export was STEP.
seed = {"Format": "STEP", "Options": "Store file in a new tab", "STL": {"STL Format": "Binary", "Units": "Millimeter", "Resolution": "Medium"}}
got = run.remembered_export(seed)
assert (got["format"], got["options"], got["stl_units"], got["stl_resolution"], got["stl_text"]) == ("STEP", 2, 4, 1, False), got
assert "mesh_resolution" not in run.remembered_export({"Format": "STL", "Resolution": "Fine"}), "STL's resolution is its own"
got = run.remembered_export({"Format": "OBJ", "STEP": {"Version": "AP214", "Use latest version": False, "Include derived tolerances in export": True}})
assert got["step"] == {"version": {"latest": False, "version": 1}, "derived_tolerances": True}, got
assert run.remembered_export({"Format": "PARASOLID", "Version": "37.1", "Use latest version": False})["parasolid"] == {"latest": False, "version": 1}
# Live's export toast names the start copy's file as its File name does.
for live_msg, ours in (("start spline-fillet-asm Copy 1 - Part 1 is being prepared for export. You will receive a notification with its status.", "Part Studio 1 - Part 1"),
                       ("start spline-fillet-asm asm Copy 1 is being prepared for export. You will receive a notification with its status.", "start spline-fillet-asm asm")):
    st = {"message": live_msg}
    run.normalize_live(st)
    assert st["message"] == ours + " is being prepared for export. You will receive a notification with its status.", st
