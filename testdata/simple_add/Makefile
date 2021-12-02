

CC=clang
CFLAGS=--target=wasm32-wasi --sysroot=/opt/wasi-sdk/wasi-sysroot


SRC=simple_add.c test.c
BINARY=test.wasm

OBJ=$(patsubst %.c,%.o.wasm,$(SRC))
OBJ_WAT=$(patsubst %.c,%.o.wat,$(SRC))
BINARY_WAT=$(patsubst %.wasm,%.wat,$(BINARY))

RUNTIME=wasmtime


%.wat : %.wasm
	wasm2wat -o $@ $< 

.PHONY: all
all: $(BINARY) $(BINARY_WAT) $(OBJ_WAT)

%.o.wasm: %.c
	$(CC) $(CFLAGS) -c -O1 -g -o $@ $<

$(BINARY): $(OBJ)
	$(CC) $(CFLAGS) -o $@ $^


.PHONY: clean
clean:
	rm -f $(OBJ)
	rm -f $(OBJ_WAT)

.PHONY: clean-all
clean-all: clean
	rm -f $(BINARY)
	rm -f $(BINARY_WAT)

run: $(BINARY)
	$(RUNTIME) $<

