# unsafe-sound-abstractions

> Never expose a safe API that can hit undefined behavior; if the caller must uphold a UB precondition, the function is `unsafe`

## Why It Matters

*Safe* and `unsafe` are technical terms, not severity labels: a safe function
can be operationally disastrous, while an `unsafe` one can be routine when the
caller upholds its contract. An abstraction is unsound when any call available
to safe code, including an unusual or theoretical path, can produce undefined
behavior. Unsound abstractions are never allowed. If the invariant cannot be
established inside the API, mark the entry `unsafe`, write its `# Safety`
contract, and give the obligation to the caller.

## Bad

```rust
pub fn word_bits<T>(value: &T) -> &u64 {
    unsafe { &*(value as *const T as *const u64) }
}

pub struct Carry<T>(T);

unsafe impl<T> Send for Carry<T> {}
unsafe impl<T> Sync for Carry<T> {}

fn main() {
    let n = 1u8;
    let _ = word_bits(&n);
}
```

## Good

```rust
/// # Safety
///
/// `ptr` must be aligned and valid for a `u32` read.
pub unsafe fn load_word(ptr: *const u32) -> u32 {
    // SAFETY: caller promised a live, aligned `u32`.
    unsafe { ptr.read() }
}

mod words {
    pub struct Word([u8; 4]);

    impl Word {
        pub fn from_le_bytes(bytes: [u8; 4]) -> Self {
            Self(bytes)
        }

        pub fn get(&self) -> u32 {
            u32::from_le_bytes(self.0)
        }
    }
}

fn main() {
    let stored = 7u32;
    // SAFETY: `stored` is a live, aligned `u32` for this call.
    let n = unsafe { load_word(&stored) };
    assert_eq!(n, 7);
    assert_eq!(words::Word::from_le_bytes(7u32.to_le_bytes()).get(), 7);
}
```

## No Exceptions

Most rules yield when the alternative is worse. This one does not. Unsound code is never acceptable, even as a temporary shortcut, a test helper, or an internal "we would never call it that way" function. If safe callers can reach UB, the API is wrong.

## Where Soundness Is Judged

- Soundness is judged from *other safe code*, including adversarial `Deref` / `Clone` / `Drop` impls and unusual but legal generic instantiations.
- The soundness boundary is the **module**, not the function. A safe method may rely on an invariant that another item in the *same* module established and that privacy keeps outsiders from breaking.
- Crossing a module (or crate) line without re-establishing the invariant is a new API. If that API's callers must promise something the compiler cannot check, it is `unsafe`.
- Dangerous-but-defined work stays safe (`unsafe-means-ub`). Missing a `Send` bound is not a license to implement `Send` for every `T`.
- A WASM export the host can call is the same boundary as any other FFI entry
  point: a `pub extern "C"` function that reconstructs a `&str`/`String` from
  host-supplied `(ptr, len)` via `from_raw_parts` and `from_utf8_unchecked`
  is unsound if left safe, because the host is an untrusted caller from the
  module's point of view. Mark it `unsafe`, or route it through a generated
  binder that validates the bytes first.

## See Also

- [unsafe-means-ub](unsafe-means-ub.md) - `unsafe` means UB risk, not "this is scary"
- [unsafe-justify-use](unsafe-justify-use.md) - even a sound block still needs a reason to exist
- [unsafe-safety-comment](unsafe-safety-comment.md) - write the contract next to every `unsafe` block
- [doc-safety-section](doc-safety-section.md) - `# Safety` on every `unsafe fn`
- [unsafe-send-sync-manual](unsafe-send-sync-manual.md) - a blanket `Send`/`Sync` impl is the usual unsound shortcut
