// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/*
MIT License

Copyright (c) 2018 cryptocoinjs

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
 */

import { Buffer } from "ext:deno_node/buffer.ts";
import { pbkdf2Sync as pbkdf2 } from "ext:deno_node/internal/crypto/pbkdf2.ts";
import { HASH_DATA } from "ext:deno_node/internal/crypto/types.ts";

type Opts = Partial<{
  N: number;
  cost: number;
  p: number;
  parallelization: number;
  r: number;
  blockSize: number;
  maxmem: number;
}>;

const fixOpts = (opts?: Opts) => {
  const out = { N: 16384, p: 1, r: 8, maxmem: 32 << 20 };
  if (!opts) return out;

  if (opts.N) out.N = opts.N;
  else if (opts.cost) out.N = opts.cost;

  if (opts.p) out.p = opts.p;
  else if (opts.parallelization) out.p = opts.parallelization;

  if (opts.r) out.r = opts.r;
  else if (opts.blockSize) out.r = opts.blockSize;

  if (opts.maxmem) out.maxmem = opts.maxmem;

  return out;
};

function blockxor(S: Buffer, Si: number, D: Buffer, Di: number, len: number) {
  let i = -1;
  while (++i < len) D[Di + i] ^= S[Si + i];
}
function arraycopy(
  src: Buffer,
  srcPos: number,
  dest: Buffer,
  destPos: number,
  length: number,
) {
  src.copy(dest, destPos, srcPos, srcPos + length);
}

const R = (a: number, b: number) => (a << b) | (a >>> (32 - b));

class ScryptRom {
  B: Buffer;
  r: number;
  N: number;
  p: number;
  XY: Buffer;
  V: Buffer;
  B32: Int32Array;
  x: Int32Array;
  _X: Buffer;
  constructor(b: Buffer, r: number, N: number, p: number) {
    this.B = b;
    this.r = r;
    this.N = N;
    this.p = p;
    this.XY = Buffer.allocUnsafe(256 * r);
    this.V = Buffer.allocUnsafe(128 * r * N);
    this.B32 = new Int32Array(16); // salsa20_8
    this.x = new Int32Array(16); // salsa20_8
    this._X = Buffer.allocUnsafe(64); // blockmix_salsa8
  }

  run() {
    const p = this.p | 0;
    const r = this.r | 0;
    for (let i = 0; i < p; i++) this.scryptROMix(i, r);

    return this.B;
  }

  scryptROMix(i: number, r: number) {
    const blockStart = i * 128 * r;
    const offset = (2 * r - 1) * 64;
    const blockLen = 128 * r;
    const B = this.B;
    const N = this.N | 0;
    const V = this.V;
    const XY = this.XY;
    B.copy(XY, 0, blockStart, blockStart + blockLen);
    for (let i1 = 0; i1 < N; i1++) {
      XY.copy(V, i1 * blockLen, 0, blockLen);
      this.blockmix_salsa8(blockLen);
    }

    let j: number;
    for (let i2 = 0; i2 < N; i2++) {
      j = XY.readUInt32LE(offset) & (N - 1);
      blockxor(V, j * blockLen, XY, 0, blockLen);
      this.blockmix_salsa8(blockLen);
    }
    XY.copy(B, blockStart, 0, blockLen);
  }

  blockmix_salsa8(blockLen: number) {
    const BY = this.XY;
    const r = this.r;
    const _X = this._X;
    arraycopy(BY, (2 * r - 1) * 64, _X, 0, 64);
    let i;
    for (i = 0; i < 2 * r; i++) {
      blockxor(BY, i * 64, _X, 0, 64);
      this.salsa20_8();
      arraycopy(_X, 0, BY, blockLen + i * 64, 64);
    }
    for (i = 0; i < r; i++) {
      arraycopy(BY, blockLen + i * 2 * 64, BY, i * 64, 64);
      arraycopy(BY, blockLen + (i * 2 + 1) * 64, BY, (i + r) * 64, 64);
    }
  }

