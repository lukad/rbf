extern crate clap;
extern crate librbf;

use std::fs::File;
use std::io::{self, Read};

use librbf::{parse, Jit};

use clap::{App, AppSettings, Arg};

fn read_source(path: &str) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut source = String::new();
    file.read_to_string(&mut source)?;
    Ok(source)
}

fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .setting(AppSettings::ColoredHelp)
        .arg(
            Arg::with_name("PROGRAM")
                .help("The program")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("emit")
                .short("e")
                .value_name("EMIT")
                .help("Shows the program instead of running it")
                .possible_value("ast")
                .takes_value(true),
        )
        .get_matches();

    let source_path = matches.value_of("PROGRAM").unwrap();
    let source = match read_source(source_path) {
        Ok(source) => source,
        Err(a) => {
            eprintln!("Could not read program: {}", a);
            std::process::exit(1);
        }
    };
    let program = parse(source.as_str());

    if let Some("ast") = matches.value_of("emit") {
        println!("{:?}", program);
        return;
    }

    let mut jit = Jit::allocate();
    jit.generate(&program);
    jit.run();
}
