// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const core = globalThis.Deno.core;
const ops = core.ops;
const primordials = globalThis.__bootstrap.primordials;
import * as abortSignal from "internal:deno_web/03_abort_signal.js";
import { pathFromURL } from "internal:runtime/js/06_util.js";
import { open } from "internal:runtime/js/40_files.js";
import { ReadableStreamPrototype } from "internal:deno_web/06_streams.js";
const { ObjectPrototypeIsPrototypeOf } = primordials;

function writeFileSync(
  path,
  data,
  options = {},
) {
  options.signal?.throwIfAborted();
  ops.op_write_file_sync(
    pathFromURL(path),
    options.mode,
    options.append ?? false,
    options.create ?? true,
    options.createNew ?? false,
    data,
  );
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
    if (ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, data)) {
      const file = await open(path, {
        mode: options.mode,
        append: options.append ?? false,
        create: options.create ?? true,
        createNew: options.createNew ?? false,
        write: true,
      });
      await data.pipeTo(file.writable, {
        signal: options.signal,
      });
    } else {
      await core.opAsync(
        "op_write_file_async",
        pathFromURL(path),
        options.mode,
        options.append ?? false,
        options.create ?? true,
        options.createNew ?? false,
        data,
        cancelRid,
      );
    }
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
  if (ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, data)) {
    return writeFile(
      path,
      data.pipeThrough(new TextEncoderStream()),
      options,
    );
  } else {
    const encoder = new TextEncoder();
    return writeFile(path, encoder.encode(data), options);
  }
}

export { writeFile, writeFileSync, writeTextFile, writeTextFileSync };
