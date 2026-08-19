# test-env-independent

> Assert what the program decides; normalize or exclude everything the host decides

## Why It Matters

A test that compares full output against a golden file will encode whatever the
machine contributed: the developer's username and group, file sizes that differ
by a byte on another filesystem, modification times, terminal width, locale
separators, temporary paths containing a PID. It passes on the machine that
recorded it and fails everywhere else, and the failure says nothing useful —
the program was right, the fixture was over-specified. Deciding which fields
the program actually determines is the difference between a regression test and
a machine fingerprint.

## Bad

```rust
#[test]
fn lists_the_directory() {
    let output = run(&["-l", "tests/inputs"]);
    // Records this machine's user, group, size, and timestamp
    assert_eq!(output, "-rw-r--r-- 1 kyclark staff 217 Aug 11 08:26 Cargo.toml\n");
}
```

## Good

```rust
/// Replaces the fields the host decides, leaving the ones the program decides.
///
/// `ls -l` lays out: mode, links, owner, group, size, month, day, time, name.
/// The first two and the name are the program's; the rest belong to the machine.
pub fn normalize(line: &str) -> String {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 9 {
        return line.to_owned();
    }
    let name = fields[8..].join(" ");
    format!("{} {} <host> <host> <host> <date> {}", fields[0], fields[1], name)
}

fn main() {
    let recorded = "-rw-r--r-- 1 kyclark staff 217 Aug 11 08:26 Cargo.toml";
    let elsewhere = "-rw-r--r-- 1 ci nogroup 219 Jan 2 03:04 Cargo.toml";

    // The same assertion holds on both machines, and still catches a wrong
    // mode, a wrong link count, or a wrong name.
    assert_eq!(normalize(recorded), normalize(elsewhere));
    assert_eq!(normalize(recorded), "-rw-r--r-- 1 <host> <host> <host> <date> Cargo.toml");

    let wrong_mode = "-rwxr-xr-x 1 ci nogroup 219 Jan 2 03:04 Cargo.toml";
    assert_ne!(normalize(recorded), normalize(wrong_mode));
}
```

## Key Points

- Normalize rather than delete: replacing a field with a placeholder still
  asserts that it was present and in position.
- Set the environment the test depends on — locale, timezone, terminal width —
  instead of tolerating whatever the runner has.
- Create fixtures inside the test, and give temporary paths stable names in the
  output before comparing.
- Sort anything whose order the program does not define, such as directory
  entries.
- If a value is genuinely part of the contract, assert it precisely; the goal
  is deliberate coverage, not a looser comparison.

## See Also

- [test-cli-blackbox](test-cli-blackbox.md) - where these golden comparisons happen
- [api-dir-enumeration](api-dir-enumeration.md) - why listing order must be imposed
- [test-snapshot-testing](test-snapshot-testing.md) - snapshots need the same normalization
- [test-no-tautology](test-no-tautology.md) - assert the program's decisions, not restated constants
