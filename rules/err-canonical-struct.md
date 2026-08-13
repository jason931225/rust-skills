# err-canonical-struct

> Expose library errors as situation-specific opaque structs with a private kind, captured backtrace, and `is_*` helpers

## Why It Matters

A public error enum freezes every failure mode as part of the crate's API. Adding an internal case, wrapping a new upstream crate, or hiding an unhandleable fault becomes a breaking change, and callers start matching on details they cannot recover from. Microsoft Pragmatic Rust Guidelines (M-ERRORS-CANONICAL-STRUCTS) keep each error a situation-specific struct that owns a `Backtrace`, an optional cause, and the accessors callers actually need. Simple crates export one `Error`; larger crates split by domain (`AccessError`, `ConfigurationError`) instead of one global enum. Callers classify with `is_*` helpers and context getters, not by matching public variants.

## Bad

```rust
// One crate-wide enum so every function can "just" return it.
// Callers match on variants you consider internal, and you cannot
// add a failure mode without a breaking change.
pub enum GlobalEverythingErrorEnum {
    DownloadFailed,
    VmBootFailed,
    JsonBroken,
    TomlBroken,
    Io(std::io::Error),
}

fn download_iso() -> Result<(), GlobalEverythingErrorEnum> {
    Err(GlobalEverythingErrorEnum::DownloadFailed)
}

fn start_vm() -> Result<(), GlobalEverythingErrorEnum> {
    Err(GlobalEverythingErrorEnum::VmBootFailed)
}

// Distinct parse situations get distinct variants instead of one reusable type.
fn parse_json() -> Result<(), GlobalEverythingErrorEnum> {
    Err(GlobalEverythingErrorEnum::JsonBroken)
}

fn parse_toml() -> Result<(), GlobalEverythingErrorEnum> {
    Err(GlobalEverythingErrorEnum::TomlBroken)
}

fn is_io(err: &GlobalEverythingErrorEnum) -> bool {
    matches!(err, GlobalEverythingErrorEnum::Io(_))
}
```

## Good

```rust
use std::backtrace::Backtrace;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ConfigurationError {
    backtrace: Backtrace,
    config_file: PathBuf,
}

impl ConfigurationError {
    pub(crate) fn new(config_file: PathBuf) -> Self {
        Self {
            backtrace: Backtrace::capture(),
            config_file,
        }
    }

    pub fn config_file(&self) -> &Path {
        &self.config_file
    }
}

impl Display for ConfigurationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Summary sentence of what happened.
        writeln!(
            f,
            "failed to load configuration from {}",
            self.config_file.display()
        )?;
        // Captured backtrace (empty unless the process asked for one).
        write!(f, "{}", self.backtrace)
    }
}

impl std::error::Error for ConfigurationError {}
```

## Domain Grouping

Split public error types when the situations do not overlap. Reuse a type when they do. Do not invent a new type per function, and do not collapse unrelated domains into one enum just to avoid extra structs.

```rust
// Prefer this
fn download_iso() -> Result<(), DownloadError> {
    Err(DownloadError)
}
fn start_vm() -> Result<(), VmError> {
    Err(VmError)
}

// Over that
fn download_iso_bad() -> Result<(), GlobalEverythingErrorEnum> {
    Err(GlobalEverythingErrorEnum)
}
fn start_vm_bad() -> Result<(), GlobalEverythingErrorEnum> {
    Err(GlobalEverythingErrorEnum)
}

// However, not every function warrants a new error type. Errors
// should be general enough to be reused.
fn parse_json() -> Result<(), ParseError> {
    Err(ParseError)
}
fn parse_toml() -> Result<(), ParseError> {
    Err(ParseError)
}

pub struct DownloadError;
pub struct VmError;
pub struct ParseError;
pub struct GlobalEverythingErrorEnum;
```

## Private Kind and `is_*` Helpers

If the API mixes operations or wraps several upstream libraries, store an inner `ErrorKind`. Keep that enum crate-private so you do not publish every failure mode — including internal, unhandleable ones. Expose `is_*` helpers instead of letting callers match public variants.

```rust
use std::backtrace::Backtrace;

#[derive(Debug)]
pub(crate) enum ErrorKind {
    Io(std::io::Error),
    Protocol,
}

#[derive(Debug)]
pub struct HttpError {
    kind: ErrorKind,
    backtrace: Backtrace,
}

impl HttpError {
    pub fn is_io(&self) -> bool {
        matches!(&self.kind, ErrorKind::Io(_))
    }

    pub fn is_protocol(&self) -> bool {
        matches!(&self.kind, ErrorKind::Protocol)
    }
}
```

`err-custom-type` still applies to closed application domains where matching on variants is the product. A library surface that must evolve uses this opaque struct: `thiserror` may generate `Display` / `Error` boilerplate, but the public type is not a matchable enum of every internal cause.

## Construction and Conversion

Most upstream errors do not provide a backtrace. Capture one when you construct the error — either in an `Error::new()` flavor or in `From<UpstreamError>`.

```rust
use std::backtrace::Backtrace;

impl HttpError {
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<std::io::Error> for HttpError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorKind::Io(err))
    }
}

impl HttpError {
    pub fn io_source(&self) -> Option<&std::io::Error> {
        match &self.kind {
            ErrorKind::Io(err) => Some(err),
            ErrorKind::Protocol => None,
        }
    }
}
```

Use `From` for owned conversions so `?` works (`err-from-impl`). Reach for `map_err` only when the type is foreign or you must attach extra context (`err-context-chain`).

## Display and `Error`

`Display` renders a summary sentence, the captured backtrace, and any upstream
cause. Because that rendering already includes the cause, use an empty
`std::error::Error` implementation for this convention; do not also return the
same cause from `source()` and make reporters print it twice.

```rust
use std::fmt::{Display, Formatter};

impl Display for HttpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Print a summary sentence what happened.
        writeln!(f, "http request failed")?;
        // Print `self.backtrace`.
        writeln!(f, "{}", self.backtrace)?;
        // Print any additional upstream 'cause' information you might have.
        if let ErrorKind::Io(err) = &self.kind {
            write!(f, "{err}")?;
        }
        Ok(())
    }
}

impl std::error::Error for HttpError {}
```

If the crate emits many errors, add a private `bail!()` helper that constructs the struct and returns `Err`. Do not publish that macro.

## Backtrace Cost

A trace connects an error reported much later to the call site that created it,
which is especially useful after asynchronous handoffs. `Backtrace::capture()`
consults the standard backtrace environment policy and avoids walking frames
when capture is disabled. Keep capture in the constructor and let operators
enable it for diagnosis; benchmark an error-heavy hot path before designing a
custom suppression mechanism.

## See Also

- [err-custom-type](err-custom-type.md) - closed application domains may still use matchable enums; library surfaces that must evolve stay opaque
- [err-from-impl](err-from-impl.md) - capture the backtrace inside `From`, then let `?` convert
- [err-source-chain](err-source-chain.md) - alternative convention: omit the cause from `Display` and expose it through `source()`
- [err-context-chain](err-context-chain.md) - attach situation text without making the kind public
- [err-thiserror-lib](err-thiserror-lib.md) - generate `Display` / `Error` boilerplate; do not publish every derived variant
- [api-non-exhaustive](api-non-exhaustive.md) - if a kind must be public, it is still not a substitute for `is_*` helpers
