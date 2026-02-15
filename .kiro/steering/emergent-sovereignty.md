---
inclusion: always
---

# Emergent Sovereignty — Steering Guide

## Project Overview

This is a headless, high-concurrency Rust deep biological/physical simulation built as a cellular automaton. All macro-structures (species, ecosystems, symbiotic networks) emerge purely from micro-interactions between physical actors. There are no top-down abstractions.

## Design Pillars

### 1. Biological Physicality

The atomic unit of simulation is the `Actor` — a single biological organism or "blob." All state resides in actors as physical components.

- There are NO abstract health bars, stats, or top-down `Species`, `Nation`, or `TechTree` structs. Any code introducing abstract hit points or a static species class must be rejected.
- Every Actor is a collection of physical components (e.g., `Membrane`, `Nucleus`, `EnergyReserves`, `Sensors`).
- Damage is local and physical. Instead of "taking 10 damage," an actor "loses membrane integrity," which affects its ability to filter nutrients or resist environmental stress.
- Relationships between actors are physical: `Symbiosis` (mutualistic exchange) and `Parasitism` (one-sided extraction). These replace the old political sovereignty model.
- Borders do not exist as geometric boundaries. Territorial zones emerge from spatial repulsion based on chemical identity signals between actors.
- When asked to implement a "Border System," implement it as spatial repulsion driven by chemical signal divergence between neighboring actors.

### 2. Metabolic Realism

Economics is complex metabolism. There is no abstract currency or global exchange.

- "Trade" is the physical exchange of chemical compounds or biomass between Actors. There is no global exchange, no central price oracle, no shared order book.
- Value is determined by biological need: an actor low on Nitrogen values it more than one that is satiated. Subjective valuation derives from internal chemical state.
- Exchange only occurs between actors within sensory/contact radius of each other.
- Each actor maintains its own metabolic priorities based on local scarcity, internal reserves, and memory of past interactions.

### 3. Environmental Depth

The environment is an active participant, not a passive backdrop.

- The world is a grid of cells. Each cell holds persistent state: `ChemicalGradients`, `Heat`, and `Moisture`.
- Actors interact with tiles through `Diffusion` (passive chemical exchange with the environment) and `Consumption` (active extraction of resources from a tile).
- Environmental state changes over time: chemicals diffuse between tiles, heat radiates, moisture evaporates. These processes run as independent systems.

### 4. Non-Omniscient Simulation

- No "god-view" logic. Actors only perceive and react to data within their local sensory radius.
- Information propagation is physical: it travels actor-to-actor via chemical signals, decaying and mutating as it spreads.
- Before writing any system logic, ask: "Could this logic be moved from a global system into an actor-to-actor interaction?"

## Architecture & Code Standards

### ECS Pattern (Data-Oriented Design)

- Use an Entity Component System architecture. Components are plain data structs. Systems operate on component queries.
- Favor contiguous memory layouts (SoA over AoS where possible).
- Design hot-path loops (metabolism, diffusion, movement, perception) to be SIMD-friendly.

### Memory

- Strict ownership. No `Rc`/`Arc` unless concurrency demands it.
- Use `SmallVec` or `Slab` for actor inventories and local collections to minimize heap fragmentation.
- Avoid `HashMap` in hot paths; prefer indexed `Vec` or slot-map patterns.

### Concurrency

- All systems must be thread-safe to leverage multi-core ARM (Apple Silicon).
- Prefer data parallelism over task parallelism. Partition the world spatially for parallel system execution.
- Minimize lock contention. Use double-buffering or message-passing for cross-actor interactions.

### Safety

- Minimize `unsafe` blocks. Lean on the borrow checker for actor-interaction safety.
- If `unsafe` is required, it must be documented with a `// SAFETY:` comment explaining the invariant.

## Code Review Checklist

When reviewing or generating code, verify:

1. No static `Species`, `Nation`, or equivalent top-down classification struct exists.
2. No abstract health bars or stat points. All damage/healing is expressed as changes to physical components.
3. All actor behavior is driven by local perception, not global state queries.
4. Resource exchange is computed locally between two actors via metabolic need, not read from a shared source.
5. Inventories use `SmallVec` or `Slab`, not `Vec<Box<...>>` or heap-heavy collections.
6. Hot-path data is laid out contiguously for cache efficiency.
7. Any new "system" could not reasonably be expressed as an actor-to-actor interaction instead.
8. `unsafe` blocks have `// SAFETY:` justification comments.
9. Environment tiles carry physical state (`ChemicalGradients`, `Heat`, `Moisture`) and are not inert.

## Key Domain Concepts

| Concept | Deep Sim Implementation |
|---|---|
| Actor | The atomic blob. ECS entity with physical components (Membrane, Energy, Sensors). |
| Species | Emergent clustering of shared genetic/identity markers; not a hardcoded class. |
| Sovereignty | Physical dominance or mutualistic symbiosis between Actors. |
| Memory | A buffer of past physical interactions (e.g., "This actor gave me Glucose"). |
| Technology | Epigenetic "unlocks" or behavioral patterns passed via observation/mimicry. |
| World | A grid of cells with persistent chemical and thermal state. |
