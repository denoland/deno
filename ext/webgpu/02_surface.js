// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./lib.deno_webgpu.d.ts" />

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
const { Symbol, SymbolFor, ObjectPrototypeIsPrototypeOf } = primordials;
import {
  _device,
  assertDevice,
  createGPUTexture,
  GPUTextureUsage,
} from "ext:deno_webgpu/01_webgpu.js";

const _surfaceRid = Symbol("[[surfaceRid]]");
const _configuration = Symbol("[[configuration]]");
const _canvas = Symbol("[[canvas]]");
const _currentTexture = Symbol("[[currentTexture]]");
class GPUCanvasContext {
  /** @type {number} */
  [_surfaceRid];
  /** @type {InnerGPUDevice} */
  [_device];
  [_configuration];
  [_canvas];
  /** @type {GPUTexture | undefined} */
  [_currentTexture];

  get canvas() {
    webidl.assertBranded(this, GPUCanvasContextPrototype);
    return this[_canvas];
  }

  constructor() {
    webidl.illegalConstructor();
  }

  configure(configuration) {
    webidl.assertBranded(this, GPUCanvasContextPrototype);
    const prefix = "Failed to execute 'configure' on 'GPUCanvasContext'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    configuration = webidl.converters.GPUCanvasConfiguration(configuration, {
      prefix,
      context: "Argument 1",
    });

    this[_device] = configuration.device[_device];
    this[_configuration] = configuration;
    const device = assertDevice(this, {
      prefix,
      context: "configuration.device",
    });

    const { err } = ops.op_webgpu_surface_configure({
      surfaceRid: this[_surfaceRid],
      deviceRid: device.rid,
      format: configuration.format,
      viewFormats: configuration.viewFormats,
      usage: configuration.usage,
      width: configuration.width,
      height: configuration.height,
      alphaMode: configuration.alphaMode,
    });

    device.pushError(err);
  }

  unconfigure() {
    webidl.assertBranded(this, GPUCanvasContextPrototype);

    this[_configuration] = null;
    this[_device] = null;
  }

  getCurrentTexture() {
    webidl.assertBranded(this, GPUCanvasContextPrototype);
    const prefix =
      "Failed to execute 'getCurrentTexture' on 'GPUCanvasContext'";

    if (this[_configuration] === null) {
      throw new DOMException("context is not configured.", "InvalidStateError");
    }

    const device = assertDevice(this, { prefix, context: "this" });

    if (this[_currentTexture]) {
      return this[_currentTexture];
    }

    const { rid } = ops.op_webgpu_surface_get_current_texture(
      device.rid,
      this[_surfaceRid],
    );

    const texture = createGPUTexture(
      {
        size: {
          width: this[_configuration].width,
          height: this[_configuration].height,
          depthOrArrayLayers: 1,
        },
        mipLevelCount: 1,
        sampleCount: 1,
        dimension: "2d",
        format: this[_configuration].format,
        usage: this[_configuration].usage,
      },
      device,
      rid,
    );
    device.trackResource(texture);
    this[_currentTexture] = texture;
    return texture;
  }

  // Extended from spec. Required to present the texture; browser don't need this.
  present() {
    webidl.assertBranded(this, GPUCanvasContextPrototype);
    const prefix = "Failed to execute 'present' on 'GPUCanvasContext'";
    const device = assertDevice(this[_currentTexture], {
      prefix,
      context: "this",
    });
    ops.op_webgpu_surface_present(device.rid, this[_surfaceRid]);
    this[_currentTexture].destroy();
    this[_currentTexture] = undefined;
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

function createCanvasContext(options) {
  const canvasContext = webidl.createBranded(GPUCanvasContext);
  canvasContext[_surfaceRid] = options.surfaceRid;
  canvasContext[_canvas] = options.canvas;
  return canvasContext;
}

// Converters

// ENUM: GPUCanvasAlphaMode
webidl.converters["GPUCanvasAlphaMode"] = webidl.createEnumConverter(
  "GPUCanvasAlphaMode",
  [
    "opaque",
    "premultiplied",
  ],
);

// NON-SPEC: ENUM: GPUPresentMode
webidl.converters["GPUPresentMode"] = webidl.createEnumConverter(
  "GPUPresentMode",
  [
    "autoVsync",
    "autoNoVsync",
    "fifo",
    "fifoRelaxed",
    "immediate",
    "mailbox",
  ],
);

// DICT: GPUCanvasConfiguration
const dictMembersGPUCanvasConfiguration = [
  { key: "device", converter: webidl.converters.GPUDevice, required: true },
  {
    key: "format",
    converter: webidl.converters.GPUTextureFormat,
    required: true,
  },
  {
    key: "usage",
    converter: webidl.converters["GPUTextureUsageFlags"],
    defaultValue: GPUTextureUsage.RENDER_ATTACHMENT,
  },
  {
    key: "alphaMode",
    converter: webidl.converters["GPUCanvasAlphaMode"],
    defaultValue: "opaque",
  },

  // Extended from spec
  {
    key: "presentMode",
    converter: webidl.converters["GPUPresentMode"],
  },
  {
    key: "width",
    converter: webidl.converters["long"],
    required: true,
  },
  {
    key: "height",
    converter: webidl.converters["long"],
    required: true,
  },
  {
    key: "viewFormats",
    converter: webidl.createSequenceConverter(
      webidl.converters["GPUTextureFormat"],
    ),
    get defaultValue() {
      return [];
    },
  },
];
webidl.converters["GPUCanvasConfiguration"] = webidl
  .createDictionaryConverter(
    "GPUCanvasConfiguration",
    dictMembersGPUCanvasConfiguration,
  );

window.__bootstrap.webgpu = {
  ...window.__bootstrap.webgpu,
  GPUCanvasContext,
  createCanvasContext,
};
