# rbf

A JIT compiling brainfuck interpreter written in rust. It's fast!

The interpreter is split into two crates using a workspace:

* `rbf`: The executable that let's you run brainfuck code
* `librbf`: This is where all the interpreter code is implemented

The way this interpreter works is:

1. Parse code into an intermediate representation
2. Apply some optimizations
3. Generate code using [dynasm-rs](https://github.com/CensoredUsername/dynasm-rs)
4. Run the generated code

## Installation

This project requires rust 1.45.0 or newer.

### Using `cargo install`

``` bash
$ cargo install https://github.com/lukad/rbf.git
```

### Compiling manually with cargo

``` bash
$ git clone https://github.com/lukad/rbf.git
$ cd rbf
$ cargo install
```

## Library usage

Add `librbf` to your depedencies in the `Cargo.toml`.

``` toml
[dependencies]
librbf = { git = "https://github.com/lukad/rbf.git" }
```

Use it in your code.

``` rust
use librbf::{optimize, parse, Jit};

fn main() {
    let source = "++++++++[>++++++++<-]>.".as_bytes();
    let program = optimize(parse(source));
    let fun = Jit::new().compile(&program);
    fun.run();
}
```

## Optimizations

### IR optimizations

The parser first groups runs of the same Brainfuck instruction, then the
optimizer simplifies the resulting instruction stream recursively:

* Adjacent increments and decrements are folded into one `Add`:
  `+-++-+` becomes `Add(2)`
* Adjacent pointer moves are folded into one `Move`:
  `>><<<<>` becomes `Move(-1)`
* No-op `Add` and `Move` instructions are removed after folding:
  `++--` and `>><<` become empty programs
* Clear loops are converted to `Set(0)`:
  `[-]` becomes `Set(0)`
* Known cell values are folded through later operations:
  `[-]+++` becomes `Set(3)`, and `+++[-]+` becomes `Set(1)`
* Loops after a known-zero cell are removed:
  `[-][]+` becomes `Set(1)`
* Loops that only move the data pointer are converted to scans:
  `[>>]` becomes `Scan(2)`
* Transfer loops are converted to a single `MulRun` when they decrement the
  source cell by one, return to the source cell, and otherwise only use adds and
  moves:
  `[>++++<-]` becomes `MulRun(vec![(1, 4)])`
* Transfer offsets are merged and sorted inside `MulRun`:
  `[>+++>++<<-]` becomes `MulRun(vec![(1, 3), (2, 2)])`
* Constant writes are folded when the current cell value is known:
  `[-].` becomes `WriteConst(0)`
* Adjacent constant writes are combined into `WriteBytes`:
  `[-].[-]+.` becomes `WriteBytes(vec![0, 1])`

### AArch64 code generation

The AArch64 backend applies a few additional optimizations while lowering the
optimized IR to machine code:

* Pointer moves are kept as a virtual offset and only flushed to the tape
  pointer before operations that need the real pointer, such as loops, scans,
  and the end of the program
* Loads, stores, and zero stores use direct AArch64 byte addressing when the
  virtual offset fits the instruction encoding, larger offsets compute a
  temporary address first
* Small pointer flushes use immediate `add`/`sub`, larger pointer moves load
  the amount into a scratch register
* The backend tracks known cell values across straight-line code. Known-value
  `Add`, `Mul`, and `MulRun` operations are folded during code generation,
  while redundant `Set` instructions are skipped
* Writes from known cells are emitted as constant-byte writes and `WriteBytes`
  calls the bulk output helper once for the whole byte slice
* `Scan` and `Loop` instructions are skipped when the current cell is already
  known to be zero
* `MulRun` loads the source cell once, reuses it for every transfer, and clears
  the source cell at the end. Factors of `1` and `-1` use add/sub paths without
  multiplication
