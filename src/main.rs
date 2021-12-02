use anyhow::Result;

use wasmut::runtime::*;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Insufficient args");
        std::process::exit(1);
    }

    let bytecode = std::fs::read(&args[1])?;

    let mut runtime = WasmtimeRuntime::new(&bytecode)?;

    let result = runtime.call_returning_i32(&args[2])?;
    dbg!(result);
    Ok(())
}
