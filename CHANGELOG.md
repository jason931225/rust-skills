# Changelog

All notable changes to this skill are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and the project aims to follow
semantic versioning for the rule set.

## [Unreleased]

### Added
- `lint-warnings-deny-config`: prefer Cargo `[build] warnings = "deny"` for rustc
  CI instead of a global `RUSTFLAGS=-Dwarnings`.
- `num-bit-width`: use the Rust 1.97 integer bit-manipulation APIs
  (`bit_width`, `highest_one`/`lowest_one`, `isolate_*_one`) instead of
  hand-rolled shift and mask loops.
- `pat-if-let-guards`: bind fallible match-arm data with Rust 1.95 `if let`
  guards instead of parsing twice or unwrapping.
- `conc-atomic-update`: use Rust 1.95 atomic `update` / `try_update` instead of
  hand-written compare-exchange retry loops.
- `proj-cfg-select`: make one-of-many conditional compilation explicit with
  Rust 1.95 `cfg_select!`.
- `trait-ord-consistent`: keep equality and ordering contracts aligned so
  ordered collections remain correct.

### Changed
- Audited Rust 1.95 through 1.97 release changes, pinned the compile-check
  toolchain and CI to 1.97.1, and updated "Current for Rust" claims.
- Updated existing conversion, `NonZero`, workspace dependency, enum layout,
  collection insertion, visibility linting, read-only lockfile, pattern, cfg,
  atomic, and unsafe/FFI guidance for useful 1.95–1.97 changes and
  compatibility notes.
- Added executable release-behavior checks for the new language/library
  guidance; generated examples remain compile-checked against the baseline.
- Fixed `unsafe-no-mangle-unsafe` for Rust 1.97 `link_section` validation (ELF
  `.init_array` is now an error on Mach-O).

Now 271 rules across 26 categories.

## [1.5.1]

### Changed
- Depth pass: expanded `own-rc-single-thread` (breaking cycles with `Weak`, the
  `Rc::clone` idiom, `strong_count`/`weak_count`, `!Send`/`!Sync`) and
  `own-refcell-interior` (`Cell` for `Copy` types).
- Added cross-references from ~18 foundational rules to the newer categories
  (`conc-`, `conv-`, `num-`, `serde-`, `trait-`, `closure-`, `coll-`, `pat-`)
  for better navigation between related rules.

## [1.5.0]

### Added
- **Const & Compile-Time** category (`const-`, 4 rules): `const fn`, `const` vs
  `static`, const generics, inline `const { }` blocks.
- **Trait & Generics Design** category (`trait-`, 6 rules): static vs dynamic
  dispatch, associated types vs generic params, default methods, blanket impls,
  object safety, the orphan rule + newtype.
- **Collections** category (`coll-`, 4 rules): map choice (HashMap/BTreeMap/
  IndexMap), sequence choice (Vec/VecDeque), set membership, `BinaryHeap`.
- `checks/gen_index.py` — generates `SKILL.md`'s priority table and Quick
  Reference (and the rule counts) from `rules/` so the index can't drift; CI
  runs it in `--check` mode.
- `CONTRIBUTING.md` and this `CHANGELOG.md`.

Now 265 rules across 26 categories.

## [1.4.0]

### Added
- **Closures** category (`closure-`, 5 rules): Fn/FnMut/FnOnce bounds, returning
  `impl Fn`, `move` capture, static vs dynamic dispatch, disjoint capture.

Now 251 rules across 23 categories.

## [1.3.0]

### Added
- **Serde** category (`serde-`, 8 rules): rename_all, default, skip, flatten,
  enum representation, deny_unknown_fields, custom (de)serialize, validate-on-
  deserialize.
- **Numeric & Arithmetic Safety** category (`num-`, 5 rules): explicit overflow
  handling, `as` vs `TryFrom`, float comparison, clamping, `NonZero`.

Now 246 rules across 22 categories.

## [1.2.0]

### Added
- **Macros** category (`macro-`, 8 rules): declarative-macro hygiene and
  fragment specifiers, and procedural-macro design with `syn`/`quote`.
- **Observability** category (`obs-`, 7 rules): `tracing`, spans, structured
  fields, error chains, and keeping secrets out of logs.

Now 233 rules across 20 categories.

## [1.1.x]

### Added
- **Unsafe Code** (`unsafe-`), **Concurrency** (`conc-`), **Conversions**
  (`conv-`), and **Pattern Matching** (`pat-`) categories, plus new rules across
  existing categories — 39 rules in total.
- A compile-check harness (`checks/`) and a GitHub Actions CI workflow that
  validates rule structure, links, the index, and that examples compile.

### Changed
- Updated throughout for the Rust 2024 edition and current stable (Rust 1.96):
  fixed `&mut T` is not `Copy`, `impl Trait` in traits, `collect_into` status,
  `resolver = "3"`, `env::set_var` now `unsafe`, and more.

Now 218 rules across 18 categories.

## [1.0.0]

### Added
- Initial release: 179 rules across 14 categories.

[Unreleased]: https://github.com/leonardomso/rust-skills
[1.5.0]: https://github.com/leonardomso/rust-skills
[1.4.0]: https://github.com/leonardomso/rust-skills
[1.3.0]: https://github.com/leonardomso/rust-skills
[1.2.0]: https://github.com/leonardomso/rust-skills
[1.1.x]: https://github.com/leonardomso/rust-skills
[1.0.0]: https://github.com/leonardomso/rust-skills
