# Configuration options

## `engine` section
  - `timeout_multiplier`: 
    Before executing mutants, wasmut will run the wasm module without 
    any mutations and measure the number of cycles it takes to execute.
    Mutants then are allowed to execute with a timeout of 
    `timeout = original_cycles * timeout_multiplier`

    ```toml
    timeout_multiplier = 2.0
    ```

  - `map_dirs`: Map directories into the WebAssembly runtime. By default, modules cannot access the host's filesystem. If your module needs to access any files, 
  you can use the `map_dirs` option to define path mappings.
    ```toml
    # Map testdata/count_words/files to /files
    map_dirs = [["testdata/count_words/files", "files"],]
    ```


## `filter` section


  - `allowed_function/allowed_file`: By default, all files and functions are allowed, which means that every WebAssembly instruction can potentially be mutated. This is not very practical, so it possible to specify and allowlist for functions and/or files.
  In allowed_functions and allowed_files, you can specify a list of regular expressions that are used to match the function and file names. A wasm-instruction is allowed to be mutated if its function or file matches at least one of the corresponding regular expressions. An empty regular expression also matches everything.
  Use the `list-files` or `list-functions` commands to get a list of all functions and files in the wasm module.

    ```toml
    allowed_functions = ["^add"]
    allowed_files = ["src/add.c", "src/main.c"]
    ```

## `report` section
  - `path_rewrite`: When rendering reports, `wasmut` needs to have access to the original source files.
  `wasmut` uses DWARF debug information embedded in the WebAssembly modules to locate them. As DWARF embeds absolute paths for the source files into the module, 
  it can be problematic if you want to want to create reports for WebAssembly modules that where build on another host.
  The `path_rewrite` option allows to specify a regular expression and a replacement that will be applied to any source file path before creating the report.
  Internally, Rust's `Regex::replace` is used. Consult the [documentation](https://docs.rs/regex/latest/regex/struct.Regex.html#method.replace) for any advanced replacement scenarios.
  
    ```toml
    # Replace /home/user/wasmut/ with "build"
    # e.g. /home/user/test/main.c -> 
    #      build/test/main.c
    path_rewrite = ["^/home/user/", "build"]
    ```

## Full example
```toml
[engine]
timeout_multiplier = 4.0
map_dirs = [["testdata/count_words/files", "files"],]

[filter]
allowed_functions = ["^count_words"]
#allowed_files = [""]

[report]
path_rewrite = ["^.*/wasmut/", ""]
```