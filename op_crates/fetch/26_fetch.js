// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./11_streams_types.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { byteLowerCase } = window.__bootstrap.infra;
  const { InnerBody, extractBody } = window.__bootstrap.fetchBody;
  const {
    toInnerRequest,
    fromInnerResponse,
    redirectStatus,
    nullBodyStatus,
    networkError,
  } = window.__bootstrap.fetch;

  const REQUEST_BODY_HEADER_NAMES = [
    "content-encoding",
    "content-language",
    "content-location",
    "content-type",
  ];

  /**
   * @param {{ method: string, url: string, headers: [string, string][], clientRid: number | null, hasBody: boolean }} args 
   * @param {Uint8Array | null} body 
   * @returns {{ requestRid: number, requestBodyRid: number | null }}
   */
  function opFetch(args, body) {
    return core.opSync("op_fetch", args, body);
  }

  /**
   * @param {number} rid 
   * @returns {Promise<{ status: number, statusText: string, headers: [string, string][], url: string, responseRid: number }>}
   */
  function opFetchSend(rid) {
    return core.opAsync("op_fetch_send", rid);
  }

  /**
   * @param {number} rid 
   * @param {Uint8Array} body 
   * @returns {Promise<void>}
   */
  function opFetchRequestWrite(rid, body) {
    return core.opAsync("op_fetch_request_write", rid, body);
  }

  /**
   * @param {number} rid 
   * @param {Uint8Array} body 
   * @returns {Promise<number>}
   */
  function opFetchResponseRead(rid, body) {
    return core.opAsync("op_fetch_response_read", rid, body);
  }

  /**
   * @param {number} responseBodyRid
   * @returns {ReadableStream<Uint8Array>}
   */
  function createResponseBodyStream(responseBodyRid) {
    return new ReadableStream({
      type: "bytes",
      async pull(controller) {
        try {
          // This is the largest possible size for a single packet on a TLS
          // stream.
          const chunk = new Uint8Array(16 * 1024 + 256);
          const read = await opFetchResponseRead(
            responseBodyRid,
            chunk,
          );
          if (read > 0) {
            // We read some data. Enqueue it onto the stream.
            controller.enqueue(chunk.subarray(0, read));
          } else {
            // We have reached the end of the body, so we close the stream.
            controller.close();
            core.close(responseBodyRid);
          }
        } catch (err) {
          // There was an error while reading a chunk of the body, so we
          // error.
          controller.error(err);
          controller.close();
          core.close(responseBodyRid);
        }
      },
      cancel() {
        core.close(responseBodyRid);
      },
    });
  }

  /**
   * @param {InnerRequest} req 
   * @param {boolean} recursive
   * @returns {Promise<InnerResponse>}
   */
  async function mainFetch(req, recursive) {
    // TODO(lucacasonato): fast-path statically known body (somehow skip stream)
    /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
    let reqBody = null;
    if (req.body !== null) {
      if (req.body.length === null) {
        reqBody = req.body.stream;
      } else {
        const reader = req.body.stream.getReader();
        const r1 = await reader.read();
        if (r1.done) throw new TypeError("Unreachable");
        reqBody = r1.value;
        const r2 = await reader.read();
        if (!r2.done) throw new TypeError("Unreachable");
      }
    }

    const { requestRid, requestBodyRid } = opFetch({
      method: req.method,
      url: req.currentUrl.toString(),
      headers: req.headerList,
      clientRid: req.clientRid,
      hasBody: reqBody !== null,
    }, reqBody instanceof Uint8Array ? reqBody : null);

    if (requestBodyRid !== null) {
      if (reqBody === null || !(reqBody instanceof ReadableStream)) {
        throw new TypeError("Unreachable");
      }
      const reader = reqBody.getReader();
      (async () => {
        while (true) {
          const { value, done } = await reader.read();
          if (done) break;
          if (!(value instanceof Uint8Array)) {
            await reader.cancel("value not a Uint8Array");
            break;
          }
          try {
            await opFetchRequestWrite(requestBodyRid, value);
          } catch (err) {
            await reader.cancel(err);
            break;
          }
        }
        core.close(requestBodyRid);
      })();
    }

    const resp = await opFetchSend(requestRid);
    /** @type {InnerResponse} */
    const response = {
      headerList: resp.headers,
      status: resp.status,
      body: null,
      statusMessage: resp.statusText,
      type: "basic",
      get url() {
        if (this.urlList.length == 0) return null;
        return this.urlList[this.urlList.length - 1];
      },
      urlList: req.urlList,
    };
    if (redirectStatus(resp.status)) {
      switch (req.redirectMode) {
        case "error":
          core.close(resp.responseRid);
          return networkError(
            "Encountered redirect while redirect mode is set to 'error'",
          );
        case "follow":
          core.close(resp.responseRid);
          return httpRedirectFetch(req, response);
        case "manual":
          break;
      }
    }

    if (nullBodyStatus(response.status)) {
      core.close(resp.responseRid);
    } else {
      response.body = new InnerBody(createResponseBodyStream(resp.responseRid));
    }

    if (recursive) return response;

    if (response.urlList.length === 0) {
      response.urlList = [...req.urlList];
    }

    return response;
  }

  /**
   * @param {InnerRequest} request
   * @param {InnerResponse} response
   * @returns {Promise<InnerResponse>}
   */
  function httpRedirectFetch(request, response) {
    const locationHeaders = response.headerList.filter((entry) =>
      byteLowerCase(entry[0]) === "location"
    );
    if (locationHeaders.length === 0) {
      return response;
    }
    const locationURL = new URL(
      locationHeaders[0][1],
      response.url ?? undefined,
    );
    if (locationURL.hash === "") {
      locationURL.hash = request.url.hash;
    }
    if (locationURL.protocol !== "https:" && locationURL.protocol !== "http:") {
      return networkError("Can not redirect to a non HTTP(s) url");
    }
    if (request.redirectCount === 20) {
      return networkError("Maximum number of redirects (20) reached");
    }
    request.redirectCount++;
    if (
      response.status !== 303 && request.body !== null &&
      request.body.source === null
    ) {
      return networkError(
        "Can not redeliver a streaming request body after a redirect",
      );
    }
    if (
      ((response.status === 301 || response.status === 302) &&
        request.method === "POST") ||
      (response.status === 303 &&
        (request.method !== "GET" && request.method !== "HEAD"))
    ) {
      request.method = "GET";
      request.body = null;
      for (let i = 0; i < request.headerList.length; i++) {
        if (
          REQUEST_BODY_HEADER_NAMES.includes(
            byteLowerCase(request.headerList[i][0]),
          )
        ) {
          request.headerList.splice(i, 1);
          i--;
        }
      }
    }
    if (request.body !== null) {
      const res = extractBody(request.body.source);
      request.body = res.body;
    }
    request.urlList.push(locationURL);
    return mainFetch(request, true);
  }

  /**
   * @param {RequestInfo} input 
   * @param {RequestInit} init 
   */
  async function fetch(input, init = {}) {
    const prefix = "Failed to call 'fetch'";
    input = webidl.converters["RequestInfo"](input, {
      prefix,
      context: "Argument 1",
    });
    init = webidl.converters["RequestInit"](init, {
      prefix,
      context: "Argument 2",
    });

    // 1.
    const requestObject = new Request(input, init);
    // 2.
    const request = toInnerRequest(requestObject);
    // 10.
    if (!requestObject.headers.has("Accept")) {
      request.headerList.push(["Accept", "*/*"]);
    }

    // 12.
    const response = await mainFetch(request, false);
    if (response.type === "error") {
      throw new TypeError(
        "Fetch failed: " + (response.error ?? "unknown error"),
      );
    }

    return fromInnerResponse(response, "immutable");
  }

  window.__bootstrap.fetch ??= {};
  window.__bootstrap.fetch.fetch = fetch;
})(this);
