// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as path from "../path/mod.ts";

/**
 * Copy bytes from one Uint8Array to another.  Bytes from `src` which don't fit
 * into `dst` will not be copied.
 *
 * @param src Source byte array
 * @param dst Destination byte array
 * @param off Offset into `dst` at which to begin writing values from `src`.
 * @return number of bytes copied
 */
export function copyBytes(src: Uint8Array, dst: Uint8Array, off = 0): number {
  off = Math.max(0, Math.min(off, dst.byteLength));
  const dstBytesAvailable = dst.byteLength - off;
  if (src.byteLength > dstBytesAvailable) {
    src = src.subarray(0, dstBytesAvailable);
  }
  dst.set(src, off);
  return src.byteLength;
}

export function charCode(s: string): number {
  return s.charCodeAt(0);
}

/** Create or open a temporal file at specified directory with prefix and
 *  postfix
 * */
export async function tempFile(
  dir: string,
  opts: {
    prefix?: string;
    postfix?: string;
  } = { prefix: "", postfix: "" }
): Promise<{ file: Deno.File; filepath: string }> {
  const r = Math.floor(Math.random() * 1000000);
  const filepath = path.resolve(
    `${dir}/${opts.prefix || ""}${r}${opts.postfix || ""}`
  );
  await Deno.mkdir(path.dirname(filepath), { recursive: true });
  const file = await Deno.open(filepath, {
    create: true,
    read: true,
    write: true,
    append: true,
  });
  return { file, filepath };
}
