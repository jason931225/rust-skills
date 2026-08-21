# Changelog

All notable changes to this skill are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and the project aims to follow
semantic versioning for the rule set.

## [Unreleased]

### Fixed
- 30 defects found by auditing this session's own new rules and sections, which
  had never been checked. 13 were medium; none survived a compiler.
  `pat-combinator-over-branch` claimed a diverging arm "cannot be a combinator
  argument" — it compiles with an `unreachable_code` warning and then returns
  the diverging value unconditionally, which is worse than the error the rule
  described. `proj-stable-toolchain` recommended
  `--config 'build.rustflags=[...]'` as the one-off escape hatch, which the
  precedence rule that section had just explained silently discards; the
  working form overrides `target.<triple>.rustflags`.
  `api-health-probes` and `trait-ord-consistent` both said derived `Ord` on a
  fieldless enum follows declaration order — it compares discriminant values,
  and the two coincide only when no discriminants are written, which matters
  because another rule tells readers to pin them.
- `serde-format-choice`'s Good `CacheEntry` was byte-identical to its Bad one,
  with the contrast living entirely in a comment; `type-capability-token`'s
  token proved nothing, because the handle was as conditional as the proof;
  `perf-iter-lazy` claimed a `#[must_use]` warning fires on an example that
  binds the value, which suppresses it; `test-observable-coverage` asserted on
  a private field in the rule that forbids exactly that;
  `api-builder-pattern`'s "Evidence from reqwest" block quoted a body that
  file does not contain.
- Also corrected: an invented `E0366` message, `E0308` where the shipped
  example produces `E0277`, an unreachable `select!` `else` arm, a
  `select_all` call that cannot compile without `Box::pin`, and an allocation
  count contradicted by this library's own in-place-specialization section.

### Added
- `serde-format-choice`: pick the encoding from who decodes the bytes. Every
  other `serde-` rule decides how to encode once a format is fixed, which
  assumed somebody chose one.
- `own-mutation-scope`: confine mutation to the block that builds the value and
  bind the result immutably, so `mut` ends where construction ends rather than
  at the end of the scope.
- Sections closing the rest of the backlog: derived-unit arithmetic on
  `type-affine-quantity`; what a wrapper actually costs on `api-newtype-safety`;
  editing versus rebuilding a collection on `anti-collect-intermediate`; the
  equality chain that never enrols in exhaustiveness checking on
  `pat-exhaustive-enum`; multi-output traversals on `perf-iter-over-index`;
  work, tick, and deadline in one synchronous select loop on
  `conc-thread-channel`; and ordering coverage gaps by consequence on
  `test-observable-coverage`.

### Changed
- `type-phantom-marker` no longer asserts "It has zero runtime cost" without
  qualification; it points at the section that measures where the erasure holds
  and where it stops.
- Both source ledgers are fully dispositioned. The training ledger's derived
  `semantic_status` now reads `reviewed`, which the validator enforces in both
  directions.

### Changed
- The training ledger gains an `out-of-library-scope` rationale class, and the
  twelve build-tooling and embedded units it describes move off `unreviewed`.
  `tutorial-scaffolding` says a unit carries no engineering contract; these
  carry a real one the library has decided not to state. Leaving them
  `unreviewed` claimed nobody had looked, which stopped being true once someone
  had — so the ledger could only misdescribe them in one direction or the
  other until the vocabulary admitted a third answer.

### Added
- `async-sync-core` gains the adoption decision the boundary rule presumed:
  what a runtime costs before it buys anything — lock types change, `Send +
  'static` propagates outward through every generic that feeds a task, and
  every test grows an executor — against what it actually buys, which is many
  mostly-waiting operations without an OS thread each. The crossover is a
  measurement, so the rule states the comparison and not a number.
- `perf-iter-lazy` gains laziness as a correctness hazard: an adapter body runs
  once per item pulled, so a side effect in a chain nobody consumes runs zero
  times. `#[must_use]` catches the simplest case and stops helping as soon as
  the value is bound or partially consumed.
