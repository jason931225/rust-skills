# checks — verify rule structure, behavior, and examples

A dev tool that type-checks the ` ```rust ` code blocks in `../rules/*.md` so the
"Good" examples we tell agents to write actually compile. Focused Rust tests
also execute language and library behavior that compilation alone cannot prove.
Not part of the published skill.

## Run

```bash
# full local/CI gate (retrieves and verifies the pinned Microsoft source)
bash checks/check.sh

# individual focused behavior suites
cd checks
cargo test --test source_guidance
cargo test --test language_guidance

# individual example compile-check
python3 gen.py                                              # extract blocks -> examples/
cargo check --examples --keep-going --message-format=json > check.json
python3 analyze.py check.json                               # classify results
python3 analyze.py check.json --check-baseline baseline.txt # CI gate: exact reviewed parity
```

The full gate runs in CI (`.github/workflows/ci.yml`) on Rust 1.97.1. It
validates structure and source inventories, runs the source-guidance and
language-guidance suites, then compile-checks generated examples against the
reviewed baseline.

`validate.py` checks the 89-item Microsoft Pragmatic Rust Guidelines v2026.6
manifest against an exact source checkout: source IDs and hashes, mappings,
context pages, nested links, and known upstream defects must match. Focused
Rust tests execute selected runtime invariants; they are examples, not proof
that prose mappings are semantically complete. Static API guidance is checked
through manifest review, link/index parity, and extracted-example compilation.

The validator also checks the 431-unit *Zero To Production In Rust* disposition
ledger. Its PDF, TOC, page, and extraction digests identify the reviewed source
without locking mutable interpretations behind a second hard-coded checksum.
CI cannot redistribute or independently read the purchased PDF, so the ledger
records `blocked-source-reread` until that exact PDF is available for an
independent semantic review.

`microsoft_training_coverage.json` inventories all 2,124 semantic units of the
Microsoft *RustTraining* books at commit
`9d19c482d66ef3995dca794bda74c7852134e0b7`: type-driven correctness 240, Rust
patterns 295, async 142, engineering 181, C/C++ 468, C# 477, and Python 321.
A unit is one ATX heading outside fenced code blocks in a `SUMMARY.md`-linked
chapter of `<book>/src`. Chapters are ordered by first reference in
`SUMMARY.md`, and each unit records its source path, per-file ordinal, heading
path, line range, and the SHA-256 of its source lines. Only heading text is
retained; unit bodies are kept as digests, so the ledger is not a copy of the
upstream books.

Three digests pin that inventory. A book's `unit_inventory_sha256` is the
SHA-256 of its `<chapter-relative-path>:<ordinal>:<heading level>:<heading
text>` lines joined by LF; `chapter_inventory_sha256` is the same construction
over `<chapter-relative-path>:<file sha256>`; and
`aggregate_inventory_sha256` (`df9e3cd5b41145ebae2c4440adc1024eda17f915522f19c2f292c9d77e6514ec`)
is the SHA-256 of `<book>:<unit_inventory_sha256>` lines in declared book
order. `validate.py` recomputes all three from the ledger rows themselves, so
reordering, editing, dropping, or inventing a row fails the gate.

Every unit is `unreviewed`. That is a backlog state, not a coverage claim: no
unit has been read against the rule library, and a shared heading or topic is
not traceability. An unreviewed row must carry no mapped rule, an `unassessed`
typed `exact_difference`, the `pending-semantic-review` rationale class, the
`inventory-parity-only` executable applicability, and no reviewer — the
validator rejects any row that claims more. Moving a unit off `unreviewed`
requires an exact rule edge, a typed difference with detail, a named reviewer,
and a review bound to the source file digest; mapped rules must resolve to real
files in `rules/`.

Validation requires the source. Fetch the exact commit and point the validator
at it to recompute the whole inventory from the checkout — chapter set,
per-file digests, unit ordering, line ranges, and unit digests — instead of
trusting hashes repeated inside the ledger:

```bash
git clone https://github.com/microsoft/RustTraining /path/to/rusttraining
git -C /path/to/rusttraining checkout --detach 9d19c482d66ef3995dca794bda74c7852134e0b7
MICROSOFT_RUSTTRAINING_ROOT=/path/to/rusttraining python3 checks/validate.py
```

Without that checkout validation fails closed. Ledger-only digests can detect
accidental row drift, but they cannot independently prove that repeated source
and unit hashes still identify the pinned bytes.

For a standalone source-backed Microsoft validation, point the validator at an
exact checkout:

```bash
MICROSOFT_RUST_GUIDELINES_ROOT=/path/to/microsoft-rust-guidelines \
  python3 checks/validate.py
```

## Updating the baseline

`baseline.txt` lists the currently accepted suspects (fragments/pseudocode the
heuristics cannot auto-classify). The CI gate requires exact parity: a new
signature fails, and a stale signature fails so it cannot mask a later
regression after an example is repaired. After intentionally adding/changing
examples, regenerate it on the pinned toolchain and review every changed line:

```bash
rustup run 1.97.1 cargo check --examples --keep-going --message-format=json > check.json
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

`rust_release_coverage.json` inventories all 162 release-note entries from
Rust 1.95.0 through 1.97.1. Each entry records its source identity, claim,
disposition, mapped rules, exact difference, rationale class, evidence,
executable check, and remaining uncertainty. The validator parses the release
notes shipped by the pinned `rust-docs` component and requires exact inventory
parity; stabilization alone is recorded as reference material rather than
automatically becoming a recommendation.

`tests/language_guidance.rs` executes the language and standard-library
semantics referenced by the 1.95–1.97 refresh: if-let guard binding, atomic
update outcomes, single-branch cfg selection, total ordering in `BTreeMap`,
fallible integer-to-bool conversion, `NonZero` range iteration, and integer
bit-helper zero cases. It also checks that mutable sequence insertion returns
the inserted value for immediate initialization.

`analyze.py` buckets each failing example by compiler error code:

- **fragment** — every error is name resolution (undefined symbol/crate). These
  reference helpers defined elsewhere in the rule; expected, ignored.
- **artifact** — caused by extraction (a `&self` method body wrapped as a free
  fn, pseudocode `...`/`???` tokens, dangling doc comments). Not real bugs.
- **low** — only "type annotations needed"; compiles in the rule's real context.
- **SUSPECT** — anything else (type mismatch, no-method, bad syntax, wrong
  arity, missing trait impl). These are the ones to review and fix.

## Notes

- Run on Rust ≥ 1.97: the harness is pinned to 1.97.1 and `baseline.txt` is
  generated there. Older toolchains produce spurious failures (e.g. the
  `MaybeUninit` array `From` conversions, stable since 1.95).
- Generated files (`examples/`, `*.json`, `manifest.json`, `target/`) are
  gitignored; only the harness source and focused tests are tracked.
