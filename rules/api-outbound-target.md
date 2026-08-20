# api-outbound-target

> Resolve and authorize every caller-influenced outbound request target before connecting

## Why It Matters

A server that fetches a URL supplied by a caller can be aimed inward. Cloud
instance-metadata endpoints, admin dashboards, and databases usually trust the
network position rather than a credential, so one unvalidated fetch can return
provisioning credentials or internal data. Checking the hostname string is not
enough: DNS can answer with a private address, a redirect can retarget the
request, and the address that gets connected to may not be the one that was
checked.

## Target Authorization Requirements

- Prefer an allowlist of exact hosts or a fixed set of upstreams. Free-form
  URLs are the fallback, not the default.
- Require an expected scheme and port; reject anything else, including
  `file:`, `gopher:`, and other non-HTTP schemes.
- Resolve the host and authorize the resolved IP addresses — loopback, private,
  link-local (including the `169.254.0.0/16` metadata range), unique-local,
  carrier-grade NAT, multicast, reserved, and unspecified addresses are denied.
- Re-authorize on every redirect hop, or disable redirect following entirely.
- Bound the request: connect and total deadlines, a response size cap, and a
  redirect-hop limit.
- Do not return the upstream body, headers, or status verbatim to the caller;
  return the fields the feature actually needs.

## Bad

```rust
async fn fetch(url: &str) -> Result<String, reqwest::Error> {
    // The caller chooses the target; http://169.254.169.254/ returns cloud
    // credentials, and the body is handed straight back
    reqwest::get(url).await?.text().await
}
```

## Good

```rust
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, PartialEq)]
pub enum TargetError {
    Scheme,
    HostNotAllowed,
    AddressNotAllowed,
}

fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.octets()[0] == 0
                // 100.64.0.0/10 carrier-grade NAT
                || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
                // 240.0.0.0/4 reserved
                || v4.octets()[0] >= 240)
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7 unique local and fe80::/10 link local
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

/// Authorizes one hop: the scheme, the host, and every address it resolved to.
pub fn authorize_hop(
    scheme: &str,
    host: &str,
    allowed_hosts: &[&str],
    resolved: &[IpAddr],
) -> Result<(), TargetError> {
    if scheme != "https" {
        return Err(TargetError::Scheme);
    }
    if !allowed_hosts.contains(&host) {
        return Err(TargetError::HostNotAllowed);
    }
    if resolved.is_empty() || !resolved.iter().copied().all(is_public) {
        return Err(TargetError::AddressNotAllowed);
    }
    Ok(())
}

fn main() {
    let allowed = ["api.partner.example"];
    let public = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    let metadata = IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254));
    let local = IpAddr::V6(Ipv6Addr::LOCALHOST);

    assert_eq!(authorize_hop("https", "api.partner.example", &allowed, &[public]), Ok(()));
    assert_eq!(
        authorize_hop("https", "api.partner.example", &allowed, &[metadata]),
        Err(TargetError::AddressNotAllowed)
    );
    assert_eq!(
        authorize_hop("https", "api.partner.example", &allowed, &[public, local]),
        Err(TargetError::AddressNotAllowed)
    );
    assert_eq!(authorize_hop("http", "api.partner.example", &allowed, &[public]), Err(TargetError::Scheme));
    assert_eq!(authorize_hop("https", "internal.corp", &allowed, &[public]), Err(TargetError::HostNotAllowed));
}
```

Authorizing the addresses and then connecting to a fresh resolution leaves a
DNS-rebinding window. Close it by connecting to an address that was authorized
in this decision rather than re-resolving the name.

## Targets The Tests Must Refuse

- a loopback, private, link-local, and metadata address target is refused;
- a target that resolves to one public and one private address is refused;
- a redirect from an allowed host to an internal host is refused;
- a non-HTTPS scheme is refused;
- an oversized or slow upstream response is cut off at the documented bound;
- refusal responses do not disclose the resolved address or upstream error.

## See Also

- [api-authz-fail-closed](api-authz-fail-closed.md) - deny unless the decision succeeds
- [async-http-client-reuse](async-http-client-reuse.md) - one configured client, deadlines on every call
- [async-bounded-dependency](async-bounded-dependency.md) - bound admission and failure handling for outbound calls
- [api-resource-limits](api-resource-limits.md) - cap response bytes and time
- [err-edge-mapping](err-edge-mapping.md) - map refusals to safe responses
