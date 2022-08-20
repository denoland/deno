// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";
((window) => {
  const core = window.__bootstrap.core;
  const ops = core.ops;
  const { abortSignal } = window.__bootstrap;
  const { pathFromURL } = window.__bootstrap.util;

  function writeFileSync(
    path,
    data,
    options = {},
  ) {
    options.signal?.throwIfAborted();
    ops.op_write_file_sync({
      path: pathFromURL(path),
      data,
      mode: options.mode,
      append: options.append ?? false,
      create: options.create ?? true,
    });
  }

  async function writeFile(
    path,
    data,
    options = {},
  ) {
    let cancelRid;
    let abortHandler;
    if (options.signal) {
      options.signal.throwIfAborted();
      cancelRid = ops.op_cancel_handle();
      abortHandler = () => core.tryClose(cancelRid);
      options.signal[abortSignal.add](abortHandler);
    }
    try {
      await core.opAsync("op_write_file_async", {
        path: pathFromURL(path),
        data,
        mode: options.mode,
        append: options.append ?? false,
        create: options.create ?? true,
        cancelRid,
      });
    } finally {
      if (options.signal) {
        options.signal[abortSignal.remove](abortHandler);

        // always throw the abort error when aborted
        options.signal.throwIfAborted();
      }
    }
  }

  function writeTextFileSync(
    path,
    data,
    options = {},
  ) {
    const encoder = new TextEncoder();
    return writeFileSync(path, encoder.encode(data), options);
  }

  function writeTextFile(
    path,
    data,
    options = {},
  ) {
    const encoder = new TextEncoder();
    return writeFile(path, encoder.encode(data), options);
  }

  window.__bootstrap.writeFile = {
    writeTextFile,
    writeTextFileSync,
    writeFile,
    writeFileSync,
  };
})(this);
