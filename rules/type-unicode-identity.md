# type-unicode-identity

> Canonicalize a hostname or domain to its ASCII/Punycode form before it is trusted as an identity, stored, compared, or shown to a person making a security decision

## Why It Matters

DNS, TLS server-name indication, and the HTTP `Host` header are all specified
in ASCII; every internationalized domain a network stack actually resolves is
transcoded to its Punycode (`xn--...`) form first. A Rust string comparison
like `raw == allowlisted` is not wrong about the bytes it is given — it is
exact — but it is comparing the wrong representation if one side is a raw
Unicode label a user typed and the other is whatever the network layer will
actually resolve. The sharper hazard is on the human side: many scripts
contribute glyphs that render indistinguishably from ASCII letters (Cyrillic
`а`, U+0430, versus Latin `a`, U+0061), so a domain decoded from Punycode back
to Unicode "for readability" in a log, an admin allowlist screen, or a URL
preview can show a person exactly the string they expect to see while
resolving to a different, attacker-registered host. The defense is not a
smarter comparison — it is never trusting or displaying the decoded form on
any surface where a security decision depends on what a person or a policy
check reads.

## Contract

- Canonicalize a hostname or domain to ASCII/Punycode with a maintained IDNA
  implementation as the first step after receiving it, before storage,
  comparison, or use in an allowlist.
- Reject a label that fails IDNA validation rather than passing it through
  unmodified or falling back to raw Unicode.
- Do not hand-roll the transcoding or validation; label rules and
  normalization have edge cases a manual scan misses.
- Keep every identity-bearing value — allowlist entries, audit logs, redirect
  targets, TLS SNI checks — in the canonical ASCII form. Do not decode a
  Punycode label back to Unicode for display on any surface a person uses to
  approve, review, or compare it.
- If a product genuinely needs to show the decoded Unicode form (a marketing
  page, not a security decision), keep that rendering separate from the value
  used for comparison, and never derive the comparison value from it.
- Carry the canonical form in a type that only a successful canonicalization
  can construct, so a raw, unvalidated string cannot reach a comparison by
  accident.

## Bad

```rust
// Accepts whatever the caller passes as the identity itself. Nothing forces
// a raw Unicode label through canonicalization first, and a caller that
// later decodes a stored Punycode value back to Unicode "for readability" in
// a log or admin screen can show a reviewer a glyph-perfect impostor.
fn is_allowed_host(host: &str, allowlist: &[&str]) -> bool {
    allowlist.contains(&host)
}
```

## Good

```rust
/// A hostname that has already been canonicalized to ASCII/Punycode. The
/// only way to get one is through `canonicalize`, so a value that reaches
/// comparison, storage, or a log can never be an un-transcoded raw label.
#[derive(Debug, PartialEq)]
pub struct Hostname(String);

#[derive(Debug, PartialEq)]
pub struct NotCanonical;

impl Hostname {
    /// Stand-in for a maintained IDNA implementation (the `idna` crate);
    /// production code calls `idna::domain_to_ascii`, which also normalizes
    /// and validates labels this narrow check does not attempt.
    pub fn canonicalize(raw: &str) -> Result<Self, NotCanonical> {
        if raw.is_ascii() && !raw.is_empty() {
            Ok(Hostname(raw.to_ascii_lowercase()))
        } else {
            // A real implementation transcodes non-ASCII labels to Punycode;
            // this stand-in refuses rather than risk a hand-rolled decode.
            Err(NotCanonical)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_allowed(host: &Hostname, allowlist: &[&str]) -> bool {
    allowlist.contains(&host.as_str())
}

fn main() {
    let host = Hostname::canonicalize("Apple.com").expect("ascii label canonicalizes");
    assert!(is_allowed(&host, &["apple.com"]));

    // A label containing the Cyrillic lookalike for "a" (U+0430) is never
    // turned into a Hostname at all — it cannot reach the comparison, and it
    // is never available to be decoded back to Unicode for a reviewer to
    // misread as the genuine domain.
    assert_eq!(Hostname::canonicalize("\u{430}pple.com"), Err(NotCanonical));
}
```

## Failure Tests

- a label containing a non-ASCII lookalike character fails canonicalization
  rather than being accepted or silently transliterated;
- the ASCII form of a legitimate domain canonicalizes successfully and
  compares equal to its allowlist entry;
- two inputs that a person would read as the same domain but that differ in
  case canonicalize to one comparison key;
- nothing in the comparison or storage path accepts a `&str` that did not
  pass through canonicalization — the type system, not a convention, enforces
  it;
- a value that failed canonicalization is never available to a display path,
  so it cannot be rendered back to a person as if it were trusted.

## See Also

- [type-case-insensitive-match](type-case-insensitive-match.md) - the matcher, not the data, should absorb another axis of equivalence
- [type-newtype-validated](type-newtype-validated.md) - the general pattern this rule specializes: validate once at construction
- [api-browser-security](api-browser-security.md) - escaping and redirect-target checks this identity comparison feeds
- [api-credential-scope](api-credential-scope.md) - binding a credential to the origin it was issued for depends on comparing that origin correctly
- [api-path-containment](api-path-containment.md) - the filesystem analog: resolve against a canonical form before you trust it
