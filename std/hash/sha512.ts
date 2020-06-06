/*
 * [js-sha512]{@link https://github.com/emn178/js-sha512}
 *
 * @version 0.8.0
 * @author Chen, Yi-Cyuan [emn178@gmail.com]
 * @copyright Chen, Yi-Cyuan 2014-2018
 * @license MIT
 */

export type Message = string | number[] | ArrayBuffer;

// prettier-ignore
const HEX_CHARS = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f"] as const;
const EXTRA = [-2147483648, 8388608, 32768, 128] as const;
const SHIFT = [24, 16, 8, 0] as const;
// prettier-ignore
const K = [
  0x428a2f98, 0xd728ae22, 0x71374491, 0x23ef65cd, 0xb5c0fbcf, 0xec4d3b2f, 0xe9b5dba5, 0x8189dbbc, 0x3956c25b,
  0xf348b538, 0x59f111f1, 0xb605d019, 0x923f82a4, 0xaf194f9b, 0xab1c5ed5, 0xda6d8118, 0xd807aa98, 0xa3030242,
  0x12835b01, 0x45706fbe, 0x243185be, 0x4ee4b28c, 0x550c7dc3, 0xd5ffb4e2, 0x72be5d74, 0xf27b896f, 0x80deb1fe,
  0x3b1696b1, 0x9bdc06a7, 0x25c71235, 0xc19bf174, 0xcf692694, 0xe49b69c1, 0x9ef14ad2, 0xefbe4786, 0x384f25e3,
  0x0fc19dc6, 0x8b8cd5b5, 0x240ca1cc, 0x77ac9c65, 0x2de92c6f, 0x592b0275, 0x4a7484aa, 0x6ea6e483, 0x5cb0a9dc,
  0xbd41fbd4, 0x76f988da, 0x831153b5, 0x983e5152, 0xee66dfab, 0xa831c66d, 0x2db43210, 0xb00327c8, 0x98fb213f,
  0xbf597fc7, 0xbeef0ee4, 0xc6e00bf3, 0x3da88fc2, 0xd5a79147, 0x930aa725, 0x06ca6351, 0xe003826f, 0x14292967,
  0x0a0e6e70, 0x27b70a85, 0x46d22ffc, 0x2e1b2138, 0x5c26c926, 0x4d2c6dfc, 0x5ac42aed, 0x53380d13, 0x9d95b3df,
  0x650a7354, 0x8baf63de, 0x766a0abb, 0x3c77b2a8, 0x81c2c92e, 0x47edaee6, 0x92722c85, 0x1482353b, 0xa2bfe8a1,
  0x4cf10364, 0xa81a664b, 0xbc423001, 0xc24b8b70, 0xd0f89791, 0xc76c51a3, 0x0654be30, 0xd192e819, 0xd6ef5218,
  0xd6990624, 0x5565a910, 0xf40e3585, 0x5771202a, 0x106aa070, 0x32bbd1b8, 0x19a4c116, 0xb8d2d0c8, 0x1e376c08,
  0x5141ab53, 0x2748774c, 0xdf8eeb99, 0x34b0bcb5, 0xe19b48a8, 0x391c0cb3, 0xc5c95a63, 0x4ed8aa4a, 0xe3418acb,
  0x5b9cca4f, 0x7763e373, 0x682e6ff3, 0xd6b2b8a3, 0x748f82ee, 0x5defb2fc, 0x78a5636f, 0x43172f60, 0x84c87814,
  0xa1f0ab72, 0x8cc70208, 0x1a6439ec, 0x90befffa, 0x23631e28, 0xa4506ceb, 0xde82bde9, 0xbef9a3f7, 0xb2c67915,
  0xc67178f2, 0xe372532b, 0xca273ece, 0xea26619c, 0xd186b8c7, 0x21c0c207, 0xeada7dd6, 0xcde0eb1e, 0xf57d4f7f,
  0xee6ed178, 0x06f067aa, 0x72176fba, 0x0a637dc5, 0xa2c898a6, 0x113f9804, 0xbef90dae, 0x1b710b35, 0x131c471b,
  0x28db77f5, 0x23047d84, 0x32caab7b, 0x40c72493, 0x3c9ebe0a, 0x15c9bebc, 0x431d67c4, 0x9c100d4c, 0x4cc5d4be,
  0xcb3e42b6, 0x597f299c, 0xfc657e2a, 0x5fcb6fab, 0x3ad6faec, 0x6c44198c, 0x4a475817
] as const;

const blocks: number[] = [];

// prettier-ignore
export class Sha512 {
  #blocks!: number[];
  #block!: number;
  #bits!: number;
  #start!: number;
  #bytes!: number;
  #hBytes!: number;
  #lastByteIndex = 0;
  #finalized!: boolean;
  #hashed!: boolean;
  #h0h!: number;
  #h0l!: number;
  #h1h!: number;
  #h1l!: number;
  #h2h!: number;
  #h2l!: number;
  #h3h!: number;
  #h3l!: number;
  #h4h!: number;
  #h4l!: number;
  #h5h!: number;
  #h5l!: number;
  #h6h!: number;
  #h6l!: number;
  #h7h!: number;
  #h7l!: number;

  constructor(bits = 512, sharedMemory = false) {
    this.init(bits, sharedMemory);
  }

