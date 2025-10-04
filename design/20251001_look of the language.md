Absolutely — here’s the **updated, exhaustive spec** with your amendment applied:

> **Amendment:** *No `discard` keyword.*
> If a block/branch should evaluate to `()`, you **end the block with a new line containing only `()`**. For non-final side-effects that would have used `;`, write `let _ = expr`.

---

# 📜 Semicolon-Free Rust (Revised Spec)

## 0) Design goals

* Preserve Rust semantics (ownership, borrowing, types).
* No JavaScript-style ASI; layout is deterministic.
* Keep Rust expression orientation: blocks return their last expression.
* Concise inline conditional (two-branch only).
* Compatible with macros and tooling.

---

## 1) Lines, continuation, and suites

### 1.1 Statement boundaries

* **Newline ends a statement**, unless a **continuation condition** applies.

### 1.2 Continuation conditions

A physical line **continues** if any of these hold:

* Inside unmatched delimiters: `(` `[` `{` `<`.
* Line ends with a follower token: `.`, `?`, `::`, `=`, any binary operator, `->`, `=>`, `as`, `where`, `return`, `break`, `continue`, `yield`.
* Next line begins with `.`, `?`, `::`, `,`, `]`, `)`, `}`, `elif`, `else`, `where`, or an operator.

### 1.3 Block forms (two styles)

* **Suite (colon + indentation):** a header ending with `:` starts an indented block.
* **Braces:** `{ ... }` remain fully supported.
* **Do not mix** `:` and `{ ... }` on the same block header.

---

## 2) Expressions, “statements,” and unit

### 2.1 Last-expression-wins

* A block evaluates to the value of its **final expression**.

### 2.2 Forcing unit (`()`) at block end

* To make a block/branch return `()`, **end it with a separate line**:

  ```text
  ...    # do stuff
  ()     # <- block value is ()
  ```

### 2.3 Non-final side-effects (no semicolons)

* A **standalone line** that is a non-unit expression is **not allowed** (type error), because its value would be silently dropped.
* Use **`let _ = expr`** to explicitly evaluate and drop intermediate values:

  ```text
  let _ = might_return_value()
  ```

### 2.4 Unit-returning calls

* Calls whose type is `()` may appear as bare lines:

  ```text
  println!("hi")  # ok: type is ()
  ```

---

## 3) `if` / `elif` / `else`

### 3.1 Semantics

* Identical to Rust: each branch is a block; **last expression** is the branch value.
* The whole `if` expression evaluates to the taken branch’s value.

### 3.2 Inline one-liner (two branches only)

* Allowed only for a **single `if … else …`**:

  ```text
  if cond: expr_true else: expr_false
  ```

  Equivalent to:

  ```rust
  if cond { expr_true } else { expr_false }
  ```

### 3.3 All other cases are multi-line

* Any `elif` chain or non-trivial bodies must be **suite blocks**:

  ```text
  if x > 0:
      x
  elif x < 0:
      -x
  else:
      0
  ```

### 3.4 Forcing unit in a branch

* End the branch with `()`:

  ```text
  if ready():
      println!("ok")
      ()
  else:
      log_warn("not ready")
      ()
  ```

---

## 4) `match`

* Arms can be brace arms or **suite arms**:

  ```text
  match v:
      0 => 0
      1 | 2 =>:
          let t = heavy()
          t
      _ => compute(v)
  ```
* Each arm’s **last expression** is the arm value.
* To make an arm return `()`, end it with `()`:

  ```text
  match cmd:
      Cmd::Log(m) =>:
          println!("{m}")
          ()
      Cmd::Value(x) => x
  ```

---

## 5) Loops

```text
for i in 0..n:
    work(i)

while ready():
    tick()

loop:
    if should_break():
        break
```

* Loop body’s value is ignored; loops evaluate to `()` unless `break expr`.
* Inside bodies, use `let _ = expr` for non-unit side-effects; end with `()` if you need to force unit result for a block.

---

## 6) Functions, impls, modules

### 6.1 Functions

```text
fn add(a: i32, b: i32) -> i32:
    a + b
```

### 6.2 Structs / Enums

```text
struct Point:
    x: f32,
    y: f32

enum Mode:
    Fast,
    Safe,
    Custom(u32),
```

### 6.3 Impl

```text
impl Point:
    fn len(&self) -> f32:
        (self.x*self.x + self.y*self.y).sqrt()
```

### 6.4 Modules

```text
mod math:
    pub fn dot(a: (f32,f32), b: (f32,f32)) -> f32:
        a.0*b.0 + a.1*b.1
```

---

## 7) Macros

* Invocation as expressions:

  ```text
  assert!(x > 0)
  vec![1,2,3]
  ```

