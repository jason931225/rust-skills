# ffi-opaque-handle-lifecycle

> Hand C an opaque pointer from `Box::into_raw` and exactly one paired free, and validate it on every call

## Why It Matters

Exporting a Rust object across the C ABI means giving up the compiler's
ownership tracking at the boundary, so the contract has to be written down and
enforced by hand. Two frees on the same handle is a double free; a call after
free is a use-after-free; returning a pointer into a `Vec` that Rust still owns
invites the caller to outlive it. None of these are visible on the Rust side,
which is why the shim needs exactly one constructor, exactly one destructor,
and a null check at every entry point.

## Bad

```rust
#[unsafe(no_mangle)]
pub extern "C" fn parser_new() -> *mut Parser {
    &mut Parser::new()          // dangling immediately: the value is dropped
}

#[unsafe(no_mangle)]
pub extern "C" fn parser_feed(parser: *mut Parser, byte: u8) {
    unsafe { (*parser).feed(byte) }   // no null check, no ownership contract
}
```

## Good

```rust
pub struct Parser {
    fed: usize,
}

impl Parser {
    fn new() -> Self {
        Self { fed: 0 }
    }

    fn feed(&mut self, _byte: u8) {
        self.fed += 1;
    }
}

/// Transfers ownership to the caller. The only way to get a handle.
pub extern "C" fn parser_new() -> *mut Parser {
    Box::into_raw(Box::new(Parser::new()))
}

/// Borrows for the duration of the call; the handle stays the caller's.
///
/// # Safety
///
/// `parser` must be a handle from `parser_new` that has not been freed.
pub unsafe extern "C" fn parser_feed(parser: *mut Parser, byte: u8) -> i32 {
    let Some(parser) = (unsafe { parser.as_mut() }) else {
        return -1; // null is a caller error, not a crash
    };
    parser.feed(byte);
    0
}

/// The single paired free. Takes ownership back and drops it.
///
/// # Safety
///
/// `parser` must come from `parser_new` and must not be used afterwards.
pub unsafe extern "C" fn parser_free(parser: *mut Parser) {
    if !parser.is_null() {
        drop(unsafe { Box::from_raw(parser) });
    }
}

fn main() {
    let handle = parser_new();
    // SAFETY: `handle` came from `parser_new` and has not been freed.
    unsafe {
        assert_eq!(parser_feed(handle, b'a'), 0);
        assert_eq!(parser_feed(std::ptr::null_mut(), b'a'), -1, "null is reported");
        parser_free(handle);
        parser_free(std::ptr::null_mut()); // freeing null is a no-op
    }
}
```

## Boundary Obligations For Handles

- One constructor, one destructor, and say in the header which is which; a
  handle freed by `free()` instead of your function is a heap mismatch.
- Never return a pointer into memory Rust still owns — `Vec::as_ptr` outlives
  nothing.
- Check null at every entry point and report it as a status code rather than
  dereferencing.
- Catch unwinding at the boundary: a panic that escapes an `extern "C"` function aborts the process (it is undefined behaviour only for an ABI without unwind support); a panic crossing the C ABI is undefined
  behaviour.
- Keep the handle opaque in the header (a forward-declared struct), so the
  caller cannot depend on the layout or construct one itself.

## See Also

- [ffi-status-to-result](ffi-status-to-result.md) - the reverse direction, checking a foreign status
- [ffi-logic-in-core](ffi-logic-in-core.md) - keep the shim thin and the logic in a safe crate
- [err-catch-unwind-boundary](err-catch-unwind-boundary.md) - stopping a panic at the ABI edge
- [unsafe-safety-comment](unsafe-safety-comment.md) - the proof each block carries
