// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  forgivingBase64Decode,
  forgivingBase64UrlEncode,
} from "ext:deno_web/00_infra.js";

export function asciiToBytes(str: string) {
  const byteArray = [];
  for (let i = 0; i < str.length; ++i) {
    byteArray.push(str.charCodeAt(i) & 255);
  }
  return new Uint8Array(byteArray);
}

export function base64ToBytes(str: string) {
  str = base64clean(str);
  str = str.replaceAll("-", "+").replaceAll("_", "/");
  return forgivingBase64Decode(str);
}

const INVALID_BASE64_RE = /[^+/0-9A-Za-z-_]/g;
function base64clean(str: string) {
  // Node takes equal signs as end of the Base64 encoding
  str = str.split("=")[0];
  // Node strips out invalid characters like \n and \t from the string, std/base64 does not
  str = str.trim().replace(INVALID_BASE64_RE, "");
  // Node converts strings with length < 2 to ''
  if (str.length < 2) return "";
  // Node allows for non-padded base64 strings (missing trailing ===), std/base64 does not
  while (str.length % 4 !== 0) {
    str = str + "=";
  }
  return str;
}

export function base64UrlToBytes(str: string) {
  str = base64clean(str);
  str = str.replaceAll("+", "-").replaceAll("/", "_");
  return forgivingBase64UrlEncode(str);
}

export function hexToBytes(str: string) {
  const byteArray = new Uint8Array(Math.floor((str || "").length / 2));
  let i;
  for (i = 0; i < byteArray.length; i++) {
    const a = Number.parseInt(str[i * 2], 16);
    const b = Number.parseInt(str[i * 2 + 1], 16);
    if (Number.isNaN(a) && Number.isNaN(b)) {
      break;
    }
    byteArray[i] = (a << 4) | b;
  }
  return new Uint8Array(
    i === byteArray.length ? byteArray : byteArray.slice(0, i),
  );
}

export function utf16leToBytes(str: string, units: number) {
  let c, hi, lo;
  const byteArray = [];
  for (let i = 0; i < str.length; ++i) {
    if ((units -= 2) < 0) {
      break;
    }
    c = str.charCodeAt(i);
    hi = c >> 8;
    lo = c % 256;
    byteArray.push(lo);
    byteArray.push(hi);
  }
  return new Uint8Array(byteArray);
}

export function bytesToAscii(bytes: Uint8Array) {
  let ret = "";
  for (let i = 0; i < bytes.length; ++i) {
    ret += String.fromCharCode(bytes[i] & 127);
  }
  return ret;
}

export function bytesToUtf16le(bytes: Uint8Array) {
  let res = "";
  for (let i = 0; i < bytes.length - 1; i += 2) {
    res += String.fromCharCode(bytes[i] + bytes[i + 1] * 256);
  }
  return res;
}
