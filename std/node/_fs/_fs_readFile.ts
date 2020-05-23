// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { intoCallbackAPIWithIntercept, MaybeEmpty } from "../_utils.ts";

import { getEncoding, FileOptions } from "./_fs_common.ts";
import { fromFileUrl } from "../path.ts";

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

  intoCallbackAPIWithIntercept<Uint8Array, string | Uint8Array>(
    denoReadFile,
    (data: Uint8Array): string | Uint8Array => maybeDecode(data, encoding),
    cb,
    path
  );
}

export function readFileSync(
  path: string | URL,
  opt?: FileOptions | string
): string | Uint8Array {
  path = path instanceof URL ? fromFileUrl(path) : path;
  return maybeDecode(denoReadFileSync(path), getEncoding(opt));
}
