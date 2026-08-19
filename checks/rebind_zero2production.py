#!/usr/bin/env python3
"""Re-anchor the Zero To Production ledger to the authoritative PDF binary.

The ledger was built against a PDF with SHA-256 5de8b3ef…20f75e. Issue #1 names
f122f6e8…c168cf47b as authoritative and forbids transferring rows by title,
page, or ordinal — a different revision could carry the same heading on the same
page while saying something else.

This tool transfers rows only on *content* identity: it extracts every page of
the authoritative binary with the engine the ledger records, looks up each row's
stored `page_text_sha256` among those pages, and requires every row — including
the two that carry no page number — to match exactly one page, with a single
uniform offset for the placed ones.

What that proves, and what it does not: the rows reference 255 of the 433
physical pages. Those pages are byte-identical between the two binaries. The
remaining pages are *not* covered by any stored digest, because the superseded
binary is gone and no evidence about them survives. A disposition summarizes a
section, so a revision that edited a page inside a reviewed section without
touching its heading page would not be detected here. The ledger records that
scope explicitly rather than claiming a whole-document proof.

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
    import pypdf
    from pypdf import PdfReader
except ImportError:  # pragma: no cover - tooling guard
    sys.exit("pypdf is required: pip install pypdf==6.6.0")

HERE = pathlib.Path(__file__).resolve().parent
LEDGER = HERE / "zero2production_coverage.json"
FILE_NAME = "Zero to Production in Rust.pdf"
AUTHORITATIVE_SHA256 = "f122f6e8f68ecdbf8bd5f5b9f06506e73822a993bc71194530894a3c168cf47b"
LEGACY_SHA256 = "5de8b3ef43e1175f18579130c9bef9ef492f63c527deb5ac65a9a1bb6320f75e"
# The legacy digests were produced by this exact extractor. Text extraction
# output changes between pypdf releases, so another version would report every
# row unmatched against the correct binary.
REQUIRED_PYPDF = "6.6.0"
ENGINE = f"pypdf {REQUIRED_PYPDF} layout mode"


def page_digests(path):
    reader = PdfReader(str(path))
    by_digest = collections.defaultdict(list)
    for number, page in enumerate(reader.pages, start=1):
        try:
            text = page.extract_text(extraction_mode="layout") or ""
        except Exception as exc:
            sys.exit(f"{path.name}: page {number} failed to extract: {exc}")
        by_digest[hashlib.sha256(text.encode("utf-8")).hexdigest()].append(number)
    return by_digest, len(reader.pages)


def toc_digest(units):
    lines = [f"{u.get('section')}:{u.get('title')}:{u.get('page')}" for u in units]
    return hashlib.sha256("\n".join(lines).encode("utf-8")).hexdigest()


def binding_digest(units):
    """Pins each row to the page and bytes it was matched against, so a digest
    swapped offline fails validation even without the binary present."""
    lines = [
        f"{u.get('section')}:{u.get('page')}:{u.get('page_text_sha256')}" for u in units
    ]
    return hashlib.sha256("\n".join(lines).encode("utf-8")).hexdigest()


def match_rows(units, by_digest):
    """Returns (offsets, matched_pages, ambiguous, unmatched) over every row."""
    offsets = collections.Counter()
    matched_pages = set()
    ambiguous, unmatched = [], []
    for unit in units:
        pages = by_digest.get(unit["page_text_sha256"], [])
        if not pages:
            unmatched.append(unit["section"])
        elif len(pages) > 1:
            ambiguous.append(unit["section"])
        else:
            matched_pages.add(pages[0])
            if unit.get("page"):
                offsets[pages[0] - unit["page"]] += 1
    return offsets, matched_pages, ambiguous, unmatched


def main():
    if pypdf.__version__ != REQUIRED_PYPDF:
        sys.exit(
            f"pypdf {pypdf.__version__} is installed but the stored digests were produced "
            f"by {REQUIRED_PYPDF}; extraction output differs between releases"
        )
    root = os.environ.get("RUST_PDF_CORPUS_ROOT")
    if not root:
        sys.exit("set RUST_PDF_CORPUS_ROOT to the directory holding the authoritative PDF")
    path = pathlib.Path(root) / FILE_NAME
    if not path.is_file():
        sys.exit(f"missing source binary: {path}")
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != AUTHORITATIVE_SHA256:
        sys.exit(f"{FILE_NAME}: not the authoritative binary ({digest})")

    ledger = json.loads(LEDGER.read_text(encoding="utf-8"))
    units = ledger["units"]
    declared = ledger["source"]["sha256"]
    by_digest, page_count = page_digests(path)
    offsets, matched_pages, ambiguous, unmatched = match_rows(units, by_digest)

    if declared == AUTHORITATIVE_SHA256:
        # Verification mode: every row must still resolve, with no shift.
        if unmatched or ambiguous or list(offsets) != [0]:
            sys.exit(
                "ledger claims the authoritative binary but does not match it: "
                f"unmatched={len(unmatched)} ambiguous={len(ambiguous)} offsets={dict(offsets)}"
            )
        stored = ledger.get("rebinding", {}).get("binding_digest")
        if stored != binding_digest(units):
            sys.exit("row bindings have changed since the ledger was written")
        print(
            f"verified: all {len(units)} rows match the authoritative binary "
            f"({len(matched_pages)} of {page_count} pages proven identical)"
        )
        return

    # Rebinding mode is only for the one ledger this tool knows how to move.
    if declared != LEGACY_SHA256:
        sys.exit(
            f"refusing to rebind: the ledger names {declared}, which is neither the "
            "authoritative binary nor the superseded one this tool transfers from"
        )
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
        "engine": ENGINE,
        "generator": "checks/rebind_zero2production.py",
        "unlisted_continuation_pages": [page + offset for page in continuation],
    }
    ledger["rebinding"] = {
        "method": "page-text-digest identity against the authoritative binary",
        "engine": ENGINE,
        "superseded_sha256": LEGACY_SHA256,
        "authoritative_sha256": AUTHORITATIVE_SHA256,
        "page_offset": offset,
        "rows_total": len(units),
        "rows_matched": len(units),
        "rows_ambiguous": 0,
        "rows_unmatched": 0,
        "pages_proven_identical": len(matched_pages),
        "physical_pages": page_count,
        "binding_digest": binding_digest(units),
        "claim": (
            "every row's stored page text is byte-identical to exactly one page of the "
            "authoritative binary, so dispositions carry across by content identity; no "
            "row was transferred by title, page, or ordinal"
        ),
        "proof_scope": (
            f"{len(matched_pages)} of {page_count} physical pages are covered by a stored "
            "digest. The rest are not: the superseded binary is gone, so no evidence about "
            "them survives. A disposition summarizes a section, so an edit to a page inside "
            "a reviewed section that left its heading page untouched would not be detected."
        ),
    }
    ledger["audit_status"] = {
        "semantic_status": "source-rebound-partial-proof",
        "reason": (
            "The ledger is bound to the authoritative binary. Every one of its "
            f"{len(units)} rows matched exactly one page whose extracted text is "
            "byte-identical to the page the disposition was written against, with a "
            f"uniform page offset. That covers {len(matched_pages)} of {page_count} "
            "physical pages; the remainder carry no stored digest and are unproven."
        ),
        "required_evidence": (
            "Re-run checks/rebind_zero2production.py against the authoritative binary with "
            f"pypdf {REQUIRED_PYPDF}; it re-verifies every row and the binding digest. "
            "Closing the unproven-page gap requires a full reread, not a digest comparison."
        ),
    }
    LEDGER.write_text(json.dumps(ledger, indent=2) + "\n", encoding="utf-8")
    print(
        f"rebound {len(units)} rows (offset {offset:+d}); "
        f"{len(matched_pages)} of {page_count} pages proven identical; "
        f"binding digest {ledger['rebinding']['binding_digest']}"
    )


if __name__ == "__main__":
    main()
