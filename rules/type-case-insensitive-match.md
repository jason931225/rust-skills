# type-case-insensitive-match

> Configure the matcher for case-insensitivity instead of case-folding the data

## Why It Matters

Lowercasing both sides before comparing looks equivalent and is not. It
allocates a copy of every input, discards the original casing that later output
may need, and gets Unicode wrong: `to_lowercase` is locale-independent, so
Turkish dotless İ and German ß do not round-trip, and folding twice is not the
same as folding once. Matchers — regex engines, collators, database
collations — have a case-insensitivity setting that applies the correct
folding at comparison time without touching the data.

## Bad

```rust
fn find(haystack: &[String], needle: &str, insensitive: bool) -> Vec<String> {
    haystack.iter().filter(|line| {
        if insensitive {
            // Allocates twice per line, and folds with the wrong rules for
            // several scripts
            line.to_lowercase().contains(&needle.to_lowercase())
        } else {
            line.contains(needle)
        }
    }).cloned().collect()
}
```

## Good

```rust
/// The flag belongs to the matcher, built once, not to the data.
pub struct Matcher {
    needle: String,
    case_insensitive: bool,
}

impl Matcher {
    pub fn new(needle: &str, case_insensitive: bool) -> Self {
        Self { needle: needle.to_owned(), case_insensitive }
    }

    pub fn matches(&self, line: &str) -> bool {
        if self.case_insensitive {
            // ASCII folding is well-defined and allocation-free; a real
            // matcher (regex, ICU collation) applies full Unicode folding.
            line.to_ascii_lowercase()
                .contains(&self.needle.to_ascii_lowercase())
        } else {
            line.contains(&self.needle)
        }
    }
}

fn main() {
    let sensitive = Matcher::new("Error", false);
    let insensitive = Matcher::new("Error", true);

    assert!(sensitive.matches("Error: disk full"));
    assert!(!sensitive.matches("error: disk full"));
    assert!(insensitive.matches("error: disk full"));

    // The data keeps its original casing for output.
    let line = "ERROR: disk full";
    assert!(insensitive.matches(line));
    assert_eq!(line, "ERROR: disk full");
}
```

## Matcher Configuration Over Folding

- `RegexBuilder::case_insensitive(true)` and equivalent settings apply the
  engine's own folding; prefer them to rewriting the input.
- For a single ASCII comparison, `eq_ignore_ascii_case` says exactly what it
  does and allocates nothing.
- Keep the original text for output, logging, and storage; folding is a
  comparison detail, not a transformation of the record.
- Full Unicode case-insensitivity is case *folding*, not lowercasing, and
  belongs to a library that implements it.
- Database comparisons have the same choice: set the column or query collation
  rather than wrapping columns in `lower()`, which also defeats indexes.

## See Also

- [perf-hoist-loop-invariant](perf-hoist-loop-invariant.md) - build the configured matcher once
- [type-unicode-length](type-unicode-length.md) - the same care for text boundaries
- [type-text-decode-policy](type-text-decode-policy.md) - deciding what the bytes mean first
- [anti-clone-excessive](anti-clone-excessive.md) - folding both sides allocates per comparison
