// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Encodings } from "ext:deno_node/_utils.ts";
import { pathFromURL } from "ext:deno_web/00_infra.js";
import { Buffer } from "node:buffer";
import {
  CallbackWithError,
  getValidatedEncoding,
  isFileOptions,
  WriteFileOptions,
} from "ext:deno_node/_fs/_fs_common.ts";
import {
  AbortError,
  denoErrorToNodeError,
} from "ext:deno_node/internal/errors.ts";
import {
  constants,
  validateStringAfterArrayBufferView,
} from "ext:deno_node/internal/fs/utils.mjs";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { openPromise, openSync } from "ext:deno_node/_fs/_fs_open.ts";
import { isIterable } from "ext:deno_node/internal/streams/utils.js";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { primordials } from "ext:core/mod.js";
import type { BufferEncoding } from "ext:deno_node/_global.d.ts";
import { URLPrototype } from "ext:deno_web/00_url.js";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";

const {
  MathMin,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
  Uint8Array,
} = primordials;

interface Writer {
  write(p: Uint8Array): Promise<number>;
}

async function getRid(
  pathOrRid: string | number,
  flag: string = "w",
): Promise<number> {
  if (typeof pathOrRid === "number") {
    return pathOrRid;
  }
  const fileHandle = await openPromise(pathOrRid, flag);
  return fileHandle.fd;
}

function getRidSync(pathOrRid: string | number, flag: string = "w"): number {
  if (typeof pathOrRid === "number") {
    return pathOrRid;
  }
  return openSync(pathOrRid, flag);
}

export function writeFile(
  pathOrRid: string | number | URL | FileHandle,
  data: string | Uint8Array,
  optOrCallback: Encodings | CallbackWithError | WriteFileOptions | undefined,
  callback?: CallbackWithError,
) {
  let flag: string | undefined;
  let mode: number | undefined;
  let signal: AbortSignal | undefined;
  let options: Encodings | WriteFileOptions | undefined;

  if (typeof optOrCallback === "function") {
    callback = optOrCallback;
    options = undefined;
  } else {
    options = optOrCallback;
  }

  validateFunction(callback, "callback");

  pathOrRid = ObjectPrototypeIsPrototypeOf(URLPrototype, pathOrRid)
    ? pathFromURL(pathOrRid as URL)
    : pathOrRid;
  pathOrRid = ObjectPrototypeIsPrototypeOf(FileHandle.prototype, pathOrRid)
    ? (pathOrRid as FileHandle).fd
    : pathOrRid as string | number;

  if (isFileOptions(options)) {
    flag = options.flag;
    mode = options.mode;
    signal = options.signal;
  }

  const encoding = getValidatedEncoding(options) || "utf8";

  if (!isArrayBufferView(data) && !isCustomIterable(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  const isRid = typeof pathOrRid === "number";
  let file;

  let error: Error | null = null;
  (async () => {
    try {
      const rid = await getRid(pathOrRid, flag);
      file = new FsFile(rid, SymbolFor("Deno.internal.FsFile"));
      checkAborted(signal);

      if (!isRid && mode) {
        await Deno.chmod(pathOrRid as string, mode);
        checkAborted(signal);
      }

      await writeAll(file, data, encoding, signal);
    } catch (e) {
      error = denoErrorToNodeError(e as Error, { syscall: "write" });
    } finally {
      // Make sure to close resource
      if (!isRid && file) file.close();
      callback(error);
    }
  })();
}

export const writeFilePromise = promisify(writeFile) as (
  pathOrRid: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

export function writeFileSync(
  pathOrRid: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) {
  let flag: string | undefined;
  let mode: number | undefined;

  pathOrRid = ObjectPrototypeIsPrototypeOf(URLPrototype, pathOrRid)
    ? pathFromURL(pathOrRid as URL)
    : pathOrRid as string | number;

  if (isFileOptions(options)) {
    flag = options.flag;
    mode = options.mode;
  }

  const encoding = getValidatedEncoding(options) || "utf8";

  if (!isArrayBufferView(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  const isRid = typeof pathOrRid === "number";
  let file;

  let error: Error | null = null;
  try {
    const rid = getRidSync(pathOrRid, flag);
    file = new FsFile(rid, SymbolFor("Deno.internal.FsFile"));

    if (!isRid && mode) {
      Deno.chmodSync(pathOrRid as string, mode);
    }

    // TODO(crowlKats): duplicate from runtime/js/13_buffer.js
    let nwritten = 0;
    while (nwritten < (data as Uint8Array).length) {
      nwritten += file.writeSync((data as Uint8Array).subarray(nwritten));
    }
  } catch (e) {
    error = denoErrorToNodeError(e as Error, { syscall: "write" });
  } finally {
    // Make sure to close resource
    if (!isRid && file) file.close();
  }

  if (error) throw error;
}

async function writeAll(
  w: Writer,
  data: Uint8Array | Iterable<Uint8Array>,
  encoding: BufferEncoding,
  signal?: AbortSignal,
) {
  if (!isCustomIterable(data)) {
    data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    let remaining = data.byteLength;
    while (remaining > 0) {
      const writeSize = MathMin(constants.kWriteFileMaxChunkSize, remaining);
      const bytesWritten = await w.write(
        data.subarray(data.byteLength - remaining, writeSize),
      );
      remaining -= bytesWritten;
      checkAborted(signal);
      data = new Uint8Array(
        data.buffer,
        data.byteOffset + bytesWritten,
        data.byteLength - bytesWritten,
      );
    }
  } else {
    for await (const buf of data) {
      checkAborted(signal);
      const toWrite = isArrayBufferView(buf) ? buf : Buffer.from(buf, encoding);
      let remaining = toWrite.byteLength;
      while (remaining > 0) {
        const writeSize = MathMin(constants.kWriteFileMaxChunkSize, remaining);
        const bytesWritten = await w.write(
          toWrite.subarray(toWrite.byteLength - remaining, writeSize),
        );
        remaining -= bytesWritten;
        checkAborted(signal);
      }
    }
  }

  checkAborted(signal);
}

function isCustomIterable(obj: unknown): obj is Iterable<Uint8Array> {
  return isIterable(obj) && !isArrayBufferView(obj) && typeof obj !== "string";
}

function checkAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new AbortError();
  }
}
