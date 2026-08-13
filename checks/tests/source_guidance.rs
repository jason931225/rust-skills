use std::fmt;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};

#[derive(Default)]
struct Frame {
    bytes: Vec<u8>,
}

struct Store;

impl Store {
    fn load_into(&self, key: u64, frame: &mut Frame) {
        frame.bytes.clear();
        frame.bytes.extend_from_slice(&key.to_le_bytes());
    }
}

#[test]
fn caller_owned_output_reuses_capacity() {
    let store = Store;
    let mut frame = Frame::default();

    store.load_into(1, &mut frame);
    let capacity = frame.bytes.capacity();
    store.load_into(2, &mut frame);

    assert_eq!(frame.bytes, 2_u64.to_le_bytes());
    assert_eq!(frame.bytes.capacity(), capacity);
}

fn require_send<T: Send>(_: &T) {}

async fn send_entry_point() -> usize {
    let value = {
        let local = std::rc::Rc::new(3_usize);
        *local
    };
    std::future::ready(()).await;
    value
}

#[test]
fn public_future_remains_send_after_local_temporary_drops() {
    let future = send_entry_point();
    require_send(&future);
}

struct Origin(&'static str);
struct Destination(&'static str);
struct Route {
    origin: Origin,
    destination: Destination,
}

impl Route {
    fn new(origin: Origin, destination: Destination) -> Self {
        Self {
            origin,
            destination,
        }
    }
}

#[test]
fn cascaded_initialization_preserves_semantic_roles() {
    let route = Route::new(Origin("oslo"), Destination("helsinki"));
    assert_eq!(route.origin.0, "oslo");
    assert_eq!(route.destination.0, "helsinki");
}

struct Secret {
    _value: &'static str,
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

#[test]
fn sensitive_debug_output_is_regression_checked() {
    let rendered = format!(
        "{:?}",
        Secret {
            _value: "token-123",
        }
    );
    assert_eq!(rendered, "[redacted]");
    assert!(!rendered.contains("token-123"));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Probe {
    Ready,
    NotReady,
}

fn readiness(startup_complete: bool, draining: bool) -> Probe {
    if startup_complete && !draining {
        Probe::Ready
    } else {
        Probe::NotReady
    }
}

#[test]
fn readiness_rejects_starting_and_draining_instances() {
    assert_eq!(readiness(false, false), Probe::NotReady);
    assert_eq!(readiness(true, false), Probe::Ready);
    assert_eq!(readiness(true, true), Probe::NotReady);
}

fn expand_backfill_contract<'a>(old_rows: &'a [Option<&'a str>]) -> Vec<&'a str> {
    old_rows
        .iter()
        .map(|value| value.unwrap_or("pending"))
        .collect()
}

#[test]
fn schema_backfill_is_restartable_and_preserves_existing_values() {
    let rows = [None, Some("confirmed"), None];
    let once = expand_backfill_contract(&rows);
    let migrated = once
        .iter()
        .copied()
        .map(Some)
        .collect::<Vec<Option<&str>>>();
    let twice = expand_backfill_contract(&migrated);
    assert_eq!(once, ["pending", "confirmed", "pending"]);
    assert_eq!(twice, once);
}

fn parse_request_id(value: &str) -> Option<&str> {
    (!value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
    .then_some(value)
}

#[test]
fn request_ids_are_bounded_and_header_safe() {
    assert_eq!(parse_request_id("req_123-abc"), Some("req_123-abc"));
    assert_eq!(parse_request_id(""), None);
    assert_eq!(parse_request_id("line\nbreak"), None);
    assert_eq!(parse_request_id(&"a".repeat(65)), None);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SavedResponse {
    status: u16,
    body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum IdempotencyClaim {
    New,
    Replay(SavedResponse),
    Conflict,
}

fn claim(existing_fingerprint: Option<&[u8]>, fingerprint: &[u8]) -> IdempotencyClaim {
    match existing_fingerprint {
        None => IdempotencyClaim::New,
        Some(existing) if existing == fingerprint => IdempotencyClaim::Replay(SavedResponse {
            status: 202,
            body: b"accepted".to_vec(),
        }),
        Some(_) => IdempotencyClaim::Conflict,
    }
}

#[test]
fn idempotency_replays_matches_and_rejects_key_reuse() {
    assert_eq!(claim(None, b"request-a"), IdempotencyClaim::New);
    assert!(matches!(
        claim(Some(b"request-a"), b"request-a"),
        IdempotencyClaim::Replay(_)
    ));
    assert_eq!(
        claim(Some(b"request-a"), b"request-b"),
        IdempotencyClaim::Conflict
    );
}

fn retry_delay(base_ms: u64, attempt: u32, jitter_ms: u64) -> u64 {
    base_ms
        .saturating_mul(1_u64 << attempt.min(16))
        .saturating_add(jitter_ms)
}

#[test]
fn retry_backoff_grows_caps_and_adds_jitter() {
    assert_eq!(retry_delay(100, 0, 7), 107);
    assert_eq!(retry_delay(100, 3, 7), 807);
    assert_eq!(retry_delay(100, 20, 7), 6_553_607);
}

#[test]
fn black_box_http_contract_crosses_a_real_socket() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        let mut request = [0_u8; 256];
        let bytes = connection.read(&mut request).unwrap();
        let request = std::str::from_utf8(&request[..bytes]).unwrap();
        assert!(request.starts_with("GET /health/live HTTP/1.1\r\n"));
        connection
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .unwrap();
    });

    let mut client = TcpStream::connect(address).unwrap();
    client
        .write_all(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    server.join().unwrap();

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.ends_with("\r\n\r\nok"));
}
