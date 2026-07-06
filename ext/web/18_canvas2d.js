// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  op_fontdb_load_local_fonts,
  op_fontdb_local_font_data,
  op_fontdb_query_local_fonts,
  CanvasFilter,
  CanvasGradient,
  CanvasPattern,
  OffscreenCanvasRenderingContext2D,
  Path2D,
  TextMetrics,
} = core.ops;

const {
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  SafeArrayIterator,
  Symbol,
  SymbolFor,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);
const { markNotSerializable } = core.loadExtScript(
  "ext:deno_web/13_message_port.js",
);

const illegalConstructorKey = Symbol("illegalConstructorKey");

webidl.configureInterface(TextMetrics);
webidl.configureInterface(CanvasFilter);
webidl.configureInterface(CanvasGradient);
webidl.configureInterface(CanvasPattern);
webidl.configureInterface(OffscreenCanvasRenderingContext2D);
webidl.configureInterface(Path2D);

ObjectDefineProperty(
  TextMetrics.prototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(TextMetrics.prototype, this),
          keys: [
            "width",
            "actualBoundingBoxLeft",
            "actualBoundingBoxRight",
            "fontBoundingBoxAscent",
            "fontBoundingBoxDescent",
            "actualBoundingBoxAscent",
            "actualBoundingBoxDescent",
            "emHeightAscent",
            "emHeightDescent",
            "hangingBaseline",
            "alphabeticBaseline",
            "ideographicBaseline",
          ],
        }),
        inspectOptions,
      );
    },
    enumerable: true,
    configurable: true,
    writable: true,
  },
);

ObjectDefineProperty(
  OffscreenCanvasRenderingContext2D.prototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(
            OffscreenCanvasRenderingContext2D.prototype,
            this,
          ),
          keys: [
            "canvas",
            "fillStyle",
            "strokeStyle",
            "globalAlpha",
            "font",
            "textAlign",
            "textBaseline",
            "globalCompositeOperation",
            "filter",
            "imageSmoothingEnabled",
            "imageSmoothingQuality",
            "lineWidth",
            "lineCap",
            "lineJoin",
            "miterLimit",
            "lineDashOffset",
            "shadowBlur",
            "shadowColor",
            "shadowOffsetX",
            "shadowOffsetY",
          ],
        }),
        inspectOptions,
      );
    },
    enumerable: true,
    configurable: true,
    writable: true,
  },
);

function loadLocalFonts() {
  return op_fontdb_load_local_fonts();
}

let _fileMod;
const loadFile = () =>
  _fileMod ??
    (_fileMod = core.loadExtScript("ext:deno_web/09_file.js"));

class FontData {
  [webidl.brand] = webidl.brand;

  /** @type {string} */
  #postscriptName;
  /** @type {string} */
  #fullName;
  /** @type {string} */
  #family;
  /** @type {string} */
  #style;

  constructor(key, postscriptName, fullName, family, style) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
    this.#postscriptName = postscriptName;
    this.#fullName = fullName;
    this.#family = family;
    this.#style = style;
  }

  get postscriptName() {
    webidl.assertBranded(this, FontDataPrototype);
    return this.#postscriptName;
  }

  get fullName() {
    webidl.assertBranded(this, FontDataPrototype);
    return this.#fullName;
  }

  get family() {
    webidl.assertBranded(this, FontDataPrototype);
    return this.#family;
  }

  get style() {
    webidl.assertBranded(this, FontDataPrototype);
    return this.#style;
  }

  async blob() {
    webidl.assertBranded(this, FontDataPrototype);
    const data = await op_fontdb_local_font_data(this.#postscriptName);
    const { Blob } = loadFile();
    return new Blob([data], { type: "application/octet-stream" });
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(FontDataPrototype, this),
        keys: [
          "postscriptName",
          "fullName",
          "family",
          "style",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(FontData);
const FontDataPrototype = FontData.prototype;
markNotSerializable(FontDataPrototype);

async function queryLocalFonts(options = { __proto__: null }) {
  let postscriptNames = null;
  if (options !== undefined && options !== null) {
    if (options.postscriptNames !== undefined) {
      const prefix = "Failed to execute 'queryLocalFonts'";
      postscriptNames = [];
      for (const name of new SafeArrayIterator(options.postscriptNames)) {
        ArrayPrototypePush(
          postscriptNames,
          webidl.converters.DOMString(
            name,
            prefix,
            "postscriptNames element",
          ),
        );
      }
    }
  }

  const results = await op_fontdb_query_local_fonts(postscriptNames);
  return ArrayPrototypeMap(results, (info) =>
    new FontData(
      illegalConstructorKey,
      info.postscriptName,
      info.fullName,
      info.family,
      info.style,
    ));
}

return {
  CanvasFilter,
  CanvasGradient,
  CanvasPattern,
  FontData,
  FontDataPrototype,
  OffscreenCanvasRenderingContext2D,
  Path2D,
  loadLocalFonts,
  queryLocalFonts,
  TextMetrics,
};
})();
