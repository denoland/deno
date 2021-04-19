// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../file/internal.d.ts" />
/// <reference path="../file/lib.deno_file.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./11_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const {
    HTTP_TAB_OR_SPACE_PREFIX_RE,
    HTTP_TAB_OR_SPACE_SUFFIX_RE,
    HTTP_WHITESPACE_PREFIX_RE,
    HTTP_WHITESPACE_SUFFIX_RE,
    HTTP_TOKEN_CODE_POINT_RE,
    byteLowerCase,
    collectSequenceOfCodepoints,
    collectHttpQuotedString,
  } = window.__bootstrap.infra;

  const _headerList = Symbol("header list");
  const _iterableHeaders = Symbol("iterable headers");
  const _guard = Symbol("guard");

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
    potentialValue = potentialValue.replaceAll(HTTP_WHITESPACE_PREFIX_RE, "");
    potentialValue = potentialValue.replaceAll(HTTP_WHITESPACE_SUFFIX_RE, "");
    return potentialValue;
  }

  /**
   * @param {Headers} headers
   * @param {HeadersInit} object
   */
  function fillHeaders(headers, object) {
    if (Array.isArray(object)) {
      for (const header of object) {
        if (header.length !== 2) {
          throw new TypeError(
            `Invalid header. Length must be 2, but is ${header.length}`,
          );
        }
        appendHeader(headers, header[0], header[1]);
      }
    } else {
      for (const key of Object.keys(object)) {
        appendHeader(headers, key, object[key]);
      }
    }
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
    if (!HTTP_TOKEN_CODE_POINT_RE.test(name)) {
      throw new TypeError("Header name is not valid.");
    }
    if (
      value.includes("\x00") || value.includes("\x0A") || value.includes("\x0D")
    ) {
      throw new TypeError("Header value is not valid.");
    }

    // 3.
    if (headers[_guard] == "immutable") {
      throw new TypeError("Headers are immutable.");
    }

    // 7.
    const list = headers[_headerList];
    const lowercaseName = byteLowerCase(name);
    for (let i = 0; i < list.length; i++) {
      if (byteLowerCase(list[i][0]) === lowercaseName) {
        name = list[i][0];
        break;
      }
    }
    list.push([name, value]);
  }

  /**
   * @param {HeaderList} list
   * @param {string} name
   */
  function getHeader(list, name) {
    const lowercaseName = byteLowerCase(name);
    const entries = list.filter((entry) =>
      byteLowerCase(entry[0]) === lowercaseName
    ).map((entry) => entry[1]);
    if (entries.length === 0) {
      return null;
    } else {
      return entries.join("\x2C\x20");
    }
  }

  /**
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

      value = value.replaceAll(HTTP_TAB_OR_SPACE_PREFIX_RE, "");
      value = value.replaceAll(HTTP_TAB_OR_SPACE_SUFFIX_RE, "");

      values.push(value);
      value = "";
    }
    return values;
  }

  class Headers {
    /** @type {HeaderList} */
    [_headerList] = [];
    /** @type {"immutable" | "request" | "request-no-cors" | "response" | "none"} */
    [_guard];

    get [_iterableHeaders]() {
      const list = this[_headerList];

      const headers = [];
      const headerNamesSet = new Set();
      for (const entry of list) {
        headerNamesSet.add(byteLowerCase(entry[0]));
      }
      const names = [...headerNamesSet].sort();
      for (const name of names) {
        // The following if statement, and if block of the following statement
        // are not spec compliant. `set-cookie` is the only header that can not
        // be concatentated, so must be given to the user as multiple headers.
        // The else block of the if statement is spec compliant again.
        if (name == "set-cookie") {
          const setCookie = list.filter((entry) =>
            byteLowerCase(entry[0]) === "set-cookie"
          );
          if (setCookie.length === 0) throw new TypeError("Unreachable");
          for (const entry of setCookie) {
            headers.push([name, entry[1]]);
          }
        } else {
          const value = getHeader(list, name);
          if (value === null) throw new TypeError("Unreachable");
          headers.push([name, value]);
        }
      }
      return headers;
    }

    /** @param {HeadersInit} [init] */
    constructor(init = undefined) {
      const prefix = "Failed to construct 'Event'";
      if (init !== undefined) {
        init = webidl.converters["HeadersInit"](init, {
          prefix,
          context: "Argument 1",
        });
      }

      this[webidl.brand] = webidl.brand;
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
      webidl.assertBranded(this, Headers);
      const prefix = "Failed to execute 'append' on 'Headers'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      name = webidl.converters["ByteString"](name, {
        prefix,
        context: "Argument 1",
      });
      value = webidl.converters["ByteString"](value, {
        prefix,
        context: "Argument 2",
      });
      appendHeader(this, name, value);
    }

    /**
     * @param {string} name 
     */
    delete(name) {
      const prefix = "Failed to execute 'delete' on 'Headers'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters["ByteString"](name, {
        prefix,
        context: "Argument 1",
      });

      if (!HTTP_TOKEN_CODE_POINT_RE.test(name)) {
        throw new TypeError("Header name is not valid.");
      }
      if (this[_guard] == "immutable") {
        throw new TypeError("Headers are immutable.");
      }

      const list = this[_headerList];
      const lowercaseName = byteLowerCase(name);
      for (let i = 0; i < list.length; i++) {
        if (byteLowerCase(list[i][0]) === lowercaseName) {
          list.splice(i, 1);
          i--;
        }
      }
    }

    /**
     * @param {string} name 
     */
    get(name) {
      const prefix = "Failed to execute 'get' on 'Headers'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters["ByteString"](name, {
        prefix,
        context: "Argument 1",
      });

      if (!HTTP_TOKEN_CODE_POINT_RE.test(name)) {
        throw new TypeError("Header name is not valid.");
      }

      const list = this[_headerList];
      return getHeader(list, name);
    }

    /**
     * @param {string} name 
     */
    has(name) {
      const prefix = "Failed to execute 'has' on 'Headers'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters["ByteString"](name, {
        prefix,
        context: "Argument 1",
      });

      if (!HTTP_TOKEN_CODE_POINT_RE.test(name)) {
        throw new TypeError("Header name is not valid.");
      }

      const list = this[_headerList];
      const lowercaseName = byteLowerCase(name);
      for (let i = 0; i < list.length; i++) {
        if (byteLowerCase(list[i][0]) === lowercaseName) {
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
      webidl.assertBranded(this, Headers);
      const prefix = "Failed to execute 'set' on 'Headers'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      name = webidl.converters["ByteString"](name, {
        prefix,
        context: "Argument 1",
      });
      value = webidl.converters["ByteString"](value, {
        prefix,
        context: "Argument 2",
      });

      value = normalizeHeaderValue(value);

      // 2.
      if (!HTTP_TOKEN_CODE_POINT_RE.test(name)) {
        throw new TypeError("Header name is not valid.");
      }
      if (
        value.includes("\x00") || value.includes("\x0A") ||
        value.includes("\x0D")
      ) {
        throw new TypeError("Header value is not valid.");
      }

      if (this[_guard] == "immutable") {
        throw new TypeError("Headers are immutable.");
      }

      const list = this[_headerList];
      const lowercaseName = byteLowerCase(name);
      let added = false;
      for (let i = 0; i < list.length; i++) {
        if (byteLowerCase(list[i][0]) === lowercaseName) {
          if (!added) {
            list[i][1] = value;
            added = true;
          } else {
            list.splice(i, 1);
            i--;
          }
        }
      }
      if (!added) {
        list.push([name, value]);
      }
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      const headers = {};
      for (const header of this) {
        headers[header[0]] = header[1];
      }
      return `Headers ${inspect(headers)}`;
    }

    get [Symbol.toStringTag]() {
      return "Headers";
    }
  }

  webidl.mixinPairIterable("Headers", Headers, _iterableHeaders, 0, 1);

  webidl.converters["sequence<ByteString>"] = webidl
    .createSequenceConverter(webidl.converters["ByteString"]);
  webidl.converters["sequence<sequence<ByteString>>"] = webidl
    .createSequenceConverter(webidl.converters["sequence<ByteString>"]);
  webidl.converters["record<ByteString, ByteString>"] = webidl
    .createRecordConverter(
      webidl.converters["ByteString"],
      webidl.converters["ByteString"],
    );
  webidl.converters["HeadersInit"] = (V, opts) => {
    // Union for (sequence<sequence<ByteString>> or record<ByteString, ByteString>)
    if (typeof V === "object" && V !== null) {
      if (V[Symbol.iterator] !== undefined) {
        return webidl.converters["sequence<sequence<ByteString>>"](V, opts);
      }
      return webidl.converters["record<ByteString, ByteString>"](V, opts);
    }
    throw webidl.makeException(
      TypeError,
      "The provided value is not of type '(sequence<sequence<ByteString>> or record<ByteString, ByteString>)'",
      opts,
    );
  };
  webidl.converters["Headers"] = webidl.createInterfaceConverter(
    "Headers",
    Headers,
  );

  /**
   * @param {HeaderList} list
   * @param {"immutable" | "request" | "request-no-cors" | "response" | "none"} guard
   * @returns {Headers}
   */
  function headersFromHeaderList(list, guard) {
    const headers = webidl.createBranded(Headers);
    headers[_headerList] = list;
    headers[_guard] = guard;
    return headers;
  }

  /**
   * @param {Headers}
   * @returns {HeaderList}
   */
  function headerListFromHeaders(headers) {
    return headers[_headerList];
  }

  /**
   * @param {Headers}
   * @returns {"immutable" | "request" | "request-no-cors" | "response" | "none"}
   */
  function guardFromHeaders(headers) {
    return headers[_guard];
  }

  window.__bootstrap.headers = {
    Headers,
    headersFromHeaderList,
    headerListFromHeaders,
    fillHeaders,
    getDecodeSplitHeader,
    guardFromHeaders,
  };
})(this);
