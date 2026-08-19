# proj-dependency-policy

> Admit dependencies deliberately, commit the lockfile for anything you ship, and audit the tree continuously

## Why It Matters

Every dependency is code you ship, with the privileges of your process. The
build machine, the CI runner, and the deployed binary all inherit whatever the
transitive tree contains, and a tree of hundreds of crates is not something a
reviewer reads. The failure modes are ordinary rather than exotic: an
unmaintained crate with a known advisory, two major versions of the same
library linked into one binary, a license that legal will not accept, or a
registry that is unavailable the day you need to rebuild.

## Contract

- Weigh each new dependency against the cost of the code you would otherwise
  write: what it pulls in transitively, whether it is maintained, and whether
  it is already in the tree in another version.
- Commit `Cargo.lock` for binaries and any artifact you deploy, so the build
  that ships is the build that was tested. Libraries publish requirements, not
  a lockfile.
- Run an advisory audit and a policy check (bans, licenses, sources,
  duplicate majors) in CI, on a schedule as well as on pull requests — new
  advisories appear without your code changing.
- Give advisory exceptions an owner and an expiry, and never silence the whole
  check for one finding.
- Prefer default-features-off with explicit features, so a dependency's
  optional surface is a decision rather than an inheritance.
- Where offline or reviewable builds matter, vendor dependencies so updates
  arrive as reviewable diffs, accepting the repository size that costs; a
  private mirror is the alternative for larger teams.
- Watch build-time dependencies too: build scripts and proc macros execute on
  every developer machine and CI runner.

## Bad

```toml
[dependencies]
# a framework pulled in for one helper function, with every optional feature on
mega-framework = { version = "3", features = ["full"] }
```

## Good

```toml
[dependencies]
# Narrow surface, explicit features, no default set inherited by accident.
serde = { version = "1", default-features = false, features = ["derive"] }
```

```bash
cargo audit --deny warnings          # RustSec advisories
cargo deny check bans licenses sources
cargo tree --duplicates              # two major versions of one crate
cargo vendor                         # optional: reviewable, offline-buildable tree
```

Run these against the committed lockfile in CI, and keep the schedule separate
from the pull-request gate so a new advisory reaches you without a code change.

## See Also

- [lint-static-verification](lint-static-verification.md) - where the audit jobs sit in the CI gate
- [proj-semver-contract](proj-semver-contract.md) - which versions you require and why
- [proj-reproducible-runtime](proj-reproducible-runtime.md) - the lockfile is part of the shipped artifact
- [proj-feature-additive](proj-feature-additive.md) - feature choices propagate through the tree
- [proj-build-rs-minimal](proj-build-rs-minimal.md) - build scripts run with your privileges
