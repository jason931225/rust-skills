# api-scoped-closure-access

> Lend a resource that needs setup and teardown through a closure, not through paired open and close methods

## Why It Matters

An API that exposes `begin()` and `end()`, or `lock()` and `unlock()`, relies
on every caller pairing them on every path. An early `?`, a `break`, or a panic
between the two leaves the resource in its intermediate state — a terminal left
in raw mode, a transaction open, a device still claimed. Lending the resource
to a caller-supplied closure makes the pairing structural: the library performs
the teardown after the closure returns, on every path, and the borrow prevents
the caller from keeping the handle past the scope.

## Bad

```rust
let terminal = Terminal::new();
terminal.enter_raw_mode()?;
let input = read_line()?;      // early return leaves the terminal in raw mode
terminal.leave_raw_mode()?;
```

## Good

```rust
#[derive(Debug, PartialEq)]
pub enum Mode {
    Normal,
    Raw,
}

pub struct Terminal {
    mode: Mode,
}

/// Borrowed handle: it cannot outlive the closure, so the caller cannot keep
/// using the terminal after the mode is restored.
pub struct RawTerminal<'a> {
    terminal: &'a mut Terminal,
}

impl RawTerminal<'_> {
    pub fn mode(&self) -> &Mode {
        &self.terminal.mode
    }
}

impl Terminal {
    pub fn new() -> Self {
        Self { mode: Mode::Normal }
    }

    /// Sets up, lends, and restores — on every path out of the closure.
    pub fn with_raw_mode<T>(&mut self, body: impl FnOnce(&mut RawTerminal<'_>) -> T) -> T {
        self.mode = Mode::Raw;
        let mut handle = RawTerminal { terminal: self };
        let outcome = body(&mut handle);
        self.mode = Mode::Normal;
        outcome
    }
}

fn main() {
    let mut terminal = Terminal::new();

    let seen = terminal.with_raw_mode(|raw| {
        assert_eq!(*raw.mode(), Mode::Raw);
        "read something"
    });

    assert_eq!(seen, "read something");
    // Restored without the caller doing anything.
    assert_eq!(terminal.mode, Mode::Normal);
}
```

## Key Points

- Return the closure's value so the shape composes; returning `()` forces
  callers back into out-parameters.
- A closure returning `Result` still restores state, because the restore
  happens after the call rather than inside it.
- Panic safety needs more: run the teardown from a guard's `Drop` if a panic
  must not skip it, since a plain sequential restore is skipped while unwinding.
- Keep the handle borrowed from the owner, not owned by the closure, so it
  cannot be stored and used later.
- Where callers genuinely need to interleave other work, expose the guard type
  instead and let `Drop` do the teardown — the same contract, weaker sequencing.

## See Also

- [test-fixture-raii](test-fixture-raii.md) - the `Drop`-based form of the same idea
- [closure-fn-trait-bounds](closure-fn-trait-bounds.md) - which `Fn` bound the body needs
- [conc-scoped-threads](conc-scoped-threads.md) - scoped borrowing with the same shape
- [mem-drop-order](mem-drop-order.md) - what runs, and when, on the way out
