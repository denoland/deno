// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import { containsOnlyASCII } from "./util";
import { TextEncoder } from "./text_encoding";

export const bytesSymbol = Symbol("bytes");

export class DenoBlob implements domTypes.Blob {
  private readonly [bytesSymbol]: Uint8Array;
  readonly size: number = 0;
  readonly type: string = "";

  /** A blob object represents a file-like object of immutable, raw data. */
  constructor(
    blobParts?: domTypes.BlobPart[],
    options?: domTypes.BlobPropertyBag
  ) {
    if (arguments.length === 0) {
      this[bytesSymbol] = new Uint8Array();
      return;
    }

    options = options || {};
    // Set ending property's default value to "transparent".
    if (!options.hasOwnProperty("ending")) {
      options.ending = "transparent";
    }

    if (options.type && !containsOnlyASCII(options.type)) {
      const errMsg = "The 'type' property must consist of ASCII characters.";
      throw new SyntaxError(errMsg);
    }

    const bytes = processBlobParts(blobParts!, options);
    // Normalize options.type.
    let type = options.type ? options.type : "";
    if (type.length) {
      for (let i = 0; i < type.length; ++i) {
        const char = type[i];
        if (char < "\u0020" || char > "\u007E") {
          type = "";
          break;
        }
      }
      type = type.toLowerCase();
    }
    // Set Blob object's properties.
    this[bytesSymbol] = bytes;
    this.size = bytes.byteLength;
    this.type = type;
  }

  slice(start?: number, end?: number, contentType?: string): DenoBlob {
    return new DenoBlob([this[bytesSymbol].slice(start, end)], {
      type: contentType || this.type
    });
  }
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
    .map(u8 => u8.byteLength)
    .reduce((a, b) => a + b, 0);
  const ab = new ArrayBuffer(byteLength);
  const bytes = new Uint8Array(ab);

  let courser = 0;
  for (const u8 of uint8Arrays) {
    bytes.set(u8, courser);
    courser += u8.byteLength;
  }

  return bytes;
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

function convertLineEndingsToNative(s: string): string {
  // TODO(qti3e) Implement convertLineEndingsToNative.
  // https://w3c.github.io/FileAPI/#convert-line-endings-to-native
  return s;
}
