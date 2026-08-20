# api-upload-serving

> Serve user-uploaded files inertly: fixed content type, forced download, separate origin

## Why It Matters

An upload endpoint that stores what a user sends and serves it back from the
application's own origin has published attacker-controlled content under a
trusted name. An SVG is a script container, an HTML file runs with the site's
cookies, and a browser that sniffs a mistyped `Content-Type` will happily
execute either — turning file upload into stored cross-site scripting against
every viewer. Validating the extension does not help, because the browser
decides how to interpret the bytes, not the filename.

## Upload Serving Requirements

- Serve uploads from a separate origin — a different domain, not merely a
  different path — so a script that does run holds no session for the app.
- Send `Content-Disposition: attachment` with an explicit filename for anything
  not on a small render allowlist.
- Send `X-Content-Type-Options: nosniff` and a `Content-Type` chosen by the
  server from its own allowlist, never echoed from the upload.
- Re-encode images through a decoder rather than trusting the uploaded bytes,
  and treat SVG as markup, not as an image.
- Store under a server-generated identifier; keep the user's filename as
  metadata for display and downloads only.
- Apply a restrictive `Content-Security-Policy` and a `Sandbox` disposition to
  anything rendered inline.
- Authorize the fetch. A guessable path is not access control.

## Bad

```rust
async fn download(id: web::Path<String>) -> HttpResponse {
    let upload = store.load(&id).await?;
    HttpResponse::Ok()
        // Content type echoed from the upload, served from the app's origin:
        // an "image/svg+xml" upload executes script with the site's cookies
        .content_type(upload.declared_content_type)
        .body(upload.bytes)
}
```

## Good

```rust
#[derive(Debug, PartialEq)]
pub struct ServeHeaders {
    pub content_type: &'static str,
    pub disposition: String,
    pub nosniff: bool,
}

/// Only these are rendered inline; the server decides the type, and anything
/// else is delivered as an opaque download.
const INLINE: [(&str, &str); 3] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("pdf", "application/pdf"),
];

pub fn serve_headers(verified_kind: &str, display_name: &str) -> ServeHeaders {
    let inline = INLINE.iter().find(|(kind, _)| *kind == verified_kind);
    match inline {
        Some((_, content_type)) => ServeHeaders {
            content_type,
            disposition: format!("inline; filename=\"{}\"", sanitize(display_name)),
            nosniff: true,
        },
        // Unknown or scriptable content is never rendered by the browser.
        None => ServeHeaders {
            content_type: "application/octet-stream",
            disposition: format!("attachment; filename=\"{}\"", sanitize(display_name)),
            nosniff: true,
        },
    }
}

/// The filename reaches a header, so it may not carry quotes or line breaks.
fn sanitize(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '\r' | '\n'))
        .collect()
}

fn main() {
    let png = serve_headers("png", "holiday.png");
    assert_eq!(png.content_type, "image/png");
    assert!(png.disposition.starts_with("inline"));

    // An SVG is markup: it is not on the allowlist, so it downloads.
    let svg = serve_headers("svg", "logo.svg");
    assert_eq!(svg.content_type, "application/octet-stream");
    assert!(svg.disposition.starts_with("attachment"));
    assert!(svg.nosniff);

    // A header-injecting filename cannot break out of the disposition. The
    // value keeps exactly the two quotes that delimit the filename, and the
    // CR/LF that would have started a new header are gone.
    let hostile = serve_headers("svg", "a\"\r\nSet-Cookie: x=1");
    assert!(!hostile.disposition.contains('\r'));
    assert!(!hostile.disposition.contains('\n'));
    assert_eq!(
        hostile.disposition.matches('"').count(),
        2,
        "only the delimiting quotes survive; the injected one is stripped"
    );
    assert_eq!(hostile.disposition, "attachment; filename=\"aSet-Cookie: x=1\"");
}
```

## Inert Delivery Cases To Pin

- an SVG, an HTML file, and a mislabelled executable are all delivered as
  attachments with `nosniff`;
- the served content type never equals the one supplied at upload;
- a filename containing quotes, CR, or LF cannot alter the response headers;
- a request for another user's upload is rejected by authorization, not by
  identifier length;
- inline rendering happens on the separate origin, and that origin holds no
  session cookie.

## See Also

- [api-browser-security](api-browser-security.md) - the escaping and CSRF contract this extends
- [api-path-containment](api-path-containment.md) - where the upload is written
- [api-authz-fail-closed](api-authz-fail-closed.md) - a guessable identifier is not access control
- [api-resource-limits](api-resource-limits.md) - bound upload size and decode cost
- [api-extract-or-reject](api-extract-or-reject.md) - validate the upload before storing it
