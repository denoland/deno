// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as base64 from "base64-js";
import { DenoError, ErrorKind } from "./errors";

export function atob(s: string): string {
  const rem = s.length % 4;
  // base64-js requires length exactly times of 4
  if (rem > 0) {
    s = s.padEnd(s.length + (4 - rem), "=");
  }
  let byteArray;
  try {
    byteArray = base64.toByteArray(s);
  } catch (_) {
    throw new DenoError(
      ErrorKind.InvalidInput,
      "The string to be decoded is not correctly encoded"
    );
  }
  let result = "";
  for (let i = 0; i < byteArray.length; i++) {
    result += String.fromCharCode(byteArray[i]);
  }
  return result;
}

export function btoa(s: string): string {
  const byteArray = [];
  for (let i = 0; i < s.length; i++) {
    const charCode = s[i].charCodeAt(0);
    if (charCode > 0xff) {
      throw new DenoError(
        ErrorKind.InvalidInput,
        "The string to be encoded contains characters " +
          "outside of the Latin1 range."
      );
    }
    byteArray.push(charCode);
  }
  const result = base64.fromByteArray(Uint8Array.from(byteArray));
  return result;
}

// @types/text-encoding relies on lib.dom.d.ts for some interfaces. We do not
// want to include lib.dom.d.ts (due to size) into deno's global type scope.
// Therefore this hack: add a few of the missing interfaces in
// @types/text-encoding to the global scope before importing.

declare global {
  type BufferSource = ArrayBufferView | ArrayBuffer;

  interface TextDecodeOptions {
    stream?: boolean;
  }

  interface TextDecoderOptions {
    fatal?: boolean;
    ignoreBOM?: boolean;
  }

  interface TextDecoder {
    readonly encoding: string;
    readonly fatal: boolean;
    readonly ignoreBOM: boolean;
    decode(input?: BufferSource, options?: TextDecodeOptions): string;
  }
}

export { TextEncoder, TextDecoder } from "text-encoding";
