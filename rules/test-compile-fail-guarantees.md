# test-compile-fail-guarantees

> Pin every type-system-only guarantee with a committed compile-fail test

## Why It Matters

A guarantee that lives only in the type system is a claim about code that must
*not* exist, and no runtime test can observe it: the suite stays exactly as
green after `#[derive(Clone)]` lands on a single-use token, after a parameter
relaxes from `Ticket` to `&Ticket`, or after a state-gated method moves into a
blanket `impl<S>`. Those edits are usually one-line conveniences made to
silence an unrelated borrow error, and review reads them as harmless. The
pattern keeps its shape and the documentation keeps its promise while the
property it was built for is gone. A compile-fail case is the only assertion
that turns red when the impossible becomes possible.

## Bad

```rust
#[derive(Clone)]                       // added to fix a borrow error in a caller
pub struct AdminToken(Uuid);

pub fn purge(token: &AdminToken) {}    // signature loosened from `token: AdminToken`

#[cfg(test)]
mod tests {
    #[test]
    fn purge_removes_every_row() {
        let token = issue_token();
        purge(&token);
        assert_eq!(count_rows(), 0);   // passes; the token is now reusable forever
    }
}
```

## Good

The suite that pins the guarantees is a directory of cases that must fail, each
paired with the compiler message it must fail with, driven by a single UI-test
harness:

```text
tests/compile_fail.rs           harness: one test, points a UI runner at tests/ui/*.rs
tests/ui/ticket_reuse.rs        spends a single-use token twice
tests/ui/ticket_reuse.stderr    expected E0382: use of moved value
tests/ui/closed_session.rs      calls a state-gated method in the wrong state
tests/ui/closed_session.stderr  expected E0599: no method named `close`
tests/ui/foreign_impl.rs        implements a sealed trait from a separate crate
tests/ui/foreign_impl.stderr    expected E0277: the trait bound `Mine: Sealed` is not satisfied
```

The guarantees themselves, plus the in-crate signature pins that `cargo check`
already enforces:

```rust
use std::marker::PhantomData;

/// Deliberately not `Clone` or `Copy`: the value is a permission spent once.
pub struct Ticket(u64);

pub struct Redeemed(pub u64);

/// Consumes the ticket, so a second redemption is a use-after-move error.
pub fn redeem(ticket: Ticket) -> Redeemed {
    Redeemed(ticket.0)
}

pub struct Open;
pub struct Closed;

pub struct Session<S> {
    id: u64,
    _state: PhantomData<S>,
}

impl Session<Open> {
    pub fn open(id: u64) -> Self {
        Session { id, _state: PhantomData }
    }

    pub fn close(self) -> Session<Closed> {
        Session { id: self.id, _state: PhantomData }
    }
}

impl<S> Session<S> {
    pub fn id(&self) -> u64 {
        self.id
    }
}

// Signature pins: these stop compiling if `redeem` is loosened to `&Ticket`,
// or if `close` stops consuming the receiver or stops changing the state
// parameter. They need no dev-dependency and run in every `cargo check`.
const _: fn(Ticket) -> Redeemed = redeem;
const _: fn(Session<Open>) -> Session<Closed> = Session::close;

fn main() {
    let ticket = Ticket(7);
    assert_eq!(redeem(ticket).0, 7, "the permitted path still works");
    // redeem(ticket) here: error[E0382], use of moved value.

    let session = Session::<Open>::open(42);
    let closed = session.close();
    assert_eq!(closed.id(), 42);
    // closed.close() here: error[E0599], no method `close` on `Session<Closed>`.
}
```

## Key Points

- Give each guarantee its own case, named after the mistake it catches:
  spending a token twice, calling a method in the wrong state, implementing a
  sealed trait externally, adding two incompatible unit newtypes.
- Assert the expected compiler message, not merely that compilation failed. A
  case that breaks on a renamed import or a typo satisfies a bare
  "does-not-compile" check while testing nothing.
- Recorded stderr snapshots are toolchain-sensitive. Record them on the pinned
  stable toolchain CI uses, and treat regeneration after an upgrade as ordinary
  maintenance rather than a reason to drop the assertion.
- Each case compiles a crate, so the suite belongs in a job that tolerates the
  wall time; one harness test walking the directory keeps it to a single
  target.
- Guarantees phrased as "outside code cannot do this" must be exercised from
  outside. A case compiled inside the crate can reach the private sealing
  module and proves nothing about downstream users.
- Signature pins catch the loosened-parameter half in-crate and cost nothing,
  but no in-crate construct catches an added derive — that half needs the
  compile-fail case.
- Derive macros and other generated code earn the same treatment: a rejection
  path that silently stops rejecting is invisible to every runtime test.

## See Also

- [type-single-use-token](type-single-use-token.md) - the absent `Clone` that a compile-fail case pins
- [api-typestate](api-typestate.md) - state-gated methods are the guarantee under test
- [api-sealed-trait](api-sealed-trait.md) - an external impl attempt is a compile-fail case
- [test-observable-coverage](test-observable-coverage.md) - runtime coverage cannot reach a guarantee about absent code
