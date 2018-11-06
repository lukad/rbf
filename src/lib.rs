#![feature(slice_patterns)]

#[macro_use]

extern crate combine;

mod ast;
mod machine;
mod parser;

pub use ast::*;
pub use machine::Machine;
pub use parser::parse;
