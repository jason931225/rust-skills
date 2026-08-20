# api-path-containment

> Resolve caller-supplied path components against a fixed root and reject anything that escapes it

## Why It Matters

A request parameter that reaches the filesystem is a path expression, not a
name. `/assets/` joined with `../../etc/passwd` reads credentials; an upload
named `../root/.ssh/authorized_keys` overwrites them. Rust's memory safety
does not help here — `Path::join` cheerfully accepts `..`, and an absolute
component silently replaces the whole prefix. Containment has to be decided
before the open, on the resolved path, not by scanning the raw string.

## Root Containment Requirements

- Treat the storage root as the only trusted path. Callers supply a key, never
  a path.
- Reject a component that is empty, `.`, `..`, absolute, a root or prefix
  component, or contains a path separator or NUL.
- Resolve the candidate and confirm the result is still under the canonical
  root before any open, create, or delete.
- Canonicalize the root once at startup so symlinked or relative roots cannot
  shift underneath later checks.
- Remember that a symlink inside the root can point outside it: canonicalize
  the target too, or open with a platform API that refuses to traverse links.
- Do not echo the resolved path, the root, or the underlying I/O error to the
  caller — that turns a rejected traversal into filesystem reconnaissance.

## Bad

```rust
fn read_asset(root: &Path, asset_id: &str) -> std::io::Result<Vec<u8>> {
    // "../../etc/passwd" escapes the root; an absolute id discards it entirely
    std::fs::read(root.join(asset_id))
}
```

## Good

```rust
use std::path::{Component, Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum PathError {
    Malformed,
    Escapes,
}

/// Joins one caller-supplied key onto `root`, rejecting anything that is not a
/// plain single-segment name.
pub fn resolve_in_root(root: &Path, key: &str) -> Result<PathBuf, PathError> {
    if key.is_empty() || key.contains('\0') {
        return Err(PathError::Malformed);
    }
    let mut components = Path::new(key).components();
    let (Some(Component::Normal(name)), None) = (components.next(), components.next()) else {
        return Err(PathError::Malformed);
    };
    let candidate = root.join(name);
    // Defence in depth: the join above cannot escape, but a later refactor
    // that admits multi-segment keys must still fail closed here.
    if !candidate.starts_with(root) {
        return Err(PathError::Escapes);
    }
    Ok(candidate)
}

fn main() {
    let root = Path::new("/srv/assets");
    assert_eq!(resolve_in_root(root, "logo.png"), Ok(root.join("logo.png")));
    assert_eq!(resolve_in_root(root, "../etc/passwd"), Err(PathError::Malformed));
    assert_eq!(resolve_in_root(root, "/etc/passwd"), Err(PathError::Malformed));
    assert_eq!(resolve_in_root(root, "a/b"), Err(PathError::Malformed));
}
```

Where keys legitimately carry a nested shape, keep the same discipline: accept
only `Component::Normal` segments, then canonicalize the result and re-check
containment against the canonical root before opening.

## Traversal Cases To Test

- `..`, `../..`, and a percent-decoded `..` are rejected on read and on write;
- an absolute path and a Windows prefix (`C:\`, `\\?\`) are rejected;
- an embedded NUL is rejected;
- a symlink inside the root pointing outside it does not yield its target;
- the rejection response is identical for "escaped" and "does not exist".

## See Also

- [api-parse-dont-validate](api-parse-dont-validate.md) - turn the key into a contained type once
- [api-extract-or-reject](api-extract-or-reject.md) - reject before any filesystem effect
- [err-edge-mapping](err-edge-mapping.md) - do not leak paths or I/O errors to callers
- [type-newtype-validated](type-newtype-validated.md) - keep the validated key distinct from a raw string
- [async-tokio-fs](async-tokio-fs.md) - bound and isolate the file work itself
