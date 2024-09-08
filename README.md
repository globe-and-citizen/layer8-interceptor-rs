# Layer8 Interceptor

This repository contains a Rust implementation of a Layer8 Interceptor. Analogous to <https://github.com/globe-and-citizen/layer8-interceptor>

At the time of writing this document, it is a 1:1 port of the original implementation.
The offering for this is a smaller wasm binary size and potentially better performance.

## How To Build

### Prerequisites

- [Rust Toolchain](https://www.rust-lang.org/tools/install)
- [wasm-bindgen](https://crates.io/crates/wasm-bindgen)

    ```sh
    cargo install wasm-bindgen-cli
    ```

- [wasm-opt](https://crates.io/crates/wasm-opt)

    ```sh
    cargo install wasm-opt
    ```

### Build

1. We first build the application in release mode, making sure to target the wasm32-unknown-unknown target. 

    ```sh
    cargo build --release --target wasm32-unknown-unknown 
    ```

2. Next we use wasm-bindgen to generate the JavaScript bindings for our Rust code.

    ```sh
    wasm-bindgen --out-dir out ./target/wasm32-unknown-unknown/release/layer8_interceptor_rs.wasm 
    ``` 

3. Lastly we can use wasm-opt to optimize the generated wasm file for size.

    ```sh
    wasm-opt -Oz -o out/layer8_interceptor_rs_bg.wasm out/layer8_interceptor_rs_bg.wasm
    ```

## Running the interceptor with an example

🚧 Work in progress 🚧
