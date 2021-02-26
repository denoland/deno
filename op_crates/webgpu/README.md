# deno_webgpu

This op crate implements the WebGPU API as defined in
https://gpuweb.github.io/gpuweb/ in Deno. The implementation targets the spec
draft as of February 22, 2021. The spec is still very much in flux. This op
crate tries to stay up to date with the spec, but is constrained by the features
implemented in our GPU backend library [wgpu](https://github.com/gfx-rs/wgpu).

The spec is still very bare bones, and is still missing many details. As the
spec becomes more concrete, we will implement to follow the spec more closely.

For testing this op crate will make use of the WebGPU conformance tests suite,
running through our WPT runner. This will be used to validate implementation
conformance.

## Links

Specification: https://gpuweb.github.io/gpuweb/ Design documents:
https://github.com/gpuweb/gpuweb/tree/main/design Conformance tests suite:
https://github.com/gpuweb/cts WebGPU examples for Deno:
https://github.com/crowlKats/webgpu-examples Guide for using wgpu in Rust:
https://sotrh.github.io/learn-wgpu/ wgpu-users matrix channel:
https://matrix.to/#/#wgpu-users:matrix.org
