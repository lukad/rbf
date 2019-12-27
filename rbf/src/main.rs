extern crate clap;
extern crate librbf;

use std::fs::File;
use std::io::Write;

use librbf::{parse, Jit};

use clap::{App, AppSettings, Arg};

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
    let file = File::open(source_path).expect("Could not read program");

    let program = parse(file);

    if let Some("ast") = matches.value_of("emit") {
        println!("{:?}", program);
        return;
    }

    {
        let jit = Jit::new();
        let fun = jit.compile(&program);
        fun.run();
        std::io::stdout().flush().unwrap();
    }
}
