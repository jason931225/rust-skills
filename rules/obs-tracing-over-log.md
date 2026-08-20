# obs-tracing-over-log

> Use `tracing` for structured, span-aware diagnostics instead of `println!` or bare `log`

## Why It Matters

`println!` and `eprintln!` have no log level, target, or structured fields. The
`log` facade adds levels and targets but not spans, while `tracing` records
structured events and contextual spans. Context does not automatically enter
every spawned task or thread, so instrument the future or task and propagate
trace context across process boundaries. A library that emits no diagnostics
needs no tracing dependency; one that does should depend on the facade and
leave subscriber or exporter ownership to the binary.

## Bad

```rust
fn handle_login(id: u64) {
    println!("user {} logged in", id);
    // No level, no structure, no filtering, goes to stdout unconditionally
}

fn main() {
    handle_login(42);
}
```

## Good

```rust
use tracing::info;

fn handle_login(id: u64) {
    // Structured field: user.id is queryable in JSON/OpenTelemetry backends
    info!(user.id = %id, "user logged in");
}

fn main() {
    // One-time subscriber init belongs in the binary, not in libraries
    tracing_subscriber::fmt::init();
    handle_login(42);
}
```

## Choosing Between Logging Approaches

| Approach | Levels | Structured | Async-aware spans | `log` compat |
|---|---|---|---|---|
| `println!` | No | No | No | No |
| `log` facade | Yes | No | No | Yes |
| `tracing` | Yes | Yes | With explicit instrumentation/propagation | Yes (via feature) |

- Add `tracing` to crates that emit events or spans. Put `tracing-subscriber` and exporter configuration in binaries only.
- The `%expr` sigil uses `Display`; `?expr` uses `Debug`; bare `field = value` records typed primitives.
- `tracing` ships a `log` compatibility bridge: set `tracing-subscriber`'s `log` feature or call `tracing_log::LogTracer::init()` to capture existing `log`-emitting dependencies.
- `println!` is valid for a CLI's intentional user interface on stdout. Diagnostics, progress internals, and library output still go through telemetry (or an explicit CLI renderer).
- Remove `dbg!` from production paths; it writes unstructured file/line diagnostics to stderr.

## See Also

- [obs-structured-fields](obs-structured-fields.md) - record key-value fields, not interpolated strings
- [obs-instrument-spans](obs-instrument-spans.md) - attach context to async tasks with spans
- [async-tokio-runtime](async-tokio-runtime.md) - production async runtime setup
