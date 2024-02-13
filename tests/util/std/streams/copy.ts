// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { DEFAULT_BUFFER_SIZE } from "./_common.ts";
import type { Reader, Writer } from "../types.js";

/**
 * @deprecated (will be removed after 1.0.0) Use {@linkcode ReadableStream.pipeTo} instead.
 *
 * Copies from `src` to `dst` until either EOF (`null`) is read from `src` or
 * an error occurs. It resolves to the number of bytes copied or rejects with
 * the first error encountered while copying.
 *
 * ```ts
 * import { copy } from "https://deno.land/std@$STD_VERSION/streams/copy.ts";
 *
 * const source = await Deno.open("my_file.txt");
 * const bytesCopied1 = await copy(source, Deno.stdout);
 * const destination = await Deno.create("my_file_2.txt");
 * const bytesCopied2 = await copy(source, destination);
 * ```
 *
 * @param src The source to copy from
 * @param dst The destination to copy to
 * @param options Can be used to tune size of the buffer. Default size is 32kB
 */
export async function copy(
  src: Reader,
  dst: Writer,
  options?: {
    bufSize?: number;
  },
): Promise<number> {
  let n = 0;
  const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
  const b = new Uint8Array(bufSize);
  let gotEOF = false;
  while (gotEOF === false) {
    const result = await src.read(b);
    if (result === null) {
      gotEOF = true;
    } else {
      let nwritten = 0;
      while (nwritten < result) {
        nwritten += await dst.write(b.subarray(nwritten, result));
      }
      n += nwritten;
    }
  }
  return n;
}
