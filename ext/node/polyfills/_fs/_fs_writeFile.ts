// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Encodings } from "ext:deno_node/_utils.ts";
import { pathFromURL } from "ext:deno_web/00_infra.js";
import { Buffer } from "node:buffer";
import {
  CallbackWithError,
  checkEncoding,
  getEncoding,
  getOpenOptions,
  isFileOptions,
  WriteFileOptions,
} from "ext:deno_node/_fs/_fs_common.ts";
import { isWindows } from "ext:deno_node/_util/os.ts";
import {
  AbortError,
  denoErrorToNodeError,
} from "ext:deno_node/internal/errors.ts";
import {
  validateStringAfterArrayBufferView,
} from "ext:deno_node/internal/fs/utils.mjs";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { FileHandle } from "../internal/fs/handle.ts";

interface Writer {
  write(p: Uint8Array): Promise<number>;
}

export function writeFile(
  pathOrFileHandleOrFd: string | number | URL | FileHandle,
  data: string | Uint8Array,
  optOrCallback: Encodings | CallbackWithError | WriteFileOptions | undefined,
  callback?: CallbackWithError,
) {
  const callbackFn: CallbackWithError | undefined =
    optOrCallback instanceof Function ? optOrCallback : callback;
  const options: Encodings | WriteFileOptions | undefined =
    optOrCallback instanceof Function ? undefined : optOrCallback;

  if (!callbackFn) {
    throw new TypeError("Callback must be a function.");
  }

  pathOrFileHandleOrFd = pathOrFileHandleOrFd instanceof URL
    ? pathFromURL(pathOrFileHandleOrFd)
    : pathOrFileHandleOrFd;

  const flag: string | undefined = isFileOptions(options)
    ? options.flag
    : undefined;

  const mode: number | undefined = isFileOptions(options)
    ? options.mode
    : undefined;

  const encoding = checkEncoding(getEncoding(options)) || "utf8";
  const openOptions = getOpenOptions(flag || "w");

  if (!ArrayBuffer.isView(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  if (pathOrFileHandleOrFd instanceof FileHandle) {
    pathOrFileHandleOrFd = pathOrFileHandleOrFd.fd;
  }

  const isFd = typeof pathOrFileHandleOrFd === "number";
  let file;

  let error: Error | null = null;
  (async () => {
    try {
      file = isFd
        ? new FsFile(
          pathOrFileHandleOrFd as number,
          Symbol.for("Deno.internal.FsFile"),
        )
        : await Deno.open(pathOrFileHandleOrFd as string, openOptions);

      // ignore mode because it's not supported on windows
      // TODO(@bartlomieju): remove `!isWindows` when `Deno.chmod` is supported
      if (!isFd && mode && !isWindows) {
        await Deno.chmod(pathOrFileHandleOrFd as string, mode);
      }

      const signal: AbortSignal | undefined = isFileOptions(options)
        ? options.signal
        : undefined;
      await writeAll(file, data as Uint8Array, { signal });
    } catch (e) {
      error = e instanceof AbortError
        ? e
        : e instanceof Error
        ? denoErrorToNodeError(e, { syscall: "write" })
        : new Error("[non-error thrown]");
    } finally {
      // Make sure to close resource
      if (!isFd && file) file.close();
      callbackFn(error);
    }
  })();
}

export const writeFilePromise = promisify(writeFile) as (
  pathOrFdOrFileHandle: string | number | URL | FileHandle,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

export function writeFileSync(
  pathOrFdOrFileHandle: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) {
  pathOrFdOrFileHandle = pathOrFdOrFileHandle instanceof URL
    ? pathFromURL(pathOrFdOrFileHandle)
    : pathOrFdOrFileHandle;

  const flag: string | undefined = isFileOptions(options)
    ? options.flag
    : undefined;

  const mode: number | undefined = isFileOptions(options)
    ? options.mode
    : undefined;

  const encoding = checkEncoding(getEncoding(options)) || "utf8";
  const openOptions = getOpenOptions(flag || "w");

  if (!ArrayBuffer.isView(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  if (pathOrFdOrFileHandle instanceof FileHandle) {
    pathOrFdOrFileHandle = pathOrFdOrFileHandle.fd;
  }

  const isFd = typeof pathOrFdOrFileHandle === "number";
  let file;

  let error: Error | null = null;
  try {
    file = isFd
      ? new FsFile(
        pathOrFdOrFileHandle as number,
        Symbol.for("Deno.internal.FsFile"),
      )
      : Deno.openSync(pathOrFdOrFileHandle as string, openOptions);

    // ignore mode because it's not supported on windows
    // TODO(@bartlomieju): remove `!isWindows` when `Deno.chmod` is supported
    if (!isFd && mode && !isWindows) {
      Deno.chmodSync(pathOrFdOrFileHandle as string, mode);
    }

    // TODO(crowlKats): duplicate from runtime/js/13_buffer.js
    let nwritten = 0;
    while (nwritten < (data as Uint8Array).length) {
      nwritten += file.writeSync((data as Uint8Array).subarray(nwritten));
    }
  } catch (e) {
    error = e instanceof AbortError
      ? e
      : e instanceof Error
      ? denoErrorToNodeError(e, { syscall: "write" })
      : new Error("[non-error thrown]");
  } finally {
    // Make sure to close resource
    if (!isFd && file) file.close();
  }

  if (error) throw error;
}

interface WriteAllOptions {
  offset?: number;
  length?: number;
  signal?: AbortSignal;
}
async function writeAll(
  w: Writer,
  arr: Uint8Array,
  options: WriteAllOptions = {},
) {
  const { offset = 0, length = arr.byteLength, signal } = options;
  checkAborted(signal);

  const written = await w.write(arr.subarray(offset, offset + length));

  if (written === length) {
    return;
  }

  await writeAll(w, arr, {
    offset: offset + written,
    length: length - written,
    signal,
  });
}

function checkAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new AbortError();
  }
}
