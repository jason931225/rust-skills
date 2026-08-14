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

# Microsoft RustTraining source-unit inventory and disposition ledger.
MICROSOFT_TRAINING_COVERAGE = HERE / "microsoft_training_coverage.json"
EXPECTED_TRAINING_COMMIT = "9d19c482d66ef3995dca794bda74c7852134e0b7"
EXPECTED_TRAINING_AGGREGATE = (
    "df9e3cd5b41145ebae2c4440adc1024eda17f915522f19c2f292c9d77e6514ec"
)
# (group_id, book directory, unit count, unit inventory digest) at the pinned commit.
EXPECTED_TRAINING_GROUPS = [
    (
        "type-driven-correctness",
        "type-driven-correctness-book",
        240,
        "1a1ad624dd164b0cd0372bcb9b821c14af809c65de09e8aabcd86b15cba1f46f",
    ),
    (
        "rust-patterns",
        "rust-patterns-book",
        295,
        "377924b633a38952a63f00e262f70775e60632edcda82c718e1badfd33158cda",
    ),
    (
        "async",
        "async-book",
        142,
        "647749123a9a765401ab8790714654e51e90b7807e4767c53e16a452b2eff6bd",
    ),
    (
        "engineering",
        "engineering-book",
        181,
        "45460cda7ffe9d87961c2598d0a9cb154992fb63a996d9da7249918e68bafc5e",
    ),
    (
        "c-cpp",
        "c-cpp-book",
        468,
        "de38833cec43e3ae08b474122b7bee7bf14d4980f9580093ff475656cdda69e4",
    ),
    (
        "csharp",
        "csharp-book",
        477,
        "136c4a92d984a3938ccc91a7614288816ebfa9ef33c1df9ac4dfff63e441c166",
    ),
    (
        "python",
        "python-book",
        321,
        "852ba08c367ba4bbc0e98ada78b9768276c627a397672bccf68d996dada8d66f",
    ),
]
EXPECTED_TRAINING_UNITS = sum(group[2] for group in EXPECTED_TRAINING_GROUPS)
allowed_training_dispositions = {
    "unreviewed",
    "covered",
    "documented-deviation",
    "project-specific",
    "reject",
}
allowed_training_difference_kinds = {
    "unassessed",
    "no-difference",
    "extends-rule",
    "contradicts-rule",
    "out-of-scope",
}
allowed_training_rationale_classes = {
    "pending-semantic-review"
} | allowed_book_rationale_classes
allowed_training_applicability = {"inventory-parity-only", "behavior-assertion"}
training_heading_re = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
training_fence_re = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")
training_link_re = re.compile(r"\[[^]]+\]\(([^)#]+\.md)(?:#[^)]+)?\)")


def training_inventory_line(relative, ordinal, level, heading):
    """One RustTraining inventory line: path, per-file ordinal, level, heading."""
    return f"{relative}:{ordinal}:{level}:{heading}"


def microsoft_training_inventory_parity(rows_by_book):
    """Recompute each book's unit inventory digest from ledger rows, in order."""
    digests = {}
    for book, rows in rows_by_book.items():
        lines = []
        for row in rows:
            source_path = str(row.get("source_path", ""))
            _, _, relative = source_path.partition("/src/")
            heading_path = row.get("heading_path")
            heading = heading_path[-1] if isinstance(heading_path, list) and heading_path else ""
            lines.append(
                training_inventory_line(
                    relative,
                    row.get("ordinal"),
                    row.get("heading_level"),
                    heading,
                )
            )
        digests[book] = hashlib.sha256("\n".join(lines).encode()).hexdigest()
    return digests


