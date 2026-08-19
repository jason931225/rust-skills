# proj-dependency-policy

> Admit dependencies deliberately, commit the lockfile for anything you ship, and audit the tree continuously

## Why It Matters

Every dependency is shipped code running with the process's privileges. The
build machine, the CI runner, and the deployed binary all inherit whatever the
transitive tree contains, and a tree of hundreds of crates is not something a
reviewer reads. The failure modes are ordinary rather than exotic: an
unmaintained crate with a known advisory, two major versions of the same
library linked into one binary, a license that legal will not accept, or a
registry that is unavailable on the day a rebuild is needed.

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
from the pull-request gate so a new advisory surfaces without a code change.

## Key Points

- Weigh each new dependency against the cost of writing that code directly:
  what it pulls in transitively, whether it is maintained, and whether it is
  already in the tree under another version.
- Commit `Cargo.lock` for binaries and any deployed artifact, so the build that
  ships is the build that was tested. Libraries publish requirements, not a
  lockfile.
- Run an advisory audit and a policy check (bans, licenses, sources,
  duplicate majors) in CI, on a schedule as well as on pull requests — new
  advisories appear without any local code changing.
- Give advisory exceptions an owner and an expiry, and never silence the whole
  check for one finding.
- Prefer default-features-off with explicit features, so a dependency's
  optional surface is a decision rather than an inheritance.
- Where offline or reviewable builds matter, vendor dependencies so updates
  arrive as reviewable diffs, accepting the repository size that costs; a
  private mirror is the alternative for larger teams.
- Watch build-time dependencies too: build scripts and proc macros execute on
  every developer machine and CI runner.

## See Also

- [lint-static-verification](lint-static-verification.md) - where the audit jobs sit in the CI gate
- [proj-semver-contract](proj-semver-contract.md) - which versions to require and why
- [proj-reproducible-runtime](proj-reproducible-runtime.md) - the lockfile is part of the shipped artifact
- [proj-build-rs-minimal](proj-build-rs-minimal.md) - build scripts run with the same privileges
