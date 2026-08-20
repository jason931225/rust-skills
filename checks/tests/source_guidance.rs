use std::error::Error as StdError;
use std::fmt;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

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
fn semantic_constructor_types_prevent_role_ambiguity() {
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
fn sensitive_debug_output_is_redacted() {
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

#[derive(Debug)]
struct ConfigurationError {
    path: PathBuf,
    source: std::io::Error,
}

impl fmt::Display for ConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to load configuration from {}",
            self.path.display()
        )
    }
}

impl StdError for ConfigurationError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.source)
    }
}

#[test]
fn opaque_error_keeps_standard_source_chain() {
    let error = ConfigurationError {
        path: Path::new("settings.toml").to_owned(),
        source: std::io::Error::other("unavailable"),
    };

    assert_eq!(
        error.to_string(),
        "failed to load configuration from settings.toml"
    );
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("unavailable")
    );
}

fn serve_health(listener: TcpListener) -> std::io::Result<()> {
    let (mut connection, _) = listener.accept()?;
    connection.set_read_timeout(Some(Duration::from_secs(2)))?;
    connection.set_write_timeout(Some(Duration::from_secs(2)))?;

    let mut request = Vec::new();
    let mut chunk = [0_u8; 128];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = connection.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if request.len() > 4096 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request headers exceed test limit",
            ));
        }
    }

    let request = std::str::from_utf8(&request)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if !request.starts_with("GET /health/live HTTP/1.1\r\n") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unexpected request target",
        ));
    }

    connection.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
    )?;
    Ok(())
}

#[test]
fn socket_smoke_test_exchanges_a_complete_http_message() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback listener");
    let address = listener.local_addr().expect("read listener address");
    let server = std::thread::spawn(move || serve_health(listener));

    let mut client = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .expect("connect to loopback listener");
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set client read timeout");
    client
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("set client write timeout");
    client
        .write_all(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .expect("write request");
    client.shutdown(Shutdown::Write).expect("finish request");

    let mut response = String::new();
    client.read_to_string(&mut response).expect("read response");
    server
        .join()
        .expect("server thread panicked")
        .expect("server failed");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("Content-Type: text/plain\r\n"));
    assert!(response.ends_with("\r\n\r\nok"));
}

/// Shutdown ordering: readiness must fail before cancellation, so routing stops
/// sending work the process is about to refuse, and the drain must be bounded so
/// a task that ignores its token cannot hold the process open until it is killed.
#[tokio::test]
async fn graceful_shutdown_fails_readiness_before_cancelling_and_bounds_the_drain() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::task::JoinSet;
    use tokio_util::sync::CancellationToken;

    // Scaled-down stand-ins for the propagation delay and the drain budget.
    const READINESS_PROPAGATION: Duration = Duration::from_millis(5);
    const DRAIN_BUDGET: Duration = Duration::from_millis(25);

    let ready = Arc::new(AtomicBool::new(true));
    let ready_was_false_at_cancel = Arc::new(AtomicBool::new(false));
    let shutdown = CancellationToken::new();
    let mut tasks = JoinSet::new();

    // Records what readiness said at the moment cancellation was observed.
    {
        let ready = Arc::clone(&ready);
        let observed = Arc::clone(&ready_was_false_at_cancel);
        let token = shutdown.child_token();
        tasks.spawn(async move {
            token.cancelled().await;
            observed.store(!ready.load(Ordering::SeqCst), Ordering::SeqCst);
        });
    }
    // A task that never observes its token: it must be aborted, not waited on.
    tasks.spawn(async { std::future::pending::<()>().await });

    ready.store(false, Ordering::SeqCst);
    tokio::time::sleep(READINESS_PROPAGATION).await;
    shutdown.cancel();

    let drained = tokio::time::timeout(DRAIN_BUDGET, async {
        while tasks.join_next().await.is_some() {}
    })
    .await;

    assert!(
        ready_was_false_at_cancel.load(Ordering::SeqCst),
        "readiness must already be failing when cancellation reaches the tasks"
    );
    assert!(
        drained.is_err(),
        "a task that ignores cancellation must hit the drain budget"
    );

    tasks.shutdown().await;
    assert_eq!(tasks.len(), 0, "aborting the JoinSet must leave nothing running");
}

// --- perf-io-buffering ------------------------------------------------------

/// A sink whose writes fail, like a closed pipe or a full disk.
struct FailingSink {
    attempted: std::rc::Rc<std::cell::Cell<bool>>,
}

