// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { writeAll } from "./write_all.ts";
import type { Closer, Writer } from "../types.d.ts";

function isCloser(value: unknown): value is Closer {
  return typeof value === "object" && value !== null && value !== undefined &&
    "close" in value &&
    // deno-lint-ignore no-explicit-any
    typeof (value as Record<string, any>)["close"] === "function";
}

/** @deprecated (will be removed after 1.0.0) Use {@linkcode WritableStream} directly. */
export interface WritableStreamFromWriterOptions {
  /**
   * If the `writer` is also a `Closer`, automatically close the `writer`
   * when the stream is closed, aborted, or a write error occurs.
   *
   * @default {true}
   */
  autoClose?: boolean;
}

/**
 * @deprecated (will be removed after 1.0.0) Use {@linkcode WritableStream} directly.
 *
 * Create a `WritableStream` from a `Writer`.
 */
export function writableStreamFromWriter(
  writer: Writer,
  options: WritableStreamFromWriterOptions = {},
): WritableStream<Uint8Array> {
  const { autoClose = true } = options;

  return new WritableStream({
    async write(chunk, controller) {
      try {
        await writeAll(writer, chunk);
      } catch (e) {
        controller.error(e);
        if (isCloser(writer) && autoClose) {
          writer.close();
        }
      }
    },
    close() {
      if (isCloser(writer) && autoClose) {
        writer.close();
      }
    },
    abort() {
      if (isCloser(writer) && autoClose) {
        writer.close();
      }
    },
  });
}
