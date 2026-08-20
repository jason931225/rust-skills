# api-fallible-self-return

> When a fallible method consumes `self`, hand the receiver back in the error

## Why It Matters

A method that takes `self` and returns `Result<Next, Error>` destroys its
receiver on the failing path. When the failure is recoverable — a wrong
password, a rejected frame, a transient handshake error — the caller is left
holding an error and nothing else: the socket, the buffer, or the half-built
value is gone, and the only way forward is to rebuild something that was
already working. The standard library treats this as an obligation, which is
why `String::from_utf8` returns the bytes inside `FromUtf8Error`,
`BufWriter::into_inner` returns the writer inside `IntoInnerError`, and
`Rc::try_unwrap` returns the original `Rc`.

## Bad

```rust
impl Connection<Connected> {
    // A wrong password consumes the socket. The caller cannot retry without
    // reconnecting, and nothing in the signature warned them.
    fn authenticate(self, password: &str) -> Result<Connection<Authed>, Error> {
        let session = do_auth(&self.socket, password)?;
        Ok(Connection { socket: self.socket, session })
    }
}
```

## Good

```rust
#[derive(Debug, PartialEq)]
pub enum AuthError {
    WrongPassword,
    Fatal,
}

#[derive(Debug)]
pub struct Connected {
    /// Stands in for a socket: expensive to obtain, impossible to rebuild
    /// from the error alone.
    handle: u32,
}

#[derive(Debug)]
pub struct Authenticated {
    handle: u32,
}

impl Connected {
    /// The receiver comes back with the error, so a recoverable failure costs
    /// an attempt rather than the connection.
    pub fn authenticate(self, password: &str) -> Result<Authenticated, (AuthError, Self)> {
        match password {
            "correct-horse" => Ok(Authenticated { handle: self.handle }),
            "" => Err((AuthError::Fatal, self)),
            _ => Err((AuthError::WrongPassword, self)),
        }
    }
}

fn main() {
    let connection = Connected { handle: 7 };

    // First attempt fails, and the connection survives to be retried.
    let connection = match connection.authenticate("guess") {
        Ok(_) => unreachable!("wrong password"),
        Err((error, recovered)) => {
            assert_eq!(error, AuthError::WrongPassword);
            recovered
        }
    };

    let authenticated = connection.authenticate("correct-horse").expect("retry succeeds");
    assert_eq!(authenticated.handle, 7, "the same connection, not a new one");
}
```

## When To Return The Receiver

- The test is whether the receiver is reconstructible. Returning a consumed
  `u32` back is noise; returning a connected socket is the difference between
  retrying and reconnecting.
- Prefer a dedicated error type that owns the receiver and exposes it through
  `into_inner`, the way `IntoInnerError` does, once the tuple gets unwieldy or
  the error crosses a public API.
- For an unrecoverable failure — the resource is already broken — consuming it
  is correct, and the signature says so.
- `&mut self` sidesteps the question entirely when the state machine does not
  need distinct types per state; typestate is what forces the choice.
- Document which errors are retryable, so the caller knows the recovered
  receiver is worth reusing.

## See Also

- [api-typestate](api-typestate.md) - consuming transitions are where this arises
- [err-canonical-struct](err-canonical-struct.md) - the error type that carries the receiver
- [conv-tryfrom-fallible](conv-tryfrom-fallible.md) - fallible conversions that reject a value
- [api-builder-pattern](api-builder-pattern.md) - builders chain by consuming `self` too
