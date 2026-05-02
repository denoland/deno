// Copyright 2018-2026 the Deno authors. MIT license.

// Subset of node's `internal/http2/core` module - exposes the internal
// HTTP/2 classes that some tests use for `instanceof` checks.

import {
  ClientHttp2Session,
  Http2Session,
  Http2Stream,
  ServerHttp2Session,
} from "node:http2";

export { ClientHttp2Session, Http2Session, Http2Stream, ServerHttp2Session };

export default {
  ClientHttp2Session,
  Http2Session,
  Http2Stream,
  ServerHttp2Session,
};
