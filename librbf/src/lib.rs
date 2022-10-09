extern crate dynasm;
extern crate dynasmrt;

extern crate libc;

mod ast;
mod jit;
mod optimizer;
mod parser;

pub use ast::*;
pub use jit::Jit;
pub use optimizer::optimize;
pub use parser::parse;
