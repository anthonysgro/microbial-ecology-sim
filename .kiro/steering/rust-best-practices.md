# Rust Best Practices — Steering Guide

This guide enforces strict, opinionated Rust standards for a high-performance, headless biological simulation engine. It targets Rust 1.75+, ECS architecture, Apple Silicon multi-core, and deterministic simulation. All code generation and review must comply.

---

## 1. Language Standards

- Enforce idiomatic Rust. No C-style patterns, no Java-isms.
- Composition over inheritance. No deep trait hierarchies.
- Use explicit lifetimes when they improve readability at API boundaries. Elide them in internal code where the compiler infers correctly.
- Avoid `dyn Trait` in hot paths. Use monomorphization via generics or enums for dispatch.
- Use `#[derive(Debug, Clone, Copy, PartialEq)]` where appropriate. Derive `Default` only when a meaningful zero-state exists.
- Use `thiserror` for all domain error types. Each module defines its own error enum.
- Use `anyhow` only at application boundaries (`main.rs`, CLI, I/O entry points). Never in core simulation logic.
- Prefer `impl Trait` in argument position over trait objects for private APIs.
- Avoid orphan impls. Keep trait implementations co-located with either the trait or the type.

## 2. Memory & Allocation Discipline

- Zero heap allocations in hot paths. If a hot loop allocates, it is a bug.
- All `Vec` used in `HOT` systems must have capacity reserved before the hot loop via `Vec::with_capacity()` or `Vec::reserve()`. Capacity growth (reallocation) inside a `HOT` system is considered a bug. Use `debug_assert!(vec.capacity() >= expected)` to guard against silent regressions.
- Prefer stack allocation. Use arrays and tuples over `Vec` when size is known at compile time.
- Use `SmallVec<[T; N]>` for collections that are almost always small but may occasionally grow (actor inventories, local neighbor lists).
- Never use `Vec<Box<T>>`. Use `Vec<T>` directly, or a slab/arena allocator.
- Avoid `HashMap` in tight loops. Prefer `Vec` with index-based lookup, `SlotMap`, or `IndexVec`.
- `FxHashMap` is permitted only in order-independent contexts where iteration order does not affect simulation output. Never use it where determinism depends on iteration order.
- When deterministic hashing is required, use `hashbrown::HashMap` with a fixed hasher seed.
- Use `IndexMap` when insertion-order iteration is needed.
- Prefer Structure of Arrays (SoA) over Array of Structures (AoS) in performance-critical systems. Store each component type in its own contiguous `Vec`.
- Never clone large structs. Pass by reference. Use `Arc` only when shared ownership across threads is required.
- Every `unsafe` block must have a `// SAFETY:` comment explaining the invariant it relies on. No exceptions.
- Prefer `MaybeUninit` over zeroing large buffers when initialization is deferred.
- Use `#[repr(C)]` on structs that participate in SIMD operations or FFI.

## 3. Concurrency & Parallelism

- Data parallelism over task parallelism. Partition the grid spatially and process partitions in parallel.
- Use `rayon` for data-parallel iteration (`par_iter`, `par_chunks_mut`). Use explicit thread pools only when `rayon` is insufficient.
- No global locks. No `lazy_static` mutexes guarding shared simulation state.
- Use double-buffering for state transitions: read from buffer A, write to buffer B, swap. This eliminates read-write contention.
- Never use `Rc`. Use `Arc` only when data must be shared across thread boundaries.
- Avoid `Mutex` in hot loops. If synchronization is needed, prefer lock-free patterns, channels (`crossbeam`), or atomic operations.
- All systems must be `Send + Sync`. If a type cannot be `Send + Sync`, justify it in a comment.
- Partition work by spatial locality to maximize cache coherence across cores.
- On Apple Silicon, respect the unified memory architecture: avoid unnecessary copies between "CPU" and "GPU" conceptual boundaries.

## 4. ECS & Data-Oriented Design

- Components are plain data structs. No methods beyond trivial constructors. No trait implementations beyond derives.
- Systems are functions that operate on component queries. They do not own state.
- No global singleton state. No `static mut`. No `OnceCell` holding simulation data.
- Avoid trait hierarchies deeper than one level. Prefer composition of components over polymorphic behavior.
- Memory layout must be flat and predictable. No `Box<dyn Component>` in component storage.
- Hot data (position, velocity, energy) must be tightly packed in contiguous arrays. Cold data (debug info, history logs) lives in separate storage.
- Separate hot and cold fields into distinct structs even within the same logical entity.
- Component access patterns should be designed for sequential iteration, not random access.

