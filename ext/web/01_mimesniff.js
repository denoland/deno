// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />

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
  MathMin,
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
 * https://aomediacodec.github.io/av1-avif/#image-and-image-collection-brand
 * @type {Map<number, number>}
 */
const MP4_MAJOR_BRAND = new SafeMap([
  [0x61766966, 1], // avif
  [0x61766973, 2], // avis
]);

/**
 * Blink: 144
 * https://source.chromium.org/chromium/chromium/src/+/68cb614cb19c91d08da59d9a7a2f1c8dc671e9cc:third_party/blink/renderer/platform/image-decoders/avif/avif_image_decoder.cc;l=488-500
 * Gecko: 512
 * https://github.com/mozilla-firefox/firefox/blob/e1eada69e2ddd86a398ccb141dcbf772254162eb/toolkit/components/mediasniffer/nsMediaSniffer.cpp#L32-L33
 * WebKit: 100
 * https://github.com/WebKit/WebKit/blob/c9d41ab32a016631dd0b0c23249ea274a27d2046/Source/WebCore/platform/graphics/cg/ImageDecoderCG.cpp#L731-L740
 *
 * The structure of FileTypeBox is no guarantee that present at the beginning of the file.
 * So we need to traverse until we find a match for a particular signature, with some upper bound.
 */
const MAX_BYTES_SNIFFED = 64;

/**
 * AVIF is based on ISO-BMFF structures to generate a HEIF/MIAF compatible file,
 * so we can use the same logic as MP4 to sniff the file format.
 *
 * https://mimesniff.spec.whatwg.org/#signature-for-mp4
 * @param {Uint8Array} byteSequence
 * @param {number} maxAttemptLength
 * @returns {Map<number, number> | false}
 */
function matchesMP4(byteSequence, maxAttemptLength) {
  // 3. Assert 12 bytes to determine major brand later
  if (maxAttemptLength < 12) {
    return false;
  }
  // 4.
  const boxSize = (
    (byteSequence[0] << 24) |
    (byteSequence[1] << 16) |
    (byteSequence[2] << 8) |
    byteSequence[3]
  ) >>> 0;
  // 5.
  if (maxAttemptLength < boxSize || boxSize % 4 !== 0) {
    return false;
  }
  // 6. Check 4 bytes box type
  // 0x66 0x74 0x79 0x70 ("ftyp")
  if (
    byteSequence[4] !== 0x66 ||
    byteSequence[5] !== 0x74 ||
    byteSequence[6] !== 0x79 ||
    byteSequence[7] !== 0x70
  ) {
    return false;
  }
  // 7. Assert 4 bytes ("<major brand>")
  const brandInt32 = (
    (byteSequence[8] << 24) |
    (byteSequence[9] << 16) |
    (byteSequence[10] << 8) |
    byteSequence[11]
  ) >>> 0;
  if (MapPrototypeHas(MP4_MAJOR_BRAND, brandInt32)) {
    return MapPrototypeGet(MP4_MAJOR_BRAND, brandInt32);
  }
  // 8. Skip minor version (4 bytes)
  let bytesRead = 16;
  // 9. Check compatible brands
  while (bytesRead < boxSize) {
    const compatibleBrandInt32 = (
      (byteSequence[bytesRead] << 24) |
      (byteSequence[bytesRead + 1] << 16) |
      (byteSequence[bytesRead + 2] << 8) |
      byteSequence[bytesRead + 3]
    ) >>> 0;
    const brand = MapPrototypeGet(MP4_MAJOR_BRAND, compatibleBrandInt32);
    if (brand) {
      return brand;
    }
    bytesRead += 4;
  }
  // 10.
  return false;
}

/**
 * @param {Uint8Array} byteSequence
 * @returns {string | null}
 */
function getAvifMimeType(byteSequence) {
  const length = TypedArrayPrototypeGetLength(byteSequence);
  const maxAttemptLength = MathMin(length, MAX_BYTES_SNIFFED);
  const brand = matchesMP4(byteSequence, maxAttemptLength);
  if (brand === false) {
    return null;
  }
  /**
   * TODO: return a unique number to avoid conversion from string to number in
   * {@link file://./../image/01_image.js}
   */
  return "image/avif";
}

/**
 * Ref: https://mimesniff.spec.whatwg.org/#rules-for-sniffing-images-specifically
 * @param {string | null} mimeTypeString
 * @param {Uint8Array} byteSequence
 * @returns {string | null}
 */
function sniffImage(mimeTypeString, byteSequence) {
  // NOTE: Do we need to implement the "supplied MIME type" detection exactly?
  // https://mimesniff.spec.whatwg.org/#supplied-mime-type-detection-algorithm

  if (mimeTypeString !== null && isXML(mimeTypeString)) {
    return mimeTypeString;
  }

  const imageTypeMatched = imageTypePatternMatchingAlgorithm(byteSequence);
  if (imageTypeMatched !== undefined) {
    return imageTypeMatched;
  }

  // NOTE: Some browsers have implementation-defined image formats.
  // For example, The AVIF image format is supported by all browsers today.
  // However, the mime sniffing standardization seems to have hard going.
  // See: https://github.com/whatwg/mimesniff/issues/143
  const avifMimeType = getAvifMimeType(byteSequence);
  if (avifMimeType !== null) {
    return avifMimeType;
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
