#!/usr/bin/env python3
"""Coverage of the feature-studio area by the interaction suite: tools/ui-parity/coverage_universe.py feature-studio."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import coverage_universe  # noqa: E402

sys.argv[1:1] = ["feature-studio"]
raise SystemExit(coverage_universe.main())
