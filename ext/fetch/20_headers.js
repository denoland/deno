// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayFrom,
  ArrayIsArray,
  ArrayPrototypePush,
  ArrayPrototypeSort,
  ArrayPrototypeSplice,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  RegExpPrototypeTest,
  SafeArrayIterator,
  Symbol,
  SymbolFor,
  SymbolIterator,
  StringPrototypeReplaceAll,
  StringPrototypeCharCodeAt,
  TypeError,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { markNotSerializable } = core.loadExtScript(
  "ext:deno_web/13_message_port.js",
);
const {
  byteLowerCase,
  collectHttpQuotedString,
  collectSequenceOfCodepoints,
  HTTP_TAB_OR_SPACE_PREFIX_RE,
  HTTP_TAB_OR_SPACE_SUFFIX_RE,
  HTTP_TOKEN_CODE_POINT_RE,
  httpTrim,
} = core.loadExtScript("ext:deno_web/00_infra.js");

const _headerList = Symbol("header list");
const _lowerNames = Symbol("lowercase header names");
const _iterableHeaders = Symbol("iterable headers");
const _iterableHeadersCache = Symbol("iterable headers cache");
const _iterableHeadersCacheListLength = Symbol(
  "iterable headers cache list length",
);
const _guard = Symbol("guard");
const _headerListGetter = Symbol("header list getter");
const _headerGet = Symbol("header get");
const _headerTarget = Symbol("header target");
const _brand = webidl.brand;
const webidlConverterByteString = webidl.converters.ByteString;
const webidlConverterSequenceByteString =
  webidl.converters["sequence<ByteString>"];

/**
 * Returns a parallel array to `headers[_headerList]` whose i-th entry is the
 * byte-lowercased form of `headers[_headerList][i][0]`.
 *
 * Rebuilt from scratch when the lengths diverge, which catches the
 * `Request` constructor's splice/refill block, which empties `_headerList`
 * without going through `appendHeader` / `set` / `delete`.
 *
 * `initializeAResponse` (`23_response.js`) reads via `ensureLowerNames`
 * directly and pushes to both arrays in lockstep, so its mutation no longer
 * relies on the length-divergence rebuild.
 */
function ensureLowerNames(headers) {
  let lower = headers[_lowerNames];
  const list = headerListFromHeaders(headers);
  if (lower === null || lower.length !== list.length) {
    lower = [];
    for (let i = 0; i < list.length; i++) {
      lower[i] = byteLowerCase(list[i][0]);
    }
    headers[_lowerNames] = lower;
  }
  return lower;
}

