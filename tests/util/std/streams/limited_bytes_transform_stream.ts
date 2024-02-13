// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** A TransformStream that will only read & enqueue `size` amount of bytes.
 * This operation is chunk based and not BYOB based,
 * and as such will read more than needed.
 *
 * if options.error is set, then instead of terminating the stream,
 * an error will be thrown.
 *
 * ```ts
 * import { LimitedBytesTransformStream } from "https://deno.land/std@$STD_VERSION/streams/limited_bytes_transform_stream.ts";
 * const res = await fetch("https://example.com");
 * const parts = res.body!
 *   .pipeThrough(new LimitedBytesTransformStream(512 * 1024));
 * ```
 */
export class LimitedBytesTransformStream
  extends TransformStream<Uint8Array, Uint8Array> {
  #read = 0;
  constructor(size: number, options: { error?: boolean } = {}) {
    super({
      transform: (chunk, controller) => {
        if ((this.#read + chunk.byteLength) > size) {
          if (options.error) {
            throw new RangeError(`Exceeded byte size limit of '${size}'`);
          } else {
            controller.terminate();
          }
        } else {
          this.#read += chunk.byteLength;
          controller.enqueue(chunk);
        }
      },
    });
  }
}
