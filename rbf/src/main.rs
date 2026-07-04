use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};
use librbf::{Jit, opt, parse};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(value_name = "PROGRAM", help = "The program")]
    program: PathBuf,

    #[arg(
        short = 't',
        long = "tape-size",
        default_value_t = 30_000,
        help = "The tape size"
    )]
    tape_size: usize,

    #[arg(
        short,
        value_name = "EMIT",
        help = "Shows the program instead of running it"
    )]
    emit: Option<Emit>,

    #[arg(long = "no-opt", action = ArgAction::SetFalse, help = "Disables optimization")]
    opt: bool,
}

#[derive(Clone, Debug, ValueEnum)]
enum Emit {
    Ast,
}

fn main() {
    let args = Args::parse();
    let file = File::open(args.program).expect("Could not read program");

    let parse = parse(file);
    let program = if args.opt { opt(parse) } else { parse };

    if matches!(args.emit, Some(Emit::Ast)) {
        println!("{:?}", program);
        return;
    }

    {
        let jit = Jit::new().set_tape_size(args.tape_size);
        let fun = jit.compile(&program);
        fun.run();
        std::io::stdout().flush().unwrap();
    }
}
