// Copyright 2018-2026 the Deno authors. MIT license.

import { op_http2_error_string } from "ext:core/ops";
import * as constants from "ext:deno_node/internal/http2/constants.ts";

class Http2Stream {
  respond(
    this: {
      respond(headers: string, count: number, options: number): number;
    },
    headers: string,
    count: number,
    options: number,
  ): number {
    // Tests replace this prototype method; otherwise `this` is the native
    // handle, so dispatch falls through to the handle's own `respond` op.
    return this.respond(headers, count, options);
  }
}

function nghttp2ErrorString(integerCode: number) {
  return op_http2_error_string(integerCode);
}

export { constants, Http2Stream, nghttp2ErrorString };

export default {
  constants,
  Http2Stream,
  nghttp2ErrorString,
};
