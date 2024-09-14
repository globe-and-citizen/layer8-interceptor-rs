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

- [wasm32-target]

  ```sh
  rustup target add wasm32-unknown-unknown
  ```

### Build

1. We first build the application in release mode, making sure to target the wasm32-unknown-unknown target.

   ```sh
   cargo build --release --target wasm32-unknown-unknown
   ```

    > If you run into ` cargo:warning=error: unable to create target: 'No available targets are compatible with triple "wasm32-unknown-unknown"'` you will need to use a newer version of llvm.
    > Please follow the first two steps from the attached documentation to achieve this. [Setup newer llvm/clang.](https://learn.sapio-lang.org/ch01-01-installation.html#local-quickstart)

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

## Tests With Code Coverage

To generate code coverage we use `cargo-llvm-cov`. To install it run:

```sh
cargo install cargo-llvm-cov
```

You can run code coverage by running:

```sh
cargo llvm-cov > test-coverage.txt
```

The result will be in the newly created `test-coverage.txt` file. This can be used to generate a HTML report or a `lcov.info` file.

To generate a HTML report run:

``` sh
cargo llvm-cov --html --open
```

To generate a `lcov.info` file run:

```sh
cargo llvm-cov --workspace --lcov --output-path lcov.info
```

The generated `lcov.info` can be used with IDE tools like [coverage gutters](https://marketplace.visualstudio.com/items?itemName=ryanluker.vscode-coverage-gutters) to watch code coverage.
