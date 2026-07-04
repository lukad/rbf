use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use librbf::{Jit, Program, optimize, parse};

const PARSE_KERNEL: &str = "++>---<[->+<]>[<+>-]>>+[<<+>>-]<[>+[+]<-]";
const EXECUTE_KERNEL: &str = "-[>+[+]<-]";

fn repeated_source(kernel: &str, repeats: usize) -> String {
    kernel.repeat(repeats)
}

fn parse_program(source: &[u8]) -> Program {
    optimize(parse(source))
}

fn bench_parse(c: &mut Criterion) {
    let medium = repeated_source(PARSE_KERNEL, 512);
    let large = repeated_source(PARSE_KERNEL, 8_192);
    let mut group = c.benchmark_group("parse");

    for (name, source) in [("medium", medium.as_bytes()), ("large", large.as_bytes())] {
        group.bench_with_input(BenchmarkId::from_parameter(name), source, |b, source| {
            b.iter(|| parse_program(black_box(source)));
        });
    }

    group.finish();
}

fn bench_compile(c: &mut Criterion) {
    let medium_source = repeated_source(PARSE_KERNEL, 512);
    let large_source = repeated_source(PARSE_KERNEL, 8_192);
    let medium = parse_program(medium_source.as_bytes());
    let large = parse_program(large_source.as_bytes());
    let mut group = c.benchmark_group("compile");

    for (name, program) in [("medium", &medium), ("large", &large)] {
        group.bench_with_input(BenchmarkId::from_parameter(name), program, |b, program| {
            b.iter(|| black_box(Jit::new().compile(black_box(program))));
        });
    }

    group.finish();
}

fn bench_execute(c: &mut Criterion) {
    let source = repeated_source(EXECUTE_KERNEL, 64);
    let program = parse_program(source.as_bytes());
    let function = Jit::new().compile(&program);

    c.bench_function("execute/wrapping_loop", |b| {
        b.iter(|| function.run());
    });
}

criterion_group!(benches, bench_parse, bench_compile, bench_execute);
criterion_main!(benches);
