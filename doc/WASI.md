id: WASI
title: Building with WASI-SDK

# Building with WASI-SDK

## WASI

- [WebAssembly System Interface](https://github.com/WebAssembly/WASI)
- [WASI High Level Goals](https://github.com/WebAssembly/WASI?tab=readme-ov-file#wasi-high-level-goals)

## WASI High Level Goals

(In the spirit of [WebAssembly's High-Level Goals](https://github.com/WebAssembly/design/blob/main/HighLevelGoals.md).)

1. Define a set of portable, modular, runtime-independent, and
   WebAssembly-native APIs which can be used by WebAssembly code to interact
   with the outside world. These APIs preserve the essential sandboxed nature of
   WebAssembly through a [Capability-based] API design.
2. Specify and implement incrementally. Start with a Minimum Viable Product
   (MVP), then adding additional features, prioritized by feedback and
   experience.
3. Supplement API designs with documentation and tests, and, when feasible,
   reference implementations which can be shared between wasm engines.
4. Make a great platform:
    * Work with WebAssembly tool and library authors to help them provide
      WASI support for their users.
    * When being WebAssembly-native means the platform isn't directly
      compatible with existing applications written for other platforms,
      design to enable compatibility to be provided by tools and libraries.
    * Allow the overall API to evolve over time; to make changes to API
      modules that have been standardized, build implementations of them
      using libraries on top of new API modules to provide compatibility.

[Capability-based]: https://en.wikipedia.org/wiki/Capability-based_security



## Install dependencies

See [Building and Running](https://github.com/tmikov/hermes/blob/master/doc/BuildingAndRunning.md):

Hermes is a C++17 project. clang, gcc, and Visual C++ are supported. Hermes also requires cmake, git, ICU, Python. It builds with [CMake](https://cmake.org) and [ninja](https://ninja-build.org).

The Hermes REPL will also use libreadline, if available.

To install dependencies on Ubuntu:

    apt install build-essential cmake git ninja-build libicu-dev python3 tzdata libreadline-dev

On Arch Linux:

    pacman -S cmake git ninja icu python zip readline

On Mac via Homebrew:

    brew install cmake git ninja
    
Python `pygments` is also a dependency.

    sudo apt install python3-pip
    python3 -m pip install pygments
    
### Static Hermes

    git clone --branch shermes-wasm https://github.com/tmikov/hermes
    
### WASI-SDK

[Download SDK packages here.](https://github.com/WebAssembly/wasi-sdk/releases). 

Or, fetch with `wget` or `curl`

    wget --show-progress --progress=bar -H -O wasi-sdk.tar.gz \
    'https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-25/wasi-sdk-25.0-x86_64-linux.tar.gz' \
    && tar -xf wasi-sdk.tar.gz \
    && mv wasi-sdk-25.0-x86_64-linux wasi-sdk \
    && rm wasi-sdk.tar.gz 
   
## Building

Follow part of instructions here [Building with Emscripten](https://github.com/tmikov/hermes/blob/master/doc/Emscripten.md) 
and here [Cross Compilation](https://github.com/tmikov/hermes/blob/master/doc/CrossCompilation.md) for building `hermes`, `shermes`, and `hermesc`.

### Build host

    mkdir hermes-builds
    cd hermes-builds
    export HermesSourcePath=../hermes
    export WasiSdk=../wasi-sdk
    cmake -G Ninja -DCMAKE_BUILD_TYPE=Release -S ${HermesSourcePath?} -B build-host
    cmake --build build-host --target hermesc --target hermes --target shermes --parallel
    
### Compile the Hermes VM to Wasm with WASI support

    cmake -G Ninja -S ${HermesSourcePath?} -B build-wasm \
    -DCMAKE_BUILD_TYPE=MinSizeRel \
    -DCMAKE_TOOLCHAIN_FILE=${WasiSdk?}/share/cmake/wasi-sdk.cmake \
    -DIMPORT_HOST_COMPILERS=build-host/ImportHostCompilers.cmake \
    -DHERMES_UNICODE_LITE=ON \
    -DLLVM_ENABLE_THREADS=0 \
    -DHERMES_ALLOW_BOOST_CONTEXT=0 \
    -DHERMES_CHECK_NATIVE_STACK=OFF
    #  Build the VM and libraries
    cmake --build build-wasm --target sh-demo --parallel
    
## JavaScript source code

We're running this JavaScript [demo.js](https://github.com/tmikov/hermes/blob/shermes-wasm/tools/sh-demo/demo.js) compiled to WASM using WASI runtime 

```
function createCounterWithGenerator() {
    let count = 0; // Shared mutable variable

    const increment = (by = 1) => count += by;

    function* doSteps(steps) {
        for (let i = 0; i < steps; i++)
            yield increment(); // Use the default parameter for `by` in `increment`
    }

    return {
        increment,
        doSteps
    };
}

console.log("Closures\n=========");
const counter = createCounterWithGenerator();
const steps = 5;

console.log(`Generating ${steps} increments with default step size:`);
for (const value of counter.doSteps(steps))
    console.log(value);

console.log("Further increments:");
for (const value of counter.doSteps(3))
    console.log(value);

// ===================================================================

function show_tdz() {
    function getval() {
        return val;
    }
    let val = getval() + 1;
}

console.log("\nTDZ\n=========");
try {
    show_tdz();
} catch (e) {
    console.log("TDZ Error!", e.stack);
}

// ===================================================================

const prototypeObj = {
    first: "I am in the prototype"
};

const obj = {
    get second() {
        // Add and increment the `third` property
        if (!this.third) {
            this.third = 1; // Initialize if it doesn't exist
        } else {
            this.third++;
        }
        return `Getter executed, third is now ${this.third}`;
    },

    __proto__: prototypeObj // Set prototype using object literal syntax
};

console.log("\nPrototypical Inheritance\n=========");

console.log("First property:", obj.first); // Inherited from prototype
console.log("Second property:", obj.second); // Triggers the getter
console.log("Third property:", obj.third); // Dynamically added and incremented
console.log("Second property again:", obj.second); // Getter increments third
console.log("Third property now:", obj.third); // Reflects incremented value

// ===================================================================

class PrototypeClass {
    constructor() {}

    // Define `first` as a getter in the prototype
    get first() {
        return "I am in the prototype";
    }
}

class DerivedClass extends PrototypeClass {
    constructor() {
        super();
    }

    get second() {
        // Add and increment the `third` property
        if (!this.third) {
            this.third = 1; // Initialize if it doesn't exist
        } else {
            this.third++;
        }
        return `Getter executed, third is now ${this.third}`;
    }
}

console.log("\nClasses\n=========");

const clInst = new DerivedClass();

console.log("First property:", clInst.first); // Inherited from PrototypeClass
console.log("Second property:", clInst.second); // Triggers the getter
console.log("Third property:", clInst.third); // Dynamically added and incremented
console.log("Second property again:", clInst.second); // Getter increments third
console.log("Third property now:", clInst.third); // Reflects incremented value
```

## Run sh-demo

    wasmtime build-wasm/tools/sh-demo/sh-demo
    
or

    wasmer build-wasm/tools/sh-demo/sh-demo
    
or

     wasmtime compile --optimize opt-level=s build-wasm/tools/sh-demo/sh-demo
     wasmtime --allow-precompiled sh-demo.cwasm

    
```
Closures
=========
Generating 5 increments with default step size:
1
2
3
4
5
Further increments:
6
7
8

TDZ
=========

Prototypical Inheritance
=========
First property: I am in the prototype
Second property: Getter executed, third is now 1
Third property: 1
Second property again: Getter executed, third is now 2
Third property now: 2

Classes
=========
First property: I am in the prototype
Second property: Getter executed, third is now 1
Third property: 1
Second property again: Getter executed, third is now 2
Third property now: 2
```
    
## Emit C from JavaScript source with shermes, compile to WASM with WASI-SDK clang and clang++

Within `hermes-builds` directory

```
cat hello.js
var x = "hello"; print(`${x} world`);
```

### Emit C

```
build-host/bin/shermes -Xenable-tdz -emit-c hello.js
```
### Compile to `.o` file with WASI-SDK's `clang` 

```
../wasi-sdk/bin/wasm32-wasi-clang hello.c -c \
  -O3 \
  -DNDEBUG \
  -fno-strict-aliasing -fno-strict-overflow \
  -I./build-wasm/lib/config \
  -I../hermes/include \
  -mllvm -wasm-enable-sjlj \
  -o hello
```

### Compile to `.wasm` file WASI-SDK's `clang++`

```
../wasi-sdk/bin/clang++ -O3 hello.o ./build-wasm/tools/sh-demo/CMakeFiles/sh-demo.dir/cxa.cpp.obj \
  -o hello.wasm \
  -L./build-wasm/lib \
  -L./build-wasm/jsi \
  -L./build-wasm/tools/shermes \
  -lshermes_console_a -lhermesvmlean_a -ljsi -lwasi-emulated-mman
```

### Test

```
wasmtime hello.wasm
hello world
```

```
wasmer hello.wasm
hello world
```


### Shell script

```
#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

set -e  # Exit immediately if a command exits with a non-zero status
set -u  # Treat unset variables as an error and exit immediately

# Extract the filename without path and extension
file_name=$(basename "$1")   # Remove path
file_name="${file_name%.*}"      # Remove extension

rm -rf out

mkdir out

out="$PWD/out"

rm -rf ${file_name}.c ${file_name}.o ${file_name}.wasm ${file_name}.hbc ${file_name}

./build-host/bin/hermes ${file_name}.js --emit-binary -out "${out}/${file_name}.hbc"

./build-host/bin/shermes -v -Os -g -static-link -Xenable-tdz -emit-c "${file_name}.js" -o "${out}/${file_name}.c"

./build-host/bin/shermes -v -Os -g -Xenable-tdz ${file_name}.js -o "${out}/${file_name}" 

../wasi-sdk/bin/wasm32-wasi-clang "${out}/${file_name}.c" -c \
  -O3 \
  -DNDEBUG \
  -fno-strict-aliasing -fno-strict-overflow \
  -I./build-wasm/lib/config \
  -I../hermes/include \
  -mllvm -wasm-enable-sjlj \
  -o "${out}/${file_name}.o"

../wasi-sdk/bin/clang++ -O3 "${out}/${file_name}.o" ./build-wasm/tools/sh-demo/CMakeFiles/sh-demo.dir/cxa.cpp.obj -o "${out}/${file_name}.wasm" \
  -L./build-wasm/lib \
  -L./build-wasm/jsi \
  -L./build-wasm/tools/shermes \
  -lshermes_console_a -lhermesvmlean_a -ljsi -lwasi-emulated-mman

../wasi-sdk/bin/strip "${out}/${file_name}.wasm"

ls -lh "${out}"
```

`./wasm-standalone-test.sh hello.js`

```
./wasm-standalone-test.sh hello.js
/usr/bin/cc /tmp/hello.js-c2d6b3.c -Os -I/media/user/123/hermes-builds/build-host/lib/config -I/media/user/123/hermes/include -DNDEBUG -g -fno-strict-aliasing -fno-strict-overflow -L/media/user/123/hermes-builds/build-host/lib -L/media/user/123/hermes-builds/build-host/jsi -L/media/user/123/hermes-builds/build-host/tools/shermes -lshermes_console -Wl,-rpath /media/user/123/hermes-builds/build-host/lib -Wl,-rpath /media/user/123/hermes-builds/build-host/jsi -Wl,-rpath /media/user/123/hermes-builds/build-host/tools/shermes -lm -lhermesvm -o /media/user/123/hermes-builds/out/hello
In file included from /media/user/123/hermes-builds/out/hello.c:2:
../hermes/include/hermes/VM/static_h.h:334:2: warning: "JS exceptions are currenly broken with WASI" [-W#warnings]
  334 | #warning "JS exceptions are currenly broken with WASI"
      |  ^
../hermes/include/hermes/VM/static_h.h:336:39: warning: omitting the parameter name in a function definition is a C23 extension [-Wc23-extensions]
  336 | static inline int _noop_setjmp(jmp_buf) {
      |                                       ^
../hermes/include/hermes/VM/static_h.h:339:41: warning: omitting the parameter name in a function definition is a C23 extension [-Wc23-extensions]
  339 | static inline void _noop_longjmp(jmp_buf, int) {
      |                                         ^
../hermes/include/hermes/VM/static_h.h:339:46: warning: omitting the parameter name in a function definition is a C23 extension [-Wc23-extensions]
  339 | static inline void _noop_longjmp(jmp_buf, int) {
      |                                              ^
4 warnings generated.
total 1.6M
-rwxrwxr-x 1 user user  29K Jan  4 14:23 hello
-rw-rw-r-- 1 user user  12K Jan  4 14:23 hello.c
-rw-rw-r-- 1 user user  700 Jan  4 14:23 hello.hbc
-rw-rw-r-- 1 user user 4.3K Jan  4 14:23 hello.o
-rwxrwxr-x 1 user user 1.5M Jan  4 14:23 hello.wasm

```
