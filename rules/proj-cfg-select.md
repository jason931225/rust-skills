# proj-cfg-select

> Use `cfg_select!` for one-of-many conditional items or expressions

## Why It Matters

Repeated `#[cfg(...)]` attributes make mutually exclusive platform branches hard to audit. Two predicates can overlap and define an item twice, or leave a supported target with no definition. Rust 1.95 stabilized `cfg_select!`, which evaluates branches in order and keeps only the first match. A visible `_` fallback makes the coverage decision explicit in one place.

## Bad

```rust
#[cfg(unix)]
fn path_separator() -> char {
    '/'
}

#[cfg(windows)]
fn path_separator() -> char {
    '\\'
}

// A new target needs another detached definition. Overlapping custom cfgs can
// also select two definitions and fail far from this code.
```

## Good

```rust
cfg_select! {
    unix => {
        fn path_separator() -> char {
            '/'
        }
    }
    windows => {
        fn path_separator() -> char {
            '\\'
        }
    }
    _ => {
        compile_error!("unsupported path platform");
    }
}

fn native_label() -> &'static str {
    cfg_select! {
        target_os = "linux" => "linux",
        target_os = "macos" => "macos",
        target_os = "windows" => "windows",
        _ => "other",
    }
}
```

## Branch Ordering And Cfg Checking

- Branches are tested in source order; the first matching branch wins. Order overlapping predicates from most specific to least specific.
- Use `_` for an intentional fallback. Prefer `compile_error!` when an unsupported target must fail closed.
- `cfg_select!` works in item and expression position and is available from the prelude.
- Keep `check-cfg` declarations for custom cfg names. Selection does not validate spelling or allowed values.
- Use ordinary `#[cfg]` for one independent item; use `cfg_select!` when the branches represent one choice.

## Choosing The Predicate That Actually Distinguishes The Targets

Branch ordering only helps once the branches test the right key. Ask the
compiler which keys differ rather than guessing, with `rustc --print cfg
--target <triple>`; the answers are frequently narrower than expected.

The two Windows ABIs differ in **exactly one** key:

```text
$ diff <(rustc --print cfg --target x86_64-pc-windows-gnu  | sort) \
       <(rustc --print cfg --target x86_64-pc-windows-msvc | sort)
6c6
< target_env="gnu"
---
> target_env="msvc"
```

`windows`, `target_os`, `target_family`, `target_arch`, `target_vendor`, and
`target_pointer_width` are identical between them. So `#[cfg(windows)]` cannot
select between MinGW and MSVC — it matches both — and a branch meant to pick a
C runtime has to test `target_env`. The same key separates `gnu` from `musl`
on Linux, which is why an `#[cfg(target_os = "linux")]` arm cannot tell a
static musl build from a glibc one.

Pointer width is not an architecture check either:

```text
x86_64-unknown-linux-gnu       target_arch="x86_64"  target_pointer_width="64"
aarch64-unknown-linux-gnu      target_arch="aarch64" target_pointer_width="64"
```

Select on `target_pointer_width` only when the code genuinely depends on the
width of a pointer, and on `target_arch` when it depends on the instruction
set. Choosing the wrong one produces a branch that is right on the machine you
tested and silently wrong on the next target that shares the key.

## See Also

- [lint-cfg-check](./lint-cfg-check.md) - declare custom cfg names and reject typos
- [proj-feature-additive](./proj-feature-additive.md) - keep Cargo features additive rather than mutually exclusive
- [proj-mod-by-feature](./proj-mod-by-feature.md) - organize feature-specific implementation code coherently
