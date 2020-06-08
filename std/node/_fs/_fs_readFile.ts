// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { intoCallbackAPIWithIntercept, MaybeEmpty } from "../_utils.ts";

import { getEncoding, FileOptions } from "./_fs_common.ts";
import { Buffer } from "../buffer.ts";
import { fromFileUrl } from "../path.ts";

const { readFile: denoReadFile, readFileSync: denoReadFileSync } = Deno;

type ReadFileCallback = (
  err: MaybeEmpty<Error>,
  data: MaybeEmpty<string | Buffer>
) => void;

function maybeDecode(
  data: Uint8Array,
  encoding: string | null
): string | Buffer {
  const buffer = new Buffer(data.buffer, data.byteOffset, data.byteLength);
  if (encoding) return buffer.toString(encoding);
  return buffer;
}

export function readFile(
  path: string | URL,
  optOrCallback: ReadFileCallback | FileOptions | string | undefined,
  callback?: ReadFileCallback
): void {
  path = path instanceof URL ? fromFileUrl(path) : path;
  let cb: ReadFileCallback | undefined;
  if (typeof optOrCallback === "function") {
    cb = optOrCallback;
  } else {
    cb = callback;
  }

  const encoding = getEncoding(optOrCallback);

  intoCallbackAPIWithIntercept<Uint8Array, string | Buffer>(
    denoReadFile,
    (data: Uint8Array): string | Buffer => maybeDecode(data, encoding),
    cb,
    path
  );
}

export function readFileSync(
  path: string | URL,
  opt?: FileOptions | string
): string | Buffer {
  path = path instanceof URL ? fromFileUrl(path) : path;
  return maybeDecode(denoReadFileSync(path), getEncoding(opt));
}
