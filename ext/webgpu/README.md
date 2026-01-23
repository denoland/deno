# deno_webgpu

This op crate implements the WebGPU API as defined in
https://gpuweb.github.io/gpuweb/ in Deno. The implementation targets the spec
draft as of March 31, 2024. The spec is still very much in flux. This extension
tries to stay up to date with the spec, but is constrained by the features
implemented in our GPU backend library [wgpu](https://github.com/gfx-rs/wgpu).

The spec is still very bare bones, and is still missing many details. As the
spec becomes more concrete, we will implement to follow the spec more closely.

In addition, setting the `DENO_WEBGPU_TRACE` environmental variable will output
a
[wgpu trace](https://github.com/gfx-rs/wgpu/wiki/Debugging-wgpu-Applications#tracing-infrastructure)
to the specified directory.

This op crate is tested primarily by running the
[WebGPU conformance test suite](https://github.com/gpuweb/cts) using `wgpu`'s
[`cts_runner`](https://github.com/gfx-rs/wgpu/blob/trunk/README.md#webgpu-conformance-test-suite).
`cts_runner` also has a few
[directed tests](https://github.com/gfx-rs/wgpu/tree/trunk/cts_runner/tests) to
fill in missing coverage.

GPU availability in GitHub CI is limited, so some configurations rely on
software like DX WARP & Vulkan lavapipe.

## Links

Specification: https://gpuweb.github.io/gpuweb/

Design documents: https://github.com/gpuweb/gpuweb/tree/main/design

Conformance tests suite: https://github.com/gpuweb/cts

WebGPU examples for Deno: https://github.com/crowlKats/webgpu-examples

wgpu-users matrix channel: https://matrix.to/#/#wgpu-users:matrix.org
