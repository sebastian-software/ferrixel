#!/usr/bin/env python3
"""SPDX-aware transitive dependency-license metadata screening for Ferrixel.

This is an early CI gate, NOT a substitute for inspecting the actual license
texts, notices, source files, distribution obligations and patent terms.
"""
from __future__ import annotations

import csv
import json
import re
import subprocess
import sys
from pathlib import Path

OUT = Path("target/m1-license-inventory.csv")
PERMISSIVE = {
    "0BSD", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "BSL-1.0",
    "CC0-1.0", "ISC", "MIT", "MIT-0", "NCSA", "Unlicense",
    "Unicode-3.0", "Zlib",
}
KNOWN_EXCEPTIONS = {"LLVM-exception"}
STRONG_COPYLEFT = ("AGPL-", "GPL-", "SSPL-")
REVIEW_COPYLEFT = ("EUPL-", "LGPL-", "MPL-")
TOKEN_RE = re.compile(r"\(|\)|[A-Za-z0-9][A-Za-z0-9.+-]*")
SCORE = {"METADATA_OK": 0, "REVIEW": 1, "BLOCK": 2}


def license_status(name: str) -> str:
    if name in PERMISSIVE:
        return "METADATA_OK"
    if name.startswith(STRONG_COPYLEFT):
        return "BLOCK"
    return "REVIEW"


class SpdxParser:
    """Small fail-closed parser for SPDX AND / OR / WITH and parentheses."""

    def __init__(self, expression: str) -> None:
        self.parts = TOKEN_RE.findall(expression)
        self.position = 0

    def take(self, wanted: str | None = None) -> str:
        if self.position >= len(self.parts):
            raise ValueError("unexpected end of SPDX expression")
        token = self.parts[self.position]
        if wanted is not None and token != wanted:
            raise ValueError(f"expected {wanted}, got {token}")
        self.position += 1
        return token

    def parse(self) -> str:
        result = self.parse_or()
        if self.position != len(self.parts):
            raise ValueError("unparsed SPDX tokens")
        return result

    def parse_or(self) -> str:
        result = self.parse_and()
        while self.position < len(self.parts) and self.parts[self.position] == "OR":
            self.take("OR")
            option = self.parse_and()
            # A recipient can elect any one branch of an SPDX OR.
            if SCORE[option] < SCORE[result]:
                result = option
        return result

    def parse_and(self) -> str:
        result = self.parse_term()
        while self.position < len(self.parts) and self.parts[self.position] == "AND":
            self.take("AND")
            requirement = self.parse_term()
            # SPDX AND requires compliance with every constituent license.
            if SCORE[requirement] > SCORE[result]:
                result = requirement
        return result

    def parse_term(self) -> str:
        if self.position >= len(self.parts):
            raise ValueError("missing SPDX license term")
        if self.parts[self.position] == "(":
            self.take("(")
            result = self.parse_or()
            self.take(")")
        else:
            name = self.take()
            if name in {"OR", "AND", "WITH", ")"}:
                raise ValueError("expected license identifier")
            result = license_status(name)

        if self.position < len(self.parts) and self.parts[self.position] == "WITH":
            self.take("WITH")
            exception = self.take()
            if exception not in KNOWN_EXCEPTIONS:
                result = "REVIEW" if result == "METADATA_OK" else result
        return result


def classify(expression: str | None) -> tuple[str, str]:
    if not expression:
        return "REVIEW", "Missing SPDX license metadata"
    try:
        # The tokenization must exactly cover the string after removing
        # whitespace; unrecognized symbols must not be ignored.
        tokens = TOKEN_RE.findall(expression)
        if "".join(tokens) != "".join(expression.split()):
            raise ValueError("unsupported SPDX syntax")
        status = SpdxParser(expression).parse()
    except ValueError as error:
        return "REVIEW", f"Unparsed SPDX expression: {error}"
    return {
        "METADATA_OK": (status, "Provisional SPDX choice; full notice review remains"),
        "REVIEW": (status, "Non-allowlisted/copyleft term or exception requires review"),
        "BLOCK": (status, "No acceptable license branch; manual integration decision"),
    }[status]


def main() -> int:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--all-features"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        sys.stderr.write(result.stderr)
        return result.returncode

    metadata = json.loads(result.stdout)
    root = metadata["resolve"]["root"]
    graph = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    reachable = set()
    queue = [root]
    while queue:
        item = queue.pop()
        if item in reachable:
            continue
        reachable.add(item)
        queue.extend(graph[item].get("dependencies", []))

    packages = [p for p in metadata["packages"] if p["id"] in reachable]
    packages.sort(key=lambda p: (p["name"], p["version"]))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    unresolved = []
    with OUT.open("w", newline="", encoding="utf-8") as output:
        writer = csv.writer(output)
        writer.writerow(["crate", "version", "license_spdx", "repository", "status", "note"])
        for package in packages:
            status, note = classify(package.get("license"))
            writer.writerow([
                package["name"], package["version"], package.get("license", ""),
                package.get("repository", ""), status, note,
            ])
            if status != "METADATA_OK":
                unresolved.append((package["name"], package["version"], status, note))

    print(f"M1 metadata inventory: {len(packages)} crates; written to {OUT}")
    print(f"Provisional SPDX choice available for {len(packages) - len(unresolved)} crates.")
    for name, version, status, note in unresolved:
        print(f"{status}: {name} {version}: {note}", file=sys.stderr)

    if unresolved:
        print("Manual license/source/notice review remains open.", file=sys.stderr)
        if "--strict" in sys.argv or any(status == "BLOCK" for _, _, status, _ in unresolved):
            return 1
    print("Metadata screening is NOT a complete license-compliance audit.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
