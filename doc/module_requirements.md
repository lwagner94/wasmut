# WebAssembly module requirements

`wasmut` currently supports WebAssembly modules using the [WebAssembly System Interface (WASI)](https://wasi.dev/).
`wasmut` will execute the `_start` function as an entry point into the module and will use the 
module's exit code (set by the return value of `main` or explicit calls to `exit`) to determine the outcome 
of the module's tests - 0 indicating success, and any non-zero exit code as a failure.

`wasmut` makes heavy use of DWARF debug information for mutant filtering and report
generation. Make sure to compile the WebAssembly module using the correct compiler flags
to ensure that debug information is embedded into the module.

Furthermore, compiler optimizations have a strong influence on `wasmut`'s 
performance. Some more experiments have to be done to give any recommendations,
but for now simply refer to the examples in the `testdata` directory
for any hints on what compiler options to use.




