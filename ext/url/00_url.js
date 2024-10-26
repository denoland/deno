// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />

import { primordials } from "ext:core/mod.js";
import {
  op_url_get_serialization,
  op_url_parse,
  op_url_parse_search_params,
  op_url_parse_with_base,
  op_url_reparse,
  op_url_stringify_search_params,
} from "ext:core/ops";
const {
  ArrayIsArray,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeSome,
  ArrayPrototypeSort,
  ArrayPrototypeSplice,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  SafeArrayIterator,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  Symbol,
  SymbolFor,
  SymbolIterator,
  TypeError,
  Uint32Array,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";

const _list = Symbol("list");
const _urlObject = Symbol("url object");

// WARNING: must match rust code's UrlSetter::*
const SET_HASH = 0;
const SET_HOST = 1;
const SET_HOSTNAME = 2;
const SET_PASSWORD = 3;
const SET_PATHNAME = 4;
const SET_PORT = 5;
const SET_PROTOCOL = 6;
const SET_SEARCH = 7;
const SET_USERNAME = 8;

// Helper functions
/**
 * @param {string} href
 * @param {number} setter
 * @param {string} value
 * @returns {string}
 */
function opUrlReparse(href, setter, value) {
  const status = op_url_reparse(
    href,
    setter,
    value,
    componentsBuf,
  );
  return getSerialization(status, href);
}

/**
 * @param {string} href
 * @param {string} [maybeBase]
 * @returns {number}
 */
function opUrlParse(href, maybeBase) {
  if (maybeBase === undefined) {
    return op_url_parse(href, componentsBuf);
  }
  return op_url_parse_with_base(
    href,
    maybeBase,
    componentsBuf,
  );
}

/**
 * @param {number} status
 * @param {string} href
 * @param {string} [maybeBase]
 * @returns {string}
 */
function getSerialization(status, href, maybeBase) {
  if (status === 0) {
    return href;
  } else if (status === 1) {
    return op_url_get_serialization();
  } else {
    throw new TypeError(
      `Invalid URL: '${href}'` +
        (maybeBase ? ` with base '${maybeBase}'` : ""),
    );
  }
}

class URLSearchParams {
  [_list];
  [_urlObject] = null;

  /**
   * @param {string | [string][] | Record<string, string>} init
   */
  constructor(init = "") {
    const prefix = "Failed to construct 'URL'";
    init = webidl.converters
      ["sequence<sequence<USVString>> or record<USVString, USVString> or USVString"](
        init,
        prefix,
        "Argument 1",
      );
    this[webidl.brand] = webidl.brand;
    if (!init) {
      // if there is no query string, return early
      this[_list] = [];
      return;
    }

    if (typeof init === "string") {
      // Overload: USVString
      // If init is a string and starts with U+003F (?),
      // remove the first code point from init.
      if (init[0] == "?") {
        init = StringPrototypeSlice(init, 1);
      }
      this[_list] = op_url_parse_search_params(init);
    } else if (ArrayIsArray(init)) {
      // Overload: sequence<sequence<USVString>>
      this[_list] = ArrayPrototypeMap(init, (pair, i) => {
        if (pair.length !== 2) {
          throw new TypeError(
            `${prefix}: Item ${
              i + 0
            } in the parameter list does have length 2 exactly`,
          );
        }
        return [pair[0], pair[1]];
      });
    } else {
      // Overload: record<USVString, USVString>
      this[_list] = ArrayPrototypeMap(
        ObjectKeys(init),
        (key) => [key, init[key]],
      );
    }
  }

  #updateUrlSearch() {
    const url = this[_urlObject];
    if (url === null) {
      return;
    }
    // deno-lint-ignore prefer-primordials
    url[_updateUrlSearch](this.toString());
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  append(name, value) {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    const prefix = "Failed to execute 'append' on 'URLSearchParams'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    name = webidl.converters.USVString(name, prefix, "Argument 1");
    value = webidl.converters.USVString(value, prefix, "Argument 2");
    ArrayPrototypePush(this[_list], [name, value]);
    this.#updateUrlSearch();
  }

  /**
   * @param {string} name
   * @param {string} [value]
   */
  delete(name, value = undefined) {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    const prefix = "Failed to execute 'append' on 'URLSearchParams'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters.USVString(name, prefix, "Argument 1");
    const list = this[_list];
    let i = 0;
    if (value === undefined) {
      while (i < list.length) {
        if (list[i][0] === name) {
          ArrayPrototypeSplice(list, i, 1);
        } else {
          i++;
        }
      }
    } else {
      value = webidl.converters.USVString(value, prefix, "Argument 2");
      while (i < list.length) {
        if (list[i][0] === name && list[i][1] === value) {
          ArrayPrototypeSplice(list, i, 1);
        } else {
          i++;
        }
      }
    }
    this.#updateUrlSearch();
  }

  /**
   * @param {string} name
   * @returns {string[]}
   */
  getAll(name) {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    const prefix = "Failed to execute 'getAll' on 'URLSearchParams'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters.USVString(name, prefix, "Argument 1");
    const values = [];
    const entries = this[_list];
    for (let i = 0; i < entries.length; ++i) {
      const entry = entries[i];
      if (entry[0] === name) {
        ArrayPrototypePush(values, entry[1]);
      }
    }
    return values;
  }

  /**
   * @param {string} name
   * @return {string | null}
   */
  get(name) {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    const prefix = "Failed to execute 'get' on 'URLSearchParams'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters.USVString(name, prefix, "Argument 1");
    const entries = this[_list];
    for (let i = 0; i < entries.length; ++i) {
      const entry = entries[i];
      if (entry[0] === name) {
        return entry[1];
      }
    }
    return null;
  }

  /**
   * @param {string} name
   * @param {string} [value]
   * @return {boolean}
   */
  has(name, value = undefined) {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    const prefix = "Failed to execute 'has' on 'URLSearchParams'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters.USVString(name, prefix, "Argument 1");
    if (value !== undefined) {
      value = webidl.converters.USVString(value, prefix, "Argument 2");
      return ArrayPrototypeSome(
        this[_list],
        (entry) => entry[0] === name && entry[1] === value,
      );
    }
    return ArrayPrototypeSome(this[_list], (entry) => entry[0] === name);
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  set(name, value) {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    const prefix = "Failed to execute 'set' on 'URLSearchParams'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    name = webidl.converters.USVString(name, prefix, "Argument 1");
    value = webidl.converters.USVString(value, prefix, "Argument 2");

    const list = this[_list];

    // If there are any name-value pairs whose name is name, in list,
    // set the value of the first such name-value pair to value
    // and remove the others.
    let found = false;
    let i = 0;
    while (i < list.length) {
      if (list[i][0] === name) {
        if (!found) {
          list[i][1] = value;
          found = true;
          i++;
        } else {
          ArrayPrototypeSplice(list, i, 1);
        }
      } else {
        i++;
      }
    }

    // Otherwise, append a new name-value pair whose name is name
    // and value is value, to list.
    if (!found) {
      ArrayPrototypePush(list, [name, value]);
    }

    this.#updateUrlSearch();
  }

  sort() {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    ArrayPrototypeSort(
      this[_list],
      (a, b) => (a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1),
    );
    this.#updateUrlSearch();
  }

  /**
   * @return {string}
   */
  toString() {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    return op_url_stringify_search_params(this[_list]);
  }

  get size() {
    webidl.assertBranded(this, URLSearchParamsPrototype);
    return this[_list].length;
  }
}

webidl.mixinPairIterable("URLSearchParams", URLSearchParams, _list, 0, 1);

webidl.configureInterface(URLSearchParams);
const URLSearchParamsPrototype = URLSearchParams.prototype;

webidl.converters["URLSearchParams"] = webidl.createInterfaceConverter(
  "URLSearchParams",
  URLSearchParamsPrototype,
);

const _updateUrlSearch = Symbol("updateUrlSearch");

function trim(s) {
  if (s.length === 1) return "";
  return s;
}

// Represents a "no port" value. A port in URL cannot be greater than 2^16 - 1
const NO_PORT = 65536;

const skipInit = Symbol();
const componentsBuf = new Uint32Array(8);

class URL {
  /** @type {URLSearchParams|null} */
  #queryObject = null;
  /** @type {string} */
  #serialization;
  /** @type {number} */
  #schemeEnd;
  /** @type {number} */
  #usernameEnd;
  /** @type {number} */
  #hostStart;
  /** @type {number} */
  #hostEnd;
  /** @type {number} */
  #port;
  /** @type {number} */
  #pathStart;
  /** @type {number} */
  #queryStart;
  /** @type {number} */
  #fragmentStart;

  [_updateUrlSearch](value) {
    this.#serialization = opUrlReparse(
      this.#serialization,
      SET_SEARCH,
      value,
    );
    this.#updateComponents();
  }

  /**
   * @param {string} url
   * @param {string} [base]
   */
  constructor(url, base = undefined) {
    // skip initialization for URL.parse
    if (url === skipInit) {
      return;
    }
    const prefix = "Failed to construct 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    url = webidl.converters.DOMString(url, prefix, "Argument 1");
    if (base !== undefined) {
      base = webidl.converters.DOMString(base, prefix, "Argument 2");
    }
    const status = opUrlParse(url, base);
    this[webidl.brand] = webidl.brand;
    this.#serialization = getSerialization(status, url, base);
    this.#updateComponents();
  }

  /**
   * @param {string} url
   * @param {string} [base]
   */
  static parse(url, base = undefined) {
    const prefix = "Failed to execute 'URL.parse'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    url = webidl.converters.DOMString(url, prefix, "Argument 1");
    if (base !== undefined) {
      base = webidl.converters.DOMString(base, prefix, "Argument 2");
    }
    const status = opUrlParse(url, base);
    if (status !== 0 && status !== 1) {
      return null;
    }
    // If initialized with webidl.createBranded, private properties are not be accessible,
    // so it is passed through the constructor
    const self = new this(skipInit);
    self[webidl.brand] = webidl.brand;
    self.#serialization = getSerialization(status, url, base);
    self.#updateComponents();
    return self;
  }

  /**
   * @param {string} url
   * @param {string} [base]
   */
  static canParse(url, base = undefined) {
    const prefix = "Failed to execute 'URL.canParse'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    url = webidl.converters.DOMString(url, prefix, "Argument 1");
    if (base !== undefined) {
      base = webidl.converters.DOMString(base, prefix, "Argument 2");
    }
    const status = opUrlParse(url, base);
    return status === 0 || status === 1;
  }

  #updateComponents() {
    ({
      0: this.#schemeEnd,
      1: this.#usernameEnd,
      2: this.#hostStart,
      3: this.#hostEnd,
      4: this.#port,
      5: this.#pathStart,
      6: this.#queryStart,
      7: this.#fragmentStart,
    } = componentsBuf);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(URLPrototype, this),
        keys: [
          "href",
          "origin",
          "protocol",
          "username",
          "password",
          "host",
          "hostname",
          "port",
          "pathname",
          "hash",
          "search",
        ],
      }),
      inspectOptions,
    );
  }

  #updateSearchParams() {
    if (this.#queryObject !== null) {
      const params = this.#queryObject[_list];
      const newParams = op_url_parse_search_params(
        StringPrototypeSlice(this.search, 1),
      );
      ArrayPrototypeSplice(
        params,
        0,
        params.length,
        ...new SafeArrayIterator(newParams),
      );
    }
  }

  #hasAuthority() {
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/lib.rs#L824
    return StringPrototypeStartsWith(
      StringPrototypeSlice(this.#serialization, this.#schemeEnd),
      "://",
    );
  }

  /** @return {string} */
  get hash() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/quirks.rs#L263
    return this.#fragmentStart
      ? trim(StringPrototypeSlice(this.#serialization, this.#fragmentStart))
      : "";
  }

  /** @param {string} value */
  set hash(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'hash' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_HASH,
        value,
      );
      this.#updateComponents();
    } catch {
      /* pass */
    }
  }

  /** @return {string} */
  get host() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/quirks.rs#L101
    return StringPrototypeSlice(
      this.#serialization,
      this.#hostStart,
      this.#pathStart,
    );
  }

  /** @param {string} value */
  set host(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'host' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_HOST,
        value,
      );
      this.#updateComponents();
    } catch {
      /* pass */
    }
  }

  /** @return {string} */
  get hostname() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/lib.rs#L988
    return StringPrototypeSlice(
      this.#serialization,
      this.#hostStart,
      this.#hostEnd,
    );
  }

  /** @param {string} value */
  set hostname(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'hostname' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_HOSTNAME,
        value,
      );
      this.#updateComponents();
    } catch {
      /* pass */
    }
  }

  /** @return {string} */
  get href() {
    webidl.assertBranded(this, URLPrototype);
    return this.#serialization;
  }

  /** @param {string} value */
  set href(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'href' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    const status = opUrlParse(value);
    this.#serialization = getSerialization(status, value);
    this.#updateComponents();
    this.#updateSearchParams();
  }

  /** @return {string} */
  get origin() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/origin.rs#L14
    const scheme = StringPrototypeSlice(
      this.#serialization,
      0,
      this.#schemeEnd,
    );
    if (
      scheme === "http" || scheme === "https" || scheme === "ftp" ||
      scheme === "ws" || scheme === "wss"
    ) {
      return `${scheme}://${this.host}`;
    }

    if (scheme === "blob") {
      // TODO(@littledivy): Fast path.
      try {
        return new URL(this.pathname).origin;
      } catch {
        return "null";
      }
    }

    return "null";
  }

  /** @return {string} */
  get password() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/lib.rs#L914
    if (
      this.#hasAuthority() &&
      this.#usernameEnd !== this.#serialization.length &&
      this.#serialization[this.#usernameEnd] === ":"
    ) {
      return StringPrototypeSlice(
        this.#serialization,
        this.#usernameEnd + 1,
        this.#hostStart - 1,
      );
    }
    return "";
  }

  /** @param {string} value */
  set password(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'password' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_PASSWORD,
        value,
      );
      this.#updateComponents();
    } catch {
      /* pass */
    }
  }

  /** @return {string} */
  get pathname() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/lib.rs#L1203
    if (!this.#queryStart && !this.#fragmentStart) {
      return StringPrototypeSlice(this.#serialization, this.#pathStart);
    }

    const nextComponentStart = this.#queryStart || this.#fragmentStart;
    return StringPrototypeSlice(
      this.#serialization,
      this.#pathStart,
      nextComponentStart,
    );
  }

  /** @param {string} value */
  set pathname(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'pathname' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_PATHNAME,
        value,
      );
      this.#updateComponents();
    } catch {
      /* pass */
    }
  }

  /** @return {string} */
  get port() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/quirks.rs#L196
    if (this.#port === NO_PORT) {
      return StringPrototypeSlice(
        this.#serialization,
        this.#hostEnd,
        this.#pathStart,
      );
    } else {
      return StringPrototypeSlice(
        this.#serialization,
        this.#hostEnd + 1, /* : */
        this.#pathStart,
      );
    }
  }

  /** @param {string} value */
  set port(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'port' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_PORT,
        value,
      );
      this.#updateComponents();
    } catch {
      /* pass */
    }
  }

  /** @return {string} */
  get protocol() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/quirks.rs#L56
    return StringPrototypeSlice(
      this.#serialization,
      0,
      this.#schemeEnd + 1, /* : */
    );
  }

  /** @param {string} value */
  set protocol(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'protocol' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_PROTOCOL,
        value,
      );
      this.#updateComponents();
    } catch {
      /* pass */
    }
  }

  /** @return {string} */
  get search() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/quirks.rs#L249
    const afterPath = this.#queryStart || this.#fragmentStart ||
      this.#serialization.length;
    const afterQuery = this.#fragmentStart || this.#serialization.length;
    return trim(
      StringPrototypeSlice(this.#serialization, afterPath, afterQuery),
    );
  }

  /** @param {string} value */
  set search(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'search' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_SEARCH,
        value,
      );
      this.#updateComponents();
      this.#updateSearchParams();
    } catch {
      /* pass */
    }
  }

  /** @return {string} */
  get username() {
    webidl.assertBranded(this, URLPrototype);
    // https://github.com/servo/rust-url/blob/1d307ae51a28fecc630ecec03380788bfb03a643/url/src/lib.rs#L881
    const schemeSeparatorLen = 3; /* :// */
    if (
      this.#hasAuthority() &&
      this.#usernameEnd > this.#schemeEnd + schemeSeparatorLen
    ) {
      return StringPrototypeSlice(
        this.#serialization,
        this.#schemeEnd + schemeSeparatorLen,
        this.#usernameEnd,
      );
    } else {
      return "";
    }
  }

  /** @param {string} value */
  set username(value) {
    webidl.assertBranded(this, URLPrototype);
    const prefix = "Failed to set 'username' on 'URL'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    value = webidl.converters.DOMString(value, prefix, "Argument 1");
    try {
      this.#serialization = opUrlReparse(
        this.#serialization,
        SET_USERNAME,
        value,
      );
      this.#updateComponents();
    } catch {
      /* pass */
    }
  }

  /** @return {URLSearchParams} */
  get searchParams() {
    if (this.#queryObject == null) {
      this.#queryObject = new URLSearchParams(this.search);
      this.#queryObject[_urlObject] = this;
    }
    return this.#queryObject;
  }

  /** @return {string} */
  toString() {
    webidl.assertBranded(this, URLPrototype);
    return this.#serialization;
  }

  /** @return {string} */
  toJSON() {
    webidl.assertBranded(this, URLPrototype);
    return this.#serialization;
  }
}

webidl.configureInterface(URL);
const URLPrototype = URL.prototype;

/**
 * This function implements application/x-www-form-urlencoded parsing.
 * https://url.spec.whatwg.org/#concept-urlencoded-parser
 * @param {Uint8Array} bytes
 * @returns {[string, string][]}
 */
function parseUrlEncoded(bytes) {
  return op_url_parse_search_params(null, bytes);
}

webidl
  .converters[
    "sequence<sequence<USVString>> or record<USVString, USVString> or USVString"
  ] = (V, prefix, context, opts) => {
    // Union for (sequence<sequence<USVString>> or record<USVString, USVString> or USVString)
    if (webidl.type(V) === "Object" && V !== null) {
      if (V[SymbolIterator] !== undefined) {
        return webidl.converters["sequence<sequence<USVString>>"](
          V,
          prefix,
          context,
          opts,
        );
      }
      return webidl.converters["record<USVString, USVString>"](
        V,
        prefix,
        context,
        opts,
      );
    }
    return webidl.converters.USVString(V, prefix, context, opts);
  };

export {
  parseUrlEncoded,
  URL,
  URLPrototype,
  URLSearchParams,
  URLSearchParamsPrototype,
};
