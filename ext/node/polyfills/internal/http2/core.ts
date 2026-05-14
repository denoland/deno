// Copyright 2018-2026 the Deno authors. MIT license.

// Subset of node's `internal/http2/core` module - exposes the internal
// HTTP/2 classes that some tests use for `instanceof` checks.

(function () {
const { core } = globalThis.__bootstrap;
const lazyHttp2 = core.createLazyLoader("node:http2");

return {
  get ClientHttp2Session() {
    return lazyHttp2().ClientHttp2Session;
  },
  get Http2Session() {
    return lazyHttp2().Http2Session;
  },
  get Http2Stream() {
    return lazyHttp2().Http2Stream;
  },
  get ServerHttp2Session() {
    return lazyHttp2().ServerHttp2Session;
  },
  get default() {
    return {
      ClientHttp2Session: lazyHttp2().ClientHttp2Session,
      Http2Session: lazyHttp2().Http2Session,
      Http2Stream: lazyHttp2().Http2Stream,
      ServerHttp2Session: lazyHttp2().ServerHttp2Session,
    };
  },
};
})();
