"""Focused tests for Cargo JSON inventory failure modes."""

from __future__ import annotations

import json
import sys
import unittest
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from check_clippy_policy import (  # noqa: E402
    InventoryKey,
    PolicyFailure,
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
