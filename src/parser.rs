use ast::Instruction::*;
use ast::*;

use combine::char::char;
use combine::{between, choice, many, many1, satisfy, skip_many, Parser, Stream};

parser!{
    #[inline(always)]
    fn program[I]()(I) -> Program
        where [I: Stream<Item=char>]
    {
        let comments = || skip_many(satisfy(|c| !"+-><,.[]".chars().any(|t| t == c)));
        let chars = |c| many1(char(c));

        let add = chars('+').map(|s: String| Add(s.len() as i64));
        let sub = chars('-').map(|s: String| Add(-(s.len() as i64)));
        let left = chars('<').map(|s: String| Move(-(s.len() as i64)));
        let right = chars('>').map(|s: String| Move(s.len() as i64));
        let read = char(',').map(|_: char| Read);
        let write = char('.').map(|_: char| Write);

        let bfloop = between(char('['), char(']'), program()).map(Loop);

        let instruction = choice((
            add,
            sub,
            left,
            right,
            read,
            write,
            bfloop
        )).skip(comments());

        (
            comments(),
            many(instruction)
        ).map(|(_comments, instructions)| instructions)
    }
}

fn optimize(program: Program) -> Program {
    match *program {
        [] => vec![],
        [Add(0), ref rest..] => optimize(rest.to_vec()),
        [Move(0), ref rest..] => optimize(rest.to_vec()),

        [Add(a), Add(b), ref rest..] => {
            let mut v = optimize(rest.to_vec());
            v.insert(0, Add(a + b));
            v
        }
        [Move(a), Move(b), ref rest..] => {
            let mut v = optimize(rest.to_vec());
            v.insert(0, Move(a + b));
            v
        }

        [Loop(ref body), ref rest..] => {
            let mut v: Program = vec![];
            if let Some(mut body) = optimize_loop(body.clone()) {
                v.append(&mut body);
            }
            v.append(&mut optimize(rest.to_vec()));
            v
        }

        [Set(a), Add(b), ref rest..] => {
            let mut v = optimize(rest.to_vec());
            v.insert(0, Set(a + b));
            v
        }
        [Add(_), Set(b), ref rest..] => {
            let mut v = optimize(rest.to_vec());
            v.insert(0, Set(b));
            v
        }
        [Set(_), Set(x), ref rest..] => {
            let mut v = optimize(rest.to_vec());
            v.insert(0, Set(x));
            v
        }
        [Set(0), Loop(_), ref rest..] => {
            let mut v = optimize(rest.to_vec());
            v.insert(0, Set(0));
            v
        }

        [ref ins, ref rest..] => {
            let mut v = optimize(rest.to_vec());
            v.insert(0, ins.clone());
            v
        }
    }
}

fn optimize_loop(program: Program) -> Option<Program> {
    match *program {
        [] => None,
        [Add(-1)] => Some(vec![Set(0)]),
        [Move(n)] => Some(vec![Scan(n)]),
        _ => Some(vec![Loop(optimize(program))]),
    }
}

fn opt(program: Program) -> Program {
    let mut opt_a = optimize(program);
    let mut opt_b = optimize(opt_a.clone());

    while opt_a != opt_b {
        opt_a = optimize(opt_b.clone());
        opt_b = optimize(opt_a.clone());
    }

    opt_b
}

/// Transform source input into a [Program](type.Program.html).
pub fn parse(input: &str) -> Program {
    let (prog, _state) = program().parse(input).unwrap();
    opt(prog)
}
