// Copyright 2018-2025 the Deno authors. MIT license.

import { internals, primordials } from "ext:core/mod.js";
import { op_create_image_bitmap } from "ext:core/ops";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import { BlobPrototype } from "ext:deno_web/09_file.js";
import { sniffImage } from "ext:deno_web/01_mimesniff.js";
const {
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  TypedArrayPrototypeGetBuffer,
  Uint8Array,
  PromiseReject,
  RangeError,
  ArrayPrototypeJoin,
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
  // Add the value when implementing to add support for ImageBitmapSource
  const imageBitmapSources = [
    "Blob",
    "ImageData",
    "ImageBitmap",
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

    // 1.
    if (sw === 0) {
      return PromiseReject(new RangeError("sw has to be greater than 0"));
    }

    if (sh === 0) {
      return PromiseReject(new RangeError("sh has to be greater than 0"));
    }
  }

  // 2.
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

  // 3.
  const isBlob = ObjectPrototypeIsPrototypeOf(BlobPrototype, image);
  const isImageData = ObjectPrototypeIsPrototypeOf(ImageDataPrototype, image);
  const isImageBitmap = ObjectPrototypeIsPrototypeOf(
    ImageBitmapPrototype,
    image,
  );
  if (!isBlob && !isImageData && !isImageBitmap) {
    return PromiseReject(
      new DOMException(
        `${prefix}: The provided value for 'image' is not of type '(${
          ArrayPrototypeJoin(imageBitmapSources, " or ")
        })'`,
        "InvalidStateError",
      ),
    );
  }

  // 4.
  return (async () => {
    //
    // For performance reasons, the arguments passed to op are represented as numbers that don't need to be serialized.
    //

    let width = 0;
    let height = 0;
    // If the image doesn't have a MIME type, mark it as 0.
    let mimeType = 0;
    let imageBitmapSource, buf;
    if (isBlob) {
      imageBitmapSource = 0;
      buf = new Uint8Array(await image.arrayBuffer());
      const mimeTypeString = sniffImage(image.type);

      if (mimeTypeString === "image/png") {
        mimeType = 1;
      } else if (mimeTypeString === "image/jpeg") {
        mimeType = 2;
      } else if (mimeTypeString === "image/gif") {
        mimeType = 3;
        // NOTE: Temporarily not supported due to build size concerns
        // https://github.com/denoland/deno/pull/25517#issuecomment-2626044644
        return PromiseReject(
          new DOMException(
            "The MIME type of source image is not supported currently",
            "InvalidStateError",
          ),
        );
      } else if (mimeTypeString === "image/bmp") {
        mimeType = 4;
      } else if (mimeTypeString === "image/x-icon") {
        mimeType = 5;
      } else if (mimeTypeString === "image/webp") {
        mimeType = 6;
        // NOTE: Temporarily not supported due to build size concerns
        // https://github.com/denoland/deno/pull/25517#issuecomment-2626044644
        return PromiseReject(
          new DOMException(
            "The MIME type of source image is not supported currently",
            "InvalidStateError",
          ),
        );
      } else if (mimeTypeString === "") {
        return PromiseReject(
          new DOMException(
            `The MIME type of source image is not specified\n
hint: When you want to get a "Blob" from "fetch", make sure to go through a file server that returns the appropriate content-type response header,
      and specify the URL to the file server like "await(await fetch('http://localhost:8000/sample.png').blob()".
      Alternatively, if you are reading a local file using 'Deno.readFile' etc.,
      set the appropriate MIME type like "new Blob([await Deno.readFile('sample.png')], { type: 'image/png' })".\n`,
            "InvalidStateError",
          ),
        );
      } else {
        return PromiseReject(
          new DOMException(
            `The the MIME type ${mimeTypeString} of source image is not a supported format\n
info: The following MIME types are supported.
docs: https://mimesniff.spec.whatwg.org/#image-type-pattern-matching-algorithm\n`,
            "InvalidStateError",
          ),
        );
      }
    } else if (isImageData) {
      width = image[_width];
      height = image[_height];
      imageBitmapSource = 1;
      buf = new Uint8Array(TypedArrayPrototypeGetBuffer(image[_data]));
    } else if (isImageBitmap) {
      width = image[_width];
      height = image[_height];
      imageBitmapSource = 2;
      buf = new Uint8Array(TypedArrayPrototypeGetBuffer(image[_bitmapData]));
    }

    // If those options are not provided, assign 0 to mean undefined(None).
    const _sx = typeof sxOrOptions === "number" ? sxOrOptions : 0;
    const _sy = sy ?? 0;
    const _sw = sw ?? 0;
    const _sh = sh ?? 0;

    // If those options are not provided, assign 0 to mean undefined(None).
    const resizeWidth = options.resizeWidth ?? 0;
    const resizeHeight = options.resizeHeight ?? 0;

    // If the imageOrientation option is set "from-image" or not set, assign 0.
    const imageOrientation = options.imageOrientation === "flipY" ? 1 : 0;

    // If the premultiplyAlpha option is "default" or not set, assign 0.
    let premultiplyAlpha = 0;
    if (options.premultiplyAlpha === "premultiply") {
      premultiplyAlpha = 1;
    } else if (options.premultiplyAlpha === "none") {
      premultiplyAlpha = 2;
    }

    // If the colorSpaceConversion option is "default" or not set, assign 0.
    const colorSpaceConversion = options.colorSpaceConversion === "none"
      ? 1
      : 0;

    // If the resizeQuality option is "low" or not set, assign 0.
    let resizeQuality = 0;
    if (options.resizeQuality === "pixelated") {
      resizeQuality = 1;
    } else if (options.resizeQuality === "medium") {
      resizeQuality = 2;
    } else if (options.resizeQuality === "high") {
      resizeQuality = 3;
    }

    const processedImage = op_create_image_bitmap(
      buf,
      width,
      height,
      _sx,
      _sy,
      _sw,
      _sh,
      imageOrientation,
      premultiplyAlpha,
      colorSpaceConversion,
      resizeWidth,
      resizeHeight,
      resizeQuality,
      imageBitmapSource,
      mimeType,
    );
    imageBitmap[_bitmapData] = processedImage[0];
    imageBitmap[_width] = processedImage[1];
    imageBitmap[_height] = processedImage[2];
    return imageBitmap;
  })();
}

function getBitmapData(imageBitmap) {
  return imageBitmap[_bitmapData];
}

internals.getBitmapData = getBitmapData;

export { _bitmapData, _detached, createImageBitmap, ImageBitmap };
