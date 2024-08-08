// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "./01_dom_exception.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
const {
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeGetSymbolToStringTag,
  Uint8ClampedArray,
} = primordials;

webidl.converters["PredefinedColorSpace"] = webidl.createEnumConverter(
  "PredefinedColorSpace",
  [
    "srgb",
    "display-p3",
  ],
);

webidl.converters["ImageDataSettings"] = webidl.createDictionaryConverter(
  "ImageDataSettings",
  [
    { key: "colorSpace", converter: webidl.converters["PredefinedColorSpace"] },
  ],
);

/** images are stored as RGBA8 */
const _data = Symbol("[[data]]");
const _width = Symbol("[[width]]");
const _height = Symbol("[[height]]");
class ImageData {
  /** @type {number} */
  [_width];
  /** @type {height} */
  [_height];
  /** @type {Uint8Array} */
  [_data];
  /** @type {'srgb' | 'display-p3'} */
  #colorSpace;

  constructor(arg0, arg1, arg2 = undefined, arg3 = undefined) {
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
    const prefix = "Failed to construct 'ImageData'";

    // Overload: new ImageData(data, sw [, sh [, settings ] ])
    if (
      arguments.length > 3 ||
      TypedArrayPrototypeGetSymbolToStringTag(arg0) === "Uint8ClampedArray"
    ) {
      data = webidl.converters.Uint8ClampedArray(arg0, prefix, "Argument 1");
      sourceWidth = webidl.converters["unsigned long"](
        arg1,
        prefix,
        "Argument 2",
      );
      const dataLength = TypedArrayPrototypeGetLength(data);

      if (webidl.type(arg2) !== "Undefined") {
        sourceHeight = webidl.converters["unsigned long"](
          arg2,
          prefix,
          "Argument 3",
        );
      }

      settings = webidl.converters["ImageDataSettings"](
        arg3,
        prefix,
        "Argument 4",
      );

      if (dataLength === 0) {
        throw new DOMException(
          "Failed to construct 'ImageData': The input data has zero elements.",
          "InvalidStateError",
        );
      }

      if (dataLength % 4 !== 0) {
        throw new DOMException(
          "Failed to construct 'ImageData': The input data length is not a multiple of 4.",
          "InvalidStateError",
        );
      }

      if (sourceWidth < 1) {
        throw new DOMException(
          "Failed to construct 'ImageData': The source width is zero or not a number.",
          "IndexSizeError",
        );
      }

      if (webidl.type(sourceHeight) !== "Undefined" && sourceHeight < 1) {
        throw new DOMException(
          "Failed to construct 'ImageData': The source height is zero or not a number.",
          "IndexSizeError",
        );
      }

      if (dataLength / 4 % sourceWidth !== 0) {
        throw new DOMException(
          "Failed to construct 'ImageData': The input data length is not a multiple of (4 * width).",
          "IndexSizeError",
        );
      }

      if (
        webidl.type(sourceHeight) !== "Undefined" &&
        (sourceWidth * sourceHeight * 4 !== dataLength)
      ) {
        throw new DOMException(
          "Failed to construct 'ImageData': The input data length is not equal to (4 * width * height).",
          "IndexSizeError",
        );
      }

      if (webidl.type(sourceHeight) === "Undefined") {
        this[_height] = dataLength / 4 / sourceWidth;
      } else {
        this[_height] = sourceHeight;
      }

      this.#colorSpace = settings.colorSpace ?? "srgb";
      this[_width] = sourceWidth;
      this[_data] = data;
      return;
    }

    // Overload: new ImageData(sw, sh [, settings])
    sourceWidth = webidl.converters["unsigned long"](
      arg0,
      prefix,
      "Argument 1",
    );
    sourceHeight = webidl.converters["unsigned long"](
      arg1,
      prefix,
      "Argument 2",
    );

    settings = webidl.converters["ImageDataSettings"](
      arg2,
      prefix,
      "Argument 3",
    );

    if (sourceWidth < 1) {
      throw new DOMException(
        "Failed to construct 'ImageData': The source width is zero or not a number.",
        "IndexSizeError",
      );
    }

    if (sourceHeight < 1) {
      throw new DOMException(
        "Failed to construct 'ImageData': The source height is zero or not a number.",
        "IndexSizeError",
      );
    }

    this.#colorSpace = settings.colorSpace ?? "srgb";
    this[_width] = sourceWidth;
    this[_height] = sourceHeight;
    this[_data] = new Uint8ClampedArray(sourceWidth * sourceHeight * 4);
  }

  get width() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this[_width];
  }

  get height() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this[_height];
  }

  get data() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this[_data];
  }

  get colorSpace() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this.#colorSpace;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(ImageDataPrototype, this),
        keys: [
          "data",
          "width",
          "height",
          "colorSpace",
        ],
      }),
      inspectOptions,
    );
  }
}

const ImageDataPrototype = ImageData.prototype;

export { _data, _height, _width, ImageData, ImageDataPrototype };
