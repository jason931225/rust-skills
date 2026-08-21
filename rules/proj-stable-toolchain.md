# proj-stable-toolchain

> Build and run production applications on a pinned stable toolchain and test upgrades continuously

## Why It Matters

`stable`, `beta`, and `nightly` are release channels, while a target triple
selects the platform. Floating implicitly with the developer machine makes CI
and production disagree; shipping nightly ties the product to unfinished
features. Pin the current stable release used for admission and upgrade it
deliberately.

## Toolchain Pinning Requirements

- Install and select toolchains with `rustup` or an equivalent hermetic
  mechanism.
- Commit `rust-toolchain.toml` with an exact stable version and required
  components/targets.
- Build, test, and run the shipped application on stable.
- Use nightly only for explicitly isolated tools such as Miri or fuzzing; do
  not make the production binary depend on nightly features.
- Test the next stable/beta channel as advisory early warning while the pinned
  stable toolchain remains admission authority.
- Honor Rust target-tier guarantees and test every advertised deployment
  target.
- Upgrade regularly with the dependency graph and review compiler/lint changes
  rather than postponing them indefinitely.

## Bad

```toml
[toolchain]
channel = "stable"
```

A floating channel can select different compilers across developer machines,
CI, and later rebuilds.

## Good

```toml
[toolchain]
channel = "1.97.1"
profile = "minimal"
components = ["clippy", "rustfmt"]
targets = ["x86_64-unknown-linux-gnu"]
```

## The Per-Target Half Of A Pinned Build

`rust-toolchain.toml` pins *which* compiler and which targets. It says nothing
about *how* each target links and runs — the linker, the per-target rustflags,
the environment, the runner used to execute a cross-built test binary. Those
belong in a checked-in `.cargo/config.toml`, for the same reason the toolchain
file exists: a setting that lives in someone's shell is not part of the build.

```toml
# .cargo/config.toml — committed, so every machine links the same way.
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
rustflags = ["-C", "target-cpu=neoverse-n1"]

[target.thumbv7em-none-eabihf]
runner = "probe-rs run --chip STM32F303RETx"
```

Two merge rules decide whether that file is actually in effect, and both fail
silently.

**A matching `[target.*].rustflags` replaces `[build].rustflags` outright.**
They do not concatenate — the `[build]` list is dropped. With both present:

```text
from_target: true
from_build:  false
```

**`RUSTFLAGS` in the environment replaces every rustflags list from config.**
An ad-hoc `RUSTFLAGS=...` on a command line therefore discards the committed
per-target settings rather than adding to them:

```text
$ RUSTFLAGS="--cfg from_env" cargo run
from_target: false
from_build:  false
```

That is the practical hazard: a one-off `RUSTFLAGS=` to try a lint or a
codegen flag quietly removes the cross-linker configuration the build depends
on, and the failure surfaces as a link error with no mention of the flag that
caused it. Put per-target settings in `[target.*]` and keep `[build].rustflags` for what
genuinely applies everywhere. For a one-off flag, reach for `--config` on the
**same key** the committed file uses — the precedence rule above applies to CLI
overrides too, so `--config 'build.rustflags=[...]'` is discarded whenever a
matching `[target.*]` entry exists, which is precisely the configuration this
section is about:

```text
--config 'build.rustflags=[...]'            from_cli: false   (dropped)
--config 'target.<triple>.rustflags=[...]'  from_cli: true    (joined)
```

So override `target.<triple>.rustflags`, which joins with the committed list,
and treat `RUSTFLAGS=` as unavailable for this purpose rather than as the quick
option.

Cargo also walks parent directories for `.cargo/config.toml`, so a workspace
one level up can be supplying settings a crate never declares. That is useful
for a monorepo and surprising when debugging one crate in isolation.

## See Also

- [proj-msrv-declare](proj-msrv-declare.md) - library compatibility policy is separate from the app toolchain
- [proj-latest-edition](proj-latest-edition.md) - edition is not a release channel
- [proj-works-out-of-box](proj-works-out-of-box.md) - keep target builds free of hidden host tools
- [lint-rustfmt-check](lint-rustfmt-check.md) - pin formatter behavior with the toolchain
- [unsafe-miri-ci](unsafe-miri-ci.md) - isolate nightly-only verification
