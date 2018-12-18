import { Buffer, Reader } from "deno";

export function assert(cond: boolean, msg = "assert") {
  if (!cond) {
    throw Error(msg);
  }
}

// `off` is the offset into `dst` where it will at which to begin writing values
// from `src`.
// Returns the number of bytes copied.
export function copyBytes(dst: Uint8Array, src: Uint8Array, off = 0): number {
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

const encoder = new TextEncoder();
export function stringsReader(s: string): Reader {
  const ui8 = encoder.encode(s);
  return new Buffer(ui8.buffer as ArrayBuffer);
}
