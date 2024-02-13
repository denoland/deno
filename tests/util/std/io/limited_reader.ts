// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * A `LimitedReader` reads from `reader` but limits the amount of data returned to just `limit` bytes.
 * Each call to `read` updates `limit` to reflect the new amount remaining.
 * `read` returns `null` when `limit` <= `0` or
 * when the underlying `reader` returns `null`.
 */
import type { Reader } from "../types.d.ts";

/**
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export class LimitedReader implements Reader {
  constructor(public reader: Reader, public limit: number) {}

  async read(p: Uint8Array): Promise<number | null> {
    if (this.limit <= 0) {
      return null;
    }

    if (p.length > this.limit) {
      p = p.subarray(0, this.limit);
    }
    const n = await this.reader.read(p);
    if (n === null) {
      return null;
    }

    this.limit -= n;
    return n;
  }
}