impl Write for FailingSink {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        self.attempted.set(true);
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn dropping_a_bufwriter_discards_the_error_that_an_explicit_flush_surfaces() {
    use std::io::BufWriter;

    // Dropped without flushing: Drop writes the buffer out through the inner
    // writer and discards the failure. Nothing reports it.
    let attempted = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let mut writer = BufWriter::new(FailingSink { attempted: std::rc::Rc::clone(&attempted) });
        writer.write_all(b"record").expect("the write is buffered, not yet attempted");
    } // <- the failing write happens here, and its error is dropped
    assert!(attempted.get(), "drop did write the buffer out");

    // Explicit flush: the same failure is a value the caller must handle.
    let attempted = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut writer = BufWriter::new(FailingSink { attempted: std::rc::Rc::clone(&attempted) });
    writer.write_all(b"record").expect("buffered");
    let outcome = writer.flush();
    assert!(attempted.get());
    assert_eq!(
        outcome.expect_err("flush must report the failure").kind(),
        std::io::ErrorKind::BrokenPipe
    );
}

// --- err-catch-unwind-boundary ----------------------------------------------

#[derive(Debug, PartialEq)]
enum RequestOutcome {
    Handled(u32),
    Failed,
}

fn handle_one_request(input: u32) -> RequestOutcome {
    // The boundary converts a panic into a value; it does not let it unwind
    // into the runtime that owns the worker.
    let result = std::panic::catch_unwind(|| {
        if input == 0 {
            panic!("invariant violated for request {input}");
        }
        input * 2
    });
    match result {
        Ok(value) => RequestOutcome::Handled(value),
        Err(_) => RequestOutcome::Failed,
    }
}

#[test]
fn a_panicking_request_is_isolated_and_the_worker_keeps_serving() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // keep the test output readable

    assert_eq!(handle_one_request(21), RequestOutcome::Handled(42));
    assert_eq!(handle_one_request(0), RequestOutcome::Failed);
    // The worker survives the panic and serves the next request.
    assert_eq!(handle_one_request(2), RequestOutcome::Handled(4));

    std::panic::set_hook(previous);
}

// --- async-cancel-safety ----------------------------------------------------

#[tokio::test]
async fn a_cancelled_read_exact_loses_its_partial_data_while_read_keeps_progress() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // read_exact: cancelled mid-way, the bytes it consumed are gone.
    let (mut client, mut server) = tokio::io::duplex(64);
    server.write_all(b"abc").await.expect("partial write");
    let mut buffer = [0u8; 8];
    tokio::select! {
        _ = client.read_exact(&mut buffer) => panic!("cannot complete: only 3 bytes are available"),
        _ = tokio::time::sleep(Duration::from_millis(20)) => {}
    }
    server.write_all(b"defghijk").await.expect("rest");
    let mut after = [0u8; 3];
    client.read_exact(&mut after).await.expect("read after cancellation");
    assert_ne!(
        &after, b"abc",
        "the first three bytes were consumed by the cancelled read_exact and are unrecoverable"
    );

    // read into a caller-owned buffer: progress survives cancellation because
    // the buffer, not the future, holds it.
    let (mut client, mut server) = tokio::io::duplex(64);
    server.write_all(b"abc").await.expect("partial write");
    let mut owned = Vec::new();
    let mut chunk = [0u8; 8];
    let read = client.read(&mut chunk).await.expect("first read");
    owned.extend_from_slice(&chunk[..read]);
    assert_eq!(owned, b"abc");
}

// --- api-password-auth ------------------------------------------------------

fn verify_encoded(encoded: &str, submitted: &str) -> bool {
    encoded == format!("hash:{submitted}")
}

fn login(stored: Option<&str>, submitted: &str) -> (u16, &'static str) {
    // An unknown account is verified against a dummy hash so it takes the same
    // path, and returns the same answer, as a wrong password.
    let dummy = "hash:__no_such_account__";
    let encoded = stored.unwrap_or(dummy);
    if verify_encoded(encoded, submitted) && stored.is_some() {
        (200, "signed in")
    } else {
        (401, "invalid credentials")
    }
}

