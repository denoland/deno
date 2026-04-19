// FoundationDB-style tuple layer key encoder/decoder for Deno KV.
// Implements the same binary format as denokv_proto::codec.

// ---------------------------------------------------------------------------
// Type tags - cross-type ordering: Bytes < String < Int < Float < False < True
// ---------------------------------------------------------------------------
const BYTES = 0x01;
const STRING = 0x02;
const NEGINTSTART = 0x0b;
const INTZERO = 0x14;
const POSINTEND = 0x1d;
const DOUBLE = 0x21;
const FALSE = 0x26;
const TRUE = 0x27;
const ESCAPE = 0xff;

// ---------------------------------------------------------------------------
// KeyPart
// ---------------------------------------------------------------------------
type KeyPart =
  | { type: "bytes"; value: Uint8Array }
  | { type: "string"; value: string }
  | { type: "int"; value: bigint }
  | { type: "float"; value: number }
  | { type: "false" }
  | { type: "true" };

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
let textEncoder: TextEncoder;
let textDecoder: TextDecoder;

function getEncoder(): TextEncoder {
  if (!textEncoder) textEncoder = new TextEncoder();
  return textEncoder;
}

function getDecoder(): TextDecoder {
  if (!textDecoder) textDecoder = new TextDecoder();
  return textDecoder;
}

// Scratch buffer for float64 <-> uint64 bit reinterpretation
const f64Buf = new ArrayBuffer(8);
const f64View = new DataView(f64Buf);

/** Convert a non-negative bigint to minimal big-endian bytes. */
function bigintToBeBytes(v: bigint): Uint8Array {
  if (v === 0n) return new Uint8Array(0);
  let hex = v.toString(16);
  if (hex.length & 1) hex = "0" + hex;
  const bytes = new Uint8Array(hex.length >> 1);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(hex.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

/** Convert big-endian bytes to a non-negative bigint. */
function beBytesToBigint(bytes: Uint8Array): bigint {
  if (bytes.length === 0) return 0n;
  let v = 0n;
  for (let i = 0; i < bytes.length; i++) {
    v = (v << 8n) | BigInt(bytes[i]);
  }
  return v;
}

/** Ones'-complement every byte in place and return the array. */
function onesComplement(bytes: Uint8Array): Uint8Array {
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] ^= 0xff;
  }
  return bytes;
}

// ---------------------------------------------------------------------------
// Growable buffer used during encoding
// ---------------------------------------------------------------------------
class BufWriter {
  buf: Uint8Array;
  len = 0;

  constructor(initialCap = 64) {
    this.buf = new Uint8Array(initialCap);
  }

  private grow(need: number): void {
    if (this.len + need <= this.buf.length) return;
    let cap = this.buf.length;
    while (cap < this.len + need) cap *= 2;
    const next = new Uint8Array(cap);
    next.set(this.buf.subarray(0, this.len));
    this.buf = next;
  }

  push(byte: number): void {
    this.grow(1);
    this.buf[this.len++] = byte;
  }

  pushBytes(data: Uint8Array): void {
    this.grow(data.length);
    this.buf.set(data, this.len);
    this.len += data.length;
  }

  finish(): Uint8Array {
    return this.buf.slice(0, this.len);
  }
}

// ---------------------------------------------------------------------------
// Encode helpers
// ---------------------------------------------------------------------------

/** Encode a null-escaped byte sequence (used for both BYTES and STRING). */
function encodeNullEscaped(w: BufWriter, tag: number, data: Uint8Array): void {
  w.push(tag);
  for (let i = 0; i < data.length; i++) {
    w.push(data[i]);
    if (data[i] === 0x00) {
      w.push(ESCAPE);
    }
  }
  w.push(0x00); // terminator
}

function encodeBytes(w: BufWriter, value: Uint8Array): void {
  encodeNullEscaped(w, BYTES, value);
}

function encodeString(w: BufWriter, value: string): void {
  encodeNullEscaped(w, STRING, getEncoder().encode(value));
}

function encodeInt(w: BufWriter, value: bigint): void {
  if (value === 0n) {
    w.push(INTZERO);
    return;
  }

  if (value > 0n) {
    const beBytes = bigintToBeBytes(value);
    const n = beBytes.length;
    if (n <= 8) {
      w.push(INTZERO + n);
      w.pushBytes(beBytes);
    } else {
      w.push(POSINTEND);
      w.push(n);
      w.pushBytes(beBytes);
    }
  } else {
    // negative
    const abs = -value;
    const beBytes = bigintToBeBytes(abs);
    const n = beBytes.length;
    const complemented = onesComplement(beBytes);
    if (n <= 8) {
      w.push(INTZERO - n);
      w.pushBytes(complemented);
    } else {
      w.push(NEGINTSTART);
      w.push(n ^ 0xff);
      w.pushBytes(complemented);
    }
  }
}

