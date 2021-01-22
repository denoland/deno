// Modified from https://github.com/beatgammit/base64-js
// Copyright (c) 2014 Jameson Little. MIT License.

const lookup: string[] = [];
const revLookup: number[] = [];

// RFC4648 base32
const code = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
for (let i = 0, len = code.length; i < len; ++i) {
  lookup[i] = code[i];
  revLookup[code.charCodeAt(i)] = i;
}

const placeHolderPadLookup = [0, 1, , 2, 3, , 4];
function _getPadLen(placeHoldersLen: number): number {
  const maybeLen = placeHolderPadLookup[placeHoldersLen];
  if (typeof maybeLen !== "number") {
    throw new Error("Invalid pad length");
  }
  return maybeLen;
}

function getLens(b32: string): [number, number] {
  const len = b32.length;

  if (len % 8 > 0) {
    throw new Error("Invalid string. Length must be a multiple of 8");
  }

  let validLen = b32.indexOf("=");
  if (validLen === -1) validLen = len;

  const placeHoldersLen = validLen === len ? 0 : 8 - (validLen % 8);

  return [validLen, placeHoldersLen];
}

function _byteLength(validLen: number, placeHoldersLen: number): number {
  return ((validLen + placeHoldersLen) * 5) / 8 - _getPadLen(placeHoldersLen);
}

/**
 * Decodes a given RFC4648 base32 encoded string.
 * @param b32
 */
export function decode(b32: string): Uint8Array {
  let tmp: number;
  const [validLen, placeHoldersLen] = getLens(b32);

  const arr = new Uint8Array(_byteLength(validLen, placeHoldersLen));

  let curByte = 0;

  // if there are placeholders, only get up to the last complete 8 chars
  const len = placeHoldersLen > 0 ? validLen - 8 : validLen;

  let i: number;
  for (i = 0; i < len; i += 8) {
    tmp = (revLookup[b32.charCodeAt(i)] << 20) |
      (revLookup[b32.charCodeAt(i + 1)] << 15) |
      (revLookup[b32.charCodeAt(i + 2)] << 10) |
      (revLookup[b32.charCodeAt(i + 3)] << 5) |
      revLookup[b32.charCodeAt(i + 4)];
    arr[curByte++] = (tmp >> 17) & 0xff;
    arr[curByte++] = (tmp >> 9) & 0xff;
    arr[curByte++] = (tmp >> 1) & 0xff;

    tmp = ((tmp & 1) << 15) |
      (revLookup[b32.charCodeAt(i + 5)] << 10) |
      (revLookup[b32.charCodeAt(i + 6)] << 5) |
      revLookup[b32.charCodeAt(i + 7)];
    arr[curByte++] = (tmp >> 8) & 0xff;
    arr[curByte++] = tmp & 0xff;
  }

  if (placeHoldersLen === 1) {
    tmp = (revLookup[b32.charCodeAt(i)] << 20) |
      (revLookup[b32.charCodeAt(i + 1)] << 15) |
      (revLookup[b32.charCodeAt(i + 2)] << 10) |
      (revLookup[b32.charCodeAt(i + 3)] << 5) |
      revLookup[b32.charCodeAt(i + 4)];
    arr[curByte++] = (tmp >> 17) & 0xff;
    arr[curByte++] = (tmp >> 9) & 0xff;
    arr[curByte++] = (tmp >> 1) & 0xff;
    tmp = ((tmp & 1) << 7) |
      (revLookup[b32.charCodeAt(i + 5)] << 2) |
      (revLookup[b32.charCodeAt(i + 6)] >> 3);
    arr[curByte++] = tmp & 0xff;
  } else if (placeHoldersLen === 3) {
    tmp = (revLookup[b32.charCodeAt(i)] << 19) |
      (revLookup[b32.charCodeAt(i + 1)] << 14) |
      (revLookup[b32.charCodeAt(i + 2)] << 9) |
      (revLookup[b32.charCodeAt(i + 3)] << 4) |
      (revLookup[b32.charCodeAt(i + 4)] >> 1);
    arr[curByte++] = (tmp >> 16) & 0xff;
    arr[curByte++] = (tmp >> 8) & 0xff;
    arr[curByte++] = tmp & 0xff;
  } else if (placeHoldersLen === 4) {
    tmp = (revLookup[b32.charCodeAt(i)] << 11) |
      (revLookup[b32.charCodeAt(i + 1)] << 6) |
      (revLookup[b32.charCodeAt(i + 2)] << 1) |
      (revLookup[b32.charCodeAt(i + 3)] >> 4);
    arr[curByte++] = (tmp >> 8) & 0xff;
    arr[curByte++] = tmp & 0xff;
  } else if (placeHoldersLen === 6) {
    tmp = (revLookup[b32.charCodeAt(i)] << 3) |
      (revLookup[b32.charCodeAt(i + 1)] >> 2);
    arr[curByte++] = tmp & 0xff;
  }

  return arr;
}

