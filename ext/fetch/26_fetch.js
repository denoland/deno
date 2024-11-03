// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />

import { core, primordials } from "ext:core/mod.js";
import {
  op_fetch,
  op_fetch_send,
  op_wasm_streaming_feed,
  op_wasm_streaming_set_url,
} from "ext:core/ops";
const {
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  Error,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromisePrototypeCatch,
  SafeArrayIterator,
  String,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  TypeError,
  TypedArrayPrototypeGetSymbolToStringTag,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { byteLowerCase } from "ext:deno_web/00_infra.js";
import {
  errorReadableStream,
  getReadableStreamResourceBacking,
  readableStreamForRid,
  ReadableStreamPrototype,
  resourceForReadableStream,
} from "ext:deno_web/06_streams.js";
import { extractBody, InnerBody } from "ext:deno_fetch/22_body.js";
import { processUrlList, toInnerRequest } from "ext:deno_fetch/23_request.js";
import {
  abortedNetworkError,
  fromInnerResponse,
  networkError,
  nullBodyStatus,
  redirectStatus,
  toInnerResponse,
} from "ext:deno_fetch/23_response.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";

const REQUEST_BODY_HEADER_NAMES = [
  "content-encoding",
  "content-language",
  "content-location",
  "content-type",
];

/**
 * @param {number} rid
 * @returns {Promise<{ status: number, statusText: string, headers: [string, string][], url: string, responseRid: number, error: [string, string]? }>}
 */
function opFetchSend(rid) {
  return op_fetch_send(rid);
}

/**
 * @param {number} responseBodyRid
 * @param {AbortSignal} [terminator]
 * @returns {ReadableStream<Uint8Array>}
 */
function createResponseBodyStream(responseBodyRid, terminator) {
  const readable = readableStreamForRid(responseBodyRid);

  function onAbort() {
    errorReadableStream(readable, terminator.reason);
    core.tryClose(responseBodyRid);
  }

  // TODO(lucacasonato): clean up registration
  terminator[abortSignal.add](onAbort);

  return readable;
}

/**
 * @param {InnerRequest} req
 * @param {boolean} recursive
 * @param {AbortSignal} terminator
 * @returns {Promise<InnerResponse>}
 */
async function mainFetch(req, recursive, terminator) {
  if (req.blobUrlEntry !== null) {
    if (req.method !== "GET") {
      throw new TypeError("Blob URL fetch only supports GET method");
    }

    const body = new InnerBody(req.blobUrlEntry.stream());
    terminator[abortSignal.add](() => body.error(terminator.reason));
    processUrlList(req.urlList, req.urlListProcessed);

    return {
      headerList: [
        ["content-length", String(req.blobUrlEntry.size)],
        ["content-type", req.blobUrlEntry.type],
      ],
      status: 200,
      statusMessage: "OK",
      body,
      type: "basic",
      url() {
        if (this.urlList.length == 0) return null;
        return this.urlList[this.urlList.length - 1];
      },
      urlList: recursive
        ? []
        : [...new SafeArrayIterator(req.urlListProcessed)],
    };
  }

  /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
  let reqBody = null;
  let reqRid = null;

  if (req.body) {
    const stream = req.body.streamOrStatic;
    const body = stream.body;

    if (TypedArrayPrototypeGetSymbolToStringTag(body) === "Uint8Array") {
      reqBody = body;
    } else if (typeof body === "string") {
      reqBody = core.encode(body);
    } else if (ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, stream)) {
      const resourceBacking = getReadableStreamResourceBacking(stream);
      if (resourceBacking) {
        reqRid = resourceBacking.rid;
      } else {
        reqRid = resourceForReadableStream(stream, req.body.length);
      }
    } else {
      throw new TypeError("Invalid body");
    }
  }

  const { requestRid, cancelHandleRid } = op_fetch(
    req.method,
    req.currentUrl(),
    req.headerList,
    req.clientRid,
    reqBody !== null || reqRid !== null,
    reqBody,
    reqRid,
  );

  function onAbort() {
    if (cancelHandleRid !== null) {
      core.tryClose(cancelHandleRid);
    }
  }
  terminator[abortSignal.add](onAbort);
  let resp;
  try {
    resp = await opFetchSend(requestRid);
  } catch (err) {
    if (terminator.aborted) return abortedNetworkError();
    throw err;
  } finally {
    if (cancelHandleRid !== null) {
      core.tryClose(cancelHandleRid);
    }
  }
  // Re-throw any body errors
  if (resp.error !== null) {
    const { 0: message, 1: cause } = resp.error;
    throw new TypeError(message, { cause: new Error(cause) });
  }
  if (terminator.aborted) return abortedNetworkError();

  processUrlList(req.urlList, req.urlListProcessed);

  /** @type {InnerResponse} */
  const response = {
    headerList: resp.headers,
    status: resp.status,
    body: null,
    statusMessage: resp.statusText,
    type: "basic",
    url() {
      if (this.urlList.length == 0) return null;
      return this.urlList[this.urlList.length - 1];
    },
    urlList: req.urlListProcessed,
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
        return httpRedirectFetch(req, response, terminator);
      case "manual":
        break;
    }
  }

  if (nullBodyStatus(response.status)) {
    core.close(resp.responseRid);
  } else {
    if (req.method === "HEAD" || req.method === "CONNECT") {
      response.body = null;
      core.close(resp.responseRid);
    } else {
      response.body = new InnerBody(
        createResponseBodyStream(resp.responseRid, terminator),
      );
    }
  }

  if (recursive) return response;

  if (response.urlList.length === 0) {
    processUrlList(req.urlList, req.urlListProcessed);
    response.urlList = [...new SafeArrayIterator(req.urlListProcessed)];
  }

  return response;
}

