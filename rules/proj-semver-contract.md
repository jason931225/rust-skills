# proj-semver-contract

> Version by what breaks callers, depend on the earliest version you actually need, and keep a written changelog

## Why It Matters

Cargo resolves dependencies from version requirements, so a version number is
not a label — it is the input to every downstream build. Requiring the newest
release of a dependency a crate does not actually need makes that crate
unusable beside one that has pinned an older version; the build fails for a
user who did nothing wrong. In the other direction, an unannounced breaking
change in a minor release breaks compilation for everyone who allowed it.
Both mistakes stay invisible in the crate's own repository, where only one
version graph is ever resolved.

## Bad

```toml
[dependencies]
# "whatever was newest on my laptop" - now no downstream crate may pin < 1.7,
# even though this crate only uses APIs that existed in 1.2
hugs = "1.7.3"
```

## Good

```toml
[package]
name = "widget"
version = "2.4.0"          # additive release: new API, nothing removed
rust-version = "1.85"      # raising this is a minor bump, not a patch

[dependencies]
# The earliest release that has `Hug::gentle`, which this crate calls.
hugs = "1.2"
```

```bash
# Prove the lower bounds are real rather than aspirational. `-Z minimal-versions`
# applies to the whole graph, so unmaintained lower bounds in transitive crates
# can fail the build; `-Z direct-minimal-versions` constrains only this crate's
# own requirements, which is usually what a lower-bound check should assert.
cargo +nightly generate-lockfile -Z direct-minimal-versions
cargo check --workspace --all-features
```

Between releases keep the manifest at the next version with a prerelease
suffix — publish 2.4.0, then set `2.4.1-alpha.1`; when a breaking change lands
before the next release, the manifest becomes `3.0.0-alpha.1` immediately, so
git dependents get a resolver error instead of a mysterious compile failure.

## Key Points

- Follow Cargo's semantic versioning: breaking changes take a major bump,
  additions a minor bump, fixes a patch bump. Treat "breaking" as what breaks a
  caller's compile — a new public field, a removed trait impl, a tightened
  bound, a renamed variant.
- Require the *earliest* dependency version that provides everything the crate
  calls, not the newest release available. Verify that bound by resolving with
  minimal versions in CI rather than guessing it.
- Raising the MSRV is at least a minor bump, so a caller pinned to the old
  compiler can express `>=1, <1.7` and still receive patch-level security
  fixes.
- Keep a hand-written changelog (the Keep a Changelog format works well). A
  dump of git log does not tell a reader how to migrate.
- Between releases, carry a prerelease suffix and bump the part of the version
  the pending change requires, so git and path dependents see the break.
- Use `#[non_exhaustive]`, sealed traits, and private fields so that additions
  stay additive; run an API-diff tool in CI rather than deciding by eye.

## See Also

- [proj-msrv-declare](proj-msrv-declare.md) - declaring and testing the compiler floor
- [api-non-exhaustive](api-non-exhaustive.md) - keep struct and enum additions non-breaking
- [proj-feature-additive](proj-feature-additive.md) - features must not change existing behaviour
- [proj-dependency-policy](proj-dependency-policy.md) - which dependencies to take on in the first place
