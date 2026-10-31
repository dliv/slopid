#!/usr/bin/env python3
"""Prove Slopid's direct Clippy bans and their normal compiler wiring."""

from __future__ import annotations

import json
import os
import re
import shlex
import signal
import subprocess
import sys
import tempfile
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
EXPECTATIONS = ROOT / "scripts" / "clippy_policy_expectations.json"
COMMAND_TIMEOUT_SECONDS = 180
POLICY_LINTS = {
    "clippy::disallowed_methods": re.compile(
        r"use of a disallowed method `([^`]+)`"
    ),
    "clippy::disallowed_macros": re.compile(
        r"use of a disallowed macro `([^`]+)`"
    ),
}
KNOWN_CARGO_RECORDS = {
    "compiler-artifact",
    "compiler-message",
    "build-script-executed",
    "build-finished",
}
COMPILER_OVERRIDES = (
    "CLIPPY_CONF_DIR",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
)


class PolicyFailure(Exception):
    """A failed policy check with enough detail to diagnose its cause."""


@dataclass(frozen=True, order=True)
class InventoryKey:
    file: str
    lint: str
    item: str
    scope: str = "production"


@dataclass(frozen=True)
class CompilerDiagnostic:
    level: str
    code: str | None
    message: str
    spans: tuple[dict[str, Any], ...]


@dataclass(frozen=True)
class CommandResult:
    command: tuple[str, ...]
    cwd: Path
    returncode: int
    stdout: str
    stderr: str


def _scope_for_path(relative_file: str) -> str:
    first = Path(relative_file).parts[0]
    if first == "src":
        return "production"
    if first == "tests":
        return "test"
    return "other"


