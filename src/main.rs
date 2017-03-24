use std::fs::File;
use std::process;

extern crate rbf_lib;

use rbf_lib::{SourceReader, Program};

fn main() {
    let filename = match std::env::args().nth(1) {
        Some(s) => s,
        None => process::exit(1),
    };

    let mut file = match File::open(filename) {
        Ok(f) => f,
        Err(s) => {
            println!("Could not read foo.bf: {}", s);
            process::exit(1);
        }
    };
    let mut reader = SourceReader::new(&mut file);

    println!("{:?}", Program::parse(&mut reader).unwrap().0);
}
