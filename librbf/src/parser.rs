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

/// Parses Brainfuck source and returns a [Program](type.Program.html).
pub fn parse<R: Read>(input: R) -> Program {
    let stream = BufferedStream::new(State::new(ReadStream::new(input)), 1);
    let ((prog, _eof), _state) = (program(), eof()).parse(stream).unwrap();
    prog
}
