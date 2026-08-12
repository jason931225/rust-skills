# api-inherent-core

> Put a type's essential methods on the type itself; implement traits by forwarding to them

## Why It Matters

If the only way to call `download_file` is through a trait, every caller must discover and import that trait. Inherent methods show up on the type in rustdoc and in completion, so the common path needs no extra `use`. The Microsoft Pragmatic Rust Guidelines keep traits as optional adapters: implement the work once on the type, then forward from the trait so generic code still compiles.

## Bad

```rust
pub trait Download {
    fn download_file(&self, url: &str) -> String;
}

pub struct HttpClient;

impl Download for HttpClient {
    fn download_file(&self, url: &str) -> String {
        format!("fetched {url}")
    }
}

fn main() {
    let client = HttpClient;
    // Does not compile until `Download` is in scope.
    let _ = Download::download_file(&client, "https://example.com");
}
```

## Good

```rust
pub trait Download {
    fn download_file(&self, url: &str) -> String;
}

pub struct HttpClient;

impl HttpClient {
    pub fn download_file(&self, url: &str) -> String {
        format!("fetched {url}")
    }
}

impl Download for HttpClient {
    fn download_file(&self, url: &str) -> String {
        Self::download_file(self, url)
    }
}

fn fetch_via_trait(client: &impl Download, url: &str) -> String {
    client.download_file(url)
}

fn main() {
    let client = HttpClient;
    let direct = client.download_file("https://example.com");
    let via_trait = fetch_via_trait(&client, "https://example.com");
    assert_eq!(direct, via_trait);
}
```

## See Also

- [api-extension-trait](api-extension-trait.md) - traits are for adding methods to foreign types, not hiding your own
- [api-sealed-trait](api-sealed-trait.md) - keep a trait implementable only by your crate when it is not a user hook
- [name-no-get-prefix](name-no-get-prefix.md) - name the inherent method the way callers already search for it
