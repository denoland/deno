// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  op_fontdb_add,
  op_fontdb_load,
  op_fontdb_load_local,
  op_fontdb_load_object_url,
  op_fontdb_load_resource,
  op_fontdb_local_font_data,
  op_fontdb_query_local_fonts,
  op_fontdb_register_all_local_fonts,
  op_fontdb_remove,
  op_fontdb_unload,
  op_match_font_faces,
  op_normalize_font_face_family,
  op_parse_css_font_src,
  op_parse_css_font_weight,
  op_parse_css_font_width,
  CanvasGradient,
  CanvasPattern,
  OffscreenCanvasRenderingContext2D,
  Path2D,
  TextMetrics,
} = core.ops;

const {
  ArrayBufferIsView,
  ArrayBufferPrototype,
  ArrayPrototypeEvery,
  ArrayPrototypeFilter,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  FunctionPrototypeCall,
  NumberIsFinite,
  NumberParseInt,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectGetOwnPropertyDescriptor,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  PromiseReject,
  PromiseResolve,
  RegExpPrototypeExec,
  SafeArrayIterator,
  SafeFinalizationRegistry,
  SafePromiseAll,
  SafeRegExp,
  SafeSet,
  SafeSetIterator,
  SetPrototypeAdd,
  SetPrototypeClear,
  SetPrototypeDelete,
  SetPrototypeHas,
  String,
  StringPrototypeCodePointAt,
  StringPrototypeIncludes,
  StringPrototypeReplace,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeTrim,
  Symbol,
  SymbolFor,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  TypedArrayPrototypeSlice,
  TypeError,
  Uint8Array,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { EventTarget, Event, defineEventHandler } = core.loadExtScript(
  "ext:deno_web/02_event.js",
);
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);
const { DOMException } = core.loadExtScript(
  "ext:deno_web/01_dom_exception.js",
);
const { markNotSerializable } = core.loadExtScript(
  "ext:deno_web/13_message_port.js",
);
const { getReadableStreamResourceBacking, readableStreamCollectWithOp } = core
  .loadExtScript("ext:deno_web/06_streams.js");

let _fileMod;
const loadFile = () =>
  _fileMod ??
    (_fileMod = core.loadExtScript("ext:deno_web/09_file.js"));

// Lazy: deno_web is below deno_fetch; only FontFace url() needs fetch.
let _fetchMod;
const loadFetch = () =>
  _fetchMod ??
    (_fetchMod = core.loadExtScript("ext:deno_fetch/26_fetch.js"));

const CSS_FONT_STYLE_VALUES = new SafeSet(["normal", "italic", "oblique"]);

// https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-display
const CSS_FONT_DISPLAY_VALUES = new SafeSet([
  "auto",
  "block",
  "swap",
  "fallback",
  "optional",
]);

// U+XXXX | U+XXXX-YYYY | U+XX??
// https://drafts.csswg.org/css-fonts-4/#descdef-font-face-unicode-range
const UNICODE_RANGE_SINGLE_RE = new SafeRegExp(
  /^\s*[Uu]\+[0-9A-Fa-f?]{1,6}(?:-[0-9A-Fa-f]{1,6})?\s*$/,
);

// "<tag>" [<integer> | on | off]?  (tag: 4 printable ASCII)
// https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-feature-settings
const FONT_FEATURE_SETTINGS_ITEM_RE = new SafeRegExp(
  /^\s*"[\x20-\x7E]{4}"\s*(?:\d+|on|off)?\s*$/,
);

