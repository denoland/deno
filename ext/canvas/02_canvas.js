import webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { op_image_encode_png } from "ext:core/ops";
import { _width, _height, _bitmapData, _detached, ImageBitmap } from "ext:deno_canvas/01_image.js";

const _context = Symbol("[[context]]");
const _contextMode = Symbol("[[contextMode]]");
class OffscreenCanvas extends EventTarget {
  [_bitmapData];

  [_width];
  get width() {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    return this[_width];
  }

  [_height];
  get height() {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    return this[_width];
  }

  [_contextMode] = null;
  [_context] = null;
  [_detached] = false;

  constructor(width, height) {
    super();

    this[webidl.brand] = webidl.brand;

    const prefix = "Failed to construct 'OffscreenCanvas'";
    webidl.requiredArguments(arguments.length, 2, prefix);

    width = webidl.converters["unsigned long long"](width, prefix, "Argument 1", {
      enforceRange: true,
    });
    height = webidl.converters["unsigned long long"](height, prefix, "Argument 2", {
      enforceRange: true,
    });

    this[_width] = width;
    this[_height] = height;
    this[_bitmapData] = new Uint8Array(width * height * 3);
  }

  getContext(contextId, options = null) {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    const prefix = "Failed to call 'getContext' on 'OffscreenCanvas'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    contextId = webidl.converters.OffscreenRenderingContextId(contextId, prefix, "Argument 1");
    options = webidl.converters.any(options);

    if (webidl.type(options) !== "Object") {
      options = null;
    }

    if (contextId === "bitmaprenderer") {
      switch (this[_contextMode]) {
        case null: {
          const settings = webidl.converters.ImageBitmapRenderingContextSettings(options, prefix, "Argument 2");
          const context = webidl.createBranded(ImageBitmapRenderingContext);
          context[_canvas] = this;
          context[_bitmapData] = this[_bitmapData];
          setOutputBitmap(context);
          context[_alpha] = settings.alpha;

          this[_contextMode] = "bitmaprenderer";
          this[_context] = context;
          return context;
        }
        case "bitmaprenderer": {
          return this[_context];
        }
        case "webgpu": {
          return null;
        }
      }
    } else if (contextId === "webgpu") {
      switch (this[_contextMode]) {
        case null: {
          // TODO Let context be the result of following the instructions given in WebGPU's Canvas Rendering section. [WEBGPU]
          this[_contextMode] = "bitmaprenderer";
          this[_context] = context;
          return context;
        }
        case "bitmaprenderer": {
          return null;
        }
        case "webgpu": {
          return this[_context];
        }
      }
    } else {
      throw new DOMException(`Context '${contextId}' not implemented`, "NotSupportedError");
    }
  }

  transferToImageBitmap() {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    // TODO: If the value of this OffscreenCanvas object's [[Detached]] internal slot is set to true, then throw an "InvalidStateError" DOMException.

    if (!this[_contextMode]) {
      throw new DOMException("Cannot get bitmap from canvas without a context", "InvalidStateError");
    }

    let image = webidl.createBranded(ImageBitmap);
    image[_bitmapData] = this[_bitmapData];
    image[_width] = this[_width];
    image[_height] = this[_height];
    this[_bitmapData] = new Uint8Array(this[_bitmapData].length);
    return image;
  }

  convertToBlob(options = {}) {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    const prefix = "Failed to call 'getContext' on 'OffscreenCanvas'";
    options = webidl.converters.ImageEncodeOptions(options, prefix, "Argument 1");

    // TODO: If the value of this OffscreenCanvas object's [[Detached]] internal slot is set to true, then return a promise rejected with an "InvalidStateError" DOMException.

    if (this[_bitmapData].length === 0) {
      throw new DOMException("", "DOMException");
    }

    const png = op_image_encode_png(this[_bitmapData], this[_width], this[_height]);

    if (!png) {
      throw new DOMException("", "EncodingError");
    }

    return Promise.resolve(new Blob([png], {
      type: "image/png",
    }));
  }
}
const OffscreenCanvasPrototype = OffscreenCanvas.prototype;

const _canvas = Symbol("[[canvas]]");
const _bitmapMode = Symbol("[[bitmapMode]]");
const _alpha = Symbol("[[alpha]]");
class ImageBitmapRenderingContext {
  [_canvas];
  get canvas() {
    webidl.assertBranded(this, ImageBitmapRenderingContextPrototype);
    return this[_canvas];
  }

  [_bitmapMode];

  constructor() {
    webidl.illegalConstructor();
  }

  transferFromImageBitmap(bitmap) {
    webidl.assertBranded(this, ImageBitmapRenderingContextPrototype);
    const prefix = "Failed to call 'getContext' on 'OffscreenCanvas'";
    bitmap = webidl.converters["ImageBitmap?"](bitmap, prefix, "Argument 1");

    if (bitmap === null) {
      setOutputBitmap(this);
    } else {
      if (bitmap[_detached]) {
        throw new DOMException("bitmap cannot be used as it has been detached", "InvalidStateError");
      }
      setOutputBitmap(this, bitmap);
      bitmap[_detached] = true;
      bitmap[_bitmapData] = null;
    }
  }
}
const ImageBitmapRenderingContextPrototype = ImageBitmapRenderingContext.prototype;

function setOutputBitmap(context, bitmap) {
  if (!bitmap) {
    context[_bitmapMode] = "blank";
    context[_bitmapData] = new Uint8Array(context[_canvas][_bitmapData].length);
  } else {
    context[_bitmapMode] = "valid";
    context[_bitmapData] = bitmap[_bitmapData];
  }
}

// ENUM: OffscreenRenderingContextId
webidl.converters["OffscreenRenderingContextId"] = webidl.createEnumConverter(
  "OffscreenRenderingContextId",
  [
    "2d", "bitmaprenderer", "webgl", "webgl2", "webgpu"
  ],
);

// DICT: ImageEncodeOptions
const dictImageEncodeOptions = [
  { key: "type", converter: webidl.converters.DOMString, defaultValue: "image/png" },
  {
    key: "quality",
    converter: webidl.converters["unrestricted double"],
  }
];
webidl.converters["ImageEncodeOptions"] = webidl
  .createDictionaryConverter(
    "ImageEncodeOptions",
    dictImageEncodeOptions,
  );

webidl.converters["ImageBitmap?"] = webidl.createNullableConverter(webidl.converters["ImageBitmap"]);

// DICT: ImageBitmapRenderingContextSettings
const dictImageBitmapRenderingContextSettings = [
  { key: "alpha", converter: webidl.converters.boolean, defaultValue: true },
];
webidl.converters["ImageBitmapRenderingContextSettings"] = webidl
  .createDictionaryConverter(
    "ImageBitmapRenderingContextSettings",
    dictImageBitmapRenderingContextSettings,
  );

