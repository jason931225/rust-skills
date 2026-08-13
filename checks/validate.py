#!/usr/bin/env python3
"""Structural / link / index validation for the rule library.

Checks (no Rust toolchain needed):
  - every rules/<id>.md starts with `# <id>` matching its filename
  - has a `> ` one-line summary near the top
  - has `## Why It Matters` and `## See Also`
  - every `](other.md)` link resolves to an existing rule file
  - SKILL.md links exactly the set of files in rules/ (no broken links, no orphans)

Exits non-zero (and prints every problem) if anything fails.
"""
import hashlib, json, os, pathlib, re, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
RULES = ROOT / "rules"
SKILL = ROOT / "SKILL.md"
MICROSOFT_COVERAGE = HERE / "microsoft_guidelines_coverage.json"

errors = []
def err(msg): errors.append(msg)

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
    for section in ("## Why It Matters", "## See Also"):
        if section not in text:
            err(f"{p.name}: missing '{section}' section")
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
    expected_mapping_digest = "cf440e7818f27e00051fa4e078cceee6b1ced6821cca7a062215fd6d068d6201"
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
