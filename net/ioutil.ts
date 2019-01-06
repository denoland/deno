// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { BufReader } from "./bufio.ts";

/* Read big endian 16bit short from BufReader */
export async function readShort(buf: BufReader): Promise<number> {
  const [high, low] = [await buf.readByte(), await buf.readByte()];
  return (high << 8) | low;
}

/* Read big endian 32bit integer from BufReader */
export async function readInt(buf: BufReader): Promise<number> {
  const [high, low] = [await readShort(buf), await readShort(buf)];
  return (high << 16) | low;
}

const BIT32 = 0xffffffff;
/* Read big endian 64bit long from BufReader */
export async function readLong(buf: BufReader): Promise<number> {
  const [high, low] = [await readInt(buf), await readInt(buf)];
  // ECMAScript doesn't support 64bit bit ops.
  return high ? high * (BIT32 + 1) + low : low;
}

/* Slice number into 64bit big endian byte array */
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
