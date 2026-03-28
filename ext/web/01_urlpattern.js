// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_url.d.ts" />

import { primordials } from "ext:core/mod.js";
import {
  op_urlpattern_parse,
  op_urlpattern_process_match_input,
} from "ext:core/ops";
const {
  ArrayPrototypePush,
  MathRandom,
  ObjectAssign,
  ObjectCreate,
  ObjectPrototypeIsPrototypeOf,
  RegExpPrototypeExec,
  RegExpPrototypeTest,
  SafeMap,
  SafeRegExp,
  StringPrototypeSlice,
  Symbol,
  SymbolFor,
  TypeError,
  Uint32Array,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "./01_console.js";

const _components = Symbol("components");
const urlPatternSettings = { groupStringFallback: false };

/**
 * @typedef Components
 * @property {Component} protocol
 * @property {Component} username
 * @property {Component} password
 * @property {Component} hostname
 * @property {Component} port
 * @property {Component} pathname
 * @property {Component} search
 * @property {Component} hash
 */
const COMPONENTS_KEYS = [
  "protocol",
  "username",
  "password",
  "hostname",
  "port",
  "pathname",
  "search",
  "hash",
];

/**
 * @typedef Component
 * @property {string} patternString
 * @property {RegExp} regexp
 * @property {string[]} groupNameList
 */

/**
 * This implements a least-recently-used cache that has a pseudo-"young
 * generation" by using sampling. The idea is that we want to keep the most
 * recently used items in the cache, but we don't want to pay the cost of
 * updating the cache on every access. This relies on the fact that the data
 * we're caching is not uniformly distributed, and that the most recently used
 * items are more likely to be used again soon (long tail distribution).
 *
 * The LRU cache is implemented as a Map, with the key being the cache key and
 * the value being the cache value. When an item is accessed, it is moved to the
 * end of the Map. When an item is inserted, if the Map is at capacity, the
 * first item in the Map is deleted. Because maps iterate using insertion order,
 * this means that the oldest item is always the first.
 *
 * The sampling is implemented by using a random number generator to decide
 * whether to update the cache on each access. This means that the cache will
 * not be updated on every access, but will be updated on a random subset of
 * accesses.
 *
 * @template K
 * @template V
 */
class SampledLRUCache {
  /** @type {SafeMap<K, V>} */
  #map = new SafeMap();
  #capacity = 0;
  #sampleRate = 0.1;

  /** @type {K} */
  #lastUsedKey = undefined;
  /** @type {V} */
  #lastUsedValue = undefined;

  /** @param {number} capacity */
  constructor(capacity) {
    this.#capacity = capacity;
  }

  /**
   * @param {K} key
   * @param {(key: K) => V} factory
   * @return {V}
   */
  getOrInsert(key, factory) {
    if (this.#lastUsedKey === key) return this.#lastUsedValue;
    const value = this.#map.get(key);
    if (value !== undefined) {
      if (MathRandom() < this.#sampleRate) {
        // put the item into the map
        this.#map.delete(key);
        this.#map.set(key, value);
      }
      this.#lastUsedKey = key;
      this.#lastUsedValue = value;
      return value;
    } else {
      // value doesn't exist yet, create
      const value = factory(key);
      if (MathRandom() < this.#sampleRate) {
        // if the map is at capacity, delete the oldest (first) element
        if (this.#map.size > this.#capacity) {
          // deno-lint-ignore prefer-primordials
          this.#map.delete(this.#map.keys().next().value);
        }
        // insert the new value
        this.#map.set(key, value);
      }
      this.#lastUsedKey = key;
      this.#lastUsedValue = value;
      return value;
    }
  }
}

const matchInputCache = new SampledLRUCache(4096);

/**
 * Shared buffer for receiving URL component offsets from the
 * op_urlpattern_process_match_input op.
 *
 * Layout: buf[0..8] = cumulative start offsets, buf[8] = total length.
 * Component i's value = concat.slice(buf[i], buf[i+1]).
 */
const matchBuf = new Uint32Array(9);

/**
 * Calls the op and extracts the 8 component values from the concatenated
 * string + offset buffer. Returns a flat 8-element string array, or null.
 * @param {string | object} input
 * @param {string | undefined} baseURL
 * @returns {string[] | null}
 */
function processMatchInput(input, baseURL) {
  const concat = op_urlpattern_process_match_input(
    input,
    baseURL ?? null,
    matchBuf,
  );
  if (concat === null) return null;
  return [
    StringPrototypeSlice(concat, matchBuf[0], matchBuf[1]),
    StringPrototypeSlice(concat, matchBuf[1], matchBuf[2]),
    StringPrototypeSlice(concat, matchBuf[2], matchBuf[3]),
    StringPrototypeSlice(concat, matchBuf[3], matchBuf[4]),
    StringPrototypeSlice(concat, matchBuf[4], matchBuf[5]),
    StringPrototypeSlice(concat, matchBuf[5], matchBuf[6]),
    StringPrototypeSlice(concat, matchBuf[6], matchBuf[7]),
    StringPrototypeSlice(concat, matchBuf[7], matchBuf[8]),
  ];
}

/**
 * Cache-compatible factory: calls processMatchInput with no baseURL.
 * @param {string | object} input
 * @returns {string[] | null}
 */
function processMatchInputCached(input) {
  return processMatchInput(input, undefined);
}

const _hasRegExpGroups = Symbol("[[hasRegExpGroups]]");

class URLPattern {
  /** @type {Component[]} */
  [_components];
  [_hasRegExpGroups];

  #reusedResult;

  /**
   * @param {URLPatternInput} input
   * @param {string} [baseURLOrOptions]
   * @param {string} [maybeOptions]
   */
  constructor(
    input,
    baseURLOrOptions = undefined,
    maybeOptions = undefined,
  ) {
    this[webidl.brand] = webidl.brand;
    const prefix = "Failed to construct 'URLPattern'";

    let baseURL;
    let options;
    if (webidl.type(baseURLOrOptions) === "String") {
      webidl.requiredArguments(arguments.length, 1, prefix);
      input = webidl.converters.URLPatternInput(input, prefix, "Argument 1");
      baseURL = webidl.converters.USVString(
        baseURLOrOptions,
        prefix,
        "Argument 2",
      );
      options = webidl.converters.URLPatternOptions(
        maybeOptions !== undefined ? maybeOptions : { __proto: null },
        prefix,
        "Argument 3",
      );
    } else {
      if (input !== undefined) {
        input = webidl.converters.URLPatternInput(input, prefix, "Argument 1");
      } else {
        input = { __proto__: null };
      }
      options = webidl.converters.URLPatternOptions(
        baseURLOrOptions,
        prefix,
        "Argument 2",
      );
    }

    const parsed = op_urlpattern_parse(input, baseURL, options);
    this[_hasRegExpGroups] = parsed.hasRegexpGroups;

    const flags = options.ignoreCase ? "ui" : "u";
    const components = [
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
    ];
    for (let i = 0; i < 8; ++i) {
      const key = COMPONENTS_KEYS[i];
      const c = parsed[key];
      try {
        c.regexp = new SafeRegExp(c.regexpString, flags);
      } catch (e) {
        throw new TypeError(`${prefix}: ${key} is invalid; ${e.message}`);
      }
      components[i] = c;
    }
    this[_components] = components;
  }

  get protocol() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_components][0].patternString;
  }

  get username() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_components][1].patternString;
  }

  get password() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_components][2].patternString;
  }

  get hostname() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_components][3].patternString;
  }

  get port() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_components][4].patternString;
  }

  get pathname() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_components][5].patternString;
  }

  get search() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_components][6].patternString;
  }

  get hash() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_components][7].patternString;
  }

  get hasRegExpGroups() {
    webidl.assertBranded(this, URLPatternPrototype);
    return this[_hasRegExpGroups];
  }

  /**
   * @param {URLPatternInput} input
   * @param {string} [baseURL]
   * @returns {boolean}
   */
  test(input, baseURL = undefined) {
    webidl.assertBranded(this, URLPatternPrototype);
    const prefix = "Failed to execute 'test' on 'URLPattern'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    input = webidl.converters.URLPatternInput(input, prefix, "Argument 1");
    if (baseURL !== undefined) {
      baseURL = webidl.converters.USVString(baseURL, prefix, "Argument 2");
    }

    const values = baseURL === undefined
      ? matchInputCache.getOrInsert(input, processMatchInputCached)
      : processMatchInput(input, baseURL);
    if (values === null) return false;

    const components = this[_components];
    for (let i = 0; i < 8; ++i) {
      const component = components[i];
      switch (component.regexpString) {
        case "^$":
          if (values[i] !== "") return false;
          break;
        case "^(.*)$":
          break;
        default: {
          if (!RegExpPrototypeTest(component.regexp, values[i])) return false;
        }
      }
    }

    return true;
  }

  /**
   * @param {URLPatternInput} input
   * @param {string} [baseURL]
   * @returns {URLPatternResult | null}
   */
  exec(input, baseURL = undefined) {
    webidl.assertBranded(this, URLPatternPrototype);
    const prefix = "Failed to execute 'exec' on 'URLPattern'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    input = webidl.converters.URLPatternInput(input, prefix, "Argument 1");
    if (baseURL !== undefined) {
      baseURL = webidl.converters.USVString(baseURL, prefix, "Argument 2");
    }

    const values = baseURL === undefined
      ? matchInputCache.getOrInsert(input, processMatchInputCached)
      : processMatchInput(input, baseURL);
    if (values === null) {
      return null;
    }

    // globalThis.allocAttempt++;
    this.#reusedResult ??= { inputs: [undefined] };
    const result = this.#reusedResult;
    // We don't construct the `inputs` until after the matching is done under
    // the assumption that most patterns do not match.

    const components = this[_components];

    for (let i = 0; i < 8; ++i) {
      const key = COMPONENTS_KEYS[i];
      /** @type {Component} */
      const component = components[i];

      const res = result[key] ??= {
        input: values[i],
        groups: component.regexpString === "^(.*)$" ? { "0": values[i] } : {},
      };

      switch (component.regexpString) {
        case "^$":
          if (values[i] !== "") return null;
          break;
        case "^(.*)$":
          res.groups["0"] = values[i];
          break;
        default: {
          const match = RegExpPrototypeExec(component.regexp, values[i]);
          if (match === null) return null;
          const groupList = component.groupNameList;
          const groups = res.groups;
          for (let j = 0; j < groupList.length; ++j) {
            // TODO(lucacasonato): this is vulnerable to override mistake
            if (urlPatternSettings.groupStringFallback) {
              groups[groupList[j]] = match[j + 1] ?? "";
            } else {
              groups[groupList[j]] = match[j + 1];
            }
          }
          break;
        }
      }
      res.input = values[i];
    }

    // Reconstruct inputs from the original arguments (the op no longer
    // returns them -- they were a pass-through of the caller's arguments).
    result.inputs[0] = typeof input === "string"
      ? input
      : ObjectAssign(ObjectCreate(null), input);
    if (baseURL !== undefined) ArrayPrototypePush(result.inputs, baseURL);

    this.#reusedResult = undefined;
    return result;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(URLPatternPrototype, this),
        keys: [
          "protocol",
          "username",
          "password",
          "hostname",
          "port",
          "pathname",
          "search",
          "hash",
          "hasRegExpGroups",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(URLPattern);
const URLPatternPrototype = URLPattern.prototype;

webidl.converters.URLPatternInit = webidl
  .createDictionaryConverter("URLPatternInit", [
    { key: "protocol", converter: webidl.converters.USVString },
    { key: "username", converter: webidl.converters.USVString },
    { key: "password", converter: webidl.converters.USVString },
    { key: "hostname", converter: webidl.converters.USVString },
    { key: "port", converter: webidl.converters.USVString },
    { key: "pathname", converter: webidl.converters.USVString },
    { key: "search", converter: webidl.converters.USVString },
    { key: "hash", converter: webidl.converters.USVString },
    { key: "baseURL", converter: webidl.converters.USVString },
  ]);

webidl.converters["URLPatternInput"] = (V, prefix, context, opts) => {
  // Union for (URLPatternInit or USVString)
  if (typeof V == "object") {
    return webidl.converters.URLPatternInit(V, prefix, context, opts);
  }
  return webidl.converters.USVString(V, prefix, context, opts);
};

webidl.converters.URLPatternOptions = webidl
  .createDictionaryConverter("URLPatternOptions", [
    {
      key: "ignoreCase",
      converter: webidl.converters.boolean,
      defaultValue: false,
    },
  ]);

export { URLPattern, urlPatternSettings };
