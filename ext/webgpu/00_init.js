// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
const ops = core.ops;

let webgpu;

function webGPUNonEnumerable(getter) {
  let valueIsSet = false;
  let value;

  return {
    get() {
      loadWebGPU();

      if (valueIsSet) {
        return value;
      } else {
        return getter();
      }
    },
    set(v) {
      loadWebGPU();

      valueIsSet = true;
      value = v;
    },
    enumerable: false,
    configurable: true,
  };
}

function loadWebGPU() {
  if (!webgpu) {
    webgpu = ops.op_lazy_load_esm("ext:deno_webgpu/01_webgpu.js");
  }
}

export { loadWebGPU, webgpu, webGPUNonEnumerable };
