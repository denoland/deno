// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { errorReadableStream } = window.__bootstrap.streams;
  const { InnerBody, extractBody } = window.__bootstrap.fetchBody;
  const {
    toInnerRequest,
    toInnerResponse,
    fromInnerResponse,
    redirectStatus,
    nullBodyStatus,
    networkError,
    abortedNetworkError,
  } = window.__bootstrap.fetch;
  const abortSignal = window.__bootstrap.abortSignal;
  const { DOMException } = window.__bootstrap.domException;
  const {
    ArrayPrototypePush,
    ArrayPrototypeSplice,
    ArrayPrototypeFilter,
    ArrayPrototypeIncludes,
    Promise,
    PromisePrototypeThen,
    PromisePrototypeCatch,
    StringPrototypeToLowerCase,
    TypedArrayPrototypeSubarray,
    TypeError,
    Uint8Array,
  } = window.__bootstrap.primordials;

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

  // A finalization registry to clean up underlying fetch resources that are GC'ed.
  const RESOURCE_REGISTRY = new FinalizationRegistry((rid) => {
    try {
      core.close(rid);
    } catch {
      // might have already been closed
    }
  });

  /**
   * @param {number} responseBodyRid
   * @param {AbortSignal} [terminator]
   * @returns {ReadableStream<Uint8Array>}
   */
  function createResponseBodyStream(responseBodyRid, terminator) {
    function onAbort() {
      if (readable) {
        errorReadableStream(
          readable,
          new DOMException("Ongoing fetch was aborted.", "AbortError"),
        );
      }
      try {
        core.close(responseBodyRid);
      } catch (_) {
        // might have already been closed
      }
    }
    // TODO(lucacasonato): clean up registration
    terminator[abortSignal.add](onAbort);
    const readable = new ReadableStream({
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
            controller.enqueue(TypedArrayPrototypeSubarray(chunk, 0, read));
          } else {
            RESOURCE_REGISTRY.unregister(readable);
            // We have reached the end of the body, so we close the stream.
            controller.close();
            try {
              core.close(responseBodyRid);
            } catch (_) {
              // might have already been closed
            }
          }
        } catch (err) {
          RESOURCE_REGISTRY.unregister(readable);
          if (terminator.aborted) {
            controller.error(
              new DOMException("Ongoing fetch was aborted.", "AbortError"),
            );
          } else {
            // There was an error while reading a chunk of the body, so we
            // error.
            controller.error(err);
          }
          try {
            core.close(responseBodyRid);
          } catch (_) {
            // might have already been closed
          }
        }
      },
      cancel() {
        if (!terminator.aborted) {
          terminator[abortSignal.signalAbort]();
        }
      },
    });
    RESOURCE_REGISTRY.register(readable, responseBodyRid, readable);
    return readable;
  }

  /**
   * @param {InnerRequest} req
   * @param {boolean} recursive
   * @param {AbortSignal} terminator
   * @returns {Promise<InnerResponse>}
   */
  async function mainFetch(req, recursive, terminator) {
    /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
    let reqBody = null;

    if (req.body !== null) {
      if (req.body.streamOrStatic instanceof ReadableStream) {
        if (req.body.length === null || req.body.source instanceof Blob) {
          reqBody = req.body.stream;
        } else {
          const reader = req.body.stream.getReader();
          const r1 = await reader.read();
          if (r1.done) {
            reqBody = new Uint8Array(0);
          } else {
            reqBody = r1.value;
            const r2 = await reader.read();
            if (!r2.done) throw new TypeError("Unreachable");
          }
        }
      } else {
        req.body.streamOrStatic.consumed = true;
        reqBody = req.body.streamOrStatic.body;
      }
    }

    const { requestRid, requestBodyRid, cancelHandleRid } = opFetch({
      method: req.method,
      url: req.currentUrl(),
      headers: req.headerList,
      clientRid: req.clientRid,
      hasBody: reqBody !== null,
      bodyLength: req.body?.length,
    }, reqBody instanceof Uint8Array ? reqBody : null);

    function onAbort() {
      try {
        core.close(cancelHandleRid);
      } catch (_) {
        // might have already been closed
      }
      try {
        core.close(requestBodyRid);
      } catch (_) {
        // might have already been closed
      }
    }
    terminator[abortSignal.add](onAbort);

    if (requestBodyRid !== null) {
      if (reqBody === null || !(reqBody instanceof ReadableStream)) {
        throw new TypeError("Unreachable");
      }
      const reader = reqBody.getReader();
      (async () => {
        while (true) {
          const { value, done } = await PromisePrototypeCatch(
            reader.read(),
            (err) => {
              if (terminator.aborted) return { done: true, value: undefined };
              throw err;
            },
          );
          if (done) break;
          if (!(value instanceof Uint8Array)) {
            await reader.cancel("value not a Uint8Array");
            break;
          }
          try {
            await PromisePrototypeCatch(
              opFetchRequestWrite(requestBodyRid, value),
              (err) => {
                if (terminator.aborted) return;
                throw err;
              },
            );
            if (terminator.aborted) break;
          } catch (err) {
            await reader.cancel(err);
            break;
          }
        }
        try {
          core.close(requestBodyRid);
        } catch (_) {
          // might have already been closed
        }
      })();
    }

    let resp;
    try {
      resp = await PromisePrototypeCatch(opFetchSend(requestRid), (err) => {
        if (terminator.aborted) return;
        throw err;
      });
    } finally {
      try {
        core.close(cancelHandleRid);
      } catch (_) {
        // might have already been closed
      }
    }
    if (terminator.aborted) return abortedNetworkError();

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
      response.urlList = [...req.urlList];
    }

    return response;
  }

  /**
   * @param {InnerRequest} request
   * @param {InnerResponse} response
   * @returns {Promise<InnerResponse>}
   */
  function httpRedirectFetch(request, response, terminator) {
    const locationHeaders = ArrayPrototypeFilter(
      response.headerList,
      (entry) => entry[0] === "location",
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
            request.headerList[i][0],
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
    ArrayPrototypePush(request.urlList, locationURL.href);
    return mainFetch(request, true, terminator);
  }

  /**
   * @param {RequestInfo} input
   * @param {RequestInit} init
   */
  function fetch(input, init = {}) {
    // 1.
    const p = new Promise((resolve, reject) => {
      const prefix = "Failed to call 'fetch'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      input = webidl.converters["RequestInfo"](input, {
        prefix,
        context: "Argument 1",
      });
      init = webidl.converters["RequestInit"](init, {
        prefix,
        context: "Argument 2",
      });

      // 2.
      const requestObject = new Request(input, init);
      // 3.
      const request = toInnerRequest(requestObject);
      // 4.
      if (requestObject.signal.aborted) {
        reject(abortFetch(request, null));
        return;
      }

      // 7.
      let responseObject = null;
      // 9.
      let locallyAborted = false;
      // 10.
      function onabort() {
        locallyAborted = true;
        reject(abortFetch(request, responseObject));
      }
      requestObject.signal[abortSignal.add](onabort);

      if (!requestObject.headers.has("accept")) {
        ArrayPrototypePush(request.headerList, ["accept", "*/*"]);
      }

      // 12.
      PromisePrototypeCatch(
        PromisePrototypeThen(
          mainFetch(request, false, requestObject.signal),
          (response) => {
            // 12.1.
            if (locallyAborted) return;
            // 12.2.
            if (response.aborted) {
              reject(request, responseObject);
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
    return p;
  }

  function abortFetch(request, responseObject) {
    const error = new DOMException("Ongoing fetch was aborted.", "AbortError");
    if (request.body !== null) request.body.cancel(error);
    if (responseObject !== null) {
      const response = toInnerResponse(responseObject);
      if (response.body !== null) response.body.error(error);
    }
    return error;
  }

  /**
   * Handle the Promise<Response> argument to the WebAssembly streaming
   * APIs. This function should be registered through
   * `Deno.core.setWasmStreamingCallback`.
   *
   * @param {any} source The source parameter that the WebAssembly
   * streaming API was called with.
   * @param {number} rid An rid that can be used with
   * `Deno.core.wasmStreamingFeed`.
   */
  function handleWasmStreaming(source, rid) {
    // This implements part of
    // https://webassembly.github.io/spec/web-api/#compile-a-potential-webassembly-response
    (async () => {
      try {
        const res = webidl.converters["Response"](await source, {
          prefix: "Failed to call 'WebAssembly.compileStreaming'",
          context: "Argument 1",
        });

        // 2.3.
        // The spec is ambiguous here, see
        // https://github.com/WebAssembly/spec/issues/1138. The WPT tests
        // expect the raw value of the Content-Type attribute lowercased.
        const contentType = res.headers.get("Content-Type");
        if (
          typeof contentType !== "string" ||
          StringPrototypeToLowerCase(contentType) !== "application/wasm"
        ) {
          throw new TypeError("Invalid WebAssembly content type.");
        }

        // 2.5.
        if (!res.ok) {
          throw new TypeError(`HTTP status code ${res.status}`);
        }

        // 2.6.
        // Rather than consuming the body as an ArrayBuffer, this passes each
        // chunk to the feed as soon as it's available.
        if (res.body !== null) {
          const reader = res.body.getReader();
          while (true) {
            const { value: chunk, done } = await reader.read();
            if (done) break;
            core.wasmStreamingFeed(rid, "bytes", chunk);
          }
        }

        // 2.7.
        core.wasmStreamingFeed(rid, "finish");
      } catch (err) {
        // 2.8 and 3
        core.wasmStreamingFeed(rid, "abort", err);
      }
    })();
  }

  window.__bootstrap.fetch ??= {};
  window.__bootstrap.fetch.fetch = fetch;
  window.__bootstrap.fetch.handleWasmStreaming = handleWasmStreaming;
})(this);
