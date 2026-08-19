# test-cli-blackbox

> Test a command-line program by running the built binary and asserting on exit status, stdout, and stderr

## Why It Matters

A CLI's contract is what the shell observes: the exit status, what landed on
standard output, what landed on standard error, and what it did to the
filesystem. Unit tests over internal functions can all pass while the binary
exits zero on failure, prints diagnostics to stdout where they corrupt a pipe,
or never wires the argument up at all. Running the real executable the way a
user or a script does is the only test that covers argument parsing, output
routing, and exit status together.

## Contract

- Put these tests in `tests/`, where each file is its own integration crate,
  and run the binary produced by the build rather than a rebuilt copy.
- Assert the exit status explicitly for both success and failure paths — a
  test that only checks output will not notice a program that always exits 0.
- Assert on stdout and stderr separately, and assert that failure output does
  *not* go to stdout.
- Drive stdin for programs that read it, including the `-` argument
  convention, and cover the empty-input case.
- Use temporary directories for anything that touches the filesystem, so tests
  stay parallel-safe and self-cleaning.
- Keep golden-output comparisons in files next to the test, and treat a
  changed golden file as a deliberate contract change.

## Bad

```rust
#[test]
fn finds_matches() {
    // Tests the library function, not the program: nothing here would notice
    // if main() ignored the flag, printed to stdout, or exited 0 on error.
    assert_eq!(search("needle", "haystack needle"), vec!["haystack needle"]);
}
```

## Good

```rust
use std::io::Write;
use std::process::{Command, Stdio};

/// Runs the built binary with `args` and `stdin`, returning
/// (exit code, stdout, stderr).
fn run(binary: &str, args: &[&str], stdin: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn the binary under test");
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(stdin.as_bytes())
        .expect("failed to write stdin");
    let output = child.wait_with_output().expect("failed to collect output");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn main() {
    // `env!("CARGO_BIN_EXE_<name>")` resolves to the binary this test build
    // produced, so no separate build step or PATH lookup is involved.
    let (code, out, err) = run("/bin/echo", &["ok"], "");
    assert_eq!(code, Some(0));
    assert_eq!(out.trim(), "ok");
    assert!(err.is_empty(), "diagnostics must not appear on stderr here");
}
```

`assert_cmd` and `predicates` wrap this pattern with better failure messages
(`Command::cargo_bin("prog")?.arg("-").write_stdin(input).assert().failure()`);
the substance is the same — spawn the real binary, assert on all three
observable channels.

## Failure Tests

- a missing required argument exits non-zero and prints usage to stderr;
- an unreadable or absent file exits non-zero, names the file, and still
  processes the remaining arguments where the tool documents that behaviour;
- `-` reads standard input;
- empty input produces the documented empty output and exit 0;
- output is byte-identical to the golden file, including the trailing newline.

## See Also

- [proj-cli-contract](proj-cli-contract.md) - the exit-status and stream contract being asserted
- [test-integration-dir](test-integration-dir.md) - where these tests live
- [test-http-blackbox](test-http-blackbox.md) - the same discipline for an HTTP surface
- [test-fixture-raii](test-fixture-raii.md) - temporary directories that clean themselves up
- [test-observable-coverage](test-observable-coverage.md) - assert observable behaviour, not internals
