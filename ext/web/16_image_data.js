// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import * as webidl from "ext:deno_webidl/00_webidl.js";
import DOMException from "ext:deno_web/01_dom_exception.js";
const primordials = globalThis.__bootstrap.primordials;
const {
  TypeError,
} = primordials;

const PredefinedColorSpace = [
  "srgb",
  "display-p3",
];

class ImageData {
  /** @type {number} */
  #width;
  /** @type {height} */
  #height;
  /** @type {Uint8Array} */
  #data;
  /** @type {'srgb' | 'display-p3'} */
  #colorSpace;

  constructor() {
    webidl.requiredArguments(
      arguments.length,
      2,
      'Failed to construct "ImageData"',
    );

    const [arg0, arg1, arg2, arg3] = arguments;
    let sourceWidth;
    let sourceHeight;
    let data;
    let settings;

    // Overload: new ImageData(data, sw [, sh [, settings ] ])
    if (typeof arg0 === "object") {
      data = arg0;
      sourceWidth = typeof arg1 !== "undefined"
        ? parseInt(arg1, 10)
        : undefined;
      sourceHeight = typeof arg2 !== "undefined"
        ? parseInt(arg2, 10)
        : undefined;
      settings = arg3;

      if (!(data instanceof Uint8ClampedArray)) {
        throw new TypeError(
          "Failed to construct 'ImageData': The provided value is not an instance of Uint8ClampedArray.",
          "TypeError",
        );
      }

      if (Number.isNaN(sourceWidth) || sourceWidth < 1) {
        throw new DOMException(
          "Failed to construct 'ImageData': The source width is zero or not a number.",
          "IndexSizeError",
        );
      }

      if (
        typeof sourceHeight !== "undefined" && Number.isNaN(sourceWidth) ||
        sourceHeight < 1
      ) {
        throw new DOMException(
          "Failed to construct 'ImageData': The source height is zero or not a number.",
          "IndexSizeError",
        );
      }

      if (data.length % 4 !== 0) {
        throw new DOMException(
          "Failed to construct 'ImageData': The input data length is not a multiple of 4.",
          "InvalidStateError",
        );
      }

      if (data.length / 4 % sourceWidth !== 0) {
        throw new DOMException(
          "Failed to construct 'ImageData': The input data length is not a multiple of (4 * width).",
          "IndexSizeError",
        );
      }

      if (
        typeof sourceHeight !== "undefined" &&
        (sourceWidth * sourceHeight * 4 !== data.length)
      ) {
        throw new DOMException(
          "Failed to construct 'ImageData': The input data length is not equal to (4 * width * height).",
          "IndexSizeError",
        );
      }

      if (
        typeof settings !== "undefined" &&
        typeof settings.colorSpace !== "undefined" &&
        !PredefinedColorSpace.includes(settings.colorSpace)
      ) {
        throw new TypeError(
          `Failed to read the 'colorSpace' property from 'ImageDataSettings': The provided value '${settings.colorSpace}' is not a valid enum value of type PredefinedColorSpace.`,
        );
      }

      this.#width = sourceWidth;
      this.#height = typeof sourceHeight === "undefined"
        ? data.length / 4 / sourceWidth
        : sourceHeight;
      this.#data = data;
      this.#colorSpace = typeof settings !== "undefined" &&
          typeof settings.colorSpace !== "undefined"
        ? settings.colorSpace
        : "srgb";
      return;
    }

    // Overload: new ImageData(sw, sh [, settings])
    sourceWidth = typeof arg0 !== "undefined" ? parseInt(arg0, 10) : undefined;
    sourceHeight = typeof arg1 !== "undefined" ? parseInt(arg1, 10) : undefined;
    settings = arg2;

    if (Number.isNaN(sourceWidth) || sourceWidth < 1) {
      throw new DOMException(
        "Failed to construct 'ImageData': The source width is zero or not a number.",
        "IndexSizeError",
      );
    }

    if (Number.isNaN(sourceWidth) || sourceHeight < 1) {
      throw new DOMException(
        "Failed to construct 'ImageData': The source height is zero or not a number.",
        "IndexSizeError",
      );
    }

    if (
      typeof settings !== "undefined" &&
      typeof settings.colorSpace !== "undefined" &&
      !PredefinedColorSpace.includes(settings.colorSpace)
    ) {
      throw new TypeError(
        `Failed to read the 'colorSpace' property from 'ImageDataSettings': The provided value '${settings.colorSpace}' is not a valid enum value of type PredefinedColorSpace.`,
      );
    }

    this.#width = sourceWidth;
    this.#height = sourceHeight;
    this.#colorSpace = typeof settings !== "undefined" &&
        typeof settings.colorSpace !== "undefined"
      ? settings.colorSpace
      : "srgb";
    this.#data = new Uint8ClampedArray(sourceWidth * sourceHeight * 4);
  }

  get width() {
    return this.#width;
  }

  get height() {
    return this.#height;
  }

  get data() {
    return this.#data;
  }

  get colorSpace() {
    return this.#colorSpace;
  }
}

export { ImageData };