/**
 * @param {InnerRequest} request
 * @param {InnerResponse} response
 * @param {AbortSignal} terminator
 * @returns {Promise<InnerResponse>}
 */
function httpRedirectFetch(request, response, terminator) {
  const locationHeaders = ArrayPrototypeFilter(
    response.headerList,
    (entry) => byteLowerCase(entry[0]) === "location",
  );
  if (locationHeaders.length === 0) {
    return response;
  }
  const locationURL = new URL(
    locationHeaders[0][1],
    response.url() ?? undefined,
  );
  if (locationURL.hash === "") {
    locationURL.hash = request.currentUrl().hash;
  }
  if (locationURL.protocol !== "https:" && locationURL.protocol !== "http:") {
    return networkError("Can not redirect to a non HTTP(s) url");
  }
  if (request.redirectCount === 20) {
    return networkError("Maximum number of redirects (20) reached");
  }
  request.redirectCount++;
  if (
    response.status !== 303 &&
    request.body !== null &&
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
      request.method !== "GET" &&
      request.method !== "HEAD")
  ) {
    request.method = "GET";
    request.body = null;
    for (let i = 0; i < request.headerList.length; i++) {
      if (
        ArrayPrototypeIncludes(
          REQUEST_BODY_HEADER_NAMES,
          byteLowerCase(request.headerList[i][0]),
        )
      ) {
        ArrayPrototypeSplice(request.headerList, i, 1);
        i--;
      }
    }
  }
  if (request.body !== null) {
    const res = extractBody(request.body.source);
    request.body = res.body;
  }
  ArrayPrototypePush(request.urlList, () => locationURL.href);
  return mainFetch(request, true, terminator);
}

/**
 * @param {RequestInfo} input
 * @param {RequestInit} init
 */