  salsa20_8() {
    const B32 = this.B32;
    const B = this._X;
    const x = this.x;

    let i;
    for (i = 0; i < 16; i++) {
      B32[i] = (B[i * 4 + 0] & 0xff) << 0;
      B32[i] |= (B[i * 4 + 1] & 0xff) << 8;
      B32[i] |= (B[i * 4 + 2] & 0xff) << 16;
      B32[i] |= (B[i * 4 + 3] & 0xff) << 24;
    }

    for (i = 0; i < 16; i++) x[i] = B32[i];

    for (i = 0; i < 4; i++) {
      x[4] ^= R(x[0] + x[12], 7);
      x[8] ^= R(x[4] + x[0], 9);
      x[12] ^= R(x[8] + x[4], 13);
      x[0] ^= R(x[12] + x[8], 18);
      x[9] ^= R(x[5] + x[1], 7);
      x[13] ^= R(x[9] + x[5], 9);
      x[1] ^= R(x[13] + x[9], 13);
      x[5] ^= R(x[1] + x[13], 18);
      x[14] ^= R(x[10] + x[6], 7);
      x[2] ^= R(x[14] + x[10], 9);
      x[6] ^= R(x[2] + x[14], 13);
      x[10] ^= R(x[6] + x[2], 18);
      x[3] ^= R(x[15] + x[11], 7);
      x[7] ^= R(x[3] + x[15], 9);
      x[11] ^= R(x[7] + x[3], 13);
      x[15] ^= R(x[11] + x[7], 18);
      x[1] ^= R(x[0] + x[3], 7);
      x[2] ^= R(x[1] + x[0], 9);
      x[3] ^= R(x[2] + x[1], 13);
      x[0] ^= R(x[3] + x[2], 18);
      x[6] ^= R(x[5] + x[4], 7);
      x[7] ^= R(x[6] + x[5], 9);
      x[4] ^= R(x[7] + x[6], 13);
      x[5] ^= R(x[4] + x[7], 18);
      x[11] ^= R(x[10] + x[9], 7);
      x[8] ^= R(x[11] + x[10], 9);
      x[9] ^= R(x[8] + x[11], 13);
      x[10] ^= R(x[9] + x[8], 18);
      x[12] ^= R(x[15] + x[14], 7);
      x[13] ^= R(x[12] + x[15], 9);
      x[14] ^= R(x[13] + x[12], 13);
      x[15] ^= R(x[14] + x[13], 18);
    }
    for (i = 0; i < 16; i++) B32[i] += x[i];

    let bi;

    for (i = 0; i < 16; i++) {
      bi = i * 4;
      B[bi + 0] = (B32[i] >> 0) & 0xff;
      B[bi + 1] = (B32[i] >> 8) & 0xff;
      B[bi + 2] = (B32[i] >> 16) & 0xff;
      B[bi + 3] = (B32[i] >> 24) & 0xff;
    }
  }

  clean() {
    this.XY.fill(0);
    this.V.fill(0);
    this._X.fill(0);
    this.B.fill(0);
    for (let i = 0; i < 16; i++) {
      this.B32[i] = 0;
      this.x[i] = 0;
    }
  }
}

export function scryptSync(
  password: HASH_DATA,
  salt: HASH_DATA,
  keylen: number,
  _opts?: Opts,
): Buffer {
  const { N, r, p, maxmem } = fixOpts(_opts);

  const blen = p * 128 * r;

  if (32 * r * (N + 2) * 4 + blen > maxmem) {
    throw new Error("excedes max memory");
  }

  const b = pbkdf2(password, salt, 1, blen, "sha256");

  const scryptRom = new ScryptRom(b, r, N, p);
  const out = scryptRom.run();

  const fin = pbkdf2(password, out, 1, keylen, "sha256");
  scryptRom.clean();
  return fin;
}

type Callback = (err: unknown, result?: Buffer) => void;

export function scrypt(
  password: HASH_DATA,
  salt: HASH_DATA,
  keylen: number,
  _opts: Opts | null | Callback,
  cb?: Callback,
) {
  if (!cb) {
    cb = _opts as Callback;
    _opts = null;
  }
  const { N, r, p, maxmem } = fixOpts(_opts as Opts);

  const blen = p * 128 * r;
  if (32 * r * (N + 2) * 4 + blen > maxmem) {
    throw new Error("excedes max memory");
  }

  try {
    const b = pbkdf2(password, salt, 1, blen, "sha256");

    const scryptRom = new ScryptRom(b, r, N, p);
    const out = scryptRom.run();
    const result = pbkdf2(password, out, 1, keylen, "sha256");
    scryptRom.clean();
    cb(null, result);
  } catch (err: unknown) {
    return cb(err);
  }
}

export default {
  scrypt,
  scryptSync,
};