## 5. Determinism & Simulation Integrity

- Never iterate over `HashMap` or `HashSet` when order affects simulation output. Use `BTreeMap`, `IndexMap`, or sorted `Vec` instead.
- If an unordered collection is used, add a comment: `// ORDER-INDEPENDENT: iteration order does not affect output because [reason]`.
- All RNG must be seeded. Use `rand::SeedableRng` with a deterministic seed derived from simulation tick and entity ID.
- Avoid floating-point accumulation across threads. If parallel reduction is needed, use integer arithmetic or fixed-point, or accumulate per-thread and merge deterministically.
- Document any intentionally nondeterministic behavior with `// NONDETERMINISTIC: [justification]`.
- Simulation state must be fully reconstructable from a seed + tick number for replay/debugging.


## 6. Performance Standards

- Benchmark hot systems using `criterion`. Every system that runs per-tick must have a benchmark.
- Profile with `cargo flamegraph` before optimizing. No optimization without profiling data.
- Use `#[inline]` only when profiling shows a measurable benefit at a call boundary. Never blanket-inline.
- Use `#[inline(always)]` only for trivial accessors (field getters, index calculations) in hot paths confirmed by profiling.
- Avoid premature micro-optimizations. Correct, clear code first. Optimize the measured bottleneck.
- Use `debug_assert!` for invariant checks in development builds. These compile away in release.
- Use `#[cfg(debug_assertions)]` for expensive validation that should not exist in release builds.
- Prefer iterators for clarity and auto-vectorization. In SIMD-critical inner loops, manual indexing with explicit slice bounds is permitted if profiling shows measurable benefit.
- Avoid branch-heavy code in inner loops. Prefer branchless patterns or lookup tables where profiling justifies it.
- Target 64-byte cache line alignment for frequently accessed contiguous data on Apple Silicon.

## 7. Error Handling & Logging

- Core simulation code must never panic. No `unwrap()`, no `expect()` in simulation logic. Use `Result` for all fallible operations.
- `unwrap()` and `expect()` are permitted only in tests, build scripts, and one-time initialization code with a justifying comment.
- Use `tracing` for structured logging. Gate all logging behind a `logging` feature flag.
- Never allocate strings in hot paths for logging. Use `tracing`'s zero-alloc span/event macros.
- Error types must be `Send + Sync + 'static` for compatibility with threaded execution.
- Propagate errors with `?`. Avoid `match` on `Result` unless you need to transform or recover.
- Log at appropriate levels: `error` for unrecoverable issues, `warn` for degraded behavior, `debug`/`trace` for development diagnostics.

## 8. Module & Project Structure

Clear separation between domains:

```
src/
├── grid/          # Environment grid: cells, chemical gradients, heat, moisture
├── actor/         # Actor components, metabolic systems, sensory systems
├── sim/           # Simulation loop, tick orchestration, scheduling
├── physics/       # Diffusion, spatial partitioning, movement
├── io/            # Serialization, snapshots, replay (application boundary)
├── lib.rs         # Public API surface
└── main.rs        # Entry point, CLI, configuration (anyhow allowed here)
```

- No circular dependencies between modules. Dependency flows downward: `sim` depends on `grid` and `actor`, not the reverse.
- Keep modules small and focused. A module with more than ~500 lines should be split.
- Use `pub(crate)` by default. Expose `pub` only for the library's external API.
- Internal helper functions are `fn` (private). Never `pub` unless consumed outside the module.
- Feature-gate optional functionality (`serde` support, logging, benchmarks).

## 9. Linting & Tooling

Enforce in `Cargo.toml` or CI:

```toml
[lints.clippy]
all = "deny"
pedantic = "warn"
nursery = "warn"
unwrap_used = "deny"
expect_used = "deny"
todo = "deny"
dbg_macro = "deny"
print_stdout = "deny"
print_stderr = "deny"
```

