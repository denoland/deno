// Copyright 2018-2026 the Deno authors. MIT license.

/// <reference path="../../core/internal.d.ts" />

// The console/inspect implementation lives in Rust (ext/web/console/); this
// file is a thin shim that wires the cppgc-wrapped `Console` class and the
// inspect ops into the historical JS export surface.

(function () {
const { core, internals, primordials } = __bootstrap;
const {
  op_console_css_to_ansi,
  op_console_format_value,
  op_console_get_string_width,
  op_console_inspect,
  op_console_inspect_args,
  op_console_parse_css,
  op_console_parse_css_color,
  op_console_quote_string,
  op_console_strip_vt,
  Console: ConsoleWrap,
} = core.ops;
const {
  AggregateError,
  AggregateErrorPrototype,
  Array,
  ArrayBuffer,
  ArrayBufferPrototype,
  ArrayPrototype,
  BigIntPrototypeValueOf,
  Boolean,
  BooleanPrototype,
  BooleanPrototypeValueOf,
  DataView,
  DataViewPrototype,
  Date,
  DatePrototype,
  Error,
  ErrorCaptureStackTrace,
  ErrorPrototype,
  Function,
  FunctionPrototype,
  FunctionPrototypeToString,
  Map,
  MapPrototype,
  Number,
  NumberPrototype,
  NumberPrototypeValueOf,
  Object,
  ObjectAssign,
  ObjectCreate,
  ObjectDefineProperty,
  ObjectIs,
  ObjectPrototype,
  ObjectSetPrototypeOf,
  Promise,
  PromisePrototype,
  RangeError,
  RangeErrorPrototype,
  ReflectGetOwnPropertyDescriptor,
  ReflectGetPrototypeOf,
  RegExp,
  RegExpPrototype,
  RegExpPrototypeToString,
  SafeArrayIterator,
  Set,
  SetPrototype,
  String,
  StringPrototype,
  StringPrototypeValueOf,
  Symbol,
  SymbolFor,
  SymbolHasInstance,
  SymbolPrototypeValueOf,
  SymbolToStringTag,
  TypedArray,
  TypedArrayPrototype,
  TypeError,
  TypeErrorPrototype,
  WeakMap,
  WeakMapPrototype,
  WeakSet,
  WeakSetPrototype,
} = primordials;

// ---------------------------------------------------------------------------
// no-color hooks (installed by the runtime bootstrap)

let noColorStdout = () => false;
let noColorStderr = () => false;

function setNoColorFns(stdoutFn, stderrFn) {
  noColorStdout = stdoutFn;
  noColorStderr = stderrFn;
}

function getStdoutNoColor() {
  return noColorStdout();
}

function getStderrNoColor() {
  return noColorStderr();
}

// ---------------------------------------------------------------------------
// styles / colors tables (exported; node's util.inspect builds on these)

const styles = {
  special: "cyan",
  number: "yellow",
  bigint: "yellow",
  boolean: "yellow",
  undefined: "grey",
  null: "bold",
  string: "green",
  symbol: "green",
  date: "magenta",
  // "name": intentionally not styling
  regexp: "red",
  module: "underline",
  internalError: "red",
  temporal: "cyan",
};

const defaultFG = 39;
const defaultBG = 49;

// Set Graphics Rendition https://en.wikipedia.org/wiki/ANSI_escape_code#graphics
// Each color consists of an array with the color code as first entry and the
// reset code as second entry.
const colors = {
  reset: [0, 0],
  bold: [1, 22],
  dim: [2, 22], // Alias: faint
  italic: [3, 23],
  underline: [4, 24],
  blink: [5, 25],
  // Swap foreground and background colors
  inverse: [7, 27], // Alias: swapcolors, swapColors
  hidden: [8, 28], // Alias: conceal
  strikethrough: [9, 29], // Alias: strikeThrough, crossedout, crossedOut
  doubleunderline: [21, 24], // Alias: doubleUnderline
  black: [30, defaultFG],
  red: [31, defaultFG],
  green: [32, defaultFG],
  yellow: [33, defaultFG],
  blue: [34, defaultFG],
  magenta: [35, defaultFG],
  cyan: [36, defaultFG],
  white: [37, defaultFG],
  bgBlack: [40, defaultBG],
  bgRed: [41, defaultBG],
  bgGreen: [42, defaultBG],
  bgYellow: [43, defaultBG],
  bgBlue: [44, defaultBG],
  bgMagenta: [45, defaultBG],
  bgCyan: [46, defaultBG],
  bgWhite: [47, defaultBG],
  framed: [51, 54],
  overlined: [53, 55],
  gray: [90, defaultFG], // Alias: grey, blackBright
  redBright: [91, defaultFG],
  greenBright: [92, defaultFG],
  yellowBright: [93, defaultFG],
  blueBright: [94, defaultFG],
  magentaBright: [95, defaultFG],
  cyanBright: [96, defaultFG],
  whiteBright: [97, defaultFG],
  bgGray: [100, defaultBG], // Alias: bgGrey, bgBlackBright
  bgRedBright: [101, defaultBG],
  bgGreenBright: [102, defaultBG],
  bgYellowBright: [103, defaultBG],
  bgBlueBright: [104, defaultBG],
  bgMagentaBright: [105, defaultBG],
  bgCyanBright: [106, defaultBG],
  bgWhiteBright: [107, defaultBG],
};

function defineColorAlias(target, alias) {
  ObjectDefineProperty(colors, alias, {
    __proto__: null,
    get() {
      return this[target];
    },
    set(value) {
      this[target] = value;
    },
    configurable: true,
    enumerable: false,
  });
}

defineColorAlias("gray", "grey");
defineColorAlias("gray", "blackBright");
defineColorAlias("bgGray", "bgGrey");
defineColorAlias("bgGray", "bgBlackBright");
defineColorAlias("dim", "faint");
defineColorAlias("strikethrough", "crossedout");
defineColorAlias("strikethrough", "strikeThrough");
defineColorAlias("strikethrough", "crossedOut");
defineColorAlias("hidden", "conceal");
defineColorAlias("inverse", "swapColors");
defineColorAlias("inverse", "swapcolors");
defineColorAlias("doubleunderline", "doubleUnderline");

// ---------------------------------------------------------------------------
// stylize helpers

// Marker recognized by the Rust engine so the table lookup runs natively.
const stylizeMarker = SymbolFor("Deno.privateConsoleStylize");

function stylizeNoColor(str) {
  return str;
}
ObjectDefineProperty(stylizeNoColor, stylizeMarker, {
  __proto__: null,
  value: "noColor",
  enumerable: false,
});

function createStylizeWithColor(styles, colors) {
  function stylizeWithColor(str, styleType) {
    const style = styles[styleType];
    if (style !== undefined) {
      const color = colors[style];
      if (color !== undefined) {
        return `\u001b[${color[0]}m${str}\u001b[${color[1]}m`;
      }
    }
    return str;
  }
  ObjectDefineProperty(stylizeWithColor, stylizeMarker, {
    __proto__: null,
    value: [styles, colors],
    enumerable: false,
  });
  return stylizeWithColor;
}

// ---------------------------------------------------------------------------
// intrinsics handed to the Rust engine on every call (captured here at
// module initialization so they are primordial-safe and snapshot-safe)

let _URLPrototype;
function getURLPrototype() {
  if (!_URLPrototype) {
    _URLPrototype = core.loadExtScript("ext:deno_web/00_url.js").URLPrototype;
  }
  return _URLPrototype;
}

function getCwd() {
  try {
    return Deno.cwd();
  } catch {
    return undefined;
  }
}

function makeCrossContextStylize(stylize) {
  return ObjectSetPrototypeOf((value, flavour) => {
    let stylized;
    try {
      stylized = `${stylize(value, flavour)}`;
    } catch {
      // Continue regardless of error.
    }

    if (typeof stylized !== "string") return value;
    return stylized;
  }, null);
}

const intrinsics = {
  __proto__: null,
  functionToString: function functionToString() {
    return FunctionPrototypeToString(this);
  },
  regExpToString: function regExpToString() {
    return RegExpPrototypeToString(this);
  },
  numberValueOf: function numberValueOf() {
    return NumberPrototypeValueOf(this);
  },
  stringValueOf: function stringValueOf() {
    return StringPrototypeValueOf(this);
  },
  booleanValueOf: function booleanValueOf() {
    return BooleanPrototypeValueOf(this);
  },
  bigIntValueOf: function bigIntValueOf() {
    return BigIntPrototypeValueOf(this);
  },
  symbolValueOf: function symbolValueOf() {
    return SymbolPrototypeValueOf(this);
  },
  inspect,
  stylizeNoColor,
  createStylizeWithColor,
  styles,
  colors,
  objectPrototype: ObjectPrototype,
  errorPrototype: ErrorPrototype,
  // Special-cased builtin prototypes (`wellKnownPrototypes`), as flat
  // [prototype, name, constructor] triples.
  wellKnown: [
    ArrayPrototype, "Array", Array,
    ArrayBufferPrototype, "ArrayBuffer", ArrayBuffer,
    FunctionPrototype, "Function", Function,
    MapPrototype, "Map", Map,
    SetPrototype, "Set", Set,
    ObjectPrototype, "Object", Object,
    TypedArrayPrototype, "TypedArray", TypedArray,
    RegExpPrototype, "RegExp", RegExp,
    DatePrototype, "Date", Date,
    DataViewPrototype, "DataView", DataView,
    ErrorPrototype, "Error", Error,
    AggregateErrorPrototype, "AggregateError", AggregateError,
    RangeErrorPrototype, "RangeError", RangeError,
    TypeErrorPrototype, "TypeError", TypeError,
    BooleanPrototype, "Boolean", Boolean,
    NumberPrototype, "Number", Number,
    StringPrototype, "String", String,
    PromisePrototype, "Promise", Promise,
    WeakMapPrototype, "WeakMap", WeakMap,
    WeakSetPrototype, "WeakSet", WeakSet,
  ],
  getURLPrototype,
  getCwd,
  makeCrossContextStylize,
};

// ---------------------------------------------------------------------------
// inspect entry points

const denoInspectDefaultOptions = {
  indentationLvl: 0,
  currentDepth: 0,
  stylize: stylizeNoColor,

  showHidden: false,
  depth: 4,
  colors: false,
  showProxy: false,
  breakLength: 80,
  escapeSequences: true,
  compact: 3,
  sorted: false,
  getters: false,

  // node only
  maxArrayLength: 100,
  maxStringLength: 10_000, // deno: strAbbreviateSize: 10_000
  customInspect: true,

  // deno only
  quotes: ['"', "'", "`"],
  iterableLimit: 100, // similar to node's maxArrayLength
  trailingComma: false,

  inspect,

  indentLevel: 0,
};

function getDefaultInspectOptions() {
  return {
    budget: {},
    seen: [],
    ...denoInspectDefaultOptions,
  };
}

class CSI {
  static kClear = "\x1b[1;1H";
  static kClearScreenDown = "\x1b[0J";
}

function quoteString(string, ctx) {
  return op_console_quote_string(string, ctx);
}

function getStringWidth(str, removeControlChars = true) {
  return op_console_get_string_width(str, removeControlChars);
}

function stripVTControlCharacters(str) {
  return op_console_strip_vt(str);
}

function formatNumber(fn, value) {
  // Format -0 as '-0'. Checking `value === -0` won't distinguish 0 from -0.
  return fn(ObjectIs(value, -0) ? "-0" : `${value}`, "number");
}

function formatBigInt(fn, value) {
  return fn(`${value}n`, "bigint");
}

function formatValue(ctx, value, recurseTimes) {
  return op_console_format_value(intrinsics, ctx, value, recurseTimes ?? 0);
}

function inspectArgs(args, inspectOptions = { __proto__: null }) {
  const colors = inspectOptions.colors ?? !noColorStdout();
  return op_console_inspect_args(intrinsics, args, {
    __proto__: null,
    ...inspectOptions,
    colors,
  });
}

function inspect(
  value,
  inspectOptions = { __proto__: null },
) {
  return op_console_inspect(intrinsics, value, inspectOptions);
}

function cssToAnsi(css, prevCss = null) {
  return op_console_css_to_ansi(css, prevCss);
}

function parseCss(cssString) {
  return op_console_parse_css(cssString);
}

function parseCssColor(colorString) {
  return op_console_parse_css_color(colorString);
}

/** @param noColor {boolean} */
function getConsoleInspectOptions(noColor) {
  return {
    ...getDefaultInspectOptions(),
    colors: !noColor,
    stylize: noColor ? stylizeNoColor : createStylizeWithColor(styles, colors),
  };
}

// ---------------------------------------------------------------------------
// Console

const isConsoleInstance = Symbol("isConsoleInstance");

class Console {
  #wrap = null;
  #printFunc = null;
  // Reference to the namespace object returned from the constructor; `group`
  // routes its label through the namespace's (possibly inspector-wrapped)
  // `log` so DevTools renders the label inside the group container.
  #consoleRef = null;
  [isConsoleInstance] = false;

  #indentLevel = 0;

  // The cppgc-backed implementation can't be created while snapshotting
  // (no cppgc heap exists yet), so it is created lazily on first use.
  #getWrap() {
    if (this.#wrap === null) {
      this.#wrap = new ConsoleWrap(
        this.#printFunc,
        intrinsics,
        getStdoutNoColor,
        getStderrNoColor,
      );
      this.#wrap.indentLevel = this.#indentLevel;
    }
    return this.#wrap;
  }

  constructor(printFunc) {
    this.#printFunc = printFunc;
    const self = this;
    ObjectDefineProperty(this, "indentLevel", {
      __proto__: null,
      get() {
        return self.#wrap === null
          ? self.#indentLevel
          : self.#wrap.indentLevel;
      },
      set(value) {
        if (self.#wrap === null) {
          self.#indentLevel = value;
        } else {
          self.#wrap.indentLevel = value;
        }
      },
      enumerable: true,
      configurable: true,
    });
    this[isConsoleInstance] = true;

    // ref https://console.spec.whatwg.org/#console-namespace
    // For historical web-compatibility reasons, the namespace object for
    // console must have as its [[Prototype]] an empty object, created as if
    // by ObjectCreate(%ObjectPrototype%), instead of %ObjectPrototype%.
    const console = ObjectCreate({}, {
      [SymbolToStringTag]: {
        enumerable: false,
        writable: false,
        configurable: true,
        value: "console",
      },
    });
    ObjectAssign(console, this);
    this.#consoleRef = console;
    return console;
  }

  log = (...args) => this.#getWrap().log(...new SafeArrayIterator(args));

  debug = (...args) => this.#getWrap().debug(...new SafeArrayIterator(args));

  info = (...args) => this.#getWrap().info(...new SafeArrayIterator(args));

  dir = (obj = undefined, options = { __proto__: null }) =>
    this.#getWrap().dir(obj, options);

  // Per https://console.spec.whatwg.org/#dirxml, dirxml uses the log
  // printer (not dir). Use a fresh arrow so the method's .name is "dirxml".
  dirxml = (...args) => this.log(...new SafeArrayIterator(args));

  warn = (...args) => this.#getWrap().warn(...new SafeArrayIterator(args));

  error = (...args) => this.#getWrap().error(...new SafeArrayIterator(args));

  assert = (condition = false, ...args) =>
    this.#getWrap().assert(condition, ...new SafeArrayIterator(args));

  count = (label = "default") => this.#getWrap().count(String(label));

  countReset = (label = "default") => this.#getWrap().countReset(String(label));

  table = (data = undefined, properties = undefined) =>
    this.#getWrap().table(data, properties);

  time = (label = "default") => this.#getWrap().time(String(label));

  timeLog = (label = "default", ...args) =>
    this.#getWrap().timeLog(String(label), ...new SafeArrayIterator(args));

  timeEnd = (label = "default") => this.#getWrap().timeEnd(String(label));

  group = (...label) => {
    if (label.length > 0) {
      // Route through the namespace object's `log` so that, when the
      // inspector wraps console methods, both the V8 console binding (for
      // DevTools) and the internal `log` (for the terminal) are invoked.
      this.#consoleRef.log(...new SafeArrayIterator(label));
    }
    this.#getWrap().indentLevel++;
  };

  groupCollapsed = this.group;

  groupEnd = () => {
    const wrap = this.#getWrap();
    if (wrap.indentLevel > 0) {
      wrap.indentLevel--;
    }
  };

  clear = () => this.#getWrap().clear();

  trace = (...args) => {
    const message = inspectArgs(
      args,
      {
        ...getConsoleInspectOptions(noColorStderr()),
        indentLevel: 0,
      },
    );
    const err = {
      name: "Trace",
      message,
    };
    ErrorCaptureStackTrace(err, this.trace);
    this.#printFunc(err.stack, 4);
    this.#printFunc("\n", 4);
  };

  // These methods are noops, but when the inspector is connected, they
  // call into V8.
  profile = (_label) => {};
  profileEnd = (_label) => {};
  timeStamp = (_label) => {};

  static [SymbolHasInstance](instance) {
    return instance[isConsoleInstance];
  }
}

