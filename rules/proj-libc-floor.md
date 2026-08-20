# proj-libc-floor

> Choose the C library and dynamic-link floor the fleet must satisfy, and verify the shipped binary against it

## Why It Matters

A Rust binary built against glibc records the symbol versions of the build
machine, so a binary compiled on a newer distribution fails to start on an
older one with a message about `GLIBC_2.34 not found` — after deployment, on
the hosts that matter, with no compile-time warning. The choice is between a
statically linked musl target that runs anywhere and a pinned glibc floor that
keeps dynamic linking and NSS behaviour. Either is fine; not choosing means the
floor is whatever the last build machine had.

## Bad

```dockerfile
# Builder is whatever the base image ships today; the floor moves whenever the
# image is rebuilt, and the failure appears on the oldest host in the fleet
FROM rust:latest AS build
RUN cargo build --release
FROM debian:oldstable
COPY --from=build /app/target/release/service /usr/bin/service
```

## Good

```bash
# Static musl: no libc floor at all, at the cost of the system resolver and
# dlopen. The check is that the artifact has no interpreter and no NEEDED.
cargo build --release --target x86_64-unknown-linux-musl
file target/x86_64-unknown-linux-musl/release/service   # "statically linked"

# Or a pinned glibc floor: build on the oldest supported base, and verify the
# highest version the binary demands stays at or below it.
objdump -T target/release/service \
  | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1     # must be <= the fleet floor
```

```rust
/// The verification is a comparison, so it belongs in CI rather than in a
/// reviewer's head.
pub fn within_floor(required: &str, floor: &str) -> bool {
    fn parts(version: &str) -> Vec<u32> {
        version
            .trim_start_matches("GLIBC_")
            .split('.')
            .filter_map(|piece| piece.parse().ok())
            .collect()
    }
    parts(required) <= parts(floor)
}

fn main() {
    assert!(within_floor("GLIBC_2.28", "GLIBC_2.31"));
    assert!(within_floor("GLIBC_2.31", "GLIBC_2.31"));
    // The case that breaks a deployment: the build machine was newer.
    assert!(!within_floor("GLIBC_2.34", "GLIBC_2.31"));
}
```

## Choosing And Holding The Floor

- Decide the floor from the oldest host the fleet actually runs, and record it
  where the build can read it.
- Static musl removes the question, but changes DNS resolution, `dlopen`, and
  some `getaddrinfo` behaviour — test those paths rather than assuming parity.
- Build in the oldest supported environment; a newer toolchain cannot target an
  older glibc after the fact.
- Verify the artifact you ship, not a rebuild of it, and fail the pipeline on a
  symbol above the floor.
- `cross`, `cargo-zigbuild`, and a pinned builder image are the usual ways to
  hold the floor steady; whichever is chosen, it belongs in the manifest of the
  build, not in a person's memory.

## The Same Split On Windows

Windows has the same class of decision under different names.
`x86_64-pc-windows-msvc` links the UCRT, emits PDB debug info, and follows the
MSVC C++ ABI; `x86_64-pc-windows-gnu` links MinGW's `msvcrt`, emits DWARF, and
follows the GNU ABI. They are different C runtimes: objects built for one do
not link against the other, and C++ interop does not cross between them.

Pick the triple from what the deployment and any native dependencies require,
state it the way this rule states a glibc floor, and verify the shipped binary
against it — a locally-working `windows-gnu` build is not evidence for a fleet
that expects `windows-msvc`.

## See Also

- [proj-reproducible-runtime](proj-reproducible-runtime.md) - building the artifact that gets promoted
- [proj-build-target-cfg](proj-build-target-cfg.md) - build scripts must read the target, not the host
- [opt-target-cpu](opt-target-cpu.md) - the CPU baseline decision, with the same fleet-wide reasoning
- [proj-continuous-delivery](proj-continuous-delivery.md) - the pipeline that must fail on a floor violation
