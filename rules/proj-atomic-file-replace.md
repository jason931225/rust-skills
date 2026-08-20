# proj-atomic-file-replace

> Replace a whole file through a same-directory temporary, a sync, and a rename — never by truncating the original

## Why It Matters

`File::create` truncates before the first byte is written, so from that call
until the write returns, the file on disk is neither the old contents nor the
new ones — and a crash, a full disk, or a serialization error in the middle
leaves it that way permanently. Closing the handle does not help: a successful
`write_all` means the bytes reached the kernel, not the storage device, so a
power loss seconds later can still lose them. The fix is structural rather than
careful: write somewhere else, force the bytes out, then swap the name, so the
destination only ever holds one complete version. Every configuration file,
state snapshot, and index rewrite that a process may be killed during needs
this shape.

## Atomic Replace Requirements

- Write to a temporary file in the *same directory* as the destination; a
  rename across filesystems is a copy, and copies are not atomic.
- Give the temporary a name no concurrent writer will pick, and remove it on
  every error path — the destination must be untouched when the write fails.
- Call `sync_all` on the temporary before renaming. Without it the rename can
  reach disk before the contents, leaving a complete name over empty data.
- Rename over the destination. Do not remove the destination first: that opens
  a window in which the file does not exist.
- Sync the containing directory after the rename, or the new directory entry
  itself is not durable.
- On Windows, `fs::rename` does not replace an existing file and there is no
  directory sync; use the platform's replace call and its own ordering rules.
- Decide explicitly whether the new file inherits the old one's permissions and
  owner — a fresh temporary gets neither.

## Bad

```rust
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

fn save(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // Truncates immediately. If write_all fails halfway, or the process is
    // killed here, the previous contents are gone and the new ones are partial.
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    // Returning drops the handle, which closes it. Nothing has been forced to
    // the storage device, so a power loss now can still lose the whole write.
    Ok(())
}
```

## Good

```rust
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

/// Give `path` the contents `bytes`, or leave it exactly as it was.
pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "destination has no directory")
    })?;
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "destination has no file name")
    })?;
    // Same directory, so the rename stays inside one filesystem; the pid keeps
    // two writers from choosing the same temporary.
    let temp = dir.join(format!(
        ".{}.tmp.{}",
        name.to_string_lossy(),
        std::process::id()
    ));

    let written = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(bytes)?;
        // Force the contents out before the name that promises them exists.
        file.sync_all()
    })();
    if let Err(err) = written {
        // The destination has not been touched, so the old version survives.
        let _ = fs::remove_file(&temp);
        return Err(err);
    }

    if let Err(err) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }

    // The rename is durable only once the directory entry is. Windows has no
    // equivalent and needs ReplaceFileW rather than this sequence.
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    Ok(())
}

fn main() -> io::Result<()> {
    let dir = std::env::temp_dir().join(format!("replace-demo-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    let path = dir.join("state.json");

    replace(&path, br#"{"version":1}"#)?;
    replace(&path, br#"{"version":2}"#)?;
    assert_eq!(fs::read(&path)?, br#"{"version":2}"#);

    // A replacement that cannot even start leaves the previous version whole.
    assert!(replace(&dir.join("missing").join("state.json"), b"x").is_err());
    assert_eq!(fs::read(&path)?, br#"{"version":2}"#);

    fs::remove_dir_all(&dir)
}
```

## Crash And Concurrency Cases

- a write that fails partway leaves the destination byte-for-byte unchanged,
  and leaves no temporary behind;
- replacing a file twice in a row leaves only the second version;
- a reader that opens the destination between two replacements sees one
  complete version, never a prefix;
- the temporary is created in the destination's directory, so the rename does
  not cross a filesystem;
- replacing a file the process cannot write to fails without destroying it.

## See Also

- [proj-append-log-recovery](proj-append-log-recovery.md) - the durability story for appends rather than replacements
- [proj-secret-file-mode](proj-secret-file-mode.md) - the temporary needs the destination's restrictive mode too
- [async-tokio-fs](async-tokio-fs.md) - running this sequence off the executor threads
- [err-short-read](err-short-read.md) - the other place a byte count is mistaken for a guarantee
- [api-record-checksum](api-record-checksum.md) - detecting the corruption this sequence prevents
