# macro-proc-helper-attributes

> Give trait-dependent derive logic an explicit helper attribute instead of trying to query type information a proc macro cannot see

## Why It Matters

A procedural macro runs before type checking: it receives tokens, not a
resolved, type-checked AST, so it has no way to ask "does this field's type
implement `Default`?" or "is this generic parameter bounded by `Clone`?" A
derive that needs to branch on that kind of fact cannot compute the answer
itself — it can only emit code and let the compiler's own type checking
accept or reject it afterward, or it can ask the *caller* to state the fact
explicitly through a helper attribute (`#[builder(required)]`). Using an
attribute the derive did not declare is a hard compiler error, not a warning,
and declaring one does not hide it from other macros processing the same
item — every attribute on an item is visible to every macro that runs on it,
so the derive that owns an attribute has to skip ones it does not recognize
rather than erroring on them.

## Helper Attribute Requirements

- Do not attempt to answer a trait-resolution question ("does this type
  implement X?") inside macro expansion; a proc macro has tokens, not a
  type checker. Emit code that will only compile if the fact holds, or take
  an explicit helper attribute stating it.
- Declare every helper attribute the derive recognizes with
  `#[proc_macro_derive(Name, attributes(helper_name))]`. An attribute used on
  an item without being declared this way is a compile error at the call
  site, not at the macro's own compilation.
- Parse a helper attribute's argument list with `syn::parse::Parse` via
  `Attribute::parse_args`, using `Punctuated::parse_terminated` for a
  comma-separated list rather than matching on token strings by hand;
  `parse_args` already strips the attribute's own outer delimiter, so reach
  for `parenthesized!`/`braced!`/`bracketed!` only for a group nested inside
  the arguments, not for that outer one.
- Skip any attribute on the item whose path is not the macro's own; other
  macros' helper attributes (and ordinary attributes like `#[derive(..)]`
  itself) remain visible on the item and must not be treated as errors.
- Reject a helper attribute applied to the wrong target (an item-level
  attribute used on a field, or vice versa) with a `syn::Error` spanned to
  that attribute, not to the whole derive input.

## Bad

```rust
// A derive that wants to skip generating a `Default`-dependent method for
// fields whose type does not implement `Default`. There is no way to ask
// that question from inside a proc macro — this code cannot compile as
// written, because `T: Default` is a fact the macro cannot inspect.
fn should_emit_default_field(field_type: &syn::Type) -> bool {
    // No API exists to answer this from token-only input.
    todo!("query whether `field_type` implements `Default`")
}
```

## Good

```rust
use syn::{parse::Parse, parse::ParseStream, punctuated::Punctuated, Token};

/// The caller states the fact explicitly with `#[builder(required)]`,
/// rather than the macro trying to infer it from the field's type.
struct BuilderArgs {
    flags: Punctuated<syn::Ident, Token![,]>,
}

impl Parse for BuilderArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // `Attribute::parse_args` (below) already strips the outer
        // parentheses of `#[builder(...)]`; `input` here is only the
        // content inside them. A nested delimited group inside the args
        // would need its own `parenthesized!`/`braced!`/`bracketed!` call.
        Ok(BuilderArgs {
            flags: Punctuated::parse_terminated(input)?,
        })
    }
}

fn is_required(attrs: &[syn::Attribute]) -> syn::Result<bool> {
    for attr in attrs {
        // Skip every attribute this derive does not own — other macros'
        // helper attributes, and ordinary attributes, are not errors here.
        if !attr.path().is_ident("builder") {
            continue;
        }
        let args: BuilderArgs = attr.parse_args()?;
        return Ok(args.flags.iter().any(|flag| flag == "required"));
    }
    Ok(false)
}

fn main() {
    let attrs: Vec<syn::Attribute> = vec![syn::parse_quote!(#[builder(required)])];
    assert!(is_required(&attrs).expect("parses"));

    let unrelated: Vec<syn::Attribute> = vec![syn::parse_quote!(#[serde(skip)])];
    assert!(!is_required(&unrelated).expect("unrecognized attributes are skipped, not errors"));
}
```

## Derive Cases To Pin

- an item carrying the derive's own helper attribute with `required` present
  is recognized as required;
- an item carrying only an unrelated attribute (a different macro's helper,
  or an ordinary attribute) is treated as not required, not as a parse
  error;
- a malformed argument list inside the helper attribute (a token that is not
  a valid identifier) produces a `syn::Error` spanned to that attribute;
- using the helper attribute without declaring it in
  `#[proc_macro_derive(_, attributes(_))]` is confirmed to be a compile
  error at the call site (a documentation/regression check, not something
  the runtime test itself can trigger).

## See Also

- [macro-proc-syn-quote](macro-proc-syn-quote.md) - the parsing and code-generation toolchain this rule's `Parse` impl builds on
- [macro-proc-error-spans](macro-proc-error-spans.md) - reporting a malformed helper attribute as a spanned compile error
- [macro-no-implied-items](macro-no-implied-items.md) - the caller-visible surface a helper attribute adds to the derive's contract
- [api-typestate](api-typestate.md) - encoding a fact statically is often the alternative to asking for it as a helper attribute
- [trait-object-safety](trait-object-safety.md) - a different case where a trait-level fact must be established by the type system, not inferred
