Excellent — here’s the **complete, merged design document** that combines **your original spec** and the **new hybrid AOT–VM fallback system** into a single, unified description.
It’s fully self-contained, with **crate layout**, **data flow**, **AOT & bytecode semantics**, **hybrid fallback execution**, and **CLI & runtime details** — no duplication, just integrated clarity.

---

# Kayton Language Compilation and Execution Architecture

*(Unified Spec: Frontend → Sema → Rewriter → Bytecode / AOT / Hybrid)*

---

## Overview

Kayton is a dual-mode language system capable of:

* **Bytecode execution (fast path)** for low-latency dynamic runs.
* **Ahead-of-Time (AOT) compilation to Rust** for high-performance static binaries.
* **Hybrid fallback execution**, where AOT-compiled Rust seamlessly runs embedded **bytecode thunks** when some code cannot be fully lowered to Rust.

The architecture allows both execution modes to share front-end, semantics, and ABI definitions, ensuring consistent runtime behavior and host extension support.

---

## Workspace Layout

```
kayton/
├─ Cargo.toml
├─ rust-toolchain.toml
├─ xtask/                         # optional dev tasks (codegen, release, etc.)
│  └─ src/main.rs
│
└─ crates/
   ├─ kayton-front/               # unified front-end: lexer + parser + HIR
   │  ├─ src/
   │  │  ├─ lexer.rs
   │  │  ├─ parser.rs
   │  │  ├─ hir.rs                # HIR definitions (no separate crate)
   │  │  ├─ lowering.rs           # surface → HIR desugaring
   │  │  ├─ spans.rs
   │  │  ├─ symbols.rs            # interner
   │  │  └─ lib.rs
   │  └─ Cargo.toml
   │
   ├─ kayton-sema/                # FastSema (VM) + DeepSema (AOT)
   │  ├─ src/
   │  │  ├─ fast.rs               # lightweight typing, name res, const-fold
   │  │  ├─ deep.rs               # full inference, mono/devirt, effects
   │  │  ├─ constraints.rs
   │  │  ├─ types.rs
   │  │  ├─ traits.rs
   │  │  ├─ effects.rs
   │  │  ├─ diagnostics.rs
   │  │  └─ lib.rs
   │  └─ Cargo.toml
   │
   ├─ kayton-rewriter-aot/        # AOT-only rewriting: HIR + DeepSema → HIR′ or Kayton+
   │  ├─ src/
   │  │  ├─ insert_types.rs
   │  │  ├─ specialize.rs
   │  │  ├─ devirt.rs
   │  │  ├─ lower_advanced.rs
   │  │  ├─ pretty.rs
   │  │  └─ lib.rs
   │  └─ Cargo.toml
   │
   ├─ kayton-bytecode/            # ISA, constant pool, verifier, serializer
   │  └─ src/...
   │
   ├─ kayton-emitter-bc/          # HIR + FastSema → bytecode modules / thunks
   │  └─ src/...
   │
   ├─ kayton-vm/                  # bytecode interpreter (HPy-style handles)
   │  └─ src/...
   │
   ├─ kayton-rt/                  # AOT runtime shim (embedded VM & ABI bridge)
   │  ├─ src/
   │  │  ├─ lib.rs                # core runtime; feature "embed_vm" optional
   │  │  ├─ abi_bridge.rs         # KayVal <-> Rust marshalling
   │  │  ├─ thunk_registry.rs     # thunk_id → &[u8] bytecode map
   │  │  └─ exec.rs               # exec_bytecode_thunk(thunk_id, args)
   │  └─ Cargo.toml
   │
   ├─ kayton-backend-rust/        # HIR′ + DeepSema → Rust crates (AOT)
   │  └─ src/...
   │
   ├─ kayton-abi/                 # C ABI (context + handles + vtable)
   │  └─ src/...
   │
   ├─ kayton-api/                 # Safe Rust wrappers over ABI
   │  └─ src/...
   │
   ├─ kayton-plugin-macros/       # #[kayton_extension] proc macros for plugins
   │  └─ src/...
   │
   ├─ kayton-host/                # dyn loader/registry for extensions
   │  └─ src/...
   │
   ├─ kayton-stdlib/              # standard library (rlib + dylib)
   │  └─ src/...
   │
   ├─ kayton-compiler/            # library driver: parse → choose path → build
   │  └─ src/lib.rs
   │
   ├─ kayton-cli/                 # CLI: run VM, AOT build, rewrite, inspect
   │  └─ src/main.rs
   │
   └─ kayton-testing/             # golden tests, fuzzers, benches
      └─ src/...
```

