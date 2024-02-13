// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import type { JsonValue, ParseStreamOptions } from "./common.ts";
import { parse } from "./_common.ts";

const branks = /^[ \t\r\n]*$/;
function isBrankString(str: string) {
  return branks.test(str);
}

/**
 * Parse each chunk as JSON.
 *
 * This can be used to parse [JSON lines](https://jsonlines.org/), [NDJSON](http://ndjson.org/) and [JSON Text Sequences](https://datatracker.ietf.org/doc/html/rfc7464).
 * Chunks consisting of spaces, tab characters, or newline characters will be ignored.
 *
 * @example
 * parse JSON lines or NDJSON
 * ```ts
 * import { TextLineStream } from "https://deno.land/std@$STD_VERSION/streams/text_line_stream.ts";
 * import { JsonParseStream } from "https://deno.land/std@$STD_VERSION/json/json_parse_stream.ts";
 *
 * const url = "https://deno.land/std@$STD_VERSION/json/testdata/test.jsonl";
 * const { body } = await fetch(url);
 *
 * const readable = body!
 *   .pipeThrough(new TextDecoderStream())  // convert Uint8Array to string
 *   .pipeThrough(new TextLineStream()) // transform into a stream where each chunk is divided by a newline
 *   .pipeThrough(new JsonParseStream()); // parse each chunk as JSON
 *
 * for await (const data of readable) {
 *   console.log(data);
 * }
 * ```
 *
 * @example
 * parse JSON Text Sequences
 * ```ts
 * import { TextDelimiterStream } from "https://deno.land/std@$STD_VERSION/streams/text_delimiter_stream.ts";
 * import { JsonParseStream } from "https://deno.land/std@$STD_VERSION/json/json_parse_stream.ts";
 *
 * const url =
 *   "https://deno.land/std@$STD_VERSION/json/testdata/test.json-seq";
 * const { body } = await fetch(url);
 *
 * const delimiter = "\x1E";
 * const readable = body!
 *   .pipeThrough(new TextDecoderStream())
 *   .pipeThrough(new TextDelimiterStream(delimiter)) // transform into a stream where each chunk is divided by a delimiter
 *   .pipeThrough(new JsonParseStream());
 *
 * for await (const data of readable) {
 *   console.log(data);
 * }
 * ```
 */
export class JsonParseStream extends TransformStream<string, JsonValue> {
  /**
   * @param options
   * @param options.writableStrategy Controls the buffer of the TransformStream used internally. Check https://developer.mozilla.org/en-US/docs/Web/API/TransformStream/TransformStream.
   * @param options.readableStrategy Controls the buffer of the TransformStream used internally. Check https://developer.mozilla.org/en-US/docs/Web/API/TransformStream/TransformStream.
   */
  constructor({ writableStrategy, readableStrategy }: ParseStreamOptions = {}) {
    super(
      {
        transform(chunk, controller) {
          if (!isBrankString(chunk)) {
            controller.enqueue(parse(chunk));
          }
        },
      },
      writableStrategy,
      readableStrategy,
    );
  }
}
