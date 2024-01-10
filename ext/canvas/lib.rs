// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  esm = ["01_image.js"],
);
