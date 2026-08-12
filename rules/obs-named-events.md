# obs-named-events

> Give telemetry a stable event name (and a message template) so releases stay queryable

## Why It Matters

Structured fields (`user.id`, `elapsed_ms`) make one line searchable. They do not group "file opened" across versions if the message string keeps changing or is assembled with `format!`. The Microsoft Pragmatic Rust Guidelines add a second axis: a hierarchical name (`file.open.success`) that dashboards filter on, plus a template that names the fields instead of interpolating them. `obs-structured-fields` is the field vocabulary; this rule is the event identity. `clippy::literal_string_with_formatting_args` is often allowed so templates can keep `{{field}}` braces.

## Bad

```rust
fn on_open(path: &str) {
    tracing::info!("file opened: {}", path);
}
```

## Good

```rust
use tracing::info;

fn on_open(path: &str) {
    info!(
        event = "file.open.success",
        file.path = path,
        "file opened"
    );
}

fn main() {
    on_open("notes.txt");
}
```

## See Also

- [obs-structured-fields](obs-structured-fields.md) - named fields on the event; this rule names the event itself
- [obs-tracing-over-log](obs-tracing-over-log.md) - emit through `tracing`, not `println!`
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - a stable name does not excuse logging secrets
