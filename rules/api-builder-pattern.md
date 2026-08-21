# api-builder-pattern

> Use Builder pattern for complex construction

## Why It Matters

When a type has many optional parameters or complex initialization, the Builder pattern provides a clear, flexible API. It avoids constructors with many parameters (which are error-prone) and makes the code self-documenting.

## Bad

```rust
// Constructor with many parameters - hard to read, easy to get wrong
let client = Client::new(
    "https://api.example.com",  // Which is which?
    30,                          // Timeout? Retries?
    true,                        // What does this mean?
    None,
    Some("auth_token"),
    false,
);

// Or many Option fields
struct Client {
    url: String,
    timeout: Option<Duration>,
    retries: Option<u32>,
    // ... 10 more optional fields
}
```

## Good

```rust
#[derive(Default)]
#[must_use = "builders do nothing unless you call build()"]
pub struct ClientBuilder {
    base_url: Option<String>,
    timeout: Option<Duration>,
    max_retries: u32,
    auth_token: Option<String>,
}

impl Client {
    pub fn builder(base_url: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            base_url: Some(base_url.into()),
            ..ClientBuilder::default()
        }
    }
}

impl ClientBuilder {
    /// Sets the base URL for all requests.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }
    
    /// Sets the request timeout. Default is 30 seconds.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    
    /// Sets the maximum number of retries. Default is 0 (`#[derive(Default)]`); set it explicitly if you want 3.
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }
    
    /// Sets the authentication token.
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }
    
    /// Builds the client with the configured options.
    pub fn build(self) -> Result<Client, BuilderError> {
        let base_url = self.base_url
            .ok_or(BuilderError::MissingBaseUrl)?;
        
        Ok(Client {
            base_url,
            timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
            max_retries: self.max_retries,
            auth_token: self.auth_token,
        })
    }
}

// Usage - clear and self-documenting
let client = Client::builder("https://api.example.com")
    .timeout(Duration::from_secs(10))
    .max_retries(5)
    .auth_token("secret")
    .build()?;
```

## Builder Variations

```rust
// 1. Infallible builder (build() returns T, not Result)
impl WidgetBuilder {
    pub fn build(self) -> Widget {
        Widget {
            color: self.color.unwrap_or(Color::Black),
            size: self.size.unwrap_or(Size::Medium),
        }
    }
}

// 2. Typestate builder (compile-time required field checking)
pub struct ClientBuilder<Url> {
    url: Url,
    timeout: Option<Duration>,
}

pub struct NoUrl;
pub struct HasUrl(String);

impl ClientBuilder<NoUrl> {
    fn new() -> Self {
        Self { url: NoUrl, timeout: None }
    }
    
    pub fn url(self, url: String) -> ClientBuilder<HasUrl> {
        ClientBuilder { url: HasUrl(url), timeout: self.timeout }
    }
}

impl ClientBuilder<HasUrl> {
    pub fn build(self) -> Client {
        // url is guaranteed to be set
        Client { url: self.url.0, timeout: self.timeout }
    }
}

// 3. Consuming vs borrowing (consuming is more common)
// Consuming (takes self)
pub fn timeout(mut self, t: Duration) -> Self { ... }

// Borrowing (takes &mut self, allows reuse)
pub fn timeout(&mut self, t: Duration) -> &mut Self { ... }
```

## Evidence from reqwest

```rust
// https://github.com/seanmonstar/reqwest/blob/master/src/async_impl/client.rs

#[must_use]
pub struct ClientBuilder {
    config: Config,
}

impl Client {
    // The real body delegates rather than building the struct inline; the
    // point here is that `Client::builder()` is the entry point, not
    // `ClientBuilder`.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }
}

impl ClientBuilder {
    pub fn new() -> ClientBuilder {
        ClientBuilder { config: Config::default() }
    }

    pub fn timeout(mut self, timeout: Duration) -> ClientBuilder {
        self.config.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Result<Client, Error> {
        // Validation and construction
    }
}
```

## Attributes That Carry The Contract

```rust
#[derive(Default)]  // Enables MyBuilder::default()
#[must_use = "builders do nothing unless you call build()"]
pub struct MyBuilder { ... }

impl MyBuilder {
    #[must_use]  // Each method should have this
    pub fn option(mut self, value: T) -> Self { ... }
}
```

## Construction Contract

- Offer `Foo::builder(required...)`; do not make `FooBuilder::new()` the primary public entry point.
- Pass required dependencies when the builder is created. Builder setters are for optional or permutation-heavy configuration.
- Keep setters infallible and name them after the field (`timeout`, not `set_timeout`). Validate interacting options once in `build()`.
- Two optional values do not automatically justify a builder; inherent `new` / `with_*` constructors may be clearer.
- When required arguments themselves form semantic groups, use cascaded helper types rather than hiding them in builder state.

## Where Failure Belongs In A Fluent Chain

A chain reads as one expression, which is exactly why the steps inside it
should not be places things happen. Keep every setter a pure assignment — no
file reads, no DNS lookups, no connections, no validation that can fail — and
concentrate fallibility in the terminal call:

```rust
pub struct Config {
    endpoint: String,
    retries: u32,
}

impl Config {
    /// The entry point lives on the built type, not on the builder.
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ConfigBuilder {
    endpoint: Option<String>,
    retries: Option<u32>,
}

impl ConfigBuilder {
    /// Setters record intent and cannot fail.
    #[must_use]
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    #[must_use]
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = Some(retries);
        self
    }

    /// One place where interacting options are checked and one error type.
    pub fn build(self) -> Result<Config, &'static str> {
        let endpoint = self.endpoint.ok_or("endpoint is required")?;
        if endpoint.is_empty() {
            return Err("endpoint must not be empty");
        }
        Ok(Config { endpoint, retries: self.retries.unwrap_or(3) })
    }
}

fn main() {
    let config = Config::builder()
        .endpoint("https://example.invalid")
        .retries(5)
        .build()
        .expect("valid configuration");
    assert_eq!(config.retries, 5);

    assert!(Config::builder().build().is_err(), "missing endpoint is caught at build");
}
```

The reason is diagnostic rather than aesthetic. When a setter can fail, the
chain either returns `Result` at every step — so each `?` hides which link
broke — or it defers the failure and reports it later against a call that looks
unrelated. One fallible terminal call gives one place to attach context, and
the option interactions are validated together, where the relationship between
them is visible.

The same applies to any fluent API, not only builders: if a step performs I/O,
it is not a step in an expression, it is a statement that should be written as
one.

## See Also

- [api-builder-must-use](api-builder-must-use.md) - Add #[must_use] to builders
- [api-typestate](api-typestate.md) - Compile-time state machines
- [api-impl-into](api-impl-into.md) - Accept impl Into for flexibility
- [name-no-weasel](name-no-weasel.md) - Call it a Builder, not a Factory
- [api-init-cascaded](api-init-cascaded.md) - group long required argument lists before reaching for optional builder state