- `api-builder-pattern` gains where failure belongs in a fluent chain — setters
  are pure assignments, fallibility concentrates in the terminal call — because
  a chain that can fail at every step gives every `?` the same anonymity.

### Added
- `async-assert-send` gains the generic case. The compile-time assertion cannot
  be written for a generic future, so a generic API that hands its parameter to
  another worker declares `Send + 'static` itself. Without it the caller's error
  points into the callee's body and tokio's source; with it, `required by a
  bound in store` points at the signature the caller wrote against.
- `api-health-probes` gains the fold: an `Ord`-ordered enum over an explicitly
  enumerated source list, so the worst verdict wins by derivation rather than by
  a hand-maintained `if` chain, and an empty source list is a decision instead
  of a fallthrough to healthy.
- `trait-ord-consistent` gains the fact that makes that work — derived ordering
  follows declaration order for enum variants and field order for structs, so
  alphabetising variants silently changes every comparison.
- `async-durable-worker` gains who owns the retry decision: backoff, jitter, and
  the budget belong to the worker; which failures are transient belongs to the
  caller, as a closure rather than error kinds the worker invented.
- `type-capability-token` gains proof minted from a successful initialization,
  so a degraded startup path cannot name the source that failed to come up.

### Added
- `async-select-racing` gains what `biased` actually costs. The random default
  is the fairness mechanism, and `biased` trades it away: draining two
  permanently-ready channels 2000 times gives `first=1920 second=80` under
  `biased` against `991/1009` unbiased. It also states the direction the usual
  advice omits — `biased` fixes starvation only when the branch at risk is
  listed first, and produces it when the hot branch is.

### Added
- `proj-stable-toolchain` gains the per-target half of a pinned build:
  `rust-toolchain.toml` pins which compiler and targets, `.cargo/config.toml`
  pins how each target links and runs. Carries the two merge rules that fail
  silently — a matching `[target.*].rustflags` replaces `[build].rustflags`
  outright rather than concatenating, and `RUSTFLAGS` in the environment
  replaces every rustflags list from config, so an ad-hoc `RUSTFLAGS=` discards
  the committed cross-linker settings.
- `proj-cfg-select` gains predicate selection. The two Windows ABIs differ in
  exactly one cfg key, `target_env`, so `#[cfg(windows)]` matches both and
  cannot separate MinGW from MSVC; `target_pointer_width` is 64 for both x86_64
  and aarch64, so it is not an architecture check.
- `proj-works-out-of-box` gains the one-cfg-gated-layer boundary, so logic
  above it compiles and tests unchanged on every target.
- `proj-semver-contract` gains `0.y.z`: the minor is the breaking position, so
  `"0.59"` already means `>=0.59.0, <0.60.0`, and two incompatible minor lines
  in one graph link twice with separate statics and `TypeId`s.
- `ffi-sys-vs-ffi-name` gains the dependency choice between raw `-sys` bindings
  and a safe wrapper, and why taking both for one library is a mistake.
- `type-enum-states` gains the case for an enum over a group of integer
  constants, and `#[repr(uN)]` for discriminants an external contract fixes.

### Added
- `api-typestate` gains capability markers: an empty marker trait implemented by
  every state that has a capability, with one `impl` block (and free functions)
  bound on the marker instead of one block per concrete state. Adding a state
  becomes one line, and a state without the marker still fails — with an error
  naming the unsatisfied capability rather than a missing method.

### Added
- `trait-config-family`: collapse three or more collaborator generics into one
  config trait whose associated types name the whole family. The cost of the
  multi-parameter form is not verbosity — adding a fourth collaborator forces
  every downstream signature to change in lockstep, and leaving one behind is
  `E0107`, so the churn is mandatory rather than stylistic. Carries the
  boundary that keeps it from contradicting `api-typestate`: a state axis stays
  its own parameter, giving `Handle<Cfg, S>`.
- `pat-combinator-over-branch`: write the named combinator instead of the
  hand-rolled branch, and the limit that makes that safe. Two measured facts
  carry it — `then_some` and `map_or` evaluate their alternative eagerly while
  `then` and `map_or_else` defer it, and collecting into `Result` short-circuits
  at the first `Err` with the rest of the source left unpulled. The
  counter-boundary is a section, not a caveat: a diverging or differently-typed
  arm cannot be a combinator argument at all, and the compiler says so.
