// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../../deno_core/core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeIncludes,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  RegExpPrototypeTest,
  RegExpPrototypeExec,
  SafeMap,
  SafeMapIterator,
  StringPrototypeReplaceAll,
  StringPrototypeToLowerCase,
  StringPrototypeEndsWith,
  Uint8Array,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeIncludes,
} = primordials;

import {
  assert,
  collectHttpQuotedString,
  collectSequenceOfCodepoints,
  HTTP_QUOTED_STRING_TOKEN_POINT_RE,
  HTTP_TOKEN_CODE_POINT_RE,
  HTTP_WHITESPACE,
  HTTP_WHITESPACE_PREFIX_RE,
  HTTP_WHITESPACE_SUFFIX_RE,
} from "./00_infra.js";

/**
 * @typedef MimeType
 * @property {string} type
 * @property {string} subtype
 * @property {Map<string,string>} parameters
 */

/**
 * @param {string} input
 * @returns {MimeType | null}
 */
function parseMimeType(input) {
  // 1.
  input = StringPrototypeReplaceAll(input, HTTP_WHITESPACE_PREFIX_RE, "");
  input = StringPrototypeReplaceAll(input, HTTP_WHITESPACE_SUFFIX_RE, "");

  // 2.
  let position = 0;
  const endOfInput = input.length;

  // 3.
  const res1 = collectSequenceOfCodepoints(
    input,
    position,
    (c) => c != "\u002F",
  );
  const type = res1.result;
  position = res1.position;

  // 4.
  if (type === "" || !RegExpPrototypeTest(HTTP_TOKEN_CODE_POINT_RE, type)) {
    return null;
  }

  // 5.
  if (position >= endOfInput) return null;

  // 6.
  position++;

  // 7.
  const res2 = collectSequenceOfCodepoints(
    input,
    position,
    (c) => c != "\u003B",
  );
  let subtype = res2.result;
  position = res2.position;

  // 8.
  subtype = StringPrototypeReplaceAll(subtype, HTTP_WHITESPACE_SUFFIX_RE, "");

  // 9.
  if (
    subtype === "" || !RegExpPrototypeTest(HTTP_TOKEN_CODE_POINT_RE, subtype)
  ) {
    return null;
  }

  // 10.
  const mimeType = {
    type: StringPrototypeToLowerCase(type),
    subtype: StringPrototypeToLowerCase(subtype),
    /** @type {Map<string, string>} */
    parameters: new SafeMap(),
  };

  // 11.
  while (position < endOfInput) {
    // 11.1.
    position++;

    // 11.2.
    const res1 = collectSequenceOfCodepoints(
      input,
      position,
      (c) => ArrayPrototypeIncludes(HTTP_WHITESPACE, c),
    );
    position = res1.position;

    // 11.3.
    const res2 = collectSequenceOfCodepoints(
      input,
      position,
      (c) => c !== "\u003B" && c !== "\u003D",
    );
    let parameterName = res2.result;
    position = res2.position;

    // 11.4.
    parameterName = StringPrototypeToLowerCase(parameterName);

    // 11.5.
    if (position < endOfInput) {
      if (input[position] == "\u003B") continue;
      position++;
    }

    // 11.6.
    if (position >= endOfInput) break;

    // 11.7.
    let parameterValue = null;

    // 11.8.
    if (input[position] === "\u0022") {
      // 11.8.1.
      const res = collectHttpQuotedString(input, position, true);
      parameterValue = res.result;
      position = res.position;

      // 11.8.2.
      position++;
    } else { // 11.9.
      // 11.9.1.
      const res = collectSequenceOfCodepoints(
        input,
        position,
        (c) => c !== "\u003B",
      );
      parameterValue = res.result;
      position = res.position;

      // 11.9.2.
      parameterValue = StringPrototypeReplaceAll(
        parameterValue,
        HTTP_WHITESPACE_SUFFIX_RE,
        "",
      );

      // 11.9.3.
      if (parameterValue === "") continue;
    }

    // 11.10.
    if (
      parameterName !== "" &&
      RegExpPrototypeTest(HTTP_TOKEN_CODE_POINT_RE, parameterName) &&
      RegExpPrototypeTest(
        HTTP_QUOTED_STRING_TOKEN_POINT_RE,
        parameterValue,
      ) &&
      !MapPrototypeHas(mimeType.parameters, parameterName)
    ) {
      MapPrototypeSet(mimeType.parameters, parameterName, parameterValue);
    }
  }

  // 12.
  return mimeType;
}

