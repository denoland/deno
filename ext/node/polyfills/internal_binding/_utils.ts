// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  forgivingBase64Decode,
  forgivingBase64UrlDecode,
} from "ext:deno_web/00_infra.js";

export function asciiToBytes(str: string) {
  const length = str.length;
  const byteArray = new Uint8Array(length);
  for (let i = 0; i < length; ++i) {
    byteArray[i] = str.charCodeAt(i) & 255;
  }
  return byteArray;
}

export function base64ToBytes(str: string) {
  try {
    return forgivingBase64Decode(str);
  } catch {
    str = base64clean(str);
    str = str.replaceAll("-", "+").replaceAll("_", "/");
    return forgivingBase64Decode(str);
  }
}

const INVALID_BASE64_RE = /[^+/0-9A-Za-z-_]/g;
function base64clean(str: string) {
  // Node takes equal signs as end of the Base64 encoding
  const eqIndex = str.indexOf("=");
  str = eqIndex !== -1 ? str.substring(0, eqIndex).trimStart() : str.trim();
  // Node strips out invalid characters like \n and \t from the string, std/base64 does not
  str = str.replace(INVALID_BASE64_RE, "");
  // Node converts strings with length < 2 to ''
  const length = str.length;
  if (length < 2) return "";
  // Node allows for non-padded base64 strings (missing trailing ===), std/base64 does not
  switch (length % 4) {
    case 0:
      return str;
    case 1:
      return `${str}===`;
    case 2:
      return `${str}==`;
    case 3:
      return `${str}=`;
    default:
      throw new Error("Unexpected NaN value for string length");
  }
}

export function base64UrlToBytes(str: string) {
  str = base64clean(str);
  str = str.replaceAll("+", "-").replaceAll("/", "_");
  return forgivingBase64UrlDecode(str);
}

// https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/deps/nbytes/src/nbytes.cpp#L144-L158
// deno-fmt-ignore
export const unhexTable = new Int8Array([
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 0 - 15
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 16 - 31
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 32 - 47
   0,  1,  2,  3,  4,  5,  6,  7,  8,  9, -1, -1, -1, -1, -1, -1, // 48 - 63
  -1, 10, 11, 12, 13, 14, 15, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 64 - 79
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 80 - 95
  -1, 10, 11, 12, 13, 14, 15, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 96 - 111
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 112 - 127
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 128 ...
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // ... 255
]);

export function hexToBytes(str: string) {
  const length = str.length >>> 1;
  const byteArray = new Uint8Array(length);
  let i: number;
  for (i = 0; i < length; i++) {
    const a = unhexTable[str.charCodeAt(i * 2) & 0xff];
    const b = unhexTable[str.charCodeAt(i * 2 + 1) & 0xff];
    if (!~a || !~b) {
      break;
    }
    byteArray[i] = (a << 4) | b;
  }
  // Returning a buffer subarray is okay: This API's return value
  // is never exposed to users and is only ever used for its length
  // and the data within the subarray.
  return i === length ? byteArray : byteArray.subarray(0, i);
}

export function utf16leToBytes(str: string, units?: number) {
  // If units is defined, round it to even values for 16 byte "steps"
  // and use it as an upper bound value for our string byte array's length.
  const length = Math.min(str.length * 2, units ? (units >>> 1) * 2 : Infinity);
  const byteArray = new Uint8Array(length);
  const view = new DataView(byteArray.buffer);
  let i: number;
  for (i = 0; i * 2 < length; i++) {
    view.setUint16(i * 2, str.charCodeAt(i), true);
  }
  // Returning a buffer subarray is okay: This API's return value
  // is never exposed to users and is only ever used for its length
  // and the data within the subarray.
  return i * 2 === length ? byteArray : byteArray.subarray(0, i * 2);
}

export function bytesToAscii(bytes: Uint8Array) {
  let res = "";
  const length = bytes.byteLength;
  for (let i = 0; i < length; ++i) {
    res = `${res}${String.fromCharCode(bytes[i] & 127)}`;
  }
  return res;
}

export function bytesToUtf16le(bytes: Uint8Array) {
  let res = "";
  const length = bytes.byteLength;
  const view = new DataView(bytes.buffer, bytes.byteOffset, length);
  for (let i = 0; i < length - 1; i += 2) {
    res = `${res}${String.fromCharCode(view.getUint16(i, true))}`;
  }
  return res;
}
