use anyhow::Result;

use wasmut::runtime::*;
use wasmut::wasmmodule::WasmModule;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Insufficient args");
        std::process::exit(1);
    }

    // let bytecode = std::fs::read(&args[1])?;
    let module = WasmModule::from_file(&args[1])?;

    //discover_mutation_positions(&bytecode)?;

    let mut runtime = create_runtime(RuntimeType::Wasmer, module)?;
    let tests = runtime.discover_test_functions()?;
    dbg!(tests);

    let result = runtime.call_returning_i32(&args[2])?;
    dbg!(result);
    Ok(())
}
