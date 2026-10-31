#!/usr/bin/env python3
"""Unit tests for the restrictive and alternative SPDX metadata decisions."""
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().with_name("audit-licenses.py")
SPEC = importlib.util.spec_from_file_location("ferrixel_license_audit", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class LicenseMetadataTests(unittest.TestCase):
    def status(self, expression: str | None) -> str:
        return AUDIT.classify(expression)[0]

    def test_permissive_choices(self) -> None:
        for expression in (
            "MIT", "MIT OR Apache-2.0", "MIT/Apache-2.0",
            "BSD-3-Clause/MIT", "(MIT OR Apache-2.0) AND NCSA",
            "Apache-2.0 WITH LLVM-exception",
            "MIT OR LGPL-2.1-or-later",
        ):
            with self.subTest(expression=expression):
                self.assertEqual(self.status(expression), "METADATA_OK")

    def test_weak_copyleft_is_explicitly_reviewed(self) -> None:
        for expression in (
            "MPL-2.0",
            "MIT AND MPL-2.0",
            "(GPL-3.0-only OR MPL-2.0)",
        ):
            with self.subTest(expression=expression):
                self.assertEqual(self.status(expression), "REVIEW")

    def test_strong_copyleft_without_alternative_is_blocked(self) -> None:
        for expression in (
            "AGPL-3.0-only", "GPL-3.0-only AND MIT",
            "(GPL-2.0-only OR AGPL-3.0-only)",
        ):
            with self.subTest(expression=expression):
                self.assertEqual(self.status(expression), "BLOCK")

    def test_unrecognized_or_malformed_metadata_fails_closed(self) -> None:
        for expression in (
            None, "LicenseRef-Unknown", "MIT OR",
            "MIT AND (Apache-2.0", "MIT WITH Unknown-exception",
            "MIT??Apache-2.0",
        ):
            with self.subTest(expression=expression):
                self.assertEqual(self.status(expression), "REVIEW")


if __name__ == "__main__":
    unittest.main()
