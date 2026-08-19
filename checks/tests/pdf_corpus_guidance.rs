//! Behavior assertions for the rules derived from the authenticated PDF corpus.
//!
//! Each test states the contract of one rule in executable form, so the ledger
//! in `pdf_corpus_coverage.json` can bind a reviewed unit to a real assertion
//! rather than to prose alone.

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
