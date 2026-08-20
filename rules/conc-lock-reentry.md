# conc-lock-reentry

> `Mutex` is not reentrant: acquire it once per public entry point, do the work in helpers that take the already-locked data, and give multiple locks one global order

## Why It Matters

`std::sync::Mutex` and `RwLock` are not reentrant, so a thread that locks a
mutex it already holds deadlocks against itself — and the shape that causes it
is ordinary good practice everywhere else: a public method takes the lock and
then calls another public method of the same type, which takes it again. It
compiles cleanly and hangs at runtime, with no panic, no error, and a
backtrace pointing at a lock that looks perfectly reasonable. This is the
single most concentrated concurrency defect measured in real Rust code:
across a study of 59 blocking bugs in production Rust projects, every one
occurred in *safe* code calling synchronization APIs, and 30 of them were
double-acquisitions caused by misunderstanding how long a guard lives.

## Acquire Once, Then Work On The Data

- Give each public method exactly one acquisition. Put the logic in private
  helpers that take the already-locked data (`&[T]`, `&mut State`) rather than
  `&self`, so a helper *cannot* re-acquire — the signature makes it impossible
  instead of leaving it to discipline.
- Never call a public method of the same type while holding its lock. If two
  public methods need the same work, extract it downward into a helper both
  call after locking; do not have one call the other.
- Know where the guard actually dies. It lives to the end of the enclosing
  statement, which is longer than it looks: a guard produced in a `match`
  scrutinee is held for **every arm**, so re-locking inside an arm deadlocks.
  Bind it to a named local and drop it, or restructure, when the body needs
  the lock again.
- Where several locks must be held at once, define one global acquisition
  order and follow it everywhere. Two call sites taking A-then-B and B-then-A
  deadlock only when they interleave, so this reproduces under load and not in
  tests.
- Prefer holding one lock over a coarser structure to holding two fine-grained
  ones, unless contention measurements justify the split. Splitting a lock is
  a performance decision that buys an ordering obligation.
- Reach for a reentrant or recursive lock only deliberately. `std` does not
  provide one, and its absence is a design signal: re-entry usually means the
  invariant the lock protects is not clearly owned by one layer.

## Bad

```rust
use std::sync::Mutex;

struct Bank {
    accounts: Mutex<Vec<u64>>,
}

impl Bank {
    fn balance(&self) -> u64 {
        self.accounts.lock().expect("accounts mutex").iter().sum()
    }

    // Compiles cleanly, hangs forever: the guard from this `lock()` is still
    // alive when `balance()` tries to acquire the same mutex.
    fn audit(&self) -> u64 {
        let guard = self.accounts.lock().expect("accounts mutex");
        let total = self.balance();
        total + guard.len() as u64
    }
}
```

```rust
use std::sync::Mutex;

// The same bug through a temporary: the guard built in the scrutinee lives
// for the whole `match`, so this deadlocks in the arm.
fn deadlocks_in_arm(m: &Mutex<Vec<u64>>) {
    match m.lock().expect("mutex").first().copied() {
        Some(_first) => {
            let _again = m.lock().expect("mutex"); // still held from above
        }
        None => {}
    }
}
```

## Good

```rust
use std::sync::Mutex;

pub struct Bank {
    accounts: Mutex<Vec<u64>>,
}

impl Bank {
    pub fn new(accounts: Vec<u64>) -> Self {
        Self { accounts: Mutex::new(accounts) }
    }

    /// Private, and takes the data rather than `&self`. It has no way to
    /// acquire the lock, so no caller can accidentally re-enter through it.
    fn total_locked(accounts: &[u64]) -> u64 {
        accounts.iter().sum()
    }

    /// One acquisition, then helpers.
    pub fn balance(&self) -> u64 {
        let accounts = self.accounts.lock().expect("accounts mutex");
        Self::total_locked(&accounts)
    }

    /// Also one acquisition — it calls the helper, never `balance()`.
    pub fn audit(&self) -> u64 {
        let accounts = self.accounts.lock().expect("accounts mutex");
        Self::total_locked(&accounts) + accounts.len() as u64
    }
}

fn main() {
    let bank = Bank::new(vec![1, 2, 3]);
    assert_eq!(bank.balance(), 6);
    // Would hang if `audit` called `balance` while holding the guard.
    assert_eq!(bank.audit(), 9);
}
```

## Cases To Pin In Tests

- every public method completes when called directly, and when called while
  another public method of the same type is on the stack — the second case is
  the one that hangs;
- a test that exercises the re-entrant path runs under a timeout, since a
  deadlock is a hang rather than a failure and an untimed test suite waits
  forever rather than reporting;
- where multiple locks are held, a test acquires them in the documented order
  from two threads concurrently; a reversed-order call site is the bug, and it
  will not show up single-threaded;
- a `match` or `if let` whose scrutinee produces a guard does not re-acquire
  in any arm — check the arms, not just the scrutinee.

## See Also

- [own-mutex-interior](own-mutex-interior.md) - choosing `Mutex` for cross-thread interior mutability in the first place
- [async-no-lock-await](async-no-lock-await.md) - the async sibling, where the hazard is holding a guard across a suspension point
- [conc-condvar-predicate-loop](conc-condvar-predicate-loop.md) - the other way a thread waits forever under a mutex
- [mem-drop-order](mem-drop-order.md) - when a guard bound to a local is actually released
- [own-rwlock-readers](own-rwlock-readers.md) - `RwLock` has the same re-entry hazard, plus writer starvation
