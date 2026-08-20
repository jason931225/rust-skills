# type-generic-bounds

> Add trait bounds only where needed, prefer where clauses for readability

## Why It Matters

Trait bounds constrain what types can be used with generic code. Adding unnecessary bounds limits flexibility. Adding bounds in the right place (impl vs function vs where clause) affects usability and readability. Well-placed bounds keep APIs flexible while ensuring type safety.

## Bad

```rust
// Bounds on struct definition - limits all uses
struct Container<T: Clone + Debug> {  // Even storage requires Clone?
    items: Vec<T>,
}

// Inline bounds make signature hard to read
fn process<T: Clone + Debug + Send + Sync + 'static, E: Error + Send + Clone>(
    value: T
) -> Result<T, E> { ... }

// Redundant bounds
fn print_twice<T: Clone + Debug>(value: T)
where
    T: Clone,  // Already specified above
{ ... }
```

## Good

```rust
// No bounds on struct - store anything
struct Container<T> {
    items: Vec<T>,
}

// Bounds only on impls that need them
impl<T: Clone> Container<T> {
    fn duplicate(&self) -> Self {
        Container { items: self.items.clone() }
    }
}

impl<T: Debug> Container<T> {
    fn debug_print(&self) {
        println!("{:?}", self.items);
    }
}

// Where clause for readability
fn process<T, E>(value: T) -> Result<T, E>
where
    T: Clone + Debug + Send + Sync + 'static,
    E: Error + Send + Clone,
{ ... }
```

## Bound Placement

```rust
// On struct: affects all uses of the type
struct MustBeClone<T: Clone> { data: T }  // Rarely needed

// On impl: affects specific functionality
impl<T: Clone> Container<T> { ... }  // Common pattern

// On function: affects that function only
fn requires_send<T: Send>(value: T) { ... }

// Recommendation: start with no bounds, add as needed
```

## Where Clause Benefits

```rust
// Inline: hard to read
fn complex<T: Clone + Debug + Send, U: AsRef<str> + Into<String>>(t: T, u: U) { }

// Where clause: clear and scannable
fn complex<T, U>(t: T, u: U)
where
    T: Clone + Debug + Send,
    U: AsRef<str> + Into<String>,
{ }

// Essential for complex bounds
fn foo<T, U>(t: T, u: U)
where
    T: Iterator<Item = U>,
    U: Clone + Into<String>,
    Vec<U>: Debug,  // Bounds on expressions
{ }
```

## The Implicit `Sized` Bound

Every type parameter carries an invisible `T: Sized` bound. That is the right
default — it is what lets a generic hold a `T` by value — but it silently
excludes the unsized types: slices, `str`, and every `dyn Trait`. A wrapper
that only ever holds its `T` behind a pointer does not need the bound, and
leaving it in place makes the wrapper unusable with exactly the types a
pointer-holding wrapper exists for.

```rust
use std::fmt::Debug;

/// Implicitly `T: Sized`, so this cannot hold a trait object even though the
/// `Box` makes the size irrelevant. `Holder<dyn Debug>` fails to compile with
/// E0277, and the error names the cause: "required by an implicit `Sized`
/// bound in `Holder`".
struct Holder<T> {
    inner: Box<T>,
}

/// Relaxing the bound is what makes the wrapper work for the unsized types.
struct Relaxed<T: ?Sized> {
    inner: Box<T>,
}

fn main() {
    let sized = Holder { inner: Box::new(5u8) };
    assert_eq!(*sized.inner, 5);

    let unsized_: Relaxed<dyn Debug> = Relaxed { inner: Box::new(5u8) };
    assert_eq!(format!("{:?}", unsized_.inner), "5");
    // `Holder<dyn Debug>` does not compile.
}
```

Relax the bound when the parameter is only ever reached through a reference,
`Box`, `Rc`, or `Arc`. Keep it whenever the code stores, returns, or moves a
`T` by value — `?Sized` is not a free generalization, and adding it to a type
that genuinely needs a size just moves the error somewhere less obvious.

Note the size consequence, because it is easy to miss when budgeting a type:
a reference to an unsized type is a *wide* pointer carrying a payload pointer
plus a length or vtable pointer, so it is two words rather than one.
`&[u8]` and `&dyn Trait` are 16 bytes on a 64-bit target where `&u8` is 8.
`Option<Box<dyn Trait>>` is still 16, because the null niche in the data
pointer absorbs the discriminant.

## Implied Bounds

```rust
// Supertrait bounds are implied
trait Foo: Clone + Debug {}

fn process<T: Foo>(value: T) {
    // T: Clone and T: Debug are implied by T: Foo
    let cloned = value.clone();
    println!("{:?}", cloned);
}

// Associated type bounds
fn process<I>(iter: I)
where
    I: Iterator,
    I::Item: Clone,  // Bound on associated type
{ }
```

## Conditional Trait Implementation

```rust
struct Wrapper<T>(T);

// Implement Clone only when T: Clone
impl<T: Clone> Clone for Wrapper<T> {
    fn clone(&self) -> Self {
        Wrapper(self.0.clone())
    }
}

// Implement Debug only when T: Debug  
impl<T: Debug> Debug for Wrapper<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Wrapper").field(&self.0).finish()
    }
}

// Wrapper<i32> is Clone + Debug
// Wrapper<NonCloneable> is neither
```

## See Also

- [api-impl-into](./api-impl-into.md) - Using Into bounds
- [api-impl-asref](./api-impl-asref.md) - Using AsRef bounds
- [name-type-param-single](./name-type-param-single.md) - Type parameter naming
- [trait-dyn-vs-generic](./trait-dyn-vs-generic.md) - Static vs dynamic dispatch
- [trait-associated-type-vs-generic](./trait-associated-type-vs-generic.md) - Associated types vs generics