  protected init(bits: number, sharedMemory: boolean): void {
    if (sharedMemory) {
      blocks[0] = blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] =
      blocks[9] = blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = blocks[16] =
      blocks[17] = blocks[18] = blocks[19] = blocks[20] = blocks[21] = blocks[22] = blocks[23] = blocks[24] =
      blocks[25] = blocks[26] = blocks[27] = blocks[28] = blocks[29] = blocks[30] = blocks[31] = blocks[32] = 0;
      this.#blocks = blocks;
    } else {
      this.#blocks =
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    }
    if (bits === 224) {
      this.#h0h = 0x8c3d37c8;
      this.#h0l = 0x19544da2;
      this.#h1h = 0x73e19966;
      this.#h1l = 0x89dcd4d6;
      this.#h2h = 0x1dfab7ae;
      this.#h2l = 0x32ff9c82;
      this.#h3h = 0x679dd514;
      this.#h3l = 0x582f9fcf;
      this.#h4h = 0x0f6d2b69;
      this.#h4l = 0x7bd44da8;
      this.#h5h = 0x77e36f73;
      this.#h5l = 0x04c48942;
      this.#h6h = 0x3f9d85a8;
      this.#h6l = 0x6a1d36c8;
      this.#h7h = 0x1112e6ad;
      this.#h7l = 0x91d692a1;
    } else if (bits === 256) {
      this.#h0h = 0x22312194;
      this.#h0l = 0xfc2bf72c;
      this.#h1h = 0x9f555fa3;
      this.#h1l = 0xc84c64c2;
      this.#h2h = 0x2393b86b;
      this.#h2l = 0x6f53b151;
      this.#h3h = 0x96387719;
      this.#h3l = 0x5940eabd;
      this.#h4h = 0x96283ee2;
      this.#h4l = 0xa88effe3;
      this.#h5h = 0xbe5e1e25;
      this.#h5l = 0x53863992;
      this.#h6h = 0x2b0199fc;
      this.#h6l = 0x2c85b8aa;
      this.#h7h = 0x0eb72ddc;
      this.#h7l = 0x81c52ca2;
    } else if (bits === 384) {
      this.#h0h = 0xcbbb9d5d;
      this.#h0l = 0xc1059ed8;
      this.#h1h = 0x629a292a;
      this.#h1l = 0x367cd507;
      this.#h2h = 0x9159015a;
      this.#h2l = 0x3070dd17;
      this.#h3h = 0x152fecd8;
      this.#h3l = 0xf70e5939;
      this.#h4h = 0x67332667;
      this.#h4l = 0xffc00b31;
      this.#h5h = 0x8eb44a87;
      this.#h5l = 0x68581511;
      this.#h6h = 0xdb0c2e0d;
      this.#h6l = 0x64f98fa7;
      this.#h7h = 0x47b5481d;
      this.#h7l = 0xbefa4fa4;
    } else { // 512
      this.#h0h = 0x6a09e667;
      this.#h0l = 0xf3bcc908;
      this.#h1h = 0xbb67ae85;
      this.#h1l = 0x84caa73b;
      this.#h2h = 0x3c6ef372;
      this.#h2l = 0xfe94f82b;
      this.#h3h = 0xa54ff53a;
      this.#h3l = 0x5f1d36f1;
      this.#h4h = 0x510e527f;
      this.#h4l = 0xade682d1;
      this.#h5h = 0x9b05688c;
      this.#h5l = 0x2b3e6c1f;
      this.#h6h = 0x1f83d9ab;
      this.#h6l = 0xfb41bd6b;
      this.#h7h = 0x5be0cd19;
      this.#h7l = 0x137e2179;
    }
    this.#bits = bits;
    this.#block = this.#start = this.#bytes = this.#hBytes = 0;
    this.#finalized = this.#hashed = false;
  }

  update(message: Message): this {
    if (this.#finalized) {
      return this;
    }
    let msg: string | number[] | Uint8Array;
    if (message instanceof ArrayBuffer) {
      msg = new Uint8Array(message);
    } else {
      msg = message;
    }
    const length = msg.length;
    const blocks = this.#blocks;
    let index = 0;
    while (index < length) {
      let i: number;
      if (this.#hashed) {
        this.#hashed = false;
        blocks[0] = this.#block;
        blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] =
        blocks[9] = blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = blocks[16] =
        blocks[17] = blocks[18] = blocks[19] = blocks[20] = blocks[21] = blocks[22] = blocks[23] = blocks[24] =
        blocks[25] = blocks[26] = blocks[27] = blocks[28] = blocks[29] = blocks[30] = blocks[31] = blocks[32] = 0;
      }
      if (typeof msg !== "string") {
        for (i = this.#start; index < length && i < 128; ++index) {
          blocks[i >> 2] |= msg[index] << SHIFT[i++ & 3];
        }
      } else {
        for (i = this.#start; index < length && i < 128; ++index) {
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
            code = 0x10000 + (((code & 0x3ff) << 10) | (msg.charCodeAt(++index) & 0x3ff));
            blocks[i >> 2] |= (0xf0 | (code >> 18)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | ((code >> 12) & 0x3f)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | ((code >> 6) & 0x3f)) << SHIFT[i++ & 3];
            blocks[i >> 2] |= (0x80 | (code & 0x3f)) << SHIFT[i++ & 3];
          }
        }
      }
      this.#lastByteIndex = i;
      this.#bytes += i - this.#start;
      if (i >= 128) {
        this.#block = blocks[32];
        this.#start = i - 128;
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
    blocks[32] = this.#block;
    blocks[i >> 2] |= EXTRA[i & 3];
    this.#block = blocks[32];
    if (i >= 112) {
      if (!this.#hashed) {
        this.hash();
      }
      blocks[0] = this.#block;
      blocks[1] = blocks[2] = blocks[3] = blocks[4] = blocks[5] = blocks[6] = blocks[7] = blocks[8] =
      blocks[9] =blocks[10] = blocks[11] = blocks[12] = blocks[13] = blocks[14] = blocks[15] = blocks[16] =
      blocks[17] = blocks[18] = blocks[19] = blocks[20] = blocks[21] = blocks[22] = blocks[23] = blocks[24] =
      blocks[25] = blocks[26] = blocks[27] = blocks[28] = blocks[29] = blocks[30] = blocks[31] = blocks[32] = 0;
    }
    blocks[30] = (this.#hBytes << 3) | (this.#bytes >>> 29);
    blocks[31] = this.#bytes << 3;
    this.hash();
  }

  protected hash(): void {
    const
      h0h = this.#h0h, h0l = this.#h0l, h1h = this.#h1h, h1l = this.#h1l, h2h = this.#h2h, h2l = this.#h2l,
      h3h = this.#h3h, h3l = this.#h3l, h4h = this.#h4h, h4l = this.#h4l, h5h = this.#h5h, h5l = this.#h5l,
      h6h = this.#h6h, h6l = this.#h6l, h7h = this.#h7h, h7l = this.#h7l;

    let s0h, s0l, s1h, s1l, c1, c2, c3, c4, abh, abl, dah, dal, cdh, cdl, bch, bcl, majh, majl,
      t1h, t1l, t2h, t2l, chh, chl: number;

    const blocks = this.#blocks;

    for (let j = 32; j < 160; j += 2) {
      t1h = blocks[j - 30];
      t1l = blocks[j - 29];
      s0h = ((t1h >>> 1) | (t1l << 31)) ^ ((t1h >>> 8) | (t1l << 24)) ^ (t1h >>> 7);
      s0l = ((t1l >>> 1) | (t1h << 31)) ^ ((t1l >>> 8) | (t1h << 24)) ^ ((t1l >>> 7) | (t1h << 25));

      t1h = blocks[j - 4];
      t1l = blocks[j - 3];
      s1h = ((t1h >>> 19) | (t1l << 13)) ^ ((t1l >>> 29) | (t1h << 3)) ^ (t1h >>> 6);
      s1l = ((t1l >>> 19) | (t1h << 13)) ^ ((t1h >>> 29) | (t1l << 3)) ^ ((t1l >>> 6) | (t1h << 26));

      t1h = blocks[j - 32];
      t1l = blocks[j - 31];
      t2h = blocks[j - 14];
      t2l = blocks[j - 13];

      c1 = (t2l & 0xffff) + (t1l & 0xffff) + (s0l & 0xffff) + (s1l & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (s0l >>> 16) + (s1l >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (s0h & 0xffff) + (s1h & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (s0h >>> 16) + (s1h >>> 16) + (c3 >>> 16);

      blocks[j] = (c4 << 16) | (c3 & 0xffff);
      blocks[j + 1] = (c2 << 16) | (c1 & 0xffff);
    }

    let ah = h0h, al = h0l, bh = h1h, bl = h1l, ch = h2h, cl = h2l, dh = h3h, dl = h3l, eh = h4h, el = h4l,
      fh = h5h, fl = h5l, gh = h6h, gl = h6l, hh = h7h, hl = h7l;

    bch = bh & ch;
    bcl = bl & cl;

    for (let j = 0; j < 160; j += 8) {
      s0h = ((ah >>> 28) | (al << 4)) ^ ((al >>> 2) | (ah << 30)) ^ ((al >>> 7) | (ah << 25));
      s0l = ((al >>> 28) | (ah << 4)) ^ ((ah >>> 2) | (al << 30)) ^ ((ah >>> 7) | (al << 25));

      s1h = ((eh >>> 14) | (el << 18)) ^ ((eh >>> 18) | (el << 14)) ^ ((el >>> 9) | (eh << 23));
      s1l = ((el >>> 14) | (eh << 18)) ^ ((el >>> 18) | (eh << 14)) ^ ((eh >>> 9) | (el << 23));

      abh = ah & bh;
      abl = al & bl;
      majh = abh ^ (ah & ch) ^ bch;
      majl = abl ^ (al & cl) ^ bcl;

      chh = (eh & fh) ^ (~eh & gh);
      chl = (el & fl) ^ (~el & gl);

      t1h = blocks[j];
      t1l = blocks[j + 1];
      t2h = K[j];
      t2l = K[j + 1];

      c1 = (t2l & 0xffff) + (t1l & 0xffff) + (chl & 0xffff) + (s1l & 0xffff) + (hl & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (chl >>> 16) + (s1l >>> 16) + (hl >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (chh & 0xffff) + (s1h & 0xffff) + (hh & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (chh >>> 16) + (s1h >>> 16) + (hh >>> 16) + (c3 >>> 16);

      t1h = (c4 << 16) | (c3 & 0xffff);
      t1l = (c2 << 16) | (c1 & 0xffff);

      c1 = (majl & 0xffff) + (s0l & 0xffff);
      c2 = (majl >>> 16) + (s0l >>> 16) + (c1 >>> 16);
      c3 = (majh & 0xffff) + (s0h & 0xffff) + (c2 >>> 16);
      c4 = (majh >>> 16) + (s0h >>> 16) + (c3 >>> 16);

      t2h = (c4 << 16) | (c3 & 0xffff);
      t2l = (c2 << 16) | (c1 & 0xffff);

      c1 = (dl & 0xffff) + (t1l & 0xffff);
      c2 = (dl >>> 16) + (t1l >>> 16) + (c1 >>> 16);
      c3 = (dh & 0xffff) + (t1h & 0xffff) + (c2 >>> 16);
      c4 = (dh >>> 16) + (t1h >>> 16) + (c3 >>> 16);

      hh = (c4 << 16) | (c3 & 0xffff);
      hl = (c2 << 16) | (c1 & 0xffff);

      c1 = (t2l & 0xffff) + (t1l & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (c3 >>> 16);

      dh = (c4 << 16) | (c3 & 0xffff);
      dl = (c2 << 16) | (c1 & 0xffff);

      s0h = ((dh >>> 28) | (dl << 4)) ^ ((dl >>> 2) | (dh << 30)) ^ ((dl >>> 7) | (dh << 25));
      s0l = ((dl >>> 28) | (dh << 4)) ^ ((dh >>> 2) | (dl << 30)) ^ ((dh >>> 7) | (dl << 25));

      s1h = ((hh >>> 14) | (hl << 18)) ^ ((hh >>> 18) | (hl << 14)) ^ ((hl >>> 9) | (hh << 23));
      s1l = ((hl >>> 14) | (hh << 18)) ^ ((hl >>> 18) | (hh << 14)) ^ ((hh >>> 9) | (hl << 23));

      dah = dh & ah;
      dal = dl & al;
      majh = dah ^ (dh & bh) ^ abh;
      majl = dal ^ (dl & bl) ^ abl;

      chh = (hh & eh) ^ (~hh & fh);
      chl = (hl & el) ^ (~hl & fl);

      t1h = blocks[j + 2];
      t1l = blocks[j + 3];
      t2h = K[j + 2];
      t2l = K[j + 3];

      c1 = (t2l & 0xffff) + (t1l & 0xffff) + (chl & 0xffff) + (s1l & 0xffff) + (gl & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (chl >>> 16) + (s1l >>> 16) + (gl >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (chh & 0xffff) + (s1h & 0xffff) + (gh & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (chh >>> 16) + (s1h >>> 16) + (gh >>> 16) + (c3 >>> 16);

      t1h = (c4 << 16) | (c3 & 0xffff);
      t1l = (c2 << 16) | (c1 & 0xffff);

      c1 = (majl & 0xffff) + (s0l & 0xffff);
      c2 = (majl >>> 16) + (s0l >>> 16) + (c1 >>> 16);
      c3 = (majh & 0xffff) + (s0h & 0xffff) + (c2 >>> 16);
      c4 = (majh >>> 16) + (s0h >>> 16) + (c3 >>> 16);

      t2h = (c4 << 16) | (c3 & 0xffff);
      t2l = (c2 << 16) | (c1 & 0xffff);

      c1 = (cl & 0xffff) + (t1l & 0xffff);
      c2 = (cl >>> 16) + (t1l >>> 16) + (c1 >>> 16);
      c3 = (ch & 0xffff) + (t1h & 0xffff) + (c2 >>> 16);
      c4 = (ch >>> 16) + (t1h >>> 16) + (c3 >>> 16);

      gh = (c4 << 16) | (c3 & 0xffff);
      gl = (c2 << 16) | (c1 & 0xffff);

      c1 = (t2l & 0xffff) + (t1l & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (c3 >>> 16);

      ch = (c4 << 16) | (c3 & 0xffff);
      cl = (c2 << 16) | (c1 & 0xffff);

      s0h = ((ch >>> 28) | (cl << 4)) ^ ((cl >>> 2) | (ch << 30)) ^ ((cl >>> 7) | (ch << 25));
      s0l = ((cl >>> 28) | (ch << 4)) ^ ((ch >>> 2) | (cl << 30)) ^ ((ch >>> 7) | (cl << 25));

      s1h = ((gh >>> 14) | (gl << 18)) ^ ((gh >>> 18) | (gl << 14)) ^ ((gl >>> 9) | (gh << 23));
      s1l = ((gl >>> 14) | (gh << 18)) ^ ((gl >>> 18) | (gh << 14)) ^ ((gh >>> 9) | (gl << 23));

      cdh = ch & dh;
      cdl = cl & dl;
      majh = cdh ^ (ch & ah) ^ dah;
      majl = cdl ^ (cl & al) ^ dal;

      chh = (gh & hh) ^ (~gh & eh);
      chl = (gl & hl) ^ (~gl & el);

      t1h = blocks[j + 4];
      t1l = blocks[j + 5];
      t2h = K[j + 4];
      t2l = K[j + 5];

      c1 = (t2l & 0xffff) + (t1l & 0xffff) + (chl & 0xffff) + (s1l & 0xffff) + (fl & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (chl >>> 16) + (s1l >>> 16) + (fl >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (chh & 0xffff) + (s1h & 0xffff) + (fh & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (chh >>> 16) + (s1h >>> 16) + (fh >>> 16) + (c3 >>> 16);

      t1h = (c4 << 16) | (c3 & 0xffff);
      t1l = (c2 << 16) | (c1 & 0xffff);

      c1 = (majl & 0xffff) + (s0l & 0xffff);
      c2 = (majl >>> 16) + (s0l >>> 16) + (c1 >>> 16);
      c3 = (majh & 0xffff) + (s0h & 0xffff) + (c2 >>> 16);
      c4 = (majh >>> 16) + (s0h >>> 16) + (c3 >>> 16);

      t2h = (c4 << 16) | (c3 & 0xffff);
      t2l = (c2 << 16) | (c1 & 0xffff);

      c1 = (bl & 0xffff) + (t1l & 0xffff);
      c2 = (bl >>> 16) + (t1l >>> 16) + (c1 >>> 16);
      c3 = (bh & 0xffff) + (t1h & 0xffff) + (c2 >>> 16);
      c4 = (bh >>> 16) + (t1h >>> 16) + (c3 >>> 16);

      fh = (c4 << 16) | (c3 & 0xffff);
      fl = (c2 << 16) | (c1 & 0xffff);

      c1 = (t2l & 0xffff) + (t1l & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (c3 >>> 16);

      bh = (c4 << 16) | (c3 & 0xffff);
      bl = (c2 << 16) | (c1 & 0xffff);

      s0h = ((bh >>> 28) | (bl << 4)) ^ ((bl >>> 2) | (bh << 30)) ^ ((bl >>> 7) | (bh << 25));
      s0l = ((bl >>> 28) | (bh << 4)) ^ ((bh >>> 2) | (bl << 30)) ^ ((bh >>> 7) | (bl << 25));

      s1h = ((fh >>> 14) | (fl << 18)) ^ ((fh >>> 18) | (fl << 14)) ^ ((fl >>> 9) | (fh << 23));
      s1l = ((fl >>> 14) | (fh << 18)) ^ ((fl >>> 18) | (fh << 14)) ^ ((fh >>> 9) | (fl << 23));

      bch = bh & ch;
      bcl = bl & cl;
      majh = bch ^ (bh & dh) ^ cdh;
      majl = bcl ^ (bl & dl) ^ cdl;

      chh = (fh & gh) ^ (~fh & hh);
      chl = (fl & gl) ^ (~fl & hl);

      t1h = blocks[j + 6];
      t1l = blocks[j + 7];
      t2h = K[j + 6];
      t2l = K[j + 7];

      c1 = (t2l & 0xffff) + (t1l & 0xffff) + (chl & 0xffff) + (s1l & 0xffff) + (el & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (chl >>> 16) + (s1l >>> 16) + (el >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (chh & 0xffff) + (s1h & 0xffff) + (eh & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (chh >>> 16) + (s1h >>> 16) + (eh >>> 16) + (c3 >>> 16);

      t1h = (c4 << 16) | (c3 & 0xffff);
      t1l = (c2 << 16) | (c1 & 0xffff);

      c1 = (majl & 0xffff) + (s0l & 0xffff);
      c2 = (majl >>> 16) + (s0l >>> 16) + (c1 >>> 16);
      c3 = (majh & 0xffff) + (s0h & 0xffff) + (c2 >>> 16);
      c4 = (majh >>> 16) + (s0h >>> 16) + (c3 >>> 16);

      t2h = (c4 << 16) | (c3 & 0xffff);
      t2l = (c2 << 16) | (c1 & 0xffff);

      c1 = (al & 0xffff) + (t1l & 0xffff);
      c2 = (al >>> 16) + (t1l >>> 16) + (c1 >>> 16);
      c3 = (ah & 0xffff) + (t1h & 0xffff) + (c2 >>> 16);
      c4 = (ah >>> 16) + (t1h >>> 16) + (c3 >>> 16);

      eh = (c4 << 16) | (c3 & 0xffff);
      el = (c2 << 16) | (c1 & 0xffff);

      c1 = (t2l & 0xffff) + (t1l & 0xffff);
      c2 = (t2l >>> 16) + (t1l >>> 16) + (c1 >>> 16);
      c3 = (t2h & 0xffff) + (t1h & 0xffff) + (c2 >>> 16);
      c4 = (t2h >>> 16) + (t1h >>> 16) + (c3 >>> 16);

      ah = (c4 << 16) | (c3 & 0xffff);
      al = (c2 << 16) | (c1 & 0xffff);
    }

    c1 = (h0l & 0xffff) + (al & 0xffff);
    c2 = (h0l >>> 16) + (al >>> 16) + (c1 >>> 16);
    c3 = (h0h & 0xffff) + (ah & 0xffff) + (c2 >>> 16);
    c4 = (h0h >>> 16) + (ah >>> 16) + (c3 >>> 16);

    this.#h0h = (c4 << 16) | (c3 & 0xffff);
    this.#h0l = (c2 << 16) | (c1 & 0xffff);

    c1 = (h1l & 0xffff) + (bl & 0xffff);
    c2 = (h1l >>> 16) + (bl >>> 16) + (c1 >>> 16);
    c3 = (h1h & 0xffff) + (bh & 0xffff) + (c2 >>> 16);
    c4 = (h1h >>> 16) + (bh >>> 16) + (c3 >>> 16);

    this.#h1h = (c4 << 16) | (c3 & 0xffff);
    this.#h1l = (c2 << 16) | (c1 & 0xffff);

    c1 = (h2l & 0xffff) + (cl & 0xffff);
    c2 = (h2l >>> 16) + (cl >>> 16) + (c1 >>> 16);
    c3 = (h2h & 0xffff) + (ch & 0xffff) + (c2 >>> 16);
    c4 = (h2h >>> 16) + (ch >>> 16) + (c3 >>> 16);

    this.#h2h = (c4 << 16) | (c3 & 0xffff);
    this.#h2l = (c2 << 16) | (c1 & 0xffff);

    c1 = (h3l & 0xffff) + (dl & 0xffff);
    c2 = (h3l >>> 16) + (dl >>> 16) + (c1 >>> 16);
    c3 = (h3h & 0xffff) + (dh & 0xffff) + (c2 >>> 16);
    c4 = (h3h >>> 16) + (dh >>> 16) + (c3 >>> 16);

    this.#h3h = (c4 << 16) | (c3 & 0xffff);
    this.#h3l = (c2 << 16) | (c1 & 0xffff);

    c1 = (h4l & 0xffff) + (el & 0xffff);
    c2 = (h4l >>> 16) + (el >>> 16) + (c1 >>> 16);
    c3 = (h4h & 0xffff) + (eh & 0xffff) + (c2 >>> 16);
    c4 = (h4h >>> 16) + (eh >>> 16) + (c3 >>> 16);

    this.#h4h = (c4 << 16) | (c3 & 0xffff);
    this.#h4l = (c2 << 16) | (c1 & 0xffff);

    c1 = (h5l & 0xffff) + (fl & 0xffff);
    c2 = (h5l >>> 16) + (fl >>> 16) + (c1 >>> 16);
    c3 = (h5h & 0xffff) + (fh & 0xffff) + (c2 >>> 16);
    c4 = (h5h >>> 16) + (fh >>> 16) + (c3 >>> 16);

    this.#h5h = (c4 << 16) | (c3 & 0xffff);
    this.#h5l = (c2 << 16) | (c1 & 0xffff);

    c1 = (h6l & 0xffff) + (gl & 0xffff);
    c2 = (h6l >>> 16) + (gl >>> 16) + (c1 >>> 16);
    c3 = (h6h & 0xffff) + (gh & 0xffff) + (c2 >>> 16);
    c4 = (h6h >>> 16) + (gh >>> 16) + (c3 >>> 16);

    this.#h6h = (c4 << 16) | (c3 & 0xffff);
    this.#h6l = (c2 << 16) | (c1 & 0xffff);

    c1 = (h7l & 0xffff) + (hl & 0xffff);
    c2 = (h7l >>> 16) + (hl >>> 16) + (c1 >>> 16);
    c3 = (h7h & 0xffff) + (hh & 0xffff) + (c2 >>> 16);
    c4 = (h7h >>> 16) + (hh >>> 16) + (c3 >>> 16);

    this.#h7h = (c4 << 16) | (c3 & 0xffff);
    this.#h7l = (c2 << 16) | (c1 & 0xffff);
  }

  hex(): string {
    this.finalize();
    const
      h0h = this.#h0h, h0l = this.#h0l, h1h = this.#h1h, h1l = this.#h1l, h2h = this.#h2h, h2l = this.#h2l,
      h3h = this.#h3h, h3l = this.#h3l, h4h = this.#h4h, h4l = this.#h4l, h5h = this.#h5h, h5l = this.#h5l,
      h6h = this.#h6h, h6l = this.#h6l, h7h = this.#h7h, h7l = this.#h7l, bits = this.#bits;
    let hex = 
      HEX_CHARS[(h0h >> 28) & 0x0f] + HEX_CHARS[(h0h >> 24) & 0x0f] +
      HEX_CHARS[(h0h >> 20) & 0x0f] + HEX_CHARS[(h0h >> 16) & 0x0f] +
      HEX_CHARS[(h0h >> 12) & 0x0f] + HEX_CHARS[(h0h >> 8) & 0x0f] +
      HEX_CHARS[(h0h >> 4) & 0x0f] + HEX_CHARS[h0h & 0x0f] +
      HEX_CHARS[(h0l >> 28) & 0x0f] + HEX_CHARS[(h0l >> 24) & 0x0f] +
      HEX_CHARS[(h0l >> 20) & 0x0f] + HEX_CHARS[(h0l >> 16) & 0x0f] +
      HEX_CHARS[(h0l >> 12) & 0x0f] + HEX_CHARS[(h0l >> 8) & 0x0f] +
      HEX_CHARS[(h0l >> 4) & 0x0f] + HEX_CHARS[h0l & 0x0f] +
      HEX_CHARS[(h1h >> 28) & 0x0f] + HEX_CHARS[(h1h >> 24) & 0x0f] +
      HEX_CHARS[(h1h >> 20) & 0x0f] + HEX_CHARS[(h1h >> 16) & 0x0f] +
      HEX_CHARS[(h1h >> 12) & 0x0f] + HEX_CHARS[(h1h >> 8) & 0x0f] +
      HEX_CHARS[(h1h >> 4) & 0x0f] + HEX_CHARS[h1h & 0x0f] +
      HEX_CHARS[(h1l >> 28) & 0x0f] + HEX_CHARS[(h1l >> 24) & 0x0f] +
      HEX_CHARS[(h1l >> 20) & 0x0f] + HEX_CHARS[(h1l >> 16) & 0x0f] +
      HEX_CHARS[(h1l >> 12) & 0x0f] + HEX_CHARS[(h1l >> 8) & 0x0f] +
      HEX_CHARS[(h1l >> 4) & 0x0f] + HEX_CHARS[h1l & 0x0f] +
      HEX_CHARS[(h2h >> 28) & 0x0f] + HEX_CHARS[(h2h >> 24) & 0x0f] +
      HEX_CHARS[(h2h >> 20) & 0x0f] + HEX_CHARS[(h2h >> 16) & 0x0f] +
      HEX_CHARS[(h2h >> 12) & 0x0f] + HEX_CHARS[(h2h >> 8) & 0x0f] +
      HEX_CHARS[(h2h >> 4) & 0x0f] + HEX_CHARS[h2h & 0x0f] +
      HEX_CHARS[(h2l >> 28) & 0x0f] + HEX_CHARS[(h2l >> 24) & 0x0f] +
      HEX_CHARS[(h2l >> 20) & 0x0f] + HEX_CHARS[(h2l >> 16) & 0x0f] +
      HEX_CHARS[(h2l >> 12) & 0x0f] + HEX_CHARS[(h2l >> 8) & 0x0f] +
      HEX_CHARS[(h2l >> 4) & 0x0f] + HEX_CHARS[h2l & 0x0f] +
      HEX_CHARS[(h3h >> 28) & 0x0f] + HEX_CHARS[(h3h >> 24) & 0x0f] +
      HEX_CHARS[(h3h >> 20) & 0x0f] + HEX_CHARS[(h3h >> 16) & 0x0f] +
      HEX_CHARS[(h3h >> 12) & 0x0f] + HEX_CHARS[(h3h >> 8) & 0x0f] +
      HEX_CHARS[(h3h >> 4) & 0x0f] + HEX_CHARS[h3h & 0x0f];
    if (bits >= 256) {
      hex +=
        HEX_CHARS[(h3l >> 28) & 0x0f] + HEX_CHARS[(h3l >> 24) & 0x0f] +
        HEX_CHARS[(h3l >> 20) & 0x0f] + HEX_CHARS[(h3l >> 16) & 0x0f] +
        HEX_CHARS[(h3l >> 12) & 0x0f] + HEX_CHARS[(h3l >> 8) & 0x0f] +
        HEX_CHARS[(h3l >> 4) & 0x0f] + HEX_CHARS[h3l & 0x0f];
    }
    if (bits >= 384) {
      hex +=
        HEX_CHARS[(h4h >> 28) & 0x0f] + HEX_CHARS[(h4h >> 24) & 0x0f] +
        HEX_CHARS[(h4h >> 20) & 0x0f] + HEX_CHARS[(h4h >> 16) & 0x0f] +
        HEX_CHARS[(h4h >> 12) & 0x0f] + HEX_CHARS[(h4h >> 8) & 0x0f] +
        HEX_CHARS[(h4h >> 4) & 0x0f] + HEX_CHARS[h4h & 0x0f] +
        HEX_CHARS[(h4l >> 28) & 0x0f] + HEX_CHARS[(h4l >> 24) & 0x0f] +
        HEX_CHARS[(h4l >> 20) & 0x0f] + HEX_CHARS[(h4l >> 16) & 0x0f] +
        HEX_CHARS[(h4l >> 12) & 0x0f] + HEX_CHARS[(h4l >> 8) & 0x0f] +
        HEX_CHARS[(h4l >> 4) & 0x0f] + HEX_CHARS[h4l & 0x0f] +
        HEX_CHARS[(h5h >> 28) & 0x0f] + HEX_CHARS[(h5h >> 24) & 0x0f] +
        HEX_CHARS[(h5h >> 20) & 0x0f] + HEX_CHARS[(h5h >> 16) & 0x0f] +
        HEX_CHARS[(h5h >> 12) & 0x0f] + HEX_CHARS[(h5h >> 8) & 0x0f] +
        HEX_CHARS[(h5h >> 4) & 0x0f] + HEX_CHARS[h5h & 0x0f] +
        HEX_CHARS[(h5l >> 28) & 0x0f] + HEX_CHARS[(h5l >> 24) & 0x0f] +
        HEX_CHARS[(h5l >> 20) & 0x0f] + HEX_CHARS[(h5l >> 16) & 0x0f] +
        HEX_CHARS[(h5l >> 12) & 0x0f] + HEX_CHARS[(h5l >> 8) & 0x0f] +
        HEX_CHARS[(h5l >> 4) & 0x0f] + HEX_CHARS[h5l & 0x0f];
    }
    if (bits === 512) {
      hex +=
        HEX_CHARS[(h6h >> 28) & 0x0f] + HEX_CHARS[(h6h >> 24) & 0x0f] +
        HEX_CHARS[(h6h >> 20) & 0x0f] + HEX_CHARS[(h6h >> 16) & 0x0f] +
        HEX_CHARS[(h6h >> 12) & 0x0f] + HEX_CHARS[(h6h >> 8) & 0x0f] +
        HEX_CHARS[(h6h >> 4) & 0x0f] + HEX_CHARS[h6h & 0x0f] +
        HEX_CHARS[(h6l >> 28) & 0x0f] + HEX_CHARS[(h6l >> 24) & 0x0f] +
        HEX_CHARS[(h6l >> 20) & 0x0f] + HEX_CHARS[(h6l >> 16) & 0x0f] +
        HEX_CHARS[(h6l >> 12) & 0x0f] + HEX_CHARS[(h6l >> 8) & 0x0f] +
        HEX_CHARS[(h6l >> 4) & 0x0f] + HEX_CHARS[h6l & 0x0f] +
        HEX_CHARS[(h7h >> 28) & 0x0f] + HEX_CHARS[(h7h >> 24) & 0x0f] +
        HEX_CHARS[(h7h >> 20) & 0x0f] + HEX_CHARS[(h7h >> 16) & 0x0f] +
        HEX_CHARS[(h7h >> 12) & 0x0f] + HEX_CHARS[(h7h >> 8) & 0x0f] +
        HEX_CHARS[(h7h >> 4) & 0x0f] + HEX_CHARS[h7h & 0x0f] +
        HEX_CHARS[(h7l >> 28) & 0x0f] + HEX_CHARS[(h7l >> 24) & 0x0f] +
        HEX_CHARS[(h7l >> 20) & 0x0f] + HEX_CHARS[(h7l >> 16) & 0x0f] +
        HEX_CHARS[(h7l >> 12) & 0x0f] + HEX_CHARS[(h7l >> 8) & 0x0f] +
        HEX_CHARS[(h7l >> 4) & 0x0f] + HEX_CHARS[h7l & 0x0f];
    }
    return hex;
  }

  toString(): string {
    return this.hex();
  }

  digest(): number[] {
    this.finalize();
    const
      h0h = this.#h0h, h0l = this.#h0l, h1h = this.#h1h, h1l = this.#h1l, h2h = this.#h2h, h2l = this.#h2l,
      h3h = this.#h3h, h3l = this.#h3l, h4h = this.#h4h, h4l = this.#h4l, h5h = this.#h5h, h5l = this.#h5l,
      h6h = this.#h6h, h6l = this.#h6l, h7h = this.#h7h, h7l = this.#h7l, bits = this.#bits;
    const arr = [
      (h0h >> 24) & 0xff, (h0h >> 16) & 0xff, (h0h >> 8) & 0xff, h0h & 0xff,
      (h0l >> 24) & 0xff, (h0l >> 16) & 0xff, (h0l >> 8) & 0xff, h0l & 0xff,
      (h1h >> 24) & 0xff, (h1h >> 16) & 0xff, (h1h >> 8) & 0xff, h1h & 0xff,
      (h1l >> 24) & 0xff, (h1l >> 16) & 0xff, (h1l >> 8) & 0xff, h1l & 0xff,
      (h2h >> 24) & 0xff, (h2h >> 16) & 0xff, (h2h >> 8) & 0xff, h2h & 0xff,
      (h2l >> 24) & 0xff, (h2l >> 16) & 0xff, (h2l >> 8) & 0xff, h2l & 0xff,
      (h3h >> 24) & 0xff, (h3h >> 16) & 0xff, (h3h >> 8) & 0xff, h3h & 0xff
    ];
    if (bits >= 256) {
      arr.push((h3l >> 24) & 0xff, (h3l >> 16) & 0xff, (h3l >> 8) & 0xff, h3l & 0xff);
    }
    if (bits >= 384) {
      arr.push(
        (h4h >> 24) & 0xff, (h4h >> 16) & 0xff, (h4h >> 8) & 0xff, h4h & 0xff,
        (h4l >> 24) & 0xff, (h4l >> 16) & 0xff, (h4l >> 8) & 0xff, h4l & 0xff,
        (h5h >> 24) & 0xff, (h5h >> 16) & 0xff, (h5h >> 8) & 0xff, h5h & 0xff,
        (h5l >> 24) & 0xff, (h5l >> 16) & 0xff, (h5l >> 8) & 0xff, h5l & 0xff
      );
    }
    if (bits === 512) {
      arr.push(
        (h6h >> 24) & 0xff, (h6h >> 16) & 0xff, (h6h >> 8) & 0xff, h6h & 0xff,
        (h6l >> 24) & 0xff, (h6l >> 16) & 0xff, (h6l >> 8) & 0xff, h6l & 0xff,
        (h7h >> 24) & 0xff, (h7h >> 16) & 0xff, (h7h >> 8) & 0xff, h7h & 0xff,
        (h7l >> 24) & 0xff, (h7l >> 16) & 0xff, (h7l >> 8) & 0xff, h7l & 0xff
      );
    }
    return arr;
  }

  array(): number[] {
    return this.digest();
  }

  arrayBuffer(): ArrayBuffer {
    this.finalize();
    const bits = this.#bits;
    const buffer = new ArrayBuffer(bits / 8);
    const dataView = new DataView(buffer);
    dataView.setUint32(0, this.#h0h);
    dataView.setUint32(4, this.#h0l);
    dataView.setUint32(8, this.#h1h);
    dataView.setUint32(12, this.#h1l);
    dataView.setUint32(16, this.#h2h);
    dataView.setUint32(20, this.#h2l);
    dataView.setUint32(24, this.#h3h);
    if (bits >= 256) {
      dataView.setUint32(28, this.#h3l);
    }
    if (bits >= 384) {
      dataView.setUint32(32, this.#h4h);
      dataView.setUint32(36, this.#h4l);
      dataView.setUint32(40, this.#h5h);
      dataView.setUint32(44, this.#h5l);
    }
    if (bits === 512) {
      dataView.setUint32(48, this.#h6h);
      dataView.setUint32(52, this.#h6l);
      dataView.setUint32(56, this.#h7h);
      dataView.setUint32(60, this.#h7l);
    }
    return buffer;
  }
}

export class HmacSha512 extends Sha512 {
  #inner: boolean;
  #bits: number;
  #oKeyPad: number[];
  #sharedMemory: boolean;

  constructor(secretKey: Message, bits = 512, sharedMemory = false) {
    super(bits, sharedMemory);

    let key: number[] | Uint8Array;

    if (secretKey instanceof ArrayBuffer) {
      key = new Uint8Array(secretKey);
    } else if (typeof secretKey === "string") {
      const bytes: number[] = [];
      const length = secretKey.length;
      let index = 0;
      let code: number;
      for (let i = 0; i < length; ++i) {
        code = secretKey.charCodeAt(i);
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
          code =
            0x10000 +
            (((code & 0x3ff) << 10) | (secretKey.charCodeAt(++i) & 0x3ff));
          bytes[index++] = 0xf0 | (code >> 18);
          bytes[index++] = 0x80 | ((code >> 12) & 0x3f);
          bytes[index++] = 0x80 | ((code >> 6) & 0x3f);
          bytes[index++] = 0x80 | (code & 0x3f);
        }
      }
      key = bytes;
    } else {
      key = secretKey;
    }
    if (key.length > 128) {
      key = new Sha512(bits, true).update(key).array();
    }
    const oKeyPad: number[] = [];
    const iKeyPad: number[] = [];
    for (let i = 0; i < 128; ++i) {
      const b = key[i] || 0;
      oKeyPad[i] = 0x5c ^ b;
      iKeyPad[i] = 0x36 ^ b;
    }
    this.update(iKeyPad);
    this.#inner = true;
    this.#bits = bits;
    this.#oKeyPad = oKeyPad;
    this.#sharedMemory = sharedMemory;
  }

  protected finalize(): void {
    super.finalize();
    if (this.#inner) {
      this.#inner = false;
      const innerHash = this.array();
      super.init(this.#bits, this.#sharedMemory);
      this.update(this.#oKeyPad);
      this.update(innerHash);
      super.finalize();
    }
  }
}