function encodeFloat(w: BufWriter, value: number): void {
  // Canonicalize NaN: preserve sign but force canonical payload
  if (Number.isNaN(value)) {
    // Read the original bits to detect the sign bit.
    // JS `-NaN` is just `NaN`, but negative NaN can arrive via DataView
    // or typed-array manipulation and the sign bit is preserved.
    f64View.setFloat64(0, value, false);
    const signBit = f64View.getUint32(0, false) & 0x80000000;
    if (signBit) {
      // Negative NaN: canonical bits 0xfff8000000000000
      f64View.setUint32(0, 0xfff80000, false);
    } else {
      // Positive NaN: canonical bits 0x7ff8000000000000
      f64View.setUint32(0, 0x7ff80000, false);
    }
    f64View.setUint32(4, 0x00000000, false);
  } else {
    f64View.setFloat64(0, value, false);
  }

  // Read as two u32 for bit manipulation (JS doesn't have u64)
  let hi = f64View.getUint32(0, false);
  let lo = f64View.getUint32(4, false);

  // If sign bit set, XOR all bits; else XOR only sign bit
  if (hi & 0x80000000) {
    hi ^= 0xffffffff;
    lo ^= 0xffffffff;
  } else {
    hi ^= 0x80000000;
  }

  w.push(DOUBLE);
  // Write 8 big-endian bytes
  w.push((hi >>> 24) & 0xff);
  w.push((hi >>> 16) & 0xff);
  w.push((hi >>> 8) & 0xff);
  w.push(hi & 0xff);
  w.push((lo >>> 24) & 0xff);
  w.push((lo >>> 16) & 0xff);
  w.push((lo >>> 8) & 0xff);
  w.push(lo & 0xff);
}

function encodePart(w: BufWriter, part: KeyPart): void {
  switch (part.type) {
    case "bytes":
      encodeBytes(w, part.value);
      break;
    case "string":
      encodeString(w, part.value);
      break;
    case "int":
      encodeInt(w, part.value);
      break;
    case "float":
      encodeFloat(w, part.value);
      break;
    case "false":
      w.push(FALSE);
      break;
    case "true":
      w.push(TRUE);
      break;
  }
}

// ---------------------------------------------------------------------------
// Decode helpers
// ---------------------------------------------------------------------------
class BufReader {
  private data: Uint8Array;
  pos: number;

  constructor(data: Uint8Array) {
    this.data = data;
    this.pos = 0;
  }

  get remaining(): number {
    return this.data.length - this.pos;
  }

  peek(): number {
    if (this.pos >= this.data.length) {
      throw new Error("Unexpected end of key data");
    }
    return this.data[this.pos];
  }

  read(): number {
    if (this.pos >= this.data.length) {
      throw new Error("Unexpected end of key data");
    }
    return this.data[this.pos++];
  }

  readBytes(n: number): Uint8Array {
    if (this.pos + n > this.data.length) {
      throw new Error("Unexpected end of key data");
    }
    const slice = this.data.slice(this.pos, this.pos + n);
    this.pos += n;
    return slice;
  }
}

/** Decode a null-escaped byte sequence (for BYTES and STRING). */
function decodeNullEscaped(r: BufReader): Uint8Array {
  const chunks: number[] = [];
  while (true) {
    const b = r.read();
    if (b === 0x00) {
      // Check if this is an escaped null or the terminator
      if (r.remaining > 0 && r.peek() === ESCAPE) {
        // Escaped null byte - consume the ESCAPE and emit 0x00
        r.read();
        chunks.push(0x00);
      } else {
        // Terminator
        break;
      }
    } else {
      chunks.push(b);
    }
  }
  return new Uint8Array(chunks);
}

function decodeBytes(r: BufReader): KeyPart {
  return { type: "bytes", value: decodeNullEscaped(r) };
}

function decodeString(r: BufReader): KeyPart {
  const raw = decodeNullEscaped(r);
  return { type: "string", value: getDecoder().decode(raw) };
}

