// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go! <3
"use strict";

((window) => {
  const core = window.Deno.core;

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

  function readSync(rid, buffer) {
    if (buffer.length === 0) {
      return 0;
    }

    const nread = core.opSync("op_read_sync", rid, buffer);

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
    return core.opSync("op_write_sync", rid, data);
  }

  function write(rid, data) {
    return core.write(rid, data);
  }

  window.__bootstrap.io = {
    SeekMode,
    read,
    readSync,
    write,
    writeSync,
  };
})(this);
