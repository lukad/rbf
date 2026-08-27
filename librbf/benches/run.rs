//! Run time: how long the generated code takes to execute.

use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use librbf::{Function, Jit, optimize, parse};

mod common;

use common::PROGRAMS;

/// Points file descriptor 1 at `/dev/null` for as long as it is alive.
///
/// The generated code calls the runtime helpers in `jit::common`, which write
/// to `io::stdout()` directly and cannot be redirected from Rust. Swapping the
/// descriptor underneath them keeps a million asterisks out of the benchmark
/// report while still paying the real cost of the write path.
struct DevNull {
    saved: i32,
    _null: File,
}

impl DevNull {
    fn new() -> Self {
        // Get criterion's own output out of the buffer before it would be
        // redirected along with the program's.
        std::io::stdout().flush().unwrap();

        let null = File::options().write(true).open("/dev/null").unwrap();
        let saved = unsafe { libc::dup(1) };
        assert!(saved >= 0, "could not save stdout");

        let redirected = unsafe { libc::dup2(null.as_raw_fd(), 1) };
        assert!(redirected >= 0, "could not redirect stdout");

        Self { saved, _null: null }
    }
}

impl Drop for DevNull {
    fn drop(&mut self) {
        // Drain what the program wrote while the descriptor still points at
        // `/dev/null`, otherwise it lands in the report after we restore it.
        std::io::stdout().flush().unwrap();

        unsafe {
            libc::dup2(self.saved, 1);
            libc::close(self.saved);
        }
    }
}

fn time_runs(fun: &Function, iters: u64) -> Duration {
    let _null = DevNull::new();
    let start = Instant::now();

    for _ in 0..iters {
        fun.run();
    }

    start.elapsed()
}

fn run(c: &mut Criterion) {
    let mut group = c.benchmark_group("run");
    group.sample_size(50).measurement_time(Duration::from_secs(20));

    for program in PROGRAMS {
        let fun = Jit::new().compile(&optimize(parse(program.source.as_bytes())));

        if let Some(throughput) = program.throughput() {
            group.throughput(throughput);
        }

        group.bench_function(BenchmarkId::from_parameter(program.name), |b| {
            b.iter_custom(|iters| time_runs(&fun, iters))
        });
    }

    group.finish();
}

/// The same programs compiled straight from the parser, which is what `rbf
/// --no-opt` runs. Comparing a name here against the same name in `run` shows
/// what the optimizer is actually buying on that program.
fn run_unoptimized(c: &mut Criterion) {
    let mut group = c.benchmark_group("run_unoptimized");
    group.sample_size(10).measurement_time(Duration::from_secs(20));

    for program in PROGRAMS {
        let fun = Jit::new().compile(&parse(program.source.as_bytes()));

        if let Some(throughput) = program.throughput() {
            group.throughput(throughput);
        }

        group.bench_function(BenchmarkId::from_parameter(program.name), |b| {
            b.iter_custom(|iters| time_runs(&fun, iters))
        });
    }

    group.finish();
}

criterion_group!(benches, run, run_unoptimized);
criterion_main!(benches);
