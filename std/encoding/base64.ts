// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/**
 * Converts given data with base64 encoding
 * @param data input to encode
 */
export function encode(data: string | ArrayBuffer): string {
  if (typeof data === "string") {
    return btoa(data);
  } else {
    const d = new Uint8Array(data);
    let dataString = "";
    for (let i = 0; i < d.length; ++i) {
      dataString += String.fromCharCode(d[i]);
    }

    return btoa(dataString);
  }
}

/**
 * Converts given base64 encoded data back to original
 * @param data input to decode
 */
export function decode(data: string): ArrayBuffer {
  const binaryString = decodeString(data);
  const binary = new Uint8Array(binaryString.length);
  for (let i = 0; i < binary.length; ++i) {
    binary[i] = binaryString.charCodeAt(i);
  }

  return binary.buffer;
}

/**
 * Decodes data assuming the output is in string type
 * @param data input to decode
 */
export function decodeString(data: string): string {
  return atob(data);
}