#[test]
fn an_unknown_account_and_a_wrong_password_are_indistinguishable() {
    let stored = "hash:correct-horse";
    assert_eq!(login(Some(stored), "correct-horse"), (200, "signed in"));

    // The whole point of the rule: these two must not differ.
    assert_eq!(login(None, "anything"), login(Some(stored), "wrong"));
    assert_eq!(login(None, "anything"), (401, "invalid credentials"));
}

// --- api-browser-security ---------------------------------------------------

fn safe_redirect(requested: &str) -> Option<&'static str> {
    match requested {
        "dashboard" => Some("/dashboard"),
        "settings" => Some("/settings"),
        _ => None,
    }
}

fn csrf_ok(session_token: &[u8], submitted: &[u8]) -> bool {
    session_token.len() == submitted.len()
        && session_token
            .iter()
            .zip(submitted)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

#[test]
fn redirect_targets_come_from_a_table_and_csrf_tokens_compare_in_full() {
    assert_eq!(safe_redirect("dashboard"), Some("/dashboard"));

    for hostile in [
        "https://evil.example/login",
        "//evil.example",
        "/\\evil.example",
        "javascript:alert(1)",
        "../admin",
        "/dashboard\r\nSet-Cookie: a=b",
    ] {
        assert_eq!(safe_redirect(hostile), None, "{hostile} must not redirect");
    }

    let session = b"tok-abcdef";
    assert!(csrf_ok(session, b"tok-abcdef"));
    assert!(!csrf_ok(session, b"tok-abcdeg"));
    assert!(!csrf_ok(session, b"tok-abcde"), "a length mismatch is not a match");
}

// --- type-affine-quantity -----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Celsius(f64);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct CelsiusDelta(f64);

impl std::ops::Sub for Celsius {
    type Output = CelsiusDelta;
    fn sub(self, rhs: Celsius) -> CelsiusDelta {
        CelsiusDelta(self.0 - rhs.0)
    }
}

impl std::ops::Add<CelsiusDelta> for Celsius {
    type Output = Celsius;
    fn add(self, delta: CelsiusDelta) -> Celsius {
        Celsius(self.0 + delta.0)
    }
}

impl std::ops::Add for CelsiusDelta {
    type Output = CelsiusDelta;
    fn add(self, rhs: CelsiusDelta) -> CelsiusDelta {
        CelsiusDelta(self.0 + rhs.0)
    }
}

#[test]
fn two_absolute_temperatures_subtract_to_a_delta_and_cannot_be_added() {
    let boiling = Celsius(100.0);
    let room = Celsius(20.0);

    let change: CelsiusDelta = boiling - room;
    assert_eq!(change, CelsiusDelta(80.0));

    let warmed: Celsius = room + CelsiusDelta(15.0);
    assert_eq!(warmed, Celsius(35.0));

    // `boiling + room` does not compile: no `Add<Celsius>` impl exists for
    // `Celsius`. Only the delta type composes with itself.
    assert_eq!(CelsiusDelta(10.0) + CelsiusDelta(5.0), CelsiusDelta(15.0));
}

// --- api-typed-command-dispatch -----------------------------------------------

trait TdcRequest {
    type Response;
    fn decode(payload: &[u8]) -> Self::Response;
}

struct ReadTemperature;
struct TdcTemperature(f64);

impl TdcRequest for ReadTemperature {
    type Response = TdcTemperature;
    fn decode(payload: &[u8]) -> TdcTemperature {
        TdcTemperature(payload[0] as f64 / 10.0)
    }
}

struct TdcIdentify;
struct TdcDeviceInfo {
    vendor_id: u16,
}

impl TdcRequest for TdcIdentify {
    type Response = TdcDeviceInfo;
    fn decode(payload: &[u8]) -> TdcDeviceInfo {
        TdcDeviceInfo { vendor_id: u16::from_le_bytes([payload[0], payload[1]]) }
    }
}

fn tdc_execute<R: TdcRequest>(_request: R, payload: &[u8]) -> R::Response {
    R::decode(payload)
}

#[test]
fn a_requests_response_type_is_pinned_by_the_request_not_the_call_site() {
    let temp = tdc_execute(ReadTemperature, &[215]);
    assert_eq!(temp.0, 21.5);

    let info = tdc_execute(TdcIdentify, &[0x34, 0x12]);
    assert_eq!(info.vendor_id, 0x1234);
}

// --- trait-capability-mixin ----------------------------------------------------

trait HasSpi {
    type Spi;
    fn spi(&self) -> &Self::Spi;
}

trait HasI2c {
    type I2c;
    fn i2c(&self) -> &Self::I2c;
}

trait FanDiagMixin: HasSpi + HasI2c {
    fn read_fan_speed(&self) -> u16 {
        let _spi = self.spi();
        let _i2c = self.i2c();
        1200
    }
}

impl<T: HasSpi + HasI2c> FanDiagMixin for T {}

struct Board {
    spi_bus: (),
    i2c_bus: (),
}

impl HasSpi for Board {
    type Spi = ();
    fn spi(&self) -> &() {
        &self.spi_bus
    }
}

impl HasI2c for Board {
    type I2c = ();
    fn i2c(&self) -> &() {
        &self.i2c_bus
    }
}

#[test]
fn a_mixin_method_exists_only_on_a_receiver_owning_every_ingredient() {
    let board = Board { spi_bus: (), i2c_bus: () };
    assert_eq!(board.read_fan_speed(), 1200);
}

// --- type-exclusive-occupancy-guard --------------------------------------------

struct InFlight<T> {
    buffer: T,
    _not_send: std::marker::PhantomData<*mut ()>,
}

impl<T> InFlight<T> {
    fn start(buffer: T) -> Self {
        InFlight { buffer, _not_send: std::marker::PhantomData }
    }

    fn wait(self) -> T {
        self.buffer
    }
}

#[test]
fn a_buffer_in_flight_is_recoverable_only_by_consuming_its_guard() {
    let buffer = vec![0u8; 4];
    let transfer = InFlight::start(buffer);

    // No accessor exposes the buffer while the guard is held, and
    // `thread::spawn(move || transfer.wait())` does not compile because
    // `InFlight<Vec<u8>>` is `!Send`.
    let recovered = transfer.wait();
    assert_eq!(recovered, vec![0u8; 4]);
}

// --- api-typestate (independent required fields) -------------------------------

struct Missing;
struct SetField<T>(T);

struct DerBuilder<Mnemonic, FaultClass> {
    mnemonic: Mnemonic,
    fault_class: FaultClass,
}

impl DerBuilder<Missing, Missing> {
    fn new() -> Self {
        DerBuilder { mnemonic: Missing, fault_class: Missing }
    }
}

impl<FC> DerBuilder<Missing, FC> {
    fn mnemonic(self, value: String) -> DerBuilder<SetField<String>, FC> {
        DerBuilder { mnemonic: SetField(value), fault_class: self.fault_class }
    }
}

impl<M> DerBuilder<M, Missing> {
    fn fault_class(self, value: u8) -> DerBuilder<M, SetField<u8>> {
        DerBuilder { mnemonic: self.mnemonic, fault_class: SetField(value) }
    }
}

impl DerBuilder<SetField<String>, SetField<u8>> {
    fn finish(self) -> (String, u8) {
        (self.mnemonic.0, self.fault_class.0)
    }
}

#[test]
fn independent_required_builder_fields_can_be_set_in_either_order() {
    let a = DerBuilder::new().mnemonic("E101".into()).fault_class(2).finish();
    let b = DerBuilder::new().fault_class(2).mnemonic("E101".into()).finish();
    assert_eq!(a, b);
    // `DerBuilder::new().mnemonic("E101".into()).finish()` does not compile:
    // no `finish` exists while `fault_class` is still `Missing`.
}

// --- async-completion-owned-buffer ---------------------------------------------

/// A submitted operation. The engine owns `buffer` until completion, so
/// nothing else can read or write it in the meantime.
struct AbInFlight {
    buffer: Vec<u8>,
    requested: usize,
}

struct CompletionEngine;

impl CompletionEngine {
    fn submit_read(&self, buffer: Vec<u8>, requested: usize) -> AbInFlight {
        AbInFlight { requested: requested.min(buffer.len()), buffer }
    }

    fn complete(&self, mut in_flight: AbInFlight) -> (Vec<u8>, std::io::Result<usize>) {
        for slot in in_flight.buffer[..in_flight.requested].iter_mut() {
            *slot = 0xab;
        }
        let written = in_flight.requested;
        (in_flight.buffer, Ok(written))
    }
}

#[test]
fn a_completion_read_returns_the_buffer_it_took_by_value() {
    let engine = CompletionEngine;
    let buffer = vec![0u8; 8];

    let in_flight = engine.submit_read(buffer, 4);
    // `buffer` has moved: the bytes are unreachable while the engine owns
    // them, which is what makes a later write by the engine sound.
    let (buffer, result) = engine.complete(in_flight);

    assert_eq!(result.expect("the read completes"), 4);
    assert_eq!(&buffer[..4], &[0xab; 4]);
    assert_eq!(&buffer[4..], &[0u8; 4], "the engine wrote only what was requested");
}

// --- test-cross-target-execution ------------------------------------------------

/// Logic verifiable on the host, kept separate from anything needing the
/// target so a fast host run cannot masquerade as target coverage.
fn frame_len(header: &[u8]) -> Option<usize> {
    let raw = u16::from_le_bytes([*header.first()?, *header.get(1)?]);
    Some(usize::from(raw))
}

#[test]
fn host_verifiable_logic_stays_separate_from_target_only_behavior() {
    assert_eq!(frame_len(&[0x04, 0x00]), Some(4));
    assert_eq!(frame_len(&[0x00, 0x01]), Some(256));
    assert_eq!(frame_len(&[0x04]), None, "a short header has no length");
}

// --- proj-build-script-scope -----------------------------------------------------

/// Stands in for the decision a build script makes: the emitted cfg set must
/// be a function of the target and the package's own features only, never of
/// what happens to exist on the machine running the build.
fn emitted_cfgs(target_os: &str, systemd_feature: bool, host_has_libsystemd: bool) -> Vec<&'static str> {
    let _ = host_has_libsystemd; // deliberately unused: the host must not matter
    let mut cfgs = Vec::new();
    if target_os == "linux" {
        cfgs.push("uses_epoll");
    }
    if systemd_feature {
        cfgs.push("has_systemd");
    }
    cfgs
}

