// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { Buffer } from "./buffer.ts";

/**
 * Reader utility for strings.
 *
 * @example
 * ```ts
 * import { StringReader } from "https://deno.land/std@$STD_VERSION/io/string_reader.ts";
 *
 * const data = new Uint8Array(6);
 * const r = new StringReader("abcdef");
 * const res0 = await r.read(data);
 * const res1 = await r.read(new Uint8Array(6));
 *
 * // Number of bytes read
 * console.log(res0); // 6
 * console.log(res1); // null, no byte left to read. EOL
 *
 * // text
 *
 * console.log(new TextDecoder().decode(data)); // abcdef
 * ```
 *
 * **Output:**
 *
 * ```text
 * 6
 * null
 * abcdef
 * ```
 *
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export class StringReader extends Buffer {
  constructor(s: string) {
    super(new TextEncoder().encode(s).buffer);
  }
}
