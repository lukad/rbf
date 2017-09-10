extern crate rbf;

use rbf::parse;
use rbf::Instruction::*;

#[test]
fn it_parses_an_empty_program() {
    assert_eq!(parse(""), vec![]);
}

#[test]
fn it_parses_an_empty_program_with_comments() {
    assert_eq!(parse(" foo bar baz "), []);
}

#[test]
fn it_parses_a_very_very_simple_program() {
    assert_eq!(parse(" ++++ "), [Add(4)]);
}

#[test]
fn it_parses_a_very_simple_program() {
    assert_eq!(parse(">+<-,."), [Move(1), Add(1), Move(-1), Add(-1), Read, Write]);
}

#[test]
fn it_groups_consecutive_adds() {
    assert_eq!(parse("++---+-----"), [Add(-5)]);
}

#[test]
fn it_groups_consecutive_adds_with_comments() {
    assert_eq!(parse("-foo-++++\n+bar++- --"), [Add(2)]);
}

#[test]
fn it_groups_consecutive_moves() {
    assert_eq!(parse("<<<><>>>>"), [Move(1)]);
}

#[test]
fn it_groups_consecutive_moves_with_comments() {
    assert_eq!(parse(">foo><<<<\n>bar>>< <<"), [Move(-2)]);
}

#[test]
fn it_parses_simple_loops() {
    assert_eq!(parse("-[+]+"), [Add(-1), Loop(vec![Add(1)]), Add(1)]);
}

#[test]
fn it_omits_empty_loops() {
    assert_eq!(parse("++[][]+"), [Add(3)]);
}

#[test]
fn it_omits_empty_nested_loops() {
    assert_eq!(parse("++[[[][]][[][]][]]+"), [Add(3)]);
}

#[test]
fn it_parses_nested_loops() {
    let expected = vec![
        Add(-1),
        Loop(vec![
            Add(2),
            Loop(vec![Add(-2)]),
            Loop(vec![Add(2)]),
        ]),
        Add(1)
    ];
    assert_eq!(parse("-[++[--][++]]+"), expected);
}

#[test]
fn it_omits_zero_adds() {
    assert_eq!(parse(".++ --."), [Write, Write])
}

#[test]
fn it_omits_zero_moves() {
    assert_eq!(parse(".>> <<."), [Write, Write])
}

#[test]
fn it_transforms_a_scan_loop_to_a_scan() {
    assert_eq!(parse("[-]"), [Set(0)]);
}

#[test]
fn it_combines_set_with_succeeding_adds() {
    assert_eq!(parse("[-]+++]"), [Set(3)]);
}

#[test]
fn it_omits_adds_succeed_by_sets() {
    assert_eq!(parse("+++[-]+"), [Set(1)]);
}

#[test]
fn it_omits_sets_succeded_by_sets() {
    assert_eq!(parse("[-]+++++[-]--"), [Set(-2)]);
}

#[test]
fn it_omits_loops_preceed_by_sets() {
    assert_eq!(parse("[-][>+<-]."), [Set(0), Write]);
}

#[test]
fn it_transforms_move_loops_into_scans() {
    assert_eq!(parse("[>>>>]"), [Scan(4)]);
}
