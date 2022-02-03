use addr2line::Context;
use gimli::{EndianRcSlice, SectionId};
use object::{Object, ObjectSection, SymbolMap, SymbolMapName};
use std::{borrow::Cow, rc::Rc};

// Partly based on https://github.com/gimli-rs/addr2line/blob/master/examples/addr2line.rs
// Licensed under the MIT license, retrived on 2021-12-23
// Copyright (c) 2016-2018 The gimli Developers

#[derive(Debug, Default, PartialEq, Clone)]
pub struct CodeLocation {
    pub file: Option<String>,
    pub function: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

pub struct AddressResolver<'data> {
    symbols: SymbolMap<SymbolMapName<'data>>,
    context: Context<EndianRcSlice<gimli::RunTimeEndian>>,
}

fn load_file_section<Endian: gimli::Endianity>(
    id: SectionId,
    file: &object::File,
    endian: Endian,
) -> core::result::Result<EndianRcSlice<Endian>, ()> {
    let name = id.name();
    match file.section_by_name(name) {
        Some(section) => match section.uncompressed_data().unwrap() {
            Cow::Borrowed(b) => Ok(EndianRcSlice::new(Rc::from(b), endian)),
            Cow::Owned(b) => Ok(EndianRcSlice::new(Rc::from(b.as_slice()), endian)),
        },
        None => Ok(EndianRcSlice::new(Rc::from([]), endian)),
    }
}

impl<'data> AddressResolver<'data> {
    pub fn new(data: &'data [u8]) -> Self {
        let object = object::File::parse(data).unwrap();
        let endian = gimli::RunTimeEndian::Little;
        let mut load_section = |id: SectionId| -> core::result::Result<_, _> {
            load_file_section(id, &object, endian)
        };

        let symbols = object.symbol_map();
        let dwarf = gimli::Dwarf::load(&mut load_section).unwrap();
        let context = Context::from_dwarf(dwarf).unwrap();

        Self { symbols, context }
    }

    pub fn lookup_address(&self, addr: u64) -> Option<CodeLocation> {
        let mut frames = self.context.find_frames(addr).ok()?;

        if let Some(frame) = frames.next().ok()? {
            let function_name = if let Some(func) = frame.function {
                Some(function_name(&func.raw_name().ok()?, func.language))
            } else {
                self.symbols
                    .get(addr)
                    .map(|x| x.name())
                    .map(|name| function_name(name, None))
            };

            Some(CodeLocation {
                file: frame
                    .location
                    .as_ref()
                    .and_then(|l| l.file.map(String::from)),
                function: function_name,
                line: frame.location.as_ref().and_then(|l| l.line.map(u64::from)),
                column: frame
                    .location
                    .as_ref()
                    .and_then(|l| l.column.map(u64::from)),
            })
        } else {
            let func = self
                .symbols
                .get(addr)
                .map(|x| x.name())
                .map(|name| function_name(name, None));
            Some(CodeLocation {
                file: None,
                function: func,
                line: None,
                column: None,
            })
        }
    }
}

fn function_name(name: &str, language: Option<gimli::DwLang>) -> String {
    addr2line::demangle_auto(Cow::from(name), language).into()
}

// fn code_location(location: Option<Location>) -> (Option<&str>, Option<u32>, Option<u32>) {
//     if let Some(ref loc) = location {
//         (loc.file, loc.line, loc.column)
//     } else {
//         (None, None, None)
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::fs::read;

    #[test]
    fn inlined() -> Result<()> {
        let bytes = read("testdata/simple_add/test.wasm")?;
        let resolver = AddressResolver::new(&bytes);

        let location = resolver.lookup_address(100).unwrap();

        assert!(location
            .file
            .unwrap()
            .ends_with("testdata/simple_add/test.c"));
        assert_eq!(location.function, Some("test_add_2".into()));
        assert_eq!(location.line, Some(16));
        assert_eq!(location.column, Some(18));

        Ok(())
    }

    #[test]
    fn start_function() -> Result<()> {
        let bytes = read("testdata/simple_add/test.wasm")?;
        let resolver = AddressResolver::new(&bytes);

        let location = resolver.lookup_address(10).unwrap();

        assert_eq!(location.file, None);
        assert_eq!(location.function, Some("_start".into()));
        assert_eq!(location.line, None);
        assert_eq!(location.column, None);

        Ok(())
    }
}
