// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { Buffer, mkdir, open } = Deno;
type File = Deno.File;
type Reader = Deno.Reader;
import { encode } from "../strings/strings.ts";
import * as path from "../fs/path.ts";
// `off` is the offset into `dst` where it will at which to begin writing values
// from `src`.
// Returns the number of bytes copied.
export function copyBytes(dst: Uint8Array, src: Uint8Array, off = 0): number {
  off = Math.max(0, Math.min(off, dst.byteLength));
  const r = dst.byteLength - off;
  if (src.byteLength > r) {
    src = src.subarray(0, r);
  }
  dst.set(src, off);
  return src.byteLength;
}

export function charCode(s: string): number {
  return s.charCodeAt(0);
}

export function stringsReader(s: string): Reader {
  return new Buffer(encode(s).buffer);
}

/** Create or open a temporal file at specified directory with prefix and postfix  */
export async function tempFile(
  dir: string,
  opts: {
    prefix?: string;
    postfix?: string;
  } = { prefix: "", postfix: "" }
): Promise<{ file: File; filepath: string }> {
  const r = Math.floor(Math.random() * 1000000);
  const filepath = path.resolve(
    `${dir}/${opts.prefix || ""}${r}${opts.postfix || ""}`
  );
  await mkdir(path.dirname(filepath), true);
  const file = await open(filepath, "a");
  return { file, filepath };
}
