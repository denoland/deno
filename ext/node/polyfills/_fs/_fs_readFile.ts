// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  BinaryOptionsArgument,
  FileOptions,
  FileOptionsArgument,
  TextOptionsArgument,
} from "ext:deno_node/_fs/_fs_common.ts";
import { Buffer } from "node:buffer";
import { readAll, readAllSync } from "ext:deno_io/12_io.js";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { pathFromURL } from "ext:deno_web/00_infra.js";
import { Encodings } from "ext:deno_node/_utils.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { getOptions, stringToFlags } from "ext:deno_node/internal/fs/utils.mjs";
import { core } from "ext:core/mod.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import { op_fs_read_file_async, op_fs_read_file_sync } from "ext:core/ops";

const defaultOptions = {
  __proto__: null,
  flag: "r",
};

function maybeDecode(data: Uint8Array, encoding: Encodings): string;
function maybeDecode(
  data: Uint8Array,
  encoding: null | undefined,
): Buffer;
function maybeDecode(
  data: Uint8Array,
  encoding: Encodings | null | undefined,
): string | Buffer {
  const buffer = Buffer.from(data.buffer, data.byteOffset, data.byteLength);
  if (encoding) return buffer.toString(encoding);
  return buffer;
}

type TextCallback = (err: Error | null, data?: string) => void;
type BinaryCallback = (err: Error | null, data?: Buffer) => void;
type GenericCallback = (err: Error | null, data?: string | Buffer) => void;
type Callback = TextCallback | BinaryCallback | GenericCallback;
type Path = string | URL | FileHandle | number;

async function readFileAsync(
  path: string,
  options: FileOptions | undefined,
): Promise<Uint8Array> {
  let cancelRid: number | undefined;
  let abortHandler: (rid: number) => void;
  const flagsNumber = stringToFlags(options!.flag, "options.flag");
  if (options?.signal) {
    options.signal.throwIfAborted();
    cancelRid = core.createCancelHandle();
    abortHandler = () => core.tryClose(cancelRid as number);
    options.signal[abortSignal.add](abortHandler);
  }

  try {
    const read = await op_fs_read_file_async(
      path,
      cancelRid,
      flagsNumber,
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

export function readFile(
  path: Path,
  options: TextOptionsArgument,
  callback: TextCallback,
): void;
export function readFile(
  path: Path,
  options: BinaryOptionsArgument,
  callback: BinaryCallback,
): void;
export function readFile(
  path: Path,
  options: null | undefined | FileOptionsArgument,
  callback: BinaryCallback,
): void;
export function readFile(path: string | URL, callback: BinaryCallback): void;
export function readFile(
  path: Path,
  optOrCallback?: FileOptionsArgument | Callback | null | undefined,
  callback?: Callback,
) {
  path = path instanceof URL ? pathFromURL(path) : path;
  let cb: Callback | undefined;
  if (typeof optOrCallback === "function") {
    cb = optOrCallback;
  } else {
    cb = callback;
  }

  const options = getOptions<FileOptions>(optOrCallback, defaultOptions);

  let p: Promise<Uint8Array>;
  if (path instanceof FileHandle) {
    const fsFile = new FsFile(path.fd, Symbol.for("Deno.internal.FsFile"));
    p = readAll(fsFile);
  } else if (typeof path === "number") {
    const fsFile = new FsFile(path, Symbol.for("Deno.internal.FsFile"));
    p = readAll(fsFile);
  } else {
    p = readFileAsync(path, options);
  }

  if (cb) {
    p.then((data: Uint8Array) => {
      const textOrBuffer = maybeDecode(data, options?.encoding);
      (cb as BinaryCallback)(null, textOrBuffer);
    }, (err) => cb && cb(denoErrorToNodeError(err, { path, syscall: "open" })));
  }
}

export function readFilePromise(
  path: Path,
  options?: FileOptionsArgument | null | undefined,
  // deno-lint-ignore no-explicit-any
): Promise<any> {
  return new Promise((resolve, reject) => {
    readFile(path, options, (err, data) => {
      if (err) reject(err);
      else resolve(data);
    });
  });
}

export function readFileSync(
  path: string | URL | number,
  opt: TextOptionsArgument,
): string;
export function readFileSync(
  path: string | URL | number,
  opt?: BinaryOptionsArgument,
): Buffer;
export function readFileSync(
  path: string | URL | number,
  opt?: FileOptionsArgument,
): string | Buffer {
  path = path instanceof URL ? pathFromURL(path) : path;
  const options = getOptions<FileOptions>(opt, defaultOptions);
  let data;
  if (typeof path === "number") {
    const fsFile = new FsFile(path, Symbol.for("Deno.internal.FsFile"));
    data = readAllSync(fsFile);
  } else {
    const flagsNumber = stringToFlags(options?.flag, "options.flag");
    try {
      data = op_fs_read_file_sync(path, flagsNumber);
    } catch (err) {
      throw denoErrorToNodeError(err, { path, syscall: "open" });
    }
  }
  const textOrBuffer = maybeDecode(data, options?.encoding);
  return textOrBuffer;
}
