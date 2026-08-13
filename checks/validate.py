#!/usr/bin/env python3
"""Structural / link / index validation for the rule library.

Checks:
  - every rules/<id>.md starts with `# <id>` matching its filename
  - has a `> ` one-line summary near the top
  - has `## Why It Matters` and `## See Also`
  - every `](other.md)` link resolves to an existing rule file
  - SKILL.md links exactly the set of files in rules/ (no broken links, no orphans)
  - source-coverage ledgers match their pinned source inventories

Exits non-zero (and prints every problem) if anything fails.
"""
import hashlib, json, os, pathlib, re, subprocess, sys
from html.parser import HTMLParser

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
RULES = ROOT / "rules"
SKILL = ROOT / "SKILL.md"
MICROSOFT_COVERAGE = HERE / "microsoft_guidelines_coverage.json"
RUST_RELEASE_COVERAGE = HERE / "rust_release_coverage.json"

errors = []
def err(msg): errors.append(msg)


class RustReleaseParser(HTMLParser):
    versions = {"1.95.0", "1.96.0", "1.96.1", "1.97.0", "1.97.1"}

    def __init__(self):
        super().__init__()
        self.heading = None
        self.buffer = []
        self.version = None
        self.section = "Patch"
        self.list_depth = 0
        self.links = []
        self.entries = []

    def handle_starttag(self, tag, attrs):
        if tag in {"h1", "h2"}:
            self.heading = tag
            self.buffer = []
        if tag == "li" and self.version in self.versions:
            self.list_depth += 1
            if self.list_depth == 1:
                self.buffer = []
                self.links = []
        if tag == "a" and self.list_depth == 1:
            href = dict(attrs).get("href")
            if href:
                self.links.append(href)

    def handle_endtag(self, tag):
        if tag == self.heading and tag in {"h1", "h2"}:
            text = " ".join("".join(self.buffer).split()).replace("§", "").strip()
            if tag == "h1":
                match = re.match(r"Version (1\.(?:95|96|97)\.\d)", text)
                if match:
                    self.version = match.group(1)
                    self.section = "Patch"
                elif text.startswith("Version 1.94"):
                    self.version = None
            else:
                self.section = text
            self.heading = None
            self.buffer = []
        if tag == "li" and self.list_depth:
            if self.list_depth == 1:
                claim = " ".join("".join(self.buffer).split())
                self.entries.append(
                    (self.version, self.section, claim, self.links.copy())
                )
            self.list_depth -= 1
            self.buffer = []

    def handle_data(self, data):
        if self.heading in {"h1", "h2"} or self.list_depth:
            self.buffer.append(data)

rule_files = sorted(RULES.glob("*.md"))
rule_names = {p.name for p in rule_files}

link_re = re.compile(r'\]\((?:\./)?([a-z0-9-]+\.md)\)')

for p in rule_files:
    text = p.read_text(encoding="utf-8")
    lines = text.splitlines()
    head = lines[0].strip() if lines else ""
    if head != f"# {p.stem}":
        err(f"{p.name}: first line is {head!r}, expected '# {p.stem}'")
    if not any(l.startswith("> ") for l in lines[:6]):
        err(f"{p.name}: missing '> ' summary line near the top")
    for section in ("## Why It Matters", "## Bad", "## Good", "## See Also"):
        if section not in text:
            err(f"{p.name}: missing '{section}' section")
    why_match = re.search(
        r"## Why It Matters\n\n(.+?)(?=\n## )",
        text,
        flags=re.DOTALL,
    )
    if why_match:
        why_body = " ".join(why_match.group(1).split())
        why_sentences = len(
            re.findall(
                r"""(?<=[.!?])(?:["'`)]*)\s+(?=[A-Z`#])""",
                why_body,
            )
        ) + 1
        if why_sentences not in range(2, 5):
            err(
                f"{p.name}: Why It Matters has {why_sentences} sentences, "
                "expected 2-4"
            )
    for tgt in link_re.findall(text):
        if tgt not in rule_names:
            err(f"{p.name}: broken link -> {tgt}")