/**
 * @param {MimeType} mimeType
 * @returns {string}
 */
function essence(mimeType) {
  return `${mimeType.type}/${mimeType.subtype}`;
}

/**
 * @param {MimeType} mimeType
 * @returns {string}
 */
function serializeMimeType(mimeType) {
  let serialization = essence(mimeType);
  for (const param of new SafeMapIterator(mimeType.parameters)) {
    serialization += `;${param[0]}=`;
    let value = param[1];
    if (RegExpPrototypeExec(HTTP_TOKEN_CODE_POINT_RE, value) === null) {
      value = StringPrototypeReplaceAll(value, "\\", "\\\\");
      value = StringPrototypeReplaceAll(value, '"', '\\"');
      value = `"${value}"`;
    }
    serialization += value;
  }
  return serialization;
}

/**
 * Part of the Fetch spec's "extract a MIME type" algorithm
 * (https://fetch.spec.whatwg.org/#concept-header-extract-mime-type).
 * @param {string[] | null} headerValues The result of getting, decoding and
 * splitting the "Content-Type" header.
 * @returns {MimeType | null}
 */
function extractMimeType(headerValues) {
  if (headerValues === null) return null;

  let charset = null;
  let essence_ = null;
  let mimeType = null;
  for (let i = 0; i < headerValues.length; ++i) {
    const value = headerValues[i];
    const temporaryMimeType = parseMimeType(value);
    if (
      temporaryMimeType === null ||
      essence(temporaryMimeType) == "*/*"
    ) {
      continue;
    }
    mimeType = temporaryMimeType;
    if (essence(mimeType) !== essence_) {
      charset = null;
      const newCharset = MapPrototypeGet(mimeType.parameters, "charset");
      if (newCharset !== undefined) {
        charset = newCharset;
      }
      essence_ = essence(mimeType);
    } else {
      if (
        !MapPrototypeHas(mimeType.parameters, "charset") &&
        charset !== null
      ) {
        MapPrototypeSet(mimeType.parameters, "charset", charset);
      }
    }
  }
  return mimeType;
}

/**
 * Ref: https://mimesniff.spec.whatwg.org/#xml-mime-type
 * @param {MimeType} mimeType
 * @returns {boolean}
 */
function isXML(mimeType) {
  return StringPrototypeEndsWith(mimeType.subtype, "+xml") ||
    essence(mimeType) === "text/xml" || essence(mimeType) === "application/xml";
}

/**
 * Ref: https://mimesniff.spec.whatwg.org/#pattern-matching-algorithm
 * @param {Uint8Array} input
 * @param {Uint8Array} pattern
 * @param {Uint8Array} mask
 * @param {Uint8Array} ignored
 * @returns {boolean}
 */
function patternMatchingAlgorithm(input, pattern, mask, ignored) {
  assert(
    TypedArrayPrototypeGetLength(pattern) ===
      TypedArrayPrototypeGetLength(mask),
  );

  if (
    TypedArrayPrototypeGetLength(input) < TypedArrayPrototypeGetLength(pattern)
  ) {
    return false;
  }

  let s = 0;
  for (; s < TypedArrayPrototypeGetLength(input); s++) {
    if (!TypedArrayPrototypeIncludes(ignored, input[s])) {
      break;
    }
  }

  let p = 0;
  for (; p < TypedArrayPrototypeGetLength(pattern); p++, s++) {
    const maskedData = input[s] & mask[p];
    if (maskedData !== pattern[p]) {
      return false;
    }
  }

  return true;
}

