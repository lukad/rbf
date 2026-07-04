use std::collections::HashMap;
use std::io::Read;

use crate::ast::{Instruction::*, *};

use combine::{
    Parser, Stream, between,
    byte::byte,
    choice, eof, many, many1, satisfy, skip_many,
    stream::{ReadStream, buffered::BufferedStream, state::State},
};

parser! {
    #[inline(always)]
    fn program[I]()(I) -> Program
        where [I: Stream<Item=u8>]
    {
        let comments = || skip_many(satisfy(|c| !"+-><,.[]".bytes().any(|t| t == c)));
        let chars = |c| many1::<Vec<_>, _>(byte(c));

        let add = chars(b'+').map(|s: _| Add(s.len() as i64));
        let sub = chars(b'-').map(|s: _| Add(-(s.len() as i64)));
        let left = chars(b'<').map(|s: _| Move(-(s.len() as i64)));
        let right = chars(b'>').map(|s: _| Move(s.len() as i64));
        let read = byte(b',').map(|_| Read);
        let write = byte(b'.').map(|_| Write);

        let bfloop = between(byte(b'['), byte(b']'), program()).map(Loop);

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

            (Loop(body), _) => {
                if let Some(optimized_body) = optimize_loop(body.clone()) {
                    prog.extend(optimized_body);
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
            (Set(0), Some(Mul(_, _))) => {
                prog.push(Set(0));
                while let Some(Mul(_, _)) = iter.next() {}
            }
            (Set(0), Some(MulRun(_))) => {
                iter.next();
                prog.push(Set(0))
            }

            (Set(n), Some(Write)) => {
                iter.next();
                prog.push(WriteConst(*n));
            }

            (WriteConst(n), Some(WriteConst(m))) => {
                iter.next();
                prog.push(WriteBytes(vec![(*n % 0xFF) as u8, (*m % 0xFF) as u8]))
            }
            (WriteBytes(b), Some(WriteConst(n))) => {
                iter.next();
                let mut b = b.clone();
                b.push((*n % 0xFF) as u8);
                prog.push(WriteBytes(b));
            }
            (WriteBytes(b), Some(WriteBytes(c))) => {
                iter.next();
                let mut b = b.clone();
                b.extend_from_slice(c);
                prog.push(WriteBytes(b));
            }
            (WriteConst(n), Some(WriteBytes(b))) => {
                iter.next();
                let mut b = b.clone();
                b.insert(0, (*n % 0xFF) as u8);
                prog.push(WriteBytes(b));
            }

            _ => prog.push(current.clone()),
        }
    }
    prog
}

fn optimize_loop(program: Program) -> Option<Vec<Instruction>> {
    match *program {
        [] => None,
        [Add(-1)] => Some(vec![Set(0)]),
        [Move(n)] => Some(vec![Scan(n)]),
        [Set(0)] => Some(vec![Set(0)]),
        _ => Some(optimize_mul(optimize(program))),
    }
}

fn optimize_mul(program: Program) -> Vec<Instruction> {
    let mut muls = HashMap::new();
    let mut offset = 0;
    let mut is_mul = true;

    for ins in program.iter() {
        match ins {
            Add(i) => *muls.entry(offset).or_insert(0) += i,
            Move(i) => offset += i,
            _ => is_mul = false,
        }
    }

    if !is_mul || offset != 0 || muls.get(&0) != Some(&-1) {
        return vec![Loop(program)];
    }

    let mut transfers: Vec<_> = muls
        .iter()
        .filter_map(|(&k, &v)| (k != 0).then_some((k, v)))
        .collect();
    transfers.sort_by_key(|&(offset, _)| offset);

    if transfers.is_empty() {
        vec![Set(0)]
    } else {
        vec![MulRun(transfers)]
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

/// Parses Brainfuck source and returns a [Program](type.Program.html).
pub fn parse<R: Read>(input: R) -> Program {
    let stream = BufferedStream::new(State::new(ReadStream::new(input)), 1);
    let ((prog, _eof), _state) = (program(), eof()).parse(stream).unwrap();
    opt(prog)
}
