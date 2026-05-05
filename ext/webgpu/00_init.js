// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = globalThis.__bootstrap;

const loadWebGPU = core.createLazyLoader("ext:deno_webgpu/01_webgpu.js");

return { loadWebGPU };
})();
