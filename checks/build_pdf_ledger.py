#!/usr/bin/env python3
"""Rebuild the authenticated-PDF inventory in pdf_corpus_coverage.json.

The eight sources are purchased binaries that cannot be redistributed, so this
generator reads them from a directory given by RUST_PDF_CORPUS_ROOT and writes
only derived metadata: file digests, unit coordinates, and per-page text
digests. No substantial source text is emitted.

Enumeration rule (deterministic, and re-checked by validate.py):
  - a PDF with an outline contributes one unit per outline entry, in document
    order, carrying its title, depth, and destination page;
  - a PDF without an outline contributes one unit per page, because no
    finer structure is recoverable from the binary.

Usage:
    RUST_PDF_CORPUS_ROOT=/path/to/pdfs python3 checks/build_pdf_ledger.py
"""
import hashlib
import json
import os
import pathlib
import sys

try:
    from pypdf import PdfReader
except ImportError:  # pragma: no cover - tooling guard
    sys.exit("pypdf is required: pip install pypdf")

HERE = pathlib.Path(__file__).resolve().parent
LEDGER = HERE / "pdf_corpus_coverage.json"
REVIEW = HERE / "pdf_corpus_review.json"

# (slug, title, file name, pinned size in bytes, pinned SHA-256)
SOURCES = [
    ("black-hat-rust", "Black Hat Rust", "Black Hat Rust.pdf", 4086487,
     "63a99a1f2c36cf8ca5f430494f2c517af25c112f79202364abb815578ddb3fe4"),
    ("command-line-rust", "Command-Line Rust", "Command-Line Rust.pdf", 2412243,
     "8fdfd3e12771d3d17583810ac752dca6b708207575203885fa7b5efc10c94eec"),
    ("fullstack-rust", "Fullstack Rust", "Fullstack Rust.pdf", 2211377,
     "c46e30de610e560a5dbde27d3c6a38b060c77c3d03d1aaef3fdd71ae3bbe1468"),
    ("lets-get-rusty-cheat-sheet", "Lets Get Rusty Cheat Sheet",
     "Lets Get Rusty Cheat Sheet.pdf", 184513,
     "607709cd075904b05d1e808b622f2e8b477916a5f67ccb1e9edef2ecabd5b9d5"),
    ("rust-container-cheat-sheet", "Rust container cheat sheet",
     "Rust container cheat sheet.pdf", 39516,
     "78a2c41f81fca378b22921ce26daed2fb38faa5f4ab706789f2fd3ba43e28234"),
    ("rust-for-rustaceans", "Rust for Rustaceans", "Rust for Rustaceans.pdf", 12423910,
     "455e66c0b52773b6ae4a7118bcf9303d3a355d00f128b066aec9c29dee0532a0"),
    ("rust-in-action", "Rust in Action", "Rust in Action.pdf", 19173152,
     "448d5293a1ec045afe621754b1816688325e40c3ab10e046734c49847dd25fc1"),
    ("zero-to-production-in-rust", "Zero to Production in Rust",
     "Zero to Production in Rust.pdf", 2408159,
     "f122f6e8f68ecdbf8bd5f5b9f06506e73822a993bc71194530894a3c168cf47b"),
]


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def sha256_text(text):
    return sha256_bytes(text.encode("utf-8"))


def page_digests(reader):
    """SHA-256 of each page's extracted text, in page order."""
    digests = []
    for page in reader.pages:
        try:
            text = page.extract_text() or ""
        except Exception:
            text = ""
        digests.append(sha256_text(text))
    return digests


def walk_outline(reader, node, depth=0, out=None):
    if out is None:
        out = []
    for item in node:
        if isinstance(item, list):
            walk_outline(reader, item, depth + 1, out)
        else:
            try:
                page = reader.get_destination_page_number(item) + 1
            except Exception:
                page = None
            out.append((depth, str(item.title).strip(), page))
    return out


def enumerate_units(reader):
    """Returns (kind, [(depth, title, page), ...]) for one source."""
    try:
        outline = walk_outline(reader, reader.outline)
    except Exception:
        outline = []
    if outline:
        return "pdf-outline", outline
    return "pdf-page", [(0, f"page {n}", n) for n in range(1, len(reader.pages) + 1)]


def inventory_line(slug, ordinal, depth, page, title):
    return f"{slug}:{ordinal}:{depth}:{page}:{title}"


