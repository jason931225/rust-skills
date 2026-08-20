# api-typestate

> Use typestate pattern to encode state machine invariants in the type system

## Why It Matters

State machines with runtime state checks ("are we connected?", "is the transaction started?") can have invalid transitions. The typestate pattern uses different types for each state, making invalid state transitions compile errors. The compiler enforces your state machine.

## Bad

```rust
struct Connection {
    state: ConnectionState,
    socket: Option<TcpStream>,
}

enum ConnectionState {
    Disconnected,
    Connected,
    Authenticated,
}

impl Connection {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Runtime check - can fail if called in wrong state
        if self.state != ConnectionState::Authenticated {
            return Err(Error::NotAuthenticated);
        }
        self.socket.as_mut().unwrap().write_all(data)?;
        Ok(())
    }
    
    fn authenticate(&mut self, password: &str) -> Result<(), Error> {
        // Runtime check - can fail
        if self.state != ConnectionState::Connected {
            return Err(Error::NotConnected);
        }
        // ...
    }
}

// Bug: forgot to authenticate
let mut conn = Connection::new();
conn.connect()?;
conn.send(b"data")?;  // Runtime error: NotAuthenticated
```

## Good

```rust
// Different types for each state
struct Disconnected;
struct Connected { socket: TcpStream }
struct Authenticated { socket: TcpStream, session: Session }

struct Connection<State> {
    state: State,
}

impl Connection<Disconnected> {
    fn new() -> Self {
        Connection { state: Disconnected }
    }
    
    fn connect(self, addr: &str) -> Result<Connection<Connected>, Error> {
        let socket = TcpStream::connect(addr)?;
        Ok(Connection { state: Connected { socket } })
    }
}

impl Connection<Connected> {
    // A wrong password is recoverable, so the error hands the connection back
    // rather than destroying a live socket the caller cannot rebuild.
    fn authenticate(self, password: &str) -> Result<Connection<Authenticated>, (Error, Self)> {
        match do_auth(&self.state.socket, password) {
            Ok(session) => Ok(Connection {
                state: Authenticated { socket: self.state.socket, session },
            }),
            Err(error) => Err((error, self)),
        }
    }
}

impl Connection<Authenticated> {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // No runtime check needed - type guarantees we're authenticated
        self.state.socket.write_all(data)?;
        Ok(())
    }
}

// Bug: forgot to authenticate
let conn = Connection::new();
let conn = conn.connect("server:8080")?;
conn.send(b"data");  // Compile error! send() not available on Connection<Connected>

// Correct usage
let conn = Connection::new();
let conn = conn.connect("server:8080")?;
let mut conn = conn.authenticate("secret")?;
conn.send(b"data")?;  // Works - type is Connection<Authenticated>
```

## Builder Typestate

```rust
// Enforce required fields via typestate
struct BuilderNoUrl;
struct BuilderWithUrl { url: String }

struct RequestBuilder<State> {
    state: State,
    timeout: Option<Duration>,
}

impl Request {
    fn builder() -> RequestBuilder<BuilderNoUrl> {
        RequestBuilder {
            state: BuilderNoUrl,
            timeout: None,
        }
    }
}

impl RequestBuilder<BuilderNoUrl> {
    fn url(self, url: &str) -> RequestBuilder<BuilderWithUrl> {
        RequestBuilder {
            state: BuilderWithUrl { url: url.to_string() },
            timeout: self.timeout,
        }
    }
}

impl RequestBuilder<BuilderWithUrl> {
    fn timeout(mut self, t: Duration) -> Self {
        self.timeout = Some(t);
        self
    }
    
    // Only available once URL is set
    fn build(self) -> Request {
        Request {
            url: self.state.url,
            timeout: self.timeout,
        }
    }
}

// Compile error: build() not available
let bad = Request::builder().build();

// Correct: must set URL first
let good = Request::builder()
    .url("https://example.com")
    .timeout(Duration::from_secs(30))
    .build();
```

## Independent Required Fields

