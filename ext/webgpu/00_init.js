// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
const {
  op_lazy_load_esm,
} = core.ensureFastOps(true);

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
    webgpu = op_lazy_load_esm("ext:deno_webgpu/01_webgpu.js");
  }
}

export { loadWebGPU, webgpu, webGPUNonEnumerable };
