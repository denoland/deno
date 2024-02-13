// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { type BufReader } from "./buf_reader.ts";
import { readInt } from "./read_int.ts";

const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);

/**
 * Read big endian 64bit long from BufReader
 * @param buf
 *
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export async function readLong(buf: BufReader): Promise<number | null> {
  const high = await readInt(buf);
  if (high === null) return null;
  const low = await readInt(buf);
  if (low === null) throw new Deno.errors.UnexpectedEof();
  const big = (BigInt(high) << 32n) | BigInt(low);
  // We probably should provide a similar API that returns BigInt values.
  if (big > MAX_SAFE_INTEGER) {
    throw new RangeError(
      "Long value too big to be represented as a JavaScript number.",
    );
  }
  return Number(big);
}
