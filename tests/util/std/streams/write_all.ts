// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import type { Writer, WriterSync } from "../types.js";

/**
 * @deprecated (will be removed after 1.0.0) Use {@linkcode WritableStream}, {@linkcode ReadableStream.from} and {@linkcode ReadableStream.pipeTo} instead.
 *
 * Write all the content of the array buffer (`arr`) to the writer (`w`).
 *
 * ```ts
 * import { Buffer } from "https://deno.land/std@$STD_VERSION/io/buffer.ts";
 * import { writeAll } from "https://deno.land/std@$STD_VERSION/streams/write_all.ts";

 * // Example writing to stdout
 * let contentBytes = new TextEncoder().encode("Hello World");
 * await writeAll(Deno.stdout, contentBytes);
 *
 * // Example writing to file
 * contentBytes = new TextEncoder().encode("Hello World");
 * const file = await Deno.open('test.file', {write: true});
 * await writeAll(file, contentBytes);
 * file.close();
 *
 * // Example writing to buffer
 * contentBytes = new TextEncoder().encode("Hello World");
 * const writer = new Buffer();
 * await writeAll(writer, contentBytes);
 * console.log(writer.bytes().length);  // 11
 * ```
 */
export async function writeAll(w: Writer, arr: Uint8Array) {
  let nwritten = 0;
  while (nwritten < arr.length) {
    nwritten += await w.write(arr.subarray(nwritten));
  }
}

/**
 * @deprecated (will be removed after 1.0.0) Use {@linkcode WritableStream}, {@linkcode ReadableStream.from} and {@linkcode ReadableStream.pipeTo} instead.
 *
 * Synchronously write all the content of the array buffer (`arr`) to the
 * writer (`w`).
 *
 * ```ts
 * import { Buffer } from "https://deno.land/std@$STD_VERSION/io/buffer.ts";
 * import { writeAllSync } from "https://deno.land/std@$STD_VERSION/streams/write_all.ts";
 *
 * // Example writing to stdout
 * let contentBytes = new TextEncoder().encode("Hello World");
 * writeAllSync(Deno.stdout, contentBytes);
 *
 * // Example writing to file
 * contentBytes = new TextEncoder().encode("Hello World");
 * const file = Deno.openSync('test.file', {write: true});
 * writeAllSync(file, contentBytes);
 * file.close();
 *
 * // Example writing to buffer
 * contentBytes = new TextEncoder().encode("Hello World");
 * const writer = new Buffer();
 * writeAllSync(writer, contentBytes);
 * console.log(writer.bytes().length);  // 11
 * ```
 */
export function writeAllSync(w: WriterSync, arr: Uint8Array) {
  let nwritten = 0;
  while (nwritten < arr.length) {
    nwritten += w.writeSync(arr.subarray(nwritten));
  }
}
