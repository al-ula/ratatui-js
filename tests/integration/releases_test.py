"""Release-gate failure tests; no registry access or repository writes."""
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile

root = Path(__file__).resolve().parents[2]


class ReleaseGates(unittest.TestCase):
    def test_changelog_release_notes(self):
        notes = "### Added\n\n- Core update with café support.\n\n```md\n## [9.0.0]\n```\n"
        for version in ["1.2.3", "1.2.3-beta.1", "1.2.3-rc.1"]:
            for dated in [False, True]:
                self.check_changelog_release_notes(version, dated, notes)

    def check_changelog_release_notes(self, version, dated, notes):
        heading = f"## [{version}]" + (" - 2026-10-01" if dated else "")
        with self.subTest(heading=heading), tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            changelog = directory / "CHANGELOG.md"
            output = directory / "notes.md"
            changelog.write_text(
                "# Changelog\n\n## [Unreleased]\n\n- Future work.\n\n"
                + heading + "\n\n" + notes + "\n## [1.2.2]\n\n- Older work.\n",
                encoding="utf-8")
            result = subprocess.run(
                [sys.executable, root / "scripts/release_notes.py", f"v{version}",
                 "--changelog", changelog, "--output", output], capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(output.read_text(encoding="utf-8"), notes)

    def test_invalid_changelog_does_not_write_notes(self):
        cases = [
            ("v1.2.3", "## [Unreleased]\n\n- Future work.\n", "exactly one"),
            ("v1.2.3", "## [1.2.3]\n\n### Added\n", "must contain release notes"),
            ("v1.2.3", "## [1.2.3]\n- One.\n## [1.2.3]\n- Two.\n", "exactly one"),
            ("v1.2.3-beta.01", "## [1.2.3]\n- One.\n", "Release tags"),
            ("v1.2.3-beta.", "## [1.2.3]\n- One.\n", "Release tags"),
            ("v01.2.3", "## [1.2.3]\n- One.\n", "Release tags"),
        ]
        for tag, content, error in cases:
            with self.subTest(content=content), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                changelog = directory / "CHANGELOG.md"
                output = directory / "notes.md"
                changelog.write_text(content, encoding="utf-8")
                output.write_text("unchanged", encoding="utf-8")
                result = subprocess.run(
                    [sys.executable, root / "scripts/release_notes.py", tag,
                     "--changelog", changelog, "--output", output],
                    capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(error, result.stderr)
                self.assertEqual(output.read_text(encoding="utf-8"), "unchanged")

    def test_version_tags(self):
        version = json.loads((root / "packages/deno/deno.json").read_text())["version"]
        for tag, valid in [(f"v{version}", True), ("main", False), ("v999.0.0", False),
                           (f"v{version}-rc.1", False)]:
            result = subprocess.run([sys.executable, root / "scripts/verify-release.py", tag],
                                    capture_output=True)
            self.assertEqual(result.returncode == 0, valid, tag)

    def test_only_verified_checksummed_bundles_are_staged(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            output = directory / "artifacts.ts"
            archive = directory / "bundle.zip"
            data = b"native fixture bytes"
            filename = "packages/deno/native/linux-x86_64/libratatui_js_ffi.so"
            version = json.loads((root / "packages/native/deno.json").read_text())["version"]
            for verified, checksum, valid in [(True, hashlib.sha256(data).hexdigest(), True),
                                              (False, hashlib.sha256(data).hexdigest(), False),
                                              (True, "0" * 64, False)]:
                metadata = {"version": version, "abiVersion": 1, "protocolVersion": 1,
                            "target": "linux-x86_64", "verified": verified}
                with zipfile.ZipFile(archive, "w") as bundle:
                    bundle.writestr("fixture/metadata.json", json.dumps(metadata))
                    bundle.writestr("fixture/" + filename, data)
                    bundle.writestr("fixture/SHA256SUMS", checksum + "  " + filename + "\n")
                output.write_text("unchanged")
                result = subprocess.run([sys.executable, root / "scripts/prepare-jsr.py", directory,
                                         "--output", output], capture_output=True)
                self.assertEqual(result.returncode == 0, valid)
                if valid:
                    self.assertIn(base64.b64encode(data).decode(), output.read_text())
                else:
                    self.assertEqual(output.read_text(), "unchanged")


if __name__ == "__main__":
    unittest.main()
