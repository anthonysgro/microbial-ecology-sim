# Implementation Plan: Trait Stats Standard Deviation

## Overview

Add a `std_dev: f32` field to `SingleTraitStats`, compute it in `compute_single_stats`, and display it in the stats panel. Three files modified, no new files.

## Tasks

- [x] 1. Add `std_dev` field to `SingleTraitStats` and compute it
  - [x] 1.1 Add `std_dev: f32` field to `SingleTraitStats` in `src/viz_bevy/resources.rs`
    - Append `pub std_dev: f32` after the `p75` field
    - _Requirements: 1.1_
  - [x] 1.2 Extend `compute_single_stats` in `src/viz_bevy/systems.rs` to compute population std dev
    - After `mean` is computed, add a second pass: `let variance = values.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / n as f32; let std_dev = variance.sqrt();`
    - Add `std_dev` to the returned `SingleTraitStats` struct literal
    - _Requirements: 1.2, 1.3_
  - [ ]* 1.3 Write property test for std dev computation correctness
    - Add `proptest` dev-dependency if not present; create test in `src/viz_bevy/systems.rs` or a dedicated test module
    - Generate random `Vec<f32>` (len 1..200, values in `-1e6..1e6`), call `compute_single_stats`, compare `std_dev` against naive reference `sqrt(sum((x-mean)^2)/n)` within tolerance
    - **Property 1: Standard deviation computation matches reference implementation**
    - **Validates: Requirements 1.2**

- [x] 2. Display `std_dev` in the stats panel
  - [x] 2.1 Update `format_trait_stats` in `src/viz_bevy/setup.rs` to include `std_dev` in each trait row and the energy row
    - Append `std: {:>6.2}` with `s.std_dev` to the existing `writeln!` format strings for trait rows and the energy row
    - _Requirements: 2.1, 2.2, 2.3_
  - [ ]* 2.2 Write property test for formatted output containing std_dev
    - Generate random `TraitStats` with populated traits/energy, call `format_trait_stats`, verify every data row contains `std: ` followed by a two-decimal-place number
    - **Property 2: Formatted stats output includes std_dev for all rows**
    - **Validates: Requirements 2.1, 2.2, 2.3**

- [x] 3. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Update documentation
  - [x] 4.1 Update `SingleTraitStats` description in `config-documentation.md` steering file to document the new `std_dev` field
    - _Requirements: 3.1_

- [x] 5. Final checkpoint
  - Ensure all tests pass and `cargo clippy` is clean, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The computation is COLD path — a second pass over the values buffer is acceptable
- `proptest` is the recommended PBT library for Rust
- No simulation logic is modified; changes are confined to `viz_bevy`
