// Copyright 2018-2025 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";

const loadWebGPU = core.createLazyLoader("ext:deno_webgpu/01_webgpu.js");

export { loadWebGPU };