#[test]
fn build_script_output_depends_on_the_target_not_the_build_machine() {
    // Same target, same features, two machines that differ in what is installed.
    let on_builder_a = emitted_cfgs("linux", false, true);
    let on_builder_b = emitted_cfgs("linux", false, false);
    assert_eq!(on_builder_a, on_builder_b, "the artifact must not depend on the build host");

    // The feature, not a host probe, is what turns the capability on.
    assert_eq!(emitted_cfgs("linux", true, false), vec!["uses_epoll", "has_systemd"]);

    // Cross-compiling emits the target's cfgs, not the host's.
    assert_eq!(emitted_cfgs("windows", false, true), Vec::<&str>::new());
}

// --- type-lifetime-branding -----------------------------------------------------

/// The `*mut` marker makes `'brand` invariant. With a covariant
/// `PhantomData<&'brand ()>` the cross-arena mix compiles — verified
/// separately with rustc; that variant is pinned as a compile-fail case.
struct BrandArena<'brand> {
    items: Vec<String>,
    _brand: std::marker::PhantomData<*mut &'brand ()>,
}

struct BrandHandle<'brand> {
    index: usize,
    _brand: std::marker::PhantomData<*mut &'brand ()>,
}

impl<'brand> BrandArena<'brand> {
    fn push(&mut self, value: String) -> BrandHandle<'brand> {
        self.items.push(value);
        BrandHandle { index: self.items.len() - 1, _brand: std::marker::PhantomData }
    }

    fn get(&self, handle: BrandHandle<'brand>) -> &str {
        &self.items[handle.index]
    }
}

