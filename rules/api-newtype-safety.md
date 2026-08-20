# api-newtype-safety

> Use newtypes to prevent mixing semantically different values

## Why It Matters

Raw primitives like `u64` or `String` carry no semantic meaning. A function taking `(u64, u64)` can easily be called with arguments swapped. Newtypes wrap primitives in distinct types, making the compiler catch mistakes at compile time rather than runtime.

## Bad

```rust
struct User {
    id: u64,
    group_id: u64,
    created_at: u64,  // Unix timestamp
}

fn add_user_to_group(user_id: u64, group_id: u64) { ... }

// Bug: arguments swapped - compiles fine, fails at runtime
let user = User { id: 100, group_id: 5, created_at: 1234567890 };
add_user_to_group(user.group_id, user.id);  // Silent bug!

// Bug: wrong field used - timestamp passed as ID
add_user_to_group(user.created_at, user.group_id);  // Compiles fine!
```

## Good

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UserId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GroupId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Timestamp(u64);

struct User {
    id: UserId,
    group_id: GroupId,
    created_at: Timestamp,
}

fn add_user_to_group(user_id: UserId, group_id: GroupId) { ... }

// Compile error: expected UserId, found GroupId
let user = User { ... };
add_user_to_group(user.group_id, user.id);  // Error!

// Compile error: expected UserId, found Timestamp
add_user_to_group(user.created_at, user.group_id);  // Error!
```

## Derive Common Traits

```rust
// Minimal: just enough for your use case
#[derive(Debug, Clone, Copy)]
struct MeterId(u32);

// Full ID type: hashable, comparable, displayable
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OrderId(u64);

impl std::fmt::Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ORD-{:08}", self.0)
    }
}

// With serde for serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]  // Serializes as raw u64
struct ProductId(u64);
```

## Constructor Patterns

```rust
// An owned `String` field is not `Copy`, so this newtype derives `Clone` only.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Email(String);

impl Email {
    /// Creates a new Email, validating the format.
    pub fn new(s: &str) -> Result<Self, EmailError> {
        if is_valid_email(s) {
            Ok(Email(s.to_string()))
        } else {
            Err(EmailError::InvalidFormat)
        }
    }
    
    /// Returns the email as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Usage enforces validation
let email = Email::new("user@example.com")?;  // Must go through validation
```

## Layout Only When It Is a Contract

```rust
use std::mem::size_of;

#[derive(Clone, Copy)]
#[repr(transparent)]
struct Miles(f64);

#[derive(Clone, Copy)]
#[repr(transparent)]
struct Kilometers(f64);

// repr(transparent) makes same layout/ABI an explicit promise.
assert_eq!(size_of::<Miles>(), size_of::<f64>());
assert_eq!(size_of::<Kilometers>(), size_of::<f64>());

// The function accepts exactly one unit.
fn drive(distance: Miles) {
    println!("driving {} miles", distance.0);
}

let km = Kilometers(100.0);
// drive(km); // rejected: expected Miles, found Kilometers

// Explicit conversion
impl From<Kilometers> for Miles {
    fn from(km: Kilometers) -> Self {
        Miles(km.0 * 0.621371)
    }
}

drive(km.into());  // Explicit, visible conversion.
```

A logical newtype usually needs no representation attribute; let the compiler
choose its layout. Add `repr(transparent)` only when same layout/ABI is itself
part of a reviewed FFI or storage contract. It does not make a conversion or
pointer cast semantically safe.

## When Newtypes Help Most

```rust
// ✅ IDs that could be confused
fn transfer(from: AccountId, to: AccountId, amount: Money) { ... }

// ✅ Units that shouldn't mix
struct Celsius(f64);
struct Fahrenheit(f64);

// ✅ Validated strings
struct Username(String);  // Validated alphanumeric
struct Password(String);  // Never logged

// ✅ Different meanings of same type
struct Milliseconds(u64);
struct Seconds(u64);

// ❌ Overkill: single use, no confusion possible
struct X(i32);  // Just use i32
```

## When The Wrapper Is Not Free

This rule asks for wrappers in a lot of places, so it owes an answer on cost.

A newtype that adds nothing but a name is erased. Compiling three functions
that differ only in whether the argument is wrapped, the optimiser does not
merely produce similar code — identical-code-folding emits one body and points
the other symbols at it:

```text
_take_bare:
	ret
	.globl	_take_raw
_take_raw = _take_bare
```

What is not free is anything the wrapper *does*. A `Drop` impl is the common
case: its body is inserted at every scope exit, so the wrapper costs whatever
the body costs, not a fixed overhead. An empty `Drop` still folds away at
`-O`; a `Drop` that writes something does not:

```text
_take_bare:                    _take_noisy:
	ret                            adrp	x8, ...SINK@PAGE
                                       str	w0, [x8, ...]
                                       ret
```

The same goes for a wrapper that adds a field, or that stops being `Copy` —
those change size and move semantics, which is a different question from
whether the name costs anything.

Two cautions if you go looking at the assembly to check a specific case. Only
do it when a measurement has already put the code in a hot path, since this
tells you nothing about whether the wrapper matters. And expect the folded
form: the naive comparison of two disassembly listings can show an alias line
rather than two matching bodies, which reads as "the function is missing"
rather than as the strongest possible confirmation.

## See Also

- [type-newtype-ids](./type-newtype-ids.md) - Newtype pattern for IDs
- [api-parse-dont-validate](./api-parse-dont-validate.md) - Type-driven validation
- [own-copy-small](./own-copy-small.md) - Making newtypes Copy
- [api-param-order](./api-param-order.md) - Keep related parameters in one order so newtypes are not the only defense