* If the macro returns a non-unit value and you don’t use it **not at the end of a block**, write:

  ```text
  let _ = dbg!(value)
  ```

* If the macro is the **final expression** and you want the block to be unit, add a final `()` line.

* Suite blocks desugar to brace blocks **before** macro expansion (implementation detail).

---

## 8) Examples

### 8.1 Returning a value

```text
fn classify(n: i32) -> &'static str:
    if n > 10:
        "big"
    elif n > 0:
        "small"
    else:
        "zero or negative"
```

### 8.2 Side-effects with unit result

```text
fn notify(ok: bool):
    if ok:
        println!("ok")
        ()
    else:
        log_warn("not ok")
        ()
```

### 8.3 Mixed: intermediate drops, final value

```text
fn compute() -> i32:
    let _ = maybe_log("start")  # non-unit? drop explicitly
    let x = 2
    let y = 3
    x + y                       # last expression -> 5
```

### 8.4 Inline conditional (two-branch)

```text
let label = if text.len() > 10: "long" else: "short"
```

### 8.5 Larger end-to-end (revised from earlier)

```text
use std::fs
use std::path::Path
use std::time::Duration

#[derive(Debug)]
enum Mode:
    Fast,
    Safe,
    Custom(u32),

#[derive(Debug)]
struct Config:
    mode: Mode,
    path: String,
    retries: u32,

#[derive(Debug, thiserror::Error)]
enum AppError:
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid config: {0}")]
    BadConfig(String),

fn load_config(p: &str) -> Result<Config, AppError>:
    let text = fs::read_to_string(p)?

    let mode =
        if text.contains("fast"):
            Mode::Fast
        elif text.contains("safe"):
            Mode::Safe
        else:
            Mode::Custom(42)

    let retries =
        if let Some(n) = text.lines().find(|l| l.starts_with("retries=")):
            n["retries=".len()..].parse().unwrap_or(3)
        else:
            3

    let path =
        if let Some(n) = text.lines().find(|l| l.starts_with("path=")):
            n["path=".len()..].to_string()
        else:
            "data/input.txt".to_string()

    Config { mode, path, retries }

fn process(cfg: &Config) -> Result<String, AppError>:
    if !Path::new(&cfg.path).exists():
        return Err(AppError::BadConfig(format!("missing file: {}", cfg.path)))

    let delay_ms = match cfg.mode:
        Mode::Fast => 10
        Mode::Safe => 100
        Mode::Custom(n) => n.min(2_000)

    let mut last = String::new()
    for attempt in 0..=cfg.retries:
        let text = fs::read_to_string(&cfg.path)?
        last = text.trim().to_string()

        if !last.is_empty():
            break

        std::thread::sleep(Duration::from_millis(delay_ms as u64))
        if attempt == cfg.retries:
            return Err(AppError::BadConfig("empty file after retries".into()))

    let label = if last.len() > 10: "long" else: "short"
    format!("[{}:{}] {}", label, delay_ms, last)

pub fn run(config_path: &str) -> Result<String, AppError>:
    let cfg = load_config(config_path)?
    process(&cfg)
```

If any final block in the above needed to be `()`, we’d add a trailing `()` on its own line.

---

## 9) Formatting & migration

* **`rustfmt`**:

  * Keeps inline `if cond: a else: b` only when both branches are short.
  * Enforces multi-line suites for any `elif` chain or long expressions.
* **Edition migration**:

  * Remove trailing `;` automatically.
  * Convert `expr;` (non-unit) to `let _ = expr`.
  * Where a block previously ended with a statement solely to yield `()`, insert a trailing `()` line if needed.

---

## 10) Pros / trade-offs

**Pros**

* No punctuation to discard values; intent is explicit via `()` at block end.
* No ASI corner cases; rules are simple and checkable.
* `let _ = expr` mirrors idiomatic Rust for explicit value dropping.
* Inline two-branch conditional gives a ternary-like convenience.

**Trade-offs**

* Requires awareness to add `()` when you want a unit result at the end.
* Bare non-unit expressions are errors unless bound to `_` or returned.
* Indentation is structurally significant for suites.

---

### ✅ Summary

* **No semicolons**.
* **No `discard`**.
* Blocks/branches return their **last expression**; **end with `()`** on its own line to make the value `()`.
* Non-final side-effects: **`let _ = expr`** (or call functions that return `()`).
* **`elif`** replaces `else if`.
* Inline `if cond: a else: b` is allowed **only** for the two-branch one-liner; everything else is multi-line.

Want me to generate a **BNF/EBNF sketch** for the modified grammar (including the “naked expression line must be unit” rule)?
