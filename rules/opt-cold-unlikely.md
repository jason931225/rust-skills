# opt-cold-unlikely

> Mark unlikely code paths with `#[cold]` to help compiler optimization

## Why It Matters

`#[cold]` is a code-generation hint that a function is unlikely to be called.
It can influence caller optimization and code layout, but Rust does not
guarantee a separate section, branch probability, or performance gain. Apply
it only to an actually rare, sizeable path identified by representative
profiles; benchmark and inspect supported-target output before retaining it.

## Bad

```rust
// No profile evidence supports labeling these input failures cold.
fn validate(input: &str) -> Result<Data, ValidationError> {
    if input.is_empty() {
        return Err(ValidationError::Empty);  // Rare
    }
    
    if input.len() > 1000 {
        return Err(ValidationError::TooLong);  // Rare  
    }
    
    if !input.is_ascii() {
        return Err(ValidationError::NonAscii);  // Rare
    }
    
    // This is the common case
    Ok(parse_data(input))
}
```

## Good

```rust
fn validate(input: &str) -> Result<Data, ValidationError> {
    if input.is_empty() {
        return cold_empty_error();
    }
    
    if input.len() > 1000 {
        return cold_too_long_error();
    }
    
    if !input.is_ascii() {
        return cold_non_ascii_error();
    }
    
    Ok(parse_data(input))
}

#[cold]
fn cold_empty_error() -> Result<Data, ValidationError> {
    Err(ValidationError::Empty)
}

#[cold]
fn cold_too_long_error() -> Result<Data, ValidationError> {
    Err(ValidationError::TooLong)
}

#[cold]
fn cold_non_ascii_error() -> Result<Data, ValidationError> {
    Err(ValidationError::NonAscii)
}
```

## What #[cold] May Influence

`#[cold]` is a hint. It tells the code generator that a function is unlikely to
be called; none of the following is guaranteed, and all of them may differ
between backends, optimization levels, and releases:

1. **Code placement**: the function may be emitted away from hot code, so hot
   paths pack more densely into instruction cache lines.
2. **Branch weighting**: branches reaching it may be weighted as unlikely.
3. **Inlining**: it is less likely to be inlined into a hot caller, though
   `#[inline(never)]` is what actually forbids that.
4. **Optimization effort**: the backend may spend less budget on it.

Verify the generated code before relying on any of these — see the
verification note below.

## Common Cold Patterns

```rust
// Error handling
#[cold]
fn handle_error<E: std::fmt::Display>(e: E) -> ! {
    eprintln!("Fatal error: {}", e);
    std::process::exit(1);
}

// Logging rare events
#[cold]
fn log_rare_event(event: &Event) {
    log::warn!("Rare event occurred: {:?}", event);
}

// Fallback paths
#[cold]
fn slow_fallback(data: &Data) -> Output {
    // This path should rarely be taken
    compute_slowly(data)
}

// Panic handlers
#[cold]
fn panic_invalid_state(state: &State) -> ! {
    panic!("Invalid state: {:?}", state);
}
```

## Assertions and Invariants

```rust
fn get_with_cold_panic(&self, index: usize) -> &T {
    if index >= self.len {
        cold_bounds_panic(index, self.len);
    }
    // SAFETY: the branch above proves index < len, and the type invariant
    // requires ptr to reference len initialized T values.
    unsafe { &*self.ptr.add(index) }
}

#[cold]
#[inline(never)]
fn cold_bounds_panic(index: usize, len: usize) -> ! {
    panic!("index out of bounds: the len is {} but the index is {}", len, index);
}
```

## Combining with #[inline(never)]

```rust
// Combine only when measurement justifies both hints.
#[cold]
#[inline(never)]
fn error_path() -> Error {
    // Complex error construction stays out of hot code
    Error {
        backtrace: Backtrace::capture(),
        context: gather_context(),
    }
}
```

## Measuring Impact

```rust
// Check code layout with objdump
// objdump -d target/release/binary | less

// Look for .cold sections
// nm target/release/binary | grep cold

// Profile to verify improvement
// perf stat -e cache-misses,cache-references ./binary
```

## See Also

- [opt-inline-never-cold](./opt-inline-never-cold.md) - Combining with inline(never)
- [opt-likely-hint](./opt-likely-hint.md) - Branch prediction hints
- [err-result-over-panic](./err-result-over-panic.md) - Error handling
