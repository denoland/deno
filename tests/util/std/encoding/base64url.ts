// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * {@linkcode encodeBase64Url} and {@linkcode decodeBase64Url} for
 * [base64 URL safe](https://en.wikipedia.org/wiki/Base64#URL_applications) encoding.
 *
 * This module is browser compatible.
 *
 * @example
 * ```ts
 * import {
 *   decodeBase64Url,
 *   encodeBase64Url,
 * } from "https://deno.land/std@$STD_VERSION/encoding/base64url.ts";
 *
 * const binary = new TextEncoder().encode("foobar");
 * const encoded = encodeBase64Url(binary);
 * console.log(encoded);
 * // => "Zm9vYmFy"
 *
 * console.log(decodeBase64Url(encoded));
 * // => Uint8Array(6) [ 102, 111, 111, 98, 97, 114 ]
 * ```
 *
 * @module
 */

import * as base64 from "./base64.ts";

/*
 * Some variants allow or require omitting the padding '=' signs:
 * https://en.wikipedia.org/wiki/Base64#The_URL_applications
 * @param base64url
 */
function addPaddingToBase64url(base64url: string): string {
  if (base64url.length % 4 === 2) return base64url + "==";
  if (base64url.length % 4 === 3) return base64url + "=";
  if (base64url.length % 4 === 1) {
    throw new TypeError("Illegal base64url string!");
  }
  return base64url;
}

function convertBase64urlToBase64(b64url: string): string {
  if (!/^[-_A-Z0-9]*?={0,2}$/i.test(b64url)) {
    // Contains characters not part of base64url spec.
    throw new TypeError("Failed to decode base64url: invalid character");
  }
  return addPaddingToBase64url(b64url).replace(/\-/g, "+").replace(/_/g, "/");
}

function convertBase64ToBase64url(b64: string) {
  return b64.endsWith("=")
    ? b64.endsWith("==")
      ? b64.replace(/\+/g, "-").replace(/\//g, "_").slice(0, -2)
      : b64.replace(/\+/g, "-").replace(/\//g, "_").slice(0, -1)
    : b64.replace(/\+/g, "-").replace(/\//g, "_");
}

/**
 * @deprecated (will be removed in 0.210.0) Use {@linkcode encodeBase64Url} instead.
 *
 * Encodes a given ArrayBuffer or string into a base64url representation
 * @param data
 */
export const encode = encodeBase64Url;

/**
 * @deprecated (will be removed in 0.210.0) Use {@linkcode decodeBase64Url} instead.
 *
 * Converts given base64url encoded data back to original
 * @param b64url
 */
export const decode = decodeBase64Url;

/**
 * Encodes a given ArrayBuffer or string into a base64url representation
 * @param data
 */
export function encodeBase64Url(
  data: ArrayBuffer | Uint8Array | string,
): string {
  return convertBase64ToBase64url(base64.encodeBase64(data));
}

/**
 * Converts given base64url encoded data back to original
 * @param b64url
 */
export function decodeBase64Url(b64url: string): Uint8Array {
  return base64.decodeBase64(convertBase64urlToBase64(b64url));
}
