#!/usr/bin/env python3
"""Execute every generated example that carries an assertion.

`cargo check` type-checks the examples but never runs them, so an example that
compiles while asserting something false ships unnoticed. Two such bugs were
found this way: an upload-serving example asserting a Content-Disposition
header contained no quotes when it always contains the two that delimit the
filename, and a byte-slice-cast example whose alignment claim depended on
where the allocator happened to place a stack array.

Only examples with both a `fn main` and an `assert` are run: the rest are
fragments with nothing to check at runtime.
"""
import glob, pathlib, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
BIN = HERE / "target" / "debug" / "examples"
TIMEOUT = 10

def main() -> int:
    candidates = []
    for path in sorted(glob.glob(str(HERE / "examples" / "*.rs"))):
        text = pathlib.Path(path).read_text(encoding="utf-8")
        if "fn main" in text and "assert" in text:
            candidates.append(pathlib.Path(path).stem)

    build = subprocess.run(
        ["cargo", "build", "--examples", "--keep-going"],
        cwd=HERE, capture_output=True, text=True,
    )
    # A build failure here is already reported by the compile gate against the
    # baseline; this step only cares about examples that produced a binary.
    del build

    failures, ran, skipped = [], 0, 0
    for name in candidates:
        binary = BIN / name
        if not binary.exists():
            skipped += 1
            continue
        # A wall-clock timeout cannot tell a hung example from a loaded
        # machine, and this suite runs right after a full cargo build. Retry
        # once with a longer budget before calling it a failure: a real hang
        # still hangs, while a starved process gets the room it needed.
        for attempt, budget in enumerate((TIMEOUT, TIMEOUT * 3)):
            try:
                result = subprocess.run([str(binary)], capture_output=True,
                                        text=True, timeout=budget)
                break
            except subprocess.TimeoutExpired:
                result = None
        if result is None:
            failures.append((name, f"did not finish within {TIMEOUT * 3}s "
                                   "across two attempts"))
            continue
        ran += 1
        if result.returncode != 0:
            detail = (result.stderr or result.stdout).strip().splitlines()
            failures.append((name, " / ".join(detail[:3])))

    if failures:
        print(f"FAIL: {len(failures)} example(s) with assertions did not pass")
        for name, why in failures:
            print(f"  - {name}: {why}")
        return 1
    print(f"OK: {ran} examples with assertions ran clean "
          f"({skipped} had no binary and were skipped)")
    return 0

if __name__ == "__main__":
    sys.exit(main())
