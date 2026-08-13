# checks — verify rule structure, behavior, and examples

A dev tool that type-checks the ` ```rust ` code blocks in `../rules/*.md` so the
"Good" examples we tell agents to write actually compile. Focused Rust tests
also execute release-specific behavior that compilation alone cannot prove.
Not part of the published skill.

## Run

```bash
# structural / link / index checks (no toolchain needed)
python3 checks/validate.py

# execute release-specific behavior checks and compile-check the examples
cd checks
cargo test --test release_195_197
python3 gen.py                                              # extract blocks -> examples/
cargo check --examples --keep-going --message-format=json > check.json
python3 analyze.py check.json                               # classify results
python3 analyze.py check.json --check-baseline baseline.txt # CI gate: fail on NEW suspects
```

All run in CI (`.github/workflows/ci.yml`): structural validation, focused
release-behavior tests, and the generated example gate. Rust checks are pinned
to 1.97.1, the toolchain `baseline.txt` was generated on.

## Updating the baseline

`baseline.txt` lists the currently-accepted suspects (fragments/pseudocode the
heuristics can't auto-classify). The CI gate fails only on signatures *not* in
it. After intentionally adding/changing examples, regenerate it on the pinned
toolchain and review the diff:

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

`tests/release_195_197.rs` executes the release-specific semantics referenced
by the 1.95–1.97 refresh: if-let guard binding, atomic update outcomes,
single-branch cfg selection, total ordering in `BTreeMap`, fallible integer-to-
bool conversion, `NonZero` range iteration, and integer bit-helper zero cases.
It also checks that mutable sequence insertion returns the inserted value for
immediate initialization.

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
