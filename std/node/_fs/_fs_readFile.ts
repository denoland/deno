// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  notImplemented,
  intoCallbackAPIWithIntercept,
  MaybeEmpty,
} from "../_utils.ts";

import { getEncoding, FileOptions } from "./_fs_common.ts";

const { readFile: denoReadFile, readFileSync: denoReadFileSync } = Deno;

type ReadFileCallback = (
  err: MaybeEmpty<Error>,
  data: MaybeEmpty<string | Uint8Array>
) => void;

function maybeDecode(
  data: Uint8Array,
  encoding: string | null
): string | Uint8Array {
  if (encoding === "utf8") {
    return new TextDecoder().decode(data);
  }
  return data;
}

export function readFile(
  path: string,
  optOrCallback: ReadFileCallback | FileOptions,
  callback?: ReadFileCallback
): void {
  let cb: ReadFileCallback | undefined;
  if (typeof optOrCallback === "function") {
    cb = optOrCallback;
  } else {
    cb = callback;
  }

  const encoding = getEncoding(optOrCallback);

  intoCallbackAPIWithIntercept<Uint8Array, string | Uint8Array>(
    denoReadFile,
    (data: Uint8Array): string | Uint8Array => maybeDecode(data, encoding),
    cb,
    path
  );
}

export function readFileSync(
  path: string,
  opt?: FileOptions
): string | Uint8Array {
  return maybeDecode(denoReadFileSync(path), getEncoding(opt));
}
