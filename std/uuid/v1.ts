// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { bytesToUuid } from "./_common.ts";

const UUID_RE = new RegExp(
  "^[0-9a-f]{8}-[0-9a-f]{4}-1[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
  "i",
);

/**
 * Validates the UUID v1
 * @param id UUID value
 */
export function validate(id: string): boolean {
  return UUID_RE.test(id);
}

let _nodeId: number[];
let _clockseq: number;

let _lastMSecs = 0;
let _lastNSecs = 0;

type V1Options = {
  node?: number[];
  clockseq?: number;
  msecs?: number;
  nsecs?: number;
  random?: number[];
  rng?: () => number[];
};

/**
 * Generates a RFC4122 v1 UUID (time-based)
 * @param options Can use RFC time sequence values as overwrites
 * @param buf Can allow the UUID to be written in byte-form starting at the offset
 * @param offset Index to start writing on the UUID bytes in buffer
 */
export function generate(
  options?: V1Options | null,
  buf?: number[],
  offset?: number,
): string | number[] {
  let i = (buf && offset) || 0;
  const b = buf || [];

  options = options || {};
  let node = options.node || _nodeId;
  let clockseq = options.clockseq !== undefined ? options.clockseq : _clockseq;

  if (node == null || clockseq == null) {
    // deno-lint-ignore no-explicit-any
    const seedBytes: any = options.random ||
      options.rng ||
      crypto.getRandomValues(new Uint8Array(16));
    if (node == null) {
      node = _nodeId = [
        seedBytes[0] | 0x01,
        seedBytes[1],
        seedBytes[2],
        seedBytes[3],
        seedBytes[4],
        seedBytes[5],
      ];
    }
    if (clockseq == null) {
      clockseq = _clockseq = ((seedBytes[6] << 8) | seedBytes[7]) & 0x3fff;
    }
  }
  let msecs = options.msecs !== undefined
    ? options.msecs
    : new Date().getTime();

  let nsecs = options.nsecs !== undefined ? options.nsecs : _lastNSecs + 1;

  const dt = msecs - _lastMSecs + (nsecs - _lastNSecs) / 10000;

  if (dt < 0 && options.clockseq === undefined) {
    clockseq = (clockseq + 1) & 0x3fff;
  }

  if ((dt < 0 || msecs > _lastMSecs) && options.nsecs === undefined) {
    nsecs = 0;
  }

  if (nsecs >= 10000) {
    throw new Error("Can't create more than 10M uuids/sec");
  }

  _lastMSecs = msecs;
  _lastNSecs = nsecs;
  _clockseq = clockseq;

  msecs += 12219292800000;

  const tl = ((msecs & 0xfffffff) * 10000 + nsecs) % 0x100000000;
  b[i++] = (tl >>> 24) & 0xff;
  b[i++] = (tl >>> 16) & 0xff;
  b[i++] = (tl >>> 8) & 0xff;
  b[i++] = tl & 0xff;

  const tmh = ((msecs / 0x100000000) * 10000) & 0xfffffff;
  b[i++] = (tmh >>> 8) & 0xff;
  b[i++] = tmh & 0xff;

  b[i++] = ((tmh >>> 24) & 0xf) | 0x10;
  b[i++] = (tmh >>> 16) & 0xff;

  b[i++] = (clockseq >>> 8) | 0x80;

  b[i++] = clockseq & 0xff;

  for (let n = 0; n < 6; ++n) {
    b[i + n] = node[n];
  }

  return buf ? buf : bytesToUuid(b);
}
