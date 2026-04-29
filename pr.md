fix(ext/node): normalize http2 respondWithFile/FD async errors to NGHTTP2_INTERNAL_ERROR (#33656)

### Summary

When `respondWithFile` or `respondWithFD` hit an asynchronous failure (for example after `fs.open` / `fs.fstat`) but the stream has already committed response headers, the stream should be reset with **`NGHTTP2_INTERNAL_ERROR`**, matching Node’s behavior, instead of surfacing the underlying filesystem error on the client stream in inconsistent ways.

### Changes

- **`ext/node/polyfills/http2.ts`**
  - Added `handleAsyncFileResponseError` to centralize the “headers already sent” path: **`closeStream(..., NGHTTP2_INTERNAL_ERROR, kForceRstStream)`** then **`destroy()`** without leaking the raw `fs` error to the peer in that state.
  - Wired **`doSendFD`**, **`doSendFileFD`**, and **`afterOpen`** to use this path when appropriate (including `onError` fallback behavior when headers are not yet committed).

- **`tests/unit_node/http2_test.ts`**
  - Regression: **`respondWithFile`** with a missing path + forced `respond` on next tick → client **`ERR_HTTP2_STREAM_ERROR`** with **`NGHTTP2_INTERNAL_ERROR`** in the message.
  - Regression: **`respondWithFD`** with invalid fd + `statCheck` + forced `respond` on next tick → same client error shape.

### Verification

- `cargo test -p unit_node_tests -- http2_test`
- `./tools/format.js`
- `./tools/lint.js --js`

### Related

- Closes https://github.com/denoland/deno/issues/33656
