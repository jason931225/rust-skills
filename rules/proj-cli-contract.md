# proj-cli-contract

> Exit 0 only on success, send results to stdout and diagnostics to stderr, and read `-` as standard input

## Why It Matters

A command-line program is a component in a pipeline, and the shell reads only
its exit status and its two streams. POSIX gives 0 for success and 1–255 for
failure, and scripts branch on that; a tool that prints an error and exits 0
turns a broken step into a silent one. Mixing diagnostics into stdout corrupts
whatever consumes the output, and a program that dies on a closed pipe makes
`prog | head` look like a crash. These conventions are cheap to honour and
expensive to retrofit once scripts depend on the wrong behaviour.

## Contract

- Exit 0 for success and non-zero for failure. Return `ExitCode` from `main`,
  or call `std::process::exit` at one place, rather than exiting from deep
  inside the logic.
- Results go to stdout; errors, warnings, and usage go to stderr. Usage on a
  bad invocation is a failure, so it exits non-zero.
- Treat a file argument of `-` as standard input, and read stdin when no file
  argument is given, if the tool takes files at all.
- Continue past a failed input where the tool documents per-file processing,
  report each failure on stderr, and reflect the failure in the final status.
- Handle a closed downstream pipe as a normal end of output, not an error: a
  `BrokenPipe` write error means the consumer stopped reading.
- Keep messages lowercase and without trailing punctuation, and name the input
  that failed.
- Write output through a locked, buffered handle in loops instead of calling
  `println!` per line.
- Where a reference implementation exists (a classic Unix utility, a
  predecessor tool), pick one dialect deliberately — BSD and GNU builds of
  the "same" tool routinely disagree on flag combinations, default behavior,
  and edge cases — and test against that dialect's real binary output, not a
  mental model of what it does.
- Selection-by-position arguments (a field list, a byte range) are typically
  1-based and inclusive on the command line; convert to a 0-based half-open
  range once, at the parse boundary, and preserve the caller's requested
  order and duplicates rather than silently sorting or deduplicating.
- In multi-file mode, print a header only for files that were opened
  successfully, and get the blank-line-before-subsequent-header rule from a
  real invocation rather than guessing; a file that failed to open gets only
  its stderr diagnostic, no header.
- When file names must survive arbitrary bytes (including embedded
  newlines), delimit a list of paths with NUL, not newline — newline is a
  legal filename byte on Unix and NUL is the only byte that is not.
- When a tool has default output selectors, decide whether an explicit
  selector flag *replaces* the default bundle or is *added* to it; these are
  different, easily-swapped behaviors and only one of them matches any given
  reference tool.

## Bad

```rust
fn main() {
    let path = std::env::args().nth(1).unwrap(); // panics with a Rust backtrace
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        println!("error: {e}"); // diagnostics on stdout, and then...
        String::new()           // ...exit code 0 says everything worked
    });
    print!("{text}");
}
```

## Good

```rust
use std::io::{self, BufWriter, Read, Write};
use std::process::ExitCode;

fn run(paths: &[String], output: &mut impl Write) -> io::Result<bool> {
    let mut all_ok = true;
    for path in paths {
        let mut text = String::new();
        let read = if path == "-" {
            io::stdin().read_to_string(&mut text)
        } else {
            std::fs::File::open(path).and_then(|mut f| f.read_to_string(&mut text))
        };
        match read {
            // One bad input is reported and skipped; the status still fails.
            Err(error) => {
                eprintln!("{path}: {error}");
                all_ok = false;
            }
            Ok(_) => match output.write_all(text.as_bytes()) {
                // The consumer stopped reading: stop cleanly, do not report it.
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(all_ok),
                other => other?,
            },
        }
    }
    Ok(all_ok)
}

fn main() -> ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let paths = if paths.is_empty() { vec!["-".to_owned()] } else { paths };

    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    let outcome = run(&paths, &mut output).and_then(|ok| output.flush().map(|()| ok));

    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("write failed: {error}");
            ExitCode::FAILURE
        }
    }
}
```

Exit codes beyond 0 and 1 are a public contract of their own: document each
one, and keep the meanings stable across releases.

## See Also

- [test-cli-blackbox](test-cli-blackbox.md) - assert this contract against the real binary
- [err-edge-mapping](err-edge-mapping.md) - the process boundary is an edge like any other
- [proj-lib-main-split](proj-lib-main-split.md) - keep `main` thin and the logic testable
- [perf-io-buffering](perf-io-buffering.md) - buffer and lock stdout for per-line output
- [err-lowercase-msg](err-lowercase-msg.md) - message style for diagnostics
