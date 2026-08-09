use std::io::Read;

use crate::ast::{Instruction::*, *};

/// Parses Brainfuck source and returns a [Program](type.Program.html).
pub fn parse<R: Read>(mut input: R) -> Program {
    let mut source = Vec::new();
    input.read_to_end(&mut source).unwrap();

    let mut programs = vec![Program::new()];
    let mut position = 0;

    while position < source.len() {
        let op = source[position];

        match op {
            b'+' | b'-' | b'<' | b'>' => {
                let start = position;
                while source.get(position) == Some(&op) {
                    position += 1;
                }

                let count = (position - start) as i64;
                programs.last_mut().unwrap().push(match op {
                    b'+' => Add(count),
                    b'-' => Add(-count),
                    b'>' => Move(count),
                    b'<' => Move(-count),
                    _ => unreachable!(),
                });
            }
            b',' => {
                programs.last_mut().unwrap().push(Read);
                position += 1;
            }
            b'.' => {
                programs.last_mut().unwrap().push(Write);
                position += 1;
            }
            b'[' => {
                programs.push(Program::new());
                position += 1;
            }
            b']' => {
                assert!(programs.len() > 1, "unmatched closing bracket");
                let body = programs.pop().unwrap();
                programs.last_mut().unwrap().push(Loop(body));
                position += 1;
            }
            _ => position += 1,
        }
    }

    assert_eq!(programs.len(), 1, "unmatched opening bracket");
    programs.pop().unwrap()
}
