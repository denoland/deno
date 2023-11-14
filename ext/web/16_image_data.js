// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import * as webidl from "ext:deno_webidl/00_webidl.js";
import DOMException from "ext:deno_web/01_dom_exception.js";
const primordials = globalThis.__bootstrap.primordials;
const {
  TypeError,
} = primordials;

webidl.converters["PredefinedColorSpace"] = webidl.createEnumConverter("PredefinedColorSpace", [
  "srgb",
  "display-p3",
]);
class ImageData {
  /** @type {number} */
  #width;
  /** @type {height} */
  #height;
  /** @type {Uint8Array} */
  #data;
  /** @type {'srgb' | 'display-p3'} */
  #colorSpace;

  constructor(arg0, arg1, arg2, arg3) {
    webidl.requiredArguments(
      arguments.length,
      2,
      'Failed to construct "ImageData"',
    );
    this[webidl.brand] = webidl.brand;

    let sourceWidth;
    let sourceHeight;
    let data;
    let settings;

    // Overload: new ImageData(data, sw [, sh [, settings ] ])
    if (webidl.type(arg0) === "Object") {
      data = arg0;
      sourceWidth = webidl.type(arg1) !== "Undefined"
        ? parseInt(arg1, 10)
        : undefined;
      sourceHeight = webidl.type(arg2) !== "Undefined"
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
        webidl.type(sourceHeight) !== "Undefined" &&
        (Number.isNaN(sourceHeight) ||
          sourceHeight < 1)
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
        webidl.type(sourceHeight) !== "Undefined" &&
        (sourceWidth * sourceHeight * 4 !== data.length)
      ) {
        throw new DOMException(
          "Failed to construct 'ImageData': The input data length is not equal to (4 * width * height).",
          "IndexSizeError",
        );
      }

      this.#width = sourceWidth;
      this.#height = webidl.type(sourceHeight) === "Undefined"
        ? data.length / 4 / sourceWidth
        : sourceHeight;
      this.#data = data;
      this.#colorSpace = webidl.type(settings) === "Object" &&
          webidl.type(settings.colorSpace) !== "Undefined"
        ? webidl.converters.PredefinedColorSpace(
          settings.colorSpace,
          "Failed to construct 'ImageData'",
          "colorSpace",
        )
        : "srgb";
      return;
    }

    // Overload: new ImageData(sw, sh [, settings])
    sourceWidth = webidl.type(arg0) !== "Undefined"
      ? parseInt(arg0, 10)
      : undefined;
    sourceHeight = webidl.type(arg1) !== "Undefined"
      ? parseInt(arg1, 10)
      : undefined;
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

    this.#width = sourceWidth;
    this.#height = sourceHeight;
    this.#colorSpace = typeof settings !== "undefined" &&
        typeof settings.colorSpace !== "undefined"
      ? webidl.converters.PredefinedColorSpace(settings.colorSpace, "Failed to construct 'ImageData'", "colorSpace")
      : "srgb";
    this.#data = new Uint8ClampedArray(sourceWidth * sourceHeight * 4);
  }

  get width() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this.#width;
  }

  get height() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this.#height;
  }

  get data() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this.#data;
  }

  get colorSpace() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this.#colorSpace;
  }
}

const ImageDataPrototype = ImageData.prototype;

export { ImageData };
