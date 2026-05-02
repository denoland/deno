// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { ImageData } from "ext:core/ops";
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);

const ImageDataPrototype = ImageData.prototype;

// Pixel buffer is exposed by the Rust side as a `Symbol.for("Deno_imageData_data")`
// keyed accessor (see `#[symbol(...)]` on `ImageData::get_data` in
// `image_data.rs`), matching how `ImageBitmap`'s `Deno_bitmapData` is wired.
// Wrap that in the public `data` attribute getter on the prototype.
const _data = SymbolFor("Deno_imageData_data");

ObjectDefineProperty(
  ImageDataPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value: function customInspect(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(ImageDataPrototype, this),
          keys: [
            "width",
            "height",
            "pixelFormat",
            "colorSpace",
          ],
        }),
        inspectOptions,
      );
    },
    enumerable: false,
    writable: true,
    configurable: true,
  },
);
webidl.configureInterface(ImageData);

export { ImageData, ImageDataPrototype };
