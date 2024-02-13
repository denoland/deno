// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { DEFAULT_CHUNK_SIZE } from "./_common.ts";
import type { Closer, Reader } from "../types.d.ts";

function isCloser(value: unknown): value is Closer {
  return typeof value === "object" && value !== null && value !== undefined &&
    "close" in value &&
    // deno-lint-ignore no-explicit-any
    typeof (value as Record<string, any>)["close"] === "function";
}

/** @deprecated (will be removed after 1.0.0) Use {@linkcode ReadableStream} directly instead. */
export interface ReadableStreamFromReaderOptions {
  /** If the `reader` is also a `Closer`, automatically close the `reader`
   * when `EOF` is encountered, or a read error occurs.
   *
   * @default {true}
   */
  autoClose?: boolean;

  /** The size of chunks to allocate to read, the default is ~16KiB, which is
   * the maximum size that Deno operations can currently support. */
  chunkSize?: number;

  /** The queuing strategy to create the `ReadableStream` with. */
  strategy?: { highWaterMark?: number | undefined; size?: undefined };
}

/**
 * @deprecated (will be removed after 1.0.0) Use {@linkcode ReadableStream} directly instead.
 *
 * Create a `ReadableStream<Uint8Array>` from a `Reader`.
 *
 * When the pull algorithm is called on the stream, a chunk from the reader
 * will be read.  When `null` is returned from the reader, the stream will be
 * closed along with the reader (if it is also a `Closer`).
 *
 * An example converting a `Deno.FsFile` into a readable stream:
 *
 * ```ts
 * import { readableStreamFromReader } from "https://deno.land/std@$STD_VERSION/streams/readable_stream_from_reader.ts";
 *
 * const file = await Deno.open("./file.txt", { read: true });
 * const fileStream = readableStreamFromReader(file);
 * ```
 */
export function readableStreamFromReader(
  reader: Reader | (Reader & Closer),
  options: ReadableStreamFromReaderOptions = {},
): ReadableStream<Uint8Array> {
  const {
    autoClose = true,
    chunkSize = DEFAULT_CHUNK_SIZE,
    strategy,
  } = options;

  return new ReadableStream({
    async pull(controller) {
      const chunk = new Uint8Array(chunkSize);
      try {
        const read = await reader.read(chunk);
        if (read === null) {
          if (isCloser(reader) && autoClose) {
            reader.close();
          }
          controller.close();
          return;
        }
        controller.enqueue(chunk.subarray(0, read));
      } catch (e) {
        controller.error(e);
        if (isCloser(reader)) {
          reader.close();
        }
      }
    },
    cancel() {
      if (isCloser(reader) && autoClose) {
        reader.close();
      }
    },
  }, strategy);
}
