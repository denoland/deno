import webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";

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

          // TODO Set context's output bitmap to the same bitmap as target's bitmap (so that they are shared).
          // TODO Run the steps to set an ImageBitmapRenderingContext's output bitmap with context.

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
          // TODO  Let context be the result of following the instructions given in WebGPU's Canvas Rendering section. [WEBGPU]
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

    // TODO
  }

  convertToBlob(options = {}) {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    const prefix = "Failed to call 'getContext' on 'OffscreenCanvas'";
    options = webidl.converters.ImageEncodeOptions(options, prefix, "Argument 1");

    // TODO
  }
}
const OffscreenCanvasPrototype = OffscreenCanvas.prototype;

const _canvas = Symbol("[[canvas]]");
const _alpha = Symbol("[[alpha]]");
class ImageBitmapRenderingContext {
  [_canvas];
  get canvas() {
    webidl.assertBranded(this, ImageBitmapRenderingContextPrototype);
    return this[_canvas];
  }

  constructor() {
    webidl.illegalConstructor();
  }

  transferFromImageBitmap(bitmap) {
    webidl.assertBranded(this, ImageBitmapRenderingContextPrototype);
    const prefix = "Failed to call 'getContext' on 'OffscreenCanvas'";
    bitmap = webidl.converters.ImageEncodeOptions(bitmap, prefix, "Argument 1");

    // TODO
  }
}
const ImageBitmapRenderingContextPrototype = ImageBitmapRenderingContext.prototype;

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

// DICT: ImageBitmapRenderingContextSettings
const dictImageBitmapRenderingContextSettings = [
  { key: "alpha", converter: webidl.converters.boolean, defaultValue: true },
];
webidl.converters["ImageBitmapRenderingContextSettings"] = webidl
  .createDictionaryConverter(
    "ImageBitmapRenderingContextSettings",
    dictImageBitmapRenderingContextSettings,
  );

