# type-path-not-string

> Carry filesystem paths as `Path`/`PathBuf`; convert to text only for display

## Why It Matters

A path is a sequence of OS bytes, not UTF-8. On Unix any byte except NUL and
`/` is legal, and Windows paths are UTF-16 that may not round-trip. Code that
calls `to_str().unwrap()` panics on a file someone unpacked from an archive
with a Latin-1 name; code that uses `to_string_lossy()` for anything but
display substitutes U+FFFD and then operates on a path that no longer exists.
Keeping paths in their own type makes the lossy step explicit and confines it
to the one place it is safe: output for a human.

## Bad

```rust
fn backup(path: &str) -> io::Result<()> {
    // Every caller must already have lost non-UTF-8 names to get here
    let target = format!("{}.bak", path);
    fs::copy(path, target)
}
```

## Good

```rust
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Takes anything path-like and never round-trips through `str`.
fn backup_target(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    // Extend the OS string directly: no UTF-8 assumption anywhere.
    name.push(".bak");
    path.with_file_name(name)
}

fn main() {
    let target = backup_target(Path::new("/var/data/report.csv"));
    assert_eq!(target, Path::new("/var/data/report.csv.bak"));

    // Comparisons and joins stay in path space.
    let root = Path::new("/var/data");
    assert!(target.starts_with(root));

    // Text conversion happens once, at the edge, for a person to read.
    println!("wrote {}", target.display());
}
```

## Key Points

- Accept `impl AsRef<Path>` in public functions so callers can pass `&str`,
  `String`, `PathBuf`, or `&Path` without converting.
- Use `Path::display()` for output and `to_string_lossy()` only when a `String`
  is unavoidable — never to reconstruct a path you will open.
- Build names with `OsString::push`, `with_extension`, and `with_file_name`
  rather than `format!` over stringified paths.
- Reserve `to_str()` for cases where the failure is meaningful, and handle the
  `None` rather than unwrapping it.
- Command-line arguments have the same problem: read them as `OsString` when
  they name files.

## See Also

- [api-impl-asref](api-impl-asref.md) - accept borrowed path-like input
- [api-path-containment](api-path-containment.md) - the security contract for caller-supplied paths
- [type-unicode-length](type-unicode-length.md) - text boundaries need the same deliberate choice
- [proj-cli-contract](proj-cli-contract.md) - file arguments arrive as OS strings
