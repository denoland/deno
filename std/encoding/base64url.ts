// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as base64 from "./base64.ts";

/*
 * Some variants allow or require omitting the padding '=' signs:
 * https://en.wikipedia.org/wiki/Base64#URL_applications
 * @param string
 */
export function addPaddingToBase64url(b64url: string): string {
  if (b64url.length % 4 === 2) return b64url + "==";
  if (b64url.length % 4 === 3) return b64url + "=";
  if (b64url.length % 4 === 1) {
    throw new TypeError("Illegal b64url string!");
  }
  return b64url;
}

/**
 * @param string
 */
export function convertBase64urlToBase64(b64url: string): string {
  return addPaddingToBase64url(b64url).replace(/\-/g, "+").replace(/_/g, "/");
}

/**
 * @param string
 */
export function convertBase64ToBase64url(b64: string): string {
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
 * Decodes a given base64url representation
 * @param b64url
 */
export function decode(b64url: string): Uint8Array {
  return base64.decode(convertBase64urlToBase64(b64url));
}
