# trait-config-family

> Collapse three or more collaborator generics into one config trait whose associated types name the whole family, and select the family with one impl per deployment

## Why It Matters

A type that needs three collaborators usually grows three type parameters, and
every function that touches it repeats all three. The cost is not verbosity —
it is that adding a fourth collaborator is a breaking edit to every downstream
signature, whether or not that code uses the new one. Naming the family once,
as associated types on a single config trait, leaves the struct with one
parameter: adding a collaborator touches the trait, the struct, and each
deployment's impl, and nothing else recompiles differently.

## Bad

```rust
pub trait Spi { fn transfer(&mut self, byte: u8) -> u8; }
pub trait Uart { fn write(&mut self, byte: u8); }
pub trait Clock { fn now_ms(&self) -> u64; }

pub struct Diag<S: Spi, U: Uart, C: Clock> {
    pub spi: S,
    pub uart: U,
    pub clock: C,
}

// Every downstream signature repeats the full list, including the parameters
// it never mentions in the body. A fourth peripheral rewrites all of them.
pub fn heartbeat<S: Spi, U: Uart, C: Clock>(diag: &mut Diag<S, U, C>) -> u64 {
    diag.uart.write(b'.');
    diag.clock.now_ms()
}
```

## Good

```rust
pub trait Spi { fn transfer(&mut self, byte: u8) -> u8; }
pub trait Uart { fn write(&mut self, byte: u8); }
pub trait Clock { fn now_ms(&self) -> u64; }

/// One trait names the whole family a deployment selects. The bounds on the
/// associated types travel with them, so callers need no extra `where` clause.
pub trait Board {
    type Spi: Spi;
    type Uart: Uart;
    type Clock: Clock;
}

pub struct Diag<B: Board> {
    pub spi: B::Spi,
    pub uart: B::Uart,
    pub clock: B::Clock,
}

// One parameter, and it stays one when the family grows.
pub fn heartbeat<B: Board>(diag: &mut Diag<B>) -> u64 {
    diag.uart.write(b'.');
    diag.clock.now_ms()
}

pub struct Prod;
pub struct RealSpi;
pub struct RealUart;
pub struct RealClock;
impl Spi for RealSpi { fn transfer(&mut self, byte: u8) -> u8 { byte } }
impl Uart for RealUart { fn write(&mut self, _byte: u8) {} }
impl Clock for RealClock { fn now_ms(&self) -> u64 { 1_000 } }
impl Board for Prod {
    type Spi = RealSpi;
    type Uart = RealUart;
    type Clock = RealClock;
}

pub struct Sim;
pub struct FakeSpi;
pub struct FakeUart;
pub struct FakeClock;
impl Spi for FakeSpi { fn transfer(&mut self, _byte: u8) -> u8 { 0xAA } }
impl Uart for FakeUart { fn write(&mut self, _byte: u8) {} }
impl Clock for FakeClock { fn now_ms(&self) -> u64 { 0 } }
impl Board for Sim {
    type Spi = FakeSpi;
    type Uart = FakeUart;
    type Clock = FakeClock;
}

fn main() {
    let mut prod = Diag::<Prod> { spi: RealSpi, uart: RealUart, clock: RealClock };
    assert_eq!(heartbeat(&mut prod), 1_000);

    // A test swaps the entire layer by naming one type. `heartbeat` is the
    // same function, not an overload and not a generic re-instantiation the
    // caller had to spell out.
    let mut sim = Diag::<Sim> { spi: FakeSpi, uart: FakeUart, clock: FakeClock };
    assert_eq!(heartbeat(&mut sim), 0);
    assert_eq!(sim.spi.transfer(1), 0xAA);
}
```

## Adding A Collaborator Without Downstream Churn

The difference shows up on the next change, not the first write. Add a `Gpio`
peripheral to the multi-parameter form and leave one downstream signature at
three parameters, and the build stops:

```text
error[E0107]: struct takes 4 generic arguments but 3 generic arguments were supplied
   |
12 | pub fn heartbeat<S: Spi, U: Uart, C: Clock>(diag: &mut Diag<S, U, C>) -> u64 {
   |                                                        ^^^^ expected 4 generic arguments
```

So the churn is mandatory rather than stylistic: every signature naming the
struct must be edited in lockstep with it, and the compiler will not let you
stage the change. In the config-trait form the same addition is three edits —
one associated type, one field, one line per deployment impl — and every
`fn f<B: Board>(..)` compiles untouched, because none of them ever named the
peripherals individually.

## When A Config Trait Is Overkill

- **One or two collaborators.** Two parameters are readable and the churn
  argument has not bitten yet. Introducing the indirection early buys nothing
  and costs a level of naming.
- **The implementation is chosen at runtime.** A config trait picks the family
  at compile time. If the choice depends on a config file or a probe, you want
  `dyn` or an enum, not a type parameter.
- **Callers supply open-ended plugins.** A trait with a fixed set of associated
  types names a closed family; an open registry is a different shape.
- **A state axis is not a collaborator axis.** Typestate parameters stay their
  own parameter — collapse the collaborators and keep the state, giving
  `Handle<Cfg, S>` rather than folding `S` into the config trait. The state
  parameter exists precisely so that transitions change the type, which an
  associated type fixed per deployment cannot do.

## See Also

- [trait-capability-mixin](trait-capability-mixin.md) - proves one receiver owns every resource a method needs; composes with this rather than replacing it
- [trait-associated-type-vs-generic](trait-associated-type-vs-generic.md) - why each collaborator is an associated type rather than a parameter
- [api-typestate](api-typestate.md) - the state axis this rule deliberately leaves alone
- [trait-dyn-vs-generic](trait-dyn-vs-generic.md) - when to erase the type instead of naming it
- [anti-over-abstraction](anti-over-abstraction.md) - the one-or-two-collaborator floor below which this is over-engineering
