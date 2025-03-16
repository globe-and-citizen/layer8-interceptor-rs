# Layer8 Interceptor

This repository contains a Rust implementation of the Layer8 Interceptor. Analogous to <https://github.com/globe-and-citizen/layer8-interceptor>

At the time of writing this document, it is a 1:1 port of the original implementation.
The offering for this is a smaller wasm binary size and potentially better performance.

## How To Build

### Prerequisites

- [Rust Toolchain](https://www.rust-lang.org/tools/install)
- [wasm-tooling](https://crates.io/crates/wasm-bindgen)
  - wasm32-unkown-unknown target:

    ```sh
    rustup target add wasm32-unknown-unknown
    ```

  - wasm-bindgen:

    ```sh
    cargo install wasm-bindgen-cli
    ```

  - wasm-pack:

    ```sh
    cargo install wasm-pack
    ```

### Build

We use wasm-pack to build the wasm module for web.

```sh
wasm-pack build --target bundler --all-features --release   
```

> [!NOTE]  
> If you run into `cargo:warning=error: unable to create target: 'No available targets are compatible with triple "wasm32-unknown-unknown"'` you will need to use a newer version of llvm.
> Please follow the first two steps from the attached documentation to achieve this. [Setup newer llvm/clang.](https://learn.sapio-lang.org/ch01-01-installation.html#local-quickstart)

> [!WARNING]  
> (WARNING: LLVM v 19.x.x has breaking changes. So, you'll need to update but not to v19.x.x. Suggested: v18.1.0. To check your version of llvm on Windows, use the command, `$llvm-cov --version` or `$clang --version`)

## Running the interceptor with an example

We can use the We've Got Poems example from the original Layer8 Interceptor repository to test our wasm implementation. We've changed the module used to this wasm implementation.

The example can be found in the [wgp](./service_provider_mock/wgp/) directory. Navigate to it and follow the README for a trial run.

> [!NOTE]
> The wasm module needs to be bootstrapped to the Vue frontend. Please see [vite.config.js](./service_provider_mock/wgp/frontend/vite.config.js) for the configuration.

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

## Usage With Experimental Features

To use experimental features, you can use the `--features experimental` flag when building the wasm module.

```sh
make build-experimental
```

### Using websockets

To use websockets, we can use our library as so:

```js
import { WebSocket } from 'layer8-interceptor-rs'

// code here...
  async mounted() {
    this.socket = new L8WebSocket();
    await this.socket.init({
      url: "example.com",
      proxy: "l8proxy.com"
    });

    this.socket.onmessage = (event) => {
      this.messages.push({ text: event.data, id: Math.random() });
    };

    this.socket.onopen = () => {
      console.log('Connected to the WebSocket server');
    };

    this.socket.onclose = () => {
      console.log('Disconnected from the WebSocket server');
    };
  },
  methods: {
    sendMessage() {
      this.socket.send(this.message);
      this.message = '';
    },
  },

// other code here...
```

Check the [example](./service_provider_mock/tic-tac-toe) for a full example.
