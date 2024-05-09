import webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { op_image_encode_png } from "ext:core/ops";


const _width = Symbol("[[width]]");
const _height = Symbol("[[height]]");
const _context = Symbol("[[context]]");
const _contextMode = Symbol("[[contextMode]]");
class OffscreenCanvas extends EventTarget {
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


    // TODO: internal bitmap
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
          context; // TODO Set context's output bitmap to the same bitmap as target's bitmap (so that they are shared).
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

    // TODO: Let image be a newly created ImageBitmap object that references the same underlying bitmap data as this OffscreenCanvas object's bitmap.
    // TODO: Set this OffscreenCanvas object's bitmap to reference a newly created bitmap of the same dimensions and color space as the previous bitmap, and with its pixels initialized to transparent black, or opaque black if the rendering context's alpha flag is set to false.
  }

  convertToBlob(options = {}) {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    const prefix = "Failed to call 'getContext' on 'OffscreenCanvas'";
    options = webidl.converters.ImageEncodeOptions(options, prefix, "Argument 1");

    // TODO: If the value of this OffscreenCanvas object's [[Detached]] internal slot is set to true, then return a promise rejected with an "InvalidStateError" DOMException.
    // TODO: If this OffscreenCanvas object's context mode is 2d and the rendering context's bitmap's origin-clean flag is set to false, then return a promise rejected with a "SecurityError" DOMException.
    // TODO: If this OffscreenCanvas object's bitmap has no pixels (i.e., either its horizontal dimension or its vertical dimension is zero) then return a promise rejected with an "IndexSizeError" DOMException.
    // TODO: Let bitmap be a copy of this OffscreenCanvas object's bitmap.


    op_image_encode_png(, this[_width], this[_height]);

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
      // TODO: If the value of bitmap's [[Detached]] internal slot is set to true, then throw an "InvalidStateError" DOMException.
      setOutputBitmap(this, bitmap);
      // TODO: Set the value of bitmap's [[Detached]] internal slot to true.
      // TODO: Unset bitmap's bitmap data.
    }
  }
}
const ImageBitmapRenderingContextPrototype = ImageBitmapRenderingContext.prototype;

function setOutputBitmap(context, data) {
  if (!data) {
    context[_bitmapMode] = "blank";
    context[_canvas]; // TODO: Set context's output bitmap to be transparent black with a natural width equal to the numeric value of canvas's width attribute and a natural height equal to the numeric value of canvas's height attribute, those values being interpreted in CSS pixels.
  } else {
    context[_bitmapMode] = "valid";
    context; // TODO: Set context's output bitmap to refer to the same underlying bitmap data as bitmap, without making a copy.
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

