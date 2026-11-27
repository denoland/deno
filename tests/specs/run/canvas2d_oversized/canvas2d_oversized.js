// Copyright 2018-2026 the Deno authors. MIT license.

const oversized = new OffscreenCanvas(70000, 100);
console.log(oversized.getContext("2d"));

const ok = new OffscreenCanvas(65535, 65535);
console.log(ok.getContext("2d") !== null);
