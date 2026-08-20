# trait-capability-mixin

> Declare each required resource as its own associated-type ingredient trait, then bind a method to the conjunction with a supertrait-bounded blanket impl

## Why It Matters

A function that needs two resources together — an SPI bus and an I2C bus to
read a composite sensor, a database handle and a cache to serve a request —
can be written as a free function taking both as parameters, but nothing
then stops a caller from passing two unrelated instances that happen to have
the right types: an SPI bus from one device paired with an I2C bus from a
different one. A single receiver that genuinely owns both resources is a
stronger claim than "I have a value of each type lying around," and Rust can
enforce it: declare each resource as an ingredient trait with an associated
type, then declare the cross-cutting behavior on a mixin trait bounded by
the supertraits for every ingredient it needs, with an empty blanket `impl`
so every type satisfying the whole conjunction gets the behavior for free.
The method exists only on a receiver that provably has every required
ingredient, not on any two values a caller happened to have handy.

## Ingredient And Mixin Requirements

- For each independently-owned resource a type might provide, declare a
  small trait with an associated type naming that resource and an accessor
  method to reach it (`trait HasSpi { type Spi: SpiBus; fn spi(&self) -> &Self::Spi; }`).
- Declare cross-cutting behavior that needs several resources together as a
  mixin trait with a supertrait bound listing every ingredient it needs, and
  give it default methods implemented purely in terms of the ingredient
  accessors.
- Provide the mixin via an empty blanket `impl<T: Ingredient1 + Ingredient2> Mixin for T {}`
  so any type satisfying every supertrait gets the mixin automatically —
  callers never implement the mixin by hand.
- Do not substitute a free function taking each resource as a separate
  parameter for this pattern when the point is that one receiver must own
  the whole set together; a free function accepts any two values of the
  right types, which is exactly the binding this pattern exists to prevent.
- Removing one ingredient trait from a type's implementations should be a
  compile error at every call site that used the mixin — verify this is
  true, not merely that adding the ingredients compiles.

## Bad

```rust
// Any SpiBus and any I2cBus type-check here, including ones that belong to
// two unrelated devices — nothing ties them to the same physical hardware.
fn read_fan_speed(spi: &impl SpiBus, i2c: &impl I2cBus) -> u16 {
    let _ = spi;
    let _ = i2c;
    0
}
```

## Good

```rust
/// Each resource a device might provide is its own ingredient trait.
trait HasSpi {
    type Spi;
    fn spi(&self) -> &Self::Spi;
}

trait HasI2c {
    type I2c;
    fn i2c(&self) -> &Self::I2c;
}

/// The mixin exists only for a receiver that has both ingredients — the
/// supertrait bound is the whole point.
trait FanDiagMixin: HasSpi + HasI2c {
    fn read_fan_speed(&self) -> u16 {
        // Both resources are reached through `self`, so they are
        // guaranteed to belong to the same device.
        let _spi = self.spi();
        let _i2c = self.i2c();
        1200
    }
}

// Every type satisfying both supertraits gets the mixin automatically;
// nobody implements `FanDiagMixin` by hand.
impl<T: HasSpi + HasI2c> FanDiagMixin for T {}

struct Board {
    spi_bus: (),
    i2c_bus: (),
}

impl HasSpi for Board {
    type Spi = ();
    fn spi(&self) -> &() {
        &self.spi_bus
    }
}

impl HasI2c for Board {
    type I2c = ();
    fn i2c(&self) -> &() {
        &self.i2c_bus
    }
}

fn main() {
    let board = Board { spi_bus: (), i2c_bus: () };
    // `read_fan_speed` exists on `Board` only because it implements both
    // `HasSpi` and `HasI2c` — remove either impl and this call stops
    // compiling, which a two-parameter free function could not enforce.
    assert_eq!(board.read_fan_speed(), 1200);
}
```

## Missing Ingredient Cases To Test

- a type implementing only one of the two ingredient traits does not have
  the mixin method available, confirmed by the absence compiling to an
  error rather than a runtime check;
- a type implementing both ingredient traits gets the mixin method without
  writing any implementation of the mixin itself;
- the mixin's default method reaches both resources exclusively through
  `self`, so it cannot be tricked into mixing resources from two different
  receivers the way a two-parameter free function could;
- removing one ingredient `impl` from a type that previously had the mixin
  breaks every call site that used it, at compile time.

## See Also

- [trait-blanket-impl](trait-blanket-impl.md) - the mechanism this rule specializes: a blanket impl over a bound, here a conjunction of ingredient traits
- [trait-associated-type-vs-generic](trait-associated-type-vs-generic.md) - why each ingredient is an associated type, not a generic parameter threaded through every signature
- [type-capability-token](type-capability-token.md) - a sibling pattern for authority as an unforgeable value, rather than a resource conjunction on one receiver
- [api-extension-trait](api-extension-trait.md) - adding methods to a type from outside; this rule additionally requires a bound before the methods exist at all
- [trait-default-methods](trait-default-methods.md) - the default-method shape the mixin's cross-cutting behavior takes
