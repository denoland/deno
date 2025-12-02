// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "./01_dom_exception.js";
import { createFilteredInspectProxy } from "./01_console.js";
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

webidl.converters["ImageDataPixelFormat"] = webidl.createEnumConverter(
  "ImageDataPixelFormat",
  ["rgba-unorm8", "rgba-float16"],
);

webidl.converters["ImageDataSettings"] = webidl.createDictionaryConverter(
  "ImageDataSettings",
  [
    { key: "colorSpace", converter: webidl.converters["PredefinedColorSpace"] },
    {
      key: "pixelFormat",
      converter: webidl.converters["ImageDataPixelFormat"],
      defaultValue: "rgba-unorm8",
    },
  ],
);

const _data = Symbol("[[data]]");
const _width = Symbol("[[width]]");
const _height = Symbol("[[height]]");
class ImageData {
  /** @type {number} */
  [_width];
  /** @type {number} */
  [_height];
  /** @type {ImageDataArray} */
  [_data];
  /** @type {ImageDataPixelFormat} */
  #pixelFormat;
  /** @type {PredefinedColorSpace} */
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
    const tag = TypedArrayPrototypeGetSymbolToStringTag(arg0);

    // Overload: new ImageData(data, sw [, sh [, settings ] ])
    if (
      arguments.length > 3 ||
      tag === "Uint8ClampedArray" || tag === "Float16Array"
    ) {
      data = webidl.converters.ArrayBufferView(arg0, prefix, "Argument 1");
      sourceWidth = webidl.converters["unsigned long"](
        arg1,
        prefix,
        "Argument 2",
      );
      const dataLength = TypedArrayPrototypeGetLength(data);

      if (arg2 !== undefined) {
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
          "Failed to construct 'ImageData': the input data has zero elements",
          "InvalidStateError",
        );
      }

      if (dataLength % 4 !== 0) {
        throw new DOMException(
          `Failed to construct 'ImageData': the input data length is not a multiple of 4, received ${dataLength}`,
          "InvalidStateError",
        );
      }

      if (sourceWidth < 1) {
        throw new DOMException(
          "Failed to construct 'ImageData': the source width is zero or not a number",
          "IndexSizeError",
        );
      }

      if (sourceHeight !== undefined && sourceHeight < 1) {
        throw new DOMException(
          "Failed to construct 'ImageData': the source height is zero or not a number",
          "IndexSizeError",
        );
      }

      if (dataLength / 4 % sourceWidth !== 0) {
        throw new DOMException(
          "Failed to construct 'ImageData': the input data length is not a multiple of (4 * width)",
          "IndexSizeError",
        );
      }

      if (
        sourceHeight !== undefined &&
        dataLength / 4 / sourceWidth !== sourceHeight
      ) {
        throw new DOMException(
          "Failed to construct 'ImageData': the input data length is not equal to (4 * width * height)",
          "IndexSizeError",
        );
      }

      switch (tag) {
        case "Uint8ClampedArray":
          if (settings.pixelFormat !== "rgba-unorm8") {
            throw new DOMException(
              "Failed to construct 'ImageData': Uint8ClampedArray must use rgba-unorm8 pixelFormat.",
              "InvalidStateError",
            );
          }
          break;
        case "Float16Array":
          if (settings.pixelFormat !== "rgba-float16") {
            throw new DOMException(
              "Failed to construct 'ImageData': Float16Array must use rgba-float16 pixelFormat.",
              "InvalidStateError",
            );
          }
          break;
      }

      this.#pixelFormat = settings.pixelFormat;
      this.#colorSpace = settings.colorSpace ?? "srgb";
      this[_width] = sourceWidth;
      this[_height] = dataLength / 4 / sourceWidth;
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
        "Failed to construct 'ImageData': the source width is zero or not a number",
        "IndexSizeError",
      );
    }

    if (sourceHeight < 1) {
      throw new DOMException(
        "Failed to construct 'ImageData': the source height is zero or not a number",
        "IndexSizeError",
      );
    }

    switch (settings.pixelFormat) {
      case "rgba-unorm8":
        data = new Uint8ClampedArray(sourceWidth * sourceHeight * 4);
        break;
      case "rgba-float16":
        // TODO(0f-0b): add Float16Array to primordials
        data = new Float16Array(sourceWidth * sourceHeight * 4);
        break;
    }

    this.#pixelFormat = settings.pixelFormat;
    this.#colorSpace = settings.colorSpace ?? "srgb";
    this[_width] = sourceWidth;
    this[_height] = sourceHeight;
    this[_data] = data;
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

  get pixelFormat() {
    webidl.assertBranded(this, ImageDataPrototype);
    return this.#pixelFormat;
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
          "pixelFormat",
          "colorSpace",
        ],
      }),
      inspectOptions,
    );
  }
}

const ImageDataPrototype = ImageData.prototype;

export { _data, _height, _width, ImageData, ImageDataPrototype };
