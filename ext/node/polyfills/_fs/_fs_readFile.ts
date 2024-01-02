// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  BinaryOptionsArgument,
  FileOptionsArgument,
  getEncoding,
  TextOptionsArgument,
} from "ext:deno_node/_fs/_fs_common.ts";
import { Buffer } from "node:buffer";
import { readAll } from "ext:deno_io/12_io.js";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { pathFromURL } from "ext:deno_web/00_infra.js";
import {
  BinaryEncodings,
  Encodings,
  TextEncodings,
} from "ext:deno_node/_utils.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";

function maybeDecode(data: Uint8Array, encoding: TextEncodings): string;
function maybeDecode(
  data: Uint8Array,
  encoding: BinaryEncodings | null,
): Buffer;
function maybeDecode(
  data: Uint8Array,
  encoding: Encodings | null,
): string | Buffer {
  const buffer = Buffer.from(data.buffer, data.byteOffset, data.byteLength);
  if (encoding && encoding !== "binary") return buffer.toString(encoding);
  return buffer;
}

type TextCallback = (err: Error | null, data?: string) => void;
type BinaryCallback = (err: Error | null, data?: Buffer) => void;
type GenericCallback = (err: Error | null, data?: string | Buffer) => void;
type Callback = TextCallback | BinaryCallback | GenericCallback;
type Path = string | URL | FileHandle;

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

  const encoding = getEncoding(optOrCallback);

  let p: Promise<Uint8Array>;
  if (path instanceof FileHandle) {
    const fsFile = new Deno.FsFile(path.fd);
    p = readAll(fsFile);
  } else {
    p = Deno.readFile(path);
  }

  if (cb) {
    p.then((data: Uint8Array) => {
      if (encoding && encoding !== "binary") {
        const text = maybeDecode(data, encoding);
        return (cb as TextCallback)(null, text);
      }
      const buffer = maybeDecode(data, encoding);
      (cb as BinaryCallback)(null, buffer);
    }, (err) => cb && cb(err));
  }
}

export const readFilePromise = promisify(readFile) as (
  & ((path: Path, opt: TextOptionsArgument) => Promise<string>)
  & ((path: Path, opt?: BinaryOptionsArgument) => Promise<Buffer>)
  & ((path: Path, opt?: FileOptionsArgument) => Promise<Buffer>)
);

export function readFileSync(
  path: string | URL,
  opt: TextOptionsArgument,
): string;
export function readFileSync(
  path: string | URL,
  opt?: BinaryOptionsArgument,
): Buffer;
export function readFileSync(
  path: string | URL,
  opt?: FileOptionsArgument,
): string | Buffer {
  path = path instanceof URL ? pathFromURL(path) : path;
  const data = Deno.readFileSync(path);
  const encoding = getEncoding(opt);
  if (encoding && encoding !== "binary") {
    const text = maybeDecode(data, encoding);
    return text;
  }
  const buffer = maybeDecode(data, encoding);
  return buffer;
}
