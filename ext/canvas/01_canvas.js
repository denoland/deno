// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  ImageBitmapRenderingContext,
  OffscreenCanvas,
  op_init_canvas,
  op_offscreen_canvas_receive,
  op_offscreen_canvas_transfer,
} from "ext:core/ops";

const { Blob } = core.loadExtScript("ext:deno_web/09_file.js");
const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");
const OffscreenCanvasPrototype = OffscreenCanvas.prototype;
primordials.ObjectDefineProperty(
  OffscreenCanvasPrototype,
  core.hostObjectBrand,
  {
    __proto__: null,
    value: "OffscreenCanvas",
  },
);
core.registerTransferableResource(
  "OffscreenCanvas",
  (canvas) => {
    try {
      return op_offscreen_canvas_transfer(canvas);
    } catch (error) {
      throw new DOMException(error.message, "DataCloneError");
    }
  },
  (rid) => op_offscreen_canvas_receive(rid),
);

op_init_canvas(Blob);

export { ImageBitmapRenderingContext, OffscreenCanvas };