// "<tag>" <number>  (tag: 4 printable ASCII)
// https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-variation-settings
const FONT_VARIATION_SETTINGS_ITEM_RE = new SafeRegExp(
  /^\s*"[\x20-\x7E]{4}"\s+[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?\s*$/,
);

// normal | <percentage>
// https://drafts.csswg.org/css-fonts-4/#descdef-font-face-ascent-override
const METRIC_OVERRIDE_VALUE_RE = new SafeRegExp(/^\d+(?:\.\d+)?%$/);

function isValidFontStyle(v) {
  return SetPrototypeHas(CSS_FONT_STYLE_VALUES, v);
}

function isValidFontWeight(v) {
  return op_parse_css_font_weight(v) !== -1;
}

function isValidFontWidth(v) {
  return NumberIsFinite(op_parse_css_font_width(v));
}

/**
 * Valid CSS unicode-range descriptor?
 * @param {string} v
 * @returns {boolean}
 * @see https://drafts.csswg.org/css-fonts-4/#descdef-font-face-unicode-range
 */
function isValidUnicodeRange(v) {
  const parts = StringPrototypeSplit(v, ",");
  return ArrayPrototypeEvery(
    parts,
    (p) => RegExpPrototypeExec(UNICODE_RANGE_SINGLE_RE, p) !== null,
  );
}

/**
 * Valid CSS font-feature-settings descriptor?
 * @param {string} v
 * @returns {boolean}
 * @see https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-feature-settings
 */
function isValidFontFeatureSettings(v) {
  const trimmed = StringPrototypeTrim(v);
  if (trimmed === "normal") return true;
  const parts = StringPrototypeSplit(trimmed, ",");
  return ArrayPrototypeEvery(
    parts,
    (p) => RegExpPrototypeExec(FONT_FEATURE_SETTINGS_ITEM_RE, p) !== null,
  );
}

/**
 * Valid CSS font-variation-settings descriptor?
 * @param {string} v
 * @returns {boolean}
 * @see https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-variation-settings
 */
function isValidFontVariationSettings(v) {
  const trimmed = StringPrototypeTrim(v);
  if (trimmed === "normal") return true;
  const parts = StringPrototypeSplit(trimmed, ",");
  return ArrayPrototypeEvery(
    parts,
    (p) => RegExpPrototypeExec(FONT_VARIATION_SETTINGS_ITEM_RE, p) !== null,
  );
}

/**
 * Valid CSS ascent/descent/line-gap-override?
 * @param {string} v
 * @returns {boolean}
 * @see https://drafts.csswg.org/css-fonts-4/#descdef-font-face-ascent-override
 */
function isValidMetricOverride(v) {
  const trimmed = StringPrototypeTrim(v);
  return trimmed === "normal" ||
    RegExpPrototypeExec(METRIC_OVERRIDE_VALUE_RE, trimmed) !== null;
}

const EVENT_PROPS = [
  "bubbles",
  "cancelable",
  "composed",
  "currentTarget",
  "defaultPrevented",
  "eventPhase",
  "srcElement",
  "target",
  "returnValue",
  "timeStamp",
  "type",
];

const kAddToSystem = Symbol("kAddToSystem");
const kRemoveFromSystem = Symbol("kRemoveFromSystem");
const kFireBatchResult = Symbol("kFireBatchResult");
const kUnicodeRangeCoversText = Symbol("kUnicodeRangeCoversText");
const illegalConstructorKey = Symbol("illegalConstructorKey");

const FONT_HANDLE_REGISTRY = new SafeFinalizationRegistry((handle) => {
  op_fontdb_unload(handle);
});

// Capturing form of a unicode-range token.
const UNICODE_RANGE_TOKEN_RE = new SafeRegExp(
  /^\s*[Uu]\+([0-9A-Fa-f?]+)(?:-([0-9A-Fa-f]+))?\s*$/,
);
// Safe with /g: StringPrototypeReplace resets lastIndex each call.
const UNICODE_RANGE_WILDCARD_RE = new SafeRegExp(/\?/g);

/**
 * True if unicode-range covers any codepoint in text.
 * @param {string} unicodeRange
 * @param {string} text
 * @returns {boolean}
 */
function unicodeRangeCoversText(unicodeRange, text) {
  if (!unicodeRange || unicodeRange === "U+0-10FFFF") return true;
  const ranges = StringPrototypeSplit(unicodeRange, ",");
  for (let i = 0; i < text.length;) {
    const cp = StringPrototypeCodePointAt(text, i);
    for (let j = 0; j < ranges.length; ++j) {
      const m = RegExpPrototypeExec(UNICODE_RANGE_TOKEN_RE, ranges[j]);
      if (!m) continue;
      let lo, hi;
      if (m[2]) {
        lo = NumberParseInt(m[1], 16);
        hi = NumberParseInt(m[2], 16);
      } else if (StringPrototypeIncludes(m[1], "?")) {
        lo = NumberParseInt(
          StringPrototypeReplace(m[1], UNICODE_RANGE_WILDCARD_RE, "0"),
          16,
        );
        hi = NumberParseInt(
          StringPrototypeReplace(m[1], UNICODE_RANGE_WILDCARD_RE, "F"),
          16,
        );
      } else {
        lo = hi = NumberParseInt(m[1], 16);
      }
      if (cp >= lo && cp <= hi) return true;
    }
    i += cp > 0xFFFF ? 2 : 1;
  }
  return false;
}

/**
 * True if sorted [[start, end], ...] coverage hits any codepoint in text.
 * @param {number[][]} coverage
 * @param {string} text
 * @returns {boolean}
 */
function fontCoverageCoversText(coverage, text) {
  for (let i = 0; i < text.length;) {
    const cp = StringPrototypeCodePointAt(text, i);
    let lo = 0;
    let hi = coverage.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (cp < coverage[mid][0]) hi = mid - 1;
      else if (cp > coverage[mid][1]) lo = mid + 1;
      else return true;
    }
    i += cp > 0xFFFF ? 2 : 1;
  }
  return false;
}

/**
 * @param {Set<FontFace>} set
 * @param {string} font
 * @param {string} text
 * @returns {{ faces: FontFace[], foundFacesFlag: boolean, parseError: boolean }}
 * @see https://drafts.csswg.org/css-font-loading/#find-the-matching-font-faces
 */
function matchFontFaces(set, font, text) {
  const faceList = [];
  for (const face of new SafeSetIterator(set)) {
    ArrayPrototypePush(faceList, face);
  }
  const result = op_match_font_faces(
    font,
    ArrayPrototypeMap(faceList, (face) => ({
      family: face.family,
      style: face.style,
      weight: face.weight,
      width: face.width,
    })),
  );
  if (!result) return { faces: [], foundFacesFlag: false, parseError: true };

  const matched = ArrayPrototypeMap(
    result.indices,
    (i) => faceList[i],
  );
  const faces = ArrayPrototypeFilter(
    matched,
    (face) => face[kUnicodeRangeCoversText](text),
  );
  return {
    faces,
    foundFacesFlag: result.found,
    parseError: false,
  };
}

