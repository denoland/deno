// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go! <3

((window) => {
  const DEFAULT_BUFFER_SIZE = 32 * 1024;
  const { sendSync, sendAsync } = window.__bootstrap.dispatchMinimal;
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
          nwritten += await dst.write(b.subarray(nwritten, result));
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

      yield b.subarray(0, result);
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

      yield b.subarray(0, result);
    }
  }

  function readSync(rid, buffer) {
    if (buffer.length === 0) {
      return 0;
    }

    const nread = sendSync("op_read", rid, buffer);
    if (nread < 0) {
      throw new Error("read error");
    }

    return nread === 0 ? null : nread;
  }

  async function read(
    rid,
    buffer,
  ) {
    if (buffer.length === 0) {
      return 0;
    }

    const nread = await sendAsync("op_read", rid, buffer);
    if (nread < 0) {
      throw new Error("read error");
    }

    return nread === 0 ? null : nread;
  }

  function writeSync(rid, data) {
    const result = sendSync("op_write", rid, data);
    if (result < 0) {
      throw new Error("write error");
    }

    return result;
  }

  async function write(rid, data) {
    const result = await sendAsync("op_write", rid, data);
    if (result < 0) {
      throw new Error("write error");
    }

    return result;
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
  };
})(this);
