// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
const primordials = globalThis.__bootstrap.primordials;
const {
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeGetSymbolToStringTag,
  Uint8ClampedArray,
  MathCeil,
  MathMax,
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

webidl.converters["ImageOrientation"] = webidl.createEnumConverter("ImageOrientation", [
  "from-image", "flipY"
]);

webidl.converters["PremultiplyAlpha"] = webidl.createEnumConverter("PremultiplyAlpha", [
  "none", "premultiply", "default"
]);

webidl.converters["ColorSpaceConversion"] = webidl.createEnumConverter("ColorSpaceConversion", [
  "none", "default"
]);

webidl.converters["ResizeQuality"] = webidl.createEnumConverter("ResizeQuality", [
  "pixelated", "low", "medium", "high"
]);

webidl.converters["ImageBitmapOptions"] = webidl.createDictionaryConverter(
  "ImageBitmapOptions",
  [
    {
      key: "imageOrientation",
      converter: webidl.converters["ImageOrientation"],
      defaultValue: "from-image",
    },
    {
      key: "premultiplyAlpha",
      converter: webidl.converters["PremultiplyAlpha"],
      defaultValue: "default",
    },
    {
      key: "colorSpaceConversion",
      converter: webidl.converters["ColorSpaceConversion"],
      defaultValue: "default",
    },
    {
      key: "resizeWidth",
      converter: (v, prefix, context, opts) =>
        webidl.converters["unsigned long"](v, prefix, context, {
          ...opts,
          enforceRange: true,
        }),
    },
    {
      key: "resizeHeight",
      converter: (v, prefix, context, opts) =>
        webidl.converters["unsigned long"](v, prefix, context, {
          ...opts,
          enforceRange: true,
        }),
    },
    {
      key: "resizeQuality",
      converter: webidl.converters["ResizeQuality"],
      defaultValue: "low",
    },
  ],
);

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

const _bitmapData = Symbol("[[bitmapData]]");
class ImageBitmap {
  constructor() {
  }
}
const ImageBitmapPrototype = ImageBitmap.prototype;

function createImageBitmap(image, sxOrOptions = undefined, sy = undefined, sw = undefined, sh = undefined, options = undefined) {
  const prefix = "Failed to call 'createImageBitmap'";

  // Overload: createImageBitmap(image [, options ])
  if (arguments.length < 3) {
    options = webidl.converters["ImageBitmapOptions"](sxOrOptions, prefix, "Argument 2");
  } else {
    // Overload: createImageBitmap(image, sx, sy, sw, sh [, options ])
    sxOrOptions = webidl.converters["long"](sxOrOptions, prefix, "Argument 2");
    sy = webidl.converters["long"](sy, prefix, "Argument 3");
    sw = webidl.converters["long"](sw, prefix, "Argument 4");
    sh = webidl.converters["long"](sh, prefix, "Argument 5");
    options = webidl.converters["ImageBitmapOptions"](options, prefix, "Argument 6");

    if (sw === 0) {
      return Promise.reject(new RangeError("sw has to be greater than 0"));
    }

    if (sh === 0) {
      return Promise.reject(new RangeError("sh has to be greater than 0"));
    }
  }

  if (options.resizeWidth === 0) {
    return Promise.reject(new DOMException("options.resizeWidth has to be greater than 0", "InvalidStateError"));
  }
  if (options.resizeHeight === 0) {
    return Promise.reject(new DOMException("options.resizeWidth has to be greater than 0", "InvalidStateError"));
  }

  const imageBitmap = webidl.createBranded(ImageBitmap);

  if (ObjectPrototypeIsPrototypeOf(ImageDataPrototype, image)) {
    imageBitmap[_bitmapData] = crop(image[_data], image[_width], image[_height], sxOrOptions, sy, sw, sh, options);
    return Promise.resolve(imageBitmap);
  } else {
    return Promise.reject(new TypeError("Invalid or unsupported image value"));
  }
}

function crop(input, width, height, sx, sy, sw, sh, options) {
  let sourceRectangle;

  if (sx !== undefined && sy !== undefined && sw !== undefined && sh !== undefined) {
    sourceRectangle = [
      [sx, sy],
      [sx + sw, sy],
      [sx + sw, sy + sh],
      [sx, sy + sh],
    ];
  } else {
    sourceRectangle = [
      [0, 0],
      [width, 0],
      [width, height],
      [0, height],
    ];
  }
  const widthOfSourceRect = sourceRectangle[1][0] - sourceRectangle[0][0];
  const heightOfSourceRect = sourceRectangle[3][1] - sourceRectangle[0][1];

  let outputWidth;
  if (options.resizeWidth !== undefined) {
    outputWidth = options.resizeWidth;
  } else if (options.resizeHeight !== undefined) {
    outputWidth = MathCeil((widthOfSourceRect * options.resizeHeight) / heightOfSourceRect);
  } else {
    outputWidth = widthOfSourceRect;
  }

  let outputHeight;
  if (options.resizeHeight !== undefined) {
    outputHeight = options.resizeHeight;
  } else if (options.resizeWidth !== undefined) {
    outputHeight = MathCeil((heightOfSourceRect * options.resizeWidth) / widthOfSourceRect);
  } else {
    outputHeight = heightOfSourceRect;
  }

  const output = new Uint8Array(widthOfSourceRect * heightOfSourceRect * 4);

  for (let i = sourceRectangle[0][1]; i < sourceRectangle[3][1]; i++) {
    const startOfRow = i * (width * 4);
    const rowColOffset = (sourceRectangle[0][0] * 4);
    const start = startOfRow + rowColOffset;
    const end = start + (widthOfSourceRect * 4);

    const slicedRow = input.slice(start, end);

    output.set(slicedRow, (i - sourceRectangle[0][1]) * (widthOfSourceRect * 4));
  }

  /*
  [
    1, 1, 1, 1, 1,
    1, 1, 1, 1, 1,
    1, 1, 1, 1, 1,
    1, 1, 1, 1, 1,
    1, 1, 1, 1, 1,
  ]
  ->
  [

       1, 1, 1,
       1, 1, 1,
       1, 1, 1,

  ]
   */
}

export { createImageBitmap, ImageBitmap, ImageData };
