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

## Usage

``` bash
$ rbf program.bf
```

Instead of running a program, `rbf` can show what it compiled. `-e ast` prints
the intermediate representation and `-e asm` disassembles the machine code the
JIT generated for it:

``` bash
$ echo '++++++++[>++++++++<-]>.' > program.bf
$ rbf -e asm program.bf
0000  55                    push rbp
0001  4889e5                mov rbp, rsp
0004  53                    push rbx
0005  4881ec38750000        sub rsp, 0x7538
000c  488d1c24              lea rbx, qword [rsp]
0010  48b870590ece6e550000  mov rax, 0x556ece0e5970  ; memzero
001a  4889df                mov rdi, rbx
001d  48c7c630750000        mov rsi, 0x7530
0024  ffd0                  call rax
0026  c6830000000008        mov byte [rbx], 0x8
002d  c6830100000040        mov byte [rbx + 0x1], 0x40
0034  c6830000000000        mov byte [rbx], 0x0
003b  48c7c740000000        mov rdi, 0x40
0042  48b8e0590ece6e550000  mov rax, 0x556ece0e59e0  ; putchar
004c  ffd0                  call rax
004e  4881c301000000        add rbx, 0x1
0055  4881c438750000        add rsp, 0x7538
005c  5b                    pop rbx
005d  5d                    pop rbp
005e  c3                    ret

; 20 instructions, 95 bytes
```

The listing names the runtime helpers the generated code calls into, and shows
the bytes behind a bulk write. Both modes respect `--no-opt`, so they also show
what the unoptimized program looks like.

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

### Code generation

Both JIT backends (aarch64, x86_64) apply a few additional optimizations while lowering the
optimized IR to machine code:

* Pointer moves are kept as a virtual offset and only flushed to the tape
  pointer before operations that need the real pointer, such as loops, scans,
  and the end of the program
* Loads, stores, and zero stores use each architecture's direct byte addressing
  when the virtual offset fits, and compute a temporary address otherwise
* Pointer flushes use immediate arithmetic when the offset fits and a scratch
  register otherwise
* The backends track known cell values across straight-line code. Known-value
  `Add`, `Mul`, and `MulRun` operations are folded during code generation,
  while redundant `Set` instructions are skipped
* Writes from known cells are emitted as constant-byte writes and `WriteBytes`
  calls the bulk output helper once for the whole byte slice
* `Scan` and `Loop` instructions are skipped when the current cell is already
  known to be zero
* `MulRun` loads the source cell once, reuses it for every transfer, and clears
  the source cell at the end. Factors equivalent to `1` and `-1` use add/sub
  paths without multiplication
