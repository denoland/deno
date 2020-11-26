// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as base64 from "./base64.ts";

/*
 * Some variants allow or require omitting the padding '=' signs:
 * https://en.wikipedia.org/wiki/Base64#URL_applications
 * @param base64url
 */
export function addPaddingToBase64url(base64url: string): string {
  if (base64url.length % 4 === 2) return base64url + "==";
  if (base64url.length % 4 === 3) return base64url + "=";
  if (base64url.length % 4 === 1) {
    throw new TypeError("Illegal base64url string!");
  }
  return base64url;
}

function convertBase64urlToBase64(b64url: string): string {
  return addPaddingToBase64url(b64url).replace(/\-/g, "+").replace(/_/g, "/");
}

function convertBase64ToBase64url(b64: string): string {
  return b64.replace(/=/g, "").replace(/\+/g, "-").replace(/\//g, "_");
}

/**
 * Encodes a given Uint8Array into a base64url representation
 * @param uint8
 */
export function encode(uint8: Uint8Array): string {
  return convertBase64ToBase64url(base64.encode(uint8));
}

/**
 * Converts given base64url encoded data back to original
 * @param b64url
 */
export function decode(b64url: string): Uint8Array {
  return base64.decode(convertBase64urlToBase64(b64url));
}
