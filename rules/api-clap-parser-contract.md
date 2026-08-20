# api-clap-parser-contract

> Know which clap behaviors are silent by default — process-exiting parse errors, id/value_name confusion, and declaration-order positionals — and pin each one deliberately

## Why It Matters

`clap` looks like an ordinary library call, but several of its defaults do
something a first read of the code does not suggest. `Parser::parse` and
`ArgMatches::get_matches` print usage and call `std::process::exit` on a bad
command line — wrapping the call in a function that returns `Result` does not
change that, because the process is already gone by the time any `Err` would
propagate. An argument's lookup key and its help-text placeholder are two
different strings that happen to look interchangeable, so a mismatch compiles
and returns `None` at runtime instead of failing to build, and a handful of
argument-shape decisions — positional order, multi-value consumption,
optional-vs-required exclusivity — are declared correctly-looking and still
produce the wrong parser unless the specific method that encodes each one is
used. None of these show up in `--help` or in a quick manual test with valid
input; they show up on the exact malformed input a real user eventually
supplies.

## Contract

- Decide deliberately whether a bad command line should exit the process
  immediately (`parse`/`get_matches`, the common case for a `main` binary) or
  return control to the caller (`try_parse`/`try_get_matches`, needed for a
  library wrapping clap or for testing the parse path without a subprocess).
- Keep an argument's lookup id and its displayed `value_name` conceptually
  separate, and test that the id you read after parsing is the one you
  declared — a typo compiles and returns `None` from `get_one`.
- State multiplicity on the right target: an option's repeated-value setting
  (`num_args`, `action(ArgAction::Append)`) is not the same configuration as a
  positional's; verify the parsed count against a real invocation, not
  against `--help` looking correct.
- Use `ArgGroup::required(true)` (or an explicit post-parse check into an
  enum) to require exactly one of several optional, mutually exclusive flags.
  `conflicts_with`/`conflicts_with_all` only forbids combinations — it does
  not require that any of them be present.
- When a token could be mistaken for a short-flag cluster (a leading `-`
  before a negative number, or digits that look like `-1 -2 -3`), set
  `allow_hyphen_values` or otherwise declare the value shape so the parser
  does not read `-3` as flags.
- Positional order in the builder API is the order `.arg(...)` is called, not
  the argument's name or its position in `--help`; a reordered declaration
  silently rebinds which operand fills which field.
- A multi-value option consumes following tokens greedily; without `--` or a
  value count the parser can pull a positional argument into an option's
  value list. Test a value immediately followed by a positional, with and
  without a `--` separator.

## Bad

```rust
use clap::Parser;

#[derive(Parser)]
struct Args {
    // The lookup id is `path`, but the placeholder in --help reads FILE.
    // Reading `matches.get_one::<String>("file")` compiles and returns
    // `None` forever, because "file" is never the id clap registered.
    #[arg(value_name = "FILE")]
    path: String,
}

fn run() -> Args {
    // parse() exits the process on a bad argv; a caller expecting to
    // recover via `?` never gets the chance.
    Args::parse()
}
```

## Good

```rust
use clap::Parser;

#[derive(Parser, Debug, PartialEq)]
struct Config {
    #[arg(value_name = "FILE")]
    path: String,
}

fn main() {
    // `try_parse_from` returns a `Result` instead of exiting, so a caller —
    // or a test — can inspect a parse failure directly.
    let ok = Config::try_parse_from(["prog", "data.txt"]).expect("a valid argv parses");
    assert_eq!(ok, Config { path: "data.txt".to_owned() });

    // No positional supplied: a parse error is returned, not a process exit.
    assert!(Config::try_parse_from(["prog"]).is_err());
}
```

## Failure Tests

- a missing required positional returns `Err` from `try_get_matches`, and
  never calls `process::exit` inside the test process;
- a value that looks like a short-flag cluster (`-3`) is accepted as the
  intended value once the argument is declared to allow it, and rejected as
  flags before that declaration is added;
- an argument declared with two optional, mutually exclusive flags rejects an
  invocation with neither present once `required(true)` is added to their
  group, and accepts it before the group exists;
- swapping the declaration order of two positional arguments changes which
  field a given operand fills, proving order is declaration order, not name.

## See Also

- [api-parse-dont-validate](api-parse-dont-validate.md) - clap's job is exactly this: parse into a type, not validate a stringly-typed map
- [proj-cli-contract](proj-cli-contract.md) - the exit-code and stream contract clap's process-exiting default has to cooperate with
- [test-cli-blackbox](test-cli-blackbox.md) - the black-box test that catches a mis-wired argument even when unit tests over the parser pass
- [err-result-over-panic](err-result-over-panic.md) - the general case of returning control instead of terminating, which `try_parse` opts into
- [type-enum-states](type-enum-states.md) - the parsed result of a required-one-of flag group belongs in an enum, not several optional booleans