# SKILL.md index parity
skill = SKILL.read_text(encoding="utf-8")
linked = set(re.findall(r'rules/([a-z0-9-]+\.md)', skill))
for tgt in sorted(linked):
    if tgt not in rule_names:
        err(f"SKILL.md: links missing file rules/{tgt}")
for name in sorted(rule_names):
    if name not in linked:
        err(f"SKILL.md: rule rules/{name} is not listed in the index")

# Source-coverage manifests.
ZERO2PROD_COVERAGE = HERE / "zero2production_coverage.json"
EXPECTED_ZERO2PROD_TOC_DIGEST = "bd500a774e3fa5a2d97adb2d6d8c167b201fe76ca33917bda1037e5fd9402d0f"
EXPECTED_ZERO2PROD_EXTRACT_DIGEST = "8a6efbac878df8c1b397e35aeda5d30b44b733c5458f03fc67b3d7ad45b8ef26"
EXPECTED_ZERO2PROD_SOURCE_SHA256 = "5de8b3ef43e1175f18579130c9bef9ef492f63c527deb5ac65a9a1bb6320f75e"
allowed_book_audit_dispositions = {
    "covered",
    "partial",
    "missing",
    "outdated",
    "project-specific",
    "reject",
}
allowed_book_final_dispositions = {
    "covered",
    "documented-deviation",
    "outdated",
    "project-specific",
    "reject",
}
allowed_book_rationale_classes = {
    "newer-rust-behavior",
    "obsolescence",
    "production-scalability",
    "provider-specific",
    "security",
    "source-aligned",
}
book_gap_re = re.compile(
    r"\b(no (?:current )?rule|never (?:states?|requires?|defines?|treats?|forbids?)|"
    r"missing(?: claim| clause| contract| caveat| half| rule)?|unstated|absent from|"
    r"does not (?:state|mention|require|define|cover|forbid|say)|"
    r"not (?:written|specified|covered)|gap remains)\b",
    re.IGNORECASE,
)
try:
    book_coverage = json.loads(ZERO2PROD_COVERAGE.read_text(encoding="utf-8"))
except (OSError, json.JSONDecodeError) as exc:
    err(f"{ZERO2PROD_COVERAGE.name}: cannot read coverage manifest: {exc}")
    book_coverage = None

