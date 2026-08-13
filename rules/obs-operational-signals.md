# obs-operational-signals

> Define service-level signals and failure telemetry before production traffic

## Why It Matters

Tests enumerate expected behavior but cannot prove that an integrated service
will never fail in production. Operators need bounded-cardinality signals for
traffic, errors, latency, saturation, and named failure modes before a release
receives traffic. Choose signals from an explicit service objective so
telemetry cost is bounded and missing evidence fails the production-readiness
review.

## Bad

```rust
async fn handle(request: Request) -> Result<Response, Error> {
    process(request).await
}

struct Request;
struct Response;
struct Error;

async fn process(_request: Request) -> Result<Response, Error> {
    Ok(Response)
}
```

The caller sees a result, but operators cannot measure latency, error rate, or
which named failure occurred.

## Good

```rust
use std::time::{Duration, Instant};
use tracing::{error, info, instrument};

trait ServiceMetrics {
    fn request_finished(&self, elapsed: Duration, outcome: &'static str);
}

#[instrument(skip_all, fields(operation = "request.handle"))]
async fn handle(
    request: Request,
    metrics: &impl ServiceMetrics,
) -> Result<Response, Error> {
    let started = Instant::now();
    let result = process(request).await;
    let outcome = if result.is_ok() { "ok" } else { "error" };
    metrics.request_finished(started.elapsed(), outcome);

    match &result {
        Ok(_) => info!(name: "request.complete", outcome),
        Err(error) => error!(
            name: "request.failed",
            error = ?error,
            outcome
        ),
    }
    result
}

#[derive(Debug)]
struct Request;
#[derive(Debug)]
struct Response;
#[derive(Debug)]
struct Error;

async fn process(_request: Request) -> Result<Response, Error> {
    Ok(Response)
}
```

## Production Contract

- Define availability and latency objectives, their measurement window, and
  the exact success/error classification before choosing dashboards.
- Emit traffic, error, latency, and saturation signals at stable boundaries;
  add named product-specific failure counters only when they change response.
- Keep label and field cardinality bounded. User IDs, request IDs, URLs, and
  error strings belong in correlated traces or logs, not metric dimensions.
- Alert on objective burn or actionable saturation, not every individual
  error. Include the owning service, failure stage, and rollback/runbook link.
- Test that success, rejection, dependency timeout, overload, and cancellation
  update the expected signal and preserve correlation context.

## See Also

- [obs-request-correlation](obs-request-correlation.md) - correlate a user report without metric-cardinality explosions
- [obs-named-events](obs-named-events.md) - keep event identity stable across releases
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - exclude secrets and personal data
- [test-observable-coverage](test-observable-coverage.md) - test observable success and failure contracts
