// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// The following code is based off:
// https://github.com/inexorabletash/text-encoding
//
// Copyright (c) 2008-2009 Bjoern Hoehrmann <bjoern@hoehrmann.de>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

// `.apply` can actually take a typed array, though the type system doesn't
// really support it, so we have to "hack" it a bit to get past some of the
// strict type checks.
declare global {
  interface CallableFunction extends Function {
    apply<T, R>(
      this: (this: T, ...args: number[]) => R,
      thisArg: T,
      args: Uint16Array
    ): R;
  }
}

export function decodeUtf8(
  input: Uint8Array,
  fatal: boolean,
  ignoreBOM: boolean
): string {
  let outString = "";

  // Prepare a buffer so that we don't have to do a lot of string concats, which
  // are very slow.
  const outBufferLength: number = Math.min(1024, input.length);
  const outBuffer = new Uint16Array(outBufferLength);
  let outIndex = 0;

  let state = 0;
  let codepoint = 0;
  let type: number;

  let i =
    ignoreBOM && input[0] === 0xef && input[1] === 0xbb && input[2] === 0xbf
      ? 3
      : 0;

  for (; i < input.length; ++i) {
    // Encoding error handling
    if (state === 12 || (state !== 0 && (input[i] & 0xc0) !== 0x80)) {
      if (fatal)
        throw new TypeError(
          `Decoder error. Invalid byte in sequence at position ${i} in data.`
        );
      outBuffer[outIndex++] = 0xfffd; // Replacement character
      if (outIndex === outBufferLength) {
        outString += String.fromCharCode.apply(null, outBuffer);
        outIndex = 0;
      }
      state = 0;
    }

    // prettier-ignore
    type = [
       0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
       0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
       0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
       0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
       1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,  9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,
       7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,  7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
       8,8,2,2,2,2,2,2,2,2,2,2,2,2,2,2,  2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
      10,3,3,3,3,3,3,3,3,3,3,3,3,4,3,3, 11,6,6,6,5,8,8,8,8,8,8,8,8,8,8,8
    ][input[i]];
    codepoint =
      state !== 0
        ? (input[i] & 0x3f) | (codepoint << 6)
        : (0xff >> type) & input[i];
    // prettier-ignore
    state = [
       0,12,24,36,60,96,84,12,12,12,48,72, 12,12,12,12,12,12,12,12,12,12,12,12,
      12, 0,12,12,12,12,12, 0,12, 0,12,12, 12,24,12,12,12,12,12,24,12,24,12,12,
      12,12,12,12,12,12,12,24,12,12,12,12, 12,24,12,12,12,12,12,12,12,24,12,12,
      12,12,12,12,12,12,12,36,12,36,12,12, 12,36,12,12,12,12,12,36,12,36,12,12,
      12,36,12,12,12,12,12,12,12,12,12,12
    ][state + type];

    if (state !== 0) continue;

    // Add codepoint to buffer (as charcodes for utf-16), and flush buffer to
    // string if needed.
    if (codepoint > 0xffff) {
      outBuffer[outIndex++] = 0xd7c0 + (codepoint >> 10);
      if (outIndex === outBufferLength) {
        outString += String.fromCharCode.apply(null, outBuffer);
        outIndex = 0;
      }
      outBuffer[outIndex++] = 0xdc00 | (codepoint & 0x3ff);
      if (outIndex === outBufferLength) {
        outString += String.fromCharCode.apply(null, outBuffer);
        outIndex = 0;
      }
    } else {
      outBuffer[outIndex++] = codepoint;
      if (outIndex === outBufferLength) {
        outString += String.fromCharCode.apply(null, outBuffer);
        outIndex = 0;
      }
    }
  }

  // Add a replacement character if we ended in the middle of a sequence or
  // encountered an invalid code at the end.
  if (state !== 0) {
    if (fatal) throw new TypeError(`Decoder error. Unexpected end of data.`);
    outBuffer[outIndex++] = 0xfffd; // Replacement character
  }

  // Final flush of buffer
  outString += String.fromCharCode.apply(null, outBuffer.subarray(0, outIndex));

  return outString;
}
