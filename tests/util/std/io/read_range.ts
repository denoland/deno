// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { copy as copyBytes } from "../bytes/copy.ts";
import { assert } from "../assert/assert.ts";
import type { Reader, ReaderSync } from "../types.d.ts";

const DEFAULT_BUFFER_SIZE = 32 * 1024;

/**
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export interface ByteRange {
  /** The 0 based index of the start byte for a range. */
  start: number;

  /** The 0 based index of the end byte for a range, which is inclusive. */
  end: number;
}

/**
 * Read a range of bytes from a file or other resource that is readable and
 * seekable.  The range start and end are inclusive of the bytes within that
 * range.
 *
 * ```ts
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 * import { readRange } from "https://deno.land/std@$STD_VERSION/io/read_range.ts";
 *
 * // Read the first 10 bytes of a file
 * const file = await Deno.open("example.txt", { read: true });
 * const bytes = await readRange(file, { start: 0, end: 9 });
 * assertEquals(bytes.length, 10);
 * ```
 *
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export async function readRange(
  r: Reader & Deno.Seeker,
  range: ByteRange,
): Promise<Uint8Array> {
  // byte ranges are inclusive, so we have to add one to the end
  let length = range.end - range.start + 1;
  assert(length > 0, "Invalid byte range was passed.");
  await r.seek(range.start, Deno.SeekMode.Start);
  const result = new Uint8Array(length);
  let off = 0;
  while (length) {
    const p = new Uint8Array(Math.min(length, DEFAULT_BUFFER_SIZE));
    const nread = await r.read(p);
    assert(nread !== null, "Unexpected EOF reach while reading a range.");
    assert(nread > 0, "Unexpected read of 0 bytes while reading a range.");
    copyBytes(p, result, off);
    off += nread;
    length -= nread;
    assert(length >= 0, "Unexpected length remaining after reading range.");
  }
  return result;
}

/**
 * Read a range of bytes synchronously from a file or other resource that is
 * readable and seekable.  The range start and end are inclusive of the bytes
 * within that range.
 *
 * ```ts
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 * import { readRangeSync } from "https://deno.land/std@$STD_VERSION/io/read_range.ts";
 *
 * // Read the first 10 bytes of a file
 * const file = Deno.openSync("example.txt", { read: true });
 * const bytes = readRangeSync(file, { start: 0, end: 9 });
 * assertEquals(bytes.length, 10);
 * ```
 *
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export function readRangeSync(
  r: ReaderSync & Deno.SeekerSync,
  range: ByteRange,
): Uint8Array {
  // byte ranges are inclusive, so we have to add one to the end
  let length = range.end - range.start + 1;
  assert(length > 0, "Invalid byte range was passed.");
  r.seekSync(range.start, Deno.SeekMode.Start);
  const result = new Uint8Array(length);
  let off = 0;
  while (length) {
    const p = new Uint8Array(Math.min(length, DEFAULT_BUFFER_SIZE));
    const nread = r.readSync(p);
    assert(nread !== null, "Unexpected EOF reach while reading a range.");
    assert(nread > 0, "Unexpected read of 0 bytes while reading a range.");
    copyBytes(p, result, off);
    off += nread;
    length -= nread;
    assert(length >= 0, "Unexpected length remaining after reading range.");
  }
  return result;
}