---

## Data Flow

```
source.ktn
  │
  ▼
[kayton-front]                  # lexer + parser + HIR lowering
  │
  ├─► Bytecode path (latency-first)
  │     [kayton-sema::fast]
  │        │
  │        └─► [kayton-emitter-bc] → [kayton-bytecode] → .kbc → [kayton-vm]
  │               • specialized ops only when trivial
  │               • generic ops otherwise (AddDyn, CallDyn, …)
  │               • CALL_HOST via ABI handles
  │
  └─► AOT path (uncertainty crusher)
        [kayton-sema::deep]
           │
           └─► [kayton-rewriter-aot]
                 • insert explicit types & generics
                 • specialize/monomorphize
                 • devirtualize trait/interface calls
                 • lower AOT-only features
                 • output: HIR′ or Kayton+ text
                      │
                      └─► [kayton-backend-rust] → cargo → native binary
                           • prefer direct Rust std/plugins (rlib)
                           • fall back to ABI for dynamic plugins
                           • if not lowerable: embed bytecode thunk
```

---

## Hybrid Execution Path (Rust + Bytecode)

When compiling with `--hybrid` (default), the Rust backend embeds bytecode for functions that cannot be fully lowered.

### 1. Split by Lowerability

| Code section      | Action                                        |
| ----------------- | --------------------------------------------- |
| Lowerable to Rust | Generate native Rust code                     |
| Not-lowerable     | Emit bytecode thunk, include in output binary |

### 2. Thunks

* Each thunk corresponds to one function or expression site.
* Stored as static `&[u8]` blobs inside `.rodata.kayton` with an assigned `thunk_id`.
* Registered with the runtime at startup.

### 3. Rust Backend Output

```rust
#[no_mangle]
pub extern "C" fn ktn_user__foo(a: KayVal, b: KayVal) -> KayVal {
    unsafe { kayton_rt::exec_bytecode_thunk(THUNK_ID_FOO, &[a, b]) }
}
```

### 4. Runtime Fallback

* `kayton-rt` provides:

  * `exec_bytecode_thunk(thunk_id, args)`
  * Bytecode registry and host context
  * Optional embedded VM (`feature = "embed_vm"`)
  * Or dynamic VM via `kayton-host` (for smaller binaries)

---

## Path Policies

### Bytecode Path (fast)

* Uses **FastSema** only.
* Generic ops emitted freely.
* Host calls via `CALL_HOST` or `CALL_HOST_DYNAMIC`.
* Never waits for deep inference.
* Optional `warn(aot_only_feature)`.

### AOT Path (full power)

* **DeepSema**: full constraint solving, devirt, inlining, mono, effects.
* **Rewriter** produces HIR′ or readable Kayton+ source.
* **Backend** emits Rust + optional embedded thunks.
* **Fallback**:

  * Functions unlowerable to Rust are compiled to bytecode.
  * Bytecode thunks embedded in binary or loaded dynamically.
  * Calls dispatched through `kayton-rt`.

### Hybrid Modes

| Mode                 | Description                               |
| -------------------- | ----------------------------------------- |
| `--hybrid` (default) | Embed VM and thunks in binary.            |
| `--aot-strict`       | Fail if any thunk generated.              |
| `--hybrid=dynamic`   | Call out to external VM (smaller binary). |

---

## CLI UX

```
kayton run main.ktn
  # front → fast sema → bytecode → vm

kayton aot rewrite main.ktn -o main.special.ktn
  # front → deep sema → rewriter → Kayton+ source

kayton aot build main.ktn
  # front → deep sema → rewriter → backend (hybrid by default)

kayton aot build --aot-strict
  # fail if any bytecode fallback required

kayton aot build --hybrid=dynamic
  # use external VM via kayton-host

kayton aot explain --thunks
  # show sites compiled as bytecode thunks

kayton explain types --site <hir_id>
  # inspect inferred types and rewrite results
```

---

## Responsibilities

