extern crate librbf;

use librbf::{Instruction::*, Program};

fn opt(input: &str) -> Program {
    librbf::optimize(librbf::parse(input.as_bytes()))
}

#[test]
fn groups_consecutive_adds() {
    assert_eq!(opt("++---+-----"), [Add(-5)]);
}

#[test]
fn groups_consecutive_adds_with_comments() {
    assert_eq!(opt("-foo-++++\n+bar++- --"), [Add(2)]);
}

#[test]
fn groups_consecutive_moves() {
    assert_eq!(opt("<<<><>>>>"), [Move(1)]);
}

#[test]
fn groups_consecutive_moves_with_comments() {
    assert_eq!(opt(">foo><<<<\n>bar>>< <<"), [Move(-2)]);
}

#[test]
fn omits_empty_loops() {
    assert_eq!(opt("++[][]+"), [Add(3)]);
}

#[test]
fn omits_empty_nested_loops() {
    assert_eq!(opt("++[[[][]][[][]][]]+"), [Add(3)]);
}

#[test]
fn preserves_non_optimizable_nested_loops() {
    let expected = vec![
        Add(-1),
        Loop(vec![Add(2), Loop(vec![Add(-2)]), Loop(vec![Add(2)])]),
        Add(1),
    ];
    assert_eq!(opt("-[++[--][++]]+"), expected);
}

#[test]
fn omits_zero_adds() {
    assert_eq!(opt(".++ --."), [Write, Write])
}

#[test]
fn omits_zero_moves() {
    assert_eq!(opt(".>> <<."), [Write, Write])
}

#[test]
fn transforms_a_clear_loop_into_a_set() {
    assert_eq!(opt("[-]"), [Set(0)]);
}

#[test]
fn combines_set_with_following_adds() {
    assert_eq!(opt("[-]+++"), [Set(3)]);
}

#[test]
fn omits_adds_before_sets() {
    assert_eq!(opt("+++[-]+"), [Set(1)]);
}

#[test]
fn omits_sets_before_sets() {
    assert_eq!(opt("[-]+++++[-]--"), [Set(-2)]);
}

#[test]
fn folds_set_followed_by_write_into_write_const() {
    assert_eq!(opt("[-][>+.<-]."), [WriteConst(0)]);
}

#[test]
fn omits_mul_runs_after_set_0() {
    assert_eq!(opt("[-][>+<-]."), [WriteConst(0)]);
}

#[test]
fn transforms_move_loops_into_scans() {
    assert_eq!(opt("[>>>>]"), [Scan(4)]);
}

#[test]
fn transforms_multiplication_loops_into_mul_runs() {
    assert_eq!(opt("[>+<-]"), [MulRun(vec![(1, 1)])]);
}

#[test]
fn orders_mul_run_offsets() {
    assert_eq!(opt("[>+++>++<<-]"), [MulRun(vec![(1, 3), (2, 2)])]);
}
