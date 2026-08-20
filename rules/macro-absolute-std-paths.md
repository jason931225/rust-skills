# macro-absolute-std-paths

> In exported macros, name standard items by absolute `::core` paths and your own by `$crate`

## Why It Matters

`macro_rules!` hygiene protects local variables, not paths. An exported macro
expands in the caller's namespace, so `Option`, `Result`, `write!`, and `vec!`
resolve to whatever the caller has in scope — which may be their own `Result`
type, a shadowing import, or a module literally named `std`. The failure is a
confusing type error inside expanded code the caller never wrote. Naming items
absolutely removes the assumption entirely, and preferring `::core` over
`::std` keeps the macro usable from `no_std` crates.

## Bad

```rust
#[macro_export]
macro_rules! try_parse {
    ($input:expr) => {
        // `Result`, `Ok`, `Err` and `write!` all resolve in the caller's scope
        match $input.parse() {
            Ok(value) => Ok(value),
            Err(error) => Err(format!("bad input: {}", error)),
        }
    };
}
```

## Good

```rust
// `::alloc` is reachable from a std crate once it is declared; this is what a
// `no_std`-compatible macro expansion relies on.
extern crate alloc;

/// Every path is absolute: the expansion means the same thing in any scope,
/// including a `no_std` crate.
#[macro_export]
macro_rules! describe {
    ($value:expr) => {
        ::core::result::Result::<::alloc::string::String, ::core::fmt::Error>::Ok(
            ::alloc::format!("{:?}", $value),
        )
    };
}

/// Items belonging to the defining crate are named with `$crate`, which
/// resolves correctly however the caller imported it.
#[macro_export]
macro_rules! wrap_error {
    ($message:expr) => {
        $crate::Error::new($message)
    };
}

#[derive(Debug, PartialEq)]
pub struct Error {
    message: &'static str,
}

impl Error {
    pub fn new(message: &'static str) -> Self {
        Self { message }
    }
}

fn main() {
    // A caller that shadows `Result` and `Option` still expands correctly,
    // because the macro never relied on those names.
    #[allow(dead_code)]
    struct Result;
    #[allow(dead_code)]
    struct Option;

    let described = describe!(42u8).expect("expansion is independent of scope");
    assert_eq!(described, "42");
    assert_eq!(wrap_error!("boom"), crate::Error::new("boom"));
}
```

## Path Choice And Hygiene

- Prefer `::core` and `::alloc` to `::std` so the macro works in `no_std`
  crates; use `::std` only for items that exist nowhere else.
- `$crate` is the only correct way to reach your own items — the caller may have
  renamed the dependency or not imported it at all.
- Hygiene does cover local identifiers: a `let` inside the macro cannot collide
  with the caller's variables, and cannot be read by them either.
- Test expansion from a module that shadows `Result`, `Option`, and `std`; that
  is the case absolute paths exist for.
- Procedural macros have the same obligation — emit absolute paths in `quote!`.

## See Also

- [macro-rules-hygiene](macro-rules-hygiene.md) - what hygiene does and does not cover
- [macro-private-helpers](macro-private-helpers.md) - hiding generated helper items
- [macro-export-crate-path](macro-export-crate-path.md) - giving the macro a clean import path
- [macro-proc-syn-quote](macro-proc-syn-quote.md) - the same discipline in generated token streams
