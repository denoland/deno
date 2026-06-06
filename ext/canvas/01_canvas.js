// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
import {
  ImageBitmapRenderingContext,
  OffscreenCanvas,
  op_init_canvas,
} from "ext:core/ops";

const { Blob } = core.loadExtScript("ext:deno_web/09_file.js");

op_init_canvas(Blob);

export { ImageBitmapRenderingContext, OffscreenCanvas };