- `unsafe-byte-slice-cast` gains the packed-field case: `&packed.field` is
  `E0793`, so safe code cannot express it, and the surviving obligation is to
  copy the field out or reach it through `&raw const` plus `read_unaligned`.
- `test-observable-coverage` gains the measurement mechanics its policy already
  presumed — source-based instrumentation counting regions rather than lines
  (a demonstration where line coverage reports 100% and region coverage 80% on
  the same run), merging profiles across test kinds, and treating an exclusion
  as a reviewable claim. Deliberately not a threshold: that rule refuses one.

### Changed
- `checks/run_examples.py` retries a timed-out example once with a longer budget
  before failing. The suite runs straight after a full `cargo build`, so a
  wall-clock timeout could not tell a hung example from a loaded machine — one
  example failed spuriously that way and passed immediately on its own.

### Fixed
- 44 verified defects across 42 rules, from a 48-finding audit queue. Each
  finding was adversarially re-checked before any edit, and four were thrown
  out — `obs-error-chain` (all three types it named as counterexamples do print
  the chain), `own-rwlock-readers` (already corrected in an earlier pass),
  `async-explicit-close` (the disputed panic reproduces exactly as written), and
  `err-no-unwrap-prod` (the snippet type-checks under its unstated types).
  Applying the queue unchecked would have introduced four regressions while
  fixing the rest.
- `unsafe-pointer-provenance` had the provenance model backwards: it said
  `wrapping_add`/`wrapping_sub` lose provenance. The standard library documents
  the opposite — the result "remembers" the allocation `self` points to. What
  those methods actually change is that the arithmetic stays defined outside
  the allocation.
- `api-record-checksum`'s own Good example panicked on the truncated record its
  test list requires it to reject; `async-cancel-safety` misstated tokio's
  guarantee and hot-spun forever at EOF; `test-fixture-raii` called
  `env::set_var` unwrapped, which is E0133 in the 2024 edition this library
  targets.

### Changed
- `checks/analyze.py` no longer classifies E0658 as a name-resolution error.
  "Use of unstable library feature" means the example does not build on the
  stable toolchain this library pins — a reviewable fact, not a missing name —
  and bucketing it as a fragment meant a nightly-only example could ship
  invisibly. The one real instance is now a visible baseline entry, and the
  `fn f(...)` pseudocode that parses as a C-variadic is classified as the
  extraction artifact it is.

### Changed
- The RustTraining ledger's `semantic_status` is now derived from its rows
  rather than pinned to a constant. The literal `"unreviewed-backlog"` outlived
  its own truth: it kept asserting that no unit had been semantically reviewed
  while most of the ledger had been, and the check held the stale claim in
  place instead of catching it. Its `reason` prose said the same false thing
  and is now written from the counts.

### Added
- `conc-pattern-choice`, the decision the library had no rule for: which
  concurrency architecture to use. `Arc<Mutex<T>>` is usually not chosen at
  all — it is the nearest primitive, so it becomes the design, and every later
  problem is a lock problem. Gives the criterion that settles it: if two
  concurrent updates commute, the threads never needed each other's partial
  results and a lock buys only serialization; if they do not, exclusion is
  required and owner-versus-mutex becomes a measurement. Covers the cases
  where shared memory genuinely earns its lock (reader-dominated access,
  disjoint subranges of one large structure, per-message cost above the work)
  and the diagnostic that a worker pool is the wrong shape once threads need
  distinct roles.
- `conc-lock-reentry`, chosen from evidence rather than intuition. A study of
  59 blocking bugs in production Rust found every one occurred in safe code
  calling synchronization APIs, and 30 of them were double-acquisitions from
  misunderstanding guard lifetime — the single most concentrated concurrency
  defect measured, and one this library had no rule for. Covers one
  acquisition per public entry point, helpers that take the locked data so
  they cannot re-acquire, the guard produced in a `match` scrutinee that is
  held for every arm, and a single global order where several locks are held.
