# proj-secret-file-mode

> Create credential files owner-only, in an owner-only directory, before writing anything into them

## Why It Matters

A token cached in `~/.config/app/session.json` with default permissions is
readable by every account on the machine, and on a shared host or a container
with a sidecar that is a credential leak with no exploit required. Creating
the file and then calling `set_permissions` leaves a window in which the
secret is already on disk and world-readable. The permissions have to be part
of the create call, and the directory needs them too — a private file inside a
traversable directory still leaks its name, size, and mtime.

## Credential File Permission Requirements

- Create with the mode set atomically (`OpenOptions::mode(0o600)` on Unix),
  not by relaxing permissions afterwards.
- Create the containing directory `0o700`, and verify it if it already exists;
  a pre-existing world-writable directory is an attack, not a convenience.
- Refuse to read a credential file whose mode is broader than expected, the
  way SSH refuses a group-readable private key.
- Replace atomically: write a temporary file in the same directory with the
  same mode, then rename over the target, so a crash cannot leave a truncated
  or partially-permissioned file.
- Do not write secrets to a shared temporary directory, and do not put them in
  a path an attacker can pre-create as a symlink.
- On Windows the equivalent is an explicit DACL; the default inherited ACL is
  not owner-only.

## Bad

```rust
fn save_token(token: &str) -> io::Result<()> {
    // Written world-readable, then narrowed: the secret was already exposed
    fs::write("/home/app/.config/app/token", token)?;
    fs::set_permissions("/home/app/.config/app/token", Permissions::from_mode(0o600))
}
```

## Good

```rust
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

/// Creates the file with its final permissions in one call, so the secret is
/// never on disk under a wider mode.
pub fn save_secret(dir: &Path, name: &str, secret: &[u8]) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(dir)?;

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);

    let mut file = options.open(dir.join(name))?;
    file.write_all(secret)?;
    file.sync_all()
}

fn main() -> io::Result<()> {
    let dir = std::env::temp_dir().join("rust-skills-secret-example");
    save_secret(&dir, "token", b"s3cret")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(dir.join("token"))?.permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the secret must be owner-only");
    }
    fs::remove_dir_all(&dir)
}
```

## Permission Cases To Test

- the created file's mode is exactly owner read/write, checked on the file
  rather than on the umask;
- the containing directory is owner-only, including when it already existed;
- a token file with broader permissions is refused on read;
- replacing a token leaves no window where the old and new files are both
  present, and no temporary file with a wider mode;
- nothing is written to a shared temporary directory.

## See Also

- [type-secret-material](type-secret-material.md) - the in-memory half of the same contract
- [proj-typed-config](proj-typed-config.md) - where credentials enter the process
- [api-session-security](api-session-security.md) - session tokens have the same handling rules
- [async-tokio-fs](async-tokio-fs.md) - atomic replacement and durability
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - keep the same values out of telemetry
