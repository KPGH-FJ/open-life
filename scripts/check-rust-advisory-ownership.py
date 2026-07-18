#!/usr/bin/env python3
"""Fail closed unless cargo-audit warnings exactly match reviewed ownership."""

from __future__ import annotations

import argparse
import datetime as dt
import json
from pathlib import Path
from typing import Any


def load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("audit_json", type=Path)
    parser.add_argument("ownership_json", type=Path)
    args = parser.parse_args()

    audit = load_json(args.audit_json)
    ownership = load_json(args.ownership_json)
    errors: list[str] = []

    vulnerability_count = int(audit.get("vulnerabilities", {}).get("count", 0))
    if vulnerability_count:
        errors.append(f"cargo-audit reported {vulnerability_count} vulnerabilities")

    observed: dict[str, tuple[str, str, str]] = {}
    for kind, warning_list in audit.get("warnings", {}).items():
        for warning in warning_list:
            advisory_id = warning["advisory"]["id"]
            package = warning["package"]
            if advisory_id in observed:
                errors.append(f"duplicate observed advisory {advisory_id}")
            observed[advisory_id] = (kind, package["name"], package["version"])

    reviewed: dict[str, dict[str, Any]] = {}
    today = dt.date.today()
    for entry in ownership.get("entries", []):
        advisory_id = entry.get("id", "")
        if not advisory_id or advisory_id in reviewed:
            errors.append(f"missing or duplicate reviewed advisory id: {advisory_id!r}")
            continue
        reviewed[advisory_id] = entry
        for required in ("kind", "package", "version", "owner", "expires_on", "reachability", "remediation"):
            if not str(entry.get(required, "")).strip():
                errors.append(f"{advisory_id} is missing {required}")
        try:
            expires_on = dt.date.fromisoformat(entry["expires_on"])
        except (KeyError, TypeError, ValueError):
            errors.append(f"{advisory_id} has an invalid expires_on date")
        else:
            if expires_on < today:
                errors.append(f"{advisory_id} ownership expired on {expires_on.isoformat()}")

    for advisory_id in sorted(observed.keys() - reviewed.keys()):
        errors.append(f"unowned cargo-audit warning {advisory_id}: {observed[advisory_id]}")
    for advisory_id in sorted(reviewed.keys() - observed.keys()):
        errors.append(f"stale ownership entry must be deleted: {advisory_id}")
    for advisory_id in sorted(observed.keys() & reviewed.keys()):
        expected = reviewed[advisory_id]
        actual = observed[advisory_id]
        reviewed_tuple = (expected.get("kind"), expected.get("package"), expected.get("version"))
        if actual != reviewed_tuple:
            errors.append(
                f"{advisory_id} changed: observed={actual!r}, reviewed={reviewed_tuple!r}"
            )

    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"cargo-audit ownership verified for {len(observed)} informational warnings")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