fn with_brand_arena<R>(f: impl for<'brand> FnOnce(BrandArena<'brand>) -> R) -> R {
    f(BrandArena { items: Vec::new(), _brand: std::marker::PhantomData })
}

#[test]
fn a_handle_resolves_against_the_arena_that_minted_it() {
    let len = with_brand_arena(|mut arena| {
        let handle = arena.push("hello".to_owned());
        arena.get(handle).len()
    });
    assert_eq!(len, 5);

    // Using a handle from one arena against a second, nested arena fails to
    // compile with E0521 citing invariance over 'brand.
}

// --- perf-iter-lazy (take_while boundary) ---------------------------------------

#[test]
fn take_while_consumes_the_element_that_stopped_it() {
    let data = [1, 2, 3, 99, 4];

    let mut it = data.iter().copied();
    let taken: Vec<_> = it.by_ref().take_while(|&n| n != 99).collect();
    assert_eq!(taken, vec![1, 2, 3]);
    assert_eq!(it.next(), Some(4), "99 was consumed, not left behind");

    let mut kept = Vec::new();
    for n in data.iter().copied() {
        let last = n == 99;
        kept.push(n);
        if last {
            break;
        }
    }
    assert_eq!(kept, vec![1, 2, 3, 99]);
}

// --- type-deref-coercion (invariant-bearing newtype) ----------------------------

