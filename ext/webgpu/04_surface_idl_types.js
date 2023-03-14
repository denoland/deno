// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./lib.deno_webgpu.d.ts" />

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { GPUTextureUsage } from "ext:deno_webgpu/01_webgpu.js";

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
