// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const { isProxy } = Deno.core;
  const webidl = window.__bootstrap.webidl;
  const consoleInternal = window.__bootstrap.console;
  const { HTTP_TAB_OR_SPACE, regexMatcher, serializeJSValueToJSONString } =
    window.__bootstrap.infra;
  const { extractBody, mixinBody } = window.__bootstrap.fetchBody;
  const { getLocationHref } = window.__bootstrap.location;
  const { extractMimeType } = window.__bootstrap.mimesniff;
  const { URL } = window.__bootstrap.url;
  const {
    getDecodeSplitHeader,
    headerListFromHeaders,
    headersFromHeaderList,
    guardFromHeaders,
    fillHeaders,
  } = window.__bootstrap.headers;
  const {
    ArrayPrototypeMap,
    ArrayPrototypePush,
    ObjectPrototypeIsPrototypeOf,
    RangeError,
    RegExp,
    RegExpPrototypeTest,
    SafeArrayIterator,
    Symbol,
    SymbolFor,
    TypeError,
  } = window.__bootstrap.primordials;

  const VCHAR = ["\x21-\x7E"];
  const OBS_TEXT = ["\x80-\xFF"];

  const REASON_PHRASE = [
    ...new SafeArrayIterator(HTTP_TAB_OR_SPACE),
    ...new SafeArrayIterator(VCHAR),
    ...new SafeArrayIterator(OBS_TEXT),
  ];
  const REASON_PHRASE_MATCHER = regexMatcher(REASON_PHRASE);
  const REASON_PHRASE_RE = new RegExp(`^[${REASON_PHRASE_MATCHER}]*$`);

  const _response = Symbol("response");
  const _headers = Symbol("headers");
  const _mimeType = Symbol("mime type");
  const _body = Symbol("body");

  /**
   * @typedef InnerResponse
   * @property {"basic" | "cors" | "default" | "error" | "opaque" | "opaqueredirect"} type
   * @property {() => string | null} url
   * @property {string[]} urlList
   * @property {number} status
   * @property {string} statusMessage
   * @property {[string, string][]} headerList
   * @property {null | typeof __window.bootstrap.fetchBody.InnerBody} body
   * @property {boolean} aborted
   * @property {string} [error]
   */

  /**
   * @param {number} status
   * @returns {boolean}
   */
  function nullBodyStatus(status) {
    return status === 101 || status === 204 || status === 205 || status === 304;
  }

  /**
   * @param {number} status
   * @returns {boolean}
   */
  function redirectStatus(status) {
    return status === 301 || status === 302 || status === 303 ||
      status === 307 || status === 308;
  }

  /**
   * https://fetch.spec.whatwg.org/#concept-response-clone
   * @param {InnerResponse} response
   * @returns {InnerResponse}
   */
  function cloneInnerResponse(response) {
    const urlList = [...new SafeArrayIterator(response.urlList)];
    const headerList = [
      ...new SafeArrayIterator(
        ArrayPrototypeMap(response.headerList, (x) => [x[0], x[1]]),
      ),
    ];
    let body = null;
    if (response.body !== null) {
      body = response.body.clone();
    }

    return {
      type: response.type,
      body,
      headerList,
      urlList,
      status: response.status,
      statusMessage: response.statusMessage,
      aborted: response.aborted,
      url() {
        if (this.urlList.length == 0) return null;
        return this.urlList[this.urlList.length - 1];
      },
    };
  }

  /**
   * @returns {InnerResponse}
   */
  function newInnerResponse(status = 200, statusMessage = "") {
    return {
      type: "default",
      body: null,
      headerList: [],
      urlList: [],
      status,
      statusMessage,
      aborted: false,
      url() {
        if (this.urlList.length == 0) return null;
        return this.urlList[this.urlList.length - 1];
      },
    };
  }

  /**
   * @param {string} error
   * @returns {InnerResponse}
   */
  function networkError(error) {
    const resp = newInnerResponse(0);
    resp.type = "error";
    resp.error = error;
    return resp;
  }

  /**
   * @returns {InnerResponse}
   */
  function abortedNetworkError() {
    const resp = networkError("aborted");
    resp.aborted = true;
    return resp;
  }

  /**
   * https://fetch.spec.whatwg.org#initialize-a-response
   * @param {Response} response
   * @param {ResponseInit} init
   * @param {{ body: __bootstrap.fetchBody.InnerBody, contentType: string | null } | null} bodyWithType
   */
  function initializeAResponse(response, init, bodyWithType) {
    // 1.
    if ((init.status < 200 || init.status > 599) && init.status != 101) {
      throw new RangeError(
        `The status provided (${init.status}) is not equal to 101 and outside the range [200, 599].`,
      );
    }

    // 2.
    if (
      init.statusText &&
      !RegExpPrototypeTest(REASON_PHRASE_RE, init.statusText)
    ) {
      throw new TypeError("Status text is not valid.");
    }

    // 3.
    response[_response].status = init.status;

    // 4.
    response[_response].statusMessage = init.statusText;

    // 5.
    /** @type {__bootstrap.headers.Headers} */
    const headers = response[_headers];
    if (init.headers) {
      fillHeaders(headers, init.headers);
    }

    // 6.
    if (bodyWithType !== null) {
      if (nullBodyStatus(response[_response].status)) {
        throw new TypeError(
          "Response with null body status cannot have body",
        );
      }
      const { body, contentType } = bodyWithType;
      response[_response].body = body;
      if (contentType !== null && !headers.has("content-type")) {
        headers.append("Content-Type", contentType);
      }
    }
  }

  class Response {
    get [_mimeType]() {
      const values = getDecodeSplitHeader(
        headerListFromHeaders(this[_headers]),
        "Content-Type",
      );
      return extractMimeType(values);
    }
    get [_body]() {
      return this[_response].body;
    }

    /**
     * @returns {Response}
     */
    static error() {
      const inner = newInnerResponse(0);
      inner.type = "error";
      const response = webidl.createBranded(Response);
      response[_response] = inner;
      response[_headers] = headersFromHeaderList(
        response[_response].headerList,
        "immutable",
      );
      return response;
    }

    /**
     * @param {string} url
     * @param {number} status
     * @returns {Response}
     */
    static redirect(url, status = 302) {
      const prefix = "Failed to call 'Response.redirect'";
      url = webidl.converters["USVString"](url, {
        prefix,
        context: "Argument 1",
      });
      status = webidl.converters["unsigned short"](status, {
        prefix,
        context: "Argument 2",
      });

      const baseURL = getLocationHref();
      const parsedURL = new URL(url, baseURL);
      if (!redirectStatus(status)) {
        throw new RangeError("Invalid redirect status code.");
      }
      const inner = newInnerResponse(status);
      inner.type = "default";
      ArrayPrototypePush(inner.headerList, ["Location", parsedURL.href]);
      const response = webidl.createBranded(Response);
      response[_response] = inner;
      response[_headers] = headersFromHeaderList(
        response[_response].headerList,
        "immutable",
      );
      return response;
    }

    /**
     * @param {any} data
     * @param {ResponseInit} init
     * @returns {Response}
     */
    static json(data, init = {}) {
      const prefix = "Failed to call 'Response.json'";
      data = webidl.converters.any(data);
      init = webidl.converters["ResponseInit_fast"](init, {
        prefix,
        context: "Argument 2",
      });

      const str = serializeJSValueToJSONString(data);
      const res = extractBody(str);
      res.contentType = "application/json";
      const response = webidl.createBranded(Response);
      response[_response] = newInnerResponse();
      response[_headers] = headersFromHeaderList(
        response[_response].headerList,
        "response",
      );
      initializeAResponse(response, init, res);
      return response;
    }

    /**
     * @param {BodyInit | null} body
     * @param {ResponseInit} init
     */
    constructor(body = null, init = undefined) {
      const prefix = "Failed to construct 'Response'";
      body = webidl.converters["BodyInit_DOMString?"](body, {
        prefix,
        context: "Argument 1",
      });
      init = webidl.converters["ResponseInit_fast"](init, {
        prefix,
        context: "Argument 2",
      });

      this[_response] = newInnerResponse();
      this[_headers] = headersFromHeaderList(
        this[_response].headerList,
        "response",
      );

      let bodyWithType = null;
      if (body !== null) {
        bodyWithType = extractBody(body);
      }
      initializeAResponse(this, init, bodyWithType);
      this[webidl.brand] = webidl.brand;
    }

    /**
     * @returns {"basic" | "cors" | "default" | "error" | "opaque" | "opaqueredirect"}
     */
    get type() {
      webidl.assertBranded(this, ResponsePrototype);
      return this[_response].type;
    }

    /**
     * @returns {string}
     */
    get url() {
      webidl.assertBranded(this, ResponsePrototype);
      const url = this[_response].url();
      if (url === null) return "";
      const newUrl = new URL(url);
      newUrl.hash = "";
      return newUrl.href;
    }

    /**
     * @returns {boolean}
     */
    get redirected() {
      webidl.assertBranded(this, ResponsePrototype);
      return this[_response].urlList.length > 1;
    }

    /**
     * @returns {number}
     */
    get status() {
      webidl.assertBranded(this, ResponsePrototype);
      return this[_response].status;
    }

    /**
     * @returns {boolean}
     */
    get ok() {
      webidl.assertBranded(this, ResponsePrototype);
      const status = this[_response].status;
      return status >= 200 && status <= 299;
    }

    /**
     * @returns {string}
     */
    get statusText() {
      webidl.assertBranded(this, ResponsePrototype);
      return this[_response].statusMessage;
    }

    /**
     * @returns {Headers}
     */
    get headers() {
      webidl.assertBranded(this, ResponsePrototype);
      return this[_headers];
    }

    /**
     * @returns {Response}
     */
    clone() {
      webidl.assertBranded(this, ResponsePrototype);
      if (this[_body] && this[_body].unusable()) {
        throw new TypeError("Body is unusable.");
      }
      const second = webidl.createBranded(Response);
      const newRes = cloneInnerResponse(this[_response]);
      second[_response] = newRes;
      second[_headers] = headersFromHeaderList(
        newRes.headerList,
        guardFromHeaders(this[_headers]),
      );
      return second;
    }

    [SymbolFor("Deno.customInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(ResponsePrototype, this),
        keys: [
          "body",
          "bodyUsed",
          "headers",
          "ok",
          "redirected",
          "status",
          "statusText",
          "url",
        ],
      }));
    }
  }

  webidl.configurePrototype(Response);
  const ResponsePrototype = Response.prototype;
  mixinBody(ResponsePrototype, _body, _mimeType);

  webidl.converters["Response"] = webidl.createInterfaceConverter(
    "Response",
    ResponsePrototype,
  );
  webidl.converters["ResponseInit"] = webidl.createDictionaryConverter(
    "ResponseInit",
    [{
      key: "status",
      defaultValue: 200,
      converter: webidl.converters["unsigned short"],
    }, {
      key: "statusText",
      defaultValue: "",
      converter: webidl.converters["ByteString"],
    }, {
      key: "headers",
      converter: webidl.converters["HeadersInit"],
    }],
  );
  webidl.converters["ResponseInit_fast"] = function (init, opts) {
    if (init === undefined || init === null) {
      return { status: 200, statusText: "", headers: undefined };
    }
    // Fast path, if not a proxy
    if (typeof init === "object" && !isProxy(init)) {
      // Not a proxy fast path
      const status = init.status !== undefined
        ? webidl.converters["unsigned short"](init.status)
        : 200;
      const statusText = init.statusText !== undefined
        ? webidl.converters["ByteString"](init.statusText)
        : "";
      const headers = init.headers !== undefined
        ? webidl.converters["HeadersInit"](init.headers)
        : undefined;
      return { status, statusText, headers };
    }
    // Slow default path
    return webidl.converters["ResponseInit"](init, opts);
  };

  /**
   * @param {Response} response
   * @returns {InnerResponse}
   */
  function toInnerResponse(response) {
    return response[_response];
  }

  /**
   * @param {InnerResponse} inner
   * @param {"request" | "immutable" | "request-no-cors" | "response" | "none"} guard
   * @returns {Response}
   */
  function fromInnerResponse(inner, guard) {
    const response = webidl.createBranded(Response);
    response[_response] = inner;
    response[_headers] = headersFromHeaderList(inner.headerList, guard);
    return response;
  }

  window.__bootstrap.fetch ??= {};
  window.__bootstrap.fetch.Response = Response;
  window.__bootstrap.fetch.ResponsePrototype = ResponsePrototype;
  window.__bootstrap.fetch.newInnerResponse = newInnerResponse;
  window.__bootstrap.fetch.toInnerResponse = toInnerResponse;
  window.__bootstrap.fetch.fromInnerResponse = fromInnerResponse;
  window.__bootstrap.fetch.redirectStatus = redirectStatus;
  window.__bootstrap.fetch.nullBodyStatus = nullBodyStatus;
  window.__bootstrap.fetch.networkError = networkError;
  window.__bootstrap.fetch.abortedNetworkError = abortedNetworkError;
})(globalThis);
