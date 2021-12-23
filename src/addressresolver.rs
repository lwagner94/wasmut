use crate::error::Result;
use addr2line::{Context, Location};
use gimli::{EndianRcSlice, SectionId};
use object::{Object, ObjectSection, SymbolMap, SymbolMapName};
use std::{borrow::Cow, path::PathBuf, rc::Rc};

// Partly based on https://github.com/gimli-rs/addr2line/blob/master/examples/addr2line.rs
// Licensed under the MIT license, retrived on 2021-12-23
// Copyright (c) 2016-2018 The gimli Developers

#[derive(Debug, Default, PartialEq)]
pub struct CodeLocation {
    file: Option<PathBuf>,
    function: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
}

#[derive(Debug, Default, PartialEq)]
pub struct CodeLocations {
    locations: Vec<CodeLocation>,
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
    dbg!(&name);
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

    pub fn lookup_address(&self, addr: u64) -> Result<CodeLocations> {
        let mut printed_anything = false;
        let mut frames = self.context.find_frames(addr).unwrap();

        let mut locations: CodeLocations = Default::default();

        while let Some(frame) = frames.next().unwrap() {
            let function_name = if let Some(func) = frame.function {
                Some(function_name(&func.raw_name().unwrap(), func.language))
            } else  {
                self.symbols.get(addr).map(|x| x.name()).map(|name| function_name(name, None))  
            };

            let cl = code_location(frame.location);

            printed_anything = true;

            // TODO: Refactor
            locations.locations.push(CodeLocation {
                file: cl.0,
                function: function_name,
                line: cl.1,
                column: cl.2,
            });
        }

        if !printed_anything {
            let func = self
                .symbols
                .get(addr)
                .map(|x| x.name())
                .map(|name| function_name(name, None));

            locations.locations.push(CodeLocation {
                file: None,
                function: func,
                line: None,
                column: None,
            });
        }

        Ok(locations)
    }
}

fn function_name(name: &str, language: Option<gimli::DwLang>) -> String {
    addr2line::demangle_auto(Cow::from(name), language).into()
}

fn code_location(location: Option<Location>) -> (Option<PathBuf>, Option<u32>, Option<u32>) {
    if let Some(ref loc) = location {
        let file = loc.file.as_ref().unwrap();
        let path = PathBuf::from(file);
        (Some(path), loc.line, loc.column)
    } else {
        (None, None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::fs::read;

    #[test]
    fn inlined() -> Result<()> {
        let bytes = read("testdata/simple_add/test.wasm")?;
        let resolver = AddressResolver::new(&bytes);

        let path = PathBuf::from("./testdata/simple_add/test.c")
            .canonicalize()
            .unwrap();

        let locations = resolver.lookup_address(105)?;
        assert_eq!(
            locations,
            CodeLocations {
                locations: vec![
                    CodeLocation {
                        file: Some(path.clone(),),
                        function: Some("test_add_2".into(),),
                        line: None,
                        column: Some(18,),
                    },
                    CodeLocation {
                        file: Some(path.clone(),),
                        function: Some("main".into(),),
                        line: Some(21,),
                        column: Some(30,),
                    },
                ],
            }
        );

        Ok(())
    }
}
