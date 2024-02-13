// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** Optional object interface for `JsonStringifyStream`. */
export interface StringifyStreamOptions {
  /** Prefix to be added after stringify.
   *
   * @default {""}
   */
  readonly prefix?: string;
  /** Suffix to be added after stringify.
   *
   * @default {"\n"}
   */
  readonly suffix?: string;
  /** Controls the buffer of the TransformStream used internally. Check https://developer.mozilla.org/en-US/docs/Web/API/TransformStream/TransformStream. */
  readonly writableStrategy?: QueuingStrategy<unknown>;
  /** Controls the buffer of the TransformStream used internally. Check https://developer.mozilla.org/en-US/docs/Web/API/TransformStream/TransformStream. */
  readonly readableStrategy?: QueuingStrategy<string>;
}

/**
 * Convert each chunk to JSON string.
 *
 * This can be used to stringify [JSON lines](https://jsonlines.org/), [NDJSON](http://ndjson.org/), [JSON Text Sequences](https://datatracker.ietf.org/doc/html/rfc7464), and [Concatenated JSON](https://en.wikipedia.org/wiki/JSON_streaming#Concatenated_JSON).
 * You can optionally specify a prefix and suffix for each chunk. The default prefix is "" and the default suffix is "\n".
 *
 * @example
 * ```ts
 * import { JsonStringifyStream } from "https://deno.land/std@$STD_VERSION/json/json_stringify_stream.ts";
 *
 * const file = await Deno.open("./tmp.jsonl", { create: true, write: true });
 *
 * ReadableStream.from([{ foo: "bar" }, { baz: 100 }])
 *   .pipeThrough(new JsonStringifyStream()) // convert to JSON lines (ndjson)
 *   .pipeThrough(new TextEncoderStream()) // convert a string to a Uint8Array
 *   .pipeTo(file.writable)
 *   .then(() => console.log("write success"));
 * ```
 *
 * @example
 * To convert to [JSON Text Sequences](https://datatracker.ietf.org/doc/html/rfc7464), set the
 * prefix to the delimiter "\x1E" as options.
 * ```ts
 * import { JsonStringifyStream } from "https://deno.land/std@$STD_VERSION/json/json_stringify_stream.ts";
 *
 * const file = await Deno.open("./tmp.jsonl", { create: true, write: true });
 *
 * ReadableStream.from([{ foo: "bar" }, { baz: 100 }])
 *   .pipeThrough(new JsonStringifyStream({ prefix: "\x1E", suffix: "\n" })) // convert to JSON Text Sequences
 *   .pipeThrough(new TextEncoderStream())
 *   .pipeTo(file.writable)
 *   .then(() => console.log("write success"));
 * ```
 *
 * @example
 * If you want to stream [JSON lines](https://jsonlines.org/) from the server:
 * ```ts
 * import { JsonStringifyStream } from "https://deno.land/std@$STD_VERSION/json/json_stringify_stream.ts";
 *
 * // A server that streams one line of JSON every second
 * Deno.serve(() => {
 *   let intervalId: number | undefined;
 *   const readable = new ReadableStream({
 *     start(controller) {
 *       // enqueue data once per second
 *       intervalId = setInterval(() => {
 *         controller.enqueue({ now: new Date() });
 *       }, 1000);
 *     },
 *     cancel() {
 *       clearInterval(intervalId);
 *     },
 *   });
 *
 *   const body = readable
 *     .pipeThrough(new JsonStringifyStream()) // convert data to JSON lines
 *     .pipeThrough(new TextEncoderStream()); // convert a string to a Uint8Array
 *
 *   return new Response(body);
 * });
 * ```
 */
export class JsonStringifyStream extends TransformStream<unknown, string> {
  constructor({
    prefix = "",
    suffix = "\n",
    writableStrategy,
    readableStrategy,
  }: StringifyStreamOptions = {}) {
    super(
      {
        transform(chunk, controller) {
          controller.enqueue(`${prefix}${JSON.stringify(chunk)}${suffix}`);
        },
      },
      writableStrategy,
      readableStrategy,
    );
  }
}
