// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  intoCallbackAPIWithIntercept,
  MaybeEmpty,
  notImplemented,
} from "../_utils.ts";

const { readlink: denoReadlink, readlinkSync: denoReadlinkSync } = Deno;

type ReadlinkCallback = (
  err: MaybeEmpty<Error>,
  linkString: MaybeEmpty<string | Uint8Array>
) => void;

interface ReadlinkOptions {
  encoding?: string | null;
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

function getEncoding(
  optOrCallback?: ReadlinkOptions | ReadlinkCallback
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
