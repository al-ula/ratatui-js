"""Extract a tagged version's changelog section for GitHub Releases."""
import argparse
from pathlib import Path
import re


def version_from_tag(tag: str) -> str:
    number = r"(?:0|[1-9][0-9]*)"
    identifier = r"(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    match = re.fullmatch(
        rf"v({number}\.{number}\.{number}(?:-{identifier}(?:\.{identifier})*)?)", tag)
    if not match:
        raise ValueError("Release tags must have the form vMAJOR.MINOR.PATCH "
                         "with an optional prerelease such as -beta.1")
    return match.group(1)


def release_notes(changelog: str, tag: str) -> str:
    version = version_from_tag(tag)
    sections = []
    current = None
    fence = None
    for line in changelog.splitlines():
        # Headings in fenced examples are content, not section boundaries.
        marker = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if marker:
            delimiter = marker.group(1)
            if fence is None:
                fence = delimiter
            elif delimiter[0] == fence[0] and len(delimiter) >= len(fence):
                fence = None
        if fence is None and line.startswith("## "):
            heading = re.fullmatch(r"## \[([^]]+)\](?: - [0-9]{4}-[0-9]{2}-[0-9]{2})?\s*", line)
            current = [] if heading and heading.group(1) == version else None
            if current is not None:
                sections.append(current)
        elif current is not None:
            current.append(line)
    if len(sections) != 1:
        raise ValueError(f"Changelog must contain exactly one ## [{version}] section")
    notes = "\n".join(sections[0]).strip()
    if not any(line.strip() and not line.lstrip().startswith("#") for line in notes.splitlines()):
        raise ValueError(f"Changelog section for {version} must contain release notes")
    return notes + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    parser.add_argument("--changelog", type=Path,
                        default=Path(__file__).resolve().parents[1] / "CHANGELOG.md")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        notes = release_notes(args.changelog.read_text(encoding="utf-8"), args.tag)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")
    args.output.write_text(notes, encoding="utf-8")


if __name__ == "__main__":
    main()
