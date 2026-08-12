# unsafe-means-ub

> Mark a function `unsafe` only when misuse can cause undefined behavior, not because the operation is merely dangerous

## Why It Matters

`unsafe` is a contract about the abstract machine: call this wrong and you may get a data race, a wild pointer, or a broken validity invariant. Using it as a "this is scary" sticker trains callers to sprinkle `unsafe {}` around `rm -rf`. The Microsoft Pragmatic Rust Guidelines keep `unsafe` for UB risk only. A function that deletes a database, spends money, or pages on-call stays safe, gets a loud name, and documents the blast radius. `clippy::undocumented_unsafe_blocks` and `unsafe_op_in_unsafe_fn` only help if `unsafe` still means UB.

## Bad

```rust
pub unsafe fn delete_database(name: &str) {
    let _ = name;
}

pub fn wipe(name: &str) {
    // Callers now need an unsafe block for a defined, if catastrophic, action.
    unsafe { delete_database(name) }
}
```

## Good

```rust
/// Permanently drops every row in `name`.
///
/// This cannot cause undefined behavior. It *can* destroy production data.
/// Callers must pass a name they have already authorized to erase.
pub fn delete_database(name: &str) {
    let _ = name;
}

/// # Safety
///
/// `ptr` must be valid for a `u32` read.
pub unsafe fn read_u32(ptr: *const u32) -> u32 {
    // SAFETY: caller promised `ptr` is aligned and dereferenceable.
    unsafe { ptr.read() }
}

fn main() {
    delete_database("scratch");
    let value = 7u32;
    let n = unsafe { read_u32(&value) };
    assert_eq!(n, 7);
}
```

## See Also

- [unsafe-safety-comment](unsafe-safety-comment.md) - document the UB preconditions, not the business risk
- [doc-safety-section](doc-safety-section.md) - `# Safety` belongs on `unsafe fn`, not on a safe foot-gun
- [err-result-over-panic](err-result-over-panic.md) - defined failures still return `Result`
