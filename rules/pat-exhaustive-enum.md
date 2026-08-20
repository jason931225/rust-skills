# pat-exhaustive-enum

> Match owned enums exhaustively; avoid catch-all `_` that hides new variants

## Why It Matters

A `_ =>` wildcard arm silently absorbs any variant added to an enum you own, converting what should be a compile-time error into a silent runtime no-op. Exhaustive matches let the compiler act as a checklist: add a variant, get a build failure everywhere it is unhandled. Reserve `_` and `..` for **foreign** `#[non_exhaustive]` enums, where the language requires a catch-all, and document why it is necessary.

## Bad

```rust
#[derive(Debug)]
enum Status {
    Active,
    Pending,
    Closed,
}

fn describe(s: &Status) -> &'static str {
    match s {
        Status::Active => "active",
        _ => "inactive", // hides Status::Pending silently; adding a new variant goes unnoticed
    }
}
```

If `Status::Suspended` is later added, `describe` compiles and silently returns `"inactive"` for it — a logic bug the compiler never catches.

## Good

```rust
#[derive(Debug)]
enum Status {
    Active,
    Pending,
    Closed,
}

fn describe(s: &Status) -> &'static str {
    match s {
        Status::Active => "active",
        Status::Pending => "pending",
        Status::Closed => "closed",
        // Adding Status::Suspended now causes a compile error here — intended.
    }
}
```

## Grouping Variants with `|`

When several variants share the same handling, list them explicitly rather than falling back to `_`:

```rust
fn is_terminal(s: &Status) -> bool {
    match s {
        Status::Active | Status::Pending => false,
        Status::Closed => true,
    }
}
```

## When `_` Is Required: Foreign `#[non_exhaustive]` Enums

External crates may mark enums `#[non_exhaustive]`, which means the compiler *forces* a wildcard. Document the intent:

```rust
// From a hypothetical external crate:
// #[non_exhaustive]
// pub enum TheirEvent { Click, Hover, /* ... future variants */ }

fn handle_event(event: &some_crate::TheirEvent) {
    match event {
        some_crate::TheirEvent::Click => { /* ... */ }
        some_crate::TheirEvent::Hover => { /* ... */ }
        // required by #[non_exhaustive]; intentionally a no-op for unknown variants
        _ => {}
    }
}
```

## Clippy Lint

`clippy::wildcard_enum_match_arm` (part of `clippy::restriction`) warns when a wildcard arm in a match on a non-`#[non_exhaustive]` enum could be replaced with explicit variants. Enabling it catches drift over time.

## An Equality Chain Gets No Exhaustiveness Check At All

This rule argues against the wildcard arm, which presupposes a `match` exists.
Code that dispatches with `if code == Status::Ok { .. } else if ..` never
enrolled in the check in the first place, and nothing tells you so.

Add a variant, and the wildcard-free `match` stops the build:

```text
error[E0004]: non-exhaustive patterns: `StatusCode::TooManyRequests` not covered
   |
14 |     match code {
   |           ^^^^ pattern `StatusCode::TooManyRequests` not covered
```

The equality chain compiles clean under `-D warnings` and silently returns its
trailing `else`. Clippy does not rescue it either: with `all`, `pedantic`,
`nursery`, and `restriction` enabled, none of the warnings emitted concern the
dispatch shape. Choosing `match` is the whole mechanism — there is no lint
standing behind it.

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum StatusCode {
    Ok,
    NotFound,
    ServerError,
}

/// Adding a variant makes this fail to compile until it is handled.
pub fn describe(code: StatusCode) -> &'static str {
    match code {
        StatusCode::Ok => "ok",
        StatusCode::NotFound => "not found",
        StatusCode::ServerError => "server error",
    }
}

fn main() {
    assert_eq!(describe(StatusCode::NotFound), "not found");
}
```

This is why the enum needs `PartialEq` only if something genuinely compares
values. Deriving it reflexively makes the equality chain available, and the
chain is the form that looks fine forever.

## See Also

- [api-non-exhaustive](api-non-exhaustive.md) - use `#[non_exhaustive]` for future-proof enums in public APIs
- [type-enum-states](type-enum-states.md) - use enums for mutually exclusive states
- [pat-matches-macro](pat-matches-macro.md) - boolean pattern tests with `matches!()`
