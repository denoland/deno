// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const consoleInternal = window.__bootstrap.console;
  const { HTTP_TOKEN_CODE_POINT_RE, byteUpperCase } = window.__bootstrap.infra;
  const { URL } = window.__bootstrap.url;
  const { guardFromHeaders } = window.__bootstrap.headers;
  const { mixinBody, extractBody } = window.__bootstrap.fetchBody;
  const { getLocationHref } = window.__bootstrap.location;
  const { extractMimeType } = window.__bootstrap.mimesniff;
  const { blobFromObjectUrl } = window.__bootstrap.file;
  const {
    headersFromHeaderList,
    headerListFromHeaders,
    fillHeaders,
    getDecodeSplitHeader,
  } = window.__bootstrap.headers;
  const { HttpClientPrototype } = window.__bootstrap.fetch;
  const abortSignal = window.__bootstrap.abortSignal;
  const {
    ArrayPrototypeMap,
    ArrayPrototypeSlice,
    ArrayPrototypeSplice,
    ObjectKeys,
    ObjectPrototypeIsPrototypeOf,
    RegExpPrototypeTest,
    SafeArrayIterator,
    Symbol,
    SymbolFor,
    TypeError,
  } = window.__bootstrap.primordials;

  const _request = Symbol("request");
  const _headers = Symbol("headers");
  const _getHeaders = Symbol("get headers");
  const _headersCache = Symbol("headers cache");
  const _signal = Symbol("signal");
  const _mimeType = Symbol("mime type");
  const _body = Symbol("body");

  /**
   * @typedef InnerRequest
   * @property {string} method
   * @property {() => string} url
   * @property {() => string} currentUrl
   * @property {() => [string, string][]} headerList
   * @property {null | typeof __window.bootstrap.fetchBody.InnerBody} body
   * @property {"follow" | "error" | "manual"} redirectMode
   * @property {number} redirectCount
   * @property {string[]} urlList
   * @property {number | null} clientRid NOTE: non standard extension for `Deno.HttpClient`.
   * @property {Blob | null} blobUrlEntry
   */

  /**
   * @param {string} method
   * @param {string} url
   * @param {() => [string, string][]} headerList
   * @param {typeof __window.bootstrap.fetchBody.InnerBody} body
   * @param {boolean} maybeBlob
   * @returns
   */
  function newInnerRequest(method, url, headerList, body, maybeBlob) {
    let blobUrlEntry = null;
    if (maybeBlob && url.startsWith("blob:")) {
      blobUrlEntry = blobFromObjectUrl(url);
    }
    return {
      method,
      headerListInner: null,
      get headerList() {
        if (this.headerListInner === null) {
          try {
            this.headerListInner = headerList();
          } catch {
            throw new TypeError("cannot read headers: request closed");
          }
        }
        return this.headerListInner;
      },
      set headerList(value) {
        this.headerListInner = value;
      },
      body,
      redirectMode: "follow",
      redirectCount: 0,
      urlList: [url],
      clientRid: null,
      blobUrlEntry,
      url() {
        return this.urlList[0];
      },
      currentUrl() {
        return this.urlList[this.urlList.length - 1];
      },
    };
  }

  /**
   * https://fetch.spec.whatwg.org/#concept-request-clone
   * @param {InnerRequest} request
   * @returns {InnerRequest}
   */
  function cloneInnerRequest(request) {
    const headerList = [
      ...new SafeArrayIterator(
        ArrayPrototypeMap(request.headerList, (x) => [x[0], x[1]]),
      ),
    ];
    let body = null;
    if (request.body !== null) {
      body = request.body.clone();
    }

    return {
      method: request.method,
      headerList,
      body,
      redirectMode: request.redirectMode,
      redirectCount: request.redirectCount,
      urlList: request.urlList,
      clientRid: request.clientRid,
      blobUrlEntry: request.blobUrlEntry,
      url() {
        return this.urlList[0];
      },
      currentUrl() {
        return this.urlList[this.urlList.length - 1];
      },
    };
  }

  /**
   * @param {string} m
   * @returns {boolean}
   */
  function isKnownMethod(m) {
    return (
      m === "DELETE" ||
      m === "GET" ||
      m === "HEAD" ||
      m === "OPTIONS" ||
      m === "POST" ||
      m === "PUT"
    );
  }
  /**
   * @param {string} m
   * @returns {string}
   */
  function validateAndNormalizeMethod(m) {
    // Fast path for well-known methods
    if (isKnownMethod(m)) {
      return m;
    }

    // Regular path
    if (!RegExpPrototypeTest(HTTP_TOKEN_CODE_POINT_RE, m)) {
      throw new TypeError("Method is not valid.");
    }
    const upperCase = byteUpperCase(m);
    if (
      upperCase === "CONNECT" || upperCase === "TRACE" || upperCase === "TRACK"
    ) {
      throw new TypeError("Method is forbidden.");
    }
    return upperCase;
  }

  class Request {
    /** @type {InnerRequest} */
    [_request];
    /** @type {Headers} */
    [_headersCache];
    [_getHeaders];

    /** @type {Headers} */
    get [_headers]() {
      if (this[_headersCache] === undefined) {
        this[_headersCache] = this[_getHeaders]();
      }
      return this[_headersCache];
    }

    set [_headers](value) {
      this[_headersCache] = value;
    }

    /** @type {AbortSignal} */
    [_signal];
    get [_mimeType]() {
      const values = getDecodeSplitHeader(
        headerListFromHeaders(this[_headers]),
        "Content-Type",
      );
      return extractMimeType(values);
    }
    get [_body]() {
      return this[_request].body;
    }

    /**
     * https://fetch.spec.whatwg.org/#dom-request
     * @param {RequestInfo} input
     * @param {RequestInit} init
     */
    constructor(input, init = {}) {
      const prefix = "Failed to construct 'Request'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      input = webidl.converters["RequestInfo_DOMString"](input, {
        prefix,
        context: "Argument 1",
      });
      init = webidl.converters["RequestInit"](init, {
        prefix,
        context: "Argument 2",
      });

      this[webidl.brand] = webidl.brand;

      /** @type {InnerRequest} */
      let request;
      const baseURL = getLocationHref();

      // 4.
      let signal = null;

      // 5.
      if (typeof input === "string") {
        const parsedURL = new URL(input, baseURL);
        request = newInnerRequest("GET", parsedURL.href, () => [], null, true);
      } else { // 6.
        if (!ObjectPrototypeIsPrototypeOf(RequestPrototype, input)) {
          throw new TypeError("Unreachable");
        }
        request = input[_request];
        signal = input[_signal];
      }

      // 12.
      // TODO(lucacasonato): create a copy of `request`

      // 22.
      if (init.redirect !== undefined) {
        request.redirectMode = init.redirect;
      }

      // 25.
      if (init.method !== undefined) {
        let method = init.method;
        method = validateAndNormalizeMethod(method);
        request.method = method;
      }

      // 26.
      if (init.signal !== undefined) {
        signal = init.signal;
      }

      // NOTE: non standard extension. This handles Deno.HttpClient parameter
      if (init.client !== undefined) {
        if (
          init.client !== null &&
          !ObjectPrototypeIsPrototypeOf(HttpClientPrototype, init.client)
        ) {
          throw webidl.makeException(
            TypeError,
            "`client` must be a Deno.HttpClient",
            { prefix, context: "Argument 2" },
          );
        }
        request.clientRid = init.client?.rid ?? null;
      }

      // 27.
      this[_request] = request;

      // 28.
      this[_signal] = abortSignal.newSignal();

      // 29.
      if (signal !== null) {
        abortSignal.follow(this[_signal], signal);
      }

      // 30.
      this[_headers] = headersFromHeaderList(request.headerList, "request");

      // 32.
      if (ObjectKeys(init).length > 0) {
        let headers = ArrayPrototypeSlice(
          headerListFromHeaders(this[_headers]),
          0,
          headerListFromHeaders(this[_headers]).length,
        );
        if (init.headers !== undefined) {
          headers = init.headers;
        }
        ArrayPrototypeSplice(
          headerListFromHeaders(this[_headers]),
          0,
          headerListFromHeaders(this[_headers]).length,
        );
        fillHeaders(this[_headers], headers);
      }

      // 33.
      let inputBody = null;
      if (ObjectPrototypeIsPrototypeOf(RequestPrototype, input)) {
        inputBody = input[_body];
      }

      // 34.
      if (
        (request.method === "GET" || request.method === "HEAD") &&
        ((init.body !== undefined && init.body !== null) ||
          inputBody !== null)
      ) {
        throw new TypeError("Request with GET/HEAD method cannot have body.");
      }

      // 35.
      let initBody = null;

      // 36.
      if (init.body !== undefined && init.body !== null) {
        const res = extractBody(init.body);
        initBody = res.body;
        if (res.contentType !== null && !this[_headers].has("content-type")) {
          this[_headers].append("Content-Type", res.contentType);
        }
      }

      // 37.
      const inputOrInitBody = initBody ?? inputBody;

      // 39.
      let finalBody = inputOrInitBody;

      // 40.
      if (initBody === null && inputBody !== null) {
        if (input[_body] && input[_body].unusable()) {
          throw new TypeError("Input request's body is unusable.");
        }
        finalBody = inputBody.createProxy();
      }

      // 41.
      request.body = finalBody;
    }

    get method() {
      webidl.assertBranded(this, RequestPrototype);
      return this[_request].method;
    }

    get url() {
      webidl.assertBranded(this, RequestPrototype);
      return this[_request].url();
    }

    get headers() {
      webidl.assertBranded(this, RequestPrototype);
      return this[_headers];
    }

    get redirect() {
      webidl.assertBranded(this, RequestPrototype);
      return this[_request].redirectMode;
    }

    get signal() {
      webidl.assertBranded(this, RequestPrototype);
      return this[_signal];
    }

    clone() {
      webidl.assertBranded(this, RequestPrototype);
      if (this[_body] && this[_body].unusable()) {
        throw new TypeError("Body is unusable.");
      }
      const newReq = cloneInnerRequest(this[_request]);
      const newSignal = abortSignal.newSignal();
      abortSignal.follow(newSignal, this[_signal]);
      return fromInnerRequest(
        newReq,
        newSignal,
        guardFromHeaders(this[_headers]),
      );
    }

    [SymbolFor("Deno.customInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(RequestPrototype, this),
        keys: [
          "bodyUsed",
          "headers",
          "method",
          "redirect",
          "url",
        ],
      }));
    }
  }

  webidl.configurePrototype(Request);
  const RequestPrototype = Request.prototype;
  mixinBody(RequestPrototype, _body, _mimeType);

  webidl.converters["Request"] = webidl.createInterfaceConverter(
    "Request",
    RequestPrototype,
  );
  webidl.converters["RequestInfo_DOMString"] = (V, opts) => {
    // Union for (Request or USVString)
    if (typeof V == "object") {
      if (ObjectPrototypeIsPrototypeOf(RequestPrototype, V)) {
        return webidl.converters["Request"](V, opts);
      }
    }
    // Passed to new URL(...) which implicitly converts DOMString -> USVString
    return webidl.converters["DOMString"](V, opts);
  };
  webidl.converters["RequestRedirect"] = webidl.createEnumConverter(
    "RequestRedirect",
    [
      "follow",
      "error",
      "manual",
    ],
  );
  webidl.converters["RequestInit"] = webidl.createDictionaryConverter(
    "RequestInit",
    [
      { key: "method", converter: webidl.converters["ByteString"] },
      { key: "headers", converter: webidl.converters["HeadersInit"] },
      {
        key: "body",
        converter: webidl.createNullableConverter(
          webidl.converters["BodyInit_DOMString"],
        ),
      },
      { key: "redirect", converter: webidl.converters["RequestRedirect"] },
      {
        key: "signal",
        converter: webidl.createNullableConverter(
          webidl.converters["AbortSignal"],
        ),
      },
      { key: "client", converter: webidl.converters.any },
    ],
  );

  /**
   * @param {Request} request
   * @returns {InnerRequest}
   */
  function toInnerRequest(request) {
    return request[_request];
  }

  /**
   * @param {InnerRequest} inner
   * @param {"request" | "immutable" | "request-no-cors" | "response" | "none"} guard
   * @returns {Request}
   */
  function fromInnerRequest(inner, signal, guard) {
    const request = webidl.createBranded(Request);
    request[_request] = inner;
    request[_signal] = signal;
    request[_getHeaders] = () => headersFromHeaderList(inner.headerList, guard);
    return request;
  }

  window.__bootstrap.fetch ??= {};
  window.__bootstrap.fetch.Request = Request;
  window.__bootstrap.fetch.toInnerRequest = toInnerRequest;
  window.__bootstrap.fetch.fromInnerRequest = fromInnerRequest;
  window.__bootstrap.fetch.newInnerRequest = newInnerRequest;
})(globalThis);
