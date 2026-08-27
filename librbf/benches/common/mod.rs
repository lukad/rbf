//! The program corpus shared by the `compile` and `run` benchmarks.
//!
//! Each program targets one path through the compiler so a regression can be
//! attributed to a specific optimization rather than to "brainfuck got slower".

// Each bench target uses a different subset of the corpus metadata.
#![allow(dead_code)]

use criterion::Throughput;

/// What a single run of a program does, used to report throughput.
pub enum Work {
    /// Bytes written to stdout.
    Bytes(u64),
    /// Tape operations: scan steps or loop iterations.
    Ops(u64),
}

pub struct Program {
    pub name: &'static str,
    pub source: &'static str,
    /// `None` for programs whose runtime is dominated by fixed overhead.
    pub work: Option<Work>,
}

impl Program {
    pub fn throughput(&self) -> Option<Throughput> {
        match self.work {
            Some(Work::Bytes(n)) => Some(Throughput::Bytes(n)),
            Some(Work::Ops(n)) => Some(Throughput::Elements(n)),
            None => None,
        }
    }
}

pub const PROGRAMS: &[Program] = &[
    Program {
        name: "hello",
        source: include_str!("../programs/hello.bf"),
        work: None,
    },
    Program {
        name: "writes",
        source: include_str!("../programs/writes.bf"),
        work: Some(Work::Bytes(1_000_000)),
    },
    Program {
        name: "writes_nl",
        source: include_str!("../programs/writes_nl.bf"),
        work: Some(Work::Bytes(1_000_000)),
    },
    Program {
        name: "scan",
        source: include_str!("../programs/scan.bf"),
        work: Some(Work::Ops(2_000_000)),
    },
    Program {
        name: "loops",
        source: include_str!("../programs/loops.bf"),
        work: Some(Work::Ops(3_125_000)),
    },
    Program {
        name: "mandelbrot",
        source: include_str!("../programs/mandelbrot.bf"),
        work: None,
    },
    Program {
        name: "mulrun",
        source: include_str!("../programs/mulrun.bf"),
        work: Some(Work::Ops(3_125_000)),
    },
];
