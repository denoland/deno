// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  notImplemented,
  intoCallbackAPIWithIntercept,
  MaybeEmpty
} from "./_utils.ts";

import { appendFile, appendFileSync } from "./_fs/_fs_appendFile.ts";
export { appendFile, appendFileSync };

import { chmod, chmodSync } from "./_fs/_fs_chmod.ts";
export { chmod, chmodSync };

import * as constants from "./_fs/_fs_constants.ts";
export { constants };

const {
  readFile: denoReadFile,
  readFileSync: denoReadFileSync,
  readlink: denoReadlink,
  readlinkSync: denoReadlinkSync
} = Deno;

type ReadFileCallback = (
  err: MaybeEmpty<Error>,
  data: MaybeEmpty<string | Uint8Array>
) => void;

interface ReadFileOptions {
  encoding?: string | null;
  flag?: string;
}

type ReadlinkCallback = (
  err: MaybeEmpty<Error>,
  linkString: MaybeEmpty<string | Uint8Array>
) => void;

interface ReadlinkOptions {
  encoding?: string | null;
}

function getEncoding(
  optOrCallback?: ReadFileOptions | ReadFileCallback
): string | null {
  if (!optOrCallback || typeof optOrCallback === "function") {
    return null;
  } else {
    if (optOrCallback.encoding) {
      if (
        optOrCallback.encoding === "utf8" ||
        optOrCallback.encoding === "utf-8"
      ) {
        return "utf8";
      } else if (optOrCallback.encoding === "buffer") {
        return "buffer";
      } else {
        notImplemented();
      }
    }
    return null;
  }
}

function maybeDecode(
  data: Uint8Array,
  encoding: string | null
): string | Uint8Array {
  if (encoding === "utf8") {
    return new TextDecoder().decode(data);
  }
  return data;
}

function maybeEncode(
  data: string,
  encoding: string | null
): string | Uint8Array {
  if (encoding === "buffer") {
    return new TextEncoder().encode(data);
  }
  return data;
}

export function readFile(
  path: string,
  optOrCallback: ReadFileCallback | ReadFileOptions,
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
  opt?: ReadFileOptions
): string | Uint8Array {
  return maybeDecode(denoReadFileSync(path), getEncoding(opt));
}

export function readlink(
  path: string,
  optOrCallback: ReadlinkCallback | ReadlinkOptions,
  callback?: ReadlinkCallback
): void {
  let cb: ReadlinkCallback | undefined;
  if (typeof optOrCallback === "function") {
    cb = optOrCallback;
  } else {
    cb = callback;
  }

  const encoding = getEncoding(optOrCallback);

  intoCallbackAPIWithIntercept<string, Uint8Array | string>(
    denoReadlink,
    (data: string): string | Uint8Array => maybeEncode(data, encoding),
    cb,
    path
  );
}

export function readlinkSync(
  path: string,
  opt?: ReadlinkOptions
): string | Uint8Array {
  return maybeEncode(denoReadlinkSync(path), getEncoding(opt));
}

/** Revist once https://github.com/denoland/deno/issues/4017 lands */
export function access(
  path: string, // eslint-disable-line @typescript-eslint/no-unused-vars
  modeOrCallback: number | Function, // eslint-disable-line @typescript-eslint/no-unused-vars
  callback?: Function // eslint-disable-line @typescript-eslint/no-unused-vars
): void {
  notImplemented("Not yet available");
}

/** Revist once https://github.com/denoland/deno/issues/4017 lands */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function accessSync(path: string, mode?: number): undefined {
  notImplemented("Not yet available");
}
