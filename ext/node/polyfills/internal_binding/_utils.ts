// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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
  str = base64clean(str);
  str = str.replaceAll("-", "+").replaceAll("_", "/");
  return forgivingBase64Decode(str);
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

export function hexToBytes(str: string) {
  const length = str.length >>> 1;
  const byteArray = new Uint8Array(length);
  let i: number;
  for (i = 0; i < length; i++) {
    const a = Number.parseInt(str[i * 2], 16);
    const b = Number.parseInt(str[i * 2 + 1], 16);
    if (Number.isNaN(a) && Number.isNaN(b)) {
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

export function utf8ToBytes(str: string, units?: number) {
  units = units || Infinity;
  // The byte array length is determined by a conservative 'length quadrupled' calculation.
  // This overallocates massively but will never fail.
  const strLength = str.length;
  const byteArrayLength = Math.min(strLength * 4, units * 4);
  const byteArray = new Uint8Array(byteArrayLength);
  let codePoint: number;
  let leadSurrogate: null | number = null;
  /**
   * Next index to assign into.
   */
  let byteIndex = 0;
  for (let i = 0; i < strLength; ++i) {
    codePoint = str.charCodeAt(i);
    if (codePoint > 55295 && codePoint < 57344) {
      if (!leadSurrogate) {
        if (codePoint > 56319) {
          if ((units -= 3) > -1) {
            byteArray[byteIndex++] = 239;
            byteArray[byteIndex++] = 191;
            byteArray[byteIndex++] = 189;
          }
          continue;
        } else if (i + 1 === strLength) {
          if ((units -= 3) > -1) {
            byteArray[byteIndex++] = 239;
            byteArray[byteIndex++] = 191;
            byteArray[byteIndex++] = 189;
          }
          continue;
        }
        leadSurrogate = codePoint;
        continue;
      }
      if (codePoint < 56320) {
        if ((units -= 3) > -1) {
          byteArray[byteIndex++] = 239;
          byteArray[byteIndex++] = 191;
          byteArray[byteIndex++] = 189;
        }
        leadSurrogate = codePoint;
        continue;
      }
      codePoint = (leadSurrogate - 55296 << 10 | codePoint - 56320) + 65536;
    } else if (leadSurrogate) {
      if ((units -= 3) > -1) {
        byteArray[byteIndex++] = 239;
        byteArray[byteIndex++] = 191;
        byteArray[byteIndex++] = 189;
      }
    }
    leadSurrogate = null;
    if (codePoint < 128) {
      if ((units -= 1) < 0) {
        break;
      }
      byteArray[byteIndex++] = codePoint;
    } else if (codePoint < 2048) {
      if ((units -= 2) < 0) {
        break;
      }
      byteArray[byteIndex++] = codePoint >> 6 | 192;
      byteArray[byteIndex++] = codePoint & 63 | 128;
    } else if (codePoint < 65536) {
      if ((units -= 3) < 0) {
        break;
      }
      byteArray[byteIndex++] = codePoint >> 12 | 224;
      byteArray[byteIndex++] = codePoint >> 6 & 63 | 128;
      byteArray[byteIndex++] = codePoint & 63 | 128;
    } else if (codePoint < 1114112) {
      if ((units -= 4) < 0) {
        break;
      }
      byteArray[byteIndex++] = codePoint >> 18 | 240;
      byteArray[byteIndex++] = codePoint >> 12 & 63 | 128;
      byteArray[byteIndex++] = codePoint >> 6 & 63 | 128;
      byteArray[byteIndex++] = codePoint & 63 | 128;
    } else {
      throw new Error("Invalid code point");
    }
  }
  // If the next byte index (to assign into) is equal to length, ie.
  // one past the last byte of the byte array, then we've filled the buffer.
  // Otherwise: Returning a buffer subarray is okay: This API's return value
  // is never exposed to users and is only ever used for its length
  // and the data within the subarray.
  return byteIndex === byteArrayLength
    ? byteArray
    : byteArray.subarray(0, byteIndex);
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