- Run `cargo clippy -- -D warnings` in CI. No clippy warnings in merged code.
- Run `cargo fmt --check` in CI. Use the project `.rustfmt.toml` for consistent formatting.
- Maintain a minimal dependency footprint. Every new dependency must justify its inclusion. Prefer `no_std`-compatible crates where possible.
- Use `cargo audit` in CI to catch known vulnerabilities.
- Use `cargo bloat` periodically to monitor binary size and identify unexpected code generation.
- Pin dependency versions in `Cargo.lock`. Review dependency updates explicitly.

## 10. Code Review Checklist

Every PR and generated code block must pass:

- [ ] No `dyn Trait` in hot paths without profiling justification
- [ ] No heap allocation in per-tick hot loops
- [ ] No `HashMap` iteration where order affects output
- [ ] No `unwrap()` or `expect()` in simulation logic
- [ ] No `Rc` anywhere. `Arc` only with concurrency justification
- [ ] No `Vec<Box<T>>` patterns
- [ ] No global mutable state (`static mut`, `lazy_static` with `Mutex`)
- [ ] All `unsafe` blocks have `// SAFETY:` comments
- [ ] All RNG is seeded and deterministic
- [ ] Components are plain data structs with no business logic methods
- [ ] Systems do not hold state; they operate on queries
- [ ] Hot and cold data are separated into distinct structs
- [ ] SoA layout used for performance-critical component storage
- [ ] Error types use `thiserror`, not string errors or `anyhow` in core
- [ ] No `todo!()`, `dbg!()`, or `println!()` in production code
- [ ] Module dependencies flow in one direction (no cycles)
- [ ] New dependencies are justified and minimal
- [ ] `pub` visibility is intentional, not accidental
- [ ] Logging is feature-gated and zero-alloc in hot paths
- [ ] Floating-point operations are deterministic or documented as nondeterministic


## 11. Hot Path Declaration Policy

Every system must explicitly declare its thermal classification:

| Classification | Description | Constraints |
|---|---|---|
| `HOT` | Runs every tick over large data (all actors, all grid cells) | No dynamic dispatch. No heap allocation. No branching where avoidable. Must be benchmarked. Deterministic execution required. |
| `WARM` | Runs every tick over small data (local neighbors, small queries) | Heap allocation discouraged. Dynamic dispatch discouraged. Benchmark recommended. |
| `COLD` | Runs infrequently (initialization, serialization, config reload) | Standard Rust practices apply. Allocation and dispatch permitted. |

Hot modules must contain a header comment:

```rust
// HOT PATH: Executes per tick over all actors/cells.
// Allocation forbidden. Dynamic dispatch forbidden.
// Deterministic execution required.
```

Rules:

- Any change to a `HOT` system requires benchmark verification before merge.
- Promoting a system from `COLD`/`WARM` to `HOT` requires adding benchmarks and passing the hot path review checklist.
- Demoting a system from `HOT` requires explicit justification.
- Code review must verify that `HOT` systems contain no logging, no string formatting, no `dyn Trait`, no `HashMap`, and no heap allocation.
- If a `HOT` system must call into code that allocates, that call must be extracted into a `COLD` pre-computation phase that runs before the hot loop.


## 12. Simulation Tick Phasing

The simulation tick must be divided into deterministic, non-overlapping phases:

| Phase | Classification | Description |
|---|---|---|
| 1. Input Collection | `COLD`/`WARM` | Gather external inputs, events, and configuration changes. |
| 2. Precomputation | `WARM` | Compute derived data needed by the hot phase (neighbor lists, spatial indices, pre-reserved buffers). |
| 3. Parallel Update | `HOT` | Double-buffered: read from buffer A, write to buffer B. Spatially partitioned for parallel execution. |
| 4. Deterministic Merge | `WARM` | Merge cross-partition messages and resolve conflicts. Order must be deterministic and index-based. |
| 5. Post-Tick Validation | `debug_assertions` only | Invariant checks, conservation law verification, state consistency audits. Compiles away in release. |

Rules:

- No system may mutate state outside its assigned phase.
- The `HOT` parallel update phase must never read from partially written state. Double-buffering enforces this.
- Cross-partition communication must occur via explicit message buffers written during the `HOT` phase and consumed during the merge phase.
- Merge order must be deterministic: process partitions by index, not by thread completion order.
- Buffer swaps occur only at phase boundaries, never mid-phase.
- Adding a new phase or reordering existing phases requires design review and determinism verification.