function invalidateIterableHeaders(headers) {
  headers[_iterableHeadersCache] = undefined;
  headers[_iterableHeadersCacheListLength] = undefined;
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

function isByteString(value) {
  for (let i = 0; i < value.length; i++) {
    if (StringPrototypeCharCodeAt(value, i) > 0xff) {
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
  const list = headerListFromHeaders(headers);
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
  invalidateIterableHeaders(headers);
}

function appendHeaderToList(list, name, value) {
  value = normalizeHeaderValue(value);
  if (!checkHeaderNameForHttpTokenCodePoint(name)) {
    throw new TypeError(`Invalid header name: "${name}"`);
  }
  if (!checkForInvalidValueChars(value)) {
    throw new TypeError(`Invalid header value: "${value}"`);
  }

  if (list.length !== 0) {
    const lowercaseName = byteLowerCase(name);
    for (let i = 0; i < list.length; i++) {
      if (byteLowerCase(list[i][0]) === lowercaseName) {
        name = list[i][0];
        break;
      }
    }
  }
  ArrayPrototypePush(list, [name, value]);
}

// Used by constructors to initialize a fresh list. This intentionally appends
// directly to `list`; callers must not use it to mutate guarded Headers.
function fillHeaderList(list, object, prefix, context, opts) {
  // Fast path: initializing from an existing `Headers` object, e.g.
  // `new Response(body, otherResponse)` / `new Request(input, { headers })` /
  // `new Headers(headers)`. This is extremely common (frameworks reconstruct
  // responses just to tweak a header). The source's entries are already
  // validated byte strings, so copy them directly and skip the expensive
  // webidl `sequence<sequence<ByteString>>` conversion plus per-entry
  // revalidation. `_iterableHeaders` yields the same combined+sorted entries
  // the slow path would produce, so the result is identical.
  if (ObjectPrototypeIsPrototypeOf(HeadersPrototype, object)) {
    // `_iterableHeaders` yields already-combined, sorted, lowercased and
    // validated entries (one per name, except set-cookie), which is exactly the
    // shape `fillHeaderList` builds for a fresh list -- so push them straight in
    // and skip both the webidl conversion and `appendHeaderToList`'s per-entry
    // revalidation and casing dedup.
    const entries = object[_iterableHeaders];
    for (let i = 0; i < entries.length; ++i) {
      const entry = entries[i];
      ArrayPrototypePush(list, [entry[0], entry[1]]);
    }
    return;
  }
  if (ArrayIsArray(object)) {
    for (let i = 0; i < object.length; ++i) {
      const header = webidlConverterSequenceByteString(
        object[i],
        prefix,
        `${context}, index ${i}`,
        opts,
      );
      if (header.length !== 2) {
        throw new TypeError(
          `Invalid header: length must be 2, but is ${header.length}`,
        );
      }
      appendHeaderToList(list, header[0], header[1]);
    }
    return;
  }

  if (
    typeof object === "object" && object !== null &&
    object[SymbolIterator] === undefined && !core.isProxy(object)
  ) {
    for (const key in object) {
      if (!ObjectHasOwn(object, key)) {
        continue;
      }
      const value = object[key];
      appendHeaderToList(
        list,
        key,
        typeof value === "string" && isByteString(value)
          ? value
          : webidlConverterByteString(value, prefix, context, opts),
      );
    }
    return;
  }

  fillHeaderList(
    list,
    webidlConverterHeadersInit(object, prefix, context, opts),
    prefix,
    context,
    opts,
  );
}

/**
 * https://fetch.spec.whatwg.org/#concept-header-list-get
 * @param {HeaderList} list
 * @param {string} name
 */
function getHeader(list, name) {
  const lowercaseName = byteLowerCase(name);
  let value = null;
  for (let i = 0; i < list.length; i++) {
    if (byteLowerCase(list[i][0]) === lowercaseName) {
      value = value === null ? list[i][1] : value + "\x2C\x20" + list[i][1];
    }
  }
  return value;
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
  [_headerListGetter] = null;
  [_headerGet] = null;
  [_headerTarget] = null;
  /** @type {"immutable" | "request" | "request-no-cors" | "response" | "none"} */
  [_guard];

  get [_iterableHeaders]() {
    const list = headerListFromHeaders(this);

    if (
      this[_iterableHeadersCache] !== undefined &&
      this[_iterableHeadersCacheListLength] === list.length
    ) {
      return this[_iterableHeadersCache];
    }

    // The order of steps are not similar to the ones suggested by the
    // spec but produce the same result.
    const seenHeaders = { __proto__: null };
    const entries = [];
    const lowerNames = ensureLowerNames(this);
    for (let i = 0; i < list.length; ++i) {
      const entry = list[i];
      const name = lowerNames[i];
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
    this[_iterableHeadersCacheListLength] = list.length;

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

    const list = headerListFromHeaders(this);
    const lowerNames = ensureLowerNames(this);
    const lowercaseName = byteLowerCase(name);
    let writeIdx = 0;
    for (let i = 0; i < lowerNames.length; i++) {
      if (lowerNames[i] !== lowercaseName) {
        list[writeIdx] = list[i];
        lowerNames[writeIdx] = lowerNames[i];
        writeIdx++;
      }
    }
    if (writeIdx !== list.length) {
      ArrayPrototypeSplice(list, writeIdx);
      ArrayPrototypeSplice(lowerNames, writeIdx);
      invalidateIterableHeaders(this);
    }
  }

  /**
   * @param {string} name
   */
  get(name) {
    webidl.assertBranded(this, HeadersPrototype);
    const prefix = "Failed to execute 'get' on 'Headers'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    if (
      name === "authorization" || name === "content-type" ||
      name === "accept" || name === "host"
    ) {
      const headerGet = this[_headerGet];
      if (headerGet !== null) {
        return headerGet(name);
      }
      const headerTarget = this[_headerTarget];
      if (headerTarget !== null) {
        return headerTarget.header(name);
      }
      const list = headerListFromHeaders(this);
      return getHeader(list, name);
    }
    name = webidl.converters["ByteString"](name, prefix, "Argument 1");

    if (!checkHeaderNameForHttpTokenCodePoint(name)) {
      throw new TypeError(`Invalid header name: "${name}"`);
    }

    const headerGet = this[_headerGet];
    if (headerGet !== null) {
      return headerGet(name);
    }
    const headerTarget = this[_headerTarget];
    if (headerTarget !== null) {
      return headerTarget.header(name);
    }

    const list = headerListFromHeaders(this);
    // Inline `getHeader` so we can use the cached lower names and build the
    // joined value directly. For the dominant single-value case this also
    // skips the intermediate entries array and the trailing join.
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
    const list = headerListFromHeaders(this);
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

    const list = headerListFromHeaders(this);
    const lowerNames = ensureLowerNames(this);
    const lowercaseName = byteLowerCase(name);
    let writeIdx = 0;
    let added = false;
    for (let i = 0; i < lowerNames.length; i++) {
      if (lowerNames[i] === lowercaseName) {
        if (!added) {
          const entry = list[i];
          entry[1] = value;
          list[writeIdx] = entry;
          lowerNames[writeIdx] = lowerNames[i];
          writeIdx++;
          added = true;
        }
      } else {
        list[writeIdx] = list[i];
        lowerNames[writeIdx] = lowerNames[i];
        writeIdx++;
      }
    }
    if (!added) {
      ArrayPrototypePush(list, [name, value]);
      ArrayPrototypePush(lowerNames, lowercaseName);
    } else if (writeIdx !== list.length) {
      ArrayPrototypeSplice(list, writeIdx);
      ArrayPrototypeSplice(lowerNames, writeIdx);
    }
    invalidateIterableHeaders(this);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    if (ObjectPrototypeIsPrototypeOf(HeadersPrototype, this)) {
      const headers = {};
      for (const entry of new SafeArrayIterator(ArrayFrom(this))) {
        const name = entry[0];
        let value = entry[1];
        if (ObjectHasOwn(headers, name)) {
          value = `${headers[name]}, ${value}`;
        }
        ObjectDefineProperty(headers, name, {
          __proto__: null,
          value,
          enumerable: true,
          configurable: true,
          writable: true,
        });
      }
      return `${this.constructor.name} ${inspect(headers, inspectOptions)}`;
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
const webidlConverterHeadersInit = webidl.converters.HeadersInit;
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
  headers[_lowerNames] = null;
  headers[_guard] = guard;
  return headers;
}

function headersFromHeaderListLazy(headerList, guard, getHeader) {
  const headers = new Headers(_brand);
  headers[_headerList] = null;
  headers[_lowerNames] = null;
  headers[_headerListGetter] = headerList;
  headers[_headerGet] = getHeader;
  headers[_headerTarget] = null;
  headers[_guard] = guard;
  return headers;
}

function headersFromHeaderListLazyTarget(target, guard) {
  const headers = new Headers(_brand);
  headers[_headerList] = null;
  headers[_lowerNames] = null;
  headers[_headerListGetter] = null;
  headers[_headerGet] = null;
  headers[_headerTarget] = target;
  headers[_guard] = guard;
  return headers;
}

/**
 * Returns the underlying header list for direct mutation (used by
 * `initializeAResponse` and the `Request` constructor's splice/refill block).
 *
 * IMPORTANT: callers must change the list length when mutating (push, splice,
 * or empty-and-refill). Equal-length in-place mutations
 * (`list[i] = [...]` or `list[i][0] = ...`, pop-and-push pairs) silently leave
 * the parallel `[_lowerNames]` cache (see `ensureLowerNames`) and iterable
 * cache stale, because both rebuild triggers use the raw list length. There is
 * currently no caller doing this; the next one to reach for an equal-length
 * in-place mutation needs to add a cache-invalidator (and a test) alongside
 * it.
 *
 * @param {Headers} headers
 * @returns {HeaderList}
 */
function headerListFromHeaders(headers) {
  let list = headers[_headerList];
  if (list === null) {
    const target = headers[_headerTarget];
    list = target === null ? headers[_headerListGetter]() : target.headerList;
    headers[_headerList] = list;
    headers[_lowerNames] = null;
    headers[_headerListGetter] = null;
    headers[_headerGet] = null;
    headers[_headerTarget] = null;
  }
  return list;
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

return {
  fillHeaderList,
  ensureLowerNames,
  fillHeaders,
  getDecodeSplitHeader,
  getHeader,
  guardFromHeaders,
  headerListFromHeaders,
  Headers,
  headersEntries,
  headersFromHeaderList,
  headersFromHeaderListLazy,
  headersFromHeaderListLazyTarget,
};
})();
