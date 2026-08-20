# api-dir-enumeration

> Treat a directory walk as a stream of fallible entries, and never depend on the order it yields

## Why It Matters

Enumeration fails per entry, not per traversal: one unreadable subdirectory,
one dangling symlink, one file deleted between listing and stat. Code that
propagates the first error stops scanning the other ten thousand entries a user
asked about, and code that unwraps crashes on a permission bit. The order is
not a contract either — the filesystem returns entries in whatever order suits
it, so output that looks sorted on one machine will not be on another, and a
test that asserts it passes locally and fails in CI.

## Directory Walk Requirements

- Handle each entry's `Result` individually: report it, count it, and continue.
- Decide and document what a failed entry means — skipped with a warning, or
  fatal for the operation — and make the final exit status reflect it.
- Sort explicitly whenever the order is observable by a user, a test, or a
  downstream diff.
- Decide whether symlinks are followed, and guard against cycles if they are.
- Bound the walk: depth, entry count, or time. A traversal of an unknown tree
  is unbounded work.
- Do not build paths by string concatenation while walking; keep them as paths
  so unusual names survive.

## Bad

```rust
fn list(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root)? {
        // One unreadable entry aborts the whole listing
        found.push(entry?.path());
    }
    Ok(found)
}
```

## Good

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct Listing {
    pub entries: Vec<PathBuf>,
    /// Entries that could not be read, kept rather than swallowed.
    pub skipped: Vec<(PathBuf, io::ErrorKind)>,
}

pub fn list(root: &Path) -> io::Result<Listing> {
    // Opening the directory at all is a whole-operation failure.
    let read = fs::read_dir(root)?;
    let mut entries = Vec::new();
    let mut skipped = Vec::new();

    for item in read {
        match item {
            Ok(entry) => entries.push(entry.path()),
            // A single bad entry is reported, not fatal.
            Err(error) => skipped.push((root.to_path_buf(), error.kind())),
        }
    }

    // The filesystem's order is arbitrary; sort where it is observable.
    entries.sort();
    Ok(Listing { entries, skipped })
}

fn main() {
    let listing = list(Path::new(".")).expect("current directory is readable");
    assert!(listing.entries.windows(2).all(|pair| pair[0] <= pair[1]));

    // A missing root is an operation failure, distinct from a bad entry.
    assert!(list(Path::new("/no/such/directory")).is_err());
}
```

## Traversal Cases To Pin

- an unreadable subdirectory is reported and the remaining entries still appear;
- the exit status or return value reflects that something was skipped;
- output is sorted deterministically across platforms;
- a symlink loop terminates rather than recursing forever;
- a filename that is not valid Unicode survives the walk and is displayed
  without being used to reopen the file.

## See Also

- [api-path-containment](api-path-containment.md) - entries discovered by a walk are still caller-influenced paths
- [type-path-not-string](type-path-not-string.md) - keep the names as paths through the traversal
- [proj-cli-contract](proj-cli-contract.md) - per-entry failures and the final exit status
- [async-tokio-fs](async-tokio-fs.md) - bounding filesystem work off the executor
- [err-result-over-panic](err-result-over-panic.md) - a permission bit is not a bug
