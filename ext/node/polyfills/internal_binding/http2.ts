// Copyright 2018-2026 the Deno authors. MIT license.

import { op_http2_http_state } from "ext:core/ops";

// `op_http2_http_state()` returns Uint32Array/Float32Arrays that alias
// thread-local Rust buffers. Using getters defers the call until first runtime
// access so the typed arrays bind to the current process's memory rather than
// snapshot-time memory.
export const http2 = {
  get optionsBuffer() {
    return op_http2_http_state().optionsBuffer;
  },
  get settingsBuffer() {
    return op_http2_http_state().settingsBuffer;
  },
  get sessionState() {
    return op_http2_http_state().sessionState;
  },
  get streamState() {
    return op_http2_http_state().streamState;
  },
};
