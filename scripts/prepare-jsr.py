"""Merge verified native bundles into the JSR native wrapper before publishing."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import zipfile

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("bundles", type=Path)
parser.add_argument("--output", type=Path, default=root / "packages/native/artifacts.ts")
args = parser.parse_args()
version = json.loads((root / "packages/native/deno.json").read_text())["version"]
artifacts = {}
for file in sorted(args.bundles.rglob("*.zip")):
    with zipfile.ZipFile(file) as archive:
        metadata_files = [name for name in archive.namelist() if name.endswith("/metadata.json")]
        assert len(metadata_files) == 1, f"{file}: ambiguous metadata"
        prefix = metadata_files[0].removesuffix("metadata.json")
        metadata = json.loads(archive.read(metadata_files[0]))
        assert metadata["version"] == version, "Bundle version mismatch"
        assert metadata["verified"] is True, "Unverified bundles cannot be published"
        assert metadata["abiVersion"] == 1 and metadata["protocolVersion"] == 1, "Bundle contract mismatch"
        for line in archive.read(prefix + "SHA256SUMS").decode().splitlines():
            digest, name = line.split("  ", 1)
            assert hashlib.sha256(archive.read(prefix + name)).hexdigest() == digest, name
        target = metadata["target"]
        assert target in {"linux-x86_64", "darwin-aarch64", "windows-x86_64"}, target
        assert target not in artifacts, "Duplicate native target"
        filename = "ratatui_js_ffi.dll" if target.startswith("windows") else "libratatui_js_ffi.dylib" if target.startswith("darwin") else "libratatui_js_ffi.so"
        data = archive.read(prefix + f"packages/deno/native/{target}/{filename}")
        artifacts[target] = {"filename": filename, "sha256": hashlib.sha256(data).hexdigest(),
                             "base64": base64.b64encode(data).decode("ascii"),
                             "abiVersion": 1, "protocolVersion": 1}
assert artifacts, "No verified native bundles found"
args.output.write_text(
    'import type { NativeArtifact } from "./mod.ts";\n'
    'export const ARTIFACTS: Readonly<Record<string, NativeArtifact>> = '
    + json.dumps(artifacts) + ';\n')
print("JSR native assets staged:", ", ".join(artifacts))
