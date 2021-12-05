use anyhow::Result;

use wasmut::runtime::*;
use wasmut::wasmmodule::WasmModule;

#[cfg(not(tarpaulin_include))]
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Insufficient args");
        std::process::exit(1);
    }

    // let bytecode = std::fs::read(&args[1])?;
    let module = WasmModule::from_file(&args[1])?;

    //discover_mutation_positions(&bytecode)?;

    let runtime_type = RuntimeType::Wasmtime;
    dbg!(&runtime_type);

    let mut runtime = create_runtime(runtime_type, module)?;
    let tests = runtime.discover_test_functions()?;
    dbg!(&tests);

    let mut result = runtime.call_test_function(
        &tests[0],
        wasmut::ExecutionPolicy::RunUntilLimit { limit: 19 },
    )?;
    dbg!(result);
    result = runtime.call_test_function(
        &tests[0],
        wasmut::ExecutionPolicy::RunUntilLimit { limit: 18 },
    )?;
    dbg!(result);
    Ok(())
}