def training_source_units(source_root, book):
    """Extract the SUMMARY digest, chapter digests, and source heading units."""
    src = source_root / book / "src"
    summary_bytes = (src / "SUMMARY.md").read_bytes()
    chapters = []
    for relative in training_link_re.findall(summary_bytes.decode(encoding="utf-8")):
        candidate = (src / relative).resolve()
        try:
            candidate.relative_to(src)
        except ValueError as exc:
            raise ValueError(
                f"{book} SUMMARY.md link escapes its src directory: {relative}"
            ) from exc
        if candidate not in chapters:
            chapters.append(candidate)
    units = []
    files = []
    for path in chapters:
        relative = path.relative_to(src).as_posix()
        data = path.read_bytes()
        files.append((relative, hashlib.sha256(data).hexdigest()))
        text = data.decode(errors="replace")
        file_lines = text.splitlines(keepends=True)
        headings = []
        heading_stack = []
        fence = None
        for number, line in enumerate(text.splitlines(), 1):
            fence_match = training_fence_re.match(line)
            if fence is None and fence_match:
                marker = fence_match.group(1)
                fence = (marker[0], len(marker))
                continue
            if fence is not None:
                if fence_match:
                    marker = fence_match.group(1)
                    remainder = fence_match.group(2)
                    if (
                        marker[0] == fence[0]
                        and len(marker) >= fence[1]
                        and not remainder.strip()
                    ):
                        fence = None
                continue
            heading_match = training_heading_re.match(line)
            if heading_match:
                level = len(heading_match.group(1))
                heading = heading_match.group(2)
                while heading_stack and heading_stack[-1][0] >= level:
                    heading_stack.pop()
                heading_stack.append((level, heading))
                headings.append(
                    (number, level, [item[1] for item in heading_stack])
                )
        for ordinal, (number, level, heading_path) in enumerate(headings, 1):
            following = (
                headings[ordinal][0] if ordinal < len(headings) else len(file_lines) + 1
            )
            end_line = following - 1
            body_end = end_line
            while body_end > number and not file_lines[body_end - 1].strip():
                body_end -= 1
            body = "".join(file_lines[number - 1 : body_end])
            units.append(
                (
                    relative,
                    ordinal,
                    level,
                    heading_path,
                    number,
                    end_line,
                    hashlib.sha256(body.encode()).hexdigest(),
                )
            )
    return hashlib.sha256(summary_bytes).hexdigest(), files, units


try:
    training = json.loads(MICROSOFT_TRAINING_COVERAGE.read_text(encoding="utf-8"))
except (OSError, json.JSONDecodeError) as exc:
    err(f"{MICROSOFT_TRAINING_COVERAGE.name}: cannot read coverage ledger: {exc}")
    training = None

