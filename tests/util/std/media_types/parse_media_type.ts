// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { consumeMediaParam, decode2331Encoding } from "./_util.ts";

/**
 * Parses the media type and any optional parameters, per
 * [RFC 1521](https://datatracker.ietf.org/doc/html/rfc1521). Media types are
 * the values in `Content-Type` and `Content-Disposition` headers. On success
 * the function returns a tuple where the first element is the media type and
 * the second element is the optional parameters or `undefined` if there are
 * none.
 *
 * The function will throw if the parsed value is invalid.
 *
 * The returned media type will be normalized to be lower case, and returned
 * params keys will be normalized to lower case, but preserves the casing of
 * the value.
 *
 * @example
 * ```ts
 * import { parseMediaType } from "https://deno.land/std@$STD_VERSION/media_types/parse_media_type.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * assertEquals(
 *   parseMediaType("application/JSON"),
 *   [
 *     "application/json",
 *     undefined
 *   ]
 * );
 *
 * assertEquals(
 *   parseMediaType("text/html; charset=UTF-8"),
 *   [
 *     "text/html",
 *     { charset: "UTF-8" },
 *   ]
 * );
 * ```
 */
export function parseMediaType(
  v: string,
): [mediaType: string, params: Record<string, string> | undefined] {
  const [base] = v.split(";");
  const mediaType = base.toLowerCase().trim();

  const params: Record<string, string> = {};
  // Map of base parameter name -> parameter name -> value
  // for parameters containing a '*' character.
  const continuation = new Map<string, Record<string, string>>();

  v = v.slice(base.length);
  while (v.length) {
    v = v.trimStart();
    if (v.length === 0) {
      break;
    }
    const [key, value, rest] = consumeMediaParam(v);
    if (!key) {
      if (rest.trim() === ";") {
        // ignore trailing semicolons
        break;
      }
      throw new TypeError("Invalid media parameter.");
    }

    let pmap = params;
    const [baseName, rest2] = key.split("*");
    if (baseName && rest2 !== undefined) {
      if (!continuation.has(baseName)) {
        continuation.set(baseName, {});
      }
      pmap = continuation.get(baseName)!;
    }
    if (key in pmap) {
      throw new TypeError("Duplicate key parsed.");
    }
    pmap[key] = value;
    v = rest;
  }

  // Stitch together any continuations or things with stars
  // (i.e. RFC 2231 things with stars: "foo*0" or "foo*")
  let str = "";
  for (const [key, pieceMap] of continuation) {
    const singlePartKey = `${key}*`;
    const v = pieceMap[singlePartKey];
    if (v) {
      const decv = decode2331Encoding(v);
      if (decv) {
        params[key] = decv;
      }
      continue;
    }

    str = "";
    let valid = false;
    for (let n = 0;; n++) {
      const simplePart = `${key}*${n}`;
      let v = pieceMap[simplePart];
      if (v) {
        valid = true;
        str += v;
        continue;
      }
      const encodedPart = `${simplePart}*`;
      v = pieceMap[encodedPart];
      if (!v) {
        break;
      }
      valid = true;
      if (n === 0) {
        const decv = decode2331Encoding(v);
        if (decv) {
          str += decv;
        }
      } else {
        const decv = decodeURI(v);
        str += decv;
      }
    }
    if (valid) {
      params[key] = str;
    }
  }

  return Object.keys(params).length
    ? [mediaType, params]
    : [mediaType, undefined];
}
