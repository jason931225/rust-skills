# api-sql-parameters

> Build every statement from fixed text with bound parameters; allowlist the identifiers that cannot be bound

## Why It Matters

A query assembled by concatenating caller input is the oldest remote code
execution in the catalogue, and Rust's type system does nothing to stop it —
`format!` produces a `String` whether or not the interpolated value ends the
literal and starts a new statement. Bound parameters remove the possibility
structurally: the driver sends the statement and the values separately, so a
value can never be parsed as syntax. Identifiers — table names, column names,
sort directions — cannot be bound, which is exactly why they need an allowlist
rather than escaping.

## Contract

- Every value reaching a statement is a bound parameter. No `format!`, no
  `push_str`, no string interpolation of caller input into SQL.
- Identifiers that cannot be parameterised — table, column, sort direction,
  index hints — come from a fixed allowlist mapping caller tokens to literal
  strings the code owns. Never escape and interpolate them.
- `LIMIT` and `OFFSET` are values: bind them, and bound them separately, since
  an unbounded page size is a denial-of-service surface rather than an
  injection one.
- A `LIKE` pattern is a value, but its wildcards are still syntax: escape `%`
  and `_` in caller input, and set the escape character explicitly.
- Compile-time checked queries verify shape against a schema; they do not make
  concatenation safe, so the rule holds inside them too.
- Give the database account the narrowest rights the statement needs;
  parameterisation is the first control, least privilege is the second.

## Bad

```rust
async fn find(pool: &Pool, email: &str, sort: &str) -> Result<Vec<Row>, Error> {
    // email = "' OR 1=1 --" returns every row; sort is worse, since it can
    // append an entire subquery
    let sql = format!("SELECT * FROM users WHERE email = '{email}' ORDER BY {sort}");
    sqlx::query(&sql).fetch_all(pool).await
}
```

## Good

```rust
#[derive(Debug, PartialEq)]
pub enum QueryError {
    UnknownSortColumn,
}

/// Caller tokens map to literal SQL this code owns. An unknown token is
/// rejected — never escaped and interpolated.
fn sort_column(token: &str) -> Result<&'static str, QueryError> {
    match token {
        "email" => Ok("email"),
        "created" => Ok("created_at"),
        _ => Err(QueryError::UnknownSortColumn),
    }
}

/// Returns the statement text and the values to bind, so the caller can see
/// that no value ever reaches the text.
pub fn find_by_email(sort: &str) -> Result<&'static str, QueryError> {
    Ok(match sort_column(sort)? {
        "email" => "SELECT id, email FROM users WHERE email = $1 ORDER BY email LIMIT $2",
        _ => "SELECT id, email FROM users WHERE email = $1 ORDER BY created_at LIMIT $2",
    })
}

/// `%` and `_` are wildcards, so caller text must be escaped before binding.
pub fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for character in input.chars() {
        if matches!(character, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(character);
    }
    out
}

fn main() {
    // The statement is fixed text; the address is bound, never interpolated.
    let sql = find_by_email("email").expect("known column");
    assert!(sql.contains("$1"));
    assert!(!sql.contains("'"), "no value is ever quoted into the statement");

    // A hostile sort token is refused rather than escaped.
    assert_eq!(find_by_email("email; DROP TABLE users"), Err(QueryError::UnknownSortColumn));

    // Wildcards in a LIKE value are data, not syntax.
    assert_eq!(escape_like("100%_off"), r"100\%\_off");
}
```

## Failure Tests

- `' OR 1=1 --` in a bound value returns no rows rather than every row;
- a sort or column token outside the allowlist is rejected, and the rejection
  does not disclose the schema;
- a `LIKE` search for `%` matches a literal percent sign;
- a page size beyond the documented maximum is refused;
- a statement built anywhere in the codebase by string concatenation of caller
  input fails review — grep for `format!` near `query`.

## See Also

- [api-subprocess-args](api-subprocess-args.md) - the same contract for the process boundary
- [api-parse-dont-validate](api-parse-dont-validate.md) - turn caller input into typed values first
- [conc-db-transaction-boundary](conc-db-transaction-boundary.md) - where these statements execute
- [api-resource-limits](api-resource-limits.md) - bounding page size and query cost
- [obs-no-sensitive-data](obs-no-sensitive-data.md) - do not log the statement with its values