/**
 * @see https://drafts.csswg.org/css-font-loading/#fontface-interface
 */
class FontFace {
  [webidl.brand] = webidl.brand;

  /** @type {string} */
  #family;
  /** @type {"normal" | "italic" | "oblique"} */
  #style = "normal";
  /** @type {string} */
  #weight = "normal";
  /** @type {string} */
  #width = "normal";
  /** @type {string} */
  #unicodeRange = "U+0-10FFFF";
  /** @type {string} */
  #featureSettings = "normal";
  /** @type {string} */
  #variationSettings = "normal";
  /** @type {string} */
  #display = "auto";
  /** @type {string} */
  #ascentOverride = "normal";
  /** @type {string} */
  #descentOverride = "normal";
  /** @type {string} */
  #lineGapOverride = "normal";

  // Exactly one of #bytes / #srcList is set (BufferSource vs CSS src string).
  /** @type {Uint8Array | null} */
  #bytes = null;
  /** @type {{ local: boolean, value: string }[] | null} */
  #srcList = null;
  /** @type {"unloaded" | "loading" | "loaded" | "error"} */
  #status = "unloaded";
  /** @type {number | null} */
  #handle = null;
  /** @type {Promise<FontFace> | null} */
  #loadPromise = null;
  /** @type {unknown} */
  #loadError = null;

  /**
   * Set status "error" (constructor never throws; load() rejects).
   * @param {string} message
   */
  #invalidate(message) {
    if (this.#status === "error") return;
    this.#status = "error";
    this.#loadError = new DOMException(message, "SyntaxError");
  }

  // Caller-set descriptors; file metadata fills only the unset ones.
  /** @type {boolean} */
  #styleUserSet = false;
  /** @type {boolean} */
  #weightUserSet = false;
  /** @type {boolean} */
  #widthUserSet = false;
  /** @type {boolean} */
  #unicodeRangeUserSet = false;

  // Post-load cmap ranges ([[start,end],...]); null => treat as U+0-10FFFF.
  /** @type {[number, number][] | null} */
  #fontFileCoverage = null;

  // Lazy [SameObject] slots.
  /** @type {FontFaceFeatures | null} */
  #features = null;
  /** @type {FontFaceVariations | null} */
  #variations = null;
  /** @type {FontFacePalettes | null} */
  #palettes = null;

