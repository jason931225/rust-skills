# api-http-connection-lifecycle

> Force connection close on a one-shot HTTP client, and resend the hostname in every request even though the connection already resolved it

## Why It Matters

Treating the network stack as cleanly layered — sockets underneath, HTTP on
top, each ignorant of the other — is a teaching model, not how the protocol
actually behaves. HTTP/1.1 defaults every connection to `keep-alive`: after
answering, a compliant server holds the TCP connection open waiting for the
next request on the same socket. A client that sends one request, reads the
response, and then just waits for the stream to end is waiting for a close
the server has no reason to send. And a `TcpStream::connect` to an IP address
has already forgotten the hostname that resolved to it — virtual hosting,
TLS SNI, and the HTTP `Host` header all depend on the application layer
resending that name after the transport layer's job is done.

## Connection Lifecycle Requirements

- If a client will only ever send one request per connection, say so
  explicitly: request `HTTP/1.0`, or send `HTTP/1.1` with an explicit
  `Connection: close` header. Do not rely on the peer closing first.
- Always send the `Host` header (or the TLS SNI name) explicitly, even though
  the connection was made to an already-resolved IP address — the transport
  layer does not carry the name forward.
- Do not read a response by looping until the stream returns EOF unless the
  connection is actually going to close; prefer reading exactly
  `Content-Length` bytes, or draining a chunked body to its terminator, so a
  kept-alive connection does not hang the read.
- When reusing connections deliberately (a connection pool, a long-lived
  client), the pool — not each call site — owns close/keep-alive policy;
  document which one a given client type implements.
- Prefer a maintained HTTP client library over hand-rolled `TcpStream`
  framing; write the socket-level version only to understand or diagnose the
  layer a library is hiding.
- After transparent content decoding (gzip, brotli), `Content-Length` — where
  the client still exposes it — reflects the *encoded* wire size, not the
  size of the decoded bytes the caller reads back. Reporting one as the
  other produces a value that does not match either the bytes sent or the
  bytes received.

## Bad

```rust
use std::io::{Read, Write};
use std::net::TcpStream;

// Sends a plain HTTP/1.1 request with no explicit `Connection` header and no
// `Host` header, then reads until EOF. A compliant server treats the
// connection as keep-alive by default and never closes it — this call
// blocks forever waiting for a close that is not coming.
fn fetch(stream: &mut TcpStream, path: &str) -> std::io::Result<String> {
    write!(stream, "GET {path} HTTP/1.1\r\n\r\n")?;
    let mut body = String::new();
    stream.read_to_string(&mut body)?; // hangs on a kept-alive connection
    Ok(body)
}
```

## Good

```rust
use std::io::{Read, Write};

/// A minimal one-shot request: explicit `Connection: close` so the server
/// ends the connection once it has answered, and an explicit `Host` header
/// since the transport layer has already forgotten the hostname by the time
/// this runs — it only ever saw the resolved address.
fn one_shot_request(host: &str, path: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
}

fn main() {
    let request = one_shot_request("example.com", "/status");
    assert!(request.contains("Host: example.com"));
    assert!(request.contains("Connection: close"));

    // Simulates sending the request and reading the response into a buffer
    // that reaches EOF only because the server was told to close.
    struct FakeConnection(&'static str);
    impl Read for FakeConnection {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.0.len().min(buf.len());
            buf[..n].copy_from_slice(self.0.as_bytes()[..n].as_ref());
            self.0 = &self.0[n..];
            Ok(n)
        }
    }
    let mut conn = FakeConnection("HTTP/1.1 200 OK\r\n\r\nok");
    let mut body = String::new();
    conn.read_to_string(&mut body).expect("the fake connection reaches EOF");
    assert_eq!(body, "HTTP/1.1 200 OK\r\n\r\nok");
}
```

## Keep-Alive Cases To Test

- a request built for a one-shot call carries both `Host` and
  `Connection: close`;
- a response read loop that reaches EOF does so because the connection was
  told to close, not because it happened to; a version built against a
  kept-alive server hangs, and that difference is the point of the test;
- an unresolved-hostname request (missing `Host`) is distinguishable in a
  test double from a resolved one, catching the case where only the
  connect-time address was used;
- reading exactly `Content-Length` bytes (rather than looping to EOF)
  succeeds against a connection that stays open after the body.

## See Also

- [api-outbound-target](api-outbound-target.md) - authorizing the resolved address this rule then labels with its hostname
- [async-http-client-reuse](async-http-client-reuse.md) - the opposite intentional case: a pooled client that keeps connections alive on purpose
- [type-text-decode-policy](type-text-decode-policy.md) - the wire-framing discipline (explicit bytes, not platform text) this protocol also depends on
- [api-tls-required](api-tls-required.md) - the same hostname is what TLS SNI and certificate validation depend on
- [async-tokio-runtime](async-tokio-runtime.md) - a blocking read that never reaches EOF stalls whatever thread it runs on
