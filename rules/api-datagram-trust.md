# api-datagram-trust

> On connectionless transports, verify the sender and match replies with an unpredictable identifier

## Why It Matters

A UDP datagram carries no connection state, so its source address is whatever
the sender wrote there. An off-path attacker who knows a request is in flight
can send a forged reply, and if it arrives first it wins — this is how DNS
cache poisoning works. Two cheap checks remove most of that exposure: discard
datagrams from anyone but the expected peer, and give each request an
identifier an attacker cannot guess, so a blind forgery has to hit both the
address and the identifier.

## Datagram Acceptance Requirements

- Read the source address with the datagram and compare it against the peer the
  request was sent to; discard anything else without processing it.
- Give each outstanding request a random identifier from a CSPRNG, and reject
  replies whose identifier does not match a request still awaiting an answer.
- Retire the identifier once the reply is accepted, so a late duplicate cannot
  be replayed into the next exchange.
- Bound the wait and the number of outstanding requests; a reply that never
  arrives must expire rather than accumulate.
- Parse the payload only after both checks pass, so malformed forgeries never
  reach the decoder.
- None of this is authentication. Where the content matters, use an
  authenticated transport — the checks above only raise the cost of blind
  forgery.

## Bad

```rust
fn query(socket: &UdpSocket, server: SocketAddr) -> Result<Answer, Error> {
    socket.send_to(&request, server)?;
    let mut buffer = [0u8; 512];
    // Source ignored, and the request carries a predictable sequential id:
    // whoever answers first is believed
    let (len, _from) = socket.recv_from(&mut buffer)?;
    Answer::parse(&buffer[..len])
}
```

## Good

```rust
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, PartialEq)]
pub enum Reject {
    WrongPeer,
    UnknownId,
}

pub struct Pending {
    peer: SocketAddr,
    /// Unpredictable per request, from a CSPRNG in production.
    id: u16,
}

impl Pending {
    /// Both checks happen before the payload is parsed.
    pub fn accept<'a>(
        &self,
        from: SocketAddr,
        datagram: &'a [u8],
    ) -> Result<&'a [u8], Reject> {
        if from != self.peer {
            return Err(Reject::WrongPeer);
        }
        let (id, payload) = datagram.split_at(2);
        let id = u16::from_be_bytes([id[0], id[1]]);
        if id != self.id {
            return Err(Reject::UnknownId);
        }
        Ok(payload)
    }
}

fn main() {
    let server = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 53)), 53);
    let pending = Pending { peer: server, id: 0xa17f };

    let mut reply = 0xa17f_u16.to_be_bytes().to_vec();
    reply.extend_from_slice(b"answer");
    assert_eq!(pending.accept(server, &reply), Ok(&b"answer"[..]));

    // Right identifier, wrong sender.
    let elsewhere = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7)), 53);
    assert_eq!(pending.accept(elsewhere, &reply), Err(Reject::WrongPeer));

    // Right sender, guessed identifier.
    let mut forged = 0x0001_u16.to_be_bytes().to_vec();
    forged.extend_from_slice(b"poison");
    assert_eq!(pending.accept(server, &forged), Err(Reject::UnknownId));
}
```

## Forgery Cases To Test

- a datagram from an unexpected address is discarded without parsing;
- a reply whose identifier does not match any outstanding request is discarded;
- a duplicate of an accepted reply is not processed twice;
- a request that receives no reply expires and frees its identifier;
- identifiers are drawn from a CSPRNG, not a counter, and do not repeat within
  the outstanding window.

## See Also

- [api-outbound-target](api-outbound-target.md) - authorizing the peer before sending
- [api-tls-required](api-tls-required.md) - authentication these checks cannot provide
- [api-resource-limits](api-resource-limits.md) - bounding outstanding requests and wait time
- [api-extract-or-reject](api-extract-or-reject.md) - parse only after the datagram is accepted
- [api-crypto-primitives](api-crypto-primitives.md) - where the unpredictable identifier comes from
