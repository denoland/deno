// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { isIterator, isToken, needsEncoding } from "./_util.ts";

/** Serializes the media type and the optional parameters as a media type
 * conforming to RFC 2045 and RFC 2616.
 *
 * The type and parameter names are written in lower-case.
 *
 * When any of the arguments results in a standard violation then the return
 * value will be an empty string (`""`).
 *
 * @example
 * ```ts
 * import { formatMediaType } from "https://deno.land/std@$STD_VERSION/media_types/format_media_type.ts";
 *
 * formatMediaType("text/plain", { charset: "UTF-8" }); // `text/plain; charset=UTF-8`
 * ```
 */
export function formatMediaType(
  type: string,
  param?: Record<string, string> | Iterable<[string, string]>,
): string {
  let b = "";
  const [major, sub] = type.split("/");
  if (!sub) {
    if (!isToken(type)) {
      return "";
    }
    b += type.toLowerCase();
  } else {
    if (!isToken(major) || !isToken(sub)) {
      return "";
    }
    b += `${major.toLowerCase()}/${sub.toLowerCase()}`;
  }

  if (param) {
    param = isIterator(param) ? Object.fromEntries(param) : param;
    const attrs = Object.keys(param);
    attrs.sort();

    for (const attribute of attrs) {
      if (!isToken(attribute)) {
        return "";
      }
      const value = param[attribute];
      b += `; ${attribute.toLowerCase()}`;

      const needEnc = needsEncoding(value);
      if (needEnc) {
        b += "*";
      }
      b += "=";

      if (needEnc) {
        b += `utf-8''${encodeURIComponent(value)}`;
        continue;
      }

      if (isToken(value)) {
        b += value;
        continue;
      }
      b += `"${value.replace(/["\\]/gi, (m) => `\\${m}`)}"`;
    }
  }
  return b;
}
