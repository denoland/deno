// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { BufReader } from "./bufio.ts";
import { Reader, Writer } from "deno";
import { assert } from "../testing/asserts.ts";

/** copy N size at the most. If read size is lesser than N, then returns nread */
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
    const { nread, eof } = await r.read(buf);
    bytesRead += nread;
    if (nread > 0) {
      const n = await dest.write(buf.slice(0, nread));
      assert(n === nread, "could not write");
    }
    if (eof) {
      break;
    }
  }
  return bytesRead;
}

/** Read big endian 16bit short from BufReader */
export async function readShort(buf: BufReader): Promise<number> {
  const [high, low] = [await buf.readByte(), await buf.readByte()];
  return (high << 8) | low;
}

/** Read big endian 32bit integer from BufReader */
export async function readInt(buf: BufReader): Promise<number> {
  const [high, low] = [await readShort(buf), await readShort(buf)];
  return (high << 16) | low;
}

const BIT32 = 0xffffffff;

/** Read big endian 64bit long from BufReader */
export async function readLong(buf: BufReader): Promise<number> {
  const [high, low] = [await readInt(buf), await readInt(buf)];
  // ECMAScript doesn't support 64bit bit ops.
  return high ? high * (BIT32 + 1) + low : low;
}

/** Slice number into 64bit big endian byte array */
export function sliceLongToBytes(d: number, dest = new Array(8)): number[] {
  let mask = 0xff;
  let low = (d << 32) >>> 32;
  let high = (d - low) / (BIT32 + 1);
  let shift = 24;
  for (let i = 0; i < 4; i++) {
    dest[i] = (high >>> shift) & mask;
    dest[i + 4] = (low >>> shift) & mask;
    shift -= 8;
  }
  return dest;
}
