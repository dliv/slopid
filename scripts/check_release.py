#!/usr/bin/env python3
"""Validate the exact tagged source before any release side effects (Python 3.11+)."""
import argparse
from pathlib import Path
import subprocess
import sys
import tomllib


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True,
                          text=True).stdout.strip()


def validate(tag):
    if git("status", "--porcelain", "--untracked-files=normal"):
        raise ValueError("release source must be clean, including untracked files")
    manifest = tomllib.loads(Path("Cargo.toml").read_text())
    version = manifest["package"]["version"]
    if not isinstance(version, str) or tag != f"v{version}":
        raise ValueError(f"tag {tag!r} does not match Cargo version {version!r}")
    try:
        tagged = git("rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}")
    except subprocess.CalledProcessError as error:
        raise ValueError(f"release tag {tag!r} is missing or not a commit") from error
    if tagged != git("rev-parse", "HEAD"):
        raise ValueError(f"release tag {tag!r} does not point to HEAD")
    outputs = {"version": version}

    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    args = parser.parse_args()
    try:
        outputs = validate(args.tag)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"release preflight: {error}", file=sys.stderr)
        return 1
    for key, value in outputs.items():
        print(f"{key}={value}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
