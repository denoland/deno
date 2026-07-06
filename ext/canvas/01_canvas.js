// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
import {
  ImageBitmapRenderingContext,
  OffscreenCanvas,
  op_init_canvas,
} from "ext:core/ops";

const { Blob } = core.loadExtScript("ext:deno_web/09_file.js");
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");

op_init_canvas(Blob);

webidl.configureInterface(ImageBitmapRenderingContext);
webidl.configureInterface(OffscreenCanvas);
// `OffscreenCanvas.getContext()` constructs `OffscreenCanvasRenderingContext2D`
// (and its associated `CanvasGradient`/`CanvasPattern`/`CanvasFilter`/
// `Path2D`/`TextMetrics`) directly from Rust, bypassing the lazy-loaded
// `18_canvas2d.js` module that normally installs `Symbol.toStringTag` and a
// non-writable `prototype` on those classes via `configureInterface`. Load it
// eagerly here so those classes are configured before any context exists.
core.loadExtScript("ext:deno_web/18_canvas2d.js");

export { ImageBitmapRenderingContext, OffscreenCanvas };
