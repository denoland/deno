```python
# tests/test_coverage_overlay.py
"""
Unit tests for the in-editor coverage overlay feature.
"""

import json
import os
import tempfile
from pathlib import Path
from unittest.mock import MagicMock, mock_open, patch

import pytest

from coverage_overlay import CoverageOverlay, CoverageOverlayError


class TestCoverageOverlay:
    """Happy path, errors and edge-cases for CoverageOverlay."""

    @pytest.fixture
    def fake_workspace(self):
        """Create a temporary directory that simulates a workspace."""
        with tempfile.TemporaryDirectory() as tmpdir:
            yield Path(tmpdir)

    @pytest.fixture
    def coverage_file(self, fake_workspace):
        """Write a fake .coverage.json file."""
        cov_path = fake_workspace / ".coverage.json"
        cov_data = {
            "files": {
                str(fake_workspace / "src" / "math.py"): {
                    "executed_lines": [1, 2, 5, 6, 7],
                    "missing_lines": [3, 4],
                }
            }
        }
        cov_path.write_text(json.dumps(cov_data))
        return cov_path

    @pytest.fixture
    def overlay(self, coverage_file):
        """Instantiate CoverageOverlay with the fake .coverage.json."""
        return CoverageOverlay(coverage_file)

    # ------------------------------------------------------------------ #
    # Happy path
    # ------------------------------------------------------------------ #

    def test_load_coverage(self, overlay, fake_workspace):
        """Ensure coverage data is correctly loaded into memory."""
        key = str(fake_workspace / "src" / "math.py")
        assert key in overlay.data
        assert overlay.data[key]["executed_lines"] == [1, 2, 5, 6, 7]
        assert overlay.data[key]["missing_lines"] == [3, 4]

    def test_is_line_covered(self, overlay, fake_workspace):
        """Return True for covered lines, False for uncovered."""
        key = str(fake_workspace / "src" / "math.py")
        assert overlay.is_line_covered(key, 2) is True
        assert overlay.is_line_covered(key, 3) is False

    def test_decorate_editor(self, overlay, fake_workspace):
        """Decorate editor returns correct gutter markers."""
        key = str(fake_workspace / "src" / "math.py")
        markers = overlay.decorate_editor(key, total_lines=10)
        assert markers["covered"] == [1, 2, 5, 6, 7]
        assert markers["uncovered"] == [3, 4]
        assert markers["partial"] == []

    # ------------------------------------------------------------------ #
    # Errors
    # ------------------------------------------------------------------ #

    def test_missing_coverage_file(self, fake_workspace):
        """Raise CoverageOverlayError when .coverage.json does not exist."""
        missing = fake_workspace / "missing.json"
        with pytest.raises(CoverageOverlayError, match="Coverage file not found"):
            CoverageOverlay(missing)

    def test_malformed_json(self, fake_workspace):
        """Raise CoverageOverlayError when JSON is invalid."""
        bad = fake_workspace / "bad.json"
        bad.write_text("not json")
        with pytest.raises(CoverageOverlayError, match="Invalid JSON"):
            CoverageOverlay(bad)

    def test_file_not_in_coverage(self, overlay):
        """Return empty markers for files absent from coverage data."""
        markers = overlay.decorate_editor("unknown.py", total_lines=5)
        assert markers == {"covered": [], "uncovered": [], "partial": []}

    # ------------------------------------------------------------------ #
    # Edge cases
    # ------------------------------------------------------------------ #

    def test_empty_coverage(self, fake_workspace):
        """Handle empty coverage gracefully."""
        empty = fake_workspace / "empty.json"
        empty.write_text('{"files": {}}')
        overlay = CoverageOverlay(empty)
        markers = overlay.decorate_editor("any.py", total_lines=1)
        assert markers == {"covered": [], "uncovered": [], "partial": []}

    def test_line_zero(self, overlay, fake_workspace):
        """Line numbers start at 1; line 0 is always uncovered."""
        key = str(fake_workspace / "src" / "math.py")
        assert overlay.is_line_covered(key, 0) is False

    def test_negative_line(self, overlay, fake_workspace):
 """Negative line numbers are always uncovered."""
        key = str(fake_workspace / "src" / "math.py")
        assert overlay.is_line_covered(key, -1) is False

    def test_is_file_tracked(self, overlay, fake_workspace):
        """Quick check for file presence in coverage data."""
        key = str(fake_workspace / "src" / "math.py")
        assert overlay.is_file_tracked(key) is True
        assert overlay.is_file_tracked("missing.py") is False

    # ------------------------------------------------------------------ #
    # Mocks for external dependencies
    # ------------------------------------------------------------------ #

    @patch("coverage_overlay.Path.exists")
    def test_race_condition_file_removed(self, mock_exists):
        """Simulate file vanishing between check and read."""
        mock_exists.return_value = True
        with patch("builtins.open", mock_open(read_data='{"files": {}}')):
            overlay = CoverageOverlay(Path("/tmp/cov.json"))
        mock_exists.return_value = False
        with pytest.raises(CoverageOverlayError, match="Coverage file not found"):
            overlay.reload()

    def test_reload(self, overlay, coverage_file, fake_workspace):
        """Reload picks up new coverage data."""
        key = str(fake_workspace / "src" / "math.py")
        assert overlay.is_line_covered(key, 3) is False

        # Simulate external update
        cov_data = json.loads(coverage_file.read_text())
        cov_data["files"][key]["executed_lines"].extend([3, 4])
        coverage_file.write_text(json.dumps(cov_data))

        overlay.reload()
        assert overlay.is_line_covered(key, 3) is True
```