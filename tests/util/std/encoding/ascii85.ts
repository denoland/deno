// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { validateBinaryLike } from "./_util.ts";

/**
 * {@linkcode encode} and {@linkcode decode} for
 * [Ascii85/base85](https://en.wikipedia.org/wiki/Ascii85) encoding.
 *
 * This module is browser compatible.
 *
 * ## Specifying a standard and delimiter
 *
 * By default, all functions are using the most popular Adobe version of ascii85
 * and not adding any delimiter. However, there are three more standards
 * supported - btoa (different delimiter and additional compression of 4 bytes
 * equal to 32), [Z85](https://rfc.zeromq.org/spec/32/) and
 * [RFC 1924](https://tools.ietf.org/html/rfc1924). It's possible to use a
 * different encoding by specifying it in `options` object as a second parameter.
 *
 * Similarly, it's possible to make `encode` add a delimiter (`<~` and `~>` for
 * Adobe, `xbtoa Begin` and `xbtoa End` with newlines between the delimiters and
 * encoded data for btoa. Checksums for btoa are not supported. Delimiters are not
 * supported by other encodings.)
 *
 * @example
 * ```ts
 * import {
 *   decode,
 *   encode,
 * } from "https://deno.land/std@$STD_VERSION/encoding/ascii85.ts";
 *
 * const a85Repr = "LpTqp";
 *
 * const binaryData = decode(a85Repr);
 * console.log(binaryData);
 * // => Uint8Array [ 136, 180, 79, 24 ]
 *
 * console.log(encode(binaryData));
 * // => LpTqp
 * ```
 *
 * @module
 */

export type Ascii85Standard = "Adobe" | "btoa" | "RFC 1924" | "Z85";

/** encoding/decoding options */
export interface Ascii85Options {
  /** characterset and delimiter (if supported and used).
   *
   * @default {"Adobe"}
   */
  standard?: Ascii85Standard;
  /** whether to use a delimiter (if supported) - "<~" and "~>" by default */
  delimiter?: boolean;
}
const rfc1924 =
  "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";
const Z85 =
  "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.-:+=^!/*?&<>()[]{}@%$#";

/**
 * @deprecated (will be removed in 0.210.0) Use {@linkcode encodeAscii85} instead.
 *
 * Encodes a given Uint8Array into ascii85, supports multiple standards
 * @param uint8 input to encode
 * @param [options] encoding options
 * @param [options.standard=Adobe] encoding standard (Adobe, btoa, RFC 1924 or Z85)
 * @param [options.delimiter] whether to use a delimiter, if supported by encoding standard
 */
export const encode = encodeAscii85;

/**
 * Encodes a given Uint8Array into ascii85, supports multiple standards
 * @param uint8 input to encode
 * @param [options] encoding options
 * @param [options.standard=Adobe] encoding standard (Adobe, btoa, RFC 1924 or Z85)
 * @param [options.delimiter] whether to use a delimiter, if supported by encoding standard
 */
export function encodeAscii85(
  data: ArrayBuffer | Uint8Array | string,
  options?: Ascii85Options,
): string {
  let uint8 = validateBinaryLike(data);

  const standard = options?.standard ?? "Adobe";
  let output: string[] = [],
    v: number,
    n = 0,
    difference = 0;
  if (uint8.length % 4 !== 0) {
    const tmp = uint8;
    difference = 4 - (tmp.length % 4);
    uint8 = new Uint8Array(tmp.length + difference);
    uint8.set(tmp);
  }
  const view = new DataView(uint8.buffer, uint8.byteOffset, uint8.byteLength);
  for (let i = 0, len = uint8.length; i < len; i += 4) {
    v = view.getUint32(i);
    // Adobe and btoa standards compress 4 zeroes to single "z" character
    if (
      (standard === "Adobe" || standard === "btoa") &&
      v === 0 &&
      i < len - difference - 3
    ) {
      output[n++] = "z";
      continue;
    }
    // btoa compresses 4 spaces - that is, bytes equal to 32 - into single "y" character
    if (standard === "btoa" && v === 538976288) {
      output[n++] = "y";
      continue;
    }
    for (let j = 4; j >= 0; j--) {
      output[n + j] = String.fromCharCode((v % 85) + 33);
      v = Math.trunc(v / 85);
    }
    n += 5;
  }
  switch (standard) {
    case "Adobe":
      if (options?.delimiter) {
        return `<~${output.slice(0, output.length - difference).join("")}~>`;
      }
      break;
    case "btoa":
      if (options?.delimiter) {
        return `xbtoa Begin\n${
          output
            .slice(0, output.length - difference)
            .join("")
        }\nxbtoa End`;
      }
      break;
    case "RFC 1924":
      output = output.map((val) => rfc1924[val.charCodeAt(0) - 33]);
      break;
    case "Z85":
      output = output.map((val) => Z85[val.charCodeAt(0) - 33]);
      break;
  }
  return output.slice(0, output.length - difference).join("");
}

/**
 * @deprecated (will be removed in 0.210.0) Use {@linkcode decodeAscii85} instead.
 *
 * Decodes a given ascii85 encoded string.
 * @param ascii85 input to decode
 * @param [options] decoding options
 * @param [options.standard=Adobe] encoding standard used in the input string (Adobe, btoa, RFC 1924 or Z85)
 */
export const decode = decodeAscii85;

/**
 * Decodes a given ascii85 encoded string.
 * @param ascii85 input to decode
 * @param [options] decoding options
 * @param [options.standard=Adobe] encoding standard used in the input string (Adobe, btoa, RFC 1924 or Z85)
 */
export function decodeAscii85(
  ascii85: string,
  options?: Ascii85Options,
): Uint8Array {
  const encoding = options?.standard ?? "Adobe";
  // translate all encodings to most basic adobe/btoa one and decompress some special characters ("z" and "y")
  switch (encoding) {
    case "Adobe":
      ascii85 = ascii85.replaceAll(/(<~|~>)/g, "").replaceAll("z", "!!!!!");
      break;
    case "btoa":
      ascii85 = ascii85
        .replaceAll(/(xbtoa Begin|xbtoa End|\n)/g, "")
        .replaceAll("z", "!!!!!")
        .replaceAll("y", "+<VdL");
      break;
    case "RFC 1924":
      ascii85 = ascii85.replaceAll(
        /./g,
        (match) => String.fromCharCode(rfc1924.indexOf(match) + 33),
      );
      break;
    case "Z85":
      ascii85 = ascii85.replaceAll(
        /./g,
        (match) => String.fromCharCode(Z85.indexOf(match) + 33),
      );
      break;
  }
  //remove all invalid characters
  ascii85 = ascii85.replaceAll(/[^!-u]/g, "");
  const len = ascii85.length,
    output = new Uint8Array(len + 4 - (len % 4));
  const view = new DataView(output.buffer);
  let v = 0,
    n = 0,
    max = 0;
  for (let i = 0; i < len;) {
    for (max += 5; i < max; i++) {
      v = v * 85 + (i < len ? ascii85.charCodeAt(i) : 117) - 33;
    }
    view.setUint32(n, v);
    v = 0;
    n += 4;
  }
  return output.slice(0, Math.trunc(len * 0.8));
}
