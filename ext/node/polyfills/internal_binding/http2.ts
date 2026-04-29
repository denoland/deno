// Copyright 2018-2026 the Deno authors. MIT license.

import { op_http2_error_string } from "ext:core/ops";
import * as constants from "ext:deno_node/internal/http2/constants.ts";

class Http2Session {
  request(headers, count, options, parent, weight, exclusive) {
    return this.request(headers, count, options, parent, weight, exclusive);
  }
}

class Http2Stream {
  info(headers, count) {
    return this.info(headers, count);
  }
}

function nghttp2ErrorString(integerCode: number) {
  return op_http2_error_string(integerCode);
}

export { constants, Http2Session, Http2Stream, nghttp2ErrorString };

export default {
  constants,
  Http2Session,
  Http2Stream,
  nghttp2ErrorString,
};
