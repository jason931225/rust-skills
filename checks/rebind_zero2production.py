#!/usr/bin/env python3
"""Re-anchor the Zero To Production ledger to the authoritative PDF binary.

The ledger was built against a PDF with SHA-256 5de8b3ef…20f75e. Issue #1 names
f122f6e8…c168cf47b as authoritative and forbids transferring rows by title,
page, or ordinal — a different revision could carry the same heading on the same
page while saying something else.

This tool transfers rows only on *content* identity. It extracts every page of
the authoritative binary with the engine the ledger records (pypdf layout mode),
looks up each row's stored `page_text_sha256` among those pages, and requires:

  * every row with a page number matches exactly one page (no ambiguity), and
  * every match implies the same page offset (a single, uniform shift).

Under those conditions the two binaries hold byte-identical text for every
reviewed page, the dispositions were made against exactly those bytes, and the
only thing that changes is the page index. Anything weaker exits non-zero and
the ledger is left alone.

Usage:
    RUST_PDF_CORPUS_ROOT=/path/to/pdfs python3 checks/rebind_zero2production.py
"""
import collections
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
LEDGER = HERE / "zero2production_coverage.json"
FILE_NAME = "Zero to Production in Rust.pdf"
AUTHORITATIVE_SHA256 = "f122f6e8f68ecdbf8bd5f5b9f06506e73822a993bc71194530894a3c168cf47b"
LEGACY_SHA256 = "5de8b3ef43e1175f18579130c9bef9ef492f63c527deb5ac65a9a1bb6320f75e"


def page_digests(path):
    reader = PdfReader(str(path))
    digests = {}
    for number, page in enumerate(reader.pages, start=1):
        try:
            text = page.extract_text(extraction_mode="layout") or ""
        except Exception as exc:
            sys.exit(f"{path.name}: page {number} failed to extract: {exc}")
        digests.setdefault(hashlib.sha256(text.encode("utf-8")).hexdigest(), []).append(number)
    return digests, len(reader.pages)


def toc_digest(units):
    lines = [f"{u.get('section')}:{u.get('title')}:{u.get('page')}" for u in units]
    return hashlib.sha256("\n".join(lines).encode("utf-8")).hexdigest()


def main():
    root = os.environ.get("RUST_PDF_CORPUS_ROOT")
    if not root:
        sys.exit("set RUST_PDF_CORPUS_ROOT to the directory holding the authoritative PDF")
    path = pathlib.Path(root) / FILE_NAME
    if not path.is_file():
        sys.exit(f"missing source binary: {path}")
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != AUTHORITATIVE_SHA256:
        sys.exit(f"{FILE_NAME}: not the authoritative binary ({digest})")

    ledger = json.loads(LEDGER.read_text(encoding="utf-8"))
    units = ledger["units"]
    by_digest, page_count = page_digests(path)

    placed = [u for u in units if u.get("page")]
    offsets = collections.Counter()
    ambiguous = []
    unmatched = []
    for unit in placed:
        pages = by_digest.get(unit["page_text_sha256"], [])
        if not pages:
            unmatched.append(unit["section"])
        elif len(pages) > 1:
            ambiguous.append(unit["section"])
        else:
            offsets[pages[0] - unit["page"]] += 1

    already_bound = ledger["source"]["sha256"] == AUTHORITATIVE_SHA256
    if already_bound:
        # Verification mode: pages must already resolve with no shift.
        if unmatched or ambiguous or list(offsets) != [0]:
            sys.exit(
                f"ledger claims the authoritative binary but does not match it: "
                f"unmatched={len(unmatched)} ambiguous={len(ambiguous)} offsets={dict(offsets)}"
            )
        print(f"verified: {len(placed)} placed rows match the authoritative binary exactly")
        return

    if unmatched or ambiguous or len(offsets) != 1:
        sys.exit(
            "refusing to rebind: content identity is not established "
            f"(unmatched={len(unmatched)} ambiguous={len(ambiguous)} offsets={dict(offsets)})"
        )
    offset = next(iter(offsets))

    for unit in units:
        if unit.get("page"):
            unit["page"] += offset
    ledger["source"] = {
        "name": ledger["source"]["name"],
        "revision": ledger["source"]["revision"],
        "sha256": AUTHORITATIVE_SHA256,
        "physical_pages": page_count,
        "toc_unit_count": len(units),
        "toc_digest": toc_digest(units),
    }
    continuation = ledger["extraction"].get("unlisted_continuation_pages", [])
    ledger["extraction"] = {
        "engine": "pypdf layout mode",
        "generator": "checks/rebind_zero2production.py",
        "unlisted_continuation_pages": [page + offset for page in continuation],
    }
    ledger["rebinding"] = {
        "method": "page-text-digest identity against the authoritative binary",
        "superseded_sha256": LEGACY_SHA256,
        "authoritative_sha256": AUTHORITATIVE_SHA256,
        "page_offset": offset,
        "rows_with_page": len(placed),
        "rows_matched": len(placed),
        "rows_ambiguous": 0,
        "rows_unmatched": 0,
        "claim": (
            "every reviewed page's extracted text is byte-identical between the two "
            "binaries, so dispositions carry across by content identity; no row was "
            "transferred by title, page, or ordinal"
        ),
    }
    ledger["audit_status"] = {
        "semantic_status": "source-rebound",
        "reason": (
            "The ledger is now bound to the authoritative binary. Every placed row was "
            "matched to exactly one page whose extracted text is byte-identical to the "
            "page the disposition was written against, with a uniform page offset."
        ),
        "required_evidence": (
            "Re-run checks/rebind_zero2production.py against the authoritative binary; it "
            "verifies every placed row and fails if any page's text has changed."
        ),
    }
    LEDGER.write_text(json.dumps(ledger, indent=2) + "\n", encoding="utf-8")
    print(
        f"rebound {len(placed)} placed rows to the authoritative binary "
        f"(offset {offset:+d}); toc digest {ledger['source']['toc_digest']}"
    )


if __name__ == "__main__":
    main()
