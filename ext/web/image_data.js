// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { ImageData, op_image_data_set_data_symbol } from "ext:core/ops";
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);

// Symbol-keyed slot that holds the pixel buffer on each ImageData instance.
// Kept module-private; the Rust constructor stashes the typed array under
// this symbol and the `data` getter below reads it back, mirroring the shape
// of the original JS implementation.
const _data = Symbol("[[data]]");
op_image_data_set_data_symbol(_data);

const ImageDataPrototype = ImageData.prototype;

ObjectDefineProperty(ImageDataPrototype, "data", {
  __proto__: null,
  get: function data() {
    if (!ObjectPrototypeIsPrototypeOf(ImageDataPrototype, this)) {
      throw new TypeError("Illegal invocation");
    }
    return this[_data];
  },
  enumerable: true,
  configurable: true,
});

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
            "data",
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
