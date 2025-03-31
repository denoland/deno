// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_webgpu.d.ts" />

import { primordials } from "ext:core/mod.js";
import { GPUCanvasContext, UnsafeWindowSurface } from "ext:core/ops";
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
} = primordials;
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";

ObjectDefineProperty(GPUCanvasContext, SymbolFor("Deno.privateCustomInspect"), {
  __proto__: null,
  value(inspect, inspectOptions) {
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
  },
});
const GPUCanvasContextPrototype = GPUCanvasContext.prototype;

export { GPUCanvasContext, UnsafeWindowSurface };