function encodeChunk(uint8: Uint8Array, start: number, end: number): string {
  let tmp: number;
  const output = [];
  for (let i = start; i < end; i += 5) {
    tmp = ((uint8[i] << 16) & 0xff0000) |
      ((uint8[i + 1] << 8) & 0xff00) |
      (uint8[i + 2] & 0xff);
    output.push(lookup[(tmp >> 19) & 0x1f]);
    output.push(lookup[(tmp >> 14) & 0x1f]);
    output.push(lookup[(tmp >> 9) & 0x1f]);
    output.push(lookup[(tmp >> 4) & 0x1f]);
    tmp = ((tmp & 0xf) << 16) |
      ((uint8[i + 3] << 8) & 0xff00) |
      (uint8[i + 4] & 0xff);
    output.push(lookup[(tmp >> 15) & 0x1f]);
    output.push(lookup[(tmp >> 10) & 0x1f]);
    output.push(lookup[(tmp >> 5) & 0x1f]);
    output.push(lookup[tmp & 0x1f]);
  }
  return output.join("");
}

/**
 * Encodes a given Uint8Array into RFC4648 base32 representation
 * @param uint8
 */
export function encode(uint8: Uint8Array): string {
  let tmp: number;
  const len = uint8.length;
  const extraBytes = len % 5;
  const parts = [];
  const maxChunkLength = 16385; // must be multiple of 5
  const len2 = len - extraBytes;

  // go through the array every 5 bytes, we'll deal with trailing stuff later
  for (let i = 0; i < len2; i += maxChunkLength) {
    parts.push(
      encodeChunk(
        uint8,
        i,
        i + maxChunkLength > len2 ? len2 : i + maxChunkLength,
      ),
    );
  }

  // pad the end with zeros, but make sure to not forget the extra bytes
  if (extraBytes === 4) {
    tmp = ((uint8[len2] & 0xff) << 16) |
      ((uint8[len2 + 1] & 0xff) << 8) |
      (uint8[len2 + 2] & 0xff);
    parts.push(lookup[(tmp >> 19) & 0x1f]);
    parts.push(lookup[(tmp >> 14) & 0x1f]);
    parts.push(lookup[(tmp >> 9) & 0x1f]);
    parts.push(lookup[(tmp >> 4) & 0x1f]);
    tmp = ((tmp & 0xf) << 11) | (uint8[len2 + 3] << 3);
    parts.push(lookup[(tmp >> 10) & 0x1f]);
    parts.push(lookup[(tmp >> 5) & 0x1f]);
    parts.push(lookup[tmp & 0x1f]);
    parts.push("=");
  } else if (extraBytes === 3) {
    tmp = ((uint8[len2] & 0xff) << 17) |
      ((uint8[len2 + 1] & 0xff) << 9) |
      ((uint8[len2 + 2] & 0xff) << 1);
    parts.push(lookup[(tmp >> 20) & 0x1f]);
    parts.push(lookup[(tmp >> 15) & 0x1f]);
    parts.push(lookup[(tmp >> 10) & 0x1f]);
    parts.push(lookup[(tmp >> 5) & 0x1f]);
    parts.push(lookup[tmp & 0x1f]);
    parts.push("===");
  } else if (extraBytes === 2) {
    tmp = ((uint8[len2] & 0xff) << 12) | ((uint8[len2 + 1] & 0xff) << 4);
    parts.push(lookup[(tmp >> 15) & 0x1f]);
    parts.push(lookup[(tmp >> 10) & 0x1f]);
    parts.push(lookup[(tmp >> 5) & 0x1f]);
    parts.push(lookup[tmp & 0x1f]);
    parts.push("====");
  } else if (extraBytes === 1) {
    tmp = (uint8[len2] & 0xff) << 2;
    parts.push(lookup[(tmp >> 5) & 0x1f]);
    parts.push(lookup[tmp & 0x1f]);
    parts.push("======");
  }

  return parts.join("");
}
