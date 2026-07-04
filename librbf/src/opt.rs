use std::collections::HashMap;

use crate::{Instruction, Program, ast::Instruction::*};

pub fn optimize(program: Program) -> Program {
    optimize_program(program)
}

fn optimize_program(program: Program) -> Program {
    let mut out = Vec::with_capacity(program.len());

    for ins in program {
        optimize_instruction(&mut out, ins);
    }

    out
}

fn optimize_instruction(out: &mut Program, ins: Instruction) {
    match ins {
        Add(0) | Move(0) => (),
        Loop(body) => {
            if let Some(ins) = optimize_loop(optimize_program(body)) {
                optimize_non_loop(out, ins);
            }
        }
        ins => optimize_non_loop(out, ins),
    }
}

fn optimize_non_loop(out: &mut Program, ins: Instruction) {
    let Some(prev) = out.pop() else {
        out.push(ins);
        return;
    };

    match (prev, ins) {
        (Add(a), Add(b)) => optimize_instruction(out, Add(a + b)),
        (Move(a), Move(b)) => optimize_instruction(out, Move(a + b)),
        (Set(a), Add(b)) => optimize_non_loop(out, Set(a + b)),
        (Add(_), Set(n)) | (Set(_), Set(n)) => optimize_non_loop(out, Set(n)),
        (Set(0), Loop(_)) | (Set(0), Mul(_, _)) | (Set(0), MulRun(_)) => out.push(Set(0)),
        (Set(n), Write) => optimize_non_loop(out, WriteConst(n)),
        (WriteConst(a), WriteConst(b)) => {
            optimize_non_loop(out, WriteBytes(vec![byte(a), byte(b)]))
        }
        (WriteBytes(mut bytes), WriteConst(n)) => {
            bytes.push(byte(n));
            optimize_non_loop(out, WriteBytes(bytes));
        }
        (WriteBytes(mut bytes), WriteBytes(mut more)) => {
            bytes.append(&mut more);
            optimize_non_loop(out, WriteBytes(bytes));
        }
        (WriteConst(n), WriteBytes(mut bytes)) => {
            bytes.insert(0, byte(n));
            optimize_non_loop(out, WriteBytes(bytes));
        }

        (prev, ins) => {
            out.push(prev);
            out.push(ins);
        }
    }
}

fn optimize_loop(program: Program) -> Option<Instruction> {
    match &program[..] {
        [] => None,
        [Add(-1)] => Some(Set(0)),
        [Move(n)] => Some(Scan(*n)),
        [Set(0)] => Some(Set(0)),
        _ => Some(optimize_mul(program)),
    }
}

fn optimize_mul(program: Program) -> Instruction {
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
        return Loop(program);
    }

    let mut transfers: Vec<_> = muls
        .iter()
        .filter_map(|(&k, &v)| (k != 0).then_some((k, v)))
        .collect();
    transfers.sort_by_key(|&(offset, _)| offset);

    if transfers.is_empty() {
        Set(0)
    } else {
        MulRun(transfers)
    }
}

fn byte(i: i64) -> u8 {
    (i % 0xFF) as u8
}
