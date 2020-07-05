// Ported from js-yaml v3.13.1:
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { Type } from "../type.ts";
import { Any } from "../utils.ts";

// [ 64, 65, 66 ] -> [ padding, CR, LF ]
const BASE64_MAP =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=\n\r";

function resolveYamlBinary(data: Any): boolean {
  if (data === null) return false;

  let code: number;
  let bitlen = 0;
  const max = data.length;
  const map = BASE64_MAP;

  // Convert one by one.
  for (let idx = 0; idx < max; idx++) {
    code = map.indexOf(data.charAt(idx));

    // Skip CR/LF
    if (code > 64) continue;

    // Fail on illegal characters
    if (code < 0) return false;

    bitlen += 6;
  }

  // If there are any bits left, source was corrupted
  return bitlen % 8 === 0;
}

function constructYamlBinary(data: string): Deno.Buffer {
  // remove CR/LF & padding to simplify scan
  const input = data.replace(/[\r\n=]/g, "");
  const max = input.length;
  const map = BASE64_MAP;

  // Collect by 6*4 bits (3 bytes)

  const result = [];
  let bits = 0;
  for (let idx = 0; idx < max; idx++) {
    if (idx % 4 === 0 && idx) {
      result.push((bits >> 16) & 0xff);
      result.push((bits >> 8) & 0xff);
      result.push(bits & 0xff);
    }

    bits = (bits << 6) | map.indexOf(input.charAt(idx));
  }

  // Dump tail

  const tailbits = (max % 4) * 6;

  if (tailbits === 0) {
    result.push((bits >> 16) & 0xff);
    result.push((bits >> 8) & 0xff);
    result.push(bits & 0xff);
  } else if (tailbits === 18) {
    result.push((bits >> 10) & 0xff);
    result.push((bits >> 2) & 0xff);
  } else if (tailbits === 12) {
    result.push((bits >> 4) & 0xff);
  }

  return new Deno.Buffer(new Uint8Array(result));
}

function representYamlBinary(object: Uint8Array): string {
  const max = object.length;
  const map = BASE64_MAP;

  // Convert every three bytes to 4 ASCII characters.

  let result = "";
  let bits = 0;
  for (let idx = 0; idx < max; idx++) {
    if (idx % 3 === 0 && idx) {
      result += map[(bits >> 18) & 0x3f];
      result += map[(bits >> 12) & 0x3f];
      result += map[(bits >> 6) & 0x3f];
      result += map[bits & 0x3f];
    }

    bits = (bits << 8) + object[idx];
  }

  // Dump tail

  const tail = max % 3;

  if (tail === 0) {
    result += map[(bits >> 18) & 0x3f];
    result += map[(bits >> 12) & 0x3f];
    result += map[(bits >> 6) & 0x3f];
    result += map[bits & 0x3f];
  } else if (tail === 2) {
    result += map[(bits >> 10) & 0x3f];
    result += map[(bits >> 4) & 0x3f];
    result += map[(bits << 2) & 0x3f];
    result += map[64];
  } else if (tail === 1) {
    result += map[(bits >> 2) & 0x3f];
    result += map[(bits << 4) & 0x3f];
    result += map[64];
    result += map[64];
  }

  return result;
}

function isBinary(obj: Any): obj is Deno.Buffer {
  const buf = new Deno.Buffer();
  try {
    if (0 > buf.readFromSync(obj as Deno.Buffer)) return true;
    return false;
  } catch {
    return false;
  } finally {
    buf.reset();
  }
}

export const binary = new Type("tag:yaml.org,2002:binary", {
  construct: constructYamlBinary,
  kind: "scalar",
  predicate: isBinary,
  represent: representYamlBinary,
  resolve: resolveYamlBinary,
});
