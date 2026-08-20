# api-health-probes

> Separate liveness from readiness, keep probes cheap, and never perform business side effects

## Why It Matters

An orchestrator uses probes to decide whether to restart a process or route
traffic to it. A probe that sends email, mutates the database, or waits on
every dependency can amplify an outage. Liveness answers whether the process
event loop is functioning, while readiness answers whether this instance
should receive new traffic; they are different failure policies.

## Bad

```rust
pub async fn health() -> Result<(), Error> {
    database.write_test_row().await?;
    mailer.send_test_message().await?;
    Ok(())
}
```

One dependency outage causes every replica to restart and the probe itself
creates load and side effects.

## Good

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Ready,
    NotReady,
}

pub fn liveness() -> Probe {
    Probe::Ready
}

pub fn readiness(startup_complete: bool, draining: bool) -> Probe {
    if startup_complete && !draining {
        Probe::Ready
    } else {
        Probe::NotReady
    }
}

fn main() {
    assert_eq!(liveness(), Probe::Ready);
    assert_eq!(readiness(true, false), Probe::Ready);
    assert_eq!(readiness(true, true), Probe::NotReady);
}
```

## Liveness And Readiness Requirements

- Liveness is local, bounded, allocation-light, and independent of remote
  dependencies. Failure requests a restart.
- Readiness becomes true only after required startup work and false before
  graceful shutdown. Failure removes the instance from routing without
  restarting it.
- Readiness may incorporate cached/circuit state for dependencies required to
  serve, but the probe itself does not synchronously fan out to every
  dependency on every poll.
- A separate diagnostic endpoint may report dependency state, but it must be
  authenticated where details are sensitive and must not drive a restart
  storm.
- Probe handlers have no business side effects and no per-request database
  writes.
- Tests assert the startup and draining transitions, not only the default
  healthy response.

## Folding Dependency Verdicts Into One Status

A readiness probe that consults several dependencies has to turn several
verdicts into one answer, and the two ways that usually goes wrong are a chain
of `if` statements whose precedence nobody wrote down, and a new dependency
that silently never reaches the aggregate.

Order the severities as a fieldless enum and fold with `max`. A derived `Ord`
on a fieldless enum ranks variants by **declaration order**, so writing them
least-to-most severe makes "the worst wins" the derived behaviour rather than
something to hand-maintain:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Health {
    Ok,
    Degraded,
    Down,
}

pub struct Verdict {
    pub name: &'static str,
    pub health: Health,
}

/// Every source is listed here, so adding one is a visible edit to this array
/// rather than a forgotten branch somewhere else.
pub fn readiness(sources: &[Verdict]) -> Health {
    sources
        .iter()
        .map(|source| source.health)
        .max()
        .unwrap_or(Health::Down)
}

fn main() {
    let sources = [
        Verdict { name: "db", health: Health::Ok },
        Verdict { name: "cache", health: Health::Degraded },
        Verdict { name: "queue", health: Health::Ok },
    ];
    assert_eq!(readiness(&sources), Health::Degraded);
    assert!(Health::Ok < Health::Degraded && Health::Degraded < Health::Down);

    // An empty source list is not healthy by default: nothing was checked.
    assert_eq!(readiness(&[]), Health::Down);
}
```

Two details carry their weight. The empty case is a decision, not a fallthrough
— `max()` on no sources is `None`, and answering `Ok` there means a probe that
checked nothing reports ready. And keep the enumeration in one array so a
reviewer can see the whole set; a fold over a list assembled across several
modules has the same silent-omission problem as the `if` chain it replaced.

## See Also

- [test-http-blackbox](test-http-blackbox.md) - verify probe routes through the production server
- [async-cancellation-token](async-cancellation-token.md) - mark readiness false before cancellation
- [obs-named-events](obs-named-events.md) - emit state transitions instead of logging every successful probe
- [err-result-over-panic](err-result-over-panic.md) - report startup failures without probe-triggered panics
