// The following code is based off of text-encoding at:
// https://github.com/inexorabletash/text-encoding
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.

import * as base64 from "base64-js";
import * as domTypes from "./dom_types";
import { DenoError, ErrorKind } from "./errors";

/** Decodes a string of data which has been encoded using base-64. */
export function atob(s: string): string {
  const rem = s.length % 4;
  // base64-js requires length exactly times of 4
  if (rem > 0) {
    s = s.padEnd(s.length + (4 - rem), "=");
  }
  let byteArray;
  try {
    byteArray = base64.toByteArray(s);
  } catch (_) {
    throw new DenoError(
      ErrorKind.InvalidInput,
      "The string to be decoded is not correctly encoded"
    );
  }
  let result = "";
  for (let i = 0; i < byteArray.length; i++) {
    result += String.fromCharCode(byteArray[i]);
  }
  return result;
}

/** Creates a base-64 ASCII string from the input string. */
export function btoa(s: string): string {
  const byteArray = [];
  for (let i = 0; i < s.length; i++) {
    const charCode = s[i].charCodeAt(0);
    if (charCode > 0xff) {
      throw new DenoError(
        ErrorKind.InvalidInput,
        "The string to be encoded contains characters " +
          "outside of the Latin1 range."
      );
    }
    byteArray.push(charCode);
  }
  const result = base64.fromByteArray(Uint8Array.from(byteArray));
  return result;
}

interface DecoderOptions {
  fatal?: boolean;
}

interface Decoder {
  handler(stream: Stream, byte: number): number | null;
}

interface Encoder {
  handler(codePoint: number): number | number[];
}

const CONTINUE = null;
const END_OF_STREAM = -1;
const FINISHED = -1;

// The encodingMap is a hash of labels that are indexed by the conical
// encoding.
const encodingMap: { [key: string]: string[] } = {
  "windows-1252": [
    "ansi_x3.4-1968",
    "ascii",
    "cp1252",
    "cp819",
    "csisolatin1",
    "ibm819",
    "iso-8859-1",
    "iso-ir-100",
    "iso8859-1",
    "iso88591",
    "iso_8859-1",
    "iso_8859-1:1987",
    "l1",
    "latin1",
    "us-ascii",
    "windows-1252",
    "x-cp1252"
  ],
  "utf-8": ["unicode-1-1-utf-8", "utf-8", "utf8"]
};
// We convert these into a Map where every label resolves to its canonical
// encoding type.
const encodings = new Map<string, string>();
for (const key of Object.keys(encodingMap)) {
  const labels = encodingMap[key];
  for (const label of labels) {
    encodings.set(label, key);
  }
}

// A map of functions that return new instances of a decoder indexed by the
// encoding type.
const decoders = new Map<string, (options: DecoderOptions) => Decoder>();
decoders.set("utf-8", (options: DecoderOptions) => {
  return new UTF8Decoder(options);
});

// Single byte decoders are an array of code point lookups
const encodingIndexes = new Map<string, number[]>();
// tslint:disable:max-line-length
// prettier-ignore
encodingIndexes.set("windows-1252", [8364,129,8218,402,8222,8230,8224,8225,710,8240,352,8249,338,141,381,143,144,8216,8217,8220,8221,8226,8211,8212,732,8482,353,8250,339,157,382,376,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255]);
// tslint:enable
for (const [key, index] of encodingIndexes) {
  decoders.set(key, (options: DecoderOptions) => {
    return new SingleByteDecoder(index, options);
  });
}

function codePointsToString(codePoints: number[]): string {
  let s = "";
  for (const cp of codePoints) {
    s += String.fromCodePoint(cp);
  }
  return s;
}

function decoderError(fatal: boolean): number | never {
  if (fatal) {
    throw new TypeError("Decoder error.");
  }
  return 0xfffd; // default code point
}

function inRange(a: number, min: number, max: number) {
  return min <= a && a <= max;
}

function isASCIIByte(a: number) {
  return inRange(a, 0x00, 0x7f);
}

