//! Compile time: how long it takes to turn source into a callable function.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use librbf::{Jit, optimize, parse};

mod common;

use common::PROGRAMS;

fn parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");

    for program in PROGRAMS {
        group.throughput(Throughput::Bytes(program.source.len() as u64));
        group.bench_function(BenchmarkId::from_parameter(program.name), |b| {
            b.iter(|| parse(black_box(program.source.as_bytes())))
        });
    }

    group.finish();
}

fn optimizing(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimize");

    for program in PROGRAMS {
        // `optimize` consumes its input, so each iteration needs a fresh copy.
        // Cloning it in the setup step keeps it out of the measurement.
        let parsed = parse(program.source.as_bytes());

        group.bench_function(BenchmarkId::from_parameter(program.name), |b| {
            b.iter_batched(
                || parsed.clone(),
                |parsed| optimize(black_box(parsed)),
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

fn codegen(c: &mut Criterion) {
    let mut group = c.benchmark_group("codegen");

    for program in PROGRAMS {
        let optimized = optimize(parse(program.source.as_bytes()));

        group.bench_function(BenchmarkId::from_parameter(program.name), |b| {
            b.iter_batched(
                Jit::new,
                |jit| jit.compile(black_box(&optimized)),
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

fn total(c: &mut Criterion) {
    let mut group = c.benchmark_group("compile_total");

    for program in PROGRAMS {
        group.throughput(Throughput::Bytes(program.source.len() as u64));
        group.bench_function(BenchmarkId::from_parameter(program.name), |b| {
            b.iter(|| {
                let source = black_box(program.source.as_bytes());
                Jit::new().compile(&optimize(parse(source)))
            })
        });
    }

    group.finish();
}

criterion_group!(benches, parsing, optimizing, codegen, total);
criterion_main!(benches);
