// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Convert the generator function into a TransformStream.
 *
 * ```ts
 * import { toTransformStream } from "https://deno.land/std@$STD_VERSION/streams/to_transform_stream.ts";
 *
 * const readable = ReadableStream.from([0, 1, 2])
 *   .pipeThrough(toTransformStream(async function* (src) {
 *     for await (const chunk of src) {
 *       yield chunk * 100;
 *     }
 *   }));
 *
 * for await (const chunk of readable) {
 *   console.log(chunk);
 * }
 * // output: 0, 100, 200
 * ```
 *
 * @param transformer A function to transform.
 * @param writableStrategy An object that optionally defines a queuing strategy for the stream.
 * @param readableStrategy An object that optionally defines a queuing strategy for the stream.
 */
export function toTransformStream<I, O>(
  transformer: (src: ReadableStream<I>) => Iterable<O> | AsyncIterable<O>,
  writableStrategy?: QueuingStrategy<I>,
  readableStrategy?: QueuingStrategy<O>,
): TransformStream<I, O> {
  const {
    writable,
    readable,
  } = new TransformStream<I, I>(undefined, writableStrategy);

  const iterable = transformer(readable);
  const iterator: Iterator<O> | AsyncIterator<O> =
    (iterable as AsyncIterable<O>)[Symbol.asyncIterator]?.() ??
      (iterable as Iterable<O>)[Symbol.iterator]?.();
  return {
    writable,
    readable: new ReadableStream<O>({
      async pull(controller) {
        let result: IteratorResult<O>;
        try {
          result = await iterator.next();
        } catch (error) {
          // Propagate error to stream from iterator
          // If the stream status is "errored", it will be thrown, but ignore.
          await readable.cancel(error).catch(() => {});
          controller.error(error);
          return;
        }
        if (result.done) {
          controller.close();
          return;
        }
        controller.enqueue(result.value);
      },
      async cancel(reason) {
        // Propagate cancellation to readable and iterator
        if (typeof iterator.throw === "function") {
          try {
            await iterator.throw(reason);
          } catch {
            /* `iterator.throw()` always throws on site. We catch it. */
          }
        }
        await readable.cancel(reason);
      },
    }, readableStrategy),
  };
}
