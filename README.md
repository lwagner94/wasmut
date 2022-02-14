# wasmut
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/lwagner94/wasmut/actions/workflows/ci.yml/badge.svg)](https://github.com/lwagner94/wasmut/actions/workflows/ci.yml)
[![Coverage Status](https://coveralls.io/repos/github/lwagner94/wasmut/badge.svg?branch=main)](https://coveralls.io/github/lwagner94/wasmut?branch=main)

`wasmut` is a mutation testing tool for WebAssembly [WASI](https://wasi.dev/) modules.

## Installation

`wasmut` is implemented in Rust, thus you need the Rust toolchain to compile the
project. The *minimum supported Rust version (MSRV)* is 1.58.

To install `wasmut` directly from [crates.io](https://crates.io/crates/wasmut), simply run
```sh
> cargo install wasmut
```
which will install `wasmut` to `$HOME/.cargo/bin` by default. Make sure that 
this path is included in our `$PATH` variable.

Alternatively, you can also install the latest development version of `wasmut` by cloning 
the git repository.
```sh
> # The --recursive flag is only needed if you want to run the unit tests.
> git clone --recursive https://github.com/lwagner94/wasmut
> cd wasmut
> cargo install --path .
```

## Quick start
Once installed, you can start using `wasmut`. To start off, you can 
try out some of the examples in the `testdata` folder. 
If you want to use `wasmut` with any of your own modules, 
please be sure to check out the [WebAssembly Module Requirements](doc/module_requirements.md) chapter.

If you run the `mutate` command without any flags, `wasmut`
will try to load a file called `wasmut.toml` in the current 
directory and will fall back to default options if it cannot find it.
```sh
> # Run wasmut using default options (no filtering, all operators)
> wasmut mutate testdata/simple_add/test.wasm
[INFO ] No configuration file found or specified, using default config
[INFO ] Using 8 workers
[INFO ] Generated 37 mutations
...
```

Using the `-C/-c` flags, you can instruct wasmut to load 
a configuration file from a different path. The `-C` flag will try
to load `wasmut.toml` from the same directory as the module, while `-c` allows you to provide the full path to the configuration file.

```sh
> wasmut mutate testdata/simple_add/test.wasm -C
[INFO ] Loading configuration file from module directory: "testdata/simple_add/wasmut.toml"
[INFO ] Using 8 workers
[INFO ] Generated 1 mutations
[INFO ] Original module executed in 40 cycles
[INFO ] Setting timeout to 80 cycles
/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c:3:14: 
KILLED: binop_add_to_sub: Replaced I32Add with I32Sub
    return a + b;
              ^

ALIVE           0
TIMEOUT         0
ERROR           0
KILLED          1
Mutation score  100%

```

By default, `wasmut` will print the results to the console - as shown above.
If you add the `--report html` option, `wasmut` will 
create a HTML report in the `wasmut-report` folder.

```sh
> wasmut mutate testdata/simple_go/test.wasm -C --report html
[INFO ] Loading configuration file from module directory: "testdata/simple_go/wasmut.toml"
[INFO ] Using 8 workers
...
```

![](doc/images/html_index.png)
![](doc/images/html_detail.png)


## Details
  - [Mutation Operators](doc/operators.md)
  - [Configuration File Reference](doc/configuration.md)
  - [Command Line Options](doc/cli.md)
  - [WebAssembly Module Requirements](doc/module_requirements.md)

## Authors
`wasmut` was developed by Lukas Wagner.

## License
Copyright © 2021-2022 Lukas Wagner.

All code is licensed under the MIT license. See [LICENSE.txt](LICENSE.txt) file for more information.
