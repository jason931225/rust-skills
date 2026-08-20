# lint-pedantic-selective

> Enable clippy::pedantic selectively

## Why It Matters

The `clippy::pedantic` group contains opinionated lints that aren't universally applicable. Enabling it wholesale produces noise; selectively enabling useful pedantic lints improves code quality without false positives.

## Bad

```rust
// Too noisy - will fight you constantly
#![warn(clippy::pedantic)]
```

## Good

```toml
# Cargo.toml - cherry-pick useful pedantic lints
[lints.clippy]
# Enable pedantic as baseline
pedantic = { level = "warn", priority = -1 }

# Disable noisy ones
missing_errors_doc = "allow"      # Document errors separately
missing_panics_doc = "allow"      # Document panics separately
module_name_repetitions = "allow" # Allow Foo::FooError pattern
too_many_lines = "allow"          # Function length varies
must_use_candidate = "allow"      # Too many suggestions
```

## Recommended Pedantic Lints

| Lint | Why Enable |
|------|-----------|
| `doc_markdown` | Catch unmarked code in docs |
| `match_wildcard_for_single_variants` | Explicit variant matching |
| `semicolon_if_nothing_returned` | Consistent semicolons |
| `string_add_assign` | Use `+=` for string concatenation |
| `unnested_or_patterns` | Simplify match patterns |
| `unused_self` | Catch methods that should be functions |
| `used_underscore_binding` | Warn on using `_var` |
| `wildcard_imports` | Avoid glob imports |

## Often Disabled

| Lint | Why Disable |
|------|-------------|
| `missing_errors_doc` | Handle with `#[doc]` policy |
| `missing_panics_doc` | Handle with `#[doc]` policy |
| `module_name_repetitions` | Sometimes intentional |
| `must_use_candidate` | Too aggressive |
| `too_many_lines` | Arbitrary threshold |
| `struct_excessive_bools` | Valid for config structs |

## Full Configuration

```toml
# Cargo.toml
[lints.clippy]
# Start with pedantic
pedantic = { level = "warn", priority = -1 }

# Keep these
doc_markdown = "warn"
match_wildcard_for_single_variants = "warn"
semicolon_if_nothing_returned = "warn"
unused_self = "warn"
wildcard_imports = "warn"

# Disable these
missing_errors_doc = "allow"
missing_panics_doc = "allow"
module_name_repetitions = "allow"
must_use_candidate = "allow"
too_many_lines = "allow"
similar_names = "allow"
struct_excessive_bools = "allow"
```

## Alternative: Explicit Opt-in

```toml
# Only enable specific lints, not the group
[lints.clippy]
# From pedantic, only these:
doc_markdown = "warn"
semicolon_if_nothing_returned = "warn"
unused_self = "warn"
wildcard_imports = "warn"
```

## Module-Level Overrides

```rust
// Allow specific lint for a module
#![allow(clippy::module_name_repetitions)]

// Or for specific items
#[allow(clippy::too_many_arguments)]
fn complex_function(/* many args */) { }
```

## Team Consensus

Pedantic lints are style choices. Agree as a team:

1. Enable `pedantic` as baseline
2. Run `cargo clippy` on codebase
3. Discuss each warning category
4. Disable ones that don't fit your style
5. Document decisions in `clippy.toml`

## Group Entries Need A Lower Priority

A group and an individual lint in the same `[lints.*]` table both default to
priority 0, and Cargo refuses to guess which wins:

```
error: lint group `pedantic` has the same priority (0) as a lint
```

That is `clippy::lint_groups_priority`, deny-by-default, so the config does not
merely warn — the build fails. Give every *group* entry an explicit lower
priority so the individual `allow`/`warn` lines that follow override it:

```toml
[lints.clippy]
pedantic = { level = "warn", priority = -1 }   # group: lower priority
missing_errors_doc = "allow"                   # individual: wins over the group
```

The same applies to any group entry — `nursery`, `restriction`, `cargo` — not
just `pedantic`.

## See Also

- [lint-warn-style](./lint-warn-style.md) - Style warnings
- [lint-warn-complexity](./lint-warn-complexity.md) - Complexity warnings
- [lint-deny-correctness](./lint-deny-correctness.md) - Correctness lints