The single-state-parameter builder above forces an order: `url` must be set
before `timeout` becomes reachable, even though nothing about the domain
requires that order. When two or more fields are independently required, a
linear chain either forces an arbitrary order or needs a state enum whose
size grows exponentially in the number of fields. Give each independent
requirement its own type parameter instead, so setters commute and `build`
exists only once every parameter has reached its `Set` form:

```rust
struct Missing;
struct Set<T>(T);

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
    fn mnemonic(self, value: String) -> DerBuilder<Set<String>, FC> {
        DerBuilder { mnemonic: Set(value), fault_class: self.fault_class }
    }
}

impl<M> DerBuilder<M, Missing> {
    fn fault_class(self, value: u8) -> DerBuilder<M, Set<u8>> {
        DerBuilder { mnemonic: self.mnemonic, fault_class: Set(value) }
    }
}

// `finish` exists only when both parameters have reached `Set<_>` — in
// either setter order, and it does not exist at all with one still Missing.
impl DerBuilder<Set<String>, Set<u8>> {
    fn finish(self) -> (String, u8) {
        (self.mnemonic.0, self.fault_class.0)
    }
}

fn main() {
    let a = DerBuilder::new().mnemonic("E101".into()).fault_class(2).finish();
    let b = DerBuilder::new().fault_class(2).mnemonic("E101".into()).finish();
    assert_eq!(a, b);
    // `DerBuilder::new().mnemonic("E101".into()).finish()` does not compile:
    // `fault_class` is still `Missing`, and no `finish` exists for that type.
}
```

The same completeness requirement applies when a builder gathers required
data from more than one optional source (a primary reader, then a fallback):
every branch of the `match`/`if let` that supplies the data has to produce
the same complete set of `Set<_>` markers, or `build`/`finish` will not
exist for the branch that took a shortcut. Do not let one branch skip a
field "because it usually has a good default" — make the default explicit
in that branch's own `Set(default)`, not by omitting the transition.

## `Drop` Cannot Be Specialized Per State

A single generic type cannot implement `Drop` only for one of its typestate
parameters — `impl Drop for Lock<Locked>` alongside no impl for
`Lock<Unlocked>` is `E0366: implementations of Drop must be unconditional`.
When only one state needs cleanup on drop (releasing a hardware lock,
unmapping memory, closing a session), either give that state its own,
separate named type with its own `Drop` impl, or keep one generic type and
match on an inner enum inside a single unconditional `Drop for Lock<S>`.
Do not follow "just implement `Drop`" advice by replacing an explicit
`close()`/`release()` typestate transition with a destructor for I/O-ful or
fallible release — that trades a caller-visible failure and ordering
contract for the weaker guarantees `Drop` provides ([async-explicit-close](async-explicit-close.md)).

## Transaction Example

```rust
struct NotStarted;
struct InProgress { tx_id: u64 }
struct Committed;

struct Transaction<State> {
    conn: Connection,
    state: State,
}

impl Transaction<NotStarted> {
    fn begin(conn: Connection) -> Result<Transaction<InProgress>, Error> {
        let tx_id = conn.execute("BEGIN")?;
        Ok(Transaction {
            conn,
            state: InProgress { tx_id },
        })
    }
}

impl Transaction<InProgress> {
    fn execute(&mut self, sql: &str) -> Result<(), Error> {
        self.conn.execute(sql)
    }
    
    fn commit(self) -> Result<Transaction<Committed>, Error> {
        self.conn.execute("COMMIT")?;
        Ok(Transaction {
            conn: self.conn,
            state: Committed,
        })
    }
    
    fn rollback(self) -> Connection {
        let _ = self.conn.execute("ROLLBACK");
        self.conn
    }
}
```

## See Also

- [api-builder-pattern](./api-builder-pattern.md) - Basic builder pattern
- [api-parse-dont-validate](./api-parse-dont-validate.md) - Type-driven invariants
- [api-sealed-trait](./api-sealed-trait.md) - Restricting trait implementations
- [api-fallible-self-return](./api-fallible-self-return.md) - hand the receiver back when a consuming transition fails
