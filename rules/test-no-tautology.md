# test-no-tautology

> Assert a property or observable outcome, not a constant restated from the source

## Why It Matters

A test that copies `[0, 90, 180, 270]` into both the production item and the assertion cannot fail unless someone edits only one side. Those tests satisfy a coverage counter and then rot. The Microsoft Pragmatic Rust Guidelines reject tautological tests: check a property the values must keep (spacing, monotonicity, a parse round-trip) or drop the test.

## Bad

```rust
const CHECKPOINTS: [u32; 4] = [0, 90, 180, 270];

#[test]
fn checkpoints_are_correct() {
    assert_eq!(CHECKPOINTS, [0, 90, 180, 270]);
}
```

## Good

```rust
const CHECKPOINTS: [u32; 4] = [0, 90, 180, 270];

fn checkpoints_are_evenly_spaced(points: &[u32]) -> bool {
    points.windows(2).all(|pair| pair[1] - pair[0] == 90)
}

fn main() {
    assert!(checkpoints_are_evenly_spaced(&CHECKPOINTS));
    assert_eq!(CHECKPOINTS[0], 0);
    assert_eq!(*CHECKPOINTS.last().unwrap(), 270);
}
```

## See Also

- [test-descriptive-names](test-descriptive-names.md) - a name that states the property makes a tautology obvious
- [test-proptest-properties](test-proptest-properties.md) - generate inputs instead of restating one fixture
- [test-arrange-act-assert](test-arrange-act-assert.md) - keep the expected value in Assert, not copied into Arrange
