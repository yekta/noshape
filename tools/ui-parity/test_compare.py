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
