# perf-hoist-loop-invariant

> Build expensive input-independent values once, outside the loop that uses them

## Why It Matters

Compiling a regex, parsing a template, building a validator, or opening a
connection costs orders of magnitude more than using the result. Placed inside
a loop or a request handler, that cost is paid per iteration and dominates the
profile while the work itself looks fine. The compiler cannot hoist it for you:
these are heap-allocating calls with side effects, so it must assume they
matter. Moving the construction out is usually a two-line change and one of the
largest wins available in ordinary code.

## Bad

```rust
fn find_ids(lines: &[String]) -> Vec<String> {
    lines.iter().filter_map(|line| {
        // Recompiles the pattern for every line
        let re = Regex::new(r"^id-(\d+)$").unwrap();
        re.captures(line).map(|c| c[1].to_string())
    }).collect()
}
```

## Good

```rust
use std::sync::LazyLock;

/// Stands in for a genuinely expensive build: a compiled pattern, a parsed
/// template, a schema.
struct Matcher {
    prefix: String,
}

impl Matcher {
    fn compile(prefix: &str) -> Self {
        Self { prefix: prefix.to_owned() }
    }

    fn matches(&self, line: &str) -> bool {
        line.starts_with(&self.prefix)
    }
}

/// Built once, on first use, and shared by every caller.
static ID_MATCHER: LazyLock<Matcher> = LazyLock::new(|| Matcher::compile("id-"));

fn find_ids(lines: &[String]) -> Vec<&String> {
    lines.iter().filter(|line| ID_MATCHER.matches(line)).collect()
}

fn main() {
    let lines = vec!["id-1".to_owned(), "other".to_owned(), "id-2".to_owned()];
    let found = find_ids(&lines);
    assert_eq!(found.len(), 2);
}
```

## What To Hoist And Where

- A `LazyLock` static fits values that are process-wide and immutable; a field
  on a reused struct fits values that belong to one component.
- Keep the hoisted value immutable, or the loop reintroduces synchronisation.
- The same reasoning covers buffers: allocate once and clear per iteration
  rather than allocating inside the loop.
- Do not hoist something whose cost is a single arithmetic operation; the
  compiler already does that, and a static adds indirection.
- Confirm with a profile. This is one of the few optimisations whose effect is
  usually visible without one, but the profile is what tells you the loop
  mattered.

## See Also

- [perf-profile-first](perf-profile-first.md) - measure before and after
- [mem-reuse-collections](mem-reuse-collections.md) - the allocation version of the same idea
- [const-vs-static](const-vs-static.md) - when the value can be a `const` instead
- [anti-format-hot-path](anti-format-hot-path.md) - the same cost hidden in formatting
