use crate::error::{Error, Result};

#[derive(Clone)]
pub struct WasmModule {
    module: parity_wasm::elements::Module,
}

impl WasmModule {
    // TODO: Allow wat
    pub fn from_file(path: &str) -> Result<WasmModule> {
        let module = parity_wasm::elements::deserialize_file(path)
            .map_err(|e| Error::BytecodeDeserialization { source: e })?;
        Ok(WasmModule { module })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<WasmModule> {
        let module = parity_wasm::elements::deserialize_buffer(bytes)
            .map_err(|e| Error::BytecodeDeserialization { source: e })?;
        Ok(WasmModule { module })
    }
}

impl TryFrom<WasmModule> for Vec<u8> {
    type Error = Error;
    fn try_from(module: WasmModule) -> Result<Vec<u8>> {
        let bytes = parity_wasm::serialize(module.module)
            .map_err(|e| Error::BytecodeSerialization { source: e })?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::fs::read;

    // TODO: See if it makes sense to generalize tests for both runtimes?

    #[test]
    fn test_load_from_file() {
        assert!(WasmModule::from_file("testdata/simple_add/test.wasm").is_ok());
    }

    #[test]
    fn test_load_from_bytes() -> Result<()> {
        let bytecode = read("testdata/simple_add/test.wasm")?;
        WasmModule::from_bytes(&bytecode)?;
        Ok(())
    }

    #[test]
    fn test_into_buffer() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let _: Vec<u8> = module.try_into()?;
        Ok(())
    }
}
