// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { Buffer } from "../io/buffer.ts";
import { writeAll } from "./write_all.ts";
import { Reader } from "../types.d.ts";

/**
 * @deprecated (will be removed after 1.0.0) Use {@linkcode ReadableStream.from} instead.
 *
 * Create a `Reader` from an iterable of `Uint8Array`s.
 *
 * ```ts
 *      import { readerFromIterable } from "https://deno.land/std@$STD_VERSION/streams/reader_from_iterable.ts";
 *      import { copy } from "https://deno.land/std@$STD_VERSION/streams/copy.ts";
 *
 *      const file = await Deno.open("metrics.txt", { write: true });
 *      const reader = readerFromIterable((async function* () {
 *        while (true) {
 *          await new Promise((r) => setTimeout(r, 1000));
 *          const message = `data: ${JSON.stringify(Deno.metrics())}\n\n`;
 *          yield new TextEncoder().encode(message);
 *        }
 *      })());
 *      await copy(reader, file);
 * ```
 */
export function readerFromIterable(
  iterable: Iterable<Uint8Array> | AsyncIterable<Uint8Array>,
): Reader {
  const iterator: Iterator<Uint8Array> | AsyncIterator<Uint8Array> =
    (iterable as AsyncIterable<Uint8Array>)[Symbol.asyncIterator]?.() ??
      (iterable as Iterable<Uint8Array>)[Symbol.iterator]?.();
  const buffer = new Buffer();
  return {
    async read(p: Uint8Array): Promise<number | null> {
      if (buffer.length === 0) {
        const result = await iterator.next();
        if (result.done) {
          return null;
        } else {
          if (result.value.byteLength <= p.byteLength) {
            p.set(result.value);
            return result.value.byteLength;
          }
          p.set(result.value.subarray(0, p.byteLength));
          await writeAll(buffer, result.value.subarray(p.byteLength));
          return p.byteLength;
        }
      } else {
        const n = await buffer.read(p);
        if (n === null) {
          return this.read(p);
        }
        return n;
      }
    },
  };
}
