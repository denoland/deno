// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/*
 * Adapted to deno from:
 *
 * [js-sha256]{@link https://github.com/emn178/js-sha256}
 *
 * @version 0.9.0
 * @author Chen, Yi-Cyuan [emn178@gmail.com]
 * @copyright Chen, Yi-Cyuan 2014-2017
 * @license MIT
 */

export type Message = string | number[] | ArrayBuffer;

const HEX_CHARS = "0123456789abcdef".split("");
const EXTRA = [-2147483648, 8388608, 32768, 128] as const;
const SHIFT = [24, 16, 8, 0] as const;
// deno-fmt-ignore
const K = [
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
  0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
  0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
  0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
  0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
  0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
  0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
  0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
  0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
] as const;

const blocks: number[] = [];

export class Sha256 {
  #block!: number;
  #blocks!: number[];
  #bytes!: number;
  #finalized!: boolean;
  #first!: boolean;
  #h0!: number;
  #h1!: number;
  #h2!: number;
  #h3!: number;
  #h4!: number;
  #h5!: number;
  #h6!: number;
  #h7!: number;
  #hashed!: boolean;
  #hBytes!: number;
  #is224!: boolean;
  #lastByteIndex = 0;
  #start!: number;

  constructor(is224 = false, sharedMemory = false) {
    this.init(is224, sharedMemory);
  }

  protected init(is224: boolean, sharedMemory: boolean): void {
    if (sharedMemory) {
      // deno-fmt-ignore
      blocks[0] = blocks[16] = blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] = blocks[9] = blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = 0;
      this.#blocks = blocks;
    } else {
      this.#blocks = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    }

    if (is224) {
      this.#h0 = 0xc1059ed8;
      this.#h1 = 0x367cd507;
      this.#h2 = 0x3070dd17;
      this.#h3 = 0xf70e5939;
      this.#h4 = 0xffc00b31;
      this.#h5 = 0x68581511;
      this.#h6 = 0x64f98fa7;
      this.#h7 = 0xbefa4fa4;
    } else {
      // 256
      this.#h0 = 0x6a09e667;
      this.#h1 = 0xbb67ae85;
      this.#h2 = 0x3c6ef372;
      this.#h3 = 0xa54ff53a;
      this.#h4 = 0x510e527f;
      this.#h5 = 0x9b05688c;
      this.#h6 = 0x1f83d9ab;
      this.#h7 = 0x5be0cd19;
    }

    this.#block = this.#start = this.#bytes = this.#hBytes = 0;
    this.#finalized = this.#hashed = false;
    this.#first = true;
    this.#is224 = is224;
  }

  /** Update hash
   *
   * @param message The message you want to hash.
   */
  update(message: Message): this {
    if (this.#finalized) {
      return this;
    }

    let msg: string | number[] | Uint8Array | undefined;
    if (message instanceof ArrayBuffer) {
      msg = new Uint8Array(message);
    } else {
      msg = message;
    }

    let index = 0;
    const length = msg.length;
    const blocks = this.#blocks;

    while (index < length) {
      let i: number;
      if (this.#hashed) {
        this.#hashed = false;
        blocks[0] = this.#block;
        // deno-fmt-ignore
        blocks[16] = blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] = blocks[9] = blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = 0;
      }

      if (typeof msg !== "string") {
        for (i = this.#start; index < length && i < 64; ++index) {
          blocks[i >> 2] |= msg[index] << SHIFT[i++ & 3];
        }
      } else {
        for (i = this.#start; index < length && i < 64; ++index) {
          let code = msg.charCodeAt(index);
          if (code < 0x80) {
            blocks[i >> 2] |= code << SHIFT[i++ & 3];
          } else if (code < 0x800) {
            blocks[i >> 2] |= (0xc0 | (code >> 6)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | (code & 0x3f)) << SHIFT[i++ & 3];
          } else if (code < 0xd800 || code >= 0xe000) {
            blocks[i >> 2] |= (0xe0 | (code >> 12)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | ((code >> 6) & 0x3f)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | (code & 0x3f)) << SHIFT[i++ & 3];
          } else {
            code = 0x10000 +
              (((code & 0x3ff) << 10) | (msg.charCodeAt(++index) & 0x3ff));
            blocks[i >> 2] |= (0xf0 | (code >> 18)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | ((code >> 12) & 0x3f)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | ((code >> 6) & 0x3f)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | (code & 0x3f)) << SHIFT[i++ & 3];
          }
        }
      }

      this.#lastByteIndex = i;
      this.#bytes += i - this.#start;
      if (i >= 64) {
        this.#block = blocks[16];
        this.#start = i - 64;
        this.hash();
        this.#hashed = true;
      } else {
        this.#start = i;
      }
    }
    if (this.#bytes > 4294967295) {
      this.#hBytes += (this.#bytes / 4294967296) << 0;
      this.#bytes = this.#bytes % 4294967296;
    }
    return this;
  }

  protected finalize(): void {
    if (this.#finalized) {
      return;
    }
    this.#finalized = true;
    const blocks = this.#blocks;
    const i = this.#lastByteIndex;
    blocks[16] = this.#block;
    blocks[i >> 2] |= EXTRA[i & 3];
    this.#block = blocks[16];
    if (i >= 56) {
      if (!this.#hashed) {
        this.hash();
      }
      blocks[0] = this.#block;
      // deno-fmt-ignore
      blocks[16] = blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] = blocks[9] = blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = 0;
    }
    blocks[14] = (this.#hBytes << 3) | (this.#bytes >>> 29);
    blocks[15] = this.#bytes << 3;
    this.hash();
  }

  protected hash(): void {
    let a = this.#h0;
    let b = this.#h1;
    let c = this.#h2;
    let d = this.#h3;
    let e = this.#h4;
    let f = this.#h5;
    let g = this.#h6;
    let h = this.#h7;
    const blocks = this.#blocks;
    let s0: number;
    let s1: number;
    let maj: number;
    let t1: number;
    let t2: number;
    let ch: number;
    let ab: number;
    let da: number;
    let cd: number;
    let bc: number;

    for (let j = 16; j < 64; ++j) {
      // rightrotate
      t1 = blocks[j - 15];
      s0 = ((t1 >>> 7) | (t1 << 25)) ^ ((t1 >>> 18) | (t1 << 14)) ^ (t1 >>> 3);
      t1 = blocks[j - 2];
      s1 = ((t1 >>> 17) | (t1 << 15)) ^ ((t1 >>> 19) | (t1 << 13)) ^
        (t1 >>> 10);
      blocks[j] = (blocks[j - 16] + s0 + blocks[j - 7] + s1) << 0;
    }

    bc = b & c;
    for (let j = 0; j < 64; j += 4) {
      if (this.#first) {
        if (this.#is224) {
          ab = 300032;
          t1 = blocks[0] - 1413257819;
          h = (t1 - 150054599) << 0;
          d = (t1 + 24177077) << 0;
        } else {
          ab = 704751109;
          t1 = blocks[0] - 210244248;
          h = (t1 - 1521486534) << 0;
          d = (t1 + 143694565) << 0;
        }
        this.#first = false;
      } else {
        s0 = ((a >>> 2) | (a << 30)) ^
          ((a >>> 13) | (a << 19)) ^
          ((a >>> 22) | (a << 10));
        s1 = ((e >>> 6) | (e << 26)) ^
          ((e >>> 11) | (e << 21)) ^
          ((e >>> 25) | (e << 7));
        ab = a & b;
        maj = ab ^ (a & c) ^ bc;
        ch = (e & f) ^ (~e & g);
        t1 = h + s1 + ch + K[j] + blocks[j];
        t2 = s0 + maj;
        h = (d + t1) << 0;
        d = (t1 + t2) << 0;
      }
      s0 = ((d >>> 2) | (d << 30)) ^
        ((d >>> 13) | (d << 19)) ^
        ((d >>> 22) | (d << 10));
      s1 = ((h >>> 6) | (h << 26)) ^
        ((h >>> 11) | (h << 21)) ^
        ((h >>> 25) | (h << 7));
      da = d & a;
      maj = da ^ (d & b) ^ ab;
      ch = (h & e) ^ (~h & f);
      t1 = g + s1 + ch + K[j + 1] + blocks[j + 1];
      t2 = s0 + maj;
      g = (c + t1) << 0;
      c = (t1 + t2) << 0;
      s0 = ((c >>> 2) | (c << 30)) ^
        ((c >>> 13) | (c << 19)) ^
        ((c >>> 22) | (c << 10));
      s1 = ((g >>> 6) | (g << 26)) ^
        ((g >>> 11) | (g << 21)) ^
        ((g >>> 25) | (g << 7));
      cd = c & d;
      maj = cd ^ (c & a) ^ da;
      ch = (g & h) ^ (~g & e);
      t1 = f + s1 + ch + K[j + 2] + blocks[j + 2];
      t2 = s0 + maj;
      f = (b + t1) << 0;
      b = (t1 + t2) << 0;
      s0 = ((b >>> 2) | (b << 30)) ^
        ((b >>> 13) | (b << 19)) ^
        ((b >>> 22) | (b << 10));
      s1 = ((f >>> 6) | (f << 26)) ^
        ((f >>> 11) | (f << 21)) ^
        ((f >>> 25) | (f << 7));
      bc = b & c;
      maj = bc ^ (b & d) ^ cd;
      ch = (f & g) ^ (~f & h);
      t1 = e + s1 + ch + K[j + 3] + blocks[j + 3];
      t2 = s0 + maj;
      e = (a + t1) << 0;
      a = (t1 + t2) << 0;
    }

    this.#h0 = (this.#h0 + a) << 0;
    this.#h1 = (this.#h1 + b) << 0;
    this.#h2 = (this.#h2 + c) << 0;
    this.#h3 = (this.#h3 + d) << 0;
    this.#h4 = (this.#h4 + e) << 0;
    this.#h5 = (this.#h5 + f) << 0;
    this.#h6 = (this.#h6 + g) << 0;
    this.#h7 = (this.#h7 + h) << 0;
  }

  /** Return hash in hex string. */
  hex(): string {
    this.finalize();

    const h0 = this.#h0;
    const h1 = this.#h1;
    const h2 = this.#h2;
    const h3 = this.#h3;
    const h4 = this.#h4;
    const h5 = this.#h5;
    const h6 = this.#h6;
    const h7 = this.#h7;

    let hex = HEX_CHARS[(h0 >> 28) & 0x0f] +
      HEX_CHARS[(h0 >> 24) & 0x0f] +
      HEX_CHARS[(h0 >> 20) & 0x0f] +
      HEX_CHARS[(h0 >> 16) & 0x0f] +
      HEX_CHARS[(h0 >> 12) & 0x0f] +
      HEX_CHARS[(h0 >> 8) & 0x0f] +
      HEX_CHARS[(h0 >> 4) & 0x0f] +
      HEX_CHARS[h0 & 0x0f] +
      HEX_CHARS[(h1 >> 28) & 0x0f] +
      HEX_CHARS[(h1 >> 24) & 0x0f] +
      HEX_CHARS[(h1 >> 20) & 0x0f] +
      HEX_CHARS[(h1 >> 16) & 0x0f] +
      HEX_CHARS[(h1 >> 12) & 0x0f] +
      HEX_CHARS[(h1 >> 8) & 0x0f] +
      HEX_CHARS[(h1 >> 4) & 0x0f] +
      HEX_CHARS[h1 & 0x0f] +
      HEX_CHARS[(h2 >> 28) & 0x0f] +
      HEX_CHARS[(h2 >> 24) & 0x0f] +
      HEX_CHARS[(h2 >> 20) & 0x0f] +
      HEX_CHARS[(h2 >> 16) & 0x0f] +
      HEX_CHARS[(h2 >> 12) & 0x0f] +
      HEX_CHARS[(h2 >> 8) & 0x0f] +
      HEX_CHARS[(h2 >> 4) & 0x0f] +
      HEX_CHARS[h2 & 0x0f] +
      HEX_CHARS[(h3 >> 28) & 0x0f] +
      HEX_CHARS[(h3 >> 24) & 0x0f] +
      HEX_CHARS[(h3 >> 20) & 0x0f] +
      HEX_CHARS[(h3 >> 16) & 0x0f] +
      HEX_CHARS[(h3 >> 12) & 0x0f] +
      HEX_CHARS[(h3 >> 8) & 0x0f] +
      HEX_CHARS[(h3 >> 4) & 0x0f] +
      HEX_CHARS[h3 & 0x0f] +
      HEX_CHARS[(h4 >> 28) & 0x0f] +
      HEX_CHARS[(h4 >> 24) & 0x0f] +
      HEX_CHARS[(h4 >> 20) & 0x0f] +
      HEX_CHARS[(h4 >> 16) & 0x0f] +
      HEX_CHARS[(h4 >> 12) & 0x0f] +
      HEX_CHARS[(h4 >> 8) & 0x0f] +
      HEX_CHARS[(h4 >> 4) & 0x0f] +
      HEX_CHARS[h4 & 0x0f] +
      HEX_CHARS[(h5 >> 28) & 0x0f] +
      HEX_CHARS[(h5 >> 24) & 0x0f] +
      HEX_CHARS[(h5 >> 20) & 0x0f] +
      HEX_CHARS[(h5 >> 16) & 0x0f] +
      HEX_CHARS[(h5 >> 12) & 0x0f] +
      HEX_CHARS[(h5 >> 8) & 0x0f] +
      HEX_CHARS[(h5 >> 4) & 0x0f] +
      HEX_CHARS[h5 & 0x0f] +
      HEX_CHARS[(h6 >> 28) & 0x0f] +
      HEX_CHARS[(h6 >> 24) & 0x0f] +
      HEX_CHARS[(h6 >> 20) & 0x0f] +
      HEX_CHARS[(h6 >> 16) & 0x0f] +
      HEX_CHARS[(h6 >> 12) & 0x0f] +
      HEX_CHARS[(h6 >> 8) & 0x0f] +
      HEX_CHARS[(h6 >> 4) & 0x0f] +
      HEX_CHARS[h6 & 0x0f];
    if (!this.#is224) {
      hex += HEX_CHARS[(h7 >> 28) & 0x0f] +
        HEX_CHARS[(h7 >> 24) & 0x0f] +
        HEX_CHARS[(h7 >> 20) & 0x0f] +
        HEX_CHARS[(h7 >> 16) & 0x0f] +
        HEX_CHARS[(h7 >> 12) & 0x0f] +
        HEX_CHARS[(h7 >> 8) & 0x0f] +
        HEX_CHARS[(h7 >> 4) & 0x0f] +
        HEX_CHARS[h7 & 0x0f];
    }
    return hex;
  }

  /** Return hash in hex string. */
  toString(): string {
    return this.hex();
  }

  /** Return hash in integer array. */
  digest(): number[] {
    this.finalize();

    const h0 = this.#h0;
    const h1 = this.#h1;
    const h2 = this.#h2;
    const h3 = this.#h3;
    const h4 = this.#h4;
    const h5 = this.#h5;
    const h6 = this.#h6;
    const h7 = this.#h7;

    const arr = [
      (h0 >> 24) & 0xff,
      (h0 >> 16) & 0xff,
      (h0 >> 8) & 0xff,
      h0 & 0xff,
      (h1 >> 24) & 0xff,
      (h1 >> 16) & 0xff,
      (h1 >> 8) & 0xff,
      h1 & 0xff,
      (h2 >> 24) & 0xff,
      (h2 >> 16) & 0xff,
      (h2 >> 8) & 0xff,
      h2 & 0xff,
      (h3 >> 24) & 0xff,
      (h3 >> 16) & 0xff,
      (h3 >> 8) & 0xff,
      h3 & 0xff,
      (h4 >> 24) & 0xff,
      (h4 >> 16) & 0xff,
      (h4 >> 8) & 0xff,
      h4 & 0xff,
      (h5 >> 24) & 0xff,
      (h5 >> 16) & 0xff,
      (h5 >> 8) & 0xff,
      h5 & 0xff,
      (h6 >> 24) & 0xff,
      (h6 >> 16) & 0xff,
      (h6 >> 8) & 0xff,
      h6 & 0xff,
    ];
    if (!this.#is224) {
      arr.push(
        (h7 >> 24) & 0xff,
        (h7 >> 16) & 0xff,
        (h7 >> 8) & 0xff,
        h7 & 0xff,
      );
    }
    return arr;
  }

  /** Return hash in integer array. */
  array(): number[] {
    return this.digest();
  }

  /** Return hash in ArrayBuffer. */
  arrayBuffer(): ArrayBuffer {
    this.finalize();

    const buffer = new ArrayBuffer(this.#is224 ? 28 : 32);
    const dataView = new DataView(buffer);
    dataView.setUint32(0, this.#h0);
    dataView.setUint32(4, this.#h1);
    dataView.setUint32(8, this.#h2);
    dataView.setUint32(12, this.#h3);
    dataView.setUint32(16, this.#h4);
    dataView.setUint32(20, this.#h5);
    dataView.setUint32(24, this.#h6);
    if (!this.#is224) {
      dataView.setUint32(28, this.#h7);
    }
    return buffer;
  }
}

export class HmacSha256 extends Sha256 {
  #inner: boolean;
  #is224: boolean;
  #oKeyPad: number[];
  #sharedMemory: boolean;

  constructor(secretKey: Message, is224 = false, sharedMemory = false) {
    super(is224, sharedMemory);

    let key: number[] | Uint8Array | undefined;
    if (typeof secretKey === "string") {
      const bytes: number[] = [];
      const length = secretKey.length;
      let index = 0;
      for (let i = 0; i < length; ++i) {
        let code = secretKey.charCodeAt(i);
        if (code < 0x80) {
          bytes[index++] = code;
        } else if (code < 0x800) {
          bytes[index++] = 0xc0 | (code >> 6);
          bytes[index++] = 0x80 | (code & 0x3f);
        } else if (code < 0xd800 || code >= 0xe000) {
          bytes[index++] = 0xe0 | (code >> 12);
          bytes[index++] = 0x80 | ((code >> 6) & 0x3f);
          bytes[index++] = 0x80 | (code & 0x3f);
        } else {
          code = 0x10000 +
            (((code & 0x3ff) << 10) | (secretKey.charCodeAt(++i) & 0x3ff));
          bytes[index++] = 0xf0 | (code >> 18);
          bytes[index++] = 0x80 | ((code >> 12) & 0x3f);
          bytes[index++] = 0x80 | ((code >> 6) & 0x3f);
          bytes[index++] = 0x80 | (code & 0x3f);
        }
      }
      key = bytes;
    } else {
      if (secretKey instanceof ArrayBuffer) {
        key = new Uint8Array(secretKey);
      } else {
        key = secretKey;
      }
    }

    if (key.length > 64) {
      key = new Sha256(is224, true).update(key).array();
    }

    const oKeyPad: number[] = [];
    const iKeyPad: number[] = [];
    for (let i = 0; i < 64; ++i) {
      const b = key[i] || 0;
      oKeyPad[i] = 0x5c ^ b;
      iKeyPad[i] = 0x36 ^ b;
    }

    this.update(iKeyPad);
    this.#oKeyPad = oKeyPad;
    this.#inner = true;
    this.#is224 = is224;
    this.#sharedMemory = sharedMemory;
  }

  protected finalize(): void {
    super.finalize();
    if (this.#inner) {
      this.#inner = false;
      const innerHash = this.array();
      super.init(this.#is224, this.#sharedMemory);
      this.update(this.#oKeyPad);
      this.update(innerHash);
      super.finalize();
    }
  }
}
