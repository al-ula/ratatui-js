"""Reject release tags that do not match every package and crate version."""
import json
from pathlib import Path
import sys
import tomllib

from release_notes import release_notes, version_from_tag

root = Path(__file__).resolve().parents[1]
try:
    version = version_from_tag(sys.argv[1])
    release_notes((root / "CHANGELOG.md").read_text(encoding="utf-8"), sys.argv[1])
except (OSError, ValueError) as error:
    raise SystemExit(str(error)) from error
for file in sorted((root / "packages").glob("*/deno.json")):
    assert json.loads(file.read_text())["version"] == version, f"{file}: version does not match tag"
for file in sorted((root / "native").glob("*/Cargo.toml")):
    assert tomllib.loads(file.read_text())["package"]["version"] == version, f"{file}: version does not match tag"
print(f"Release versions match {sys.argv[1]}")