| Crate                                 | Purpose                                                                  |
| ------------------------------------- | ------------------------------------------------------------------------ |
| **kayton-front**                      | Tokenization, parsing, HIR, spans, interning, IDs.                       |
| **kayton-sema**                       | `fast`: quick typing. `deep`: whole-program inference.                   |
| **kayton-rewriter-aot**               | Rewrites HIR + DeepSema → explicit HIR′ or Kayton+. Tags fallback sites. |
| **kayton-emitter-bc**                 | Produces bytecode modules / thunks.                                      |
| **kayton-bytecode**                   | Defines ISA, pool, verifier, encoding.                                   |
| **kayton-vm**                         | Fast interpreter, HPy handles, dynamic dispatch.                         |
| **kayton-rt**                         | Runtime bridge for AOT binaries; executes thunks.                        |
| **kayton-backend-rust**               | Emits Rust + stub fallback functions + bytecode bundling.                |
| **kayton-abi/api/host/plugin-macros** | ABI, safe wrappers, dynamic loader, proc macros.                         |
| **kayton-stdlib**                     | Standard library for both modes.                                         |

---

## Runtime and ABI (kayton-rt)

### Core Functions

```rust
pub fn exec_bytecode_thunk(thunk_id: u32, args: &[KayVal]) -> KayVal;
pub fn register_thunk(thunk_id: u32, blob: &'static [u8], name: &'static str);
pub fn set_host_context(ctx: KayHostCtx);
```

### Features

| Feature      | Effect                                          |
| ------------ | ----------------------------------------------- |
| `embed_vm`   | Link `kayton-vm` & `kayton-bytecode` directly.  |
| `dynamic_vm` | Use VM loaded at runtime through `kayton-host`. |

### Initialization

`kayton-rt` auto-registers all thunks and initializes plugin contexts on startup.

---

## AOT-only Features Handling

* Parsed and preserved in HIR annotations.
* Ignored in VM mode.
* Lowered by AOT rewriter:

  * refined/effect types → core forms
  * trait sugar → concrete impls
  * const hints → inline constants
  * unsupported constructs → thunks

---

## Caching & Reproducibility

* Cache DeepSema results for rebuilds.
* Deterministic HIR/Thunk IDs.
* Optionally store Kayton+ output for diffs.
* CI strictness toggle (`--aot-strict`).
* `@lowered` / `@thunk(id)` annotations for debugging.

---

## Example Flow Summary

| Step               | Tool                   | Output                      |
| ------------------ | ---------------------- | --------------------------- |
| Parse & desugar    | `kayton-front`         | HIR                         |
| Fast semantic pass | `FastSema`             | typed HIR (loose)           |
| Emit bytecode      | `kayton-emitter-bc`    | `.kbc`                      |
| Deep semantic pass | `DeepSema`             | resolved constraints        |
| Rewrite (AOT)      | `kayton-rewriter-aot`  | HIR′ or Kayton+             |
| Rust backend       | `kayton-backend-rust`  | Rust code + bytecode thunks |
| Compile to binary  | `cargo`                | hybrid executable           |
| Run                | AOT Rust + embedded VM | full program                |

---

## Example Hybrid Stub

```rust
#[no_mangle]
pub extern "C" fn ktn_user__map_iter(xs: KayVal, f: KayVal) -> KayVal {
    // Thunked implementation (not lowerable to Rust)
    unsafe { kayton_rt::exec_bytecode_thunk(THUNK_ID_MAP_ITER, &[xs, f]) }
}
```

---

## Example Bytecode Bundle

```rust
#[link_section = ".rodata.kayton"]
pub static KBC_SEGMENT_0: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/thunks_0.kbc"));

pub fn __kayton_register_thunks() {
    kayton_rt::register_thunk(1, KBC_SEGMENT_0, "foo");
    kayton_rt::register_thunk(2, KBC_SEGMENT_0, "bar");
}
```

`kayton-rt` calls `__kayton_register_thunks()` during initialization.

---

## Design Principles

1. **Unified front-end:** one parser/HIR used for both AOT and VM.
2. **Latency-first bytecode:** executes with minimal preparation.
3. **Heavy interpolation (AOT-only):** advanced types, generics, effects.
4. **Deterministic builds:** stable IDs and thunk assignment.
5. **Hybrid safety:** Rust binaries always runnable, even with fallback.
6. **Transparent extensibility:** same ABI/API for both modes.
7. **Tool symmetry:** same CLI commands work for run, rewrite, build.

---

## Future Extensions

* Incremental hybrid recompilation (re-emit only affected thunks).
* Thunk caching between builds.
* JIT tier: dynamically replace thunk execution with compiled Rust fragments.
* Pluggable host runtime: swap `kayton-vm` via dynamic ABI at startup.

---

✅ **End of Unified Spec: Kayton Language Architecture (v2 — Hybrid AOT/VM)**

Would you like me to follow up with an **implementation skeleton** for `kayton-rt` (Cargo.toml + `lib.rs` stubs) and `kayton-backend-rust`’s thunk generator (build.rs snippet)?
