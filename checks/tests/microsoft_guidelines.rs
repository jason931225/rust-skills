use std::fmt;

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