- `err-panic-handler-policy`: a freestanding `#[panic_handler]` is a policy
  decision, not boilerplate. `loop {}` — the shape everyone reaches for —
  holds the core at 100% forever and reports nothing, which on a battery
  device is a flat battery and in the field is an undiagnosable hang. Covers
  reporting before halting, choosing the halt from the device's power and
  recovery model, allocating nothing in the handler, and the fact that
  `PanicInfo` (handler) and `PanicHookInfo` (hook) have been separate types
  since Rust 1.81.
- `unsafe-inline-asm`: the operand-direction, clobber, and `options(...)`
  contract for `asm!`. The compiler cannot read the assembly, so it acts on
  whatever the block promises — a wrong `nomem` or `preserves_flags` is a
  miscompilation that appears only at some optimization levels, with no
  address to inspect and nothing for a sanitizer to trap on.
### Changed
- `perf-global-allocator` now covers the freestanding case. Everything it said
  before was about *choosing* an allocator when a default exists; on a bare
  target there is none, and the allocator you supply owns a heap you hand it
  explicitly. That adds an ordering hazard a hosted program never has — the
  allocator is a `static` from program start, but its heap is unusable until
  `init` runs, and an allocation in between is undefined behaviour rather than
  a clean failure. This closes the last named gap from the no_std surveys.
- `type-generic-bounds` now covers the implicit `Sized` bound every type
  parameter carries, and when relaxing it with `?Sized` is right: a wrapper
  that only holds its `T` behind a pointer is otherwise unusable with exactly
  the types such a wrapper exists for. Keep the bound whenever the code stores
  or returns a `T` by value.
- `mem-assert-type-size` now covers wide pointers costing two words. Swapping
  a concrete type for a trait object is a layout change, not only a dispatch
  change — though `Option<Box<dyn Trait>>` still fits in two words, because
  the null niche absorbs the discriminant.
- `async-poll-contract` now covers `poll_next`. A hand-written `Stream` owes
  every obligation a `Future` does, plus a state machine: `Ready(Some(_))`
  says nothing about the next item, and `Ready(None)` is terminal the way
  `Future`'s `Ready` is. The named failure is a stream that decides it is
  finished without having registered a waker for that decision, parking a
  consumer one item short.
- `conc-thread-channel`: bound a thread-to-thread channel and treat sender
  disconnection as the shutdown signal. Found by a disposition agent that
  refused to map the channels chapter to any existing rule — every channel
  rule in the library was tokio/async, and `async-mpsc-queue`'s Bad example is
  literally `std::sync::mpsc`, so a plain threaded program had no channel
  guidance at all.
### Changed
- Dispositioned 1,053 of the 1,265 unreviewed PDF-corpus units (99 of 100
  chapters across 8 books), taking that ledger to 552 covered / 310
  project-specific / 242 reject / 9 documented-deviation / 212 unreviewed.
  Provenance strengthened again: 318 rules ledger-mapped, 109 justified.
- Black Hat Rust's concluding chapter is recorded as a `documented-deviation`:
  it advises avoiding lifetime annotations and reaching for `Rc`/`Arc` for
  long-lived references instead, which contradicts `own-lifetime-elision`,
  `own-arc-shared`, and `own-borrow-over-clone`.
- `checks/validate.py` now allows `tutorial-scaffolding` and
  `project-specific-detail` as PDF-corpus rationale classes. That set was
  written when the corpus was only mined for rule material; dispositioning it
  completely means classifying prefaces, indexes, and the books' own demo
  projects, which are neither obsolescent nor provider-specific.
- Dispositioned 789 of the 1,235 unreviewed RustTraining units (68 chapters),
  taking the ledger to 654 covered / 674 project-specific / 340 reject /
  10 documented-deviation / 446 unreviewed. 31 rules moved from written
  justification to ledger-mapped provenance (314 mapped, 113 justified).
