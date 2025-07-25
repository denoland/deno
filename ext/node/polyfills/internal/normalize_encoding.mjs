// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const { StringPrototypeToLowerCase } = primordials;

export function normalizeEncoding(enc) {
  if (enc == null || enc === "utf8" || enc === "utf-8") return "utf8";
  return slowCases(enc);
}

export function slowCases(enc) {
  switch (enc.length) {
    case 4:
      if (enc === "UTF8") return "utf8";
      if (enc === "ucs2" || enc === "UCS2") return "utf16le";
      enc = StringPrototypeToLowerCase(`${enc}`);
      if (enc === "utf8") return "utf8";
      if (enc === "ucs2") return "utf16le";
      break;
    case 3:
      if (
        enc === "hex" || enc === "HEX" ||
        StringPrototypeToLowerCase(`${enc}`) === "hex"
      ) {
        return "hex";
      }
      break;
    case 5:
      if (enc === "ascii") return "ascii";
      if (enc === "ucs-2") return "utf16le";
      if (enc === "UTF-8") return "utf8";
      if (enc === "ASCII") return "ascii";
      if (enc === "UCS-2") return "utf16le";
      enc = StringPrototypeToLowerCase(`${enc}`);
      if (enc === "utf-8") return "utf8";
      if (enc === "ascii") return "ascii";
      if (enc === "ucs-2") return "utf16le";
      break;
    case 6:
      if (enc === "base64") return "base64";
      if (enc === "latin1" || enc === "binary") return "latin1";
      if (enc === "BASE64") return "base64";
      if (enc === "LATIN1" || enc === "BINARY") return "latin1";
      enc = StringPrototypeToLowerCase(`${enc}`);
      if (enc === "base64") return "base64";
      if (enc === "latin1" || enc === "binary") return "latin1";
      break;
    case 7:
      if (
        enc === "utf16le" || enc === "UTF16LE" ||
        StringPrototypeToLowerCase(`${enc}`) === "utf16le"
      ) {
        return "utf16le";
      }
      break;
    case 8:
      if (
        enc === "utf-16le" || enc === "UTF-16LE" ||
        StringPrototypeToLowerCase(`${enc}`) === "utf-16le"
      ) {
        return "utf16le";
      }
      break;
    case 9:
      if (
        enc === "base64url" || enc === "BASE64URL" ||
        StringPrototypeToLowerCase(`${enc}`) === "base64url"
      ) {
        return "base64url";
      }
      break;
    default:
      if (enc === "") return "utf8";
  }
}
