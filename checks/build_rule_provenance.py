#!/usr/bin/env python3
"""Generate rule_provenance.json: where every rule in rules/ comes from.

A rule is either *ledger-mapped* — some source-coverage ledger names it, so its
provenance is already evidence-bound — or it carries a typed justification
recorded by a reviewer in rule_provenance_review.json.

Both halves are derived, never hand-edited: the ledger mapping is recomputed
from the coverage files on every run, and a review entry is refused if it names
a rule that does not exist or one the ledgers already cover.

Usage:
    python3 checks/build_rule_provenance.py
"""
import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
RULES = ROOT / "rules"
REVIEW = HERE / "rule_provenance_review.json"
OUT = HERE / "rule_provenance.json"

COVERAGE_LEDGERS = [
    "microsoft_guidelines_coverage.json",
    "zero2production_coverage.json",
    "rust_release_coverage.json",
    "microsoft_training_coverage.json",
    "pdf_corpus_coverage.json",
]

ALLOWED_CLASSES = {
    "rust-api-guidelines",
    "rust-reference",
    "rustonomicon",
    "std-docs",
    "cargo-book",
    "rust-performance-book",
    "clippy-docs",
    "crate-docs",
    "edition-guide",
    "synthesized-practice",
}


def rule_ids():
    return sorted(p.stem for p in RULES.glob("*.md"))


def ledger_mapped(known):
    """Every rule id any coverage ledger references, with the ledgers naming it."""
    mapped = {}
    for name in COVERAGE_LEDGERS:
        path = HERE / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        found = set(re.findall(r'"([a-z]+-[a-z0-9-]+)\.md"', text))
        found |= set(re.findall(r'"rules/([a-z0-9-]+)\.md"', text))
        found |= {m for m in re.findall(r'"([a-z]+-[a-z0-9-]+)"', text) if m in known}
        for rule in found & known:
            mapped.setdefault(rule, []).append(name)
    return {rule: sorted(names) for rule, names in mapped.items()}


def main():
    known = set(rule_ids())
    mapped = ledger_mapped(known)

    review = json.loads(REVIEW.read_text(encoding="utf-8"))
    justified = {entry["rule_id"]: entry for entry in review["entries"]}

    unknown = sorted(set(justified) - known)
    if unknown:
        sys.exit(f"review names rules that do not exist: {unknown}")
    overlap = sorted(set(justified) & set(mapped))
    if overlap:
        sys.exit(
            "these rules are already ledger-mapped and must not carry a written "
            f"justification: {overlap}"
        )
    missing = sorted(known - set(mapped) - set(justified))
    if missing:
        sys.exit(f"no provenance for: {missing}")

    entries = []
    for rule in rule_ids():
        if rule in mapped:
            entries.append({
                "rule_id": rule,
                "provenance": "ledger-mapped",
                "ledgers": mapped[rule],
            })
        else:
            record = justified[rule]
            entries.append({
                "rule_id": rule,
                "provenance": "typed-justification",
                "source_class": record["source_class"],
                "justification": record["justification"],
                "reviewer": review["reviewer"],
            })

    by_class = {}
    for entry in entries:
        key = entry.get("source_class", "ledger-mapped")
        by_class[key] = by_class.get(key, 0) + 1

    OUT.write_text(json.dumps({
        "schema_version": 1,
        "generator": "checks/build_rule_provenance.py",
        "rule_count": len(entries),
        "ledger_mapped": len(mapped),
        "typed_justification": len(entries) - len(mapped),
        "summary": {key: by_class[key] for key in sorted(by_class)},
        "entries": entries,
    }, indent=2) + "\n", encoding="utf-8")
    print(
        f"wrote {OUT.name}: {len(entries)} rules "
        f"({len(mapped)} ledger-mapped, {len(entries) - len(mapped)} justified)"
    )


if __name__ == "__main__":
    main()
