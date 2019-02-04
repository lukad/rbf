#![feature(proc_macro_hygiene)]
extern crate dynasm;
extern crate dynasmrt;

#[macro_use]
extern crate combine;
extern crate libc;

mod ast;
mod jit;
mod parser;

pub use ast::*;
pub use jit::Jit;
pub use parser::parse;