function fetch(input, init = { __proto__: null }) {
  // There is an async dispatch later that causes a stack trace disconnect.
  // We reconnect it by assigning the result of that dispatch to `opPromise`,
  // awaiting `opPromise` in an inner function also named `fetch()` and
  // returning the result from that.
  let opPromise = undefined;
  // 1.
  const result = new Promise((resolve, reject) => {
    const prefix = "Failed to execute 'fetch'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    // 2.
    const requestObject = new Request(input, init);
    // 3.
    const request = toInnerRequest(requestObject);
    // 4.
    if (requestObject.signal.aborted) {
      reject(abortFetch(request, null, requestObject.signal.reason));
      return;
    }

    // 7.
    let responseObject = null;
    // 9.
    let locallyAborted = false;
    // 10.
    function onabort() {
      locallyAborted = true;
      reject(
        abortFetch(request, responseObject, requestObject.signal.reason),
      );
    }
    requestObject.signal[abortSignal.add](onabort);

    if (!requestObject.headers.has("Accept")) {
      ArrayPrototypePush(request.headerList, ["Accept", "*/*"]);
    }

    if (!requestObject.headers.has("Accept-Language")) {
      ArrayPrototypePush(request.headerList, ["Accept-Language", "*"]);
    }

    // 12.
    opPromise = PromisePrototypeCatch(
      PromisePrototypeThen(
        mainFetch(request, false, requestObject.signal),
        (response) => {
          // 12.1.
          if (locallyAborted) return;
          // 12.2.
          if (response.aborted) {
            reject(
              abortFetch(
                request,
                responseObject,
                requestObject.signal.reason,
              ),
            );
            requestObject.signal[abortSignal.remove](onabort);
            return;
          }
          // 12.3.
          if (response.type === "error") {
            const err = new TypeError(
              "Fetch failed: " + (response.error ?? "unknown error"),
            );
            reject(err);
            requestObject.signal[abortSignal.remove](onabort);
            return;
          }
          responseObject = fromInnerResponse(response, "immutable");
          resolve(responseObject);
          requestObject.signal[abortSignal.remove](onabort);
        },
      ),
      (err) => {
        reject(err);
        requestObject.signal[abortSignal.remove](onabort);
      },
    );
  });
  if (opPromise) {
    PromisePrototypeCatch(result, () => {});
    return (async function fetch() {
      await opPromise;
      return result;
    })();
  }
  return result;
}

function abortFetch(request, responseObject, error) {
  if (request.body !== null) {
    // Cancel the body if we haven't taken it as a resource yet
    if (!request.body.streamOrStatic.locked) {
      request.body.cancel(error);
    }
  }
  if (responseObject !== null) {
    const response = toInnerResponse(responseObject);
    if (response.body !== null) response.body.error(error);
  }
  return error;
}

/**
 * Handle the Response argument to the WebAssembly streaming APIs, after
 * resolving if it was passed as a promise. This function should be registered
 * through `Deno.core.setWasmStreamingCallback`.
 *
 * @param {any} source The source parameter that the WebAssembly streaming API
 * was called with. If it was called with a Promise, `source` is the resolved
 * value of that promise.
 * @param {number} rid An rid that represents the wasm streaming resource.
 */
function handleWasmStreaming(source, rid) {
  // This implements part of
  // https://webassembly.github.io/spec/web-api/#compile-a-potential-webassembly-response
  try {
    const res = webidl.converters["Response"](
      source,
      "Failed to execute 'WebAssembly.compileStreaming'",
      "Argument 1",
    );

    // 2.3.
    // The spec is ambiguous here, see
    // https://github.com/WebAssembly/spec/issues/1138. The WPT tests expect
    // the raw value of the Content-Type attribute lowercased. We ignore this
    // for file:// because file fetches don't have a Content-Type.
    if (!StringPrototypeStartsWith(res.url, "file://")) {
      const contentType = res.headers.get("Content-Type");
      if (
        typeof contentType !== "string" ||
        StringPrototypeToLowerCase(contentType) !== "application/wasm"
      ) {
        throw new TypeError("Invalid WebAssembly content type");
      }
    }

    // 2.5.
    if (!res.ok) {
      throw new TypeError(
        `Failed to receive WebAssembly content: HTTP status code ${res.status}`,
      );
    }

    // Pass the resolved URL to v8.
    op_wasm_streaming_set_url(rid, res.url);

    if (res.body !== null) {
      // 2.6.
      // Rather than consuming the body as an ArrayBuffer, this passes each
      // chunk to the feed as soon as it's available.
      PromisePrototypeThen(
        (async () => {
          const reader = res.body.getReader();
          while (true) {
            const { value: chunk, done } = await reader.read();
            if (done) break;
            op_wasm_streaming_feed(rid, chunk);
          }
        })(),
        // 2.7
        () => core.close(rid),
        // 2.8
        (err) => core.abortWasmStreaming(rid, err),
      );
    } else {
      // 2.7
      core.close(rid);
    }
  } catch (err) {
    // 2.8
    core.abortWasmStreaming(rid, err);
  }
}

export { fetch, handleWasmStreaming, mainFetch };
