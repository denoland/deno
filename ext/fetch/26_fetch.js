// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, internals, primordials } = __bootstrap;
const {
  op_fetch,
  op_fetch_promise_is_settled,
  op_fetch_send,
  op_wasm_streaming_feed,
  op_wasm_streaming_set_url,
} = core.ops;
const {
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  DateNow,
  Error,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromisePrototypeCatch,
  SafeArrayIterator,
  SafePromisePrototypeFinally,
  String,
  StringPrototypeEndsWith,
  StringPrototypeIndexOf,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  StringPrototypeTrim,
  TypeError,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetSymbolToStringTag,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { byteLowerCase } = core.loadExtScript("ext:deno_web/00_infra.js");
const {
  errorReadableStream,
  getReadableStreamResourceBacking,
  readableStreamForRid,
  ReadableStreamPrototype,
  resourceForReadableStream,
} = core.loadExtScript("ext:deno_web/06_streams.js");
const { extractBody, InnerBody } = core.loadExtScript(
  "ext:deno_fetch/22_body.js",
);
const { processUrlList, Request, toInnerRequest } = core.loadExtScript(
  "ext:deno_fetch/23_request.js",
);
const {
  abortedNetworkError,
  fromInnerResponse,
  networkError,
  nullBodyStatus,
  redirectStatus,
  toInnerResponse,
} = core.loadExtScript("ext:deno_fetch/23_response.js");
const abortSignal = core.loadExtScript("ext:deno_web/03_abort_signal.js");
const {
  builtinTracer,
  ContextManager,
  enterSpan,
  restoreSnapshot,
} = internals.__telemetry;
const __telemetry = internals.__telemetry;
const {
  updateSpanFromClientResponse,
  updateSpanFromError,
  updateSpanFromRequest,
} = internals.__telemetryUtil;

const REQUEST_BODY_HEADER_NAMES = [
  "content-encoding",
  "content-language",
  "content-location",
  "content-type",
];

const REDIRECT_SENSITIVE_HEADER_NAMES = [
  "authorization",
  "proxy-authorization",
  "cookie",
];

// ============================================================================
// Inspector Network domain instrumentation (Chrome DevTools Protocol).
//
// When `node:inspector` has been loaded and `--inspect` is active, fetch()
// emits `Network.requestWillBeSent` / `responseReceived` / `dataReceived` /
// `loadingFinished` / `loadingFailed` events. The actual emitters and a
// monotonic requestId generator are installed by `ext/node/polyfills/
// inspector.js` onto `internals.__inspectorNetwork` so this layer doesn't
// have to depend on ext/node.
// ============================================================================

function getInspectorNetwork() {
  const ins = internals.__inspectorNetwork;
  if (ins && ins.isEnabled()) return ins;
  return null;
}

// Join repeated header values according to Chrome DevTools conventions:
// cookies use `; `, set-cookie uses `\n`, everything else uses `, `.
//
// Response header names are typically lowercased on the wire by hyper, but
// CDP / Node frontends conventionally key `Set-Cookie` with its canonical
// case (the test suite asserts `headers['Set-Cookie']`). Apply a small
// canonicalization for the names that conventionally carry case.
function joinHeaderValuesForCdp(headerList, lowerCaseNames) {
  const out = { __proto__: null };
  for (let i = 0; i < headerList.length; i++) {
    const rawName = headerList[i][0];
    const value = String(headerList[i][1]);
    const lower = byteLowerCase(rawName);
    let name;
    if (lowerCaseNames) {
      name = lower;
    } else if (lower === "set-cookie") {
      name = "Set-Cookie";
    } else {
      name = rawName;
    }
    let separator;
    if (lower === "cookie") {
      separator = "; ";
    } else if (lower === "set-cookie") {
      separator = "\n";
    } else {
      separator = ", ";
    }
    if (out[name] === undefined) {
      out[name] = value;
    } else {
      out[name] = out[name] + separator + value;
    }
  }
  return out;
}

// Parse Content-Type into { mimeType, charset } for `response.mimeType` and
// `response.charset` (and to decide whether `getResponseBody` returns the
// body as a utf-8 string or base64).
function parseContentTypeForCdp(headerList) {
  let raw = null;
  for (let i = 0; i < headerList.length; i++) {
    if (byteLowerCase(headerList[i][0]) === "content-type") {
      raw = String(headerList[i][1]);
      break;
    }
  }
  if (raw === null) return { mimeType: "", charset: "" };
  const semi = StringPrototypeIndexOf(raw, ";");
  const mimeType = semi === -1
    ? StringPrototypeTrim(raw)
    : StringPrototypeTrim(StringPrototypeSlice(raw, 0, semi));
  let charset = "";
  if (semi !== -1) {
    const rest = StringPrototypeSlice(raw, semi + 1);
    const parts = StringPrototypeSplit(rest, ";");
    for (let i = 0; i < parts.length; i++) {
      const p = StringPrototypeTrim(parts[i]);
      if (
        StringPrototypeStartsWith(StringPrototypeToLowerCase(p), "charset=")
      ) {
        charset = StringPrototypeTrim(StringPrototypeSlice(p, 8));
        // Strip optional surrounding quotes.
        if (
          charset.length >= 2 && charset[0] === '"' &&
          charset[charset.length - 1] === '"'
        ) {
          charset = StringPrototypeSlice(charset, 1, charset.length - 1);
        }
        break;
      }
    }
  }
  return { mimeType, charset };
}

// Run a background drain of the inspector branch of a tee'd response stream,
// emitting `Network.dataReceived` per chunk and `Network.loadingFinished` /
// `loadingFailed` when the stream ends. Errors from this drain are swallowed
// so they can't surface as unhandled rejections to user code.
function drainResponseForInspector(inspectorStream, requestId, ins) {
  const reader = inspectorStream.getReader();
  let totalLength = 0;
  (async () => {
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        if (value) {
          const len = TypedArrayPrototypeGetByteLength(value);
          if (len > 0) {
            totalLength += len;
            ins.dataReceived({
              requestId,
              timestamp: DateNow() / 1000,
              dataLength: len,
              encodedDataLength: len,
              data: value,
            });
          }
        }
      }
      ins.loadingFinished({
        requestId,
        timestamp: DateNow() / 1000,
        encodedDataLength: totalLength,
      });
    } catch (err) {
      try {
        ins.loadingFailed({
          requestId,
          timestamp: DateNow() / 1000,
          type: "Fetch",
          errorText: err && err.message ? String(err.message) : String(err),
        });
      } catch {
        // ignore - inspector may have detached mid-drain
      }
    } finally {
      try {
        reader.releaseLock();
      } catch {
        // ignore
      }
    }
  })();
}

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

  // ---- Inspector: Network.requestWillBeSent ------------------------------
  // Only fires when `node:inspector` is loaded AND the inspector is currently
  // attached, so the cost is one method call (`isEnabled()`) otherwise.
  const inspectorNetwork = getInspectorNetwork();
  let inspectorRequestId = null;
  if (inspectorNetwork !== null && !recursive) {
    inspectorRequestId = inspectorNetwork.nextRequestId();
    const hasPostData = reqBody !== null;
    let postDataText;
    if (
      hasPostData && TypedArrayPrototypeGetSymbolToStringTag(reqBody) ===
        "Uint8Array"
    ) {
      try {
        postDataText = core.decode(reqBody);
      } catch {
        postDataText = undefined;
      }
    }
    const requestHeadersForCdp = joinHeaderValuesForCdp(
      req.headerList,
      /* lowerCaseNames */ true,
    );
    // The buffer's request_charset gates `Network.getRequestPostData`
    // (utf-8 only). Prefer an explicit charset from Content-Type; fall
    // back to utf-8 when we successfully decoded the body as a JS string,
    // since fetch encodes string bodies as utf-8 over the wire.
    let requestCharset;
    for (let i = 0; i < req.headerList.length; i++) {
      if (byteLowerCase(req.headerList[i][0]) === "content-type") {
        requestCharset = parseContentTypeForCdp(req.headerList).charset ||
          undefined;
        break;
      }
    }
    if (requestCharset === undefined && postDataText !== undefined) {
      requestCharset = "utf-8";
    }
    try {
      inspectorNetwork.requestWillBeSent({
        requestId: inspectorRequestId,
        timestamp: DateNow() / 1000,
        wallTime: DateNow() / 1000,
        type: "Fetch",
        request: {
          url: req.currentUrl(),
          method: req.method,
          headers: requestHeadersForCdp,
          hasPostData,
          postData: postDataText,
        },
        // initiator: filled in by `op_inspector_emit_protocol_event` using
        // V8's current stack trace, so the user-code frame at the fetch()
        // call site is preserved.
        charset: requestCharset,
      });
      // We supplied the entire request body inline via `request.postData`,
      // so flip the buffer's `is_request_finished` flag immediately - else
      // `Network.getRequestPostData` would reject with "Request data is
      // not finished yet". Streaming bodies (reqRid !== null) take the
      // chunked `Network.dataSent` path instead and aren't covered here.
      if (hasPostData) {
        inspectorNetwork.dataSent({
          requestId: inspectorRequestId,
          finished: true,
        });
      }
    } catch {
      // never let inspector instrumentation break a real fetch
    }
  }

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
    if (inspectorRequestId !== null) {
      try {
        inspectorNetwork.loadingFailed({
          requestId: inspectorRequestId,
          timestamp: DateNow() / 1000,
          type: "Fetch",
          errorText: err && err.message ? String(err.message) : String(err),
        });
      } catch {
        // ignore
      }
    }
    if (terminator.aborted) return abortedNetworkError();
    throw err;
  } finally {
    if (cancelHandleRid !== null) {
      core.tryClose(cancelHandleRid);
    }
  }
  // Re-throw any body errors
  if (resp.error !== null) {
    if (inspectorRequestId !== null) {
      try {
        inspectorNetwork.loadingFailed({
          requestId: inspectorRequestId,
          timestamp: DateNow() / 1000,
          type: "Fetch",
          errorText: resp.error[0],
        });
      } catch {
        // ignore
      }
    }
    const { 0: message, 1: cause } = resp.error;
    throw new TypeError(message, { cause: new Error(cause) });
  }
  if (terminator.aborted) {
    // op_fetch_send resolved successfully, so the FetchResponseResource is already in
    // the resource table. The success path below either closes resp.responseRid
    // (redirect / null-body / HEAD / CONNECT) or hands it to createResponseBodyStream,
    // which owns its lifecycle. Only this aborted-after-resolve branch needs to close
    // the rid manually, otherwise it leaks and trips the test sanitizer.
    core.tryClose(resp.responseRid);
    return abortedNetworkError();
  }

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

  // ---- Inspector: Network.responseReceived -------------------------------
  if (inspectorRequestId !== null) {
    const { mimeType, charset } = parseContentTypeForCdp(resp.headers);
    const responseHeadersForCdp = joinHeaderValuesForCdp(
      resp.headers,
      /* lowerCaseNames */ false,
    );
    try {
      inspectorNetwork.responseReceived({
        requestId: inspectorRequestId,
        timestamp: DateNow() / 1000,
        type: "Fetch",
        response: {
          url: resp.url || req.currentUrl(),
          status: resp.status,
          statusText: resp.statusText,
          headers: responseHeadersForCdp,
          mimeType,
          charset,
        },
      });
    } catch {
      // ignore
    }
  }

  if (redirectStatus(resp.status)) {
    switch (req.redirectMode) {
      case "error":
        core.close(resp.responseRid);
        if (inspectorRequestId !== null) {
          try {
            inspectorNetwork.loadingFailed({
              requestId: inspectorRequestId,
              timestamp: DateNow() / 1000,
              type: "Fetch",
              errorText:
                "Encountered redirect while redirect mode is set to 'error'",
            });
          } catch {
            // ignore
          }
        }
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
    if (inspectorRequestId !== null) {
      try {
        inspectorNetwork.loadingFinished({
          requestId: inspectorRequestId,
          timestamp: DateNow() / 1000,
          encodedDataLength: 0,
        });
      } catch {
        // ignore
      }
    }
  } else {
    if (req.method === "HEAD" || req.method === "CONNECT") {
      response.body = null;
      core.close(resp.responseRid);
      if (inspectorRequestId !== null) {
        try {
          inspectorNetwork.loadingFinished({
            requestId: inspectorRequestId,
            timestamp: DateNow() / 1000,
            encodedDataLength: 0,
          });
        } catch {
          // ignore
        }
      }
    } else {
      let bodyStream = createResponseBodyStream(resp.responseRid, terminator);
      // Tee the response body so the inspector can drain a copy in the
      // background for `Network.dataReceived` + `loadingFinished` /
      // `getResponseBody`, while the user still consumes the original.
      if (inspectorRequestId !== null) {
        try {
          const tee = bodyStream.tee();
          bodyStream = tee[0];
          drainResponseForInspector(
            tee[1],
            inspectorRequestId,
            inspectorNetwork,
          );
        } catch {
          // tee failed; leave bodyStream untouched, no inspector data
        }
      }
      response.body = new InnerBody(bodyStream);
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

  const currentURL = new URL(request.currentUrl());
  const locationURL = new URL(
    locationHeaders[0][1],
    response.url() ?? undefined,
  );
  if (locationURL.hash === "") {
    locationURL.hash = currentURL.hash;
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

  // Drop confidential headers when redirecting to a less secure protocol
  // or to a different domain that is not a superdomain
  if (
    (locationURL.protocol !== currentURL.protocol &&
      locationURL.protocol !== "https:") ||
    (locationURL.host !== currentURL.host &&
      !isSubdomain(locationURL.host, currentURL.host))
  ) {
    for (let i = 0; i < request.headerList.length; i++) {
      if (
        ArrayPrototypeIncludes(
          REDIRECT_SENSITIVE_HEADER_NAMES,
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
function fetch(input, init = undefined) {
  let span;
  let snapshot;
  try {
    if (__telemetry.TRACING_ENABLED) {
      span = builtinTracer().startSpan("fetch", { kind: 2 });
      snapshot = enterSpan(span);
    }

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

      if (span) {
        const context = ContextManager.active();
        for (
          const propagator of new SafeArrayIterator(__telemetry.PROPAGATORS)
        ) {
          propagator.inject(context, requestObject.headers, {
            set(carrier, key, value) {
              carrier.append(key, value);
            },
          });
        }

        updateSpanFromRequest(span, requestObject);
      }

      // 3.
      const request = toInnerRequest(requestObject);
      // 4.
      if (requestObject.signal.aborted) {
        if (span) {
          // Handles this case here as this is the only case where `result` promise
          // is settled immediately.
          updateSpanFromError(span, requestObject.signal.reason);
        }
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

            if (span) {
              updateSpanFromClientResponse(span, responseObject);
            }

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
      PromisePrototypeCatch(result, (e) => {
        if (span) {
          updateSpanFromError(span, e);
        }
      });
      return (async function fetch() {
      try {
        await opPromise;
        return result;
      } finally {
        span?.end();
      }
      })();
    }
    // We need to end the span when the promise settles.
    // WPT has a test that aborted fetch is settled in the same tick.
    // This means we cannot wrap the promise if it is already settled.
    // But this is OK, because we can just immediately end the span
    // in that case.
    if (span) {
      // XXX: This should always be true, otherwise `opPromise` would be present.
      if (op_fetch_promise_is_settled(result)) {
        // It's already settled.
        span?.end();
      } else {
        // Not settled yet, we can return a new wrapper promise.
        return SafePromisePrototypeFinally(result, () => {
          span?.end();
        });
      }
    }
    return result;
  } finally {
    if (snapshot) restoreSnapshot(snapshot);
  }
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
 * Checks if the given string is a subdomain of the given domain.
 *
 * @param {String} subdomain
 * @param {String} domain
 * @returns {Boolean}
 */
function isSubdomain(subdomain, domain) {
  const dot = subdomain.length - domain.length - 1;
  return (
    dot > 0 &&
    subdomain[dot] === "." &&
    StringPrototypeEndsWith(subdomain, domain)
  );
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

return { fetch, handleWasmStreaming, mainFetch };
})();