if book_coverage is not None:
    units = book_coverage.get("units", [])
    if book_coverage.get("schema_version") != 2:
        err(f"{ZERO2PROD_COVERAGE.name}: expected schema version 2")
    expected_units = book_coverage.get("source", {}).get("toc_unit_count")
    source_sha256 = book_coverage.get("source", {}).get("sha256")
    source_toc_digest = book_coverage.get("source", {}).get("toc_digest")
    physical_pages = book_coverage.get("source", {}).get("physical_pages")
    declared_summary = book_coverage.get("summary")
    audit_status = book_coverage.get("audit_status", {})
    extract_entries = book_coverage.get("extraction", {}).get("chapter_extracts", [])
    continuation_pages = book_coverage.get("extraction", {}).get(
        "unlisted_continuation_pages"
    )
    sections = [unit.get("section") for unit in units if isinstance(unit, dict)]
    toc_lines = [
        f"{unit.get('section')}:{unit.get('title')}:{unit.get('page')}"
        for unit in units
    ]
    toc_digest = hashlib.sha256("\n".join(toc_lines).encode()).hexdigest()
    extract_lines = [
        f"{item.get('name')}:{item.get('sha256')}"
        for item in extract_entries
        if isinstance(item, dict)
    ]
    extract_digest = hashlib.sha256("\n".join(sorted(extract_lines)).encode()).hexdigest()
    if expected_units != 431 or len(units) != 431:
        err(f"{ZERO2PROD_COVERAGE.name}: expected exactly 431 TOC units")
    if source_sha256 != EXPECTED_ZERO2PROD_SOURCE_SHA256:
        err(f"{ZERO2PROD_COVERAGE.name}: unexpected PDF source digest")
    if physical_pages != 433:
        err(f"{ZERO2PROD_COVERAGE.name}: expected the audited 433-page PDF")
    if len(sections) != len(set(sections)):
        err(f"{ZERO2PROD_COVERAGE.name}: duplicate section dispositions")
    if toc_digest != EXPECTED_ZERO2PROD_TOC_DIGEST or source_toc_digest != toc_digest:
        err(f"{ZERO2PROD_COVERAGE.name}: TOC inventory differs from reviewed source")
    if len(extract_entries) != 11 or extract_digest != EXPECTED_ZERO2PROD_EXTRACT_DIGEST:
        err(f"{ZERO2PROD_COVERAGE.name}: chapter extraction evidence differs from source audit")
    if continuation_pages != [272, 390]:
        err(f"{ZERO2PROD_COVERAGE.name}: unlisted continuation pages are not accounted for")
    if audit_status.get("semantic_status") != "blocked-source-reread":
        err(f"{ZERO2PROD_COVERAGE.name}: semantic source-reread status is not explicit")
    if not audit_status.get("reason") or not audit_status.get("required_evidence"):
        err(f"{ZERO2PROD_COVERAGE.name}: semantic source-reread evidence is incomplete")
    actual_summary = {
        disposition: sum(unit.get("disposition") == disposition for unit in units)
        for disposition in sorted(allowed_book_final_dispositions)
        if any(unit.get("disposition") == disposition for unit in units)
    }
    if declared_summary != actual_summary:
        err(f"{ZERO2PROD_COVERAGE.name}: disposition summary differs from unit ledger")
    for unit in units:
        section = unit.get("section", "<missing>")
        if unit.get("audit_disposition") not in allowed_book_audit_dispositions:
            err(f"{ZERO2PROD_COVERAGE.name}: {section} has invalid audit disposition")
        if unit.get("disposition") not in allowed_book_final_dispositions:
            err(f"{ZERO2PROD_COVERAGE.name}: {section} has unresolved final disposition")
        if unit.get("disposition") == "covered" and not unit.get("rule_paths"):
            err(f"{ZERO2PROD_COVERAGE.name}: {section} is covered without a mapped rule")
        if not re.fullmatch(r"[0-9a-f]{64}", str(unit.get("page_text_sha256", ""))):
            err(f"{ZERO2PROD_COVERAGE.name}: {section} has no source-page digest")
        if not isinstance(unit.get("rationale"), str) or not unit["rationale"].strip():
            err(f"{ZERO2PROD_COVERAGE.name}: {section} has no rationale")
        if (
            unit.get("disposition") == "covered"
            and book_gap_re.search(unit.get("rationale", ""))
        ):
            err(f"{ZERO2PROD_COVERAGE.name}: {section} is covered but admits a gap")
        rationale_class = unit.get("rationale_class")
        if rationale_class is not None and rationale_class not in allowed_book_rationale_classes:
            err(f"{ZERO2PROD_COVERAGE.name}: {section} has invalid rationale class")
        if (
            unit.get("disposition") == "documented-deviation"
            and rationale_class not in allowed_book_rationale_classes
        ):
            err(f"{ZERO2PROD_COVERAGE.name}: {section} has an untyped deviation")
        if unit.get("disposition") == "documented-deviation":
            for field in ("exact_difference", "remaining_uncertainty"):
                if not isinstance(unit.get(field), str) or not unit[field].strip():
                    err(f"{ZERO2PROD_COVERAGE.name}: {section} lacks {field}")
            evidence = unit.get("supporting_evidence")
            page_evidence = f"pdf-page-sha256:{unit.get('page_text_sha256')}"
            if not isinstance(evidence, list) or page_evidence not in evidence:
                err(f"{ZERO2PROD_COVERAGE.name}: {section} lacks source evidence")
            executable_check = unit.get("executable_check")
            if (
                not isinstance(executable_check, str)
                or not (ROOT / executable_check).is_file()
            ):
                err(f"{ZERO2PROD_COVERAGE.name}: {section} check is missing")
        for target in unit.get("rule_paths", []):
            target_path = pathlib.PurePosixPath(target) if isinstance(target, str) else None
            if (
                target_path is None
                or len(target_path.parts) != 2
                or target_path.parts[0] != "rules"
                or target_path.name not in rule_names
            ):
                err(f"{ZERO2PROD_COVERAGE.name}: {section} has invalid target {target!r}")