function decodeInt(r: BufReader, tag: number): KeyPart {
  if (tag === INTZERO) {
    return { type: "int", value: 0n };
  }

  if (tag > INTZERO && tag < POSINTEND) {
    // Positive, 1-8 bytes
    const n = tag - INTZERO;
    const beBytes = r.readBytes(n);
    return { type: "int", value: beBytesToBigint(beBytes) };
  }

  if (tag === POSINTEND) {
    // Positive, >8 bytes
    const n = r.read();
    const beBytes = r.readBytes(n);
    return { type: "int", value: beBytesToBigint(beBytes) };
  }

  if (tag < INTZERO && tag > NEGINTSTART) {
    // Negative, 1-8 bytes
    const n = INTZERO - tag;
    const complemented = r.readBytes(n);
    const beBytes = onesComplement(complemented);
    return { type: "int", value: -beBytesToBigint(beBytes) };
  }

  if (tag === NEGINTSTART) {
    // Negative, >8 bytes
    const nXor = r.read();
    const n = nXor ^ 0xff;
    const complemented = r.readBytes(n);
    const beBytes = onesComplement(complemented);
    return { type: "int", value: -beBytesToBigint(beBytes) };
  }

  throw new Error(`Invalid integer tag: 0x${tag.toString(16)}`);
}

function decodeFloat(r: BufReader): KeyPart {
  const bytes = r.readBytes(8);

  // Reconstruct hi/lo u32
  let hi =
    ((bytes[0] << 24) | (bytes[1] << 16) | (bytes[2] << 8) | bytes[3]) >>> 0;
  let lo =
    ((bytes[4] << 24) | (bytes[5] << 16) | (bytes[6] << 8) | bytes[7]) >>> 0;

  // Reverse the XOR: if sign bit set after XOR, it was originally positive
  // (XOR only flipped sign bit), else it was negative (XOR flipped all bits).
  if (hi & 0x80000000) {
    // Sign bit is set in encoded form - original was non-negative, undo XOR of sign bit
    hi ^= 0x80000000;
  } else {
    // Sign bit is clear in encoded form - original was negative, undo XOR of all bits
    hi ^= 0xffffffff;
    lo ^= 0xffffffff;
  }

  f64View.setUint32(0, hi, false);
  f64View.setUint32(4, lo, false);
  const value = f64View.getFloat64(0, false);

  return { type: "float", value };
}

function decodePart(r: BufReader): KeyPart {
  const tag = r.read();

  if (tag === BYTES) return decodeBytes(r);
  if (tag === STRING) return decodeString(r);
  if (tag === DOUBLE) return decodeFloat(r);
  if (tag === FALSE) return { type: "false" };
  if (tag === TRUE) return { type: "true" };

  // Integer range: NEGINTSTART..POSINTEND inclusive
  if (tag >= NEGINTSTART && tag <= POSINTEND) {
    return decodeInt(r, tag);
  }

  throw new Error(`Unknown key part tag: 0x${tag.toString(16)}`);
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/** Encode an array of KeyParts into a single binary key. */
export function encodeKey(parts: KeyPart[]): Uint8Array {
  const w = new BufWriter();
  for (let i = 0; i < parts.length; i++) {
    encodePart(w, parts[i]);
  }
  return w.finish();
}

/** Decode a binary key into an array of KeyParts. */
export function decodeKey(bytes: Uint8Array): KeyPart[] {
  const r = new BufReader(bytes);
  const parts: KeyPart[] = [];
  while (r.remaining > 0) {
    parts.push(decodePart(r));
  }
  return parts;
}

// ---------------------------------------------------------------------------
// Deno.KvKey conversion helpers
// ---------------------------------------------------------------------------

/** Convert a Deno.KvKey element array to typed KeyParts. */
export function kvKeyToKeyParts(
  key: (string | number | bigint | Uint8Array | boolean)[],
): KeyPart[] {
  const parts: KeyPart[] = [];
  for (let i = 0; i < key.length; i++) {
    const el = key[i];
    if (typeof el === "string") {
      parts.push({ type: "string", value: el });
    } else if (typeof el === "bigint") {
      parts.push({ type: "int", value: el });
    } else if (typeof el === "number") {
      parts.push({ type: "float", value: el });
    } else if (typeof el === "boolean") {
      parts.push(el ? { type: "true" } : { type: "false" });
    } else if (el instanceof Uint8Array) {
      parts.push({ type: "bytes", value: el });
    } else {
      throw new TypeError(
        `Invalid KvKey element at index ${i}: ${typeof el}`,
      );
    }
  }
  return parts;
}

/** Convert typed KeyParts back to a plain Deno.KvKey element array. */
export function keyPartsToKvKey(
  parts: KeyPart[],
): (string | number | bigint | Uint8Array | boolean)[] {
  const key: (string | number | bigint | Uint8Array | boolean)[] = [];
  for (let i = 0; i < parts.length; i++) {
    const p = parts[i];
    switch (p.type) {
      case "string":
        key.push(p.value);
        break;
      case "int":
        key.push(p.value);
        break;
      case "float":
        key.push(p.value);
        break;
      case "bytes":
        key.push(p.value);
        break;
      case "true":
        key.push(true);
        break;
      case "false":
        key.push(false);
        break;
    }
  }
  return key;
}

export type { KeyPart };
