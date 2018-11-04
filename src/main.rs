extern crate rbf;

use std::env;
use std::fs::File;
use std::io::{self, Read};

use rbf::parse;

fn read_source<R>(mut input: R) -> String
where
    R: Read,
{
    let mut source = String::new();
    input.read_to_string(&mut source).unwrap();
    source
}

fn main() {
    let source = match env::args().nth(1) {
        Some(file) => read_source(File::open(file).unwrap()),
        None => read_source(io::stdin()),
    };

    println!("{:?}", parse(source.as_str()))
}
