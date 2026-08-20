# own-split-borrow-fields

> Group a wide struct's fields into named sub-structs so independent operations borrow disjoint state

## Why It Matters

A method taking `&mut self` borrows the entire struct, so two logically
independent operations on unrelated fields cannot run in the same scope. On a
wide aggregate — a game state, a connection registry, a device controller —
this surfaces as a borrow error with no obvious fix, and the usual workarounds
make it worse: cloning to escape the borrow, or wrapping fields in `RefCell`
and moving a compile error to runtime. Splitting the fields by concern makes
the disjointness visible to the compiler.

## Bad

```rust
struct Engine {
    audio_volume: u8,
    audio_muted: bool,
    physics_gravity: f32,
    physics_steps: u32,
    // ...twenty more fields
}

impl Engine {
    fn tick_audio(&mut self) { /* touches two fields */ }
    fn tick_physics(&mut self) { /* touches two others */ }
}

// Both borrow all of Engine, so this does not compile:
// let a = engine.tick_audio(); let p = engine.tick_physics();
```

## Good

```rust
#[derive(Debug, Default)]
pub struct Audio {
    pub volume: u8,
    pub muted: bool,
}

#[derive(Debug, Default)]
pub struct Physics {
    pub gravity: f32,
    pub steps: u32,
}

#[derive(Debug, Default)]
pub struct Engine {
    pub audio: Audio,
    pub physics: Physics,
}

/// Each function takes only the state it mutates, so the borrows are disjoint
/// and the signature documents what the function touches.
fn tick_audio(audio: &mut Audio) {
    audio.volume = audio.volume.saturating_add(1);
}

fn tick_physics(physics: &mut Physics) {
    physics.steps += 1;
}

fn main() {
    let mut engine = Engine::default();

    // Two mutable borrows of one value, live at once, because they name
    // different fields.
    let audio = &mut engine.audio;
    let physics = &mut engine.physics;
    tick_audio(audio);
    tick_physics(physics);

    assert_eq!(engine.audio.volume, 1);
    assert_eq!(engine.physics.steps, 1);
}
```

## Grouping And Borrow Boundaries

- Split by concern, not by type. The grouping should name a responsibility that
  a reader recognises.
- Free functions or methods on the sub-struct both work; what matters is that
  the parameter is the narrow type.
- The compiler splits borrows across *fields* of one struct, but not across a
  method boundary that takes `&mut self` — which is why the signature has to
  change, not just the call site.
- Reach for `RefCell` only when the sharing is genuinely dynamic; using it to
  escape a borrow error converts a compile error into a runtime panic.
- The same grouping usually improves constructors, which is a separate rule.

## See Also

- [api-init-cascaded](api-init-cascaded.md) - the same grouping applied to constructor parameters
- [own-refcell-interior](own-refcell-interior.md) - what not to reach for instead
- [closure-disjoint-capture](closure-disjoint-capture.md) - the closure-level version of disjointness
- [anti-clone-excessive](anti-clone-excessive.md) - cloning to escape a borrow is the other workaround
