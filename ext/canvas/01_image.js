// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { internals, primordials } from "ext:core/mod.js";
import { op_image_process } from "ext:core/ops";
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
  PromiseReject,
  RangeError,
  ArrayPrototypeJoin,
} = primordials;
import {
  _colorSpace,
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
  // Add the value when implementing to add support for ImageBitmapSource
  const imageBitmapSources = [
    "Blob",
    "ImageData",
  ];

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
        "options.resizeHeight has to be greater than 0",
        "InvalidStateError",
      ),
    );
  }

  const imageBitmap = webidl.createBranded(ImageBitmap);

  // 6. Switch on image
  const isBlob = ObjectPrototypeIsPrototypeOf(BlobPrototype, image);
  const isImageData = ObjectPrototypeIsPrototypeOf(ImageDataPrototype, image);
  if (
    isImageData ||
    isBlob
  ) {
    return (async () => {
      let width = 0;
      let height = 0;
      let mimeType = "";
      let imageBitmapSource, buf, predefinedColorSpace;
      if (isBlob) {
        imageBitmapSource = imageBitmapSources[0];
        buf = new Uint8Array(await image.arrayBuffer());
        mimeType = sniffImage(image.type);
      }
      if (isImageData) {
        width = image[_width];
        height = image[_height];
        imageBitmapSource = imageBitmapSources[1];
        buf = new Uint8Array(TypedArrayPrototypeGetBuffer(image[_data]));
        predefinedColorSpace = image[_colorSpace];
      }

      let sx;
      if (typeof sxOrOptions === "number") {
        sx = sxOrOptions;
      }

      const processedImage = op_image_process(
        buf,
        {
          width,
          height,
          sx,
          sy,
          sw,
          sh,
          imageOrientation: options.imageOrientation ?? "from-image",
          premultiplyAlpha: options.premultiplyAlpha ?? "default",
          predefinedColorSpace: predefinedColorSpace ?? "srgb",
          colorSpaceConversion: options.colorSpaceConversion ?? "default",
          resizeWidth: options.resizeWidth,
          resizeHeight: options.resizeHeight,
          resizeQuality: options.resizeQuality ?? "low",
          imageBitmapSource,
          mimeType,
        },
      );
      imageBitmap[_bitmapData] = processedImage.data;
      imageBitmap[_width] = processedImage.width;
      imageBitmap[_height] = processedImage.height;
      return imageBitmap;
    })();
  } else {
    return PromiseReject(
      new TypeError(
        `${prefix}: The provided value is not of type '(${
          ArrayPrototypeJoin(imageBitmapSources, " or ")
        })'.`,
      ),
    );
  }
}

function getBitmapData(imageBitmap) {
  return imageBitmap[_bitmapData];
}

internals.getBitmapData = getBitmapData;

export { _bitmapData, _detached, createImageBitmap, ImageBitmap };