const customInspect = SymbolFor("Deno.customInspect");

/** Creates an object that represents a subset of the properties of the
 * original object, suitable for handing to `inspect()`. The returned value
 * carries the original object's class name and the listed keys as own
 * enumerable data properties. */
function createFilteredInspectProxy({ object, keys, evaluate }) {
  const cls = class {};
  if (object.constructor?.name) {
    ObjectDefineProperty(cls, "name", {
      __proto__: null,
      value: object.constructor.name,
    });
  }

  const result = new cls();
  for (let i = 0; i < keys.length; i++) {
    const key = keys[i];
    const descriptor = evaluate
      ? getEvaluatedDescriptor(object, key)
      : (getDescendantPropertyDescriptor(object, key) ??
        getEvaluatedDescriptor(object, key));
    ObjectDefineProperty(result, key, descriptor);
  }
  return result;

  function getDescendantPropertyDescriptor(object, key) {
    let propertyDescriptor = ReflectGetOwnPropertyDescriptor(object, key);
    if (!propertyDescriptor) {
      const prototype = ReflectGetPrototypeOf(object);
      if (prototype) {
        propertyDescriptor = getDescendantPropertyDescriptor(prototype, key);
      }
    }
    return propertyDescriptor;
  }

  function getEvaluatedDescriptor(object, key) {
    return {
      configurable: true,
      enumerable: true,
      value: object[key],
    };
  }
}

// Expose these fields to internalObject for tests.
internals.Console = Console;
internals.cssToAnsi = cssToAnsi;
internals.inspectArgs = inspectArgs;
internals.parseCss = parseCss;
internals.parseCssColor = parseCssColor;

return {
  colors,
  Console,
  createFilteredInspectProxy,
  createStylizeWithColor,
  CSI,
  customInspect,
  formatBigInt,
  formatNumber,
  formatValue,
  getConsoleInspectOptions,
  getDefaultInspectOptions,
  getStderrNoColor,
  getStringWidth,
  getStdoutNoColor,
  inspect,
  inspectArgs,
  quoteString,
  setNoColorFns,
  stripVTControlCharacters,
  styles,
};
})();
