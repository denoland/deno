// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** A TransformStream that will only read & enqueue `size` amount of chunks.
 *
 * if options.error is set, then instead of terminating the stream,
 * an error will be thrown.
 *
 * ```ts
 * import { LimitedTransformStream } from "https://deno.land/std@$STD_VERSION/streams/limited_transform_stream.ts";
 * const res = await fetch("https://example.com");
 * const parts = res.body!.pipeThrough(new LimitedTransformStream(50));
 * ```
 */
export class LimitedTransformStream<T> extends TransformStream<T, T> {
  #read = 0;
  constructor(size: number, options: { error?: boolean } = {}) {
    super({
      transform: (chunk, controller) => {
        if ((this.#read + 1) > size) {
          if (options.error) {
            throw new RangeError(`Exceeded chunk limit of '${size}'`);
          } else {
            controller.terminate();
          }
        } else {
          this.#read++;
          controller.enqueue(chunk);
        }
      },
    });
  }
}
