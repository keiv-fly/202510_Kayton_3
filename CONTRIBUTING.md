# Contributing to Kayton

Thanks for your interest in contributing! This document summarizes how to set up your environment,
run the automated checks, and align work with the Kayton roadmap.

## Prerequisites

- Install the Rust toolchain defined in [`rust-toolchain.toml`](rust-toolchain.toml).
- Install the auxiliary cargo tools used by CI and local workflows:
  - `cargo-nextest`
  - `cargo-insta`
  - `cargo-llvm-cov`
- Ensure `cargo fmt` and `cargo clippy` are available (installed via the toolchain file).

## Workflow

1. Fork the repository and create a feature branch.
2. Run `cargo xtask fmt` before committing to keep formatting consistent.
3. Run `cargo xtask lint` to execute `clippy` with warnings treated as errors.
4. Run `cargo xtask dev:check` to make sure the workspace builds.
5. Add tests appropriate to your change. Later phases of the project introduce golden tests,
   integration tests, and fuzzers—follow the relevant design documents in `design/` for guidance.
6. Open a pull request and ensure CI passes.

## Roadmap Alignment

Development follows the phased plan in [`design/20251004 steps.md`](design/20251004%20steps.md). Each
phase builds on the previous one and has a clear definition of done. Before starting work, confirm
which phase or milestone your change supports and update any relevant documentation.

## Code Style and Lints

- The workspace forbids `unsafe` code by default. If you need to use `unsafe`, include a detailed
  justification in the pull request and add localized lint exceptions.
- Prefer small, focused commits with descriptive messages.
- Keep public API changes documented in the appropriate design or documentation files.

## Reporting Issues

Please open an issue with as much context as possible: reproduction steps, expected behavior, actual
behavior, and environment details. For security or conduct-related issues, contact the maintainers at
`conduct@kayton.dev`.

We appreciate your help in building Kayton!
