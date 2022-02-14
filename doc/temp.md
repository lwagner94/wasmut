If you run `wasmut mutate` without providing any configuration file, 
`wasmut` will simply fall back to its default options. For trivial WebAssembly modules, like the 
`simple_add` example, this is okay. However, once you start using `wasmut` for 
any real-world code bases, or even just another language such as Go or Rust,
you will quickly run into a problem. In `wasmut`'s default configuration,
every single instruction in the module might be subject to mutations, including 
library code and test code. 
To restrict this, simply provide a configuration file for `wasmut` using
the `-C` or `-c` flag. `-C` will look for a file called `wasmut.toml` in the 
same directory as the module, while `-c` allows you to provide the full path to the configuration file.
Also, `wasmut` will always try to load a `wasmut.toml` file from the current directory.


```
> ls  testdata/simple_go/
... test.wasm  wasmut.toml ...

> # Load wasmut.toml from module directory
> wasmut mutate testdata/simple_go/test.wasm -C
[INFO ] Loading configuration file from module directory: "testdata/simple_go/wasmut.toml"
[INFO ] Using 8 workers
```
```
> # Load wasmut.toml by proving the full path
> wasmut mutate testdata/simple_go/test.wasm -c testdata/simple_go/wasmut.toml
[INFO ] Loading user-specified configuration file "testdata/simple_go/wasmut.toml"
[INFO ] Using 8 workers
```

```
> cp testdata/simple_go/wasmut.toml .
> ls  testdata/simple_go/
... testdata  wasmut.toml ...

> # wasmut will now load wasmut.toml from the current directory
> wasmut mutate testdata/simple_go/test.wasm
[INFO ] Loading default configuration file "wasmut.toml"
[INFO ] Using 8 workers
```

To create a new configuration, you can use the `new-config` command:
```
# Create new wasmut.toml configuration file
> wasmut create-config
[INFO ] Created new configuration file wasmut.toml

# You can also provide a custom path for the new configuration
> wasmut new-config wasmut-custom.toml
[INFO ] Created new configuration file wasmut-custom.toml
```