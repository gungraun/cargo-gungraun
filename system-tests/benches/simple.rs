use gungraun::{library_benchmark, library_benchmark_group, main};

#[library_benchmark]
fn hello_world() {
    println!("Hello World!");
}

#[library_benchmark]
#[benches::simple(1, 2)]
fn foo_bar(num: u64) {
    println!("Foo Bar! {num}!!");
}

library_benchmark_group!(name = my_group; benchmarks = hello_world, foo_bar);
main!(library_benchmark_groups = my_group);
