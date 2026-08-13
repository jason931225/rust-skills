# obs-named-events

> Give telemetry a stable event name (and a message template) so releases stay queryable

## Why It Matters

Structured fields such as `user.id` and `elapsed_ms` make one line searchable,
but they do not group the same event across versions when its message keeps
changing. Give each event a stable hierarchical name such as
`cache.evict.success` and a literal template that names fields instead of
interpolating them. `obs-structured-fields` defines the field vocabulary; this
rule defines event identity. Allow
`clippy::literal_string_with_formatting_args` where templates need
`{{field}}` braces.

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
use tracing::{event, Level};

fn on_evict(key: &str) {
    event!(
        name: "cache.evict.success",
        Level::INFO,
        cache.key = key,
        message.template = "cache entry {cache.key} evicted",
        "cache entry evicted"
    );
}

fn main() {
    on_evict("session:42");
}
```

## Key Points

- Keep the hierarchical event name stable across releases; renaming it breaks dashboards and saved queries.
- Use `<component>.<operation>.<state>` names when that vocabulary fits (`cache.evict.success`, `db.query.failure`).
- Put values in fields, not in the event name or a preformatted message.
- A backend-facing message template may reference field names for human rendering; record the values only once as fields.
- Third-party libraries must assume events can remain enabled under load. Keep hot inner loops free of telemetry when possible; otherwise emit one lightweight event per batch or state transition so operators can reconstruct the detail offline.

## See Also

- [obs-structured-fields](obs-structured-fields.md) - named fields on the event; this rule names the event itself
- [obs-levels-filter](obs-levels-filter.md) - filtering controls volume but does not make expensive event construction free
- [obs-tracing-over-log](obs-tracing-over-log.md) - emit through `tracing`, not `println!`
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - a stable name does not excuse logging secrets
