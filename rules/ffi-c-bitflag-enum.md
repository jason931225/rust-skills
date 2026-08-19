# ffi-c-bitflag-enum

> Model a C bitmask constant group as a newtype integer with associated constants, not as a `#[repr(C)]` fieldless enum

## Why It Matters

A `#[repr(C)]` fieldless Rust enum has exactly the discriminants its variants
list — nothing else is a legal value of that type, even though the compiler
represents it as a plain integer underneath. Many C headers define related
constants as a small integer type meant to be OR'd together
(`READ = 1, WRITE = 2, EXEC = 4`), and it is tempting to mirror that group as
a Rust enum with one variant per constant. That mapping is a category error:
the C side never claimed `READ | WRITE` (`3`) was one of its named values, so
transmuting or reinterpreting the bitwise-OR result as the Rust enum produces
an enum instance holding a discriminant that is not one of its variants —
undefined behavior the moment the value exists, whether or not anything
matches on it. The Rust type that actually matches a C bitmask's contract is
an integer newtype with associated constants and bit operators, not an enum.

## Contract

- Model a C header's bitmask constants (values meant to be combined with `|`)
  as a `#[repr(transparent)]` newtype over the C integer type, with each
  constant as an associated `const` on that type, not as enum variants.
- Implement `BitOr`/`BitAnd`/`BitOrAssign` (or reuse a maintained flags crate)
  on the newtype so combining and testing flags stays type-checked without
  ever constructing an enum value that is not one of its declared variants.
- Reserve `#[repr(C)]` fieldless enums for C headers that genuinely enumerate
  mutually exclusive values — where the C side itself never produces a value
  outside the listed set.
- If existing code already treats such a group as an enum, do not "fix" it by
  adding every OR'd combination as its own variant; the combinations are
  unbounded across bit-widths and the fix reintroduces the same category
  error one flag at a time.
- Validate a value crossing the FFI boundary against the known bit range (or
  against a computed "all flags" mask) before trusting it, the same way any
  other untrusted integer from C is validated.

## Bad

```rust
// Mirrors a C header's `PermissionFlags` group as an enum. Each individual
// value is a legitimate C constant, but the enum admits only those three
// listed discriminants — nothing about `#[repr(C)]` makes `1 | 2` a fourth.
#[repr(C)]
enum PermissionFlags {
    Read = 1,
    Write = 2,
    Exec = 4,
}

fn combine(a: PermissionFlags, b: PermissionFlags) -> PermissionFlags {
    // UB: constructs a `PermissionFlags` holding the bit pattern `3`, which
    // is not `Read`, `Write`, or `Exec` — the enum has no such variant.
    unsafe { std::mem::transmute(a as u8 | b as u8) }
}
```

## Good

```rust
use std::ops::{BitOr, BitOrAssign};

/// Every possible `u8` bit pattern is a valid `Permissions` value — there is
/// no discriminant list to violate — so combining flags never risks
/// constructing an invalid instance of the type.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions(u8);

impl Permissions {
    pub const NONE: Self = Self(0);
    pub const READ: Self = Self(1);
    pub const WRITE: Self = Self(2);
    pub const EXEC: Self = Self(4);

    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }
}

impl BitOr for Permissions {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Permissions {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

fn main() {
    let combined = Permissions::READ | Permissions::WRITE;
    assert!(combined.contains(Permissions::READ));
    assert!(combined.contains(Permissions::WRITE));
    assert!(!combined.contains(Permissions::EXEC));

    // Every bit pattern, including ones with no named constant, is a value
    // this type can legitimately hold — there is nothing to violate.
    let mut mask = Permissions::NONE;
    mask |= Permissions::EXEC;
    assert_eq!(mask, Permissions::EXEC);
}
```

## Failure Tests

- combining two flags with `|` and testing each with `contains` reports both
  present and the third absent;
- a bit pattern with no matching named constant (e.g. the top bit set) is a
  legitimate `Permissions` value and does not panic or fail to construct;
- `Permissions` is `#[repr(transparent)]` over its integer, so its size and
  alignment match the C type exactly, unlike an enum whose discriminant width
  is not guaranteed by `#[repr(C)]` alone;
- an audit of every `#[repr(C)]` fieldless enum in the FFI layer confirms none
  of them is ever combined with `|` or reconstructed from an arbitrary integer
  outside its declared discriminants.

## See Also

- [type-repr-transparent](type-repr-transparent.md) - the newtype-ABI mechanism this rule relies on
- [api-newtype-safety](api-newtype-safety.md) - the general case for a newtype over a primitive
- [unsafe-byte-slice-cast](unsafe-byte-slice-cast.md) - the validity obligation this rule sidesteps by making every bit pattern legal
- [ffi-status-to-result](ffi-status-to-result.md) - converting a different kind of C integer contract into a safe Rust type
- [num-nonzero](num-nonzero.md) - a sibling case where the valid-value set is the type's whole point, not something to route around
