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
