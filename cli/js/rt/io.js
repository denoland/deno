// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

System.register("$deno$/io.ts", [], function (exports_1, context_1) {
  "use strict";
  let EOF, SeekMode;
  const __moduleName = context_1 && context_1.id;
  // https://golang.org/pkg/io/#Copy
  async function copy(dst, src) {
    let n = 0;
    const b = new Uint8Array(32 * 1024);
    let gotEOF = false;
    while (gotEOF === false) {
      const result = await src.read(b);
      if (result === EOF) {
        gotEOF = true;
      } else {
        n += await dst.write(b.subarray(0, result));
      }
    }
    return n;
  }
  exports_1("copy", copy);
  function toAsyncIterator(r) {
    const b = new Uint8Array(1024);
    return {
      [Symbol.asyncIterator]() {
        return this;
      },
      async next() {
        const result = await r.read(b);
        if (result === EOF) {
          return { value: new Uint8Array(), done: true };
        }
        return {
          value: b.subarray(0, result),
          done: false,
        };
      },
    };
  }
  exports_1("toAsyncIterator", toAsyncIterator);
  return {
    setters: [],
    execute: function () {
      exports_1("EOF", (EOF = Symbol("EOF")));
      // Seek whence values.
      // https://golang.org/pkg/io/#pkg-constants
      (function (SeekMode) {
        SeekMode[(SeekMode["SEEK_START"] = 0)] = "SEEK_START";
        SeekMode[(SeekMode["SEEK_CURRENT"] = 1)] = "SEEK_CURRENT";
        SeekMode[(SeekMode["SEEK_END"] = 2)] = "SEEK_END";
      })(SeekMode || (SeekMode = {}));
      exports_1("SeekMode", SeekMode);
    },
  };
});
