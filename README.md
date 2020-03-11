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

This project requires rust nightly because dynasm-rs
[does not yet support stable](https://github.com/CensoredUsername/dynasm-rs/issues/31).

### Using `cargo install`

``` bash
$ cargo +nightly install https://github.com/lukad/rbf.git
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
use librbf::{parse, Jit};

fn main() {
    let source = "++++++++[>++++++++<-]>.".as_bytes();
    let program = parse(source);
    let fun = Jit::new().compile(&program);
    fun.run();
}
```

## Optimizations

The following optimizations are implemented:

* Fusing of adjacent `+` and `-` instructions:  
  `+-++-+` becomes `Add(2)`
* Fusing of adjacent `>` and `<` instructions:  
  `>><<<<>` becomes `Move(-1)`
* Detection of set loops:  
  `[-]` becomes `Set(0)`
* Set instructions will be fused with following `Add` instructions:  
  `[-]+++` becomes `Set(3)`
* Detection of multiplication loops:  
  `[>++++<-]` becomes `Mul(1, 8), Set(0)`
* Detection of scan looops:  
  `[>>]` becomes `Scan(2)`
* Elimination of code without effects
