# opt-monomorph-outline

> Split a generic shell from a non-generic body so only the type-dependent part is duplicated

## Why It Matters

Monomorphization compiles a fresh copy of a generic function for every type it
is called with. That is what makes generic Rust as fast as hand-written code,
and it is also why a convenience bound like `impl AsRef<Path>` can produce
dozens of near-identical copies of a large function body. The cost lands in
compile time, binary size, and instruction cache: the CPU now holds several
copies of instructions that do exactly the same work. Usually only a few lines
actually depend on the type.

## Bad

```rust
pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Config> {
    // Every caller type duplicates the parsing, validation, and error
    // construction below, none of which depends on P
    let bytes = fs::read(path.as_ref())?;
    let text = String::from_utf8(bytes).map_err(io::Error::other)?;
    let mut config = Config::default();
    for line in text.lines() {
        /* forty lines of parsing */
    }
    Ok(config)
}
```

## Good

```rust
use std::path::Path;

#[derive(Debug, Default, PartialEq)]
pub struct Config {
    pub entries: usize,
}

/// Generic shell: the only thing duplicated per caller type is the conversion.
pub fn load<P: AsRef<Path>>(path: P) -> Result<Config, String> {
    // Declaring the helper inside keeps it out of the module's namespace, and
    // being non-generic it is compiled once for all instantiations.
    fn inner(path: &Path) -> Result<Config, String> {
        let name = path.to_str().ok_or_else(|| "path is not utf-8".to_owned())?;
        Ok(Config { entries: name.len() })
    }
    inner(path.as_ref())
}

fn main() {
    let from_str = load("app.conf").expect("loads");
    let from_path = load(Path::new("app.conf")).expect("loads");
    let from_owned = load(String::from("app.conf")).expect("loads");

    // Three caller types, one copy of the body.
    assert_eq!(from_str, from_path);
    assert_eq!(from_path, from_owned);
}
```

## Key Points

- The pattern is standard-library practice: an ergonomic generic signature over
  a single concrete implementation.
- A helper declared inside the function stays private to it; declared outside,
  keep it out of a generic `impl` block or it is monomorphized again.
- This is a code-size and compile-time optimisation, not a runtime one — the
  duplicated copies were already fast.
- Measure with a symbol-size tool before and after; a body of a few lines is
  not worth outlining.
- The same reasoning argues for `dyn Trait` when call sites are many and the
  body is large; outlining keeps static dispatch where it matters.

## See Also

- [trait-dyn-vs-generic](trait-dyn-vs-generic.md) - the other lever on the same trade-off
- [api-impl-asref](api-impl-asref.md) - the ergonomic bound this keeps affordable
- [opt-inline-small](opt-inline-small.md) - inlining decisions at the same boundary
- [anti-over-abstraction](anti-over-abstraction.md) - fewer generic parameters, less duplication
