// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../file/internal.d.ts" />
/// <reference path="../file/lib.deno_file.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./11_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { HTTP_TAB_OR_SPACE, regexMatcher } = window.__bootstrap.infra;
  const { InnerBody, extractBody, mixinBody } = window.__bootstrap.fetchBody;
  const { getLocationHref } = window.__bootstrap.location;
  const mimesniff = window.__bootstrap.mimesniff;
  const { URL } = window.__bootstrap.url;
  const {
    getDecodeSplitHeader,
    headerListFromHeaders,
    headersFromHeaderList,
    guardFromHeaders,
    fillHeaders,
  } = window.__bootstrap.headers;

  const VCHAR = ["\x21-\x7E"];
  const OBS_TEXT = ["\x80-\xFF"];

  const REASON_PHRASE = [...HTTP_TAB_OR_SPACE, ...VCHAR, ...OBS_TEXT];
  const REASON_PHRASE_MATCHER = regexMatcher(REASON_PHRASE);
  const REASON_PHRASE_RE = new RegExp(`^[${REASON_PHRASE_MATCHER}]*$`);

  const _response = Symbol("response");
  const _headers = Symbol("headers");
  const _mimeType = Symbol("mime type");
  const _body = Symbol("body");

  /**
   * @typedef InnerResponse
   * @property {"basic" | "cors" | "default" | "error" | "opaque" | "opaqueredirect"} type
   * @property {URL | null} url
   * @property {URL[]} urlList
   * @property {number} status
   * @property {string} statusMessage
   * @property {[string, string][]} headerList
   * @property {null | InnerBody} body
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
    const urlList = response.urlList.map((url) => new URL(url.toString()));
    const headerList = [...response.headerList.map((x) => [x[0], x[1]])];
    let body = null;
    if (response.body !== null) {
      body = response.body.clone();
    }

    return {
      type: response.type,
      body,
      headerList,
      get url() {
        if (this.urlList.length == 0) return null;
        return this.urlList[this.urlList.length - 1];
      },
      urlList,
      status: response.status,
      statusMessage: response.statusMessage,
    };
  }

  /**
   * @returns {InnerResponse}
   */
  function newInnerResponse() {
    return {
      type: "default",
      body: null,
      headerList: [],
      get url() {
        if (this.urlList.length == 0) return null;
        return this.urlList[this.urlList.length - 1];
      },
      urlList: [],
      status: 200,
      statusMessage: "",
    };
  }

  /**
   * @param {string} error
   * @returns {InnerResponse}
   */
  function networkError(error) {
    const resp = newInnerResponse();
    resp.status = 0;
    resp.type = "error";
    resp.error = error;
    return resp;
  }

  class Response {
    /** @type {InnerResponse} */
    [_response];
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
      return this[_response].body;
    }

    /**
     * @returns {Response}
     */
    static error() {
      const inner = newInnerResponse();
      inner.type = "error";
      inner.status = 0;
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
      const inner = newInnerResponse();
      inner.type = "default";
      inner.status = status;
      inner.headerList.push(["Location", parsedURL.toString()]);
      const response = webidl.createBranded(Response);
      response[_response] = inner;
      response[_headers] = headersFromHeaderList(
        response[_response].headerList,
        "immutable",
      );
      return response;
    }

    /**
     * @param {BodyInit | null} body 
     * @param {ResponseInit} init 
     */
    constructor(body = null, init = {}) {
      const prefix = "Failed to construct 'Response'";
      body = webidl.converters["BodyInit?"](body, {
        prefix,
        context: "Argument 1",
      });
      init = webidl.converters["ResponseInit"](init, {
        prefix,
        context: "Argument 2",
      });

      if (init["status"] < 200 || init["status"] > 599) {
        throw new RangeError(
          `The status provided (${
            init["status"]
          }) is outside the range [200, 599].`,
        );
      }

      if (!REASON_PHRASE_RE.test(init["statusText"])) {
        throw new TypeError("Status text is not valid.");
      }

      this[webidl.brand] = webidl.brand;
      this[_response] = newInnerResponse();
      this[_headers] = headersFromHeaderList(
        this[_response].headerList,
        "response",
      );
      this[_response].status = init["status"];
      this[_response].statusMessage = init["statusText"];
      if (init["headers"] !== undefined) {
        fillHeaders(this[_headers], init["headers"]);
      }
      if (body !== null) {
        if (nullBodyStatus(this[_response].status)) {
          throw new TypeError(
            "Response with null body status cannot have body",
          );
        }
        const res = extractBody(body);
        this[_response].body = res.body;
        if (res.contentType !== null && !this[_headers].has("content-type")) {
          this[_headers].append("Content-Type", res.contentType);
        }
      }
    }

    /**
     * @returns {"basic" | "cors" | "default" | "error" | "opaque" | "opaqueredirect"}
     */
    get type() {
      webidl.assertBranded(this, Response);
      return this[_response].type;
    }

    /**
     * @returns {string}
     */
    get url() {
      webidl.assertBranded(this, Response);
      const url = this[_response].url;
      if (url === null) return "";
      const newUrl = new URL(url);
      newUrl.hash = "";
      return newUrl.toString();
    }

    /**
     * @returns {boolean}
     */
    get redirected() {
      webidl.assertBranded(this, Response);
      return this[_response].urlList.length > 1;
    }

    /**
     * @returns {number}
     */
    get status() {
      webidl.assertBranded(this, Response);
      return this[_response].status;
    }

    /**
     * @returns {boolean}
     */
    get ok() {
      webidl.assertBranded(this, Response);
      const status = this[_response].status;
      return status >= 200 && status <= 299;
    }

    /**
     * @returns {string}
     */
    get statusText() {
      webidl.assertBranded(this, Response);
      return this[_response].statusMessage;
    }

    /**
     * @returns {Headers}
     */
    get headers() {
      webidl.assertBranded(this, Response);
      return this[_headers];
    }

    /**
     * @returns {Response}
     */
    clone() {
      webidl.assertBranded(this, Response);
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

    get [Symbol.toStringTag]() {
      return "Response";
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      const inner = {
        body: this.body,
        bodyUsed: this.bodyUsed,
        headers: this.headers,
        ok: this.ok,
        redirected: this.redirected,
        status: this.status,
        statusText: this.statusText,
        url: this.url,
      };
      return `Response ${inspect(inner)}`;
    }
  }

  mixinBody(Response, _body, _mimeType);

  webidl.converters["Response"] = webidl.createInterfaceConverter(
    "Response",
    Response,
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
  window.__bootstrap.fetch.toInnerResponse = toInnerResponse;
  window.__bootstrap.fetch.fromInnerResponse = fromInnerResponse;
  window.__bootstrap.fetch.redirectStatus = redirectStatus;
  window.__bootstrap.fetch.nullBodyStatus = nullBodyStatus;
  window.__bootstrap.fetch.networkError = networkError;
})(globalThis);
