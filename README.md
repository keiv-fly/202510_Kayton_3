# Kayton Language Workspace

Kayton is a hybrid ahead-of-time (AOT) and bytecode language pipeline. The project is built around a
Rust workspace that hosts the compiler, runtime, tooling, and supporting crates described in the
design documents under [`design/`](design/).

## Repository Layout

- `Cargo.toml` – workspace definition for all crates, starting with the `xtask` developer tooling.
- `rust-toolchain.toml` – pins the Rust toolchain to the latest stable release and installs required
  components (`rustfmt`, `clippy`).
- `xtask/` – helper binary that wraps common development workflows.
- `design/` – language and architecture design references.

Future phases will introduce the crates outlined in `design/20251004_starting_design_main.md` and the
roadmap in `design/20251008_roadmap.md`.

## Getting Started

1. Install the Rust toolchain declared in `rust-toolchain.toml`. The project expects the following
   cargo utilities to be available on your PATH:
   - [`cargo-nextest`](https://nexte.st/)
   - [`cargo-insta`](https://insta.rs/docs/cli/)
   - [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov)
2. Run `cargo xtask fmt` to format the workspace.
3. Run `cargo xtask lint` to execute `clippy` with warnings treated as errors.
4. Run `cargo xtask dev:check` for a fast validation build of the workspace.

Each `xtask` command forwards to the corresponding `cargo` command so they can also be used directly.

## Roadmap

Execution of the Kayton language system follows the phased implementation strategy documented in
[`design/20251004 steps.md`](design/20251004%20steps.md). Phase 0 (this change) prepares the
workspace scaffolding, CI entry points, and contribution guidelines. Subsequent phases will add the
front-end, bytecode fast path, AOT rewriting, runtime integration, and developer experience tooling.

