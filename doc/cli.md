# Command Line Interface
## Subcommands
### `help` 
Display the help menu
### `list-files`
```
List all files of the binary.

If a config is provided, this command will also show whether the file is allowed to be mutated. By
default, wasmut will try to load a wasmut.toml file from the current directory

USAGE:
    wasmut list-files [OPTIONS] <WASMFILE>

ARGS:
    <WASMFILE>
            Path to the wasm module

OPTIONS:
    -c, --config <CONFIG>
            Load wasmut.toml configuration file from the provided path

    -C, --config-samedir
            Attempt to load wasmut.toml from the same directory as the wasm module

    -h, --help
            Print help information

    -V, --version
            Print version information

```
### `list-functions`
```
List all functions of the binary.

If a config is provided, this command will also show whether the function is allowed to be mutated.
By default, wasmut will try to load a wasmut.toml file from the current directory

USAGE:
    wasmut list-functions [OPTIONS] <WASMFILE>

ARGS:
    <WASMFILE>
            Path to the wasm module

OPTIONS:
    -c, --config <CONFIG>
            Load wasmut.toml configuration file from the provided path

    -C, --config-samedir
            Attempt to load wasmut.toml from the same directory as the wasm module

    -h, --help
            Print help information

    -V, --version
            Print version information

```

### `list-operators`
```
List all available mutation operators.

If a config is provided, this command will also show whether the operator is enabled or not. By
default, wasmut will try to load a wasmut.toml file from the current directory

USAGE:
    wasmut list-operators [OPTIONS] [WASMFILE]

ARGS:
    <WASMFILE>
            Path to the wasm module

OPTIONS:
    -c, --config <CONFIG>
            Load wasmut.toml configuration file from the provided path

    -C, --config-samedir
            Attempt to load wasmut.toml from the same directory as the wasm module

    -h, --help
            Print help information

    -V, --version
            Print version information

```
### `mutate`
```
Generate and run mutants.

Given a (possibly default) configuration, wasmut will attempt to discover mutants and subsequently
execute them. After that, a report will be generated

USAGE:
    wasmut mutate [OPTIONS] <WASMFILE>

ARGS:
    <WASMFILE>
            Path to the wasm module

OPTIONS:
    -c, --config <CONFIG>
            Load wasmut.toml configuration file from the provided path

    -C, --config-samedir
            Attempt to load wasmut.toml from the same directory as the wasm module

    -h, --help
            Print help information

    -o, --output <OUTPUT>
            Output directory for reports
            
            [default: wasmut-report]

    -r, --report <REPORT>
            Report output format
            
            [default: console]
            [possible values: console, html]

    -t, --threads <THREADS>
            Number of threads to use when executing mutants

    -V, --version
            Print version information

```

### `new-config`
```
Create new configuration file

USAGE:
    wasmut new-config [PATH]

ARGS:
    <PATH>    Path to the new configuration file

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information
```

### `run`
```
Run module without any mutations

USAGE:
    wasmut run [OPTIONS] <WASMFILE>

ARGS:
    <WASMFILE>    Path to the wasm module

OPTIONS:
    -c, --config <CONFIG>    Load wasmut.toml configuration file from the provided path
    -C, --config-samedir     Attempt to load wasmut.toml from the same directory as the wasm module
    -h, --help               Print help information
    -V, --version            Print version information
```