  constructor(family, source, descriptors = { __proto__: null }) {
    const prefix = "Failed to construct 'FontFace'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    family = webidl.converters.DOMString(family, prefix, "Argument 1");

    // Invalid/generic names are quoted, not rejected.
    // https://github.com/w3c/csswg-drafts/issues/6236
    this.#family = op_normalize_font_face_family(family);

    if (typeof source === "string") {
      const srcList = op_parse_css_font_src(source);
      if (srcList === null) {
        this.#invalidate(
          `${prefix}: Could not parse the source as a CSS src descriptor.`,
        );
      } else {
        this.#srcList = srcList;
      }
    } else if (
      !ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, source) &&
      !ArrayBufferIsView(source)
    ) {
      throw new TypeError(
        `${prefix}: source must be a string, ArrayBuffer, or ArrayBufferView.`,
      );
    } else {
      // Own the data: the spec stores a copy, and op_fontdb_load detaches it.
      this.#bytes = TypedArrayPrototypeSlice(
        ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, source)
          ? new Uint8Array(source)
          : new Uint8Array(
            TypedArrayPrototypeGetBuffer(source),
            TypedArrayPrototypeGetByteOffset(source),
            TypedArrayPrototypeGetByteLength(source),
          ),
      );
    }

    if (descriptors.style !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.style,
        prefix,
        "descriptors.style",
      );
      if (isValidFontStyle(v)) {
        this.#style = v;
        this.#styleUserSet = true;
      } else {
        this.#invalidate(`${prefix}: Invalid value for 'style' descriptor.`);
      }
    }
    if (descriptors.weight !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.weight,
        prefix,
        "descriptors.weight",
      );
      if (isValidFontWeight(v)) {
        this.#weight = v;
        this.#weightUserSet = true;
      } else {
        this.#invalidate(`${prefix}: Invalid value for 'weight' descriptor.`);
      }
    }
    // `width` wins when both it and legacy `stretch` are set.
    const widthDescriptor = descriptors.width !== undefined
      ? descriptors.width
      : descriptors.stretch;
    if (widthDescriptor !== undefined) {
      const v = webidl.converters.DOMString(
        widthDescriptor,
        prefix,
        descriptors.width !== undefined
          ? "descriptors.width"
          : "descriptors.stretch",
      );
      if (isValidFontWidth(v)) {
        this.#width = v;
        this.#widthUserSet = true;
      } else {
        this.#invalidate(
          `${prefix}: Invalid value for '${
            descriptors.width !== undefined ? "width" : "stretch"
          }' descriptor.`,
        );
      }
    }
    if (descriptors.unicodeRange !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.unicodeRange,
        prefix,
        "descriptors.unicodeRange",
      );
      if (isValidUnicodeRange(v)) {
        this.#unicodeRange = v;
        this.#unicodeRangeUserSet = true;
      } else {
        this.#invalidate(
          `${prefix}: Invalid value for 'unicodeRange' descriptor.`,
        );
      }
    }
    if (descriptors.featureSettings !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.featureSettings,
        prefix,
        "descriptors.featureSettings",
      );
      if (isValidFontFeatureSettings(v)) {
        this.#featureSettings = v;
      } else {
        this.#invalidate(
          `${prefix}: Invalid value for 'featureSettings' descriptor.`,
        );
      }
    }
    if (descriptors.variationSettings !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.variationSettings,
        prefix,
        "descriptors.variationSettings",
      );
      if (isValidFontVariationSettings(v)) {
        this.#variationSettings = v;
      } else {
        this.#invalidate(
          `${prefix}: Invalid value for 'variationSettings' descriptor.`,
        );
      }
    }
    if (descriptors.display !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.display,
        prefix,
        "descriptors.display",
      );
      if (SetPrototypeHas(CSS_FONT_DISPLAY_VALUES, v)) {
        this.#display = v;
      } else {
        this.#invalidate(`${prefix}: Invalid value for 'display' descriptor.`);
      }
    }
    if (descriptors.ascentOverride !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.ascentOverride,
        prefix,
        "descriptors.ascentOverride",
      );
      if (isValidMetricOverride(v)) {
        this.#ascentOverride = v;
      } else {
        this.#invalidate(
          `${prefix}: Invalid value for 'ascentOverride' descriptor.`,
        );
      }
    }
    if (descriptors.descentOverride !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.descentOverride,
        prefix,
        "descriptors.descentOverride",
      );
      if (isValidMetricOverride(v)) {
        this.#descentOverride = v;
      } else {
        this.#invalidate(
          `${prefix}: Invalid value for 'descentOverride' descriptor.`,
        );
      }
    }
    if (descriptors.lineGapOverride !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.lineGapOverride,
        prefix,
        "descriptors.lineGapOverride",
      );
      if (isValidMetricOverride(v)) {
        this.#lineGapOverride = v;
      } else {
        this.#invalidate(
          `${prefix}: Invalid value for 'lineGapOverride' descriptor.`,
        );
      }
    }
  }

  get family() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#family;
  }

  set family(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'family' on 'FontFace'",
      "Value",
    );
    // Same quoting rule as the constructor.
    // https://github.com/w3c/csswg-drafts/issues/6236
    this.#family = op_normalize_font_face_family(v);
  }

  get style() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#style;
  }

  set style(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'style' on 'FontFace'",
      "Value",
    );
    if (!isValidFontStyle(v)) {
      throw new DOMException(
        "Failed to set 'style' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#style = v;
    this.#styleUserSet = true;
  }

  get weight() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#weight;
  }

  set weight(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'weight' on 'FontFace'",
      "Value",
    );
    if (!isValidFontWeight(v)) {
      throw new DOMException(
        "Failed to set 'weight' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#weight = v;
    this.#weightUserSet = true;
  }

  get stretch() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#width;
  }

  set stretch(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'stretch' on 'FontFace'",
      "Value",
    );
    if (!isValidFontWidth(v)) {
      throw new DOMException(
        "Failed to set 'stretch' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#width = v;
    this.#widthUserSet = true;
  }

  get width() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#width;
  }

  set width(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'width' on 'FontFace'",
      "Value",
    );
    if (!isValidFontWidth(v)) {
      throw new DOMException(
        "Failed to set 'width' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#width = v;
    this.#widthUserSet = true;
  }

  get unicodeRange() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#unicodeRange;
  }

  set unicodeRange(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'unicodeRange' on 'FontFace'",
      "Value",
    );
    if (!isValidUnicodeRange(v)) {
      throw new DOMException(
        "Failed to set 'unicodeRange' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#unicodeRange = v;
    this.#unicodeRangeUserSet = true;
  }

  get featureSettings() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#featureSettings;
  }

  set featureSettings(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'featureSettings' on 'FontFace'",
      "Value",
    );
    if (!isValidFontFeatureSettings(v)) {
      throw new DOMException(
        "Failed to set 'featureSettings' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#featureSettings = v;
  }

  get variationSettings() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#variationSettings;
  }

  set variationSettings(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'variationSettings' on 'FontFace'",
      "Value",
    );
    if (!isValidFontVariationSettings(v)) {
      throw new DOMException(
        "Failed to set 'variationSettings' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#variationSettings = v;
  }

  get display() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#display;
  }

  set display(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'display' on 'FontFace'",
      "Value",
    );
    if (!SetPrototypeHas(CSS_FONT_DISPLAY_VALUES, v)) {
      throw new DOMException(
        "Failed to set 'display' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#display = v;
  }

  get ascentOverride() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#ascentOverride;
  }

  set ascentOverride(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'ascentOverride' on 'FontFace'",
      "Value",
    );
    if (!isValidMetricOverride(v)) {
      throw new DOMException(
        "Failed to set 'ascentOverride' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#ascentOverride = v;
  }

  get descentOverride() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#descentOverride;
  }

  set descentOverride(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'descentOverride' on 'FontFace'",
      "Value",
    );
    if (!isValidMetricOverride(v)) {
      throw new DOMException(
        "Failed to set 'descentOverride' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#descentOverride = v;
  }

  get lineGapOverride() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#lineGapOverride;
  }

  set lineGapOverride(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'lineGapOverride' on 'FontFace'",
      "Value",
    );
    if (!isValidMetricOverride(v)) {
      throw new DOMException(
        "Failed to set 'lineGapOverride' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#lineGapOverride = v;
  }

  get status() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#status;
  }

  get loaded() {
    // Promise attrs reject (not throw) on brand failure.
    try {
      webidl.assertBranded(this, FontFacePrototype);
    } catch (e) {
      return PromiseReject(e);
    }
    return this.load();
  }

  // Not async: reuse #loadPromise for identity stability (load() === load()).
  load() {
    try {
      webidl.assertBranded(this, FontFacePrototype);
    } catch (e) {
      return PromiseReject(e);
    }
    if (this.#status === "loaded") return PromiseResolve(this);
    if (this.#status === "error") return PromiseReject(this.#loadError);
    if (this.#loadPromise !== null) return this.#loadPromise;

    this.#status = "loading";
    this.#loadPromise = (async () => {
      try {
        const { handle, weight, style, width, unicodeCoverage } = await this
          .#loadSource();
        this.#handle = handle;
        FONT_HANDLE_REGISTRY.register(this, handle, this);
        // File metadata only for descriptors the caller did not set.
        if (!this.#weightUserSet) this.#weight = String(weight);
        if (!this.#styleUserSet) this.#style = style;
        if (!this.#widthUserSet) this.#width = width;
        this.#fontFileCoverage = unicodeCoverage;
        this.#status = "loaded";
        return this;
      } catch (e) {
        this.#status = "error";
        this.#loadError = e;
        this.#loadPromise = null;
        throw e;
      }
    })();

    return this.#loadPromise;
  }

  get features() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#features ??= new FontFaceFeatures(illegalConstructorKey);
  }

  get variations() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#variations ??= new FontFaceVariations(illegalConstructorKey);
  }

  get palettes() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#palettes ??= new FontFacePalettes(illegalConstructorKey);
  }

  /**
   * Try src entries in order; first usable font wins (missing local() skips).
   * @returns {Promise<{ handle: number, weight: number, style: string, width: string, unicodeCoverage: [number, number][] }>}
   */
  async #loadSource() {
    if (this.#bytes !== null) {
      // op_fontdb_load detaches, so drop the now-empty view. load() is
      // memoized by #loadPromise / #status, so it is never read again.
      const bytes = this.#bytes;
      this.#bytes = null;
      return await op_fontdb_load(bytes);
    }
    let firstError = null;
    const srcList = this.#srcList;
    for (let i = 0; i < srcList.length; ++i) {
      const src = srcList[i];
      try {
        const result = src.local
          ? await op_fontdb_load_local(src.value)
          : await this.#loadUrl(src.value);
        // Missing local() falls through to the next src entry.
        if (result !== null) return result;
      } catch (e) {
        firstError ??= e;
      }
    }
    if (firstError !== null) throw firstError;
    // No usable source => NetworkError.
    throw new DOMException(
      "Failed to load 'FontFace': no usable source in the src descriptor.",
      "NetworkError",
    );
  }

  /**
   * Load one `url()` via fetch (or blob: op). css-font-loading reserves
   * SyntaxError for the BufferSource form, so every failure here becomes a
   * NetworkError -- except a permission error, which stays actionable.
   * @param {string} url
   */
  async #loadUrl(url) {
    try {
      // blob: stays on the Rust side (no JS heap copy).
      if (StringPrototypeStartsWith(url, "blob:")) {
        const result = await op_fontdb_load_object_url(url);
        if (result === null) {
          throw new TypeError("the object URL is no longer valid");
        }
        return result;
      }
      const response = await loadFetch().fetch(url);
      if (!response.ok) {
        throw new TypeError(`the server responded with ${response.status}`);
      }
      const body = response.body;
      // Drain the body in Rust to avoid a JS ArrayBuffer copy.
      if (body !== null && getReadableStreamResourceBacking(body) !== null) {
        return await readableStreamCollectWithOp(body, op_fontdb_load_resource);
      }
      // No resource backing (e.g. the inspector tees the body); copy via JS.
      return await op_fontdb_load(
        new Uint8Array(await response.arrayBuffer()),
      );
    } catch (e) {
      if (ObjectPrototypeIsPrototypeOf(core.NotCapablePrototype, e)) throw e;
      throw new DOMException(
        `Failed to load 'FontFace': ${url}: ${e.message}`,
        "NetworkError",
      );
    }
  }

  /**
   * Covers any codepoint in text? (user unicode-range, else file cmap.)
   * @param {string} text
   * @returns {boolean}
   */
  [kUnicodeRangeCoversText](text) {
    if (this.#unicodeRangeUserSet) {
      return unicodeRangeCoversText(this.#unicodeRange, text);
    }
    // Unloaded: include conservatively.
    if (this.#fontFileCoverage === null) return true;
    return fontCoverageCoversText(this.#fontFileCoverage, text);
  }

  // Empty string => keep file metadata (do not round to a keyword).
  #register() {
    op_fontdb_add(
      this.#handle,
      this.#family,
      this.#styleUserSet ? this.#style : "",
      this.#weightUserSet ? this.#weight : "",
      this.#widthUserSet ? this.#width : "",
    );
  }

  [kAddToSystem]() {
    if (this.#status === "loaded" && this.#handle !== null) {
      this.#register();
      return PromiseResolve(this);
    }
    return PromisePrototypeThen(this.load(), () => {
      this.#register();
      return this;
    });
  }

  [kRemoveFromSystem]() {
    if (this.#handle !== null) {
      op_fontdb_remove(this.#handle);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(FontFacePrototype, this),
        keys: [
          "family",
          "style",
          "weight",
          "width",
          "stretch",
          "unicodeRange",
          "featureSettings",
          "variationSettings",
          "display",
          "ascentOverride",
          "descentOverride",
          "lineGapOverride",
          "status",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(FontFace);
const FontFacePrototype = FontFace.prototype;
markNotSerializable(FontFacePrototype);

/**
 * Placeholder FontFaceFeatures surface.
 * @see https://drafts.csswg.org/css-font-loading/#fontfacefeatures
 */
class FontFaceFeatures {
  [webidl.brand] = webidl.brand;

  constructor(key = null) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
  }
}
webidl.configureInterface(FontFaceFeatures);
const FontFaceFeaturesPrototype = FontFaceFeatures.prototype;
markNotSerializable(FontFaceFeaturesPrototype);

/**
 * @see https://drafts.csswg.org/css-font-loading/#fontfacevariationaxis
 */
class FontFaceVariationAxis {
  [webidl.brand] = webidl.brand;

  /** @type {string} */
  #name;
  /** @type {string} */
  #axisTag;
  /** @type {number} */
  #minimumValue;
  /** @type {number} */
  #maximumValue;
  /** @type {number} */
  #defaultValue;

  constructor(
    key = null,
    name = "",
    axisTag = "",
    minimumValue = 0,
    maximumValue = 0,
    defaultValue = 0,
  ) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
    this.#name = name;
    this.#axisTag = axisTag;
    this.#minimumValue = minimumValue;
    this.#maximumValue = maximumValue;
    this.#defaultValue = defaultValue;
  }

  get name() {
    webidl.assertBranded(this, FontFaceVariationAxisPrototype);
    return this.#name;
  }
  get axisTag() {
    webidl.assertBranded(this, FontFaceVariationAxisPrototype);
    return this.#axisTag;
  }
  get minimumValue() {
    webidl.assertBranded(this, FontFaceVariationAxisPrototype);
    return this.#minimumValue;
  }
  get maximumValue() {
    webidl.assertBranded(this, FontFaceVariationAxisPrototype);
    return this.#maximumValue;
  }
  get defaultValue() {
    webidl.assertBranded(this, FontFaceVariationAxisPrototype);
    return this.#defaultValue;
  }
}
webidl.configureInterface(FontFaceVariationAxis);
const FontFaceVariationAxisPrototype = FontFaceVariationAxis.prototype;
markNotSerializable(FontFaceVariationAxisPrototype);

/**
 * @see https://drafts.csswg.org/css-font-loading/#fontfacevariations
 */
class FontFaceVariations {
  [webidl.brand] = webidl.brand;

  /** @type {Set<FontFaceVariationAxis>} */
  #set;

  constructor(key = null) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
    this.#set = new SafeSet();
  }

  [webidl.setlikeInner]() {
    return this.#set;
  }
}
webidl.configureInterface(FontFaceVariations);
const FontFaceVariationsPrototype = FontFaceVariations.prototype;
webidl.setlikeObjectWrap(FontFaceVariationsPrototype, true);
markNotSerializable(FontFaceVariationsPrototype);

/**
 * @see https://drafts.csswg.org/css-font-loading/#fontfacepalette
 */
class FontFacePalette {
  [webidl.brand] = webidl.brand;

  /** @type {string[]} */
  #colors;
  /** @type {boolean} */
  #usableWithLightBackground;
  /** @type {boolean} */
  #usableWithDarkBackground;

  constructor(
    key = null,
    colors = [],
    usableWithLightBackground = false,
    usableWithDarkBackground = false,
  ) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
    this.#colors = colors;
    this.#usableWithLightBackground = usableWithLightBackground;
    this.#usableWithDarkBackground = usableWithDarkBackground;
  }

  get length() {
    webidl.assertBranded(this, FontFacePalettePrototype);
    return this.#colors.length;
  }

  get usableWithLightBackground() {
    webidl.assertBranded(this, FontFacePalettePrototype);
    return this.#usableWithLightBackground;
  }

  get usableWithDarkBackground() {
    webidl.assertBranded(this, FontFacePalettePrototype);
    return this.#usableWithDarkBackground;
  }
}
webidl.mixinValueIterable(FontFacePalette);
webidl.configureInterface(FontFacePalette);
const FontFacePalettePrototype = FontFacePalette.prototype;
markNotSerializable(FontFacePalettePrototype);

/**
 * @see https://drafts.csswg.org/css-font-loading/#fontfacepalettes
 */
class FontFacePalettes {
  [webidl.brand] = webidl.brand;

  /** @type {FontFacePalette[]} */
  #palettes;

  constructor(key = null, palettes = []) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
    this.#palettes = palettes;
  }

  get length() {
    webidl.assertBranded(this, FontFacePalettesPrototype);
    return this.#palettes.length;
  }
}
webidl.mixinValueIterable(FontFacePalettes);
webidl.configureInterface(FontFacePalettes);
const FontFacePalettesPrototype = FontFacePalettes.prototype;
markNotSerializable(FontFacePalettesPrototype);

const kFontFaces = Symbol("kFontFaces");

/**
 * @see https://drafts.csswg.org/css-font-loading/#fontfacesetloadevent
 */
class FontFaceSetLoadEvent extends Event {
  [webidl.brand] = webidl.brand;

  constructor(type, init = { __proto__: null }) {
    super(type, init);
    this[kFontFaces] = ObjectFreeze(
      ArrayPrototypeSlice(init.fontfaces ?? []),
    );
  }

  get fontfaces() {
    webidl.assertBranded(this, FontFaceSetLoadEventPrototype);
    return this[kFontFaces];
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          FontFaceSetLoadEventPrototype,
          this,
        ),
        keys: [
          ...new SafeArrayIterator(EVENT_PROPS),
          "fontfaces",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(FontFaceSetLoadEvent);
const FontFaceSetLoadEventPrototype = FontFaceSetLoadEvent.prototype;
markNotSerializable(FontFaceSetLoadEventPrototype);

/**
 * @see https://drafts.csswg.org/css-font-loading/#fontfaceset
 */
class FontFaceSet extends EventTarget {
  [webidl.brand] = webidl.brand;

  /** @type {Set<FontFace>} */
  #set;
  /** @type {Set<Promise<FontFace>>} */
  #loadingPromises;
  /** @type {FontFace[]} */
  #batchLoaded;
  /** @type {FontFace[]} */
  #batchFailed;

  constructor(key = null) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
    super();
    this.#set = new SafeSet();
    this.#loadingPromises = new SafeSet();
    this.#batchLoaded = [];
    this.#batchFailed = [];
  }

  [webidl.setlikeInner]() {
    return this.#set;
  }

  add(font) {
    webidl.assertBranded(this, FontFaceSetPrototype);
    const prefix = "Failed to execute 'add' on 'FontFaceSet'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    if (!ObjectPrototypeIsPrototypeOf(FontFacePrototype, font)) {
      throw new TypeError(
        `${prefix}: Argument 1 is not of type 'FontFace'.`,
      );
    }
    SetPrototypeAdd(this.#set, font);

    const wasIdle = this.#loadingPromises.size === 0;
    const p = font[kAddToSystem]();
    SetPrototypeAdd(this.#loadingPromises, p);

    // Fire "loading" on idle -> loading.
    if (wasIdle) {
      this.dispatchEvent(
        new FontFaceSetLoadEvent("loading", { fontfaces: [] }),
      );
    }

    PromisePrototypeThen(
      p,
      () => {
        ArrayPrototypePush(this.#batchLoaded, font);
        SetPrototypeDelete(this.#loadingPromises, p);
        if (this.#loadingPromises.size === 0) this[kFireBatchResult]();
      },
      () => {
        ArrayPrototypePush(this.#batchFailed, font);
        SetPrototypeDelete(this.#loadingPromises, p);
        if (this.#loadingPromises.size === 0) this[kFireBatchResult]();
      },
    );

    return this;
  }

  [kFireBatchResult]() {
    const loaded = this.#batchLoaded;
    const failed = this.#batchFailed;
    this.#batchLoaded = [];
    this.#batchFailed = [];

    this.dispatchEvent(
      new FontFaceSetLoadEvent("loadingdone", { fontfaces: loaded }),
    );
    if (failed.length > 0) {
      this.dispatchEvent(
        new FontFaceSetLoadEvent("loadingerror", { fontfaces: failed }),
      );
    }
  }

  delete(font) {
    webidl.assertBranded(this, FontFaceSetPrototype);
    webidl.requiredArguments(
      arguments.length,
      1,
      "Failed to execute 'delete' on 'FontFaceSet'",
    );
    if (SetPrototypeDelete(this.#set, font)) {
      font[kRemoveFromSystem]();
      return true;
    }
    return false;
  }

  clear() {
    webidl.assertBranded(this, FontFaceSetPrototype);
    for (const font of new SafeSetIterator(this.#set)) {
      font[kRemoveFromSystem]();
    }
    SetPrototypeClear(this.#set);
  }

  /**
   * @see https://drafts.csswg.org/css-font-loading/#dom-fontfaceset-check
   */
  check(font, text = " ") {
    webidl.assertBranded(this, FontFaceSetPrototype);
    const prefix = "Failed to execute 'check' on 'FontFaceSet'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    font = webidl.converters.DOMString(font, prefix, "Argument 1");
    text = webidl.converters.DOMString(text, prefix, "Argument 2");
    const { faces, foundFacesFlag, parseError } = matchFontFaces(
      this.#set,
      font,
      text,
    );
    if (parseError) {
      throw new DOMException(
        `${prefix}: Could not parse font.`,
        "SyntaxError",
      );
    }
    if (!foundFacesFlag) return false;
    for (let i = 0; i < faces.length; ++i) {
      if (faces[i].status !== "loaded") return false;
    }
    return true;
  }

  /**
   * @see https://drafts.csswg.org/css-font-loading/#dom-fontfaceset-load
   */
  load(font, text = " ") {
    try {
      webidl.assertBranded(this, FontFaceSetPrototype);
    } catch (e) {
      return PromiseReject(e);
    }
    const prefix = "Failed to execute 'load' on 'FontFaceSet'";
    try {
      webidl.requiredArguments(arguments.length, 1, prefix);
      font = webidl.converters.DOMString(font, prefix, "Argument 1");
      text = webidl.converters.DOMString(text, prefix, "Argument 2");
    } catch (e) {
      return PromiseReject(e);
    }
    const { faces, parseError } = matchFontFaces(this.#set, font, text);
    if (parseError) {
      return PromiseReject(
        new DOMException(
          `${prefix}: Could not parse font.`,
          "SyntaxError",
        ),
      );
    }
    return SafePromiseAll(ArrayPrototypeMap(faces, (face) => face.load()));
  }

  get ready() {
    // Promise attrs reject (not throw) on brand failure.
    try {
      webidl.assertBranded(this, FontFaceSetPrototype);
    } catch (e) {
      return PromiseReject(e);
    }
    const pending = [...new SafeSetIterator(this.#loadingPromises)];
    if (pending.length === 0) return PromiseResolve(this);
    return PromisePrototypeThen(SafePromiseAll(pending), () => this);
  }

  get status() {
    webidl.assertBranded(this, FontFaceSetPrototype);
    return this.#loadingPromises.size === 0 ? "loaded" : "loading";
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(FontFaceSetPrototype, this),
        keys: [
          "size",
          "status",
          "onloading",
          "onloadingdone",
          "onloadingerror",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(FontFaceSet);
const FontFaceSetPrototype = FontFaceSet.prototype;
webidl.setlikeObjectWrap(FontFaceSetPrototype, true);
markNotSerializable(FontFaceSetPrototype);

defineEventHandler(FontFaceSetPrototype, "loading");
defineEventHandler(FontFaceSetPrototype, "loadingdone");
defineEventHandler(FontFaceSetPrototype, "loadingerror");
// No [LegacyLenientThis]: re-wrap on* so brand-check runs on this.
brandEventHandlers(FontFaceSetPrototype, FontFaceSetPrototype, [
  "loading",
  "loadingdone",
  "loadingerror",
]);

/**
 * Brand-check on* handlers; set accessor names to "get/set onfoo".
 * @param {object} proto
 * @param {object} brandProto
 * @param {string[]} names event names without the `on` prefix
 */
function brandEventHandlers(proto, brandProto, names) {
  for (let i = 0; i < names.length; ++i) {
    const prop = `on${names[i]}`;
    const desc = ObjectGetOwnPropertyDescriptor(proto, prop);
    if (!desc || !desc.get) continue;
    const getter = function () {
      webidl.assertBranded(this, brandProto);
      return FunctionPrototypeCall(desc.get, this);
    };
    const setter = function (value) {
      webidl.assertBranded(this, brandProto);
      FunctionPrototypeCall(desc.set, this, value);
    };
    ObjectDefineProperty(getter, "name", {
      __proto__: null,
      value: `get ${prop}`,
      configurable: true,
    });
    ObjectDefineProperty(setter, "name", {
      __proto__: null,
      value: `set ${prop}`,
      configurable: true,
    });
    ObjectDefineProperty(proto, prop, {
      __proto__: null,
      get: getter,
      set: setter,
      enumerable: true,
      configurable: true,
    });
  }
}

webidl.configureInterface(TextMetrics);
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
            "lang",
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

function registerLocalFonts() {
  return op_fontdb_register_all_local_fonts();
}

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
      for (let i = 0; i < options.postscriptNames.length; ++i) {
        ArrayPrototypePush(
          postscriptNames,
          webidl.converters.DOMString(
            options.postscriptNames[i],
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

const fonts = new FontFaceSet(illegalConstructorKey);

return {
  CanvasGradient,
  CanvasPattern,
  FontData,
  FontDataPrototype,
  FontFace,
  FontFaceFeatures,
  FontFacePalette,
  FontFacePalettes,
  FontFacePrototype,
  FontFaceSet,
  FontFaceSetLoadEvent,
  FontFaceSetPrototype,
  FontFaceVariationAxis,
  FontFaceVariations,
  OffscreenCanvasRenderingContext2D,
  Path2D,
  fonts,
  queryLocalFonts,
  registerLocalFonts,
  TextMetrics,
};
})();
