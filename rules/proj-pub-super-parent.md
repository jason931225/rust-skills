# proj-pub-super-parent

> Use pub(super) to share items across the parent module and everything inside it

## Why It Matters

`pub(super)` exposes an item to the parent module's scope: the parent itself and every module nested inside it, including the declaring module's siblings and their submodules. That is wider than a private item but narrower than `pub(crate)` — except when the parent is the crate root, where `super` is the crate and the item reaches the whole crate. Use it for helpers a module group shares and the rest of the crate should not reach, and declare them below the crate root so the boundary is real.

## Bad

```rust
// src/frontend/parser/mod.rs
pub mod lexer;
pub mod ast;

// src/frontend/parser/lexer.rs
pub fn internal_helper() {  // Visible to entire crate!
    // Helper only needed by lexer and ast
}

pub(crate) struct Token {  // Visible to entire crate
    // Only parser submodules need this
}
```

## Good

```rust
// src/frontend/parser/mod.rs
pub mod lexer;
pub mod ast;

// Shared inside `frontend`: parser and its submodules, not the rest of the crate
pub(super) struct Token {
    pub(super) kind: TokenKind,
    pub(super) span: Span,
}

pub(super) fn shared_helper() -> Token {
    // Reaches `frontend` and everything nested inside it
}

// src/frontend/parser/lexer.rs
use super::{Token, shared_helper};

pub fn lex(input: &str) -> Vec<Token> {
    shared_helper();
    // ...
}

// src/frontend/parser/ast.rs
use super::Token;

pub fn parse(tokens: Vec<Token>) -> Ast {
    // ...
}
```

## Visibility Hierarchy

```
src/
├── lib.rs                 # crate root
├── frontend/
│   ├── mod.rs             # parent of `parser`: pub(super) items visible here
│   └── parser/
│       ├── mod.rs         # declares the pub(super) items
│       ├── lexer.rs       # inside the parent subtree: can use them
│       └── ast.rs         # inside the parent subtree: can use them
└── codegen.rs             # outside `frontend`: CANNOT see them
```

The scope is the parent module's whole subtree, so every module under
`frontend` — not just `parser` and its children — can reach these items.

## Pattern: Layered Visibility

```rust
// src/storage/database/mod.rs
mod connection;
mod query;
mod pool;

// Visible in the parent (`storage`) and everything nested inside it
pub(super) struct RawConnection { /* ... */ }

// Entire crate can see
pub(crate) struct Pool { /* ... */ }

// Everyone can see
pub struct Database { /* ... */ }
```

## Pattern: Test Helpers

```rust
// src/frontend/parser/mod.rs
mod lexer;
mod ast;

#[cfg(test)]
mod tests {
    use super::*;
    
    // Declared in `parser::tests`, so it reaches all of `parser`, including
    // the test modules of sibling submodules
    pub(super) fn make_test_token() -> Token {
        Token { kind: TokenKind::Test, span: Span::dummy() }
    }
}

// src/frontend/parser/lexer.rs
#[cfg(test)]
mod tests {
    use super::super::tests::make_test_token;
    // ...
}
```

## Comparison

| Visibility | Scope | Use Case |
|------------|-------|----------|
| `pub` | Everywhere | Public API |
| `pub(crate)` | Crate-wide | Internal shared utilities |
| `pub(super)` | Parent module and its whole subtree | Sharing between sibling modules |
| `pub(in path)` | Specific path | Precise control |
| (private) | Current module | Implementation details |

## When to Use pub(super)

- Helper functions shared between sibling modules
- Types used across a module group but not the rest of the crate
- Implementation details of a module group
- Test utilities for a module tree

## See Also

- [proj-pub-crate-internal](./proj-pub-crate-internal.md) - Crate visibility
- [proj-pub-use-reexport](./proj-pub-use-reexport.md) - Re-export patterns
- [proj-mod-by-feature](./proj-mod-by-feature.md) - Feature organization