def _compiler_diagnostics(stdout: str) -> list[CompilerDiagnostic]:
    diagnostics: list[CompilerDiagnostic] = []
    for line_number, line in enumerate(stdout.splitlines(), 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise PolicyFailure(
                f"malformed Cargo JSON on stdout line {line_number}: {error}: {line!r}"
            ) from error
        if not isinstance(record, dict):
            raise PolicyFailure(
                f"Cargo JSON line {line_number} is not an object: {line!r}"
            )
        reason = record.get("reason")
        if reason not in KNOWN_CARGO_RECORDS:
            raise PolicyFailure(
                f"unknown Cargo JSON record on stdout line {line_number}: {reason!r}"
            )
        if reason != "compiler-message":
            continue
        message = record.get("message")
        if not isinstance(message, dict):
            raise PolicyFailure(
                f"Cargo compiler-message on stdout line {line_number} has no message object"
            )
        level = message.get("level")
        text = message.get("message")
        if not isinstance(level, str) or not isinstance(text, str):
            raise PolicyFailure(
                f"Cargo compiler-message on stdout line {line_number} has invalid level/message fields"
            )
        code_record = message.get("code")
        code = code_record.get("code") if isinstance(code_record, dict) else None
        if code is not None and not isinstance(code, str):
            raise PolicyFailure(
                f"Cargo compiler-message on stdout line {line_number} has an invalid diagnostic code"
            )
        spans = message.get("spans", [])
        if not isinstance(spans, list) or any(not isinstance(span, dict) for span in spans):
            raise PolicyFailure(
                f"Cargo compiler-message on stdout line {line_number} has invalid spans"
            )
        diagnostics.append(CompilerDiagnostic(level, code, text, tuple(spans)))
    return diagnostics


def _primary_span(diagnostic: CompilerDiagnostic) -> dict[str, Any]:
    primary = [span for span in diagnostic.spans if span.get("is_primary") is True]
    if len(primary) != 1:
        raise PolicyFailure(
            f"expected one primary source span for {diagnostic.code} {diagnostic.message!r}; found {len(primary)}"
        )
    span = primary[0]
    file_name = span.get("file_name")
    if not isinstance(file_name, str) or not file_name:
        raise PolicyFailure(f"primary span has no file name: {span!r}")
    return span


def _relative_source_path(file_name: str, repository: Path) -> str:
    reported = Path(file_name)
    absolute = reported.resolve() if reported.is_absolute() else (repository / reported).resolve()
    try:
        relative = absolute.relative_to(repository.resolve())
    except ValueError as error:
        raise PolicyFailure(
            f"policy diagnostic points outside the repository: {file_name!r}"
        ) from error
    return relative.as_posix()


def _span_identity(span: dict[str, Any], relative_file: str) -> tuple[Any, ...]:
    byte_start = span.get("byte_start")
    byte_end = span.get("byte_end")
    if isinstance(byte_start, int) and isinstance(byte_end, int):
        return relative_file, byte_start, byte_end
    coordinates = tuple(
        span.get(key)
        for key in ("line_start", "column_start", "line_end", "column_end")
    )
    if any(not isinstance(value, int) for value in coordinates):
        raise PolicyFailure(f"primary source span has no usable location: {span!r}")
    return (relative_file, *coordinates)


def inventory_from_json(stdout: str, repository: Path) -> Counter[InventoryKey]:
    """Extract unique direct policy sites from Cargo compiler-message records."""
    diagnostics = _compiler_diagnostics(stdout)
    observed: Counter[InventoryKey] = Counter()
    seen_spans: set[tuple[Any, ...]] = set()
    for diagnostic in diagnostics:
        if diagnostic.level == "error":
            raise PolicyFailure(
                f"compiler error during policy inventory ({diagnostic.code or 'no code'}): {diagnostic.message}"
            )
        if diagnostic.level != "warning":
            continue
        pattern = POLICY_LINTS.get(diagnostic.code or "")
        if pattern is None:
            raise PolicyFailure(
                f"unexpected warning identity during policy inventory ({diagnostic.code or 'no code'}): {diagnostic.message}"
            )
        item_match = pattern.search(diagnostic.message)
        if item_match is None:
            raise PolicyFailure(
                f"could not read canonical banned item from {diagnostic.code}: {diagnostic.message}"
            )
        span = _primary_span(diagnostic)
        relative_file = _relative_source_path(span["file_name"], repository)
        identity = (
            *_span_identity(span, relative_file),
            diagnostic.code,
            item_match.group(1),
        )
        if identity in seen_spans:
            continue
        seen_spans.add(identity)
        observed[
            InventoryKey(
                relative_file,
                diagnostic.code,
                item_match.group(1),
                _scope_for_path(relative_file),
            )
        ] += 1
    return observed


def compare_inventory(
    observed: Counter[InventoryKey], expected: Counter[InventoryKey]
) -> None:
    """Fail if any expected site is missing or any unrecorded site appeared."""
    missing = expected - observed
    extra = observed - expected
    if not missing and not extra:
        return
    lines = ["compiled Clippy site inventory differs from the checked expectation:"]
    for key, count in sorted(missing.items()):
        lines.append(
            f"  missing {count}: {key.scope} {key.file} {key.lint} {key.item}"
        )
    for key, count in sorted(extra.items()):
        lines.append(
            f"  extra {count}: {key.scope} {key.file} {key.lint} {key.item}"
        )
    raise PolicyFailure("\n".join(lines))


def _read_site_group(
    entries: Any, scope: str
) -> Counter[InventoryKey]:
    if not isinstance(entries, list):
        raise PolicyFailure(f"expectation group {scope!r} must be a list")
    counts: Counter[InventoryKey] = Counter()
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise PolicyFailure(f"expectation {scope}[{index}] must be an object")
        file = entry.get("file")
        lint = entry.get("lint")
        item = entry.get("item")
        count = entry.get("count")
        if (
            not isinstance(file, str)
            or not isinstance(lint, str)
            or not isinstance(item, str)
            or not isinstance(count, int)
            or isinstance(count, bool)
            or count < 1
        ):
            raise PolicyFailure(f"expectation {scope}[{index}] has invalid fields: {entry!r}")
        if lint not in POLICY_LINTS:
            raise PolicyFailure(f"expectation {scope}[{index}] has unsupported lint {lint!r}")
        key = InventoryKey(file, lint, item, scope)
        if key in counts:
            raise PolicyFailure(f"duplicate inventory expectation: {key}")
        counts[key] = count
    return counts


def _load_expectations(path: Path) -> Counter[InventoryKey]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PolicyFailure(f"cannot read inventory expectations at {path}: {error}") from error
    if not isinstance(document, dict):
        raise PolicyFailure("inventory expectation document must be a JSON object")
    production = _read_site_group(document.get("production_sites"), "production")
    tests = _read_site_group(document.get("test_sites"), "test")
    combined = production + tests
    for field, lint in (
        ("production_macro_count", "production"),
        ("test_macro_count", "test"),
    ):
        count = document.get(field)
        if not isinstance(count, int) or isinstance(count, bool) or count < 0:
            raise PolicyFailure(f"{field} must be a non-negative integer")
        expected_count = sum(
            number
            for key, number in combined.items()
            if key.scope == lint and key.lint == "clippy::disallowed_macros"
        )
        if count != expected_count:
            raise PolicyFailure(
                f"{field} is {count}, but its inventory entries sum to {expected_count}"
            )
    return combined


def _clean_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for name in COMPILER_OVERRIDES:
        environment.pop(name, None)
    return environment


def _run(
    command: list[str],
    cwd: Path,
    environment: dict[str, str],
    purpose: str,
) -> CommandResult:
    try:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=(os.name == "posix"),
        )
    except OSError as error:
        raise PolicyFailure(
            f"could not start {purpose}: {shlex.join(command)}: {error}"
        ) from error
    try:
        stdout, stderr = process.communicate(timeout=COMMAND_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as error:
        if os.name == "posix":
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        else:
            try:
                process.kill()
            except ProcessLookupError:
                pass
        stdout, stderr = process.communicate()
        raise PolicyFailure(
            f"{purpose} timed out after {COMMAND_TIMEOUT_SECONDS}s: {shlex.join(command)}\n"
            f"stdout:\n{stdout}\nstderr:\n{stderr}"
        ) from error
    return CommandResult(tuple(command), cwd, process.returncode, stdout, stderr)


def _command_failure(result: CommandResult, detail: str) -> PolicyFailure:
    return PolicyFailure(
        f"{detail}\ncommand: {shlex.join(result.command)}\n"
        f"cwd: {result.cwd}\nexit: {result.returncode}\n"
        f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    )


def _require_fixture_result(
    result: CommandResult,
    expected_lint: str | None,
    case_name: str,
) -> None:
    try:
        diagnostics = _compiler_diagnostics(result.stdout)
    except PolicyFailure as error:
        raise _command_failure(result, str(error)) from error
    policy_levels = [
        diagnostic
        for diagnostic in diagnostics
        if diagnostic.level in {"warning", "error"}
    ]
    if expected_lint is None:
        if result.returncode != 0:
            raise _command_failure(
                result,
                f"fixture {case_name!r} exited {result.returncode}, expected 0",
            )
        if policy_levels:
            identities = [
                (diagnostic.level, diagnostic.code, diagnostic.message)
                for diagnostic in policy_levels
            ]
            raise _command_failure(
                result,
                f"fixture {case_name!r} produced unexpected diagnostics: {identities!r}",
            )
        return

    expected_diagnostics = [
        diagnostic
        for diagnostic in policy_levels
        if diagnostic.level == "error" and diagnostic.code == expected_lint
    ]
    unrelated = [
        diagnostic
        for diagnostic in policy_levels
        if diagnostic.level != "error" or diagnostic.code != expected_lint
    ]
    if result.returncode == 0 or not expected_diagnostics or unrelated:
        identities = [
            (diagnostic.level, diagnostic.code, diagnostic.message)
            for diagnostic in policy_levels
        ]
        raise _command_failure(
            result,
            f"fixture {case_name!r} did not fail only for {expected_lint}; diagnostics: {identities!r}",
        )


def _fixture_sources() -> dict[str, tuple[str, str | None]]:
    return {
        "valid": ("pub fn fixture() {}\n", None),
        "fulfilled-expectation": (
            "#[expect(clippy::disallowed_methods, reason = \"fixture exemption\")]\n"
            "pub fn fixture() { let _ = std::env::current_dir(); }\n",
            None,
        ),
        "aliased-current-dir": (
            "use std::env as process_env;\n"
            "pub fn fixture() { let _ = process_env::current_dir(); }\n",
            "clippy::disallowed_methods",
        ),
        "println-macro": (
            "pub fn fixture() { std::println!(\"fixture\"); }\n",
            "clippy::disallowed_macros",
        ),
        "stale-expectation": (
            "#[expect(clippy::disallowed_methods, reason = \"fixture must go stale\")]\n"
            "pub fn fixture() {}\n",
            "unfulfilled_lint_expectations",
        ),
    }


def _run_fixtures(repository: Path, scratch: Path) -> None:
    shared_target = scratch / "target"
    for case_name, (source, expected_lint) in _fixture_sources().items():
        fixture = scratch / case_name
        (fixture / "src" / "bin").mkdir(parents=True)
        for name in ("Cargo.toml", "Cargo.lock", "clippy.toml"):
            (fixture / name).write_bytes((repository / name).read_bytes())
        (fixture / "src" / "lib.rs").write_text(source, encoding="utf-8")
        (fixture / "src" / "bin" / "sid.rs").write_text("fn main() {}\n", encoding="utf-8")

        environment = _clean_environment()
        environment["CLIPPY_CONF_DIR"] = str(fixture.resolve())
        environment["CARGO_TARGET_DIR"] = str(shared_target.resolve())
        command = [
            "cargo",
            "clippy",
            "--locked",
            "--lib",
            "--message-format=json",
            "--",
            "-D",
            "warnings",
        ]
        result = _run(command, fixture, environment, f"{case_name} fixture Clippy")
        _require_fixture_result(
            result,
            expected_lint=expected_lint,
            case_name=case_name,
        )


def run_policy_control(repository: Path = ROOT) -> tuple[int, int]:
    expected = _load_expectations(EXPECTATIONS)
    environment = _clean_environment()
    environment["CLIPPY_CONF_DIR"] = str(repository.resolve())
    command = [
        "cargo",
        "clippy",
        "--locked",
        "--all-targets",
        "--message-format=json",
        "--",
        "-D",
        "warnings",
        "--force-warn",
        "clippy::disallowed_methods",
        "--force-warn",
        "clippy::disallowed_macros",
    ]
    result = _run(command, repository, environment, "root Clippy inventory")
    if result.returncode != 0:
        raise _command_failure(result, "root Clippy inventory command failed")
    try:
        observed = inventory_from_json(result.stdout, repository)
        compare_inventory(observed, expected)
    except PolicyFailure as error:
        raise _command_failure(result, str(error)) from error
    with tempfile.TemporaryDirectory(prefix="sid-clippy-policy-") as scratch_dir:
        _run_fixtures(repository, Path(scratch_dir))
    production_sites = sum(
        count for key, count in observed.items() if key.scope == "production"
    )
    test_sites = sum(count for key, count in observed.items() if key.scope == "test")
    return production_sites, test_sites


def main() -> int:
    try:
        production_sites, test_sites = run_policy_control()
    except PolicyFailure as error:
        print(f"Clippy policy control failed:\n{error}", file=sys.stderr)
        return 1
    print(
        "Clippy policy control passed: "
        f"{production_sites} production sites, {test_sites} test sites, "
        "0 production/test macro sites, 5 compile-only cases."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
