/*
 * [js-sha1]{@link https://github.com/emn178/js-sha1}
 *
 * @version 0.6.0
 * @author Chen, Yi-Cyuan [emn178@gmail.com]
 * @copyright Chen, Yi-Cyuan 2014-2017
 * @license MIT
 */
/*jslint bitwise: true */

const HEX_CHARS = "0123456789abcdef".split("");
const EXTRA = [-2147483648, 8388608, 32768, 128];
const SHIFT = [24, 16, 8, 0];

const blocks = [];

export class Sha1 {
  blocks;
  block;
  start;
  bytes;
  hBytes;
  finalized;
  hashed;
  first;

  h0 = 0x67452301;
  h1 = 0xefcdab89;
  h2 = 0x98badcfe;
  h3 = 0x10325476;
  h4 = 0xc3d2e1f0;
  lastByteIndex = 0;

  constructor(sharedMemory: boolean = false) {
    if (sharedMemory) {
      blocks[0] = blocks[16] = blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] = blocks[9] = blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = 0;
      this.blocks = blocks;
    } else {
      this.blocks = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    }

    this.h0 = 0x67452301;
    this.h1 = 0xefcdab89;
    this.h2 = 0x98badcfe;
    this.h3 = 0x10325476;
    this.h4 = 0xc3d2e1f0;

