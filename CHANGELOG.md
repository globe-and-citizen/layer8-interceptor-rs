# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.0.14] - 2024-03-21

### Added

- Initial implementation of the wasm interceptor as a 1:1 port of the Go implementation here: [Layer8 Interceptor](https://github.com/globe-and-citizen/layer8-interceptor)
- Added WebSocket support and API <https://github.com/globe-and-citizen/layer8-interceptor-rs/pull/7>
- Added a tic-tac-toe example to demonstrate the usage of the WebSocket API <https://github.com/globe-and-citizen/layer8-interceptor-rs/pull/12>

### Changed

- File uploads and loading of assets in the browser is done with gzip compression <https://github.com/globe-and-citizen/layer8-interceptor-rs/pull/4>
