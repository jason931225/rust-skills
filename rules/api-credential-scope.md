# api-credential-scope

> Bind every stored credential to the origin it was issued for, and re-check that binding before sending it

## Why It Matters

A saved token, cookie jar, or authorization header belongs to one origin —
scheme, host, and port together. Code that keeps credentials in a map keyed by
name, or attaches "the session" to whatever request is going out, will
eventually send a production token to a staging host, or follow a redirect and
hand a bearer token to whoever controls the new location. The leak is silent:
the request succeeds, and the credential is now in someone else's log.

## Credential Binding Requirements

- Store credentials keyed by the full origin, not by a nickname or a bare
  hostname. `https://api.example.com` and `http://api.example.com:8080` are
  different origins.
- Re-check the binding at attach time, not only when the credential was loaded.
- Drop credentials on cross-origin redirects; re-attach only if the new origin
  has its own. The same rule applies to `Authorization` headers and cookies.
- Never widen a scope to make a call work — a token that does not match the
  target is a configuration error, not a prompt to relax the check.
- Keep the value in a redacting type, and the file owner-only.
- Expire and refresh explicitly; a credential with no recorded lifetime cannot
  be rotated safely.

## Bad

```rust
fn send(request: Request, session: &Session) -> Result<Response, Error> {
    // "the session" is attached to whatever host this request happens to target,
    // and survives a redirect to another origin
    request.header(AUTHORIZATION, session.token.clone()).send()
}
```

## Good

```rust
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Origin {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl Origin {
    pub fn new(scheme: &str, host: &str, port: u16) -> Self {
        Self { scheme: scheme.to_owned(), host: host.to_owned(), port }
    }
}

pub struct Credential {
    origin: Origin,
    // In production this is a redacting secret type.
    token: String,
}

impl Credential {
    pub fn new(origin: Origin, token: &str) -> Self {
        Self { origin, token: token.to_owned() }
    }

    /// The only way to read the token: the target must match the origin the
    /// credential was issued for.
    pub fn for_target(&self, target: &Origin) -> Option<&str> {
        (self.origin == *target).then_some(self.token.as_str())
    }
}

fn main() {
    let issued = Origin::new("https", "api.example.com", 443);
    let credential = Credential::new(issued.clone(), "t-secret");

    assert_eq!(credential.for_target(&issued), Some("t-secret"));

    // Every one of these is a different origin, and none of them gets the token.
    for target in [
        Origin::new("http", "api.example.com", 443),   // downgraded scheme
        Origin::new("https", "api.example.com", 8443), // different port
        Origin::new("https", "evil.example.com", 443), // redirect target
        Origin::new("https", "api.example.com.evil.test", 443), // suffix trick
    ] {
        assert_eq!(credential.for_target(&target), None, "{target:?} must not receive it");
    }
}
```

## Cases To Pin In Tests

- a token is withheld from a different scheme, port, host, and from a hostname
  that merely has the right suffix;
- a redirect to another origin drops the `Authorization` header;
- a redirect back to the original origin re-attaches it only from storage, not
  from the in-flight request;
- a credential file is created owner-only and its token never appears in logs;
- an expired credential is refused rather than sent and rejected upstream.

## See Also

- [type-secret-material](type-secret-material.md) - how the token itself is carried
- [proj-secret-file-mode](proj-secret-file-mode.md) - how the store is created on disk
- [api-outbound-target](api-outbound-target.md) - authorizing the target of each hop
- [api-session-security](api-session-security.md) - server-side session handling
- [api-tls-required](api-tls-required.md) - why a scheme downgrade voids the binding
