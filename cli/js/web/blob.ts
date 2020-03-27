// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import { TextEncoder } from "./text_encoding.ts";
import { build } from "../build.ts";

export const bytesSymbol = Symbol("bytes");

export function containsOnlyASCII(str: string): boolean {
  if (typeof str !== "string") {
    return false;
  }
  return /^[\x00-\x7F]*$/.test(str);
}

function convertLineEndingsToNative(s: string): string {
  const nativeLineEnd = build.os == "win" ? "\r\n" : "\n";

  let position = 0;

  let collectionResult = collectSequenceNotCRLF(s, position);

  let token = collectionResult.collected;
  position = collectionResult.newPosition;

  let result = token;

  while (position < s.length) {
    const c = s.charAt(position);
    if (c == "\r") {
      result += nativeLineEnd;
      position++;
      if (position < s.length && s.charAt(position) == "\n") {
        position++;
      }
    } else if (c == "\n") {
      position++;
      result += nativeLineEnd;
    }

    collectionResult = collectSequenceNotCRLF(s, position);

    token = collectionResult.collected;
    position = collectionResult.newPosition;

    result += token;
  }

  return result;
}

function collectSequenceNotCRLF(
  s: string,
  position: number
): { collected: string; newPosition: number } {
  const start = position;
  for (
    let c = s.charAt(position);
    position < s.length && !(c == "\r" || c == "\n");
    c = s.charAt(++position)
  );
  return { collected: s.slice(start, position), newPosition: position };
}

function toUint8Arrays(
  blobParts: domTypes.BlobPart[],
  doNormalizeLineEndingsToNative: boolean
): Uint8Array[] {
  const ret: Uint8Array[] = [];
  const enc = new TextEncoder();
  for (const element of blobParts) {
    if (typeof element === "string") {
      let str = element;
      if (doNormalizeLineEndingsToNative) {
        str = convertLineEndingsToNative(element);
      }
      ret.push(enc.encode(str));
      // eslint-disable-next-line @typescript-eslint/no-use-before-define
    } else if (element instanceof DenoBlob) {
      ret.push(element[bytesSymbol]);
    } else if (element instanceof Uint8Array) {
      ret.push(element);
    } else if (element instanceof Uint16Array) {
      const uint8 = new Uint8Array(element.buffer);
      ret.push(uint8);
    } else if (element instanceof Uint32Array) {
      const uint8 = new Uint8Array(element.buffer);
      ret.push(uint8);
    } else if (ArrayBuffer.isView(element)) {
      // Convert view to Uint8Array.
      const uint8 = new Uint8Array(element.buffer);
      ret.push(uint8);
    } else if (element instanceof ArrayBuffer) {
      // Create a new Uint8Array view for the given ArrayBuffer.
      const uint8 = new Uint8Array(element);
      ret.push(uint8);
    } else {
      ret.push(enc.encode(String(element)));
    }
  }
  return ret;
}

function processBlobParts(
  blobParts: domTypes.BlobPart[],
  options: domTypes.BlobPropertyBag
): Uint8Array {
  const normalizeLineEndingsToNative = options.ending === "native";
  // ArrayBuffer.transfer is not yet implemented in V8, so we just have to
  // pre compute size of the array buffer and do some sort of static allocation
  // instead of dynamic allocation.
  const uint8Arrays = toUint8Arrays(blobParts, normalizeLineEndingsToNative);
  const byteLength = uint8Arrays
    .map((u8): number => u8.byteLength)
    .reduce((a, b): number => a + b, 0);
  const ab = new ArrayBuffer(byteLength);
  const bytes = new Uint8Array(ab);

  let courser = 0;
  for (const u8 of uint8Arrays) {
    bytes.set(u8, courser);
    courser += u8.byteLength;
  }

  return bytes;
}

export class DenoBlob implements domTypes.Blob {
  [bytesSymbol]: Uint8Array;
  readonly size: number = 0;
  readonly type: string = "";

  constructor(
    blobParts?: domTypes.BlobPart[],
    options?: domTypes.BlobPropertyBag
  ) {
    if (arguments.length === 0) {
      this[bytesSymbol] = new Uint8Array();
      return;
    }

    const { ending = "transparent", type = "" } = options ?? {};
    if (!containsOnlyASCII(type)) {
      const errMsg = "The 'type' property must consist of ASCII characters.";
      throw new SyntaxError(errMsg);
    }

    const bytes = processBlobParts(blobParts!, { ending, type });
    // Normalize options.type.
    let normalizedType = type;
    if (type.length) {
      for (let i = 0; i < type.length; ++i) {
        const char = type[i];
        if (char < "\u0020" || char > "\u007E") {
          normalizedType = "";
          break;
        }
      }
      normalizedType = type.toLowerCase();
    }
    // Set Blob object's properties.
    this[bytesSymbol] = bytes;
    this.size = bytes.byteLength;
    this.type = normalizedType;
  }

  slice(start?: number, end?: number, contentType?: string): DenoBlob {
    return new DenoBlob([this[bytesSymbol].slice(start, end)], {
      type: contentType || this.type,
    });
  }
}
