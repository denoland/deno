// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Encodings } from "ext:deno_node/_utils.ts";
import { pathFromURL } from "ext:deno_web/00_infra.js";
import { Buffer } from "node:buffer";
import {
  CallbackWithError,
  checkEncoding,
  getEncoding,
  isFileOptions,
  WriteFileOptions,
} from "ext:deno_node/_fs/_fs_common.ts";
import {
  AbortError,
  denoErrorToNodeError,
} from "ext:deno_node/internal/errors.ts";
import {
  validateStringAfterArrayBufferView,
} from "ext:deno_node/internal/fs/utils.mjs";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { openPromise, openSync } from "ext:deno_node/_fs/_fs_open.ts";

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
  const callbackFn: CallbackWithError | undefined =
    optOrCallback instanceof Function ? optOrCallback : callback;
  const options: Encodings | WriteFileOptions | undefined =
    optOrCallback instanceof Function ? undefined : optOrCallback;

  if (!callbackFn) {
    throw new TypeError("Callback must be a function.");
  }

  pathOrRid = pathOrRid instanceof URL ? pathFromURL(pathOrRid) : pathOrRid;
  pathOrRid = pathOrRid instanceof FileHandle ? pathOrRid.fd : pathOrRid;

  const flag: string | undefined = isFileOptions(options)
    ? options.flag
    : undefined;

  const mode: number | undefined = isFileOptions(options)
    ? options.mode
    : undefined;

  const encoding = checkEncoding(getEncoding(options)) || "utf8";

  if (!ArrayBuffer.isView(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  const isRid = typeof pathOrRid === "number";
  let file;

  let error: Error | null = null;
  (async () => {
    try {
      const rid = await getRid(pathOrRid, flag);
      file = new FsFile(rid, Symbol.for("Deno.internal.FsFile"));

      if (!isRid && mode) {
        await Deno.chmod(pathOrRid as string, mode);
      }

      const signal: AbortSignal | undefined = isFileOptions(options)
        ? options.signal
        : undefined;
      await writeAll(file, data as Uint8Array, { signal });
    } catch (e) {
      error = e instanceof Error
        ? denoErrorToNodeError(e, { syscall: "write" })
        : new Error("[non-error thrown]");
    } finally {
      // Make sure to close resource
      if (!isRid && file) file.close();
      callbackFn(error);
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
  pathOrRid = pathOrRid instanceof URL ? pathFromURL(pathOrRid) : pathOrRid;

  const flag: string | undefined = isFileOptions(options)
    ? options.flag
    : undefined;

  const mode: number | undefined = isFileOptions(options)
    ? options.mode
    : undefined;

  const encoding = checkEncoding(getEncoding(options)) || "utf8";

  if (!ArrayBuffer.isView(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  const isRid = typeof pathOrRid === "number";
  let file;

  let error: Error | null = null;
  try {
    const rid = getRidSync(pathOrRid, flag);
    file = new FsFile(rid, Symbol.for("Deno.internal.FsFile"));

    if (!isRid && mode) {
      Deno.chmodSync(pathOrRid as string, mode);
    }

    // TODO(crowlKats): duplicate from runtime/js/13_buffer.js
    let nwritten = 0;
    while (nwritten < (data as Uint8Array).length) {
      nwritten += file.writeSync((data as Uint8Array).subarray(nwritten));
    }
  } catch (e) {
    error = e instanceof Error
      ? denoErrorToNodeError(e, { syscall: "write" })
      : new Error("[non-error thrown]");
  } finally {
    // Make sure to close resource
    if (!isRid && file) file.close();
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
