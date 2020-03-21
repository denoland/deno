// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { BufReader } from "./bufio.ts";
type Reader = Deno.Reader;
type Writer = Deno.Writer;
import { assert } from "../testing/asserts.ts";

/** copy N size at the most.
 *  If read size is lesser than N, then returns nread
 * */
export async function copyN(
  dest: Writer,
  r: Reader,
  size: number
): Promise<number> {
  let bytesRead = 0;
  let buf = new Uint8Array(1024);
  while (bytesRead < size) {
    if (size - bytesRead < 1024) {
      buf = new Uint8Array(size - bytesRead);
    }
    const result = await r.read(buf);
    const nread = result === Deno.EOF ? 0 : result;
    bytesRead += nread;
    if (nread > 0) {
      const n = await dest.write(buf.slice(0, nread));
      assert(n === nread, "could not write");
    }
    if (result === Deno.EOF) {
      break;
    }
  }
  return bytesRead;
}

/** Read big endian 16bit short from BufReader */
export async function readShort(buf: BufReader): Promise<number | Deno.EOF> {
  const high = await buf.readByte();
  if (high === Deno.EOF) return Deno.EOF;
  const low = await buf.readByte();
  if (low === Deno.EOF) throw new Deno.errors.UnexpectedEof();
  return (high << 8) | low;
}

/** Read big endian 32bit integer from BufReader */
export async function readInt(buf: BufReader): Promise<number | Deno.EOF> {
  const high = await readShort(buf);
  if (high === Deno.EOF) return Deno.EOF;
  const low = await readShort(buf);
  if (low === Deno.EOF) throw new Deno.errors.UnexpectedEof();
  return (high << 16) | low;
}

const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);

/** Read big endian 64bit long from BufReader */
export async function readLong(buf: BufReader): Promise<number | Deno.EOF> {
  const high = await readInt(buf);
  if (high === Deno.EOF) return Deno.EOF;
  const low = await readInt(buf);
  if (low === Deno.EOF) throw new Deno.errors.UnexpectedEof();
  const big = (BigInt(high) << 32n) | BigInt(low);
  // We probably should provide a similar API that returns BigInt values.
  if (big > MAX_SAFE_INTEGER) {
    throw new RangeError(
      "Long value too big to be represented as a Javascript number."
    );
  }
  return Number(big);
}

/** Slice number into 64bit big endian byte array */
export function sliceLongToBytes(d: number, dest = new Array(8)): number[] {
  let big = BigInt(d);
  for (let i = 0; i < 8; i++) {
    dest[7 - i] = Number(big & 0xffn);
    big >>= 8n;
  }
  return dest;
}
