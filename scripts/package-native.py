"""Package a host-built native artifact and a self-contained Deno workspace."""
import argparse
import base64
import hashlib
import json
import platform
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--verified", action="store_true", help="Mark artifacts verified after a passing host integration suite")
parser.add_argument("--output", type=Path, default=root / "dist")
args = parser.parse_args()
os_name = {"Linux": "linux", "Darwin": "darwin", "Windows": "windows"}[platform.system()]
arch = {"x86_64": "x86_64", "AMD64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}[platform.machine()]
target = f"{os_name}-{arch}"
subprocess.run(["cargo", "build", "--manifest-path", str(root / "native/Cargo.toml"),
                "--locked", "--release", "-p", "ratatui-js-ffi"], check=True)
version = json.loads((root / "packages/deno/deno.json").read_text())["version"]
destination = args.output.resolve() / f"ratatui-js-{version}-{target}"
# Refuse to silently overwrite a release artifact.
destination.mkdir(parents=True, exist_ok=False)
for package in ["protocol", "core", "native", "deno"]:
    shutil.copytree(root / "packages" / package, destination / "packages" / package,
                    ignore=shutil.ignore_patterns("*_test.ts", "testing", "native"))
for filename in ["deno.json", "deno.lock", "LICENSE-MIT", "LICENSE-APACHE", "README.md"]:
    shutil.copy2(root / filename, destination / filename)
for package in ["protocol", "core", "native", "deno"]:
    for license in ["LICENSE-MIT", "LICENSE-APACHE"]:
        shutil.copy2(root / license, destination / "packages" / package / license)
shutil.copytree(root / "examples", destination / "examples")
shutil.copytree(root / "docs", destination / "docs")
shutil.copy2(root / "native/ffi/include/ratatui_js.h", destination / "ratatui_js.h")
filename = "ratatui_js_ffi.dll" if os_name == "windows" else "libratatui_js_ffi.dylib" if os_name == "darwin" else "libratatui_js_ffi.so"
native = destination / "packages/deno/native" / target
native.mkdir(parents=True)
shutil.copy2(root / "native/target/release" / filename, native / filename)
metadata = {"version": version, "abiVersion": 1, "protocolVersion": 1, "target": target,
            "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
            "deno": subprocess.check_output(["deno", "--version"], text=True).splitlines()[0],
            "verified": args.verified}
(destination / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
artifact_bytes = (native / filename).read_bytes()
artifact = {"filename": filename, "sha256": hashlib.sha256(artifact_bytes).hexdigest(),
            "base64": base64.b64encode(artifact_bytes).decode("ascii"),
            "abiVersion": 1, "protocolVersion": 1}
(destination / "packages/native/artifacts.ts").write_text(
    'import type { NativeArtifact } from "./mod.ts";\n'
    'export const ARTIFACTS: Readonly<Record<string, NativeArtifact>> = '
    + json.dumps({target: artifact}) + ';\n')
checksums = []
for file in sorted(destination.rglob("*")):
    if file.is_file():
        checksums.append(f"{hashlib.sha256(file.read_bytes()).hexdigest()}  {file.relative_to(destination).as_posix()}")
(destination / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
shutil.make_archive(str(destination), "zip", destination.parent, destination.name)
print(destination)
