#![feature(slice_patterns)]

#[macro_use]

extern crate combine;

mod ast;
mod parser;

pub use ast::*;
pub use parser::parse;