# Rust 1.95-1.97 release-note inventory and disposition parity.
try:
    rust_release_coverage = json.loads(
        RUST_RELEASE_COVERAGE.read_text(encoding="utf-8")
    )
except (OSError, json.JSONDecodeError) as exc:
    err(f"{RUST_RELEASE_COVERAGE.name}: cannot read coverage manifest: {exc}")
    rust_release_coverage = None

if rust_release_coverage is not None:
    release_source = rust_release_coverage.get("source", {})
    release_entries = rust_release_coverage.get("entries", [])
    release_ids = [
        entry.get("id") for entry in release_entries if isinstance(entry, dict)
    ]
    allowed_release_dispositions = {
        "covered",
        "documented-deviation",
        "provider-specific",
        "reference-only",
        "superseded",
    }
    expected_versions = ["1.95.0", "1.96.0", "1.96.1", "1.97.0", "1.97.1"]
    releases_html_override = os.environ.get("RUST_RELEASES_HTML")
    if releases_html_override:
        releases_html = pathlib.Path(releases_html_override)
    else:
        try:
            sysroot = subprocess.run(
                ["rustup", "run", "1.97.1", "rustc", "--print", "sysroot"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
        except (OSError, subprocess.CalledProcessError) as exc:
            err(f"{RUST_RELEASE_COVERAGE.name}: cannot locate Rust 1.97.1 docs: {exc}")
            releases_html = None
        else:
            releases_html = pathlib.Path(sysroot) / "share/doc/rust/html/releases.html"

    parsed_release_entries = []
    if releases_html is not None:
        try:
            releases_bytes = releases_html.read_bytes()
        except OSError as exc:
            err(f"{RUST_RELEASE_COVERAGE.name}: cannot read pinned release notes: {exc}")
        else:
            release_sha256 = hashlib.sha256(releases_bytes).hexdigest()
            if release_source.get("document_sha256") != release_sha256:
                err(f"{RUST_RELEASE_COVERAGE.name}: pinned release-note digest differs")
            parser = RustReleaseParser()
            parser.feed(releases_bytes.decode("utf-8"))
            ordinals = {}
            for version, section, claim, links in parser.entries:
                key = (version, section)
                ordinals[key] = ordinals.get(key, 0) + 1
                parsed_release_entries.append(
                    (version, section, ordinals[key], claim, links)
                )

    manifest_release_entries = []
    for entry in release_entries:
        source_identity = entry.get("source_identity", {})
        manifest_release_entries.append(
            (
                source_identity.get("version"),
                source_identity.get("section"),
                source_identity.get("ordinal"),
                entry.get("claim"),
                source_identity.get("links"),
            )
        )

    if rust_release_coverage.get("schema_version") != 1:
        err(f"{RUST_RELEASE_COVERAGE.name}: expected schema version 1")
    if release_source.get("rustc_version") != "1.97.1":
        err(f"{RUST_RELEASE_COVERAGE.name}: expected Rust 1.97.1 source")
    if release_source.get("rustc_commit") != "8bab26f4f68e0e26f0bb7960be334d5b520ea452":
        err(f"{RUST_RELEASE_COVERAGE.name}: unexpected Rust source commit")
    if release_source.get("versions") != expected_versions:
        err(f"{RUST_RELEASE_COVERAGE.name}: release version inventory differs")
    if release_source.get("entry_count") != 162 or len(release_entries) != 162:
        err(f"{RUST_RELEASE_COVERAGE.name}: expected exactly 162 release-note entries")
    if len(release_ids) != len(set(release_ids)):
        err(f"{RUST_RELEASE_COVERAGE.name}: duplicate release-note entry IDs")
    if parsed_release_entries and manifest_release_entries != parsed_release_entries:
        err(f"{RUST_RELEASE_COVERAGE.name}: entry inventory differs from pinned docs")

    release_summary = {
        disposition: sum(
            entry.get("disposition") == disposition for entry in release_entries
        )
        for disposition in sorted(allowed_release_dispositions)
        if any(
            entry.get("disposition") == disposition for entry in release_entries
        )
    }
    if rust_release_coverage.get("summary") != release_summary:
        err(f"{RUST_RELEASE_COVERAGE.name}: summary differs from entry ledger")

    for entry in release_entries:
        entry_id = entry.get("id", "<missing>")
        source_identity = entry.get("source_identity", {})
        disposition = entry.get("disposition")
        if not re.fullmatch(r"rust-1\.(?:95|96|97)\.\d-[a-z0-9-]+-\d{2}", str(entry_id)):
            err(f"{RUST_RELEASE_COVERAGE.name}: invalid entry ID {entry_id!r}")
        if source_identity.get("rustc_commit") != release_source.get("rustc_commit"):
            err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} has wrong source commit")
        if disposition not in allowed_release_dispositions:
            err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} has invalid disposition")
        if not isinstance(entry.get("claim"), str) or not entry["claim"].strip():
            err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} has no claim")
        if (
            not isinstance(entry.get("exact_difference"), str)
            or not entry["exact_difference"].strip()
        ):
            err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} has no exact difference")
        if entry.get("rationale_class") not in allowed_book_rationale_classes:
            err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} has invalid rationale class")
        if (
            not isinstance(entry.get("remaining_uncertainty"), str)
            or not entry["remaining_uncertainty"].strip()
        ):
            err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} has no uncertainty record")
        evidence = entry.get("supporting_evidence")
        if not isinstance(evidence, list) or release_source.get("document_url") not in evidence:
            err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} lacks source evidence")
        for link in source_identity.get("links", []):
            if link not in evidence:
                err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} omits source link")
        targets = entry.get("target_rules", [])
        if disposition in {"covered", "documented-deviation", "superseded"} and not targets:
            err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} has no mapped rule")
        for target in targets:
            target_path = pathlib.PurePosixPath(target) if isinstance(target, str) else None
            if (
                target_path is None
                or len(target_path.parts) != 2
                or target_path.parts[0] != "rules"
                or target_path.name not in rule_names
            ):
                err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} has invalid target")
        executable_check = entry.get("executable_check")
        if executable_check:
            check_path, separator, check_symbol = executable_check.partition("::")
            resolved_check = ROOT / check_path
            if not resolved_check.is_file():
                err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} check path is missing")
            elif separator and not re.search(
                rf"\bfn\s+{re.escape(check_symbol)}\s*\(",
                resolved_check.read_text(encoding="utf-8"),
            ):
                err(f"{RUST_RELEASE_COVERAGE.name}: {entry_id} check symbol is missing")

