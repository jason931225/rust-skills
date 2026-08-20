# api-error-schema

> Return errors in the same media type and documented schema as successes, including framework-generated ones

## Why It Matters

A client that parses JSON responses breaks the moment the framework answers
with its own HTML 404, a plain-text 405, or an empty 500 body — the transport
succeeded, the parse failed, and the error the caller finally reports is a
deserialization fault rather than the real problem. These defaults come from
routing, payload limits, and panic handlers, which are exactly the paths that
are never exercised by the handler tests. Making the error representation part
of the API contract keeps a failed request as machine-readable as a successful
one.

## Error Response Requirements

- Define one error body schema and document it alongside the success schemas.
- Emit it with the API's content type for every status the service can return,
  including 404, 405, 413, 415, 429, and 500.
- Override the framework's default error rendering rather than relying on
  handlers to be reached — routing and extractor failures never enter a
  handler.
- Include a stable machine-readable code and, where the caller can act, an
  actionable message. Keep internal detail out of both.
- Carry the correlation identifier in the body or a documented header so a
  reported failure can be found in telemetry.
- Version the error schema with the API. Adding a field is additive; changing
  a code's meaning is breaking.
- A malformed request body typically fails during extraction, before any
  handler runs, and the framework's default extractor-rejection path is a
  separate pipeline from a handler's own `Result` — overriding handler-level
  error mapping alone leaves extractor failures on the framework's default
  (often a non-JSON body and the wrong status). Configure the extractor's own
  rejection handling to match the rest of the schema.
- A framework's response-building trait commonly exposes more than one
  method that can produce a body (a primary render method plus a default
  sibling for a different error kind). Override every method that can emit a
  response, not just the one exercised by your own handler code — an
  unoverridden sibling still runs and clobbers the content type and payload.
- Distinguish "the request could not even be parsed" (typically 4xx, at the
  extraction boundary) from "the request parsed but named something that
  does not exist" (404, from application logic) from "the request parsed,
  matched something, and it is empty" (200 with an empty or null body). These
  are three different situations a client needs to tell apart, not one
  generic failure.

## Bad

```rust
async fn handler(Json(input): Json<Order>) -> Result<Json<Receipt>, StatusCode> {
    // A malformed body never reaches this handler: the extractor rejects it
    // with the framework's plain-text default, and the client's JSON parser
    // fails on "Failed to deserialize the JSON body"
    Ok(Json(place(input).await?))
}
```

## Good

```rust
#[derive(Debug, PartialEq)]
pub struct ApiError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
    pub correlation_id: String,
}

impl ApiError {
    /// The one place any failure becomes a response, whatever produced it.
    pub fn render(&self) -> (u16, &'static str, String) {
        (
            self.status,
            "application/json",
            format!(
                r#"{{"code":"{}","message":"{}","correlation_id":"{}"}}"#,
                self.code, self.message, self.correlation_id
            ),
        )
    }
}

/// Framework-generated failures are mapped through the same constructor as
/// domain failures, so no path can emit a foreign representation.
pub fn from_transport(status: u16, correlation_id: &str) -> ApiError {
    let (code, message) = match status {
        404 => ("not_found", "no route matches this request"),
        405 => ("method_not_allowed", "this route does not accept that method"),
        413 => ("payload_too_large", "the request body exceeds the limit"),
        415 => ("unsupported_media_type", "send application/json"),
        _ => ("internal", "the request could not be completed"),
    };
    ApiError {
        status,
        code,
        message: message.to_owned(),
        correlation_id: correlation_id.to_owned(),
    }
}

fn main() {
    let (status, content_type, body) = from_transport(404, "req-7").render();
    assert_eq!(status, 404);
    assert_eq!(content_type, "application/json");
    assert!(body.contains(r#""code":"not_found""#));
    assert!(body.contains("req-7"), "the caller can quote this in a report");

    // A 500 says nothing about internals.
    let (_, _, internal) = from_transport(500, "req-8").render();
    assert!(!internal.contains("panic") && !internal.contains("sql"));
}
```

## Error Paths To Verify

- an unmatched route, a wrong method, an oversized body, and a wrong content
  type all return the documented schema and content type;
- a panic in a handler returns the same schema, not an empty body;
- an extractor rejection is reported with the error code, not the framework's
  prose;
- the correlation identifier in the body matches the one in the request span;
- no error body contains SQL, paths, stack frames, or dependency messages.

## See Also

- [err-edge-mapping](err-edge-mapping.md) - choosing the status and what to disclose
- [api-extract-or-reject](api-extract-or-reject.md) - the rejections this rule has to represent
- [obs-request-correlation](obs-request-correlation.md) - where the identifier comes from
- [test-http-blackbox](test-http-blackbox.md) - assert it through the production router
- [api-resource-limits](api-resource-limits.md) - the limits that produce 413 and 429
