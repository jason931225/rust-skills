#!/usr/bin/env bash
# One command that reproduces CI locally. Run from anywhere:
#
#     bash checks/check.sh
#
# It runs the exact same gates CI runs, pinned to the same toolchain
# (checks/rust-toolchain.toml -> Rust 1.97.1) and the same compile target
# (x86_64-unknown-linux-gnu), so a green run here means a green run on CI.
# On non-x86 hosts (e.g. Apple Silicon) the examples are cross-checked for that
# target — `cargo check` type-checks without linking, so no cross-linker needed.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="x86_64-unknown-linux-gnu"
SOURCE_CACHE_ROOT="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/rust-skills-source-checkouts"
MICROSOFT_COMMIT="bbf7b03f3a51548f187888fb8c516e8118ebb1c2"
MICROSOFT_RUST_GUIDELINES_ROOT="${MICROSOFT_RUST_GUIDELINES_ROOT:-$SOURCE_CACHE_ROOT/microsoft-rust-guidelines-$MICROSOFT_COMMIT}"
MICROSOFT_TRAINING_COMMIT="9d19c482d66ef3995dca794bda74c7852134e0b7"
MICROSOFT_RUSTTRAINING_ROOT="${MICROSOFT_RUSTTRAINING_ROOT:-$SOURCE_CACHE_ROOT/microsoft-rusttraining-$MICROSOFT_TRAINING_COMMIT}"
# The seven book roots the RustTraining ledger is audited against; nothing else
# in that repository is read, so the checkout stays sparse.
MICROSOFT_TRAINING_BOOKS=(
    type-driven-correctness-book
    rust-patterns-book
    async-book
    engineering-book
    c-cpp-book
    csharp-book
    python-book
)

if [[ ! -d "$MICROSOFT_RUST_GUIDELINES_ROOT/.git" ]]; then
    git clone --filter=blob:none https://github.com/microsoft/rust-guidelines.git \
        "$MICROSOFT_RUST_GUIDELINES_ROOT"
fi
if [[ "$(git -C "$MICROSOFT_RUST_GUIDELINES_ROOT" remote get-url origin)" != \
    "https://github.com/microsoft/rust-guidelines.git" ]]; then
    echo "unexpected rust-guidelines origin" >&2
    exit 1
fi
if ! git -C "$MICROSOFT_RUST_GUIDELINES_ROOT" cat-file -e "$MICROSOFT_COMMIT^{commit}"; then
    # GitHub does not advertise arbitrary commit IDs to upload-pack. Fetch the
    # named branch that owns the pin, then verify the exact object below.
    git -C "$MICROSOFT_RUST_GUIDELINES_ROOT" fetch --filter=blob:none origin main
fi
git -C "$MICROSOFT_RUST_GUIDELINES_ROOT" cat-file -e "$MICROSOFT_COMMIT^{commit}"
git -C "$MICROSOFT_RUST_GUIDELINES_ROOT" checkout --detach "$MICROSOFT_COMMIT"
export MICROSOFT_RUST_GUIDELINES_ROOT

if [[ ! -d "$MICROSOFT_RUSTTRAINING_ROOT/.git" ]]; then
    git clone --filter=blob:none --no-checkout https://github.com/microsoft/RustTraining.git \
        "$MICROSOFT_RUSTTRAINING_ROOT"
    git -C "$MICROSOFT_RUSTTRAINING_ROOT" sparse-checkout init --cone
    git -C "$MICROSOFT_RUSTTRAINING_ROOT" sparse-checkout set "${MICROSOFT_TRAINING_BOOKS[@]}"
fi
if [[ "$(git -C "$MICROSOFT_RUSTTRAINING_ROOT" remote get-url origin)" != \
    "https://github.com/microsoft/RustTraining.git" ]]; then
    echo "unexpected RustTraining origin" >&2
    exit 1
fi
if ! git -C "$MICROSOFT_RUSTTRAINING_ROOT" cat-file -e "$MICROSOFT_TRAINING_COMMIT^{commit}"; then
    git -C "$MICROSOFT_RUSTTRAINING_ROOT" fetch --filter=blob:none origin main
fi
git -C "$MICROSOFT_RUSTTRAINING_ROOT" cat-file -e "$MICROSOFT_TRAINING_COMMIT^{commit}"
git -C "$MICROSOFT_RUSTTRAINING_ROOT" checkout --detach "$MICROSOFT_TRAINING_COMMIT"
export MICROSOFT_RUSTTRAINING_ROOT

echo "==> structure, links, and index parity"
python3 "$ROOT/checks/validate.py"
python3 "$ROOT/checks/gen_index.py" --check

echo "==> source-guidance behavior checks"
cd "$ROOT/checks"
cargo test --test source_guidance

echo "==> language-guidance behavior checks"
cargo test --test language_guidance

echo "==> generating example files from rules"
python3 gen.py

echo "==> compile-checking examples (target: $TARGET)"
# cargo exits non-zero on the intentional fragment snippets; the baseline gate
# below is what decides pass/fail.
cargo check --examples --target "$TARGET" --keep-going --message-format=json \
    > check.json 2> check.err || true

echo "==> gating against the baseline"
python3 analyze.py check.json --check-baseline baseline.txt

echo "All checks passed."
