// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  intoCallbackAPIWithIntercept,
  MaybeEmpty,
  notImplemented,
} from "../_utils.ts";
import { fromFileUrl } from "../path.ts";

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
  path: string | URL,
  optOrCallback: ReadlinkCallback | ReadlinkOptions,
  callback?: ReadlinkCallback
): void {
  path = path instanceof URL ? fromFileUrl(path) : path;

  let cb: ReadlinkCallback | undefined;
  if (typeof optOrCallback === "function") {
    cb = optOrCallback;
  } else {
    cb = callback;
  }

  const encoding = getEncoding(optOrCallback);

  intoCallbackAPIWithIntercept<string, Uint8Array | string>(
    Deno.readLink,
    (data: string): string | Uint8Array => maybeEncode(data, encoding),
    cb,
    path
  );
}

export function readlinkSync(
  path: string | URL,
  opt?: ReadlinkOptions
): string | Uint8Array {
  path = path instanceof URL ? fromFileUrl(path) : path;

  return maybeEncode(Deno.readLinkSync(path), getEncoding(opt));
}
