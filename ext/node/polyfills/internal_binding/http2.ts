// Copyright 2018-2026 the Deno authors. MIT license.
// deno-fmt-ignore-file
(function () {
  const { core } = globalThis.__bootstrap;
  const { op_http2_error_string } = core.ops;
  const constants = core.loadExtScript("ext:deno_node/internal/http2/constants.ts");

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

    pushPromise(
      this: {
        pushPromise(headers: string, count: number, options: number): number;
      },
      headers: string,
      count: number,
      options: number,
    ): number {
      // Tests replace this prototype method; otherwise `this` is the native
      // handle, so dispatch falls through to the handle's own `pushPromise` op.
      return this.pushPromise(headers, count, options);
    }
  }

  class Http2Session {
    request(
      this: {
        request(
          headers: string,
          count: number,
          options: number,
          parent: number,
          weight: number,
          exclusive: boolean,
        ): unknown;
      },
      headers: string,
      count: number,
      options: number,
      parent: number,
      weight: number,
      exclusive: boolean,
    ): unknown {
      // Tests replace this prototype method; otherwise `this` is the native
      // handle, so dispatch falls through to the handle's own `request` op.
      return this.request(headers, count, options, parent, weight, exclusive);
    }
  }

  function nghttp2ErrorString(integerCode: number) {
    return op_http2_error_string(integerCode);
  }


  const _defaultExport = {
    constants,
    Http2Session,
    Http2Stream,
    nghttp2ErrorString,
  };

  return {
    constants,
    Http2Session,
    Http2Stream,
    nghttp2ErrorString,
    default: _defaultExport,
  };
})()
