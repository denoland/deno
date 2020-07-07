// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as hex from "../encoding/hex.ts";

const TYPE_ERROR_MSG = "md5: `data` is invalid type";
const BLOCK_SIZE = 64;

export type Message = string | ArrayBuffer;

/** Md5 hash */
export class Md5 {
  #a: number;
  #b: number;
  #c: number;
  #d: number;
  #block: Uint8Array;
  #pos: number;
  #n0: number;
  #n1: number;

  constructor() {
    this.#a = 0x67452301;
    this.#b = 0xefcdab89;
    this.#c = 0x98badcfe;
    this.#d = 0x10325476;
    this.#block = new Uint8Array(BLOCK_SIZE);
    this.#pos = 0;
    this.#n0 = 0;
    this.#n1 = 0;
  }

  private addLength(len: number): void {
    let n0 = this.#n0;
    n0 += len;
    if (n0 > 0xffffffff) this.#n1 += 1;
    this.#n0 = n0 >>> 0;
  }

  private hash(block: Uint8Array): void {
    let a = this.#a;
    let b = this.#b;
    let c = this.#c;
    let d = this.#d;

    const blk = (i: number): number =>
      block[i] |
      (block[i + 1] << 8) |
      (block[i + 2] << 16) |
      (block[i + 3] << 24);

    const rol32 = (x: number, n: number): number => (x << n) | (x >>> (32 - n));

    const x0 = blk(0);
    const x1 = blk(4);
    const x2 = blk(8);
    const x3 = blk(12);
    const x4 = blk(16);
    const x5 = blk(20);
    const x6 = blk(24);
    const x7 = blk(28);
    const x8 = blk(32);
    const x9 = blk(36);
    const xa = blk(40);
    const xb = blk(44);
    const xc = blk(48);
    const xd = blk(52);
    const xe = blk(56);
    const xf = blk(60);

    // round 1
    a = b + rol32((((c ^ d) & b) ^ d) + a + x0 + 0xd76aa478, 7);
    d = a + rol32((((b ^ c) & a) ^ c) + d + x1 + 0xe8c7b756, 12);
    c = d + rol32((((a ^ b) & d) ^ b) + c + x2 + 0x242070db, 17);
    b = c + rol32((((d ^ a) & c) ^ a) + b + x3 + 0xc1bdceee, 22);
    a = b + rol32((((c ^ d) & b) ^ d) + a + x4 + 0xf57c0faf, 7);
    d = a + rol32((((b ^ c) & a) ^ c) + d + x5 + 0x4787c62a, 12);
    c = d + rol32((((a ^ b) & d) ^ b) + c + x6 + 0xa8304613, 17);
    b = c + rol32((((d ^ a) & c) ^ a) + b + x7 + 0xfd469501, 22);
    a = b + rol32((((c ^ d) & b) ^ d) + a + x8 + 0x698098d8, 7);
    d = a + rol32((((b ^ c) & a) ^ c) + d + x9 + 0x8b44f7af, 12);
    c = d + rol32((((a ^ b) & d) ^ b) + c + xa + 0xffff5bb1, 17);
    b = c + rol32((((d ^ a) & c) ^ a) + b + xb + 0x895cd7be, 22);
    a = b + rol32((((c ^ d) & b) ^ d) + a + xc + 0x6b901122, 7);
    d = a + rol32((((b ^ c) & a) ^ c) + d + xd + 0xfd987193, 12);
    c = d + rol32((((a ^ b) & d) ^ b) + c + xe + 0xa679438e, 17);
    b = c + rol32((((d ^ a) & c) ^ a) + b + xf + 0x49b40821, 22);

    // round 2
    a = b + rol32((((b ^ c) & d) ^ c) + a + x1 + 0xf61e2562, 5);
    d = a + rol32((((a ^ b) & c) ^ b) + d + x6 + 0xc040b340, 9);
    c = d + rol32((((d ^ a) & b) ^ a) + c + xb + 0x265e5a51, 14);
    b = c + rol32((((c ^ d) & a) ^ d) + b + x0 + 0xe9b6c7aa, 20);
    a = b + rol32((((b ^ c) & d) ^ c) + a + x5 + 0xd62f105d, 5);
    d = a + rol32((((a ^ b) & c) ^ b) + d + xa + 0x02441453, 9);
    c = d + rol32((((d ^ a) & b) ^ a) + c + xf + 0xd8a1e681, 14);
    b = c + rol32((((c ^ d) & a) ^ d) + b + x4 + 0xe7d3fbc8, 20);
    a = b + rol32((((b ^ c) & d) ^ c) + a + x9 + 0x21e1cde6, 5);
    d = a + rol32((((a ^ b) & c) ^ b) + d + xe + 0xc33707d6, 9);
    c = d + rol32((((d ^ a) & b) ^ a) + c + x3 + 0xf4d50d87, 14);
    b = c + rol32((((c ^ d) & a) ^ d) + b + x8 + 0x455a14ed, 20);
    a = b + rol32((((b ^ c) & d) ^ c) + a + xd + 0xa9e3e905, 5);
    d = a + rol32((((a ^ b) & c) ^ b) + d + x2 + 0xfcefa3f8, 9);
    c = d + rol32((((d ^ a) & b) ^ a) + c + x7 + 0x676f02d9, 14);
    b = c + rol32((((c ^ d) & a) ^ d) + b + xc + 0x8d2a4c8a, 20);

    // round 3
    a = b + rol32((b ^ c ^ d) + a + x5 + 0xfffa3942, 4);
    d = a + rol32((a ^ b ^ c) + d + x8 + 0x8771f681, 11);
    c = d + rol32((d ^ a ^ b) + c + xb + 0x6d9d6122, 16);
    b = c + rol32((c ^ d ^ a) + b + xe + 0xfde5380c, 23);
    a = b + rol32((b ^ c ^ d) + a + x1 + 0xa4beea44, 4);
    d = a + rol32((a ^ b ^ c) + d + x4 + 0x4bdecfa9, 11);
    c = d + rol32((d ^ a ^ b) + c + x7 + 0xf6bb4b60, 16);
    b = c + rol32((c ^ d ^ a) + b + xa + 0xbebfbc70, 23);
    a = b + rol32((b ^ c ^ d) + a + xd + 0x289b7ec6, 4);
    d = a + rol32((a ^ b ^ c) + d + x0 + 0xeaa127fa, 11);
    c = d + rol32((d ^ a ^ b) + c + x3 + 0xd4ef3085, 16);
    b = c + rol32((c ^ d ^ a) + b + x6 + 0x04881d05, 23);
    a = b + rol32((b ^ c ^ d) + a + x9 + 0xd9d4d039, 4);
    d = a + rol32((a ^ b ^ c) + d + xc + 0xe6db99e5, 11);
    c = d + rol32((d ^ a ^ b) + c + xf + 0x1fa27cf8, 16);
    b = c + rol32((c ^ d ^ a) + b + x2 + 0xc4ac5665, 23);

    // round 4
    a = b + rol32((c ^ (b | ~d)) + a + x0 + 0xf4292244, 6);
    d = a + rol32((b ^ (a | ~c)) + d + x7 + 0x432aff97, 10);
    c = d + rol32((a ^ (d | ~b)) + c + xe + 0xab9423a7, 15);
    b = c + rol32((d ^ (c | ~a)) + b + x5 + 0xfc93a039, 21);
    a = b + rol32((c ^ (b | ~d)) + a + xc + 0x655b59c3, 6);
    d = a + rol32((b ^ (a | ~c)) + d + x3 + 0x8f0ccc92, 10);
    c = d + rol32((a ^ (d | ~b)) + c + xa + 0xffeff47d, 15);
    b = c + rol32((d ^ (c | ~a)) + b + x1 + 0x85845dd1, 21);
    a = b + rol32((c ^ (b | ~d)) + a + x8 + 0x6fa87e4f, 6);
    d = a + rol32((b ^ (a | ~c)) + d + xf + 0xfe2ce6e0, 10);
    c = d + rol32((a ^ (d | ~b)) + c + x6 + 0xa3014314, 15);
    b = c + rol32((d ^ (c | ~a)) + b + xd + 0x4e0811a1, 21);
    a = b + rol32((c ^ (b | ~d)) + a + x4 + 0xf7537e82, 6);
    d = a + rol32((b ^ (a | ~c)) + d + xb + 0xbd3af235, 10);
    c = d + rol32((a ^ (d | ~b)) + c + x2 + 0x2ad7d2bb, 15);
    b = c + rol32((d ^ (c | ~a)) + b + x9 + 0xeb86d391, 21);

    this.#a = (this.#a + a) >>> 0;
    this.#b = (this.#b + b) >>> 0;
    this.#c = (this.#c + c) >>> 0;
    this.#d = (this.#d + d) >>> 0;
  }