function stringToCodePoints(input: string): number[] {
  const u: number[] = [];
  for (const c of input) {
    u.push(c.codePointAt(0)!);
  }
  return u;
}

class Stream {
  private _tokens: number[];
  constructor(tokens: number[] | Uint8Array) {
    this._tokens = [].slice.call(tokens);
    this._tokens.reverse();
  }

  endOfStream(): boolean {
    return !this._tokens.length;
  }

  read(): number {
    return !this._tokens.length ? END_OF_STREAM : this._tokens.pop()!;
  }

  prepend(token: number | number[]): void {
    if (Array.isArray(token)) {
      while (token.length) {
        this._tokens.push(token.pop()!);
      }
    } else {
      this._tokens.push(token);
    }
  }

  push(token: number | number[]): void {
    if (Array.isArray(token)) {
      while (token.length) {
        this._tokens.unshift(token.shift()!);
      }
    } else {
      this._tokens.unshift(token);
    }
  }
}

class SingleByteDecoder implements Decoder {
  private _index: number[];
  private _fatal: boolean;

  constructor(index: number[], options: DecoderOptions) {
    this._fatal = options.fatal || false;
    this._index = index;
  }
  handler(stream: Stream, byte: number): number {
    if (byte === END_OF_STREAM) {
      return FINISHED;
    }
    if (isASCIIByte(byte)) {
      return byte;
    }
    const codePoint = this._index[byte - 0x80];

    if (codePoint == null) {
      return decoderError(this._fatal);
    }

    return codePoint;
  }
}

class UTF8Decoder implements Decoder {
  private _codePoint = 0;
  private _bytesSeen = 0;
  private _bytesNeeded = 0;
  private _fatal: boolean;
  private _lowerBoundary = 0x80;
  private _upperBoundary = 0xbf;

  constructor(options: DecoderOptions) {
    this._fatal = options.fatal || false;
  }

  handler(stream: Stream, byte: number): number | null {
    if (byte === END_OF_STREAM && this._bytesNeeded !== 0) {
      this._bytesNeeded = 0;
      return decoderError(this._fatal);
    }

    if (byte === END_OF_STREAM) {
      return FINISHED;
    }

    if (this._bytesNeeded === 0) {
      if (isASCIIByte(byte)) {
        // Single byte code point
        return byte;
      } else if (inRange(byte, 0xc2, 0xdf)) {
        // Two byte code point
        this._bytesNeeded = 1;
        this._codePoint = byte & 0x1f;
      } else if (inRange(byte, 0xe0, 0xef)) {
        // Three byte code point
        if (byte === 0xe0) {
          this._lowerBoundary = 0xa0;
        } else if (byte === 0xed) {
          this._upperBoundary = 0x9f;
        }
        this._bytesNeeded = 2;
        this._codePoint = byte & 0xf;
      } else if (inRange(byte, 0xf0, 0xf4)) {
        if (byte === 0xf0) {
          this._lowerBoundary = 0x90;
        } else if (byte === 0xf4) {
          this._upperBoundary = 0x8f;
        }
        this._bytesNeeded = 3;
        this._codePoint = byte & 0x7;
      } else {
        return decoderError(this._fatal);
      }
      return CONTINUE;
    }

    if (!inRange(byte, this._lowerBoundary, this._upperBoundary)) {
      // Byte out of range, so encoding error
      this._codePoint = 0;
      this._bytesNeeded = 0;
      this._bytesSeen = 0;
      stream.prepend(byte);
      return decoderError(this._fatal);
    }

    this._lowerBoundary = 0x80;
    this._upperBoundary = 0xbf;

    this._codePoint = (this._codePoint << 6) | (byte & 0x3f);

    this._bytesSeen++;

    if (this._bytesSeen !== this._bytesNeeded) {
      return CONTINUE;
    }

    const codePoint = this._codePoint;

    this._codePoint = 0;
    this._bytesNeeded = 0;
    this._bytesSeen = 0;

    return codePoint;
  }
}

