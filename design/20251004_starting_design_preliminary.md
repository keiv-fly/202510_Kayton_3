Awesome — here’s the **single, consolidated spec** with all amendments:

* HIR stays exactly as defined conceptually (desugared, source-shaped, IDs, hints), **but its code lives inside the unified front-end crate** together with the lexer and parser.
* The AOT path runs a heavy “interpolation/specialization” step that can also **re-emit Kayton source** (Kayton+) and only exists on the AOT route.
* The bytecode path stays **latency-first** and never waits for deep analysis.

I’ve removed the HIR core-structures section as you asked and replaced it with a clear **folder/crate layout** and **data-flow** that reflect the new design.

# Workspace layout

```
kayton/
├─ Cargo.toml
├─ rust-toolchain.toml
├─ xtask/                         # optional dev tasks (codegen, release, etc.)
│  └─ src/main.rs
│
└─ crates/
   ├─ kayton-front/               # ← unified front-end: lexer + parser + HIR
   │  ├─ src/
   │  │  ├─ lexer.rs
   │  │  ├─ parser.rs
   │  │  ├─ hir.rs                # HIR defs live here (not in a separate crate)
   │  │  ├─ lowering.rs           # surface → HIR desugaring
   │  │  ├─ spans.rs
   │  │  ├─ symbols.rs            # interner
   │  │  └─ lib.rs                # public facade: parse → HIR
   │  └─ Cargo.toml
   │
   ├─ kayton-sema/                # FastSema (VM) + DeepSema (AOT)
   │  ├─ src/
   │  │  ├─ fast.rs               # shallow typing, name res, lite const-fold
   │  │  ├─ deep.rs               # whole-program constraints, monomorphization, devirt
   │  │  ├─ constraints.rs
   │  │  ├─ types.rs
   │  │  ├─ traits.rs
   │  │  ├─ effects.rs            # (optional AOT-only effect system)
   │  │  ├─ diagnostics.rs
   │  │  └─ lib.rs
   │  └─ Cargo.toml
   │
   ├─ kayton-rewriter-aot/        # AOT-only: HIR + DeepSema → HIR′ or Kayton+ text
   │  ├─ src/
   │  │  ├─ insert_types.rs
   │  │  ├─ specialize.rs         # monomorphization
   │  │  ├─ devirt.rs             # trait/interface → direct calls
   │  │  ├─ lower_advanced.rs     # refined types/effects → core Kayton
   │  │  ├─ pretty.rs             # emit Kayton+ source (optional output)
   │  │  └─ lib.rs
   │  └─ Cargo.toml
   │
   ├─ kayton-bytecode/            # ISA, constant pool, verifier, (de)serializer
   │  └─ src/...
   │
   ├─ kayton-emitter-bc/          # HIR + FastSema → bytecode module
   │  └─ src/...
   │
   ├─ kayton-vm/                  # bytecode interpreter (HPy-style handles to host)
   │  └─ src/...
   │
   ├─ kayton-backend-rust/        # HIR′ + DeepSema → Rust crates (AOT)
   │  └─ src/...
   │
   ├─ kayton-abi/                 # HPy-style C ABI (context + handles + vtable)
   │  └─ src/...
   │
   ├─ kayton-api/                 # Safe Rust wrappers over ABI (RAII handles, Result, etc.)
   │  └─ src/...
   │
   ├─ kayton-plugin-macros/       # #[kayton_extension] proc macro for plugins
   │  └─ src/...
   │
   ├─ kayton-host/                # dyn loader/registry for extensions (libloading)
   │  └─ src/...
   │
   ├─ kayton-stdlib/              # std extensions (build as rlib + dylib)
   │  └─ src/...
   │
   ├─ kayton-compiler/            # library driver (parse → choose path → build)
   │  └─ src/lib.rs
   │
   ├─ kayton-cli/                 # `kayton` binary (run VM, AOT build, rewrite, inspect)
   │  └─ src/main.rs
   │
   └─ kayton-testing/             # golden tests, fuzzers, benches
      └─ src/...
```

# Data flow