def build(root):
    sources_out = []
    units_out = []
    for slug, title, file_name, expected_bytes, expected_sha in SOURCES:
        path = root / file_name
        if not path.is_file():
            sys.exit(f"missing source binary: {path}")
        data = path.read_bytes()
        digest = sha256_bytes(data)
        if len(data) != expected_bytes or digest != expected_sha:
            sys.exit(f"{file_name}: binary does not match the pinned identity")

        reader = PdfReader(str(path))
        pages = page_digests(reader)
        kind, entries = enumerate_units(reader)

        lines = []
        for ordinal, (depth, unit_title, page) in enumerate(entries, start=1):
            unit_id = f"{slug}:{ordinal:04d}"
            page_digest = pages[page - 1] if page and 1 <= page <= len(pages) else None
            lines.append(inventory_line(slug, ordinal, depth, page, unit_title))
            units_out.append({
                "unit_id": unit_id,
                "source": slug,
                "ordinal": ordinal,
                "depth": depth,
                "title": unit_title,
                "page": page,
                "page_text_sha256": page_digest,
                "claim": f"{unit_title} (depth {depth} at {file_name} page {page})",
                "audit_disposition": "unreviewed",
                "disposition": "unreviewed",
                "mapped_rule_ids": [],
                "exact_difference": {
                    "kind": "unassessed",
                    "detail": "not compared with any rule; no semantic coverage credit",
                },
                "rationale_class": "pending-semantic-review",
                "supporting_evidence": [
                    f"source-sha256:{digest}",
                    f"page-text-sha256:{page_digest}",
                ],
                "executable_check": {
                    "applicability": "inventory-parity-only",
                    "assertion_id": "checks/validate.py::validate_pdf_corpus",
                },
                "remaining_uncertainty": "unit has not been read against the rule library",
                "semantic_review": {
                    "status": "unreviewed",
                    "reviewer": None,
                    "source_sha256": digest,
                },
            })

        sources_out.append({
            "slug": slug,
            "title": title,
            "file_name": file_name,
            "bytes": len(data),
            "sha256": digest,
            "pages": len(reader.pages),
            "unit_enumeration": kind,
            "unit_count": len(entries),
            "unit_inventory_sha256": sha256_text("\n".join(lines)),
            "page_inventory_sha256": sha256_text("\n".join(pages)),
        })

    aggregate = sha256_text(
        "\n".join(f"{s['slug']}:{s['unit_inventory_sha256']}" for s in sources_out)
    )
    return sources_out, units_out, aggregate


def main():
    root = os.environ.get("RUST_PDF_CORPUS_ROOT")
    if not root:
        sys.exit("set RUST_PDF_CORPUS_ROOT to the directory holding the eight PDFs")
    sources_out, units_out, aggregate = build(pathlib.Path(root))

    # Dispositions come from the reviewed source file, never from hand-edits of
    # this generated ledger. An entry is applied only to the unit it names, and
    # only while that unit's title and page digest are unchanged.
    review = json.loads(REVIEW.read_text(encoding="utf-8"))
    entries = {entry["unit_id"]: entry for entry in review["entries"]}
    by_id = {unit["unit_id"]: unit for unit in units_out}
    unknown = sorted(set(entries) - set(by_id))
    if unknown:
        sys.exit(f"review names units that no longer exist: {unknown}")

    applied = 0
    for unit_id, entry in entries.items():
        unit = by_id[unit_id]
        if entry["title"] != unit["title"] or entry["page_text_sha256"] != unit["page_text_sha256"]:
            sys.exit(
                f"{unit_id}: the reviewed unit changed; re-review it before "
                "reapplying its disposition"
            )
        unit["audit_disposition"] = entry["audit_disposition"]
        unit["disposition"] = entry["disposition"]
        unit["mapped_rule_ids"] = entry["mapped_rule_ids"]
        unit["exact_difference"] = entry["exact_difference"]
        unit["rationale_class"] = entry["rationale_class"]
        unit["executable_check"] = entry["executable_check"]
        unit["remaining_uncertainty"] = entry["remaining_uncertainty"]
        unit["semantic_review"] = {
            "status": "reviewed",
            "reviewer": entry["reviewer"],
            "reviewed_on": review["reviewed_on"],
            "source_sha256": unit["semantic_review"]["source_sha256"],
        }
        applied += 1
    carried = applied

    summary = {}
    for unit in units_out:
        summary[unit["disposition"]] = summary.get(unit["disposition"], 0) + 1

    ledger = {
        "schema_version": 1,
        "corpus": {
            "name": "authenticated Rust PDF corpus",
            "redistributable": False,
            "unit_count": len(units_out),
            "aggregate_inventory_sha256": aggregate,
            "extraction": {
                "engine": "pypdf 6.16.1",
                "mode": "PdfReader.extract_text() default layout",
                "generator": "checks/build_pdf_ledger.py",
                "enumeration": (
                    "one unit per outline entry; one unit per page where the "
                    "PDF carries no outline"
                ),
            },
        },
        "sources": sources_out,
        "summary": {key: summary[key] for key in sorted(summary)},
        "units": units_out,
    }
    LEDGER.write_text(json.dumps(ledger, indent=2) + "\n", encoding="utf-8")
    print(
        f"wrote {LEDGER.name}: {len(units_out)} units across {len(sources_out)} sources "
        f"({carried} reviewed dispositions applied)"
    )


if __name__ == "__main__":
    main()
