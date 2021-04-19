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
  const { HTTP_TOKEN_CODE_POINT_RE, byteUpperCase } = window.__bootstrap.infra;
  const { URL } = window.__bootstrap.url;
  const { guardFromHeaders } = window.__bootstrap.headers;
  const { InnerBody, mixinBody, extractBody } = window.__bootstrap.fetchBody;
  const { getLocationHref } = window.__bootstrap.location;
  const mimesniff = window.__bootstrap.mimesniff;
  const {
    headersFromHeaderList,
    headerListFromHeaders,
    fillHeaders,
    getDecodeSplitHeader,
  } = window.__bootstrap.headers;
  const { HttpClient } = window.__bootstrap.fetch;

  const _request = Symbol("request");
  const _headers = Symbol("headers");
  const _mimeType = Symbol("mime type");
  const _body = Symbol("body");

  /**
   * @typedef InnerRequest
   * @property {string} method
   * @property {URL} url
   * @property {URL} currentUrl
   * @property {[string, string][]} headerList
   * @property {null | InnerBody} body
   * @property {"follow" | "error" | "manual"} redirectMode
   * @property {number} redirectCount
   * @property {URL[]} urlList
   * @property {number | null} clientRid NOTE: non standard extension for `Deno.HttpClient`.
   */

  /**
   * 
   * @param {string} method 
   * @param {URL} url 
   * @param {[string, string][]} headerList 
   * @param {InnerBody} body 
   * @returns 
   */
  function newInnerRequest(method, url, headerList = [], body = null) {
    return {
      method: method,
      get url() {
        return this.urlList[0];
      },
      get currentUrl() {
        return this.urlList[this.urlList.length - 1];
      },
      headerList,
      body,
      redirectMode: "follow",
      redirectCount: 0,
      urlList: [url],
      clientRid: null,
    };
  }

  /**
   * https://fetch.spec.whatwg.org/#concept-request-clone
   * @param {InnerRequest} request 
   * @returns {InnerRequest}
   */
  function cloneInnerRequest(request) {
    const headerList = [...request.headerList.map((x) => [x[0], x[1]])];
    let body = null;
    if (request.body !== null) {
      body = request.body.clone();
    }

    return {
      method: request.method,
      get url() {
        return this.urlList[0];
      },
      get currentUrl() {
        return this.urlList[this.urlList.length - 1];
      },
      headerList,
      body,
      redirectMode: request.redirectMode,
      redirectCount: request.redirectCount,
      urlList: request.urlList,
      clientRid: request.clientRid,
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
    if (!HTTP_TOKEN_CODE_POINT_RE.test(m)) {
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
    [_headers];
    get [_mimeType]() {
      let charset = null;
      let essence = null;
      let mimeType = null;
      const values = getDecodeSplitHeader(
        headerListFromHeaders(this[_headers]),
        "Content-Type",
      );
      if (values === null) return null;
      for (const value of values) {
        const temporaryMimeType = mimesniff.parseMimeType(value);
        if (
          temporaryMimeType === null ||
          mimesniff.essence(temporaryMimeType) == "*/*"
        ) {
          continue;
        }
        mimeType = temporaryMimeType;
        if (mimesniff.essence(mimeType) !== essence) {
          charset = null;
          const newCharset = mimeType.parameters.get("charset");
          if (newCharset !== undefined) {
            charset = newCharset;
          }
          essence = mimesniff.essence(mimeType);
        } else {
          if (mimeType.parameters.has("charset") === null && charset !== null) {
            mimeType.parameters.set("charset", charset);
          }
        }
      }
      if (mimeType === null) return null;
      return mimeType;
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
      input = webidl.converters["RequestInfo"](input, {
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

      // 5.
      if (typeof input === "string") {
        const parsedURL = new URL(input, baseURL);
        request = newInnerRequest("GET", parsedURL, [], null);
      } else { // 6.
        if (!(input instanceof Request)) throw new TypeError("Unreachable");
        request = input[_request];
      }

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

      // NOTE: non standard extension. This handles Deno.HttpClient parameter
      if (init.client !== undefined) {
        if (init.client !== null && !(init.client instanceof HttpClient)) {
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

      // 29.
      this[_headers] = headersFromHeaderList(request.headerList, "request");

      // 31.
      if (Object.keys(init).length > 0) {
        let headers = headerListFromHeaders(this[_headers]);
        if (init.headers !== undefined) {
          headers = init.headers;
        }
        headerListFromHeaders(this[_headers]).slice(
          0,
          headerListFromHeaders(this[_headers]).length,
        );
        fillHeaders(this[_headers], headers);
      }

      // 32.
      let inputBody = null;
      if (input instanceof Request) {
        inputBody = input[_body];
      }

      // 33.
      if (
        (request.method === "GET" || request.method === "HEAD") &&
        ((init["body"] !== undefined && init["body"] !== null) ||
          inputBody !== null)
      ) {
        throw new TypeError("HEAD and GET requests may not have a body.");
      }

      // 34.
      let initBody = null;

      // 35.
      if (init["body"] !== undefined && init["body"] !== null) {
        const res = extractBody(init["body"]);
        initBody = res.body;
        if (res.contentType !== null && !this[_headers].has("content-type")) {
          this[_headers].append("Content-Type", res.contentType);
        }
      }

      // 36.
      const inputOrInitBody = initBody ?? inputBody;

      // 38.
      const finalBody = inputOrInitBody;

      // 39.
      // TODO(lucacasonato): implement this step. Is it needed?

      // 40.
      this[_request].body = finalBody;
    }

    get method() {
      webidl.assertBranded(this, Request);
      return this[_request].method;
    }

    get url() {
      webidl.assertBranded(this, Request);
      return this[_request].url.toString();
    }

    get headers() {
      webidl.assertBranded(this, Request);
      return this[_headers];
    }

    get destination() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get referrer() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get referrerPolicy() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get mode() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get credentials() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get cache() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get redirect() {
      webidl.assertBranded(this, Request);
      return this[_request].redirectMode;
    }

    get integrity() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get keepalive() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get isReloadNavigation() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get isHistoryNavigation() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    get signal() {
      webidl.assertBranded(this, Request);
      throw new TypeError("This property is not implemented.");
    }

    clone() {
      webidl.assertBranded(this, Request);
      if (this[_body] && this[_body].unusable()) {
        throw new TypeError("Body is unusable.");
      }
      const newReq = cloneInnerRequest(this[_request]);
      return fromInnerRequest(newReq, guardFromHeaders(this[_headers]));
    }

    get [Symbol.toStringTag]() {
      return "Request";
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      const inner = {
        bodyUsed: this.bodyUsed,
        headers: this.headers,
        method: this.method,
        redirect: this.redirect,
        url: this.url,
      };
      return `Request ${inspect(inner)}`;
    }
  }

  mixinBody(Request, _body, _mimeType);

  webidl.converters["Request"] = webidl.createInterfaceConverter(
    "Request",
    Request,
  );
  webidl.converters["RequestInfo"] = (V, opts) => {
    // Union for (Request or USVString)
    if (typeof V == "object") {
      if (V instanceof Request) {
        return webidl.converters["Request"](V, opts);
      }
    }
    return webidl.converters["USVString"](V, opts);
  };

  webidl.converters["ReferrerPolicy"] = webidl.createEnumConverter(
    "ReferrerPolicy",
    [
      "",
      "no-referrer",
      "no-referrer-when-downgrade",
      "same-origin",
      "origin",
      "strict-origin",
      "origin-when-cross-origin",
      "strict-origin-when-cross-origin",
      "unsafe-url",
    ],
  );
  webidl.converters["RequestMode"] = webidl.createEnumConverter("RequestMode", [
    "navigate",
    "same-origin",
    "no-cors",
    "cors",
  ]);
  webidl.converters["RequestCredentials"] = webidl.createEnumConverter(
    "RequestCredentials",
    [
      "omit",
      "same-origin",
      "include",
    ],
  );
  webidl.converters["RequestCache"] = webidl.createEnumConverter(
    "RequestCache",
    [
      "default",
      "no-store",
      "reload",
      "no-cache",
      "force-cache",
      "only-if-cached",
    ],
  );
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
          webidl.converters["BodyInit"],
        ),
      },
      { key: "referrer", converter: webidl.converters["USVString"] },
      { key: "referrerPolicy", converter: webidl.converters["ReferrerPolicy"] },
      { key: "mode", converter: webidl.converters["RequestMode"] },
      {
        key: "credentials",
        converter: webidl.converters["RequestCredentials"],
      },
      { key: "cache", converter: webidl.converters["RequestCache"] },
      { key: "redirect", converter: webidl.converters["RequestRedirect"] },
      { key: "integrity", converter: webidl.converters["DOMString"] },
      { key: "keepalive", converter: webidl.converters["boolean"] },
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
  function fromInnerRequest(inner, guard) {
    const request = webidl.createBranded(Request);
    request[_request] = inner;
    request[_headers] = headersFromHeaderList(inner.headerList, guard);
    return request;
  }

  window.__bootstrap.fetch ??= {};
  window.__bootstrap.fetch.Request = Request;
  window.__bootstrap.fetch.toInnerRequest = toInnerRequest;
  window.__bootstrap.fetch.fromInnerRequest = fromInnerRequest;
  window.__bootstrap.fetch.newInnerRequest = newInnerRequest;
})(globalThis);
