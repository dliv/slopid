"""Focused tests for Cargo JSON inventory failure modes."""

from __future__ import annotations

import json
import os
import shutil
import signal
import sys
import tempfile
import threading
import time
import unittest
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from check_clippy_policy import (  # noqa: E402
    InventoryKey,
    PolicyFailure,
    _run,
    _run_fixtures,
    compare_inventory,
    inventory_from_json,
)


REPOSITORY = Path(__file__).resolve().parents[2]


def cargo_diagnostic(
    *,
    file: str = "src/lib.rs",
    lint: str = "clippy::disallowed_methods",
    item: str = "std::env::current_dir",
    start: int = 10,
    end: int = 35,
    level: str = "warning",
) -> str:
    record = {
        "reason": "compiler-message",
        "message": {
            "level": level,
            "message": f"use of a disallowed method `{item}`",
            "code": {"code": lint},
            "spans": [
                {
                    "file_name": file,
                    "is_primary": True,
                    "line_start": 2,
                    "column_start": 1,
                    "line_end": 2,
                    "column_end": 10,
                    "byte_start": start,
                    "byte_end": end,
                }
            ],
        },
    }
    return json.dumps(record)


class ProcessLifecycleTests(unittest.TestCase):
    @unittest.skipUnless(
        os.name == "posix", "the interruption check uses POSIX process groups"
    )
    def test_keyboard_interrupt_terminates_and_reaps_owned_child(self) -> None:
        with tempfile.TemporaryDirectory() as scratch:
            scratch_path = Path(scratch)
            pid_file = scratch_path / "child.pid"
            source = (
                "import os, pathlib, sys, time; "
                "pathlib.Path(sys.argv[1]).write_text(str(os.getpid())); "
                "time.sleep(30)"
            )
            ready_to_interrupt = threading.Event()

            def interrupt_after_spawn() -> None:
                deadline = time.monotonic() + 2
                while time.monotonic() < deadline and not ready_to_interrupt.is_set():
                    if pid_file.exists():
                        try:
                            int(pid_file.read_text(encoding="utf-8"))
                        except (OSError, ValueError):
                            pass
                        else:
                            os.kill(os.getpid(), signal.SIGINT)
                            return
                    ready_to_interrupt.wait(0.01)

            interrupter = threading.Thread(target=interrupt_after_spawn, daemon=True)
            interrupter.start()
            child_pid: int | None = None
            try:
                with self.assertRaises(KeyboardInterrupt):
                    _run(
                        [sys.executable, "-c", source, str(pid_file)],
                        scratch_path,
                        os.environ.copy(),
                        "interruption regression fixture",
                        timeout=3,
                    )
                self.assertTrue(pid_file.is_file(), "child never started")
                child_pid = int(pid_file.read_text(encoding="utf-8"))
                with self.assertRaises(ProcessLookupError):
                    os.kill(child_pid, 0)
            finally:
                ready_to_interrupt.set()
                interrupter.join(timeout=1)
                if child_pid is not None:
                    try:
                        os.killpg(child_pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    try:
                        os.waitpid(child_pid, 0)
                    except ChildProcessError:
                        pass


class FixtureCargoConfigurationTests(unittest.TestCase):
    def test_repository_cargo_config_applies_to_fixture_controls(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository = root / "repository"
            scratch = root / "scratch"
            repository.mkdir()
            scratch.mkdir()
            (repository / "Cargo.toml").write_text(
                "[package]\n"
                'name = "policy-fixture"\n'
                'version = "0.1.0"\n'
                'edition = "2024"\n\n'
                "[lints.rust]\n"
                'unfulfilled_lint_expectations = "deny"\n\n'
                "[lints.clippy]\n"
                'disallowed_methods = "deny"\n'
                'disallowed_macros = "deny"\n',
                encoding="utf-8",
            )
            (repository / "Cargo.lock").write_text(
                "version = 4\n\n"
                "[[package]]\n"
                'name = "policy-fixture"\n'
                'version = "0.1.0"\n',
                encoding="utf-8",
            )
            shutil.copyfile(REPOSITORY / "clippy.toml", repository / "clippy.toml")
            cargo_config = repository / ".cargo" / "config.toml"
            cargo_config.parent.mkdir()
            cargo_config.write_text(
                '[build]\nrustflags = ["-Aunfulfilled_lint_expectations"]\n',
                encoding="utf-8",
            )

            stale_diagnostic = (
                "fixture 'stale-expectation' did not fail only for "
                "unfulfilled_lint_expectations"
            )
            with self.assertRaises(PolicyFailure) as failure:
                _run_fixtures(repository, scratch)
            self.assertIn(stale_diagnostic, str(failure.exception))
            self.assertIn("diagnostics: []", str(failure.exception))


class InventoryTests(unittest.TestCase):
    def test_duplicate_lib_and_lib_test_reports_are_one_compiled_site(self) -> None:
        repeated = cargo_diagnostic()
        actual = inventory_from_json(f"{repeated}\n{repeated}\n", REPOSITORY)
        self.assertEqual(
            actual,
            Counter(
                {
                    InventoryKey(
                        "src/lib.rs",
                        "clippy::disallowed_methods",
                        "std::env::current_dir",
                    ): 1
                }
            ),
        )

    def test_distinct_spans_for_the_same_item_are_both_counted(self) -> None:
        actual = inventory_from_json(
            f"{cargo_diagnostic(start=10, end=35)}\n"
            f"{cargo_diagnostic(start=50, end=75)}\n",
            REPOSITORY,
        )
        self.assertEqual(
            actual[
                InventoryKey(
                    "src/lib.rs",
                    "clippy::disallowed_methods",
                    "std::env::current_dir",
                )
            ],
            2,
        )

    def test_missing_and_extra_inventory_entries_fail(self) -> None:
        current_dir = InventoryKey(
            "src/lib.rs", "clippy::disallowed_methods", "std::env::current_dir"
        )
        process_exit = InventoryKey(
            "src/lib.rs", "clippy::disallowed_methods", "std::process::exit"
        )
        with self.assertRaises(PolicyFailure):
            compare_inventory(Counter(), Counter({current_dir: 1}))
        with self.assertRaises(PolicyFailure):
            compare_inventory(Counter({process_exit: 1}), Counter())

    def test_malformed_json_record_fails_closed(self) -> None:
        with self.assertRaises(PolicyFailure):
            inventory_from_json(
                '{"reason":"build-finished","success":true}\nnot-json\n',
                REPOSITORY,
            )

    def test_unrelated_compiler_error_is_not_counted_as_policy_failure(self) -> None:
        with self.assertRaises(PolicyFailure):
            inventory_from_json(
                cargo_diagnostic(
                    lint="E0425",
                    item="unresolved_name",
                    level="error",
                )
                + "\n",
                REPOSITORY,
            )

    def test_warning_with_an_unexpected_identity_fails_closed(self) -> None:
        with self.assertRaises(PolicyFailure):
            inventory_from_json(
                cargo_diagnostic(lint="clippy::needless_return") + "\n",
                REPOSITORY,
            )


if __name__ == "__main__":
    unittest.main()
