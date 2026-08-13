# checks — compile-verify the rule examples

A dev tool that type-checks the ` ```rust ` code blocks in `../rules/*.md` so the
"Good" examples we tell agents to write actually compile. Not part of the
published skill.

## Run

```bash
# full local/CI gate (retrieves and verifies the pinned Microsoft source)
bash checks/check.sh

# individual focused behavior test
cd checks
cargo test --test source_guidance

# individual example compile-check
python3 gen.py                                              # extract blocks -> examples/
cargo check --examples --keep-going --message-format=json > check.json
python3 analyze.py check.json                               # classify results
python3 analyze.py check.json --check-baseline baseline.txt # CI gate: fail on NEW suspects
```

The full gate runs in CI (`.github/workflows/ci.yml`): validation plus focused tests and
examples pinned to Rust 1.95.0 (the toolchain `baseline.txt` was generated on).

`validate.py` also checks the 89-item Microsoft Pragmatic Rust Guidelines
v2026.6 coverage manifest at `microsoft_guidelines_coverage.json`: the pinned
source revision and ID set cannot drift, every item has a disposition, and
every mapped rule exists. It also records the non-rule navigation/context
pages, nested-link audit, and known broken references in the pinned upstream
tree so overlapping guidance is consolidated only after every source item is
read. The focused Rust test executes representative
contracts for reusable buffers, `Send` futures, semantic constructor groups,
redacted `Debug`, readiness transitions, restartable schema backfills, and
bounded correlation IDs, idempotency claim semantics, and capped retry
backoff. A focused socket round-trip prevents the HTTP black-box contract from
regressing into a bind-only tautology. Static API-shape guidance remains enforced through the
source-backed mapping plus the extracted-example compile gate rather than
tautological runtime tests.

The same validator checks `zero2production_coverage.json`: all 431 table-of-
contents units from the pinned 433-page PDF are independently dispositioned,
the TOC and final rule mapping digests are fixed, and every final state is
either covered or explicitly excluded as outdated, project-specific, or
non-normative. The purchased PDF is not redistributed; its SHA-256 and the 11
chapter-extraction digests identify the audited revision.

For a standalone structural run, point the validator at an exact checkout:

```bash
MICROSOFT_RUST_GUIDELINES_ROOT=/path/to/microsoft-rust-guidelines \
  python3 checks/validate.py
```

## Updating the baseline

`baseline.txt` lists the currently-accepted suspects (fragments/pseudocode the
heuristics can't auto-classify). The CI gate fails only on signatures *not* in
it. After intentionally adding/changing examples, regenerate it on the pinned
toolchain and review the diff:

```bash
rustup run 1.95.0 cargo check --examples --keep-going --message-format=json > check.json
python3 analyze.py check.json --emit-baseline > baseline.txt
```

When bumping the pinned toolchain in `ci.yml`, regenerate `baseline.txt` on the
same version in the same commit.

## How it works

`gen.py` extracts each candidate block into `examples/<name>.rs`, wrapping
fragments in an `async fn -> Result<...>` so `?` and `.await` type-check. It
skips blocks that can't compile standalone by design: `## Bad` anti-patterns,
nightly `#![feature]` gates, procedural-macro code, placeholder crate names
(`my_crate`, …), and bare `...` pseudocode.

`analyze.py` buckets each failing example by compiler error code:

- **fragment** — every error is name resolution (undefined symbol/crate). These
  reference helpers defined elsewhere in the rule; expected, ignored.
- **artifact** — caused by extraction (a `&self` method body wrapped as a free
  fn, pseudocode `...`/`???` tokens, dangling doc comments). Not real bugs.
- **low** — only "type annotations needed"; compiles in the rule's real context.
- **SUSPECT** — anything else (type mismatch, no-method, bad syntax, wrong
  arity, missing trait impl). These are the ones to review and fix.

## Notes

- Run on Rust ≥ 1.95: some examples use APIs stabilized in 1.95 (e.g. the
  `MaybeUninit` array `From` conversions) and will spuriously fail on older
  toolchains.
- Generated files (`examples/`, `*.json`, `manifest.json`, `target/`) are
  gitignored; only the source (`gen.py`, `analyze.py`, `Cargo.toml`) is tracked.
