// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const { pathFromURL } = window.__bootstrap.util;
  const { abortSignal } = window.__bootstrap;

  function readFileSync(path) {
    return ops.op_readfile_sync(pathFromURL(path));
  }

  async function readFile(path, options) {
    let cancelRid;
    let abortHandler;
    if (options?.signal) {
      options.signal.throwIfAborted();
      cancelRid = ops.op_cancel_handle();
      abortHandler = () => core.tryClose(cancelRid);
      options.signal[abortSignal.add](abortHandler);
    }

    try {
      const read = await core.opAsync(
        "op_readfile_async",
        pathFromURL(path),
        cancelRid,
      );
      return read;
    } finally {
      if (options?.signal) {
        options.signal[abortSignal.remove](abortHandler);

        // always throw the abort error when aborted
        options.signal.throwIfAborted();
      }
    }
  }

  function readTextFileSync(path) {
    return ops.op_readfile_text_sync(pathFromURL(path));
  }

  async function readTextFile(path, options) {
    let cancelRid;
    let abortHandler;
    if (options?.signal) {
      options.signal.throwIfAborted();
      cancelRid = ops.op_cancel_handle();
      abortHandler = () => core.tryClose(cancelRid);
      options.signal[abortSignal.add](abortHandler);
    }

    try {
      const read = await core.opAsync(
        "op_readfile_text_async",
        pathFromURL(path),
        cancelRid,
      );
      return read;
    } finally {
      if (options?.signal) {
        options.signal[abortSignal.remove](abortHandler);

        // always throw the abort error when aborted
        options.signal.throwIfAborted();
      }
    }
  }

  window.__bootstrap.readFile = {
    readFile,
    readFileSync,
    readTextFileSync,
    readTextFile,
  };
})(this);
