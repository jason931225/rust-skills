# api-typed-command-dispatch

> Give each request type its own associated response type and decoder, so a dispatcher cannot return the wrong domain type for what was asked

## Why It Matters

A dispatcher shaped as `execute(opcode: u32, payload: &[u8]) -> Vec<u8>` (or
its typed-but-untied cousin, `execute<T>(cmd: &Command) -> T`) separates the
request from how its response gets decoded. Every call site is then
responsible for knowing, out of band, which decoder matches which command —
and nothing stops a caller from decoding a temperature-log response as if it
were an identify-device response, applying the wrong byte layout, wrong
scale factor, or wrong field offsets to bytes that parse without error and
produce a plausible-looking wrong answer. This is exactly the class of bug a
type system exists to prevent, but only if the response type is tied to the
request type at the type level rather than chosen by the caller at each call
site.

## Request Response Pairing Rules

- Give each distinct request a distinct type, and put an associated type on
  it (or on a trait it implements) naming its own response type. The
  dispatcher's signature should make it impossible to supply a request and
  ask for an unrelated response type.
- Put the decoder for a response next to the request that produces it — as
  an associated function, a `TryFrom` impl, or a method on the request type
  — so the pairing lives in one place instead of being re-derived at every
  call site.
- Do not accept a raw opcode and a raw byte buffer at the public boundary if
  the set of commands is closed and known; reserve that shape for a
  transport-layer detail hidden behind the typed dispatcher, not the public
  API.
- Where the set of commands is genuinely open (a plugin system, a wire
  protocol with vendor extensions), keep the closed part of the contract
  (the request/response pairing) typed for every known command, and route
  only the unknown remainder through an explicit fallback path.
- Test that swapping which decoder a request uses is a compile error, not a
  runtime assertion — that is the property this pattern is buying.

## Bad

```rust
// The opcode and the byte layout used to decode the response are two
// separate decisions the caller must keep in sync by hand. Nothing stops
// `execute` from being called with the wrong pair.
fn execute(opcode: u32, payload: &[u8]) -> Vec<u8> {
    // dispatch on opcode, return raw bytes
    payload.to_vec()
}

fn read_temperature(device: &mut Device) -> f64 {
    let raw = execute(0x02, &[]); // "read log page" opcode
    // Decoding these bytes as a temperature reading is a decision made
    // here, disconnected from the request that produced them — an easy
    // copy-paste bug reads a different command's response the same way.
    raw[0] as f64
}
```

## Good

```rust
/// A request knows its own response type; nothing else gets to choose one.
trait Request {
    type Response;
    fn decode(payload: &[u8]) -> Self::Response;
}

struct ReadTemperature;

struct Temperature(f64);

impl Request for ReadTemperature {
    type Response = Temperature;
    fn decode(payload: &[u8]) -> Temperature {
        // The scale factor lives exactly where the byte layout does.
        Temperature(payload[0] as f64 / 10.0)
    }
}

struct Identify;

struct DeviceInfo {
    vendor_id: u16,
}

impl Request for Identify {
    type Response = DeviceInfo;
    fn decode(payload: &[u8]) -> DeviceInfo {
        DeviceInfo { vendor_id: u16::from_le_bytes([payload[0], payload[1]]) }
    }
}

/// The return type is pinned by which `Request` was supplied — a caller
/// cannot decode an `Identify` response as a `Temperature`.
fn execute<R: Request>(_request: R, payload: &[u8]) -> R::Response {
    R::decode(payload)
}

fn main() {
    let temp = execute(ReadTemperature, &[215]);
    assert_eq!(temp.0, 21.5);

    let info = execute(Identify, &[0x34, 0x12]);
    assert_eq!(info.vendor_id, 0x1234);

    // `execute(ReadTemperature, &[215])` returning anything other than
    // `Temperature` does not compile — the pairing is enforced by the
    // associated type, not by caller discipline.
}
```

## Mismatched Decode Cases To Test

- decoding a `ReadTemperature` response never produces a `DeviceInfo`, and
  the reverse, because `execute`'s return type is derived from the request
  type, not chosen independently at the call site;
- adding a new request type requires supplying its own `Response` and
  `decode`, so a new command cannot silently reuse another command's decoder
  by omission;
- a request whose decoder applies the wrong scale factor or field offset is
  caught by a test against known-good bytes for that specific request, kept
  next to the request's own decoder;
- the raw-opcode transport layer, if one exists underneath, is not reachable
  from the typed public API without going through a request type.

## See Also

- [api-parse-dont-validate](api-parse-dont-validate.md) - decoding into a type once, at the boundary, is the mechanism this rule pins to a specific request
- [trait-associated-type-vs-generic](trait-associated-type-vs-generic.md) - why an associated type, not a second generic parameter, is the right shape when each request has exactly one response
- [api-typed-response](api-typed-response.md) - building the outbound side of the same typed pairing
- [conv-tryfrom-fallible](conv-tryfrom-fallible.md) - the decode step itself, when it can fail
- [type-enum-states](type-enum-states.md) - closing the set of commands into an enum when the transport-layer fallback case does not apply