    this.block = this.start = this.bytes = this.hBytes = 0;
    this.finalized = this.hashed = false;
    this.first = true;
  }

  update(data: string | ArrayBuffer) {
    if (this.finalized) {
      return;
    }
    let message;
    let notString = typeof data !== "string";
    if (notString && data instanceof ArrayBuffer) {
      message = new Uint8Array(data);
    } else {
      message = data;
    }
    let code,
      index = 0,
      i,
      length = message.length || 0,
      blocks = this.blocks;

    while (index < length) {
      if (this.hashed) {
        this.hashed = false;
        blocks[0] = this.block;
        blocks[16] = blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] = blocks[9] = blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = 0;
      }

      if (notString) {
        for (i = this.start; index < length && i < 64; ++index) {
          blocks[i >> 2] |= message[index] << SHIFT[i++ & 3];
        }
      } else {
        for (i = this.start; index < length && i < 64; ++index) {
          code = message.charCodeAt(index);
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
            code =
              0x10000 +
              (((code & 0x3ff) << 10) | (message.charCodeAt(++index) & 0x3ff));
            blocks[i >> 2] |= (0xf0 | (code >> 18)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | ((code >> 12) & 0x3f)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | ((code >> 6) & 0x3f)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | (code & 0x3f)) << SHIFT[i++ & 3];
          }
        }
      }

      this.lastByteIndex = i;
      this.bytes += i - this.start;
      if (i >= 64) {
        this.block = blocks[16];
        this.start = i - 64;
        this.hash();
        this.hashed = true;
      } else {
        this.start = i;
      }
    }
    if (this.bytes > 4294967295) {
      this.hBytes += (this.bytes / 4294967296) << 0;
      this.bytes = this.bytes % 4294967296;
    }
    return this;
  }

  finalize() {
    if (this.finalized) {
      return;
    }
    this.finalized = true;
    let blocks = this.blocks,
      i = this.lastByteIndex;
    blocks[16] = this.block;
    blocks[i >> 2] |= EXTRA[i & 3];
    this.block = blocks[16];
    if (i >= 56) {
      if (!this.hashed) {
        this.hash();
      }
      blocks[0] = this.block;
      blocks[16] = blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] = blocks[9] = blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = 0;
    }
    blocks[14] = (this.hBytes << 3) | (this.bytes >>> 29);
    blocks[15] = this.bytes << 3;
    this.hash();
  }

  hash() {
    let a = this.h0,
      b = this.h1,
      c = this.h2,
      d = this.h3,
      e = this.h4;
    let f,
      j,
      t,
      blocks = this.blocks;

    for (j = 16; j < 80; ++j) {
      t = blocks[j - 3] ^ blocks[j - 8] ^ blocks[j - 14] ^ blocks[j - 16];
      blocks[j] = (t << 1) | (t >>> 31);
    }

    for (j = 0; j < 20; j += 5) {
      f = (b & c) | (~b & d);
      t = (a << 5) | (a >>> 27);
      e = (t + f + e + 1518500249 + blocks[j]) << 0;
      b = (b << 30) | (b >>> 2);

      f = (a & b) | (~a & c);
      t = (e << 5) | (e >>> 27);
      d = (t + f + d + 1518500249 + blocks[j + 1]) << 0;
      a = (a << 30) | (a >>> 2);

      f = (e & a) | (~e & b);
      t = (d << 5) | (d >>> 27);
      c = (t + f + c + 1518500249 + blocks[j + 2]) << 0;
      e = (e << 30) | (e >>> 2);

      f = (d & e) | (~d & a);
      t = (c << 5) | (c >>> 27);
      b = (t + f + b + 1518500249 + blocks[j + 3]) << 0;
      d = (d << 30) | (d >>> 2);

      f = (c & d) | (~c & e);
      t = (b << 5) | (b >>> 27);
      a = (t + f + a + 1518500249 + blocks[j + 4]) << 0;
      c = (c << 30) | (c >>> 2);
    }

    for (; j < 40; j += 5) {
      f = b ^ c ^ d;
      t = (a << 5) | (a >>> 27);
      e = (t + f + e + 1859775393 + blocks[j]) << 0;
      b = (b << 30) | (b >>> 2);

      f = a ^ b ^ c;
      t = (e << 5) | (e >>> 27);
      d = (t + f + d + 1859775393 + blocks[j + 1]) << 0;
      a = (a << 30) | (a >>> 2);

      f = e ^ a ^ b;
      t = (d << 5) | (d >>> 27);
      c = (t + f + c + 1859775393 + blocks[j + 2]) << 0;
      e = (e << 30) | (e >>> 2);

      f = d ^ e ^ a;
      t = (c << 5) | (c >>> 27);
      b = (t + f + b + 1859775393 + blocks[j + 3]) << 0;
      d = (d << 30) | (d >>> 2);

      f = c ^ d ^ e;
      t = (b << 5) | (b >>> 27);
      a = (t + f + a + 1859775393 + blocks[j + 4]) << 0;
      c = (c << 30) | (c >>> 2);
    }

    for (; j < 60; j += 5) {
      f = (b & c) | (b & d) | (c & d);
      t = (a << 5) | (a >>> 27);
      e = (t + f + e - 1894007588 + blocks[j]) << 0;
      b = (b << 30) | (b >>> 2);

      f = (a & b) | (a & c) | (b & c);
      t = (e << 5) | (e >>> 27);
      d = (t + f + d - 1894007588 + blocks[j + 1]) << 0;
      a = (a << 30) | (a >>> 2);

      f = (e & a) | (e & b) | (a & b);
      t = (d << 5) | (d >>> 27);
      c = (t + f + c - 1894007588 + blocks[j + 2]) << 0;
      e = (e << 30) | (e >>> 2);

      f = (d & e) | (d & a) | (e & a);
      t = (c << 5) | (c >>> 27);
      b = (t + f + b - 1894007588 + blocks[j + 3]) << 0;
      d = (d << 30) | (d >>> 2);

      f = (c & d) | (c & e) | (d & e);
      t = (b << 5) | (b >>> 27);
      a = (t + f + a - 1894007588 + blocks[j + 4]) << 0;
      c = (c << 30) | (c >>> 2);
    }

    for (; j < 80; j += 5) {
      f = b ^ c ^ d;
      t = (a << 5) | (a >>> 27);
      e = (t + f + e - 899497514 + blocks[j]) << 0;
      b = (b << 30) | (b >>> 2);

      f = a ^ b ^ c;
      t = (e << 5) | (e >>> 27);
      d = (t + f + d - 899497514 + blocks[j + 1]) << 0;
      a = (a << 30) | (a >>> 2);

      f = e ^ a ^ b;
      t = (d << 5) | (d >>> 27);
      c = (t + f + c - 899497514 + blocks[j + 2]) << 0;
      e = (e << 30) | (e >>> 2);

      f = d ^ e ^ a;
      t = (c << 5) | (c >>> 27);
      b = (t + f + b - 899497514 + blocks[j + 3]) << 0;
      d = (d << 30) | (d >>> 2);

      f = c ^ d ^ e;
      t = (b << 5) | (b >>> 27);
      a = (t + f + a - 899497514 + blocks[j + 4]) << 0;
      c = (c << 30) | (c >>> 2);
    }

    this.h0 = (this.h0 + a) << 0;
    this.h1 = (this.h1 + b) << 0;
    this.h2 = (this.h2 + c) << 0;
    this.h3 = (this.h3 + d) << 0;
    this.h4 = (this.h4 + e) << 0;
  }

  hex() {
    this.finalize();

    let h0 = this.h0,
      h1 = this.h1,
      h2 = this.h2,
      h3 = this.h3,
      h4 = this.h4;

    return (
      HEX_CHARS[(h0 >> 28) & 0x0f] +
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
      HEX_CHARS[h4 & 0x0f]
    );
  }

  toString() {
    return this.hex();
  }

  digest() {
    this.finalize();

    let h0 = this.h0,
      h1 = this.h1,
      h2 = this.h2,
      h3 = this.h3,
      h4 = this.h4;

    return [
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
      h4 & 0xff
    ];
  }

  array() {
    return this.digest();
  }

  arrayBuffer() {
    this.finalize();

    let buffer = new ArrayBuffer(20);
    let dataView = new DataView(buffer);
    dataView.setUint32(0, this.h0);
    dataView.setUint32(4, this.h1);
    dataView.setUint32(8, this.h2);
    dataView.setUint32(12, this.h3);
    dataView.setUint32(16, this.h4);
    return buffer;
  }
}
