#!/usr/bin/env python3
"""Conservative dependency-license metadata gate for the M1 feature set.

This is a screening step, NOT a substitute for source, patent, notice, and
redistribution review. Run from the repository root. Requires Python 3.10+.
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
    "CC0-1.0", "ISC", "MIT", "MIT-0", "Unlicense", "Unicode-3.0", "Zlib",
}
# Require explicit approval for copyleft or unusual source restrictions.
STRONG_COPYLEFT = ("GPL-", "AGPL-", "SSPL-")
REVIEW_COPYLEFT = ("MPL-", "LGPL-", "EUPL-")
TOKEN_RE = re.compile(r"[A-Za-z][A-Za-z0-9.\-+]*|0BSD")
OPERATORS = {"AND", "OR", "WITH"}


def classify(expression: str | None) -> tuple[str, str]:
    if not expression:
        return "REVIEW", "Missing SPDX license metadata"
    # The cargo metadata format may expose either SPDX or historic slash
    # shorthand; treat unknown syntax as unresolved, never guess a license.
    parts = TOKEN_RE.findall(expression)
    licenses = [x for x in parts if x not in OPERATORS]
    if not licenses:
        return "REVIEW", "No recognizable SPDX identifiers"
    if any(x.startswith(STRONG_COPYLEFT) for x in licenses):
        return "BLOCK", "Strong copyleft requires an explicit integration decision"
    if any(x.startswith(REVIEW_COPYLEFT) for x in licenses):
        return "REVIEW", "Weak/copyleft license: assess notice, source and file-level obligations"
    not_approved = sorted(set(licenses) - PERMISSIVE)
    if not_approved:
        return "REVIEW", "Unrecognized or non-allowlisted terms: " + ", ".join(not_approved)
    return "METADATA_OK", "Full license/NOTICE review still required"


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
    pending = [root]
    while pending:
        package_id = pending.pop()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        pending.extend(graph[package_id].get("dependencies", []))

    packages = [pkg for pkg in metadata["packages"] if pkg["id"] in reachable]
    packages.sort(key=lambda item: (item["name"], item["version"]))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    unresolved = []
    with OUT.open("w", encoding="utf-8", newline="") as file:
        writer = csv.writer(file)
        writer.writerow(["crate", "version", "license_spdx", "repository", "status", "note"])
        for package in packages:
            status, reason = classify(package.get("license"))
            writer.writerow([
                package["name"], package["version"], package.get("license", ""),
                package.get("repository", ""), status, reason,
            ])
            if status != "METADATA_OK":
                unresolved.append((package["name"], package["version"], status, reason))

    print(f"M1 metadata inventory: {len(packages)} crates; written to {OUT}")
    print(f"Strict metadata allowlist covers {len(packages) - len(unresolved)} crates.")
    if unresolved:
        for name, version, status, reason in unresolved:
            print(f"{status}: {name} {version}: {reason}", file=sys.stderr)
        print("Resolve licensing questions before a release candidate.", file=sys.stderr)
        if "--strict" in sys.argv or any(row[2] == "BLOCK" for row in unresolved):
            return 1
    else:
        print("All metadata expressions are in the provisional permissive allowlist.")

    print("Metadata screening is NOT proof of license compliance; review notices and source.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
