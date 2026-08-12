# obs-named-events

> Give telemetry a stable event name (and a message template) so releases stay queryable

## Why It Matters

Structured fields (`user.id`, `elapsed_ms`) make one line searchable. They do not group "cache evicted" across versions if the message string keeps changing or is assembled with `format!`. As Microsoft Pragmatic Rust Guidelines (M-LOG-STRUCTURED) add a second axis, use a hierarchical name (`cache.evict.success`) that dashboards filter on, plus a template that names the fields instead of interpolating them. `obs-structured-fields` is the field vocabulary; this rule is the event identity. `clippy::literal_string_with_formatting_args` is often allowed so templates can keep `{{field}}` braces.

## Bad

```rust
fn on_evict(key: &str) {
    // Interpolated message: the key is not a field, and the wording
    // will drift across releases.
    tracing::info!("evicted cache entry: {}", key);
}
```

## Good

```rust
use tracing::info;

fn on_evict(key: &str) {
    info!(
        event = "cache.evict.success",
        cache.key = key,
        "cache entry evicted"
    );
}

fn main() {
    on_evict("session:42");
}
```

## Key Points

- Keep the hierarchical event name stable across releases; renaming it breaks dashboards and saved queries.
- Put values in fields, not in the event name or the message string.

## See Also

- [obs-structured-fields](obs-structured-fields.md) - named fields on the event; this rule names the event itself
- [obs-tracing-over-log](obs-tracing-over-log.md) - emit through `tracing`, not `println!`
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - a stable name does not excuse logging secrets
