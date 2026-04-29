// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />

import { primordials } from "ext:core/mod.js";
const {
  ArrayIsArray,
  ArrayPrototypePush,
  ArrayPrototypeSort,
  ArrayPrototypeJoin,
  ArrayPrototypeSplice,
  ObjectFromEntries,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  RegExpPrototypeTest,
  Symbol,
  SymbolFor,
  SymbolIterator,
  StringPrototypeReplaceAll,
  StringPrototypeCharCodeAt,
  TypeError,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { markNotSerializable } from "ext:deno_web/13_message_port.js";
import {
  byteLowerCase,
  collectHttpQuotedString,
  collectSequenceOfCodepoints,
  HTTP_TAB_OR_SPACE_PREFIX_RE,
  HTTP_TAB_OR_SPACE_SUFFIX_RE,
  HTTP_TOKEN_CODE_POINT_RE,
  httpTrim,
} from "ext:deno_web/00_infra.js";

const _headerList = Symbol("header list");
const _lowerNames = Symbol("lowercase header names");
const _iterableHeaders = Symbol("iterable headers");
const _iterableHeadersCache = Symbol("iterable headers cache");
const _guard = Symbol("guard");
const _brand = webidl.brand;

/**
 * Returns a parallel array to `headers[_headerList]` whose i-th entry is the
 * byte-lowercased form of `headers[_headerList][i][0]`.
 *
 * Rebuilt from scratch when the lengths diverge -- that catches the only
 * external mutation pattern in the codebase, where `initializeAResponse` and
 * `Request`'s splice/refill block push directly to `_headerList` without
 * going through `appendHeader` / `set` / `delete`.
 */
function ensureLowerNames(headers) {
  let lower = headers[_lowerNames];
  const list = headers[_headerList];
  if (lower === null || lower.length !== list.length) {
    lower = [];
    for (let i = 0; i < list.length; i++) {
      lower[i] = byteLowerCase(list[i][0]);
    }
    headers[_lowerNames] = lower;
  }
  return lower;
}

/**
 * @typedef Header
 * @type {[string, string]}
 */

/**
 * @typedef HeaderList
 * @type {Header[]}
 */

/**
 * @param {string} potentialValue
 * @returns {string}
 */
function normalizeHeaderValue(potentialValue) {
  return httpTrim(potentialValue);
}

/**
 * @param {Headers} headers
 * @param {HeadersInit} object
 */
function fillHeaders(headers, object) {
  if (ArrayIsArray(object)) {
    for (let i = 0; i < object.length; ++i) {
      const header = object[i];
      if (header.length !== 2) {
        throw new TypeError(
          `Invalid header: length must be 2, but is ${header.length}`,
        );
      }
      appendHeader(headers, header[0], header[1]);
    }
  } else {
    for (const key in object) {
      if (!ObjectHasOwn(object, key)) {
        continue;
      }
      appendHeader(headers, key, object[key]);
    }
  }
}

function checkForInvalidValueChars(value) {
  for (let i = 0; i < value.length; i++) {
    const c = StringPrototypeCharCodeAt(value, i);

    if (c === 0x0a || c === 0x0d || c === 0x00) {
      return false;
    }
  }

  return true;
}

let HEADER_NAME_CACHE = { __proto__: null };
let HEADER_CACHE_SIZE = 0;
const HEADER_NAME_CACHE_SIZE_BOUNDARY = 4096;
function checkHeaderNameForHttpTokenCodePoint(name) {
  const fromCache = HEADER_NAME_CACHE[name];
  if (fromCache !== undefined) {
    return fromCache;
  }

  const valid = RegExpPrototypeTest(HTTP_TOKEN_CODE_POINT_RE, name);

  if (HEADER_CACHE_SIZE > HEADER_NAME_CACHE_SIZE_BOUNDARY) {
    HEADER_NAME_CACHE = { __proto__: null };
    HEADER_CACHE_SIZE = 0;
  }
  HEADER_CACHE_SIZE++;
  HEADER_NAME_CACHE[name] = valid;

  return valid;
}

/**
 * https://fetch.spec.whatwg.org/#concept-headers-append
 * @param {Headers} headers
 * @param {string} name
 * @param {string} value
 */
function appendHeader(headers, name, value) {
  // 1.
  value = normalizeHeaderValue(value);

  // 2.
  if (!checkHeaderNameForHttpTokenCodePoint(name)) {
    throw new TypeError(`Invalid header name: "${name}"`);
  }
  if (!checkForInvalidValueChars(value)) {
    throw new TypeError(`Invalid header value: "${value}"`);
  }

  // 3.
  if (headers[_guard] == "immutable") {
    throw new TypeError("Cannot change header: headers are immutable");
  }

  // 7.
  const list = headers[_headerList];
  const lowerNames = ensureLowerNames(headers);
  const lowercaseName = byteLowerCase(name);
  for (let i = 0; i < lowerNames.length; i++) {
    if (lowerNames[i] === lowercaseName) {
      name = list[i][0];
      break;
    }
  }
  ArrayPrototypePush(list, [name, value]);
  ArrayPrototypePush(lowerNames, lowercaseName);
}

/**
 * https://fetch.spec.whatwg.org/#concept-header-list-get
 * @param {HeaderList} list
 * @param {string} name
 */
function getHeader(list, name) {
  const lowercaseName = byteLowerCase(name);
  const entries = [];
  for (let i = 0; i < list.length; i++) {
    if (byteLowerCase(list[i][0]) === lowercaseName) {
      ArrayPrototypePush(entries, list[i][1]);
    }
  }

  if (entries.length === 0) {
    return null;
  } else {
    return ArrayPrototypeJoin(entries, "\x2C\x20");
  }
}

/**
 * https://fetch.spec.whatwg.org/#concept-header-list-get-decode-split
 * @param {HeaderList} list
 * @param {string} name
 * @returns {string[] | null}
 */
function getDecodeSplitHeader(list, name) {
  const initialValue = getHeader(list, name);
  if (initialValue === null) return null;
  const input = initialValue;
  let position = 0;
  const values = [];
  let value = "";
  while (position < initialValue.length) {
    // 7.1. collect up to " or ,
    const res = collectSequenceOfCodepoints(
      initialValue,
      position,
      (c) => c !== "\u0022" && c !== "\u002C",
    );
    value += res.result;
    position = res.position;

    if (position < initialValue.length) {
      if (input[position] === "\u0022") {
        const res = collectHttpQuotedString(input, position, false);
        value += res.result;
        position = res.position;
        if (position < initialValue.length) {
          continue;
        }
      } else {
        if (input[position] !== "\u002C") throw new TypeError("Unreachable");
        position += 1;
      }
    }

    value = StringPrototypeReplaceAll(value, HTTP_TAB_OR_SPACE_PREFIX_RE, "");
    value = StringPrototypeReplaceAll(value, HTTP_TAB_OR_SPACE_SUFFIX_RE, "");

    ArrayPrototypePush(values, value);
    value = "";
  }
  return values;
}

class Headers {
  /** @type {HeaderList} */
  [_headerList] = [];
  /** @type {string[] | null} parallel to _headerList, lazily populated */
  [_lowerNames] = null;
  /** @type {"immutable" | "request" | "request-no-cors" | "response" | "none"} */
  [_guard];

  get [_iterableHeaders]() {
    const list = this[_headerList];

    if (
      this[_guard] === "immutable" &&
      this[_iterableHeadersCache] !== undefined
    ) {
      return this[_iterableHeadersCache];
    }

    // The order of steps are not similar to the ones suggested by the
    // spec but produce the same result.
    const seenHeaders = { __proto__: null };
    const entries = [];
    for (let i = 0; i < list.length; ++i) {
      const entry = list[i];
      const name = byteLowerCase(entry[0]);
      const value = entry[1];
      if (value === null) throw new TypeError("Unreachable");
      // The following if statement is not spec compliant.
      // `set-cookie` is the only header that can not be concatenated,
      // so must be given to the user as multiple headers.
      // The else block of the if statement is spec compliant again.
      if (name === "set-cookie") {
        ArrayPrototypePush(entries, [name, value]);
      } else {
        // The following code has the same behaviour as getHeader()
        // at the end of loop. But it avoids looping through the entire
        // list to combine multiple values with same header name. It
        // instead gradually combines them as they are found.
        const seenHeaderIndex = seenHeaders[name];
        if (seenHeaderIndex !== undefined) {
          const entryValue = entries[seenHeaderIndex][1];
          entries[seenHeaderIndex][1] = entryValue.length > 0
            ? entryValue + "\x2C\x20" + value
            : value;
        } else {
          seenHeaders[name] = entries.length; // store header index in entries array
          ArrayPrototypePush(entries, [name, value]);
        }
      }
    }

    ArrayPrototypeSort(
      entries,
      (a, b) => {
        const akey = a[0];
        const bkey = b[0];
        if (akey > bkey) return 1;
        if (akey < bkey) return -1;
        return 0;
      },
    );

    this[_iterableHeadersCache] = entries;

    return entries;
  }

  /** @param {HeadersInit} [init] */
  constructor(init = undefined) {
    if (init === _brand) {
      this[_brand] = _brand;
      return;
    }

    const prefix = "Failed to construct 'Headers'";
    if (init !== undefined) {
      init = webidl.converters["HeadersInit"](init, prefix, "Argument 1");
    }

    this[_brand] = _brand;
    this[_guard] = "none";
    if (init !== undefined) {
      fillHeaders(this, init);
    }
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  append(name, value) {
    webidl.assertBranded(this, HeadersPrototype);
    const prefix = "Failed to execute 'append' on 'Headers'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    name = webidl.converters["ByteString"](name, prefix, "Argument 1");
    value = webidl.converters["ByteString"](value, prefix, "Argument 2");
    appendHeader(this, name, value);
  }

  /**
   * @param {string} name
   */
  delete(name) {
    webidl.assertBranded(this, HeadersPrototype);
    const prefix = "Failed to execute 'delete' on 'Headers'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters["ByteString"](name, prefix, "Argument 1");

    if (!checkHeaderNameForHttpTokenCodePoint(name)) {
      throw new TypeError(`Invalid header name: "${name}"`);
    }
    if (this[_guard] == "immutable") {
      throw new TypeError("Cannot change headers: headers are immutable");
    }

    const list = this[_headerList];
    const lowerNames = ensureLowerNames(this);
    const lowercaseName = byteLowerCase(name);
    for (let i = 0; i < lowerNames.length; i++) {
      if (lowerNames[i] === lowercaseName) {
        ArrayPrototypeSplice(list, i, 1);
        ArrayPrototypeSplice(lowerNames, i, 1);
        i--;
      }
    }
  }

  /**
   * @param {string} name
   */
  get(name) {
    webidl.assertBranded(this, HeadersPrototype);
    const prefix = "Failed to execute 'get' on 'Headers'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters["ByteString"](name, prefix, "Argument 1");

    if (!checkHeaderNameForHttpTokenCodePoint(name)) {
      throw new TypeError(`Invalid header name: "${name}"`);
    }

    // Inline `getHeader` so we can use the cached lower names and build the
    // joined value directly. For the dominant single-value case this also
    // skips the intermediate entries array and the trailing join.
    const list = this[_headerList];
    const lowerNames = ensureLowerNames(this);
    const lowercaseName = byteLowerCase(name);
    let value = null;
    for (let i = 0; i < lowerNames.length; i++) {
      if (lowerNames[i] === lowercaseName) {
        value = value === null ? list[i][1] : value + "\x2C\x20" + list[i][1];
      }
    }
    return value;
  }

  getSetCookie() {
    webidl.assertBranded(this, HeadersPrototype);
    const list = this[_headerList];
    const lowerNames = ensureLowerNames(this);

    const entries = [];
    for (let i = 0; i < lowerNames.length; i++) {
      if (lowerNames[i] === "set-cookie") {
        ArrayPrototypePush(entries, list[i][1]);
      }
    }

    return entries;
  }

  /**
   * @param {string} name
   */
  has(name) {
    webidl.assertBranded(this, HeadersPrototype);
    const prefix = "Failed to execute 'has' on 'Headers'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters["ByteString"](name, prefix, "Argument 1");

    if (!checkHeaderNameForHttpTokenCodePoint(name)) {
      throw new TypeError(`Invalid header name: "${name}"`);
    }

    const lowerNames = ensureLowerNames(this);
    const lowercaseName = byteLowerCase(name);
    for (let i = 0; i < lowerNames.length; i++) {
      if (lowerNames[i] === lowercaseName) {
        return true;
      }
    }
    return false;
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  set(name, value) {
    webidl.assertBranded(this, HeadersPrototype);
    const prefix = "Failed to execute 'set' on 'Headers'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    name = webidl.converters["ByteString"](name, prefix, "Argument 1");
    value = webidl.converters["ByteString"](value, prefix, "Argument 2");

    value = normalizeHeaderValue(value);

    // 2.
    if (!checkHeaderNameForHttpTokenCodePoint(name)) {
      throw new TypeError(`Invalid header name: "${name}"`);
    }
    if (!checkForInvalidValueChars(value)) {
      throw new TypeError(`Invalid header value: "${value}"`);
    }

    if (this[_guard] == "immutable") {
      throw new TypeError("Cannot change headers: headers are immutable");
    }

    const list = this[_headerList];
    const lowerNames = ensureLowerNames(this);
    const lowercaseName = byteLowerCase(name);
    let added = false;
    for (let i = 0; i < lowerNames.length; i++) {
      if (lowerNames[i] === lowercaseName) {
        if (!added) {
          list[i][1] = value;
          added = true;
        } else {
          ArrayPrototypeSplice(list, i, 1);
          ArrayPrototypeSplice(lowerNames, i, 1);
          i--;
        }
      }
    }
    if (!added) {
      ArrayPrototypePush(list, [name, value]);
      ArrayPrototypePush(lowerNames, lowercaseName);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    if (ObjectPrototypeIsPrototypeOf(HeadersPrototype, this)) {
      return `${this.constructor.name} ${
        inspect(ObjectFromEntries(this), inspectOptions)
      }`;
    } else {
      return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
    }
  }
}

webidl.mixinPairIterable("Headers", Headers, _iterableHeaders, 0, 1);

webidl.configureInterface(Headers);
const HeadersPrototype = Headers.prototype;
markNotSerializable(HeadersPrototype);

webidl.converters["HeadersInit"] = (V, prefix, context, opts) => {
  // Union for (sequence<sequence<ByteString>> or record<ByteString, ByteString>)
  if (webidl.type(V) === "Object" && V !== null) {
    if (V[SymbolIterator] !== undefined) {
      return webidl.converters["sequence<sequence<ByteString>>"](
        V,
        prefix,
        context,
        opts,
      );
    }
    return webidl.converters["record<ByteString, ByteString>"](
      V,
      prefix,
      context,
      opts,
    );
  }
  throw webidl.makeException(
    TypeError,
    "The provided value is not of type '(sequence<sequence<ByteString>> or record<ByteString, ByteString>)'",
    prefix,
    context,
  );
};
webidl.converters["Headers"] = webidl.createInterfaceConverter(
  "Headers",
  Headers.prototype,
);

/**
 * @param {HeaderList} list
 * @param {"immutable" | "request" | "request-no-cors" | "response" | "none"} guard
 * @returns {Headers}
 */
function headersFromHeaderList(list, guard) {
  const headers = new Headers(_brand);
  headers[_headerList] = list;
  headers[_guard] = guard;
  return headers;
}

/**
 * @param {Headers} headers
 * @returns {HeaderList}
 */
function headerListFromHeaders(headers) {
  return headers[_headerList];
}

/**
 * @param {Headers} headers
 * @returns {"immutable" | "request" | "request-no-cors" | "response" | "none"}
 */
function guardFromHeaders(headers) {
  return headers[_guard];
}

/**
 * @param {Headers} headers
 * @returns {[string, string][]}
 */
function headersEntries(headers) {
  return headers[_iterableHeaders];
}

export {
  fillHeaders,
  getDecodeSplitHeader,
  getHeader,
  guardFromHeaders,
  headerListFromHeaders,
  Headers,
  headersEntries,
  headersFromHeaderList,
};
