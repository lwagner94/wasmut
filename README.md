# wasmut
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/lwagner94/wasmut/actions/workflows/ci.yml/badge.svg)](https://github.com/lwagner94/wasmut/actions/workflows/ci.yml)
[![Coverage Status](https://coveralls.io/repos/github/lwagner94/wasmut/badge.svg?branch=main)](https://coveralls.io/github/lwagner94/wasmut?branch=main)

Mutation testing for WebAssembly (Work in progress)

## How to get started


```
# Run wasmut without configuration
wasmut mutate main.wasm

# Create new wasmut.toml configuration file
wasmut init

# wasmut will implicitely use wasmut.toml files in the current directory
wasmut mutate main.wasm

# You can also specify which configuration file to use
wasmut mutate -c wasmut-other.toml main.wasm

# List functions/source-files
wasmut list-functions main.wasm
wasmut list-files main.wasm

```
