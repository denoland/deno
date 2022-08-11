// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go! <3
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const {
    Uint8Array,
    ArrayPrototypePush,
    MathMin,
    TypedArrayPrototypeSubarray,
    TypedArrayPrototypeSet,
  } = window.__bootstrap.primordials;

  const DEFAULT_BUFFER_SIZE = 32 * 1024;
  // Seek whence values.
  // https://golang.org/pkg/io/#pkg-constants
  const SeekMode = {
    0: "Start",
    1: "Current",
    2: "End",

    Start: 0,
    Current: 1,
    End: 2,
  };

  async function copy(
    src,
    dst,
    options,
  ) {
    let n = 0;
    const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
    const b = new Uint8Array(bufSize);
    let gotEOF = false;
    while (gotEOF === false) {
      const result = await src.read(b);
      if (result === null) {
        gotEOF = true;
      } else {
        let nwritten = 0;
        while (nwritten < result) {
          nwritten += await dst.write(
            TypedArrayPrototypeSubarray(b, nwritten, result),
          );
        }
        n += nwritten;
      }
    }
    return n;
  }

  async function* iter(
    r,
    options,
  ) {
    const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
    const b = new Uint8Array(bufSize);
    while (true) {
      const result = await r.read(b);
      if (result === null) {
        break;
      }

      yield TypedArrayPrototypeSubarray(b, 0, result);
    }
  }

  function* iterSync(
    r,
    options,
  ) {
    const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
    const b = new Uint8Array(bufSize);
    while (true) {
      const result = r.readSync(b);
      if (result === null) {
        break;
      }

      yield TypedArrayPrototypeSubarray(b, 0, result);
    }
  }

  function readSync(rid, buffer) {
    if (buffer.length === 0) {
      return 0;
    }

    const nread = ops.op_read_sync(rid, buffer);

    return nread === 0 ? null : nread;
  }

  async function read(rid, buffer) {
    if (buffer.length === 0) {
      return 0;
    }

    const nread = await core.read(rid, buffer);

    return nread === 0 ? null : nread;
  }

  function writeSync(rid, data) {
    return ops.op_write_sync(rid, data);
  }

  function write(rid, data) {
    return core.write(rid, data);
  }

  const READ_PER_ITER = 64 * 1024; // 64kb

  function readAll(r) {
    return readAllInner(r);
  }
  async function readAllInner(r, options) {
    const buffers = [];
    const signal = options?.signal ?? null;
    while (true) {
      signal?.throwIfAborted();
      const buf = new Uint8Array(READ_PER_ITER);
      const read = await r.read(buf);
      if (typeof read == "number") {
        ArrayPrototypePush(buffers, new Uint8Array(buf.buffer, 0, read));
      } else {
        break;
      }
    }
    signal?.throwIfAborted();

    return concatBuffers(buffers);
  }

  function readAllSync(r) {
    const buffers = [];

    while (true) {
      const buf = new Uint8Array(READ_PER_ITER);
      const read = r.readSync(buf);
      if (typeof read == "number") {
        ArrayPrototypePush(buffers, buf.subarray(0, read));
      } else {
        break;
      }
    }

    return concatBuffers(buffers);
  }

  function concatBuffers(buffers) {
    let totalLen = 0;
    for (const buf of buffers) {
      totalLen += buf.byteLength;
    }

    const contents = new Uint8Array(totalLen);

    let n = 0;
    for (const buf of buffers) {
      TypedArrayPrototypeSet(contents, buf, n);
      n += buf.byteLength;
    }

    return contents;
  }

  function readAllSyncSized(r, size) {
    const buf = new Uint8Array(size + 1); // 1B to detect extended files
    let cursor = 0;

    while (cursor < size) {
      const sliceEnd = MathMin(size + 1, cursor + READ_PER_ITER);
      const slice = buf.subarray(cursor, sliceEnd);
      const read = r.readSync(slice);
      if (typeof read == "number") {
        cursor += read;
      } else {
        break;
      }
    }

    // Handle truncated or extended files during read
    if (cursor > size) {
      // Read remaining and concat
      return concatBuffers([buf, readAllSync(r)]);
    } else { // cursor == size
      return buf.subarray(0, cursor);
    }
  }

  async function readAllInnerSized(r, size, options) {
    const buf = new Uint8Array(size + 1); // 1B to detect extended files
    let cursor = 0;
    const signal = options?.signal ?? null;
    while (cursor < size) {
      signal?.throwIfAborted();
      const sliceEnd = MathMin(size + 1, cursor + READ_PER_ITER);
      const slice = buf.subarray(cursor, sliceEnd);
      const read = await r.read(slice);
      if (typeof read == "number") {
        cursor += read;
      } else {
        break;
      }
    }
    signal?.throwIfAborted();

    // Handle truncated or extended files during read
    if (cursor > size) {
      // Read remaining and concat
      return concatBuffers([buf, await readAllInner(r, options)]);
    } else {
      return buf.subarray(0, cursor);
    }
  }

  window.__bootstrap.io = {
    iterSync,
    iter,
    copy,
    SeekMode,
    read,
    readSync,
    write,
    writeSync,
    readAll,
    readAllInner,
    readAllSync,
    readAllSyncSized,
    readAllInnerSized,
  };
})(this);
