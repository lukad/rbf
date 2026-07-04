extern crate librbf;

use librbf::{Instruction::*, Program};

fn parse(input: &str) -> Program {
    librbf::parse(input.as_bytes())
}

#[test]
fn parses_empty_program() {
    assert_eq!(parse(""), vec![]);
}

#[test]
fn ignores_comments() {
    assert_eq!(parse(" foo bar baz "), []);
}

#[test]
fn parses_add_runs() {
    assert_eq!(parse(" ++++ --- "), [Add(4), Add(-3)]);
}

#[test]
fn parses_move_runs() {
    assert_eq!(parse(" >>>> <<< "), [Move(4), Move(-3)]);
}

#[test]
fn parses_io() {
    assert_eq!(parse(" , . "), [Read, Write]);
}

#[test]
fn parses_mixed_programs() {
    assert_eq!(
        parse(">+<-,."),
        [Move(1), Add(1), Move(-1), Add(-1), Read, Write]
    );
}

#[test]
fn keeps_adjacent_raw_instructions_unoptimized() {
    assert_eq!(parse("++---+-----"), [Add(2), Add(-3), Add(1), Add(-5)]);
}

#[test]
fn comments_separate_raw_instruction_runs() {
    assert_eq!(
        parse("-foo-++++\n+bar++- --"),
        [Add(-1), Add(-1), Add(4), Add(1), Add(2), Add(-1), Add(-2)]
    );
}

#[test]
fn parses_simple_loops() {
    assert_eq!(parse("-[+]+"), [Add(-1), Loop(vec![Add(1)]), Add(1)]);
}

#[test]
fn preserves_empty_loops() {
    assert_eq!(
        parse("++[][]+"),
        [Add(2), Loop(vec![]), Loop(vec![]), Add(1)]
    );
}

#[test]
fn parses_nested_loops() {
    assert_eq!(
        parse("-[++[--][++]]+"),
        [
            Add(-1),
            Loop(vec![Add(2), Loop(vec![Add(-2)]), Loop(vec![Add(2)])]),
            Add(1),
        ]
    );
}

#[test]
fn preserves_nested_empty_loops() {
    assert_eq!(
        parse("++[[[][]][[][]][]]+"),
        [
            Add(2),
            Loop(vec![
                Loop(vec![Loop(vec![]), Loop(vec![])]),
                Loop(vec![Loop(vec![]), Loop(vec![])]),
                Loop(vec![])
            ]),
            Add(1),
        ]
    );
}