# Microsoft Pragmatic Rust Guidelines v2026.6 coverage parity.
try:
    coverage = json.loads(MICROSOFT_COVERAGE.read_text(encoding="utf-8"))
except (OSError, json.JSONDecodeError) as exc:
    err(f"{MICROSOFT_COVERAGE.name}: cannot read coverage manifest: {exc}")
else:
    source = coverage.get("source", {})
    entries = coverage.get("coverage", [])
    ids = [entry.get("id") for entry in entries if isinstance(entry, dict)]
    expected_commit = "bbf7b03f3a51548f187888fb8c516e8118ebb1c2"
    expected_id_digest = "c370753a70145bc180f03e63d0e47af4ad3c78482c43516e574c03a1074d141d"
    expected_source_digest = "ffa6c25aa19f756d4f9785711c2da4d92424c14dacea9bd29de88533e361508e"
    expected_mapping_digest = "26a9d2e2f9ea1bcbdfdc2fbfbb1653fc3701da96b1b15b7d9d009a207833af94"
    expected_context_digest = "24ed39c87495d4087c4fc89e94b237704bd47dbb6611d6d4613779b6c968a409"
    actual_id_digest = hashlib.sha256(
        "\n".join(sorted(str(item) for item in ids)).encode()
    ).hexdigest()

    if source.get("version") != "2026.6":
        err(f"{MICROSOFT_COVERAGE.name}: expected source version 2026.6")
    if source.get("commit") != expected_commit:
        err(f"{MICROSOFT_COVERAGE.name}: expected source commit {expected_commit}")
    if source.get("published_count") != 89 or len(entries) != 89:
        err(f"{MICROSOFT_COVERAGE.name}: expected exactly 89 coverage entries")
    if len(ids) != len(set(ids)):
        err(f"{MICROSOFT_COVERAGE.name}: duplicate guideline IDs")
    if actual_id_digest != expected_id_digest:
        err(f"{MICROSOFT_COVERAGE.name}: guideline ID set differs from v2026.6")

    tree_audit = coverage.get("tree_audit", {})
    expected_context_pages = {
        "src/SUMMARY.md",
        "src/guidelines/README.md",
        "src/guidelines/checklist/README.md",
        "src/guidelines/libs/README.md",
        "src/guidelines/safety/README.md",
        "src/agents/README.md",
        "src/changelog/README.md",
        "src/FAQ/README.md",
    }
    if tree_audit.get("normative_guideline_files") != 89:
        err(f"{MICROSOFT_COVERAGE.name}: full-tree guideline count must be 89")
    if tree_audit.get("include_bearing_section_readmes") != 13:
        err(f"{MICROSOFT_COVERAGE.name}: expected 13 include-bearing section indexes")
    context_entries = tree_audit.get("navigation_and_context_pages_reviewed", [])
    context_paths = {
        item.get("source_path") for item in context_entries if isinstance(item, dict)
    }
    if context_paths != expected_context_pages:
        err(f"{MICROSOFT_COVERAGE.name}: context-page audit set differs from v2026.6")
    allowed_dispositions = {"covered", "documented-deviation"}
    manifest_by_id = {}
    for entry in entries:
        guideline_id = entry.get("id", "<missing>")
        manifest_by_id[guideline_id] = entry
        source_path = entry.get("source_path")
        source_sha256 = entry.get("source_sha256")
        if not isinstance(source_path, str) or not source_path.startswith("src/guidelines/"):
            err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} has invalid source path")
        if not isinstance(source_sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", source_sha256):
            err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} has invalid source digest")
        if entry.get("disposition") not in allowed_dispositions:
            err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} has invalid disposition")
        if entry.get("disposition") == "documented-deviation":
            if entry.get("rationale_class") not in allowed_book_rationale_classes:
                err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} has untyped deviation")
            for field in ("exact_difference", "remaining_uncertainty"):
                if not isinstance(entry.get(field), str) or not entry[field].strip():
                    err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} lacks {field}")
            evidence = entry.get("supporting_evidence")
            if not isinstance(evidence, list) or entry.get("source_path") not in evidence:
                err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} lacks source evidence")
            executable_check = entry.get("executable_check")
            if (
                not isinstance(executable_check, str)
                or not (ROOT / executable_check).is_file()
            ):
                err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} check is missing")
        targets = entry.get("rules")
        if not isinstance(targets, list) or not targets:
            err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} has no mapped rules")
            continue
        for target in targets:
            target_path = pathlib.PurePosixPath(target) if isinstance(target, str) else None
            if (
                target_path is None
                or len(target_path.parts) != 2
                or target_path.parts[0] != "rules"
                or target_path.name not in rule_names
            ):
                err(f"{MICROSOFT_COVERAGE.name}: {guideline_id} has invalid target {target!r}")
                continue

    inventory_lines = [
        f"{entry.get('source_path')}:{entry.get('source_sha256')}"
        for entry in sorted(entries, key=lambda item: str(item.get("source_path")))
    ]
    manifest_source_digest = hashlib.sha256("\n".join(inventory_lines).encode()).hexdigest()
    if manifest_source_digest != expected_source_digest:
        err(f"{MICROSOFT_COVERAGE.name}: source inventory differs from pinned v2026.6")
    mapping_lines = [
        f"{entry.get('id')}:{entry.get('disposition')}:{','.join(sorted(entry.get('rules', [])))}"
        for entry in sorted(entries, key=lambda item: str(item.get("id")))
    ]
    mapping_digest = hashlib.sha256("\n".join(mapping_lines).encode()).hexdigest()
    if mapping_digest != expected_mapping_digest:
        err(f"{MICROSOFT_COVERAGE.name}: guideline-to-rule mappings differ from reviewed v2026.6")
    context_inventory_lines = [
        f"{item.get('source_path')}:{item.get('source_sha256')}"
        for item in context_entries
        if isinstance(item, dict)
    ]
    context_digest = hashlib.sha256(
        "\n".join(sorted(context_inventory_lines)).encode()
    ).hexdigest()
    if context_digest != expected_context_digest:
        err(f"{MICROSOFT_COVERAGE.name}: context-page inventory differs from pinned v2026.6")

    source_root_value = os.environ.get("MICROSOFT_RUST_GUIDELINES_ROOT")
    if not source_root_value:
        err("MICROSOFT_RUST_GUIDELINES_ROOT is required for source-backed coverage validation")
    else:
        source_root = pathlib.Path(source_root_value).resolve()
        try:
            actual_commit = subprocess.run(
                ["git", "-C", str(source_root), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
        except (OSError, subprocess.CalledProcessError) as exc:
            err(f"Microsoft source checkout is not a readable git checkout: {exc}")
        else:
            if actual_commit != expected_commit:
                err(f"Microsoft source checkout is {actual_commit}, expected {expected_commit}")

        guideline_root = source_root / "src" / "guidelines"
        source_files = sorted(guideline_root.rglob("M-*.md"))
        source_ids = [path.stem for path in source_files]
        if len(source_files) != 89 or len(source_ids) != len(set(source_ids)):
            err("Microsoft source checkout must contain exactly 89 unique M-*.md files")
        source_id_digest = hashlib.sha256("\n".join(sorted(source_ids)).encode()).hexdigest()
        if source_id_digest != expected_id_digest:
            err("Microsoft source checkout ID set differs from pinned v2026.6")

        actual_inventory_lines = []
        for path in source_files:
            relative = path.relative_to(source_root).as_posix()
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            actual_inventory_lines.append(f"{relative}:{digest}")
            entry = manifest_by_id.get(path.stem)
            if entry is None:
                err(f"{MICROSOFT_COVERAGE.name}: source file {relative} is not dispositioned")
                continue
            if entry.get("source_path") != relative:
                err(f"{MICROSOFT_COVERAGE.name}: {path.stem} source path differs from checkout")
            if entry.get("source_sha256") != digest:
                err(f"{MICROSOFT_COVERAGE.name}: {path.stem} source digest differs from checkout")
        actual_source_digest = hashlib.sha256(
            "\n".join(sorted(actual_inventory_lines)).encode()
        ).hexdigest()
        if actual_source_digest != expected_source_digest:
            err("Microsoft source checkout content differs from pinned v2026.6")

        include_re = re.compile(r"\{\{#include\s+(M-[A-Z0-9-]+\.md)\}\}")
        include_readmes = []
        included_files = []
        for readme in guideline_root.rglob("README.md"):
            includes = include_re.findall(readme.read_text(encoding="utf-8-sig"))
            if includes:
                include_readmes.append(readme)
                included_files.extend(
                    (readme.parent / include).resolve() for include in includes
                )
        if len(include_readmes) != 13:
            err("Microsoft source checkout must have 13 include-bearing section indexes")
        if len(included_files) != 89 or len(set(included_files)) != 89:
            err("Microsoft section indexes must include every guideline exactly once")
        if set(included_files) != {path.resolve() for path in source_files}:
            err("Microsoft section indexes and M-*.md source files differ")

        context_by_path = {
            item.get("source_path"): item for item in context_entries if isinstance(item, dict)
        }
        for relative in expected_context_pages:
            context_path = source_root / relative
            if not context_path.is_file():
                err(f"Microsoft source checkout context page missing: {relative}")
                continue
            digest = hashlib.sha256(context_path.read_bytes()).hexdigest()
            if context_by_path.get(relative, {}).get("source_sha256") != digest:
                err(f"Microsoft context page digest differs from checkout: {relative}")

        inline_link_re = re.compile(
            r'(?<!!)\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)'
        )
        reference_re = re.compile(r"^\[([^\]]+)\]:\s+(\S+)", re.MULTILINE)
        internal_links = []
        external_links = []
        for path in source_files:
            text = path.read_text(encoding="utf-8-sig")
            links = inline_link_re.findall(text)
            links.extend(href for _, href in reference_re.findall(text))
            for href in links:
                bucket = external_links if href.startswith(("http://", "https://")) else internal_links
                bucket.append((path, href))
        if len(internal_links) != 30:
            err("Microsoft source checkout internal-link count differs from pinned v2026.6")
        if len(external_links) != 84 or len({href for _, href in external_links}) != 73:
            err("Microsoft source checkout external-link inventory differs from pinned v2026.6")

        broken_refs = set()
        nested_link_sources = source_files + [
            source_root / relative for relative in expected_context_pages
        ]
        for path in nested_link_sources:
            text = path.read_text(encoding="utf-8-sig")
            resolved_reference_hrefs = {
                f"[{label}]": href for label, href in reference_re.findall(text)
            }
            candidates = [
                (href.partition("#")[2], href)
                for href in inline_link_re.findall(text)
                if not href.startswith(("http://", "https://"))
            ]
            for label, href in reference_re.findall(text):
                if href.startswith(("http://", "https://")):
                    continue
                candidates.append((label, href))
            for label, href in candidates:
                path_part, separator, anchor = href.partition("#")
                if not separator or not anchor.startswith("M-"):
                    continue
                target = (path.parent / path_part).resolve() if path_part else path.parent.resolve()
                target_dir = target.parent if target.is_file() else target
                if not (target_dir / f"{anchor}.md").is_file():
                    broken_refs.add((path.relative_to(source_root).as_posix(), label or anchor))
        manifest_broken_refs = {
            (item.get("source_path"), item.get("reference"))
            for item in tree_audit.get("source_link_defects", [])
            if isinstance(item, dict)
        }
        if broken_refs != manifest_broken_refs:
            err("Microsoft source-link defect ledger differs from pinned checkout")

if errors:
    print(f"VALIDATION FAILED ({len(errors)} problem(s)):\n")
    for e in errors:
        print(f"  - {e}")
    sys.exit(1)
print(f"OK: {len(rule_files)} rules valid; index lists all {len(linked)} of them.")
