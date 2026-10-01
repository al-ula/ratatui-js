"""Reject release tags that do not match every package and crate version."""
import json
from pathlib import Path
import re
import sys
import tomllib

root = Path(__file__).resolve().parents[1]
match = re.fullmatch(r"v([0-9]+\.[0-9]+\.[0-9]+)", sys.argv[1])
if not match:
    raise SystemExit("Release tags must have the form vMAJOR.MINOR.PATCH")
version = match.group(1)
for file in sorted((root / "packages").glob("*/deno.json")):
    assert json.loads(file.read_text())["version"] == version, f"{file}: version does not match tag"
for file in sorted((root / "native").glob("*/Cargo.toml")):
    assert tomllib.loads(file.read_text())["package"]["version"] == version, f"{file}: version does not match tag"
print(f"Release versions match {sys.argv[1]}")
