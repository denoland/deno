// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  decode as convertBase64ToArrayBuffer,
  encode as convertArrayBufferToBase64,
} from "./base64.ts";

/*
 * Some variants allow or require omitting the padding '=' signs:
 * https://en.wikipedia.org/wiki/Base64#URL_applications
 */
export function addPaddingToBase64url(base64url: string): string {
  if (base64url.length % 4 === 2) return base64url + "==";
  if (base64url.length % 4 === 3) return base64url + "=";
  if (base64url.length % 4 === 1)
    throw new TypeError("Illegal base64url string!");
  return base64url;
}

function convertBase64urlToBase64(base64url: string): string {
  return addPaddingToBase64url(base64url)
    .replace(/\-/g, "+")
    .replace(/_/g, "/");
}

function convertBase64ToBase64url(base64: string): string {
  return base64.replace(/=/g, "").replace(/\+/g, "-").replace(/\//g, "_");
}

/**
 * Converts given data with base64url encoding.
 * Removes paddings '='.
 * @param data input to encode
 */
export function encode(data: string | ArrayBuffer): string {
  return convertBase64ToBase64url(convertArrayBufferToBase64(data));
}

/**
 * Converts given base64url encoded data back to original
 * @param data input to decode
 */
export function decode(data: string): ArrayBuffer {
  return convertBase64ToArrayBuffer(convertBase64urlToBase64(data));
}
