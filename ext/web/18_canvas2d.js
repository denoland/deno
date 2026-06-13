// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  OffscreenCanvasRenderingContext2D,
  op_canvas2d_init,
  op_fontdb_add,
  op_fontdb_load,
  op_fontdb_remove,
  op_parse_css_font_query,
  TextMetrics,
} = core.ops;

const {
  ArrayBufferIsView,
  ArrayBufferPrototype,
  ArrayPrototypeEvery,
  ArrayPrototypeFilter,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  Number,
  NumberIsFinite,
  NumberParseInt,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  PromiseReject,
  PromiseResolve,
  RegExpPrototypeExec,
  SafeArrayIterator,
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
  StringPrototypeTrim,
  StringPrototypeToLowerCase,
  Symbol,
  SymbolFor,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
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

op_canvas2d_init();

const CSS_FONT_STYLE_VALUES = new SafeSet(["normal", "italic", "oblique"]);
const CSS_FONT_STRETCH_VALUES = new SafeSet([
  "ultra-condensed",
  "extra-condensed",
  "condensed",
  "semi-condensed",
  "normal",
  "semi-expanded",
  "expanded",
  "extra-expanded",
  "ultra-expanded",
]);

// CSS font-display descriptor values.
// See https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-display
const CSS_FONT_DISPLAY_VALUES = new SafeSet([
  "auto",
  "block",
  "swap",
  "fallback",
  "optional",
]);

// Matches one CSS unicode-range token (U+XXXX, U+XXXX-YYYY, or U+XX??).
// See https://drafts.csswg.org/css-fonts-4/#descdef-font-face-unicode-range
const UNICODE_RANGE_SINGLE_RE = new SafeRegExp(
  /^\s*[Uu]\+[0-9A-Fa-f?]{1,6}(?:-[0-9A-Fa-f]{1,6})?\s*$/,
);

// CSS font-feature-settings item: "<tag>" [<integer> | on | off]?
// The tag must be exactly 4 printable ASCII characters (U+0020-U+007E).
// See https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-feature-settings
const FONT_FEATURE_SETTINGS_ITEM_RE = new SafeRegExp(
  /^\s*"[\x20-\x7E]{4}"\s*(?:\d+|on|off)?\s*$/,
);

// CSS font-variation-settings item: "<tag>" <number>
// The tag must be exactly 4 printable ASCII characters.
// See https://drafts.csswg.org/css-fonts-4/#descdef-font-face-font-variation-settings
const FONT_VARIATION_SETTINGS_ITEM_RE = new SafeRegExp(
  /^\s*"[\x20-\x7E]{4}"\s+[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?\s*$/,
);

// CSS ascent-override / descent-override / line-gap-override: normal | <percentage [0,inf]>
// See https://drafts.csswg.org/css-fonts-4/#descdef-font-face-ascent-override
const METRIC_OVERRIDE_VALUE_RE = new SafeRegExp(/^\d+(?:\.\d+)?%$/);

function isValidFontStyle(v) {
  return SetPrototypeHas(CSS_FONT_STYLE_VALUES, v);
}
function isValidFontWeight(v) {
  if (v === "normal" || v === "bold") return true;
  const n = NumberParseInt(v, 10);
  return NumberIsFinite(n) && n >= 1 && n <= 1000;
}
function isValidFontStretch(v) {
  return SetPrototypeHas(CSS_FONT_STRETCH_VALUES, v);
}

/**
 * Returns true if v is a valid CSS unicode-range descriptor value.
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
 * Returns true if v is a valid CSS font-feature-settings descriptor value.
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
 * Returns true if v is a valid CSS font-variation-settings descriptor value.
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
 * Returns true if v is a valid CSS ascent-override / descent-override / line-gap-override.
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

// Generic font families are always considered available without a font file,
// and are excluded from descriptor matching in check() / load().
const GENERIC_FONT_FAMILIES = new SafeSet([
  "serif",
  "sans-serif",
  "monospace",
  "cursive",
  "fantasy",
  "system-ui",
  "math",
  "emoji",
  "fangsong",
]);

// Matches a single CSS unicode-range token: U+XXXX, U+XXXX-YYYY, or U+XX??.
const UNICODE_RANGE_TOKEN_RE = new SafeRegExp(
  /^\s*[Uu]\+([0-9A-Fa-f?]+)(?:-([0-9A-Fa-f]+))?\s*$/,
);
// Global flag is safe here: StringPrototypeReplace resets lastIndex on every call.
const UNICODE_RANGE_WILDCARD_RE = new SafeRegExp(/\?/g);

/**
 * Normalizes a CSS font-weight string to a numeric value.
 * Uses Number() instead of NumberParseInt() to reject values like "700px".
 * @param {string} w
 * @returns {number}
 */
function normalizeFontWeight(w) {
  if (w === "normal") return 400;
  if (w === "bold") return 700;
  const n = Number(w);
  return NumberIsFinite(n) ? n : 400;
}

/**
 * Returns true if the CSS unicode-range value covers at least one codepoint in text.
 * @param {string} unicodeRange
 * @param {string} text
 * @returns {boolean}
 */
function unicodeRangeCoversText(unicodeRange, text) {
  if (!unicodeRange || unicodeRange === "U+0-10FFFF") return true;
  const ranges = StringPrototypeSplit(unicodeRange, ",");
  for (let i = 0; i < text.length;) {
    const cp = StringPrototypeCodePointAt(text, i);
    for (const range of new SafeArrayIterator(ranges)) {
      const m = RegExpPrototypeExec(UNICODE_RANGE_TOKEN_RE, range);
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
 * Returns true if the sorted [[start, end], ...] coverage array includes
 * at least one codepoint in text.
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
 * Implements the CSS Font Loading Level 3 "find the matching font faces" algorithm.
 * Step 1: descriptor matching (family, style, weight, stretch) -- sets foundFacesFlag.
 * Step 2: unicode-range filter via each face's kUnicodeRangeCoversText method.
 * @param {Set<FontFace>} set
 * @param {string} font
 * @param {string} text
 * @returns {{ faces: FontFace[], foundFacesFlag: boolean, parseError: boolean }}
 * @see https://drafts.csswg.org/css-font-loading/#find-the-matching-font-faces
 */
function matchFontFaces(set, font, text) {
  const parsed = op_parse_css_font_query(font);
  if (!parsed) return { faces: [], foundFacesFlag: false, parseError: true };

  // weight is already numeric from Rust; face.weight is a string and needs normalization.
  const { family, style, weight, stretch } = parsed;

  if (
    SetPrototypeHas(GENERIC_FONT_FAMILIES, StringPrototypeToLowerCase(family))
  ) {
    return { faces: [], foundFacesFlag: true, parseError: false };
  }

  const matched = [];
  for (const face of new SafeSetIterator(set)) {
    if (face.family !== family) continue;
    if (face.style !== style) continue;
    if (normalizeFontWeight(face.weight) !== weight) continue;
    if (face.stretch !== stretch) continue;
    ArrayPrototypePush(matched, face);
  }

  const foundFacesFlag = matched.length > 0;
  const faces = ArrayPrototypeFilter(
    matched,
    (face) => face[kUnicodeRangeCoversText](text),
  );
  return { faces, foundFacesFlag, parseError: false };
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
  #stretch = "normal";
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

  /** @type {Uint8Array} */
  #bytes;
  /** @type {"unloaded" | "loading" | "loaded" | "error"} */
  #status = "unloaded";
  /** @type {number | null} */
  #handle = null;
  /** @type {Promise<FontFace> | null} */
  #loadPromise = null;
  /** @type {unknown} */
  #loadError = null;

  // Tracks which descriptors were explicitly provided by the caller.
  // Font-file-derived metadata is only applied to non-user-set descriptors.
  /** @type {boolean} */
  #styleUserSet = false;
  /** @type {boolean} */
  #weightUserSet = false;
  /** @type {boolean} */
  #stretchUserSet = false;
  /** @type {boolean} */
  #unicodeRangeUserSet = false;

  // Unicode codepoint coverage extracted from the font file after loading.
  // Stored as sorted [[start, end], ...] pairs for binary-search lookup.
  // null means "not yet loaded" (treated as full-range U+0-10FFFF).
  /** @type {[number, number][] | null} */
  #fontFileCoverage = null;

  constructor(family, source, descriptors = { __proto__: null }) {
    const prefix = "Failed to construct 'FontFace'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    family = webidl.converters.DOMString(family, prefix, "Argument 1");

    if (typeof source === "string") {
      throw new DOMException(
        `${prefix}: URL sources are not supported.`,
        "NotSupportedError",
      );
    }
    if (
      !ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, source) &&
      !ArrayBufferIsView(source)
    ) {
      throw new TypeError(
        `${prefix}: source must be an ArrayBuffer or ArrayBufferView.`,
      );
    }

    this.#family = family;
    this.#bytes = ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, source)
      ? new Uint8Array(source)
      : new Uint8Array(
        TypedArrayPrototypeGetBuffer(source),
        TypedArrayPrototypeGetByteOffset(source),
        TypedArrayPrototypeGetByteLength(source),
      );

    if (descriptors.style !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.style,
        prefix,
        "descriptors.style",
      );
      if (!isValidFontStyle(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'style' descriptor.`,
          "SyntaxError",
        );
      }
      this.#style = v;
      this.#styleUserSet = true;
    }
    if (descriptors.weight !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.weight,
        prefix,
        "descriptors.weight",
      );
      if (!isValidFontWeight(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'weight' descriptor.`,
          "SyntaxError",
        );
      }
      this.#weight = v;
      this.#weightUserSet = true;
    }
    if (descriptors.stretch !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.stretch,
        prefix,
        "descriptors.stretch",
      );
      if (!isValidFontStretch(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'stretch' descriptor.`,
          "SyntaxError",
        );
      }
      this.#stretch = v;
      this.#stretchUserSet = true;
    }
    if (descriptors.unicodeRange !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.unicodeRange,
        prefix,
        "descriptors.unicodeRange",
      );
      if (!isValidUnicodeRange(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'unicodeRange' descriptor.`,
          "SyntaxError",
        );
      }
      this.#unicodeRange = v;
      this.#unicodeRangeUserSet = true;
    }
    if (descriptors.featureSettings !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.featureSettings,
        prefix,
        "descriptors.featureSettings",
      );
      if (!isValidFontFeatureSettings(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'featureSettings' descriptor.`,
          "SyntaxError",
        );
      }
      this.#featureSettings = v;
    }
    if (descriptors.variationSettings !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.variationSettings,
        prefix,
        "descriptors.variationSettings",
      );
      if (!isValidFontVariationSettings(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'variationSettings' descriptor.`,
          "SyntaxError",
        );
      }
      this.#variationSettings = v;
    }
    if (descriptors.display !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.display,
        prefix,
        "descriptors.display",
      );
      if (!SetPrototypeHas(CSS_FONT_DISPLAY_VALUES, v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'display' descriptor.`,
          "SyntaxError",
        );
      }
      this.#display = v;
    }
    if (descriptors.ascentOverride !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.ascentOverride,
        prefix,
        "descriptors.ascentOverride",
      );
      if (!isValidMetricOverride(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'ascentOverride' descriptor.`,
          "SyntaxError",
        );
      }
      this.#ascentOverride = v;
    }
    if (descriptors.descentOverride !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.descentOverride,
        prefix,
        "descriptors.descentOverride",
      );
      if (!isValidMetricOverride(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'descentOverride' descriptor.`,
          "SyntaxError",
        );
      }
      this.#descentOverride = v;
    }
    if (descriptors.lineGapOverride !== undefined) {
      const v = webidl.converters.DOMString(
        descriptors.lineGapOverride,
        prefix,
        "descriptors.lineGapOverride",
      );
      if (!isValidMetricOverride(v)) {
        throw new DOMException(
          `${prefix}: Invalid value for 'lineGapOverride' descriptor.`,
          "SyntaxError",
        );
      }
      this.#lineGapOverride = v;
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
    if (v.length === 0) {
      throw new DOMException(
        "Failed to set 'family' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#family = v;
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
  }

  get stretch() {
    webidl.assertBranded(this, FontFacePrototype);
    return this.#stretch;
  }

  set stretch(v) {
    webidl.assertBranded(this, FontFacePrototype);
    v = webidl.converters.DOMString(
      v,
      "Failed to set 'stretch' on 'FontFace'",
      "Value",
    );
    if (!isValidFontStretch(v)) {
      throw new DOMException(
        "Failed to set 'stretch' on 'FontFace': Invalid value.",
        "SyntaxError",
      );
    }
    this.#stretch = v;
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
    webidl.assertBranded(this, FontFacePrototype);
    return this.load();
  }

  load() {
    webidl.assertBranded(this, FontFacePrototype);
    if (this.#status === "loaded") return PromiseResolve(this);
    if (this.#status === "error") return PromiseReject(this.#loadError);
    if (this.#loadPromise !== null) return this.#loadPromise;

    this.#status = "loading";
    this.#loadPromise = (async () => {
      try {
        const { handle, weight, style, stretch, unicodeCoverage } =
          await op_fontdb_load(this.#bytes);
        this.#handle = handle;
        // Apply font-file metadata only for descriptors not set by the caller.
        // User-specified descriptors always win.
        if (!this.#weightUserSet) this.#weight = String(weight);
        if (!this.#styleUserSet) this.#style = style;
        if (!this.#stretchUserSet) this.#stretch = stretch;
        // Always store the actual coverage for use in check() / load() matching.
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

  /**
   * Returns true if this font face covers at least one codepoint in text,
   * using the user-declared unicode-range CSS string when explicitly provided,
   * or the actual font-file coverage otherwise.
   * @param {string} text
   * @returns {boolean}
   */
  [kUnicodeRangeCoversText](text) {
    if (this.#unicodeRangeUserSet) {
      return unicodeRangeCoversText(this.#unicodeRange, text);
    }
    // Before loading, coverage is unknown; conservatively include the face.
    if (this.#fontFileCoverage === null) return true;
    return fontCoverageCoversText(this.#fontFileCoverage, text);
  }

  [kAddToSystem]() {
    if (this.#status === "loaded" && this.#handle !== null) {
      op_fontdb_add(
        this.#handle,
        this.#family,
        this.#style,
        this.#weight,
        this.#stretch,
      );
      return PromiseResolve(this);
    }
    return PromisePrototypeThen(this.load(), () => {
      op_fontdb_add(
        this.#handle,
        this.#family,
        this.#style,
        this.#weight,
        this.#stretch,
      );
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

const kFontFaces = Symbol("kFontFaces");

/**
 * @see https://drafts.csswg.org/css-font-loading/#fontfacesetloadevent
 */
class FontFaceSetLoadEvent extends Event {
  [webidl.brand] = webidl.brand;

  constructor(type, init = { __proto__: null }) {
    super(type, init);
    this[kFontFaces] = ObjectFreeze(
      [...new SafeArrayIterator(init.fontfaces ?? [])],
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

const FontFaceSetLoadEventPrototype = FontFaceSetLoadEvent.prototype;

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
    if (!ObjectPrototypeIsPrototypeOf(FontFacePrototype, font)) {
      throw new TypeError(
        "Failed to execute 'add' on 'FontFaceSet': Argument 1 is not of type 'FontFace'.",
      );
    }
    SetPrototypeAdd(this.#set, font);

    const wasIdle = this.#loadingPromises.size === 0;
    const p = font[kAddToSystem]();
    SetPrototypeAdd(this.#loadingPromises, p);

    // Dispatch "loading" when transitioning from idle to loading state.
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
    const { faces, foundFacesFlag, parseError } = matchFontFaces(
      this.#set,
      font,
      text,
    );
    if (parseError) {
      throw new DOMException(
        "Failed to execute 'check' on 'FontFaceSet': Could not parse font.",
        "SyntaxError",
      );
    }
    if (!foundFacesFlag) return false;
    for (const face of new SafeArrayIterator(faces)) {
      if (face.status !== "loaded") return false;
    }
    return true;
  }

  /**
   * @see https://drafts.csswg.org/css-font-loading/#dom-fontfaceset-load
   */
  load(font, text = " ") {
    webidl.assertBranded(this, FontFaceSetPrototype);
    const { faces, parseError } = matchFontFaces(this.#set, font, text);
    if (parseError) {
      return PromiseReject(
        new DOMException(
          "Failed to execute 'load' on 'FontFaceSet': Could not parse font.",
          "SyntaxError",
        ),
      );
    }
    return SafePromiseAll(ArrayPrototypeMap(faces, (face) => face.load()));
  }

  get ready() {
    webidl.assertBranded(this, FontFaceSetPrototype);
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

webidl.configureInterface(TextMetrics);
webidl.configureInterface(OffscreenCanvasRenderingContext2D);

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

const fonts = new FontFaceSet(illegalConstructorKey);

return {
  OffscreenCanvasRenderingContext2D,
  FontFace,
  FontFacePrototype,
  FontFaceSet,
  FontFaceSetLoadEvent,
  FontFaceSetPrototype,
  fonts,
  TextMetrics,
};
})();
