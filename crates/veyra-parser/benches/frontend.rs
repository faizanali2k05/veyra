use criterion::{Criterion, black_box, criterion_group, criterion_main};
use veyra_lexer::lex;
use veyra_parser::parse_program;

fn parse_showcase(criterion: &mut Criterion) {
    let source = include_str!("../../../examples/showcase.vy");
    criterion.bench_function("lex-and-parse-showcase", |bench| {
        bench.iter(|| {
            let tokens = lex(black_box(source)).expect("benchmark source must lex");
            parse_program(tokens).expect("benchmark source must parse")
        });
    });
}

criterion_group!(benches, parse_showcase);
criterion_main!(benches);
