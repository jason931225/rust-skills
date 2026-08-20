# Changelog

All notable changes to this skill are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and the project aims to follow
semantic versioning for the rule set.

## [Unreleased]

### Added
- Four gaps from surveying "Type-Driven Correctness in Rust" (the first
  RustTraining book) via a six-chunk Grok 4.6 xhigh pass:
  `type-affine-quantity` (an absolute quantity and its delta are different
  types — implement `Sub<Self> -> Delta` but never `Add<Self>`, so adding two
  temperatures or two timestamps does not compile), `api-typed-command-dispatch`
  (give each request an associated response type and its own decoder, so a
  dispatcher cannot decode one command's reply as another's),
  `trait-capability-mixin` (bind a method to a conjunction of resources with
  supertrait-bounded ingredient traits and an empty blanket impl, so the
  method exists only on a receiver owning the whole set), and
  `type-exclusive-occupancy-guard` (while DMA or a GPU owns a buffer, hold it
  in a `!Send` guard whose only way back is a consuming `wait`). `api-typestate`
  gained two sections — independent required builder fields as separate type
  parameters so setters commute, and the `E0366` constraint that `Drop` cannot
  be specialized per typestate — and `const-fn` gained constructor-enforced
  relational invariants for layouts and derivation chains.
- Three gaps from surveying "Fullstack Rust" via Grok 4.6 xhigh reasoning
  (intro/Actix, the Diesel-backed blog app, WASM/CLI, macros) — the last
  unreviewed book in the PDF corpus: `api-request-scoped-state` (a value
  built inside a web framework's per-worker factory closure is worker-local;
  shared state must be built once and cloned in, and request-scoped values
  belong in the framework's typed extension map, not a handler parameter
  alone), `ffi-wasm-wire-abi` (export a compound value across a numeric-only
  WASM ABI as an explicit `(ptr, len)` pair, never as a pointer into a
  `String`'s or `Vec`'s undocumented `#[repr(Rust)]` layout), and
  `macro-proc-helper-attributes` (a proc macro sees only tokens, before type
  checking, so trait-dependent codegen needs an explicit caller-stated
  helper attribute, declared via `attributes(...)` and parsed with
  `syn::parse::Parse`). Six existing rules gained bullets: `api-error-schema`
  (extractor rejection is a separate pipeline from a handler's own `Result`;
  a response-building trait's un-overridden sibling method still runs),
  `conc-db-transaction-boundary` (recovering an `INSERT`'s generated id
  without `RETURNING` needs the same connection or transaction, not a
  follow-up `MAX(id)`; load-then-group beats one query per parent),
  `ffi-wasm-memory-view` (WASM integers carry no signedness of their own;
  linear memory grows but never shrinks), `unsafe-sound-abstractions` (a
  WASM export reachable from an untrusted host is the same boundary as any
  other FFI entry point), `ffi-foreign-resource-binding` (input and output
  allocations need separate paired free functions; a custom `alloc`/`dealloc`
  pair must never see a zero-sized `Layout` and must match size and
  alignment exactly), and `api-http-connection-lifecycle` (`Content-Length`
  after transparent decompression reflects the encoded size, not the
  decoded bytes a caller reads back).
- Two gaps from surveying "Command-Line Rust" (Ken Youens-Clark), which
  reimplements classic Unix utilities in Rust and surfaces clap and
  text-I/O pitfalls repeatedly across all thirteen tools it builds:
  `api-clap-parser-contract` (`Parser::parse`/`get_matches` exit the process
  on a bad argv regardless of the caller's error handling; an argument's id
  and its `value_name` are different strings that silently return `None`
  when confused; `conflicts_with` forbids combinations but does not require
  one-of) and `type-line-terminator-fidelity` (`BufRead::lines()` strips
  every terminator and fabricates a trailing newline a source file never
  had — a byte-faithful tool needs `read_line`/`read_until` and `print!`,
  not `lines()` and `println!`). `proj-cli-contract` and `test-cli-blackbox`
  each gained several bullets from the same survey: pin a specific dialect
  (BSD/GNU routinely disagree) and test against its real binary, generate
  golden output from the reference implementation rather than by hand, and
  compare raw bytes rather than lossy-decoded strings when a tool's output
  is not guaranteed UTF-8.
- Four gaps from surveying "Rust in Action" (Tim McNamara) via a five-chunk
  Grok 4.6 xhigh-effort pass across the whole book (language foundations
  through kernels and signals), independently verified before landing:
  `api-buffer-disclosure` (a Heartbleed-class leak — disclose the bytes a
  request actually wrote, never a reused buffer's length or capacity),
  `api-http-connection-lifecycle` (force `Connection: close` on a one-shot
  HTTP/1.1 client, and resend the hostname in `Host:` since the transport
  layer forgot it), `conc-thread-budget` (size a CPU-bound pool to physical
  cores, not job count; `thread::sleep` is a request, not a deadline), and
  `mem-page-commit` (the first write to freshly allocated memory is the real
  cost and the real OOM risk, not the allocation call). Five existing rules
  gained bullets from the same survey: `type-time-domain` (epoch/unit/
  signedness as part of any wire timestamp, leap-second duplicate
  timestamps, CPU cycle counters are not clocks, timezone-in-the-type, slew
  not step), `conc-signal-handler-safety` (`sigaction` not `signal()`,
  `SIGPIPE`'s default disposition, the signal/interrupt/panic taxonomy,
  `SetConsoleCtrlHandler` on Windows, and why `setjmp`/`longjmp` is
  RAII-unsound), `api-non-exhaustive` (keep an unrecognized wire-protocol
  discriminant as data, not a parse failure), `mem-with-capacity` (capacity
  is not length for a buffer an OS call like `recv_from` writes into), and
  `type-text-decode-policy` (wire framing bytes are not platform text).
- Five gaps from surveying "Rust for Rustaceans" (Jon Gjengset) via a Grok
  4.6 xhigh-effort pass across all twelve chapters, filtered to skip anything
  this library already states at that level of detail and independently
  verified before landing: `unsafe-pointer-provenance` (offset/add/sub is UB
  the moment the result leaves the original allocation, even if never
  dereferenced), `unsafe-dropck-phantom` (a raw-pointer-holding wrapper needs
  `PhantomData<T>` for drop-check to see it as owning a `T` at all),
  `ffi-c-bitflag-enum` (a `#[repr(C)]` fieldless enum is not a bitset; model
  C bitmask groups as a newtype with associated constants instead),
  `ffi-foreign-resource-binding` (return a foreign pointer to the allocator
  that produced it, and give each foreign handle kind its own type so they
  cannot be swapped), and `api-auto-trait-contract` (pin a public type's
  `Send`/`Sync`/`Unpin` status with a compile-only assertion, since auto
  traits are part of the contract without appearing in any signature). Two
  existing rules gained a bullet each: `num-nonzero` (the same niche makes
  `Option<extern "C" fn(...)>` the FFI-correct nullable function pointer) and
  `unsafe-maybeuninit` (commit a length or ownership marker only after every
  element is initialized, so an unwind partway through cannot leave a
  collection believing uninitialized memory is valid).
- Two gaps surfaced by delegating a survey of the previously-unread Black Hat
  Rust chapters (pp. 206-350: phishing/WASM, RAT architecture, end-to-end
  crypto, cross-platform builds, worm propagation) to Grok 4.6 at xhigh
  reasoning effort, then independently verified against the existing rule set
  before landing anything: `type-unicode-identity` (canonicalize a hostname to
  ASCII/Punycode before it is trusted, and never decode it back to Unicode on
  a surface a person uses to make a security decision) and
  `api-update-signature` (verify a signature over a self-update payload with
  a key that never traveled over the update channel, and reject rollback).
  Most of the survey's other flagged passages turned out to already be
  covered — autoescaping, SQL parameterization, AEAD/KDF choice, enum-shaped
  state, transaction boundaries, graceful shutdown — which is recorded rather
  than re-landed.
- `proj-atomic-file-replace`, extracted from a paragraph inside `async-tokio-fs`
  whose subject is blocking isolation. Replacing a whole file safely — a
  same-directory temporary, `sync_all`, a rename rather than a truncate, then a
  directory sync — is a data-loss contract in its own right, and nobody looking
  for it would have found it where it was. `async-tokio-fs` now points at it.
- The last three production-relevant gaps from the RustTraining survey:
  `type-capability-token` (authority appears in the signature as an unforgeable
  token, rather than as a flag each privileged function must remember to
  check), `unsafe-pin-projection` (classify each field of a `!Unpin` type once
  and keep every accessor consistent with that choice), and `proj-libc-floor`
  (choose the C library floor the fleet must satisfy and verify the shipped
  binary against it). The library is at 401 rules; the six remaining confirmed
  gaps are a bare-metal cluster left unlanded pending a decision about whether
  embedded Rust is an audience this library serves.
- Four more rules from the same deep read: `api-typed-response` (build the
  outbound payload by serializing a typed value, the producer-side mirror of
  the inbound boundary rules), `own-split-borrow-fields` (group a wide struct's
  fields so independent operations borrow disjointly, instead of reaching for
  `RefCell`), `unsafe-byte-slice-cast` (reinterpret bytes only through a
  length- and alignment-checked conversion), and `ffi-opaque-handle-lifecycle`
  (one constructor, one paired free, a null check at every entry point).
- Four rules from a deep read of the RustTraining chapters flagged as
  containing contracts the library lacked. `api-sql-parameters` closes an
  outright hole: the library governed command injection and path traversal but
  said nothing about SQL, the most common injection class. The others are
  `async-explicit-close` (no async `Drop`, so releases that await need an
  explicit `close`), `test-drop-release-paths` (observe the release on an early
  `?` and while unwinding, not on the happy path), and
  `async-runtime-agnostic-lib` (a library takes futures; the binary picks the
  runtime).
- Five more rules from the RustTraining survey, completing its confirmed-gap
  queue: `async-poll-contract` (never block, re-check readiness, re-register the
  waker before `Pending`, never poll after `Ready`),
  `unsafe-pin-address-stable` (`PhantomPinned` and `Pin<&mut Self>` for a type
  whose invariant is its address), `async-sync-core` (business rules in sync
  functions, `async` at the shell), `proj-build-target-cfg` (build scripts read
  `CARGO_CFG_TARGET_*`, never the host's `cfg!`), and
  `test-compile-fail-guarantees` (pin type-level guarantees with compile-fail
  tests). Nine RustTraining units are now reviewed.
- Four rules from the Microsoft RustTraining survey, each bound to the unit it
  came from in `microsoft_training_coverage.json`: `type-single-use-token`
  (a permission that is neither `Clone` nor `Copy`, so a second use will not
  compile), `conc-condvar-predicate-loop` (a wakeup is a hint, not proof),
  `api-scoped-closure-access` (lend a resource through a closure rather than
  paired setup and teardown), and `type-generational-handle` (a reused slot
  index needs a generation counter). The first four RustTraining units move off
  `unreviewed`.
- `checks/rule_provenance.json` records where every rule comes from: 235 are
  named by a source-coverage ledger, and the other 145 now carry a typed source
  class and a one-sentence justification. `validate.py` fails if a rule has
  neither, so the library can no longer accumulate guidance of unknown origin,
  and it rejects any justification carrying a URL, page, section, or version
  locator — the shape a fabricated citation takes.
- Six rules completing the confirmed-gap queue from the corpus survey:
  `api-datagram-trust`, `type-time-sample-once`, `unsafe-volatile-mmio`,
  `type-case-insensitive-match`, `test-env-independent`, and
  `ffi-wasm-memory-view`. All six carry behavior assertions; the corpus ledger
  now records 46 reviewed units.
- Five more corpus rules covering stored uploads, credential scoping, log
  recovery, monomorphization cost, and macro path hygiene:
  `api-upload-serving`, `api-credential-scope`, `proj-append-log-recovery`,
  `opt-monomorph-outline`, and `macro-absolute-std-paths`. Four carry behavior
  assertions; the corpus ledger now records 41 reviewed units.
- Five further corpus rules covering the process and filesystem boundary:
  `conc-signal-handler-safety`, `api-dir-enumeration`, `type-text-decode-policy`,
  `perf-hoist-loop-invariant`, and `ffi-status-to-result`. Four carry behavior
  assertions; the corpus ledger now records 37 reviewed units.
- Ten further rules from the authenticated PDF corpus, found by a parallel gap
  survey of the previously unmined slices and confirmed against the rule
  library: `err-short-read`, `err-debug-assert-scope`, `err-send-sync-static`,
  `serde-byte-order`, `serde-format-version`, `api-record-checksum`,
  `api-error-schema`, `api-subprocess-args`, `proj-secret-file-mode`, and
  `type-path-not-string`. Nine carry behavior assertions; the corpus ledger now
  records 33 reviewed units.
- Thirteen rules derived from the eight authenticated PDF sources, each bound to
  the source unit it came from: `api-path-containment`, `api-outbound-target`,
  `api-resource-limits`, `api-crypto-primitives`, `type-time-domain`,
  `type-secret-material`, `type-variance`, `test-fuzz-target`,
  `test-sanitizers`, `test-cli-blackbox`, `proj-cli-contract`,
  `proj-semver-contract`, and `proj-dependency-policy`.
- Nine behavior assertions in `checks/tests/pdf_corpus_guidance.rs` covering
  path containment, outbound target authorization, request ceilings,
  constant-time secret comparison, monotonic elapsed time, secret redaction,
  variance, the fuzz property, and the CLI stream/exit contract.
- A 1,325-unit inventory of the eight authenticated PDF sources
  (`checks/pdf_corpus_coverage.json`), rebuilt by `checks/build_pdf_ledger.py`
  from binaries authenticated against their pinned SHA-256 digests, with
  dispositions supplied by the reviewed `checks/pdf_corpus_review.json`.
  Twenty-three units are reviewed; the remaining 1,302 stay explicitly
  `unreviewed` and receive no semantic coverage credit.
- Microsoft Pragmatic Rust Guidelines (v2026.6) coverage.
  The source audit adds original rules and revises existing guidance where
  topic-level mappings omitted caveats or taught the inverse.
- Executable source coverage includes a pinned 89-item Microsoft manifest,
  source-backed inventory and nested-reference checks, focused behavioral
  examples where runtime behavior exists, and extracted-example compilation.
- A 431-unit audit of *Zero To Production In Rust* adds durable production
  contracts while explicitly dispositioning obsolete and product-specific
  tutorial recipes.
- Rust 1.95 through 1.97 guidance for atomic update helpers, conditional
  selection, `if let` guards, integer bit APIs, warning policy, and consistent
  ordering implementations.
- A 162-entry Rust 1.95.0–1.97.1 release-note ledger records every language,
  library, Cargo, rustdoc, platform, compatibility, internal, and patch item
  with its disposition, evidence, executable check, and uncertainty.
- A source-bound inventory records all 2,124 Microsoft RustTraining units as
  explicit unreviewed backlog without assigning semantic coverage credit.

### Changed
- 859 RustTraining units across 65 chapters leave the review backlog with a
  stated reason: 532 as `project-specific` cross-language onboarding and 327 as
  `reject` tutorial scaffolding, none of them mapping a rule. The 66 chapters
  an agent judged already covered were deliberately left `unreviewed`, since a
  chapter-level coverage claim applied to a dozen headings is the unearned
  credit these ledgers exist to prevent. 1,256 units remain unreviewed.
- Four pairs of rules that taught the same contract in different categories are
  merged, taking the rule count from 384 to 380: `err-doc-errors` into
  `doc-errors-section`, `doc-link-types` into `doc-intra-links`,
  `name-iter-method` into `name-iter-convention`, and `perf-collect-once` into
  `perf-iter-lazy`. Each survivor keeps the sections the retired rule alone had,
  and every inbound cross-link is repointed.
- `CONTRIBUTING.md` now records why the `anti-` category is deliberately
  redundant with its positive counterparts, so a future audit does not merge
  away a REFERENCE index that exists to be reached from the smell rather than
  the principle.
- Pinned the compile-check toolchain and CI to Rust 1.97.1 and updated existing
  conversion, `NonZero`, workspace, collection, cfg, lint, and unsafe/FFI
  guidance for the 1.95–1.97 releases.
- Added focused language-guidance behavior checks; generated examples remain
  checked against the reviewed baseline.
- Corrected forty self-contradictory book dispositions, removed redundant
  mutable-ledger checksums, and recorded the semantic audit as blocked until
  the pinned purchased PDF can be independently reread.
- Added bounded dependency admission and SLO-driven operational signal rules.
- Replaced universal release-profile and optimization prescriptions with
  measured artifact policy, and narrowed Miri and iterator guidance to the
  behavior those tools and constructs can establish.

### Fixed
- `mem-arena-allocator` said a bump arena "frees all allocations at once" and
  never mentioned that it runs **no destructors**, so the rule as written
  invited placing a `File`, socket, guard, or `Vec` in an arena and leaking the
  resource. It now states what may live in an arena and why
  `bumpalo::collections` exists, with an assertion that reclaiming a block runs
  no `Drop`.
- `api-typestate` destroyed a live connection on a wrong password: its
  `authenticate` consumed `self` and returned only an error, leaving no way to
  retry. It now returns the receiver in the error variant, and the general
  contract is stated in the new `api-fallible-self-return`.
- Seven rules made falsifiable runtime claims that nothing tested; each now has
  an assertion, and writing two of them corrected the rules. `perf-io-buffering`
  said a dropped `BufWriter` "attempts to flush" — it writes the buffer out
  through the inner writer and never calls `flush` on it, discarding that
  write's error. `api-password-auth`'s only assertion restated the function
  above it; it now asserts the property that matters, that an unknown account
  and a wrong password are indistinguishable. The others cover drop order,
  serde enum tagging, `catch_unwind` isolation, cancellation safety, and
  redirect/CSRF handling — the last replacing a non-executable pseudo-listing.
- `perf-collect-into` recommended a nightly-only API in its summary line, which
  is what the index shows; it now names `extend`, and its two identical
  "stable alternative" sections are one. `proj-mod-rs-dir` mandated `mod.rs`
  while its own example used the adjacent layout; it now asks for consistency.
  `api-password-reset` and `opt-target-cpu` had several independent obligations
  buried in prose and now state them as contracts, and two restated sections
  were removed from `type-enum-states` and `api-sealed-trait`.
- Sixteen factual defects found by a consolidation audit of all 384 rules and
  confirmed by compiling against the pinned 1.97.1 toolchain: a struct-packing
  example asserting 24 bytes for a type that is 16 (the default representation
  reorders fields, so the lesson only holds under `repr(C)`); `clamp` described
  as "unspecified" for a `NaN` bound when it panics, in release as well as
  debug; a `#[should_panic]` test returning `Result`, which does not compile;
  `thread::spawn(...).expect(...)`, which does not exist; `Option::map`
  described as `#[must_use]` when it emits no warning; E0038's wording, which
  is now "not dyn compatible"; itertools `group_by`, removed in 0.14 in favour
  of `chunk_by`; chained mockall `.returning` closures, which overwrite rather
  than queue; nightly-only rustfmt options presented as stable configuration;
  a Cargo build-script override table given `rustflags`, which it rejects;
  `#[cold]`'s effects stated as guarantees; `Chain`'s per-item cost, which
  internal iteration no longer pays; a sealed trait described as having open
  methods, which sealing forbids; a fabricated `oneshot::Sender` clone;
  a `redundant_feature_names` example that does not fire; and crates.io badge
  metadata that is no longer rendered.
- The *Zero To Production In Rust* ledger was bound to a superseded PDF and
  stuck at `blocked-source-reread`. It is now re-anchored to the authoritative
  binary by page-text identity — all 431 rows matched exactly one page, uniform
  offset +1, none ambiguous or unmatched — so its dispositions rest on the bytes
  they were written against. The proof covers the 256 of 433 pages the rows
  name; the ledger records that scope as `source-rebound-partial-proof` rather
  than claiming a whole-document result. `checks/rebind_zero2production.py`
  reproduces and re-verifies the binding under the pinned extractor, and
  validation recomputes a per-row binding digest so an edited page or digest
  fails even where the PDF cannot be opened.
- Corrected the non-`Copy` `Email(String)` newtype example, parent-subtree
  visibility guidance for `pub(super)` and `pub(crate)`, and the CI push branch.
- `async-cancellation-token` waited only on `ctrl_c()`, so a service following
  its shutdown example was killed by the orchestrator instead of draining.
  The example now waits on `SIGTERM` and `SIGINT`, states the ordering
  (fail readiness, let routing observe it, cancel, drain within a budget below
  the platform grace period, abort the remainder), and compiles — its
  compile-suspect baseline entry is retired. A behavior test asserts the
  ordering and the bounded drain.

Now 345 rules across 27 categories.

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

[Unreleased]: https://github.com/jason931225/rust-skills
[1.5.0]: https://github.com/leonardomso/rust-skills
[1.4.0]: https://github.com/leonardomso/rust-skills
[1.3.0]: https://github.com/leonardomso/rust-skills
[1.2.0]: https://github.com/leonardomso/rust-skills
[1.1.x]: https://github.com/leonardomso/rust-skills
[1.0.0]: https://github.com/leonardomso/rust-skills
