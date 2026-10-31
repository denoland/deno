// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const { ImageData } = core.ops;
const {
  ObjectDefineProperty,
  ObjectGetOwnPropertyDescriptor,
  ObjectPrototypeIsPrototypeOf,
  ReflectApply,
  SymbolFor,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);

const ImageDataPrototype = ImageData.prototype;
const getData = ObjectGetOwnPropertyDescriptor(ImageDataPrototype, "data").get;
const getWidth =
  ObjectGetOwnPropertyDescriptor(ImageDataPrototype, "width").get;
const getHeight =
  ObjectGetOwnPropertyDescriptor(ImageDataPrototype, "height").get;
const getColorSpace = ObjectGetOwnPropertyDescriptor(
  ImageDataPrototype,
  "colorSpace",
).get;
const getPixelFormat = ObjectGetOwnPropertyDescriptor(
  ImageDataPrototype,
  "pixelFormat",
).get;

ObjectDefineProperty(ImageDataPrototype, core.hostObjectBrand, {
  __proto__: null,
  value() {
    const get = (getter) => ReflectApply(getter, this, []);
    return {
      type: "ImageData",
      data: get(getData),
      width: get(getWidth),
      height: get(getHeight),
      colorSpace: get(getColorSpace),
      pixelFormat: get(getPixelFormat),
    };
  },
});

core.registerCloneableResource("ImageData", (data) => {
  return new ImageData(data.data, data.width, data.height, {
    colorSpace: data.colorSpace,
    pixelFormat: data.pixelFormat,
  });
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

return { ImageData, ImageDataPrototype };
})();