if training is not None:
    training_source = training.get("source", {})
    training_groups = training.get("groups", [])
    training_units = training.get("units", [])
    training_books = [group[1] for group in EXPECTED_TRAINING_GROUPS]

    if training.get("schema_version") != 1:
        err(f"{MICROSOFT_TRAINING_COVERAGE.name}: expected schema version 1")
    if training_source.get("commit") != EXPECTED_TRAINING_COMMIT:
        err(
            f"{MICROSOFT_TRAINING_COVERAGE.name}: source commit is not the audited "
            f"{EXPECTED_TRAINING_COMMIT}"
        )
    if training_source.get("repository") != "https://github.com/microsoft/RustTraining":
        err(f"{MICROSOFT_TRAINING_COVERAGE.name}: source repository identity differs")
    if training_source.get("unit_count") != EXPECTED_TRAINING_UNITS or len(
        training_units
    ) != EXPECTED_TRAINING_UNITS:
        err(
            f"{MICROSOFT_TRAINING_COVERAGE.name}: expected exactly "
            f"{EXPECTED_TRAINING_UNITS} source units"
        )
    if training_source.get("book_count") != len(EXPECTED_TRAINING_GROUPS) or len(
        training_groups
    ) != len(EXPECTED_TRAINING_GROUPS):
        err(
            f"{MICROSOFT_TRAINING_COVERAGE.name}: expected "
            f"{len(EXPECTED_TRAINING_GROUPS)} book groups"
        )

    declared_groups = {}
    for group, (group_id, book, count, digest) in zip(
        training_groups, EXPECTED_TRAINING_GROUPS
    ):
        if group.get("group_id") != group_id or group.get("book") != book:
            err(
                f"{MICROSOFT_TRAINING_COVERAGE.name}: group order differs from the "
                f"audited inventory at {group.get('book')!r}"
            )
            continue
        declared_groups[book] = group
        if group.get("unit_count") != count:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {book} unit count differs")
        if group.get("unit_inventory_sha256") != digest:
            err(
                f"{MICROSOFT_TRAINING_COVERAGE.name}: {book} unit inventory digest "
                "differs from the audited source"
            )
        for field in ("summary_sha256", "chapter_inventory_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", str(group.get(field, ""))):
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {book} has no {field}")

    aggregate_digest = hashlib.sha256(
        "\n".join(
            f"{group.get('book')}:{group.get('unit_inventory_sha256')}"
            for group in training_groups
        ).encode()
    ).hexdigest()
    if (
        aggregate_digest != EXPECTED_TRAINING_AGGREGATE
        or training_source.get("aggregate_inventory_sha256") != EXPECTED_TRAINING_AGGREGATE
    ):
        err(
            f"{MICROSOFT_TRAINING_COVERAGE.name}: aggregate inventory digest differs "
            "from the audited source"
        )

    expected_book_order = [
        book
        for _, book, count, _ in EXPECTED_TRAINING_GROUPS
        for _ in range(count)
    ]
    if [unit.get("book") for unit in training_units] != expected_book_order:
        err(
            f"{MICROSOFT_TRAINING_COVERAGE.name}: unit rows do not preserve "
            "contiguous declared book order"
        )

    rows_by_book = {book: [] for book in training_books}
    seen_unit_ids = set()
    seen_positions = set()
    for unit in training_units:
        unit_id = unit.get("unit_id", "<missing>")
        book = unit.get("book")
        if book not in rows_by_book:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has unknown book {book!r}")
            continue
        rows_by_book[book].append(unit)
        if unit_id in seen_unit_ids:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: duplicate unit id {unit_id}")
        seen_unit_ids.add(unit_id)
        position = (unit.get("source_path"), unit.get("ordinal"))
        if position in seen_positions:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: duplicate source position {position}")
        seen_positions.add(position)

        source_path = str(unit.get("source_path", ""))
        if not source_path.startswith(f"{book}/src/") or not source_path.endswith(".md"):
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has invalid source path")
        for field in ("source_file_sha256", "unit_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", str(unit.get(field, ""))):
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no {field}")
        heading_path = unit.get("heading_path")
        if (
            not isinstance(heading_path, list)
            or not heading_path
            or not all(isinstance(item, str) and item.strip() for item in heading_path)
        ):
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no heading path")
        for field in ("ordinal", "heading_level", "chapter_index", "start_line", "end_line"):
            if not isinstance(unit.get(field), int) or unit[field] < 1:
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has invalid {field}")
        source_prefix = f"{book}/src/"
        if (
            source_path.startswith(source_prefix)
            and source_path.endswith(".md")
            and isinstance(unit.get("start_line"), int)
            and unit["start_line"] >= 1
        ):
            chapter = source_path[len(source_prefix) : -len(".md")]
            expected_unit_id = f"{book}:{chapter}:L{unit['start_line']}"
            if unit_id != expected_unit_id:
                err(
                    f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} does not match "
                    f"source coordinate {expected_unit_id}"
                )
        if not isinstance(unit.get("claim"), str) or not unit["claim"].strip():
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no claim")
        if (
            not isinstance(unit.get("remaining_uncertainty"), str)
            or not unit["remaining_uncertainty"].strip()
        ):
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no uncertainty record")

        disposition = unit.get("disposition")
        audit_disposition = unit.get("audit_disposition")
        if audit_disposition not in allowed_training_dispositions:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has invalid audit disposition")
        if disposition not in allowed_training_dispositions:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has invalid disposition")
        if unit.get("rationale_class") not in allowed_training_rationale_classes:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has invalid rationale class")

        # Every row carries a typed difference: an allowed kind and a nonempty
        # detail. An empty detail states nothing, so it is not a disposition.
        difference = unit.get("exact_difference")
        if (
            not isinstance(difference, dict)
            or difference.get("kind") not in allowed_training_difference_kinds
            or not isinstance(difference.get("detail"), str)
            or not difference["detail"].strip()
        ):
            err(
                f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no typed exact "
                "difference with a kind and a nonempty detail"
            )
            difference = {}

        evidence = unit.get("supporting_evidence")
        required_evidence = [
            f"rusttraining-commit:{EXPECTED_TRAINING_COMMIT}",
            f"source-file-sha256:{unit.get('source_file_sha256')}",
            f"unit-sha256:{unit.get('unit_sha256')}",
        ]
        if not isinstance(evidence, list) or any(
            item not in evidence for item in required_evidence
        ):
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} lacks source-bound evidence")

        check = unit.get("executable_check")
        if (
            not isinstance(check, dict)
            or check.get("applicability") not in allowed_training_applicability
        ):
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no check applicability")
            check = {}
        assertion_id = check.get("assertion_id")
        if not isinstance(assertion_id, str) or "::" not in assertion_id:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no assertion id")
        else:
            check_path, _, check_symbol = assertion_id.partition("::")
            resolved_check = ROOT / check_path
            keyword = "def" if resolved_check.suffix == ".py" else "fn"
            if not resolved_check.is_file():
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} check path is missing")
            elif not re.search(
                rf"\b{keyword}\s+{re.escape(check_symbol)}\s*\(",
                resolved_check.read_text(encoding="utf-8"),
            ):
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} check symbol is missing")

        review = unit.get("semantic_review")
        if not isinstance(review, dict) or review.get("status") not in {
            "unreviewed",
            "reviewed",
        }:
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no semantic review status")
            review = {}
        if review.get("source_sha256") != unit.get("source_file_sha256"):
            err(
                f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} review is not bound to the "
                "source file digest"
            )

        mapped = unit.get("mapped_rule_ids")
        if not isinstance(mapped, list):
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has invalid mapped rule ids")
            mapped = []
        for rule_id in mapped:
            if not isinstance(rule_id, str) or f"{rule_id}.md" not in rule_names:
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} maps unknown rule {rule_id!r}")

        if disposition == "unreviewed":
            # An unreviewed unit may not carry any coverage claim.
            if audit_disposition != "unreviewed":
                err(
                    f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is unreviewed but "
                    "claims an assessed audit disposition"
                )
            if mapped:
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is unreviewed but maps rules")
            if difference.get("kind") != "unassessed":
                err(
                    f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is unreviewed but claims "
                    "an assessed difference"
                )
            if unit.get("rationale_class") != "pending-semantic-review":
                err(
                    f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is unreviewed but claims "
                    "a resolved rationale class"
                )
            if check.get("applicability") != "inventory-parity-only":
                err(
                    f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is unreviewed but claims "
                    "a behavior assertion"
                )
            if review.get("status") != "unreviewed" or review.get("reviewer") is not None:
                err(
                    f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is unreviewed but records "
                    "a reviewer"
                )
        else:
            if audit_disposition == "unreviewed":
                err(
                    f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is reviewed but "
                    "retains an unreviewed audit disposition"
                )
            if not mapped and disposition in {"covered", "documented-deviation"}:
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is {disposition} without a rule")
            if difference.get("kind") == "unassessed":
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no exact difference")
            if unit.get("rationale_class") == "pending-semantic-review":
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} is dispositioned but unreviewed")
            if (
                review.get("status") != "reviewed"
                or not isinstance(review.get("reviewer"), str)
                or not review["reviewer"].strip()
            ):
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {unit_id} has no named reviewer")

    actual_training_summary = {
        disposition: sum(unit.get("disposition") == disposition for unit in training_units)
        for disposition in sorted(allowed_training_dispositions)
        if any(unit.get("disposition") == disposition for unit in training_units)
    }
    if training.get("summary") != actual_training_summary:
        err(f"{MICROSOFT_TRAINING_COVERAGE.name}: disposition summary differs from unit ledger")
    training_status = training.get("audit_status", {})
    if training_status.get("semantic_status") != "unreviewed-backlog":
        err(f"{MICROSOFT_TRAINING_COVERAGE.name}: semantic review status is not explicit")
    if not training_status.get("reason") or not training_status.get("required_evidence"):
        err(f"{MICROSOFT_TRAINING_COVERAGE.name}: semantic review evidence is incomplete")

    # Row order carries the inventory digest, so ordering defects must fail.
    for book, rows in rows_by_book.items():
        if len(rows) != declared_groups.get(book, {}).get("unit_count"):
            err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {book} row count differs from its group")
        chapter_index = 0
        current_path = None
        previous_ordinal = 0
        previous_line = 0
        for row in rows:
            if row.get("source_path") != current_path:
                chapter_index += 1
                current_path = row.get("source_path")
                previous_ordinal = 0
                previous_line = 0
            if row.get("chapter_index") != chapter_index:
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {row.get('unit_id')} has out-of-order chapter")
            if row.get("ordinal") != previous_ordinal + 1:
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {row.get('unit_id')} breaks ordinal order")
            if not isinstance(row.get("start_line"), int) or row["start_line"] <= previous_line:
                err(f"{MICROSOFT_TRAINING_COVERAGE.name}: {row.get('unit_id')} breaks line order")
            else:
                previous_line = row["start_line"]
            previous_ordinal = row.get("ordinal") if isinstance(row.get("ordinal"), int) else previous_ordinal + 1

    recomputed = microsoft_training_inventory_parity(rows_by_book)
    for group_id, book, count, digest in EXPECTED_TRAINING_GROUPS:
        if recomputed.get(book) != digest:
            err(
                f"{MICROSOFT_TRAINING_COVERAGE.name}: {book} rows do not reproduce the "
                "audited unit inventory digest"
            )

    # Optional source-backed run: recompute the whole inventory from a checkout.
    training_root_value = os.environ.get("MICROSOFT_RUSTTRAINING_ROOT")
    if not training_root_value:
        err(
            "MICROSOFT_RUSTTRAINING_ROOT is required to bind RustTraining row "
            "hashes to the pinned source checkout"
        )
    else:
        training_root = pathlib.Path(training_root_value).resolve()
        try:
            actual_training_commit = subprocess.run(
                ["git", "-C", str(training_root), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
        except (OSError, subprocess.CalledProcessError) as exc:
            err(f"RustTraining source checkout is not a readable git checkout: {exc}")
        else:
            if actual_training_commit != EXPECTED_TRAINING_COMMIT:
                err(
                    f"RustTraining source checkout is {actual_training_commit}, expected "
                    f"{EXPECTED_TRAINING_COMMIT}"
                )
        for group_id, book, count, digest in EXPECTED_TRAINING_GROUPS:
            try:
                summary_digest, files, source_units = training_source_units(
                    training_root, book
                )
            except (OSError, ValueError) as exc:
                err(f"RustTraining source checkout cannot be read for {book}: {exc}")
                continue
            source_digest = hashlib.sha256(
                "\n".join(
                    training_inventory_line(
                        relative, ordinal, level, heading_path[-1]
                    )
                    for relative, ordinal, level, heading_path, _, _, _ in source_units
                ).encode()
            ).hexdigest()
            if len(source_units) != count or source_digest != digest:
                err(f"RustTraining checkout inventory for {book} differs from the audited commit")
            chapter_digest = hashlib.sha256(
                "\n".join(f"{relative}:{file_digest}" for relative, file_digest in files).encode()
            ).hexdigest()
            group = declared_groups.get(book, {})
            if group.get("summary_sha256") != summary_digest:
                err(f"RustTraining checkout SUMMARY.md for {book} differs from the ledger")
            if (
                group.get("chapter_count") != len(files)
                or group.get("chapter_inventory_sha256") != chapter_digest
            ):
                err(f"RustTraining checkout chapters for {book} differ from the ledger")
            file_digests = dict(files)
            rows = rows_by_book.get(book, [])
            if len(rows) != len(source_units):
                err(f"RustTraining checkout unit count for {book} differs from the ledger")
                continue
            for row, source_unit in zip(rows, source_units):
                (
                    relative,
                    ordinal,
                    level,
                    heading_path,
                    start,
                    end,
                    unit_digest,
                ) = source_unit
                if (
                    row.get("source_path") != f"{book}/src/{relative}"
                    or row.get("ordinal") != ordinal
                    or row.get("heading_level") != level
                    or row.get("heading_path") != heading_path
                    or row.get("start_line") != start
                    or row.get("end_line") != end
                    or row.get("unit_sha256") != unit_digest
                    or row.get("source_file_sha256") != file_digests.get(relative)
                ):
                    err(
                        f"{MICROSOFT_TRAINING_COVERAGE.name}: {row.get('unit_id')} differs from "
                        "the source checkout"
                    )

if errors:
    print(f"VALIDATION FAILED ({len(errors)} problem(s)):\n")
    for e in errors:
        print(f"  - {e}")
    sys.exit(1)
print(f"OK: {len(rule_files)} rules valid; index lists all {len(linked)} of them.")
