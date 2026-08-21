# test-observable-coverage

> Cover observable behavior and failure modes so refactors can proceed without implementation-shaped tests

## Why It Matters

A coverage percentage says which lines ran, not whether the contract was
checked; refactoring stays safe when tests exercise outputs, state transitions,
errors, and externally visible side effects through supported APIs. That
evidence also lets coding agents change internals without inventing assumptions
about private branches.

Treat uncovered observable behavior as a product gap. Do not inflate the number
by calling getters, mirroring match arms, or asserting private representation
details.

## Bad

```rust
pub struct Counter {
    value: u64,
}

impl Counter {
    pub fn increment(&mut self) {
        self.value += 1;
    }
}

#[test]
fn counter_starts_at_zero() {
    let counter = Counter { value: 0 };
    assert_eq!(counter.value, 0); // Repeats construction; no behavior ran.
}
```

## Good

```rust
#[derive(Debug, Default, PartialEq)]
pub struct Counter {
    value: u64,
}

#[derive(Debug, PartialEq)]
pub enum CounterError {
    Overflow,
}

impl Counter {
    pub fn increment(&mut self) -> Result<u64, CounterError> {
        self.value = self.value.checked_add(1).ok_or(CounterError::Overflow)?;
        Ok(self.value)
    }
}

#[test]
fn increment_reports_the_new_value() {
    let mut counter = Counter::default();
    assert_eq!(counter.increment(), Ok(1));
    assert_eq!(counter.increment(), Ok(2));
}

#[test]
fn increment_rejects_overflow_without_changing_state() {
    let mut counter = Counter { value: u64::MAX };
    assert_eq!(counter.increment(), Err(CounterError::Overflow));

    // "State unchanged" is checked through behaviour, not by reading the
    // private field: a counter that had silently advanced would report a
    // different value here, and one that saturated would return `Ok`.
    assert_eq!(counter.increment(), Err(CounterError::Overflow));
}
```

## Coverage Policy

- Inventory public operations, branch outcomes, invariants, and named failure modes before choosing a percentage target.
- Measure line or branch coverage to find omissions, then add a test only when the uncovered behavior has an independent oracle.
- Exercise the public surface in integration tests when private access is not required.
- Keep coverage and mutation reports as diagnostic evidence; neither justifies tautological tests.
- Compile and run repository examples because they are user-facing behavior, not decorative snippets.

## Reading The Measurement Itself

The policy above says to measure in order to find omissions. What you measure
with decides whether the number can find them at all.

Rust's `-C instrument-coverage` produces LLVM *source-based* coverage, which
counts execution regions rather than lines. The distinction is not academic — a
line can be fully covered while the branches on it never ran:

```rust
fn classify(n: i32) -> &'static str {
    // Two regions live on this line. A test that only passes a negative number
    // short-circuits at `n > 0`, so `n % 2 == 0` and the `positive-even` arm
    // never execute — while the line itself reports as covered.
    if n > 0 && n % 2 == 0 { "positive-even" } else { "other" }
}

fn main() {
    assert_eq!(classify(-3), "other");
}
```

Instrumenting exactly that program and reporting it gives **100% line coverage
and 80% region coverage**: six of six lines executed, two of ten regions never
did. A line-coverage number would have shown nothing left to do. This is why
the untested half of a short-circuit, and the arm of a condition that is never
taken, are the omissions worth chasing.

## Aggregating Across Test Kinds And Excluding Noise

A crate's observable behavior is exercised by unit tests, integration tests,
doctests, and the examples the policy above requires you to run. Each is a
separate binary and a separate profile, so measuring one run and reporting it
as the crate's coverage understates it — and the gap is not uniform, because
integration tests are usually the only thing touching the public surface that
matters most.

- Merge the profiles from every test kind into one report before reading it.
  Two runs measured separately cannot be compared or added.
- Exclude generated code and paths that are compiled out on the host —
  platform-gated modules, hardware-only branches. Left in, they depress the
  signal permanently and train everyone to ignore it.
- An exclusion is a claim that something is untestable here, so it belongs in
  review like any other claim. A growing exclusion list is the finding.

Both of these are about keeping the number honest enough to be diagnostic.
They are not an argument for a threshold: a merged, region-level report still
answers "what behavior has no test", not "is this crate done".

## Ordering The Gaps By What Breaks If They Are Wrong

Everything above finds gaps. Which to close first is a separate question, and
the report cannot answer it: an uncovered region contributes the same amount to
the number whatever it does.

Order by consequence. An uncovered error path at an authentication, payment, or
persistence boundary outranks an uncovered `Display` impl by a wide margin,
even though closing the second may move the percentage further. The inverse is
also worth acting on — an area with heavy coverage and low consequence is a
candidate for deleting slow tests, not for adding more.

This is a prioritisation aid and not a target in disguise. It says which gap to
close next; it still says nothing about when to stop, because a merged,
region-level report answers "what behavior has no test", not "is this crate
done".

## See Also

- [test-no-tautology](test-no-tautology.md) - an independent oracle matters more than a covered line
- [test-integration-dir](test-integration-dir.md) - test public behavior from a consumer crate
- [test-proptest-properties](test-proptest-properties.md) - cover invariants over generated inputs
- [doc-examples-section](doc-examples-section.md) - keep runnable workflows under `examples/`
