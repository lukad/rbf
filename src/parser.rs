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
    let mut iter = program.iter().peekable();
    let mut prog: Program = vec![];

    while let Some(current) = iter.next() {
        match (current, iter.peek()) {
            (Add(0), _) => (),
            (Move(0), _) => (),

            (Add(a), Some(Add(b))) => {
                iter.next();
                prog.push(Add(a + b))
            }
            (Move(a), Some(Move(b))) => {
                iter.next();
                prog.push(Move(a + b))
            }

            (Loop(ref body), _) => {
                if let Some(optimized_body) = optimize_loop(body.clone()) {
                    prog.push(optimized_body);
                }
            }

            (Set(a), Some(Add(b))) => {
                iter.next();
                prog.push(Set(a + b))
            }
            (Add(_), Some(Set(a))) => {
                iter.next();
                prog.push(Set(*a))
            }
            (Set(_), Some(Set(a))) => {
                iter.next();
                prog.push(Set(*a))
            }
            (Set(0), Some(Loop(_))) => {
                iter.next();
                prog.push(Set(0))
            }

            _ => prog.push(current.clone()),
        }
    }
    prog
}

fn optimize_loop(program: Program) -> Option<Instruction> {
    match *program {
        [] => None,
        [Add(-1)] => Some(Set(0)),
        [Move(n)] => Some(Scan(n)),
        _ => Some(Loop(optimize(program))),
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

/// Transform brainfuck source into a [Program](type.Program.html).
pub fn parse(input: &str) -> Program {
    let (prog, _state) = program().parse(input).unwrap();
    opt(prog)
}