- The engineering-book release-profiles chapter is recorded as a
  `documented-deviation`, not as coverage: it presents
  `lto = true` + `codegen-units = 1` + `strip = true` + `panic = "abort"` as
  the production profile, which `perf-release-profile`, `opt-lto-release`, and
  `opt-codegen-units` each print as their Bad example. The library
  deliberately disagrees, and the ledger now says so.
- Surveyed the three cross-language migration books (c-cpp, csharp, python —
  545 units) for rule material. Result: nothing. Recorded rather than assumed.
- `type-lifetime-branding` from the RustTraining rust-patterns-book: mint a
  unique invariant lifetime with a `for<'brand> FnOnce` bound so a handle from
  one collection cannot type-check against another. The invariant marker
  (`PhantomData<*mut &'brand ()>`) is load-bearing — the covariant
  `PhantomData<&'brand ()>` compiles the cross-instance mix and enforces
  nothing, verified against rustc both ways.
### Changed
- `type-deref-coercion` carried an error: its "Legitimate Uses" list endorsed
  `Deref` for newtypes whose purpose is adding invariants, which is the case
  where it is most dangerous. `DerefMut` there is an outright hole (assignment
  bypasses the constructor) and plain `Deref` surfaces the inner type's whole
  API, growing with each std release. Corrected, with the accurate alternative
  (inherent accessor, `AsRef`, explicit delegation).
- `mem-zero-copy` now covers the borrow-versus-transform split: a JSON string
  containing an escape has no contiguous run to borrow, so `&'de str` fails
  outright — and `Cow<'de, str>` only borrows with `#[serde(borrow)]`, without
  which it allocates unconditionally while looking zero-copy.
- `perf-iter-lazy` now covers `take_while` consuming the element that stopped
  it — the boundary item is dropped from the output *and* from the source
  iterator.
- Three gaps from surveying the RustTraining async-book and engineering-book
  via a ten-chunk Grok 4.6 xhigh pass: `async-completion-owned-buffer`
  (completion I/O — io_uring, IOCP, RDMA — cannot honestly implement the
  readiness `AsyncRead`/`AsyncWrite` traits, because the kernel owns the
  buffer past the borrow `poll_read` ends; take it by value and hand it back
  with the result), `proj-build-script-scope` (a `build.rs` configures only
  its own package unless it declares `links`, and must decide from the target
  rather than from what it finds installed on the build machine), and
  `test-cross-target-execution` (a green `cargo test` proves the host build
  works; a cross-compiled target needs a configured runner, and a `no_std`
  library still tests on the host because the harness links `std`).
### Changed
- `async-no-lock-await` now states that releasing the lock around `.await`
  can itself be the bug: the rule's own "clone out, process, update" pattern
  is a check-then-act race whenever the second half depends on state the
  first half observed, with three ranked resolutions. Also extended:
  `err-context-chain` (`.context()??` on a `JoinHandle` or `timeout` labels
  the outer `JoinError`/`Elapsed`, never the inner error),
  `async-join-parallel` (concurrency does not require `'static` — only
  `spawn` does; `LocalSet` drops `Send`, not `'static`), `test-sanitizers`
  (ASan does not catch uninitialized reads, sanitizers need `-Zbuild-std`,
  Miri cannot interpret C, coverage instrumentation must not be combined with
  a sanitizer), `unsafe-pin-projection` (the three-way combinator tradeoff,
  and that `F: Unpin` says nothing about `F::Output`),
  `api-auto-trait-contract` (auto traits do not propagate through associated
  types), `proj-dependency-policy` (an advisory scan is not a code review),
  and `proj-libc-floor` (the Windows MSVC/GNU CRT split).
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

### Removed
- `unsafe-dropck-phantom`. Its premise was false: it claimed a struct holding
  `*mut T` with a `Drop` impl is invisible to drop-check and needs
  `PhantomData<T>`. Non-parametric dropck (RFC 1238) already requires every
  generic parameter of a `Drop` type to strictly outlive it — both versions
  compile to the identical `E0597`. The marker also changes nothing about
  variance (`*mut T` is already invariant) or auto traits (already `!Send`).
  It is load-bearing only under nightly `#[may_dangle]`, which the rule never
  mentioned. Written two days ago; removed rather than patched.
