# ffi-status-to-result

> Check the status a foreign call returns and convert failure into a `Result` at the boundary

## Why It Matters

C APIs report failure out of band: a negative return, a null pointer, a
non-zero status, `errno`, or `GetLastError`. None of that is visible to Rust's
type system, so an unchecked call returns a value that looks ordinary and the
program continues with a null handle or a half-initialised struct. The error
detail is also perishable — `errno` reflects the *last* failing call, so a
logging statement or an allocation between the call and the check can overwrite
it. Convert at the boundary, immediately, while the evidence still exists.

## Bad

```rust
pub fn open_device(path: &CStr) -> Device {
    // -1 means failure; this constructs a Device around it and moves on
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR) };
    Device { fd }
}
```

## Good

```rust
use std::io;

/// Stands in for a foreign function: negative means failure, and the detail is
/// in the OS error state.
fn foreign_call(succeed: bool) -> i32 {
    if succeed { 3 } else { -1 }
}

/// The conversion happens on the next line after the call, before anything
/// else can disturb the OS error state.
pub fn checked_call(succeed: bool) -> io::Result<i32> {
    let status = foreign_call(succeed);
    if status < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(status)
}

fn main() {
    assert_eq!(checked_call(true).expect("succeeds"), 3);

    let error = checked_call(false).expect_err("fails");
    // The failure is a Rust error with the platform's detail attached, not a
    // sentinel integer travelling further into the program.
    assert!(error.raw_os_error().is_some() || error.kind() != io::ErrorKind::Other);
}
```

## Key Points

- Know each function's failure signal before wrapping it; `-1`, `0`, null, and
  a positive error code are all conventions in use, sometimes in one library.
- Read `errno` (via `io::Error::last_os_error`) or `GetLastError` on the
  statement after the call. Anything in between can overwrite it.
- Wrap the raw handle in a type whose `Drop` releases it, so an early return
  cannot leak it.
- Map foreign error codes into a domain error at the shim; do not leak raw
  status integers into the crate's public API.
- A call that can fail partway may leave out-parameters partly written — treat
  them as uninitialised unless the API documents otherwise.

## See Also

- [ffi-logic-in-core](ffi-logic-in-core.md) - keep translation in the shim and logic in the safe core
- [unsafe-extern-block](unsafe-extern-block.md) - declaring the foreign functions being checked
- [err-canonical-struct](err-canonical-struct.md) - the domain error the status becomes
- [ffi-native-escape-hatch](ffi-native-escape-hatch.md) - handing the native handle back out deliberately
