use crate::ast::Instruction::*;
use crate::ast::*;

use ariadne::{Color, Fmt, Label, Report, Source};
use chumsky::error::SimpleReason;
use chumsky::prelude::*;

fn program() -> impl Parser<char, Program, Error = Simple<char>> {
    recursive(|prog| {
        let counted = |neg, pos, m: fn(i64) -> _| {
            just(neg)
                .repeated()
                .at_least(1)
                .map(move |i| m(-(i.len() as i64)))
                .or(just(pos)
                    .repeated()
                    .at_least(1)
                    .map(move |i| m(i.len() as i64)))
                .map_with_span(|i, span| (i, span))
        };
        let add = counted('-', '+', Add);
        let mov = counted('<', '>', Move);
        let write = just('.').map_with_span(|_, span| (Write, span));
        let read = just(',').map_with_span(|_, span| (Read, span));
        choice((add, mov, write, read))
            .or(prog
                .delimited_by(just('['), just(']'))
                .map_with_span(|l, span| (Loop(l), span)))
            .recover_with(nested_delimiters('[', ']', [], |_| {
                unreachable!();
            }))
            .recover_with(skip_then_retry_until([']']))
            .padded_by(none_of("+-[]<>,.").repeated())
            .repeated()
    })
    .then_ignore(end())
}

#[derive(Debug)]
pub struct ParseError(Vec<Simple<char>>);

pub struct ErrorReport(Vec<Report>);

impl ErrorReport {
    pub fn eprint(self, source: &str) {
        for report in self.0.into_iter() {
            report.eprint(Source::from(source)).unwrap();
        }
    }
}

impl ParseError {
    pub fn is_error(&self) -> bool {
        self.0.len() > 0
    }

    pub fn report(&self) -> ErrorReport {
        let reports = self
            .0
            .iter()
            .map(|e| e.clone().map(|c| c.to_string()))
            .map(|e| {
                let report = Report::build(ariadne::ReportKind::Error, (), e.span().start);
                let report = match e.reason() {
                    SimpleReason::Unclosed { span, delimiter } => report
                        .with_message(format!(
                            "Unclosed delimiter {}",
                            delimiter.fg(Color::Yellow)
                        ))
                        .with_label(Label::new(span.clone()).with_message(format!(
                            "Unclosed delimiter {}",
                            delimiter.fg(Color::Yellow)
                        ))),
                    SimpleReason::Unexpected => report
                        .with_message(format!(
                            "{}, expected {}",
                            if e.found().is_some() {
                                "Unexpected token in input"
                            } else {
                                "Unexpected end of input"
                            },
                            if e.expected().len() == 0 {
                                "something else".to_string()
                            } else {
                                e.expected()
                                    .map(|expected| match expected {
                                        Some(expected) => expected.fg(Color::Cyan).to_string(),
                                        None => "end of input".to_string(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        ))
                        .with_label(
                            Label::new(e.span())
                                .with_message(format!(
                                    "Unexpected token {}",
                                    e.found()
                                        .unwrap_or(&"end of file".to_string())
                                        .fg(Color::Red)
                                ))
                                .with_color(Color::Red),
                        ),
                    SimpleReason::Custom(_) => todo!(),
                };
                report.finish()
            })
            .collect::<Vec<_>>();
        ErrorReport(reports)
    }
}

pub fn parse(input: &str) -> (Option<Program>, ParseError) {
    let (prog, err) = program().parse_recovery(input);
    (prog, ParseError(err))
}

#[cfg(test)]
mod test {
    use crate::{ast::Instruction::*, Program};

    fn parse(input: &str) -> Program {
        crate::parse(input).0.unwrap()
    }

    #[test]
    fn it_parses_simple_programs() {
        assert_eq!(parse("."), vec![(Write, 0..1)]);
        assert_eq!(parse(".."), vec![(Write, 0..1), (Write, 1..2)]);
        assert_eq!(parse(","), vec![(Read, 0..1)]);
        assert_eq!(parse(",,"), vec![(Read, 0..1), (Read, 1..2)]);
        assert_eq!(parse("++++"), vec![(Add(4), 0..4)]);
        assert_eq!(parse("----"), vec![(Add(-4), 0..4)]);
        assert_eq!(parse("++++---"), vec![(Add(4), 0..4), (Add(-3), 4..7)]);
        assert_eq!(parse("++++ ---"), vec![(Add(4), 0..4), (Add(-3), 5..8)]);
        assert_eq!(parse("++++@---"), vec![(Add(4), 0..4), (Add(-3), 5..8)]);
        assert_eq!(parse("f + o - o"), vec![(Add(1), 2..3), (Add(-1), 6..7)]);
        assert_eq!(parse(">>>>"), vec![(Move(4), 0..4)]);
        assert_eq!(parse("<<<<"), vec![(Move(-4), 0..4)]);
        assert_eq!(parse(">>>><<<"), vec![(Move(4), 0..4), (Move(-3), 4..7)]);
        assert_eq!(parse(">>>> <<<"), vec![(Move(4), 0..4), (Move(-3), 5..8)]);
        assert_eq!(parse(">>>>@<<<"), vec![(Move(4), 0..4), (Move(-3), 5..8)]);
        assert_eq!(parse("f > o < o"), vec![(Move(1), 2..3), (Move(-1), 6..7)]);
    }

    #[test]
    fn it_parses_loops() {
        assert_eq!(parse("[]"), vec![(Loop(vec![]), 0..2)]);
        assert_eq!(parse("[-]"), vec![(Loop(vec![(Add(-1), 1..2)]), 0..3)]);
        assert_eq!(
            parse("[[-]]"),
            vec![(Loop(vec![(Loop(vec![(Add(-1), 2..3)]), 1..4)]), 0..5)]
        );

        assert_eq!(
            parse("[+][-]"),
            vec![
                (Loop(vec![(Add(1), 1..2)]), 0..3),
                (Loop(vec![(Add(-1), 4..5)]), 3..6),
            ]
        );
    }
}