### Fixed
- The remaining 28 medium-severity audit findings, across 18 rules. Four were
  verified by compiling first: `macro-export-crate-path`'s Good example did
  not compile (`pub use greet;` beside `#[macro_export]` is E0255),
  `serde-deny-unknown-fields` claimed a typo on a *required* field is silently
  ignored (serde reports `missing field`), `own-split-borrow-fields` labelled
  two sequential `&mut self` calls as not compiling (NLL ends each borrow at
  return), and `doc-question-mark` claimed rustdoc wraps a mainless example in
  a `Result`-returning function (it does not — `?` fails with E0277 without
  `Ok::<(), E>(())`). Several others were the same self-contradiction shape as
  `type-deref-coercion`: `lint-warn-suspicious` told readers to set lint levels
  in `clippy.toml`, which does not accept them; `lint-warn-complexity` and
  `lint-clippy-nursery-selected` each listed a lint from the wrong Clippy group
  in a rule whose subject is enabling that group; and `doc-inline-reexport`'s
  Good example demonstrated the exact redundancy its Bad section condemns.
- Nine more audit findings, each verified by compiling or running it rather
  than taken on report. Two were the remaining high-severity ones:
  `closure-disjoint-capture` said `move` captures the whole named place (RFC
  2229 applies to `move` closures since edition 2021 — the untouched field
  stays usable), and `async-cancellation-token` said child tokens are
  auto-cancelled when the parent is dropped (they are not; only
  `parent.cancel()` propagates). Also `test-use-super` ("can't access private
  items" — a child test module can), `async-async-fn-bounds` (`AsyncFn` is in
  the prelude for every edition, not just 2024), `name-acronym-word` (cited
  `std::io::IoError`, which does not exist), `const-generics` (defaults
  stabilized in 1.59, not 1.65), `unsafe-pointer-provenance`
  (`from_exposed_addr` is gone; the stable name is `with_exposed_provenance`),
  `api-clap-parser-contract` (`get_matches` is on `Command`, not
  `ArgMatches`), and `conc-signal-handler-safety` (Rust's std sets `SIGPIPE`
  to `SIG_IGN` before `main`, so a write to a closed pipe returns `EPIPE`
  rather than killing the process).
- Correctness audit of all 428 rules, ten agents, every acted-on finding
  re-verified by compiling or running it. Notable: `ffi-wasm-wire-abi`
  returned a Rust tuple across `extern "C"` (not FFI-safe — the rule's own
  thesis violated by its Good example); `own-slice-over-vec` claimed `&str`
  coerces to `&Path` (E0308); `serde-rename-all` documented `FOOBAR` where
  serde produces `FOO_BAR`, a wrong wire contract; `test-tokio-async` called
  bare `#[tokio::test]` multi-threaded (it is current_thread — asserted one
  worker); `lint-pedantic-selective`'s recommended config hard-errors on
  `lint_groups_priority`; `test-snapshot-testing` suggested an `INSTA_UPDATE`
  mode for CI that can rewrite a committed golden and pass;
  `type-generational-handle` cited `slab`, which has no generations and is the
  pattern the rule forbids; `type-case-insensitive-match`'s Good example
  folded the data its own summary says not to fold; plus `const-vs-static`,
  `proj-avoid-statics`, `pat-matches-macro`, `api-must-use`, `api-impl-into`,
  `err-context-chain`, and `lint-workspace-lints`.
- Two shipped rules asserted something false. `api-upload-serving` claimed a
  `Content-Disposition` value contained no quotes when it always contains the
  two that delimit the filename, and `unsafe-byte-slice-cast` asserted a stack
  array was misaligned, which depends on where the allocator put it — the very
  thing `test-env-independent` forbids. Both compiled cleanly, so the gate
  never saw them.
### Changed
- `checks/check.sh` now executes every generated example that carries an
  assertion (201 of them) instead of only type-checking it. `cargo check`
  cannot catch a false claim in a passing compile; this closes that hole
  permanently.
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