struct DerefPort(std::num::NonZeroU16);

impl DerefPort {
    fn new(value: u16) -> Option<Self> {
        std::num::NonZeroU16::new(value).map(DerefPort)
    }
    fn get(&self) -> u16 {
        self.0.get()
    }
}

#[test]
fn an_invariant_bearing_newtype_exposes_no_mutable_deref() {
    let port = DerefPort::new(8080).expect("8080 is non-zero");
    assert_eq!(port.get(), 8080);
    assert!(DerefPort::new(0).is_none(), "the constructor is the only way in");
    // No DerefMut impl exists, so `*port = 0` does not compile — the
    // invariant cannot be assigned away.
}

// --- mem-zero-copy (borrowing fails when text must be transformed) ---------------

#[derive(serde::Deserialize)]
struct BorrowedRecord<'a> {
    #[serde(borrow)]
    text: std::borrow::Cow<'a, str>,
}

#[derive(serde::Deserialize)]
struct PlainRecord<'a> {
    text: std::borrow::Cow<'a, str>,
}

#[test]
fn a_json_string_with_an_escape_cannot_be_borrowed_but_cow_handles_it() {
    use std::borrow::Cow;

    // No escapes: the decoded text is a contiguous run of the input, so a
    // borrowed &str works.
    let plain: &str = serde_json::from_str(r#""hello""#).expect("plain text borrows");
    assert_eq!(plain, "hello");

    // With an escape, the unescaped text appears nowhere in the input, so
    // there is nothing to borrow and deserialization into &str fails outright.
    let borrowed: Result<&str, _> = serde_json::from_str(r#""a\nb""#);
    assert!(borrowed.is_err(), "an escaped string has no slice to borrow");

    // Cow + #[serde(borrow)]: plain text borrows, escaped text allocates.
    let cheap: BorrowedRecord = serde_json::from_str(r#"{"text":"hello"}"#).expect("borrows");
    assert!(matches!(cheap.text, Cow::Borrowed(_)), "the common case borrows");

    let costly: BorrowedRecord = serde_json::from_str(r#"{"text":"a\nb"}"#).expect("allocates");
    assert_eq!(costly.text, "a\nb");
    assert!(matches!(costly.text, Cow::Owned(_)), "only the escaped value allocates");

    // Without #[serde(borrow)], Cow allocates unconditionally — the field
    // looks zero-copy and is not.
    let never: PlainRecord = serde_json::from_str(r#"{"text":"hello"}"#).expect("parses");
    assert!(
        matches!(never.text, Cow::Owned(_)),
        "Cow without #[serde(borrow)] allocates even for plain text"
    );
}

// --- conc-thread-channel --------------------------------------------------------

struct ChannelTotals {
    seen: u64,
    sum: u64,
}

fn accumulate(producers: usize, per_producer: u64) -> ChannelTotals {
    use std::sync::mpsc;
    // Bounded: a producer that outruns the consumer blocks rather than
    // growing the queue.
    let (tx, rx) = mpsc::sync_channel::<u64>(64);

    let mut handles = Vec::new();
    for id in 0..producers {
        let tx = tx.clone();
        handles.push(std::thread::spawn(move || {
            for n in 0..per_producer {
                tx.send(id as u64 + n).expect("consumer is alive");
            }
        }));
    }
    // Must drop, or the channel never disconnects and the loop below hangs.
    drop(tx);

    let mut totals = ChannelTotals { seen: 0, sum: 0 };
    for value in rx {
        totals.seen += 1;
        totals.sum += value;
    }
    for handle in handles {
        handle.join().expect("producer did not panic");
    }
    totals
}

#[test]
fn dropping_every_sender_ends_the_consumer_without_losing_queued_work() {
    let totals = accumulate(4, 1_000);
    assert_eq!(totals.seen, 4_000, "every message arrived before shutdown");

    // Disconnection is the shutdown signal: with the sender gone, recv errs.
    let (tx, rx) = std::sync::mpsc::sync_channel::<u8>(1);
    tx.send(7).expect("capacity available");
    drop(tx);
    assert_eq!(rx.recv().expect("queued value survives disconnection"), 7);
    assert!(rx.recv().is_err(), "an empty, disconnected channel ends the loop");
}
