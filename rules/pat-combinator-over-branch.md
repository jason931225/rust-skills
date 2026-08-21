# pat-combinator-over-branch

> Collapse a branch into the named combinator only when both arms are short expressions of one type; keep `if let` / `match` when the arms are different control flow

## Why It Matters

`if cond { Some(x) } else { None }` and `match opt { Some(x) => f(x), None => d }`
have names — `then_some`, `map_or` — and the named form says which operation is
happening instead of making a reader reconstruct it from the branches. But the
substitution is not always sound: the combinator evaluates its arguments under
different rules than the branch did, and an arm that diverges or produces a
different type cannot be an argument at all. The useful rule is therefore two
sided, and the second half is what keeps the first from being applied blindly.

## Bad

```rust
fn tag_for(level: u8, name: &str) -> Option<String> {
    // Hand-written shapes for operations that already have names.
    if level > 3 {
        Some(name.to_uppercase())
    } else {
        None
    }
}

fn parse_all(input: &[&str]) -> Result<Vec<i32>, std::num::ParseIntError> {
    // Rebuilds `collect`'s short-circuit by hand, one push at a time.
    let mut out = Vec::new();
    for token in input {
        match token.parse::<i32>() {
            Ok(value) => out.push(value),
            Err(error) => return Err(error),
        }
    }
    Ok(out)
}
```

## Good

```rust
fn tag_for(level: u8, name: &str) -> Option<String> {
    // `then` is lazy: `to_uppercase` runs only when the condition holds.
    (level > 3).then(|| name.to_uppercase())
}

fn parse_all(input: &[&str]) -> Result<Vec<i32>, std::num::ParseIntError> {
    // `Result` implements `FromIterator`, so this stops at the first `Err`
    // and returns it — the same control flow the loop wrote out.
    input.iter().map(|token| token.parse::<i32>()).collect()
}

fn describe(port: Option<u16>) -> String {
    // Both arms are short expressions of one type, so `map_or` fits.
    port.map_or("no port".to_string(), |value| format!("port {value}"))
}

fn main() {
    assert_eq!(tag_for(5, "core"), Some("CORE".to_string()));
    assert_eq!(tag_for(1, "core"), None);
    assert_eq!(parse_all(&["1", "2", "3"]), Ok(vec![1, 2, 3]));
    assert!(parse_all(&["1", "nope", "3"]).is_err());
    assert_eq!(describe(Some(8080)), "port 8080");
    assert_eq!(describe(None), "no port");
}
```

## What Actually Gets Evaluated

Two mechanical facts decide whether a rewrite preserves behaviour.

**`then_some` and `map_or` take their alternative eagerly; `then`,
`map_or_else`, and `unwrap_or_else` take a closure.** So converting a branch
whose untaken side was expensive or side-effecting changes what runs:

```rust
use std::cell::Cell;

fn main() {
    let calls = Cell::new(0);
    let expensive = || {
        calls.set(calls.get() + 1);
        "computed"
    };

    let _ = false.then_some(expensive());   // argument is evaluated first
    assert_eq!(calls.get(), 1, "then_some evaluated its argument anyway");

    let _ = false.then(|| expensive());     // closure is never called
    assert_eq!(calls.get(), 1, "then did not");
}
```

Reach for `then_some` only when the value is already computed or is a literal;
otherwise `then`. The same split applies to `unwrap_or` versus
`unwrap_or_else`, and `map_or` versus `map_or_else`.

**Short-circuiting is a guarantee, not an optimisation.** Collecting into
`Result` stops at the first `Err` and leaves the rest of the source unpulled,
which is exactly what the hand-written loop's early `return` did:

```rust
use std::cell::Cell;

fn main() {
    let source = ["1", "2", "nope", "4", "5"];
    let pulled = Cell::new(0);
    let mut iter = source.iter().inspect(|_| pulled.set(pulled.get() + 1));

    let parsed: Result<Vec<i32>, _> =
        iter.by_ref().map(|token| token.parse::<i32>()).collect();
    let pulled_by_collect = pulled.get();
    let remaining: Vec<_> = iter.collect();

    assert!(parsed.is_err());
    assert_eq!(pulled_by_collect, 3, "stopped at the element that failed");
    assert_eq!(remaining, [&"4", &"5"], "the tail was never pulled");
}
```

That matters when the iterator is doing real work per item — reading, parsing,
calling out — because the combinator does not quietly finish the traversal in
order to report the failure.

## When The Branches Are Genuinely Different Code Paths

Arms of different types do not unify, so the compiler stops that case outright:

```text
error[E0308]: mismatched types
  |     opt.map_or("none", |x| x)
  |         ------ ^^^^^^ expected `String`, found `&str`
```

A diverging arm is the more dangerous case, because it is **not** a compile
error. `!` coerces to any type, so `opt.map_or(return Err("absent"), |x| x)`
builds with nothing but an `unreachable_code` warning — and then returns
`Err("absent")` unconditionally, including when the option was `Some`, because
`map_or` evaluates its first argument before it looks at the option. The eager
evaluation described above is what turns a `return` in the "default" position
into a `return` in every position.

So prefer the branch when:

- an arm contains `return`, `break`, or `continue` — use `let ... else` for the
  divergent case, which exists for exactly this shape;
- the arms are multi-statement bodies, or their side effects differ, so the
  combinator would hide which path performs the work;
- the control flow is the point of the code rather than incidental to producing
  a value.

A chain assembled past roughly four adapters stops reading as one operation and
becomes a puzzle; naming an intermediate binding costs nothing and restores it.

## See Also

- [pat-let-else](pat-let-else.md) - the form for the divergent arm this rule sends back to a branch
- [pat-matches-macro](pat-matches-macro.md) - the boolean-test case, with its own boundary on binding
- [type-option-nullable](type-option-nullable.md) - the `Option` surface these combinators live on
- [err-question-mark](err-question-mark.md) - propagation for the single fallible value rather than a sequence
- [perf-iter-lazy](perf-iter-lazy.md) - `any`/`all`/`find` and the rest of the short-circuiting adapters
