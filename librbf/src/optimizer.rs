use std::collections::HashMap;

use crate::ast::{Instruction::*, Program, Spanned};

fn opt(program: Program) -> Program {
    let mut iter = program.iter().peekable();
    let mut prog: Program = vec![];

    while let Some(current) = iter.next() {
        match (current, iter.peek()) {
            ((Add(0), _), _) => (),
            ((Move(0), _), _) => (),

            ((Add(a), span_a), Some((Add(b), span_b))) => {
                iter.next();
                prog.push((Add(a + b), span_a.start..span_b.end));
            }
            ((Move(a), span_a), Some((Move(b), span_b))) => {
                iter.next();
                prog.push((Move(a + b), span_a.start..span_b.end));
            }

            ((Loop(body), span), _) => {
                if let Some(optimized_body) = optimize_loop((body.clone(), span.clone())) {
                    prog.extend(optimized_body);
                }
            }
            ((Set(a), span_set), Some((Add(b), span_add))) => {
                iter.next();
                prog.push((Set(a + b), span_set.start..span_add.end));
            }

            ((Add(_), _), Some(&set @ (Set(_), _))) | ((Set(_), _), Some(&set @ (Set(_), _))) => {
                iter.next();
                prog.push(set.clone());
            }

            ((Set(0), _), Some((Loop(_), _))) => {
                iter.next();
                prog.push(current.clone())
            }
            ((Set(0), _), Some((Mul(_, _), _))) => {
                prog.push(current.clone());
                while let Some((Mul(_, _), _)) = iter.next() {}
            }
            _ => prog.push(current.clone()),
        }
    }
    prog
}

fn optimize_loop(program: Spanned<Program>) -> Option<Program> {
    match &program.0[..] {
        [] => None,
        [(Set(0), _)] => Some(vec![(Set(0), program.1)]),
        [(Add(-1), _)] => Some(vec![(Set(0), program.1)]),
        [(Move(n), _)] => Some(vec![(Scan(*n), program.1)]),
        _ => Some(optimize_mul((optimize(program.0), program.1))),
    }
}

fn optimize_mul(program: Spanned<Program>) -> Program {
    let mut muls = HashMap::new();
    let mut offset = 0;
    let mut is_mul = true;

    for ins in program.0.iter() {
        match ins {
            (Add(i), _) => *muls.entry(offset).or_insert(0) += i,
            (Move(i), _) => offset += i,
            _ => is_mul = false,
        }
        if is_mul == false {
            break;
        }
    }

    if !is_mul || offset != 0 || muls.get(&0) != Some(&-1) {
        return vec![(Loop(program.0), program.1)];
    }

    let mut result: Vec<_> = muls
        .iter()
        .map(|(&k, &v)| {
            if k == 0 {
                None
            } else {
                Some((Mul(k, v), program.1.clone()))
            }
        })
        .filter_map(|x| x)
        .collect();

    result.push((Set(0), program.1));
    result
}

pub fn optimize(program: Program) -> Program {
    let mut opt_a = opt(program);
    let mut opt_b = opt(opt_a.clone());

    while opt_a != opt_b {
        opt_a = opt(opt_b.clone());
        opt_b = opt(opt_a.clone());
    }
    opt_b
}
