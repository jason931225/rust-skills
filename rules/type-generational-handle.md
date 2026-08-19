# type-generational-handle

> Pair a reused slot index with a generation counter, and reject a handle whose generation has moved on

## Why It Matters

A pool that reuses slots — an arena of entities, a connection table, a slab —
hands out indices that stay syntactically valid after the thing they referred
to is gone. Nothing stops a caller holding index 7 across a removal, and when
slot 7 is refilled that stale handle silently addresses a different object.
This is the use-after-free bug in safe code: no memory is corrupted, but a
request is applied to the wrong entity, which is worse than a crash because it
looks like it worked.

## Bad

```rust
pub struct Pool<T> {
    slots: Vec<Option<T>>,
}

impl<T> Pool<T> {
    // A bare index outlives the value it names
    pub fn get(&self, index: usize) -> Option<&T> {
        self.slots.get(index)?.as_ref()
    }
}
```

## Good

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Handle {
    index: usize,
    generation: u32,
}

struct Slot<T> {
    value: Option<T>,
    /// Incremented on every removal, so a handle from before it cannot match.
    generation: u32,
}

pub struct Pool<T> {
    slots: Vec<Slot<T>>,
}

impl<T> Pool<T> {
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    pub fn insert(&mut self, value: T) -> Handle {
        if let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.value.is_none())
        {
            slot.value = Some(value);
            return Handle { index, generation: slot.generation };
        }
        self.slots.push(Slot { value: Some(value), generation: 0 });
        Handle { index: self.slots.len() - 1, generation: 0 }
    }

    pub fn remove(&mut self, handle: Handle) -> Option<T> {
        let slot = self.slots.get_mut(handle.index)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.generation = slot.generation.wrapping_add(1);
        slot.value.take()
    }

    /// A stale handle reads as absent rather than as someone else's value.
    pub fn get(&self, handle: Handle) -> Option<&T> {
        let slot = self.slots.get(handle.index)?;
        (slot.generation == handle.generation).then(|| slot.value.as_ref())?
    }
}

fn main() {
    let mut pool = Pool::new();
    let first = pool.insert("session-a");
    assert_eq!(pool.get(first), Some(&"session-a"));

    assert_eq!(pool.remove(first), Some("session-a"));
    let second = pool.insert("session-b");

    // The slot is reused, but the old handle does not address the new value.
    assert_eq!(second.index, first.index);
    assert_eq!(pool.get(first), None, "the stale handle must not resolve");
    assert_eq!(pool.get(second), Some(&"session-b"));
}
```

## Key Points

- Increment the generation on removal, not on insertion, so every handle issued
  before the removal is invalidated at once.
- Return `None` for a stale handle rather than panicking: holding one is a
  caller lifecycle bug, not memory unsafety, and the caller can handle it.
- Size the counter for the reuse rate. A `u32` wrapping after four billion
  removals of one slot is usually fine; a `u16` in a hot pool is not.
- Keep the fields private so a handle cannot be forged from a bare index.
- The `slotmap` and `slab` crates implement this; the rule is about not
  hand-rolling the bare-index version.

## See Also

- [type-newtype-ids](type-newtype-ids.md) - a handle is an identifier, not a number
- [api-newtype-safety](api-newtype-safety.md) - keeping handle types from mixing
- [mem-arena-allocator](mem-arena-allocator.md) - pools and arenas share the reuse hazard
- [err-result-over-panic](err-result-over-panic.md) - a stale handle is a recoverable error
