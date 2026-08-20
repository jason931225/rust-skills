# api-update-signature

> Verify a signature over every self-update payload before installing or executing it, using a key the running binary did not just download

## Why It Matters

A process that fetches and installs its own next version is, from an
attacker's position, the easiest remote-code-execution primitive available:
compromise the distribution channel once — a CDN, an S3 bucket, DNS, a
compromised release pipeline — and every instance that checks in installs
whatever was placed there, no exploit required. TLS on the download connection
authenticates the *channel*, not the *artifact*; a legitimate certificate
still serves attacker-controlled bytes if the origin itself is compromised.
The update path has to authenticate the payload independently of how it
arrived, with a key that was never transmitted alongside anything the update
process downloaded.

## Update Verification Requirements

- Sign release artifacts offline, with a private key that never touches the
  build or distribution infrastructure the update process talks to.
- Embed the corresponding public key in the binary at build time. Do not fetch
  the verification key from the same channel that serves the update.
- Verify the signature over the exact bytes that will be installed — not a
  manifest that names a separate, unverified download — before extraction,
  execution, or replacement of the running binary.
- Reject on any verification failure: wrong key, malformed signature, or a
  hash mismatch between the signed manifest and the fetched bytes. Do not fall
  back to installing unverified.
- Bind the signed payload to a monotonically increasing version so a verified
  *old* release cannot be replayed over a newer one (a rollback attack).
- Keep the update client's own TLS and hostname validation ([api-tls-required](api-tls-required.md))
  as defense in depth, not as the control that makes the artifact trustworthy.
- Fail closed: if verification cannot run — corrupt local key store, unknown
  signature format — refuse the update and keep running the current version.

## Bad

```rust
// TLS makes this connection confidential and authenticates the server, but
// says nothing about whether the server or its storage was compromised.
// Anything this download returns gets installed as-is.
fn apply_update(url: &str) -> std::io::Result<()> {
    let bytes = reqwest::blocking::get(url)?.bytes()?;
    std::fs::write(std::env::current_exe()?, &bytes)?;
    Ok(())
}
```

## Good

```rust
/// Stand-in for a real signature scheme (Ed25519 via `ed25519-dalek`, or an
/// OS code-signing API); this keyed checksum has none of the unforgeability
/// or key-recovery resistance a real signature needs. It exists only to
/// exercise the verify-then-install control flow below, not as cryptography.
fn sign(payload: &[u8], key: u8) -> u8 {
    payload.iter().fold(key, |acc, byte| acc ^ byte)
}

fn verify(payload: &[u8], signature: u8, key: u8) -> bool {
    sign(payload, key) == signature
}

/// Baked in at build time; this key never travels over the update channel.
const RELEASE_KEY: u8 = 0x5a;

pub struct SignedUpdate {
    pub version: u32,
    pub payload: Vec<u8>,
    pub signature: u8,
}

#[derive(Debug, PartialEq)]
pub enum UpdateError {
    BadSignature,
    Rollback { installed: u32, offered: u32 },
}

/// Verify a downloaded update against the embedded key before it is ever
/// written to disk or executed. Only the exact signed bytes are accepted.
pub fn verify_update(
    update: &SignedUpdate,
    installed_version: u32,
) -> Result<&[u8], UpdateError> {
    if update.version <= installed_version {
        return Err(UpdateError::Rollback {
            installed: installed_version,
            offered: update.version,
        });
    }
    if !verify(&update.payload, update.signature, RELEASE_KEY) {
        return Err(UpdateError::BadSignature);
    }
    Ok(&update.payload)
}

fn main() {
    let payload = b"binary contents go here".to_vec();
    let signature = sign(&payload, RELEASE_KEY);

    let genuine = SignedUpdate { version: 2, payload: payload.clone(), signature };
    assert_eq!(verify_update(&genuine, 1), Ok(payload.as_slice()));

    let tampered = SignedUpdate { version: 2, payload: b"different bytes".to_vec(), signature };
    assert_eq!(verify_update(&tampered, 1), Err(UpdateError::BadSignature));

    // Same signature, but offered at a version no newer than what is already
    // installed: rejected before the signature is even consulted.
    let replay = SignedUpdate { version: 1, payload, signature };
    assert_eq!(
        verify_update(&replay, 1),
        Err(UpdateError::Rollback { installed: 1, offered: 1 })
    );
}
```

## Rejection Cases To Test

- an update signed with the wrong key is rejected;
- an update whose payload was modified after signing fails verification even
  though the signature field is well-formed;
- an update whose version is not strictly greater than the installed version
  is rejected, even with a valid signature (rollback);
- a manifest that names a download URL is not enough — the signature must
  cover the bytes actually installed, not a pointer to them;
- a verification-path failure (missing or corrupt embedded key) leaves the
  current version running rather than installing unverified.

## See Also

- [api-tls-required](api-tls-required.md) - authenticates the channel; this rule authenticates the artifact
- [api-crypto-primitives](api-crypto-primitives.md) - the signature primitive itself and constant-time verification
- [proj-reproducible-runtime](proj-reproducible-runtime.md) - build the exact artifact that gets signed
- [proj-dependency-policy](proj-dependency-policy.md) - the same distribution-compromise risk for dependencies rather than releases
- [api-record-checksum](api-record-checksum.md) - detects corruption; a signature is required to also rule out tampering
