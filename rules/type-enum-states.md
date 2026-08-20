# type-enum-states

> Use enums for mutually exclusive states

## Why It Matters

When a value can be in exactly one of several states, an enum can make
cross-state field combinations unrepresentable. An exhaustive match without a
wildcard lets the compiler report a newly added variant. A `_` arm, `if let`,
interior mutation, or invalid transition method can still hide states, so the
API must preserve the enum's invariant.

## Bad

```rust
struct Connection {
    is_connected: bool,
    is_authenticated: bool,
    is_disconnected: bool,  // Can all three be true? False?
    socket: Option<TcpStream>,
    credentials: Option<Credentials>,
}

// Possible invalid states:
// - is_connected && is_disconnected (contradiction)
// - is_authenticated && !is_connected (impossible)
// - socket is None but is_connected is true (inconsistent)
```

## Good

```rust
enum ConnectionState {
    Disconnected,
    Connecting { address: SocketAddr },
    Connected { socket: TcpStream },
    Authenticated { socket: TcpStream, session: Session },
    Failed { error: ConnectionError },
}

struct Connection {
    state: ConnectionState,
}

// Impossible states are unrepresentable
// Each state has exactly the data it needs
```

## Pattern Matching Ensures Completeness

```rust
fn handle_connection(conn: &Connection) {
    match &conn.state {
        ConnectionState::Disconnected => {
            println!("Not connected");
        }
        ConnectionState::Connecting { address } => {
            println!("Connecting to {}", address);
        }
        ConnectionState::Connected { socket } => {
            println!("Connected, not authenticated");
        }
        ConnectionState::Authenticated { socket, session } => {
            println!("Authenticated as {}", session.user);
        }
        ConnectionState::Failed { error } => {
            println!("Failed: {}", error);
        }
    }
    // Compiler error if any state is missing
}
```

## State Transitions

```rust
impl Connection {
    fn connect(&mut self, addr: SocketAddr) -> Result<(), Error> {
        match &self.state {
            ConnectionState::Disconnected => {
                self.state = ConnectionState::Connecting { address: addr };
                Ok(())
            }
            _ => Err(Error::AlreadyConnected),
        }
    }
    
    fn on_connected(&mut self, socket: TcpStream) {
        if let ConnectionState::Connecting { .. } = &self.state {
            self.state = ConnectionState::Connected { socket };
        }
    }
    
    fn authenticate(&mut self, creds: Credentials) -> Result<(), Error> {
        // Do the fallible work while borrowing the current socket. A failure
        // leaves the original Connected state intact.
        let session = match &self.state {
            ConnectionState::Connected { socket } => perform_auth(socket, creds)?,
            _ => return Err(Error::NotConnected),
        };

        let ConnectionState::Connected { socket } =
            std::mem::replace(&mut self.state, ConnectionState::Disconnected)
        else {
            unreachable!("state was checked without an intervening mutation");
        };
        self.state = ConnectionState::Authenticated { socket, session };
        Ok(())
    }
}
```

## Result and Option as State Enums

```rust
// Option<T> is an enum for "might not exist"
enum Option<T> {
    Some(T),
    None,
}

// Result<T, E> is an enum for "might have failed"
enum Result<T, E> {
    Ok(T),
    Err(E),
}

// Use these instead of nullable/sentinel values
fn find_user(id: u64) -> Option<User> { ... }
fn parse_config(s: &str) -> Result<Config, ParseError> { ... }
```

## An Enum Instead Of A Group Of Integer Constants

A set of related `const` values is the same information with none of the
checking. Nothing stops an unlisted value from reaching a function that expects
one of them, and nothing tells a reader that the set is meant to be closed:

```rust
// A value outside the intended set is an ordinary u8; nothing rejects it.
pub const BLACK: u8 = 0x0;
pub const BLUE: u8 = 0x1;
pub const CYAN: u8 = 0x3;

pub fn set_colour_loose(_code: u8) {}
```

An enum makes the set closed and the conversion one-directional: every value
that reaches the function is one somebody declared, and adding a variant makes
exhaustive matches fail until they account for it.

```rust
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Colour {
    Black,
    Blue,
    Cyan,
}

pub fn set_colour(colour: Colour) -> u8 {
    colour as u8
}

fn main() {
    assert_eq!(set_colour(Colour::Cyan), 2);
    // `set_colour(7)` does not compile: there is no `Colour` with that value.
}
```

## Pinning Discriminants Only When Something Outside Demands Them

When the numbers themselves are part of a contract — a hardware register, a
wire format, an FFI enum — the compiler's freedom to choose discriminants is
the problem, and `#[repr(uN)]` removes it:

```rust
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum VgaColour {
    Black = 0x0,
    Blue = 0x1,
    Red = 0x4,
    Yellow = 0xE,
}

fn main() {
    // The values are now the contract, not an implementation detail.
    assert_eq!(VgaColour::Red as u8, 0x4);
    assert_eq!(VgaColour::Yellow as u8, 0xE);
}
```

What this costs is narrower than it is usually described. `#[repr(u8)]` does
**not** switch off niche optimisation: for a fieldless enum using a handful of
the 256 available values, `size_of::<Option<VgaColour>>()` is still 1, and even
`Option<Option<_>>` stays at 1, because unused values remain available as
niches. The niche disappears when the *values* run out, not because `repr` was
written.

The real cost is width. `#[repr(uN)]` fixes the tag at `N`, so `#[repr(u32)]`
on a two-variant enum is four bytes where the compiler would have used one.
Choose the width the external contract actually specifies, and leave the
attribute off entirely when no such contract exists — an unannotated enum is
free to be smaller, and nothing outside the program can tell.

## See Also

- [api-typestate](./api-typestate.md) - Type-level state machines
- [api-non-exhaustive](./api-non-exhaustive.md) - Forward-compatible enums
- [type-option-nullable](./type-option-nullable.md) - Option for optional values
- [pat-exhaustive-enum](./pat-exhaustive-enum.md) - Match owned enums exhaustively
- [serde-enum-representation](./serde-enum-representation.md) - Choose enum wire tagging