class UTF8Encoder implements Encoder {
  handler(codePoint: number): number | number[] {
    if (codePoint === END_OF_STREAM) {
      return FINISHED;
    }

    if (inRange(codePoint, 0x00, 0x7f)) {
      return codePoint;
    }

    let count: number;
    let offset: number;
    if (inRange(codePoint, 0x0080, 0x07ff)) {
      count = 1;
      offset = 0xc0;
    } else if (inRange(codePoint, 0x0800, 0xffff)) {
      count = 2;
      offset = 0xe0;
    } else if (inRange(codePoint, 0x10000, 0x10ffff)) {
      count = 3;
      offset = 0xf0;
    } else {
      throw TypeError(`Code point out of range: \\x${codePoint.toString(16)}`);
    }

    const bytes = [(codePoint >> (6 * count)) + offset];

    while (count > 0) {
      const temp = codePoint >> (6 * (count - 1));
      bytes.push(0x80 | (temp & 0x3f));
      count--;
    }

    return bytes;
  }
}

export interface TextDecodeOptions {
  stream?: false;
}

export interface TextDecoderOptions {
  fatal?: boolean;
  ignoreBOM?: false;
}

export class TextDecoder {
  private _encoding: string;

  /** Returns encoding's name, lowercased. */
  get encoding(): string {
    return this._encoding;
  }
  /** Returns `true` if error mode is "fatal", and `false` otherwise. */
  readonly fatal: boolean = false;
  /** Returns `true` if ignore BOM flag is set, and `false` otherwise. */
  readonly ignoreBOM = false;

  constructor(label = "utf-8", options: TextDecoderOptions = { fatal: false }) {
    if (options.ignoreBOM) {
      throw new TypeError("Ignoring the BOM not supported.");
    }
    if (options.fatal) {
      this.fatal = true;
    }
    label = String(label)
      .trim()
      .toLowerCase();
    const encoding = encodings.get(label);
    if (!encoding) {
      throw new RangeError(
        `The encoding label provided ('${label}') is invalid.`
      );
    }
    if (!decoders.has(encoding)) {
      throw new TypeError(`Internal decoder ('${encoding}') not found.`);
    }
    this._encoding = encoding;
  }

  /** Returns the result of running encoding's decoder. */
  decode(
    input?: domTypes.BufferSource,
    options: TextDecodeOptions = { stream: false }
  ): string {
    if (options.stream) {
      throw new TypeError("Stream not supported.");
    }

    let bytes: Uint8Array;
    if (typeof input === "object" && input instanceof ArrayBuffer) {
      bytes = new Uint8Array(input);
    } else if (
      typeof input === "object" &&
      "buffer" in input &&
      input.buffer instanceof ArrayBuffer
    ) {
      bytes = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    } else {
      bytes = new Uint8Array(0);
    }

    const decoder = decoders.get(this._encoding)!({ fatal: this.fatal });
    const inputStream = new Stream(bytes);
    const output: number[] = [];

    while (true) {
      const result = decoder.handler(inputStream, inputStream.read());
      if (result === FINISHED) {
        break;
      }

      if (result !== CONTINUE) {
        output.push(result);
      }
    }

    if (output.length > 0 && output[0] === 0xfeff) {
      output.shift();
    }

    return codePointsToString(output);
  }
}

export class TextEncoder {
  /** Returns "utf-8". */
  readonly encoding = "utf-8";
  /** Returns the result of running UTF-8's encoder. */
  encode(input = ""): Uint8Array {
    const encoder = new UTF8Encoder();
    const inputStream = new Stream(stringToCodePoints(input));
    const output: number[] = [];

    while (true) {
      const result = encoder.handler(inputStream.read());
      if (result === FINISHED) {
        break;
      }
      if (Array.isArray(result)) {
        output.push.apply(output, result);
      } else {
        output.push(result);
      }
    }

    return new Uint8Array(output);
  }
}