const ImageTypePatternTable = [
  // A Windows Icon signature.
  [
    new Uint8Array([0x00, 0x00, 0x01, 0x00]),
    new Uint8Array([0xFF, 0xFF, 0xFF, 0xFF]),
    new Uint8Array(),
    "image/x-icon",
  ],
  // A Windows Cursor signature.
  [
    new Uint8Array([0x00, 0x00, 0x02, 0x00]),
    new Uint8Array([0xFF, 0xFF, 0xFF, 0xFF]),
    new Uint8Array(),
    "image/x-icon",
  ],
  // The string "BM", a BMP signature.
  [
    new Uint8Array([0x42, 0x4D]),
    new Uint8Array([0xFF, 0xFF]),
    new Uint8Array(),
    "image/bmp",
  ],
  // The string "GIF87a", a GIF signature.
  [
    new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x37, 0x61]),
    new Uint8Array([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    new Uint8Array(),
    "image/gif",
  ],
  // The string "GIF89a", a GIF signature.
  [
    new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]),
    new Uint8Array([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    new Uint8Array(),
    "image/gif",
  ],
  // The string "RIFF" followed by four bytes followed by the string "WEBPVP".
  [
    new Uint8Array([
      0x52,
      0x49,
      0x46,
      0x46,
      0x00,
      0x00,
      0x00,
      0x00,
      0x57,
      0x45,
      0x42,
      0x50,
      0x56,
      0x50,
    ]),
    new Uint8Array([
      0xFF,
      0xFF,
      0xFF,
      0xFF,
      0x00,
      0x00,
      0x00,
      0x00,
      0xFF,
      0xFF,
      0xFF,
      0xFF,
      0xFF,
      0xFF,
    ]),
    new Uint8Array(),
    "image/webp",
  ],
  // An error-checking byte followed by the string "PNG" followed by CR LF SUB LF, the PNG signature.
  [
    new Uint8Array([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
    new Uint8Array([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    new Uint8Array(),
    "image/png",
  ],
  // The JPEG Start of Image marker followed by the indicator byte of another marker.
  [
    new Uint8Array([0xFF, 0xD8, 0xFF]),
    new Uint8Array([0xFF, 0xFF, 0xFF]),
    new Uint8Array(),
    "image/jpeg",
  ],
];

/**
 * Ref: https://mimesniff.spec.whatwg.org/#image-type-pattern-matching-algorithm
 * @param {Uint8Array} input
 * @returns {string | undefined}
 */
function imageTypePatternMatchingAlgorithm(input) {
  for (let i = 0; i < ImageTypePatternTable.length; i++) {
    const row = ImageTypePatternTable[i];
    const patternMatched = patternMatchingAlgorithm(
      input,
      row[0],
      row[1],
      row[2],
    );
    if (patternMatched) {
      return row[3];
    }
  }

  return undefined;
}

/**
 * Ref: https://mimesniff.spec.whatwg.org/#rules-for-sniffing-images-specifically
 * @param {string} mimeTypeString
 * @returns {string}
 */
function sniffImage(mimeTypeString) {
  const mimeType = parseMimeType(mimeTypeString);
  if (mimeType === null) {
    return mimeTypeString;
  }

  if (isXML(mimeType)) {
    return mimeTypeString;
  }

  const imageTypeMatched = imageTypePatternMatchingAlgorithm(
    new TextEncoder().encode(mimeTypeString),
  );
  if (imageTypeMatched !== undefined) {
    return imageTypeMatched;
  }

  return mimeTypeString;
}

export {
  essence,
  extractMimeType,
  parseMimeType,
  serializeMimeType,
  sniffImage,
};
