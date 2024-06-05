// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { op_image_encode_png } from "ext:core/ops";
import { core, primordials } from "ext:core/mod.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
const { _width, _height, _bitmapData, _detached, ImageBitmap } = core
  .createLazyLoader("ext:deno_canvas/01_image.js")();
import { loadWebGPU } from "ext:deno_webgpu/00_init.js";
const {
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
} = primordials;
import { assert } from "ext:deno_web/00_infra.js";

const _context = Symbol("[[context]]");
const _canvasBitmap = Symbol("[[canvasBitmap]]");
const _contextMode = Symbol("[[contextMode]]");
class OffscreenCanvas extends EventTarget {
  [_canvasBitmap];

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

    width = webidl.converters["unsigned long long"](
      width,
      prefix,
      "Argument 1",
      {
        enforceRange: true,
      },
    );
    height = webidl.converters["unsigned long long"](
      height,
      prefix,
      "Argument 2",
      {
        enforceRange: true,
      },
    );

    this[_width] = width;
    this[_height] = height;
    this[_canvasBitmap] = {
      data: new Uint8Array(width * height * 3),
    };
  }

  getContext(contextId, options = null) {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    const prefix = "Failed to call 'getContext' on 'OffscreenCanvas'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    contextId = webidl.converters.OffscreenRenderingContextId(
      contextId,
      prefix,
      "Argument 1",
    );
    options = webidl.converters.any(options);

    if (webidl.type(options) !== "Object") {
      options = null;
    }

    if (contextId === "bitmaprenderer") {
      switch (this[_contextMode]) {
        case null: {
          const settings = webidl.converters
            .ImageBitmapRenderingContextSettings(options, prefix, "Argument 2");
          const context = webidl.createBranded(ImageBitmapRenderingContext);
          context[_canvas] = this;
          context[_canvasBitmap] = this[_canvasBitmap];
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
          const context = webidl.createBranded(GPUCanvasContext);
          context[_canvas] = this;
          // TODO: Replace the drawing buffer of context.

          this[_contextMode] = "webgpu";
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
      throw new DOMException(
        `Context '${contextId}' not implemented`,
        "NotSupportedError",
      );
    }
  }

  transferToImageBitmap() {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    // TODO: If the value of this OffscreenCanvas object's [[Detached]] internal slot is set to true, then throw an "InvalidStateError" DOMException.

    if (!this[_contextMode]) {
      throw new DOMException(
        "Cannot get bitmap from canvas without a context",
        "InvalidStateError",
      );
    }

    this[_fillCanvasBitmapHook]?.();

    let image = webidl.createBranded(ImageBitmap);
    image[_bitmapData] = this[_canvasBitmap].data;
    image[_width] = this[_width];
    image[_height] = this[_height];
    this[_canvasBitmap].data = new Uint8Array(this[_bitmapData].length);
    return image;
  }

  convertToBlob(options = {}) {
    webidl.assertBranded(this, OffscreenCanvasPrototype);
    const prefix = "Failed to call 'getContext' on 'OffscreenCanvas'";
    options = webidl.converters.ImageEncodeOptions(
      options,
      prefix,
      "Argument 1",
    );

    this[_context][_fillCanvasBitmapHook]?.();


    // TODO: If the value of this OffscreenCanvas object's [[Detached]] internal slot is set to true, then return a promise rejected with an "InvalidStateError" DOMException.

    if (this[_canvasBitmap].data.length === 0) {
      throw new DOMException("", "DOMException");
    }

    const png = op_image_encode_png(
      this[_canvasBitmap].data,
      this[_width],
      this[_height],
    );

    if (!png) {
      throw new DOMException("", "EncodingError");
    }

    return Promise.resolve(
      new Blob([png], {
        type: "image/png",
      }),
    );
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
        throw new DOMException(
          "bitmap cannot be used as it has been detached",
          "InvalidStateError",
        );
      }
      setOutputBitmap(this, bitmap);
      bitmap[_detached] = true;
      bitmap[_canvasBitmap] = null;
    }
  }
}
const ImageBitmapRenderingContextPrototype =
  ImageBitmapRenderingContext.prototype;

function setOutputBitmap(context, bitmap) {
  if (!bitmap) {
    context[_bitmapMode] = "blank";
    context[_canvasBitmap].data = new Uint8Array(
      context[_canvasBitmap].data.length,
    );
  } else {
    context[_bitmapMode] = "valid";
    context[_canvasBitmap].data = bitmap[_bitmapData];
    context[_canvas][_width] = bitmap[_width];
    context[_canvas][_height] = bitmap[_height];
  }
}

const _configuration = Symbol("[[configuration]]");
const _textureDescriptor = Symbol("[[textureDescriptor]]");
const _currentTexture = Symbol("[[currentTexture]]");
const _drawingBuffer = Symbol("[[drawingBuffer]]");
const _fillCanvasBitmapHook = Symbol("[[fillCanvasBitmapHook]]");
class GPUCanvasContext {
  [_configuration];
  /** @type {GPUTexture | undefined} */
  [_currentTexture];

  [_canvas];
  get canvas() {
    webidl.assertBranded(this, GPUCanvasContextPrototype);
    return this[_canvas];
  }

  constructor() {
    webidl.illegalConstructor();
  }

