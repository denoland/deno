// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { internals, primordials } from "ext:core/mod.js";
import { op_image_decode_png, op_image_process } from "ext:core/ops";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import { BlobPrototype } from "ext:deno_web/09_file.js";
import { sniffImage } from "ext:deno_web/01_mimesniff.js";
const {
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  TypeError,
  TypedArrayPrototypeGetBuffer,
  Uint8Array,
  MathCeil,
  PromiseResolve,
  PromiseReject,
  RangeError,
} = primordials;
import {
  _data,
  _height,
  _width,
  ImageDataPrototype,
} from "ext:deno_web/16_image_data.js";

webidl.converters["ImageOrientation"] = webidl.createEnumConverter(
  "ImageOrientation",
  [
    "from-image",
    "flipY",
  ],
);

webidl.converters["PremultiplyAlpha"] = webidl.createEnumConverter(
  "PremultiplyAlpha",
  [
    "none",
    "premultiply",
    "default",
  ],
);

webidl.converters["ColorSpaceConversion"] = webidl.createEnumConverter(
  "ColorSpaceConversion",
  [
    "none",
    "default",
  ],
);

webidl.converters["ResizeQuality"] = webidl.createEnumConverter(
  "ResizeQuality",
  [
    "pixelated",
    "low",
    "medium",
    "high",
  ],
);

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

const _bitmapData = Symbol("[[bitmapData]]");
const _detached = Symbol("[[detached]]");
class ImageBitmap {
  [_width];
  [_height];
  [_bitmapData];
  [_detached];

  constructor() {
    webidl.illegalConstructor();
  }

  get width() {
    webidl.assertBranded(this, ImageBitmapPrototype);
    if (this[_detached]) {
      return 0;
    }

    return this[_width];
  }

  get height() {
    webidl.assertBranded(this, ImageBitmapPrototype);
    if (this[_detached]) {
      return 0;
    }

    return this[_height];
  }

  close() {
    webidl.assertBranded(this, ImageBitmapPrototype);
    this[_detached] = true;
    this[_bitmapData] = null;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(ImageBitmapPrototype, this),
        keys: [
          "width",
          "height",
        ],
      }),
      inspectOptions,
    );
  }
}
const ImageBitmapPrototype = ImageBitmap.prototype;

function createImageBitmap(
  image,
  sxOrOptions = undefined,
  sy = undefined,
  sw = undefined,
  sh = undefined,
  options = undefined,
) {
  const prefix = "Failed to execute 'createImageBitmap'";

  // Overload: createImageBitmap(image [, options ])
  if (arguments.length < 3) {
    options = webidl.converters["ImageBitmapOptions"](
      sxOrOptions,
      prefix,
      "Argument 2",
    );
  } else {
    // Overload: createImageBitmap(image, sx, sy, sw, sh [, options ])
    sxOrOptions = webidl.converters["long"](sxOrOptions, prefix, "Argument 2");
    sy = webidl.converters["long"](sy, prefix, "Argument 3");
    sw = webidl.converters["long"](sw, prefix, "Argument 4");
    sh = webidl.converters["long"](sh, prefix, "Argument 5");
    options = webidl.converters["ImageBitmapOptions"](
      options,
      prefix,
      "Argument 6",
    );

    if (sw === 0) {
      return PromiseReject(new RangeError("sw has to be greater than 0"));
    }

    if (sh === 0) {
      return PromiseReject(new RangeError("sh has to be greater than 0"));
    }
  }

  if (options.resizeWidth === 0) {
    return PromiseReject(
      new DOMException(
        "options.resizeWidth has to be greater than 0",
        "InvalidStateError",
      ),
    );
  }
  if (options.resizeHeight === 0) {
    return PromiseReject(
      new DOMException(
        "options.resizeWidth has to be greater than 0",
        "InvalidStateError",
      ),
    );
  }

  const imageBitmap = webidl.createBranded(ImageBitmap);

  if (ObjectPrototypeIsPrototypeOf(ImageDataPrototype, image)) {
    const processedImage = processImage(
      image[_data],
      image[_width],
      image[_height],
      sxOrOptions,
      sy,
      sw,
      sh,
      options,
    );
    imageBitmap[_bitmapData] = processedImage.data;
    imageBitmap[_width] = processedImage.outputWidth;
    imageBitmap[_height] = processedImage.outputHeight;
    return PromiseResolve(imageBitmap);
  }
  if (ObjectPrototypeIsPrototypeOf(BlobPrototype, image)) {
    return (async () => {
      const data = await image.arrayBuffer();
      const mimetype = sniffImage(image.type);
      if (mimetype !== "image/png") {
        throw new DOMException(
          `Unsupported type '${image.type}'`,
          "InvalidStateError",
        );
      }
      const { data: imageData, width, height } = op_image_decode_png(
        new Uint8Array(data),
      );
      const processedImage = processImage(
        imageData,
        width,
        height,
        sxOrOptions,
        sy,
        sw,
        sh,
        options,
      );
      imageBitmap[_bitmapData] = processedImage.data;
      imageBitmap[_width] = processedImage.outputWidth;
      imageBitmap[_height] = processedImage.outputHeight;
      return imageBitmap;
    })();
  } else {
    return PromiseReject(new TypeError("Invalid or unsupported image value"));
  }
}

function processImage(input, width, height, sx, sy, sw, sh, options) {
  let sourceRectangle;

  if (
    sx !== undefined && sy !== undefined && sw !== undefined && sh !== undefined
  ) {
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
    outputWidth = MathCeil(
      (widthOfSourceRect * options.resizeHeight) / heightOfSourceRect,
    );
  } else {
    outputWidth = widthOfSourceRect;
  }

  let outputHeight;
  if (options.resizeHeight !== undefined) {
    outputHeight = options.resizeHeight;
  } else if (options.resizeWidth !== undefined) {
    outputHeight = MathCeil(
      (heightOfSourceRect * options.resizeWidth) / widthOfSourceRect,
    );
  } else {
    outputHeight = heightOfSourceRect;
  }

  if (options.colorSpaceConversion === "none") {
    throw new TypeError(
      "Cannot create image: invalid colorSpaceConversion option, 'none' is not supported",
    );
  }

  /*
   * The cropping works differently than the spec specifies:
   * The spec states to create an infinite surface and place the top-left corner
   * of the image a 0,0 and crop based on sourceRectangle.
   *
   * We instead create a surface the size of sourceRectangle, and position
   * the image at the correct location, which is the inverse of the x & y of
   * sourceRectangle's top-left corner.
   */
  const data = op_image_process(
    new Uint8Array(TypedArrayPrototypeGetBuffer(input)),
    {
      width,
      height,
      surfaceWidth: widthOfSourceRect,
      surfaceHeight: heightOfSourceRect,
      inputX: sourceRectangle[0][0] * -1, // input_x
      inputY: sourceRectangle[0][1] * -1, // input_y
      outputWidth,
      outputHeight,
      resizeQuality: options.resizeQuality,
      flipY: options.imageOrientation === "flipY",
      premultiply: options.premultiplyAlpha === "default"
        ? null
        : (options.premultiplyAlpha === "premultiply"),
    },
  );

  return {
    data,
    outputWidth,
    outputHeight,
  };
}

function getBitmapData(imageBitmap) {
  return imageBitmap[_bitmapData];
}

internals.getBitmapData = getBitmapData;

export { _bitmapData, _detached, createImageBitmap, ImageBitmap };
