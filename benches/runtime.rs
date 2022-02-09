use criterion::{black_box, criterion_group, criterion_main, Criterion};
use wasmut::{policy::ExecutionPolicy, runtime::create_runtime, wasmmodule::WasmModule};

fn create_rt(module: WasmModule) {
    create_runtime(module, true, &[]).unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    let large_module = WasmModule::from_file("testdata/simple_rust/test.wasm").unwrap();
    let small_module = WasmModule::from_file("testdata/simple_add/test.wasm").unwrap();

    let mut large_rt = create_runtime(large_module.clone(), true, &[]).unwrap();
    let mut small_rt = create_runtime(small_module.clone(), true, &[]).unwrap();

    c.bench_function("WasmerRuntime::new() small module", |b| {
        b.iter(|| create_rt(black_box(small_module.clone())))
    });
    c.bench_function("WasmerRuntime::new() large module", |b| {
        b.iter(|| create_rt(black_box(large_module.clone())))
    });

    c.bench_function("WasmerRuntime::call_test_function() large module", |b| {
        b.iter(|| large_rt.call_test_function(ExecutionPolicy::RunUntilReturn))
    });
    c.bench_function("WasmerRuntime::call_test_function() small module", |b| {
        b.iter(|| small_rt.call_test_function(ExecutionPolicy::RunUntilReturn))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