  configure(configuration) {
    loadWebGPU();

    webidl.assertBranded(this, GPUCanvasContextPrototype);
    const prefix = "Failed to execute 'configure' on 'GPUCanvasContext'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    configuration = webidl.converters.GPUCanvasConfiguration(configuration, {
      prefix,
      context: "Argument 1",
    });

    const descriptor = getTextureDescriptorForCanvasAndConfiguration(
      this[_canvas],
      configuration,
    );
    this[_configuration] = configuration;
    this[_textureDescriptor] = descriptor;

    // TODO: Replace the drawing buffer of this, which resets this.[[drawingBuffer]] with a bitmap with the new format and tags.
  }

  unconfigure() {
    webidl.assertBranded(this, GPUCanvasContextPrototype);

    this[_configuration] = null;
    this[_textureDescriptor] = null;
    replaceDrawingBuffer(this);
  }

  getCurrentTexture() {
    webidl.assertBranded(this, GPUCanvasContextPrototype);

    if (!this[_configuration]) {
      throw new DOMException(
        "The context was not configured",
        "InvalidStateError",
      );
    }
    assert(this[_textureDescriptor]);

    if (!this[_currentTexture]) {
      replaceDrawingBuffer(this);

      this[_currentTexture] = this[_configuration].device.createTexture(
        this[_textureDescriptor],
      );
      // TODO: except with the GPUTexture's underlying storage pointing to this.[[drawingBuffer]].
    }

    return this[_currentTexture];
  }

  [_fillCanvasBitmapHook]() {
    this[_canvas][_canvasBitmap].data = getCopyOfImageContent(this);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(GPUCanvasContextPrototype, this),
        keys: [
          "canvas",
        ],
      }),
      inspectOptions,
    );
  }
}
const GPUCanvasContextPrototype = GPUCanvasContext.prototype;

function getTextureDescriptorForCanvasAndConfiguration(canvas, configuration) {
  return {
    label: "GPUCanvasContextConfigurationTexture",
    size: [canvas[_width], canvas[_height], 1],
    format: configuration.format,
    usage: configuration.usage | GPUTextureUsage.COPY_SRC,
    viewFormats: configuration.viewFormats,
  };
}

function replaceDrawingBuffer(context) {
  expireCurrentTexture(context);



  // TODO
}

function expireCurrentTexture(context) {
  if (context[_currentTexture]) {
    context[_currentTexture].destroy();
    context[_currentTexture] = null;
  }
}

function getCopyOfImageContent(context) {
  const texture = context[_currentTexture];
  const device = context[_configuration].device;
  const { padded, unpadded } = getRowPadding(context[_canvas][_width]);

  const encoder = device.createCommandEncoder({
    label: "GPUCanvasCopyCommandEncoder"
  });
  const outputBuffer = device.createBuffer({
    label: "GPUCanvasCopyBuffer",
    size: padded * context[_canvas][_height],
    usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
  });

  encoder.copyTextureToBuffer(
    {
      texture,
    },
    {
      buffer: outputBuffer,
      bytesPerRow: padded,
    },
    {
      width: context[_canvas][_width],
      height: context[_canvas][_height],
    },
  );

  device.queue.submit([encoder.finish()]);

  const { _mapBlocking } = loadWebGPU();

  outputBuffer[_mapBlocking](1);

  const buf = new Uint8Array(unpadded * context[_canvas][_height]);
  const mappedBuffer = new Uint8Array(outputBuffer.getMappedRange());

  for (let i = 0; i < context[_canvas][_height]; i++) {
    const slice = mappedBuffer
      .slice(i * padded, (i + 1) * padded)
      .slice(0, unpadded);

    buf.set(slice, i * unpadded);
  }

  outputBuffer.unmap();
  outputBuffer.destroy();

  // TODO: alphaMode

  return buf;
}


/** Buffer-Texture copies must have [`bytes_per_row`] aligned to this number. */
export const COPY_BYTES_PER_ROW_ALIGNMENT = 256;

/** Number of bytes per pixel. */
export const BYTES_PER_PIXEL = 4;

export function getRowPadding(width) {
  // It is a WebGPU requirement that
  // GPUImageCopyBuffer.layout.bytesPerRow % COPY_BYTES_PER_ROW_ALIGNMENT == 0
  // So we calculate paddedBytesPerRow by rounding unpaddedBytesPerRow
  // up to the next multiple of COPY_BYTES_PER_ROW_ALIGNMENT.

  const unpaddedBytesPerRow = width * BYTES_PER_PIXEL;
  const paddedBytesPerRowPadding = (COPY_BYTES_PER_ROW_ALIGNMENT -
      (unpaddedBytesPerRow % COPY_BYTES_PER_ROW_ALIGNMENT)) %
    COPY_BYTES_PER_ROW_ALIGNMENT;
  const paddedBytesPerRow = unpaddedBytesPerRow + paddedBytesPerRowPadding;

  return {
    unpadded: unpaddedBytesPerRow,
    padded: paddedBytesPerRow,
  };
}

// ENUM: OffscreenRenderingContextId
webidl.converters["OffscreenRenderingContextId"] = webidl.createEnumConverter(
  "OffscreenRenderingContextId",
  ["bitmaprenderer", "webgpu"],
);

// DICT: ImageEncodeOptions
const dictImageEncodeOptions = [
  {
    key: "type",
    converter: webidl.converters.DOMString,
    defaultValue: "image/png",
  },
  {
    key: "quality",
    converter: webidl.converters["unrestricted double"],
  },
];
webidl.converters["ImageEncodeOptions"] = webidl
  .createDictionaryConverter(
    "ImageEncodeOptions",
    dictImageEncodeOptions,
  );

webidl.converters["ImageBitmap?"] = webidl.createNullableConverter(
  webidl.createInterfaceConverter("ImageBitmap", ImageBitmap.prototype),
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

export { OffscreenCanvas };
