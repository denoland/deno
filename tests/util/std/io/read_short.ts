// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { type BufReader } from "./buf_reader.ts";

/**
 * Read big endian 16bit short from BufReader
 * @param buf
 *
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export async function readShort(buf: BufReader): Promise<number | null> {
  const high = await buf.readByte();
  if (high === null) return null;
  const low = await buf.readByte();
  if (low === null) throw new Deno.errors.UnexpectedEof();
  return (high << 8) | low;
}
