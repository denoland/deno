// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const ops = core.ops;
import { pathFromURL } from "internal:runtime/js/06_util.js";
import * as abortSignal from "internal:deno_web/03_abort_signal.js";

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

export { readFile, readFileSync, readTextFile, readTextFileSync };
