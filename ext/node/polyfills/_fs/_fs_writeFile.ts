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
  denoWriteFileErrorToNodeError,
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
import { primordials } from "ext:core/mod.js";
import type { BufferEncoding } from "ext:deno_node/_global.d.ts";
import { URLPrototype } from "ext:deno_web/00_url.js";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";

type WriteFileSyncData =
  | string
  | DataView
  | NodeJS.TypedArray
  | Iterable<NodeJS.TypedArray | string>;

type WriteFileData =
  | string
  | DataView
  | NodeJS.TypedArray
  | AsyncIterable<NodeJS.TypedArray | string>;

const {
  kWriteFileMaxChunkSize,
} = constants;

const {
  ArrayBufferIsView,
  MathMin,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
  Uint8Array,
} = primordials;

interface Writer {
  write(p: NodeJS.TypedArray): Promise<number>;
  writeSync(p: NodeJS.TypedArray): number;
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
  data: WriteFileData,
  options: Encodings | CallbackWithError | WriteFileOptions | undefined,
  callback?: CallbackWithError,
) {
  let flag: string | undefined;
  let mode: number | undefined;
  let signal: AbortSignal | undefined;

  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }

  validateFunction(callback, "callback");

  if (ObjectPrototypeIsPrototypeOf(URLPrototype, pathOrRid)) {
    pathOrRid = pathFromURL(pathOrRid as URL);
  } else if (ObjectPrototypeIsPrototypeOf(FileHandle.prototype, pathOrRid)) {
    pathOrRid = (pathOrRid as FileHandle).fd;
  }

  if (isFileOptions(options)) {
    flag = options.flag;
    mode = options.mode;
    signal = options.signal;
  }

  const encoding = getValidatedEncoding(options) || "utf8";

  if (!ArrayBufferIsView(data) && !isCustomIterable(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  const isRid = typeof pathOrRid === "number";
  let file;

  let error: Error | null = null;
  (async () => {
    try {
      const rid = await getRid(pathOrRid as string | number, flag);
      file = new FsFile(rid, SymbolFor("Deno.internal.FsFile"));
      checkAborted(signal);

      if (!isRid && mode) {
        await Deno.chmod(pathOrRid as string, mode);
        checkAborted(signal);
      }

      await writeAll(
        file,
        data as (Exclude<WriteFileData, string>),
        encoding,
        signal,
      );
    } catch (e) {
      error = denoWriteFileErrorToNodeError(e as Error, { syscall: "write" });
    } finally {
      // Make sure to close resource
      if (!isRid && file) file.close();
      callback(error);
    }
  })();
}

export const writeFilePromise = promisify(writeFile) as (
  pathOrRid: string | number | URL | FileHandle,
  data: WriteFileData,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

export function writeFileSync(
  pathOrRid: string | number | URL,
  data: WriteFileSyncData,
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

  if (!ArrayBufferIsView(data) && !isCustomIterable(data)) {
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

    writeAllSync(
      file,
      data as (Exclude<WriteFileSyncData, string>),
      encoding,
    );
  } catch (e) {
    error = denoWriteFileErrorToNodeError(e as Error, { syscall: "write" });
  } finally {
    // Make sure to close resource
    if (!isRid && file) file.close();
  }

  if (error) throw error;
}

function writeAllSync(
  w: Writer,
  data: Exclude<WriteFileSyncData, string>,
  encoding: BufferEncoding,
) {
  if (!isCustomIterable(data)) {
    data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    let remaining = data.byteLength;
    while (remaining > 0) {
      const bytesWritten = w.writeSync(
        data.subarray(data.byteLength - remaining),
      );
      remaining -= bytesWritten;
    }
  } else {
    for (const buf of data) {
      let toWrite = ArrayBufferIsView(buf) ? buf : Buffer.from(buf, encoding);
      toWrite = new Uint8Array(
        toWrite.buffer,
        toWrite.byteOffset,
        toWrite.byteLength,
      );
      let remaining = toWrite.byteLength;
      while (remaining > 0) {
        const bytesWritten = w.writeSync(
          toWrite.subarray(toWrite.byteLength - remaining),
        );
        remaining -= bytesWritten;
      }
    }
  }
}

async function writeAll(
  w: Writer,
  data: Exclude<WriteFileData, string>,
  encoding: BufferEncoding,
  signal?: AbortSignal,
) {
  if (!isCustomIterable(data)) {
    data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    let remaining = data.byteLength;
    while (remaining > 0) {
      const writeSize = MathMin(kWriteFileMaxChunkSize, remaining);
      const offset = data.byteLength - remaining;
      const bytesWritten = await w.write(
        data.subarray(offset, offset + writeSize),
      );
      remaining -= bytesWritten;
      checkAborted(signal);
    }
  } else {
    for await (const buf of data) {
      checkAborted(signal);
      let toWrite = ArrayBufferIsView(buf) ? buf : Buffer.from(buf, encoding);
      toWrite = new Uint8Array(
        toWrite.buffer,
        toWrite.byteOffset,
        toWrite.byteLength,
      );
      let remaining = toWrite.byteLength;
      while (remaining > 0) {
        const writeSize = MathMin(kWriteFileMaxChunkSize, remaining);
        const offset = toWrite.byteLength - remaining;
        const bytesWritten = await w.write(
          toWrite.subarray(offset, offset + writeSize),
        );
        remaining -= bytesWritten;
        checkAborted(signal);
      }
    }
  }

  checkAborted(signal);
}

function isCustomIterable(
  obj: unknown,
): obj is
  | Iterable<NodeJS.TypedArray | string>
  | AsyncIterable<NodeJS.TypedArray | string> {
  return isIterable(obj) && !ArrayBufferIsView(obj) && typeof obj !== "string";
}

function checkAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new AbortError();
  }
}