  /**
   * Update internal state
   * @param data data to update, data cannot exceed 2^32 bytes
   */
  update(data: Message): this {
    let msg: Uint8Array;

    if (typeof data === "string") {
      msg = new TextEncoder().encode(data as string);
    } else if (typeof data === "object") {
      if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
        msg = new Uint8Array(data);
      } else {
        throw new Error(TYPE_ERROR_MSG);
      }
    } else {
      throw new Error(TYPE_ERROR_MSG);
    }

    let pos = this.#pos;
    const free = BLOCK_SIZE - pos;

    if (msg.length < free) {
      this.#block.set(msg, pos);
      pos += msg.length;
    } else {
      // hash first block
      this.#block.set(msg.slice(0, free), pos);
      this.hash(this.#block);

      // hash as many blocks as possible
      let i = free;
      while (i + BLOCK_SIZE <= msg.length) {
        this.hash(msg.slice(i, i + BLOCK_SIZE));
        i += BLOCK_SIZE;
      }

      // store leftover
      this.#block.fill(0).set(msg.slice(i), 0);
      pos = msg.length - i;
    }

    this.#pos = pos;
    this.addLength(msg.length);

    return this;
  }

  /** Returns final hash */
  digest(): ArrayBuffer {
    let padLen = BLOCK_SIZE - this.#pos;
    if (padLen < 9) padLen += BLOCK_SIZE;

    const pad = new Uint8Array(padLen);

    pad[0] = 0x80;

    const n0 = this.#n0 << 3;
    const n1 = (this.#n1 << 3) | (this.#n0 >>> 29);
    pad[pad.length - 8] = n0 & 0xff;
    pad[pad.length - 7] = (n0 >>> 8) & 0xff;
    pad[pad.length - 6] = (n0 >>> 16) & 0xff;
    pad[pad.length - 5] = (n0 >>> 24) & 0xff;
    pad[pad.length - 4] = n1 & 0xff;
    pad[pad.length - 3] = (n1 >>> 8) & 0xff;
    pad[pad.length - 2] = (n1 >>> 16) & 0xff;
    pad[pad.length - 1] = (n1 >>> 24) & 0xff;

    this.update(pad.buffer);

    const hash = new ArrayBuffer(16);
    const hashView = new DataView(hash);
    hashView.setUint32(0, this.#a, true);
    hashView.setUint32(4, this.#b, true);
    hashView.setUint32(8, this.#c, true);
    hashView.setUint32(12, this.#d, true);

    return hash;
  }

  /**
   * Returns hash as a string of given format
   * @param format format of output string (hex or base64). Default is hex
   */
  toString(format: "hex" | "base64" = "hex"): string {
    const hash = this.digest();

    switch (format) {
      case "hex":
        return hex.encodeToString(new Uint8Array(hash));
      case "base64": {
        const data = new Uint8Array(hash);
        let dataString = "";
        for (let i = 0; i < data.length; ++i) {
          dataString += String.fromCharCode(data[i]);
        }
        return btoa(dataString);
      }
      default:
        throw new Error("md5: invalid format");
    }
  }
}