```
source.ktn
  │
  ▼
[kayton-front]                  # lexer + parser + HIR lowering (desugared, ID'd, interned)
  │
  ├─► Bytecode path (latency-first)
  │     [kayton-sema::fast]     # names, trivial typing, simple const-fold
  │        │
  │        └─► [kayton-emitter-bc] → [kayton-bytecode] → .kbc → [kayton-vm]
  │               • pick specialized ops only when obvious
  │               • otherwise emit generic/dynamic ops
  │               • CALL_HOST (slot or dynamic) via handles
  │
  └─► AOT path (uncertainty crusher)
        [kayton-sema::deep]     # whole-program constraints, mono, devirt, effects, inlining plan
           │
           └─► [kayton-rewriter-aot]
                 • insert explicit types & generics
                 • specialize/monomorphize defs
                 • devirtualize trait/interface calls
                 • lower AOT-only features to core Kayton
                 • output: HIR′ (preferred)  or  Kayton+ source (pretty)
                      │
                      └─► [kayton-backend-rust] → cargo → native binary
                           • prefer direct Rust calls to std/plugins (rlib)
                           • fall back to ABI for dynamic plugins
```

# Path policies

## Bytecode (fast)

* **Never** wait for deep analysis.
* Use **FastSema** only; if a type isn’t trivially known, emit **generic ops** (`AddDyn`, `CallDyn`, `IndexDyn`, …).
* Host calls always via **ABI handles** (`CALL_HOST slot` if resolvable; else `CALL_HOST_DYNAMIC`).
* Optional lint: `warn(aot_only_feature)` or `deny(aot_only_feature)`.

## AOT (powerful)

* **DeepSema** performs whole-program inference, trait resolution, monomorphization, devirtualization, and inlining/const-prop.
* **kayton-rewriter-aot** materializes results:

  * Can re-emit **Kayton+ source** (for caching, review, diffs).
  * Or pass **HIR′** directly to the Rust backend (no re-parse).
* **kayton-backend-rust** emits concrete, fully typed Rust with direct calls where possible.

# CLI UX

```
kayton run main.ktn
  # front → fast sema → bytecode → vm

kayton aot rewrite main.ktn -o main.special.ktn
  # front → deep sema → rewriter → Kayton+ source

kayton aot build main.ktn
  # front → deep sema → rewriter (HIR′) → rust backend → cargo build

kayton explain types --site <hir_id>
  # dump deep constraints/solution and show the rewrite at that site
```

# Responsibilities (quick map)

* **kayton-front**: tokenization, parsing, **HIR construction & desugaring**, symbol interning, spans, stable IDs. (No heavy semantics here.)
* **kayton-sema**:

  * `fast`: names, trivial typing, simple consts, quick host resolution.
  * `deep`: whole-program constraints, generics → mono, traits → direct calls, effects, const-prop, inline/devirt plans.
* **kayton-rewriter-aot**: rewrites **HIR + DeepSema** into **HIR′** or **Kayton+ source** with explicit types/specializations and AOT-only features lowered to the core language.
* **kayton-emitter-bc**: latency-first lowering to bytecode (specialized ops only when obvious).
* **kayton-vm**: fast generic dispatch for dynamic ops, HPy-style handle marshalling for host calls (with inline caches).
* **kayton-backend-rust**: prints concrete Rust for HIR′; directly links std/plugins as `rlib` when available; uses ABI otherwise.
* **kayton-abi / kayton-api / kayton-host / kayton-plugin-macros**: HPy-style context + handles, safe wrappers, dynamic loader, and macros for plugins.

# AOT-only features handling

* Parsed and preserved in **HIR annotations** within `kayton-front`.
* **Ignored or linted** in bytecode mode.
* **Consumed and lowered** by `kayton-rewriter-aot` after `DeepSema`:

  * refined types / effect annotations → core forms
  * trait bounds / typeclass sugar → concrete impls
  * compile-time eval hints → constants/inlines where legal

# Caching & reproducibility

* Cache **DeepSema** summaries per module to speed up AOT rebuilds.
* Optionally store **Kayton+** output as a build artifact for debugging or distribution.
* Deterministic HIR node ordering & stable `HirId` make golden tests stable.

---

If you want, I can also draft the `Cargo.toml` stanzas for each crate (with dependencies wired up), plus a minimal `kayton-front::parse_to_hir()` facade and two `kayton-cli` subcommands (`run`, `aot rewrite`) so you can `cargo build` and fill in the internals incrementally.
