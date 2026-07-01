// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, internals, primordials } = __bootstrap;
const {
  BadResourcePrototype,
  InterruptedPrototype,
  Interrupted,
  internalRidSymbol,
} = core;
const {
  op_http_cancel,
  op_http_close,
  op_http_close_after_finish,
  op_http_copy_span_to_otel_info,
  op_http_get_request_header,
  op_http_get_request_headers,
  op_http_get_request_method,
  op_http_get_request_url,
  op_http_get_request_remote_addr,
  op_http_is_raw_request,
  op_http_metric_handle_otel_error,
  op_http_notify_serving,
  op_http_read_request_body,
  op_http_request_on_cancel,
  op_http_serve,
  op_http_serve_address_override,
  op_http_serve_default_compression,
  op_http_serve_on,
  op_http_set_promise_complete,
  op_http_set_response_native,
  op_http_set_response_body_bytes,
  op_http_set_response_body_bytes_with_headers,
  op_http_set_response_body_resource,
  op_http_set_response_body_static_with_content_type,
  op_http_set_response_body_static_with_default_header,
  op_http_set_response_body_static_with_header,
  op_http_set_response_body_text,
  op_http_set_response_body_text_with_headers,
  op_http_set_response_header,
  op_http_set_response_headers,
  op_http_set_response_trailers,
  op_http_try_take_full_request_body,
  op_http_try_take_full_request_body_text,
  op_http_upgrade_raw,
  op_http_upgrade_websocket_next,
  op_http_wait,
} = core.ops;

const {
  ArrayPrototypeFind,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototype,
  PromisePrototypeCatch,
  PromiseResolve,
  SafeArrayIterator,
  SafePromisePrototypeFinally,
  SafePromiseAll,
  PromisePrototypeThen,
  StringPrototypeIncludes,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  Symbol,
  SymbolAsyncDispose,
  TypeError,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetSymbolToStringTag,
  Uint8Array,
  Promise,
  Number,
} = primordials;

const { InnerBody } = core.loadExtScript("ext:deno_fetch/22_body.js");
const {
  dropServeNativeResponse,
  fromInnerResponse,
  getInnerResponse,
  newInnerResponse,
  responseBodyUsed,
  ResponsePrototype,
  serveNativeResponseKey,
  serveFastBodyKey,
  serveFastConsumedKey,
  serveFastContentTypeKey,
  serveFastHeaderKindKey,
  serveFastStatusKey,
  SERVE_FAST_HEADER_CONTENT_TYPE,
  SERVE_FAST_HEADER_DEFAULT_TEXT,
  SERVE_FAST_HEADER_NONE,
  toInnerResponse,
  wireHeaderList,
} = core.loadExtScript("ext:deno_fetch/23_response.js");
const {
  abortRequest,
  cacheRequestHeaders,
  fromInnerRequest,
  requestHeadersExposed,
  toInnerRequest,
} = core.loadExtScript("ext:deno_fetch/23_request.js");
const { AbortController } = core.loadExtScript(
  "ext:deno_web/03_abort_signal.js",
);
const {
  getReadableStreamResourceBacking,
  readableStreamForRid,
  ReadableStreamPrototype,
  resourceForReadableStream,
} = core.loadExtScript("ext:deno_web/06_streams.js");
const {
  listen,
  listenOptionApiName,
  UpgradedConn,
} = core.loadExtScript("ext:deno_net/01_net.js");
const { hasTlsKeyPairOptions, listenTls } = core.loadExtScript(
  "ext:deno_net/02_tls.js",
);
const {
  otelState,
  builtinTracer,
  ContextManager,
  currentSnapshot,
  enterSpan,
  restoreSnapshot,
} = core.loadExtScript("ext:deno_telemetry/telemetry.ts");
const {
  updateSpanFromRequest,
  updateSpanFromServerResponse,
} = core.loadExtScript("ext:deno_telemetry/util.ts");

const _upgraded = Symbol("_upgraded");

let legacyAbortWarned = false;

function internalServerError() {
  // "Internal Server Error"
  return new Response(
    new Uint8Array([
      73,
      110,
      116,
      101,
      114,
      110,
      97,
      108,
      32,
      83,
      101,
      114,
      118,
      101,
      114,
      32,
      69,
      114,
      114,
      111,
      114,
    ]),
    { status: 500 },
  );
}

// Used to ensure that user returns a valid response (but not a different response) from handlers that are upgraded.
const UPGRADE_RESPONSE_SENTINEL = fromInnerResponse(
  newInnerResponse(101),
  "immutable",
);

function upgradeHttpRaw(req) {
  const inner = toInnerRequest(req);
  if (inner?._wantsUpgrade) {
    return inner._wantsUpgrade("upgradeHttpRaw");
  }
  throw new TypeError("'upgradeHttpRaw' may only be used with Deno.serve");
}

function addTrailers(resp, headerList) {
  const inner = toInnerResponse(resp);
  op_http_set_response_trailers(inner.external, headerList);
}

class InnerRequest {
  #external;
  #context;
  #methodValue;
  #streamRid;
  #body;
  #upgraded;
  #urlValue;
  #completed;
  #signalAccessed;
  request;

  constructor(external, context) {
    this.#external = external;
    this.#context = context;
    this.#upgraded = false;
    this.#completed = undefined;
    this.#signalAccessed = false;
  }

  close(success = true) {
    if (this.#streamRid !== undefined) {
      core.tryClose(this.#streamRid);
      this.#streamRid = undefined;
    }
    // The completion signal fires only if someone cares
    if (this.#completed) {
      if (success) {
        this.#completed.resolve(undefined);
      } else {
        if (!this.#context.legacyAbort) {
          abortRequest(this.request);
        }
        this.#completed.reject(
          new Interrupted("HTTP response was not sent successfully"),
        );
      }
    }
    if (this.#context.legacyAbort) {
      if (success && this.#signalAccessed && !legacyAbortWarned) {
        legacyAbortWarned = true;
        // deno-lint-ignore no-console
        console.warn(
          "Deno.serve: request.signal aborts on successful responses (legacy behavior, see https://github.com/denoland/deno/issues/29111). Move cleanup to the handler's return path, or opt in to the new behavior with --unstable-no-legacy-abort. See https://docs.deno.com/runtime/reference/migrate-deprecations/",
        );
      }
      abortRequest(this.request);
    }
    this.#external = null;
  }

  get [_upgraded]() {
    return this.#upgraded;
  }

  _throwIfUpgraded() {
    if (this.#upgraded) {
      throw new Deno.errors.Http("Already upgraded");
    }
  }

  _wantsUpgrade(upgradeType) {
    if (this.#upgraded) {
      throw new Deno.errors.Http("Already upgraded");
    }
    if (this.#external === null) {
      throw new Deno.errors.Http("Already closed");
    }

    if (upgradeType == "upgradeHttpRaw") {
      const external = this.#external;

      this.url();
      this.headerList;
      const remoteAddr = this.remoteAddr;
      this.close();

      this.#upgraded = true;

      const upgradeRid = op_http_upgrade_raw(external);

      const conn = new UpgradedConn(
        upgradeRid,
        remoteAddr,
        this.#context.listener.addr,
      );

      return { response: UPGRADE_RESPONSE_SENTINEL, conn };
    }

    if (upgradeType == "upgradeWebSocket") {
      const external = this.#external;

      this.url();
      this.headerList;
      this.remoteAddr;
      this.close();

      this.#upgraded = true;

      return op_http_upgrade_websocket_next(external);
    }
  }

  url() {
    if (this.#urlValue !== undefined) {
      return this.#urlValue;
    }

    if (this.#external === null) {
      throw new TypeError("Request closed");
    }

    if (this.#methodValue === undefined) {
      this.#methodValue = op_http_get_request_method(this.#external);
    }

    return this.#urlValue = op_http_get_request_url(this.#external);
  }

  get completed() {
    if (!this.#completed) {
      // NOTE: this is faster than Promise.withResolvers()
      let resolve, reject;
      const promise = new Promise((r1, r2) => {
        resolve = r1;
        reject = r2;
      });
      this.#completed = { promise, resolve, reject };
    }
    return this.#completed.promise;
  }

  get remoteAddr() {
    if (this.#external === null) {
      throw new TypeError("Request closed");
    }
    const remoteAddr = op_http_get_request_remote_addr(this.#external);
    const transport = this.#context.listener?.addr.transport;
    if (remoteAddr[0] === "unix") {
      return {
        transport,
        path: this.#context.listener.addr.path,
      };
    }
    if (StringPrototypeStartsWith(remoteAddr[0], "vsock:")) {
      return {
        transport,
        cid: Number(StringPrototypeSlice(remoteAddr[0], 6)),
        port: remoteAddr[1],
      };
    }
    return {
      transport: "tcp",
      hostname: remoteAddr[0],
      port: remoteAddr[1],
    };
  }

  get method() {
    if (this.#methodValue === undefined) {
      if (this.#external === null) {
        throw new TypeError("Request closed");
      }
      this.#methodValue = op_http_get_request_method(this.#external);
    }
    return this.#methodValue;
  }

  get body() {
    if (this.#external === null) {
      throw new TypeError("Request closed");
    }
    if (this.#body !== undefined) {
      return this.#body;
    }
    // If the method is GET, HEAD, or CONNECT, we do not want to include a body here, even if the Rust
    // side of the code is willing to provide it to us.
    if (
      this.method == "GET" || this.method == "HEAD" ||
      this.method == "CONNECT"
    ) {
      this.#body = null;
      return null;
    }
    // Fast path: if the entire body is already buffered in hyper
    // (typical small POST keep-alive case), skip the ReadableStream
    // wrapper, op_http_read_request_body resource allocation, and
    // the disturb/close plumbing -- hand the bytes straight to
    // InnerBody's static path. On `null` the body is left intact
    // and we fall through to the streaming path.
    const buffered = op_http_try_take_full_request_body(this.#external);
    if (buffered !== null) {
      this.#body = new InnerBody({ body: buffered, consumed: false });
      if (this.header("content-length") !== null) {
        this.#body.length = TypedArrayPrototypeGetByteLength(buffered);
      }
      return this.#body;
    }
    this.#streamRid = op_http_read_request_body(this.#external);
    this.#body = new InnerBody(
      readableStreamForRid(
        this.#streamRid,
        false,
        undefined,
        (controller, error) => {
          if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
            // TODO(kt3k): We would like to pass `error` as `cause` when BadResource supports it.
            controller.error(
              new error.constructor(
                `Cannot read request body as underlying resource unavailable`,
              ),
            );
          } else {
            controller.error(error);
          }
        },
      ),
    );
    return this.#body;
  }

  get headerList() {
    if (this.#external === null) {
      throw new TypeError("Request closed");
    }
    const headers = [];
    const reqHeaders = op_http_get_request_headers(this.#external);
    for (let i = 0; i < reqHeaders.length; i += 2) {
      ArrayPrototypePush(headers, [reqHeaders[i], reqHeaders[i + 1]]);
    }
    return headers;
  }

  header(name) {
    if (this.#external === null) {
      throw new TypeError("Request closed");
    }
    return op_http_get_request_header(this.#external, name);
  }

  consumeTextBody() {
    if (this.#external === null || this.#body !== undefined) {
      return null;
    }
    if (
      this.method == "GET" || this.method == "HEAD" ||
      this.method == "CONNECT"
    ) {
      this.#body = null;
      return "";
    }
    const text = op_http_try_take_full_request_body_text(this.#external);
    if (text === null) {
      return null;
    }
    this.#body = new InnerBody({ body: text, consumed: true });
    return text;
  }

  get external() {
    return this.#external;
  }

  onCancel(callback) {
    this.#signalAccessed = true;
    if (this.#external === null) {
      if (this.#context.legacyAbort) callback();
      return;
    }

    PromisePrototypeThen(
      op_http_request_on_cancel(this.#external),
      (r) => {
        return !this.#context.legacyAbort ? r && callback() : callback();
      },
    );
  }
}

class CallbackContext {
  abortController;
  scheme;
  fallbackHost;
  serverRid;
  closed;
  /** @type {Promise<void> | undefined} */
  closing;
  listener;
  asyncContextSnapshot;
  legacyAbort;

  constructor(signal, args, listener) {
    this.asyncContextSnapshot = currentSnapshot();
    // The abort signal triggers a non-graceful shutdown
    signal?.addEventListener(
      "abort",
      () => {
        op_http_cancel(this.serverRid, false);
      },
      { once: true },
    );
    this.abortController = new AbortController();
    this.serverRid = args[0];
    this.scheme = args[1];
    this.fallbackHost = args[2];
    this.legacyAbort = args[3] == false;
    this.closed = false;
    this.listener = listener;
  }

  close() {
    try {
      this.closed = true;
      core.tryClose(this.serverRid);
    } catch {
      // Pass
    }
  }
}

class ServeHandlerInfo {
  #inner: InnerRequest;
  constructor(inner: InnerRequest) {
    this.#inner = inner;
  }
  get remoteAddr() {
    return this.#inner.remoteAddr;
  }
  get completed() {
    return this.#inner.completed;
  }
}

function setResponseHeaders(req, headers) {
  if (headers && headers.length > 0) {
    if (headers.length == 1) {
      op_http_set_response_header(req, headers[0][0], headers[0][1]);
    } else {
      op_http_set_response_headers(req, headers);
    }
  }
}

function closeInnerRequestImmediately(innerRequest) {
  innerRequest?.close();
}

function closeInnerRequestForNative(innerRequest) {
  innerRequest?.close();
}

function trySetServeFastStaticResponse(
  req,
  response,
  innerRequest,
  closeInnerRequest = closeInnerRequestImmediately,
) {
  const status = response[serveFastStatusKey];
  if (status === 0) {
    return false;
  }

  const body = response[serveFastBodyKey];
  if (body === null) {
    return false;
  }

  closeInnerRequest(innerRequest);
  response[serveFastConsumedKey] = true;
  switch (response[serveFastHeaderKindKey]) {
    case SERVE_FAST_HEADER_DEFAULT_TEXT:
      op_http_set_response_body_static_with_default_header(req, body, status);
      return true;
    case SERVE_FAST_HEADER_CONTENT_TYPE:
      op_http_set_response_body_static_with_content_type(
        req,
        body,
        status,
        response[serveFastContentTypeKey],
      );
      return true;
    case SERVE_FAST_HEADER_NONE:
      if (typeof body === "string") {
        op_http_set_response_body_text(req, body, status);
      } else {
        op_http_set_response_body_bytes(req, body, status);
      }
      return true;
    default:
      throw new TypeError("Invalid response");
  }
}

// Report an error that was thrown while a streaming response body was being
// drained. The response status/headers are already on the wire at this point,
// so the value returned from `onError` can no longer be used; we route the
// error through the handler purely so it is observed (the default handler logs
// a stack trace) instead of being silently swallowed.
function reportResponseStreamError(onError, error) {
  let result;
  try {
    result = onError(error);
  } catch (e) {
    internals.log("error", "Exception in onError while handling exception", e);
    return;
  }
  if (ObjectPrototypeIsPrototypeOf(PromisePrototype, result)) {
    PromisePrototypeThen(result, undefined, (e) => {
      internals.log(
        "error",
        "Exception in onError while handling exception",
        e,
      );
    });
  }
}

function fastSyncResponseOrStream(
  req,
  respBody,
  status,
  innerRequest: InnerRequest,
  headers,
  onError,
) {
  if (respBody === null || respBody === undefined) {
    // Don't set the body
    innerRequest?.close();
    setResponseHeaders(req, headers);
    op_http_set_promise_complete(req, status);
    return;
  }

  const stream = respBody.streamOrStatic;
  const body = stream.body;
  const singleHeader = headers?.length === 1 ? headers[0] : null;
  if (body !== undefined) {
    // We ensure the response has not been consumed yet in the caller of this
    // function.
    stream.consumed = true;
    if (
      singleHeader !== null &&
      (singleHeader[0] === "Content-Type" ||
        singleHeader[0] === "content-type") &&
      singleHeader[1] === "text/plain;charset=UTF-8"
    ) {
      innerRequest?.close();
      op_http_set_response_body_static_with_default_header(req, body, status);
      return;
    }
    if (singleHeader !== null) {
      innerRequest?.close();
      if (
        singleHeader[0] === "Content-Type" ||
        singleHeader[0] === "content-type"
      ) {
        op_http_set_response_body_static_with_content_type(
          req,
          body,
          status,
          singleHeader[1],
        );
        return;
      }
      op_http_set_response_body_static_with_header(
        req,
        body,
        status,
        singleHeader[0],
        singleHeader[1],
      );
      return;
    }
  }

  if (TypedArrayPrototypeGetSymbolToStringTag(body) === "Uint8Array") {
    innerRequest?.close();
    if (headers?.length > 0) {
      op_http_set_response_body_bytes_with_headers(
        req,
        body,
        status,
        headers,
      );
    } else {
      op_http_set_response_body_bytes(req, body, status);
    }
    return;
  }

  if (typeof body === "string") {
    innerRequest?.close();
    if (headers?.length > 0) {
      op_http_set_response_body_text_with_headers(
        req,
        body,
        status,
        headers,
      );
    } else {
      op_http_set_response_body_text(req, body, status);
    }
    return;
  }

  // At this point in the response it needs to be a stream
  if (!ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, stream)) {
    innerRequest?.close();
    throw new TypeError("Invalid response");
  }
  setResponseHeaders(req, headers);
  const resourceBacking = getReadableStreamResourceBacking(stream);
  let rid, autoClose;
  if (resourceBacking) {
    rid = resourceBacking.rid;
    autoClose = resourceBacking.autoClose;
  } else {
    // The response headers/status have already been committed by the time the
    // body stream starts producing chunks, so an error thrown while draining
    // the stream (e.g. inside a `TransformStream` transformer) can no longer
    // change the response. Report it through the server's error handler so it
    // is not silently swallowed and a stack trace implicating the faulty
    // callback is surfaced. See https://github.com/denoland/deno/issues/19867.
    rid = resourceForReadableStream(stream, undefined, onError);
    autoClose = true;
  }
  PromisePrototypeThen(
    op_http_set_response_body_resource(req, rid, autoClose, status),
    (success) => {
      innerRequest?.close(success);
      op_http_close_after_finish(req);
    },
    () => {
      // Setting up the streamed response body failed because the backing
      // resource was unavailable (e.g. a `using` file handle that was disposed
      // when the handler returned, leaving `file.readable` backed by a closed
      // rid). No response has been sent at this point, so complete the request
      // with a 500 instead of letting the rejection escape as a fatal
      // unhandled promise rejection that would take the whole server down.
      innerRequest?.close();
      op_http_set_promise_complete(req, 500);
    },
  );
}

/**
 * Maps the incoming request slab ID to a fully-fledged Request object, passes it to the user-provided
 * callback, then extracts the response that was returned from that callback. The response is then pulled
 * apart and handled on the Rust side.
 *
 * This function returns a promise that will only reject in the case of abnormal exit.
 */
function mapToCallback(context, callback, onError) {
  const zeroArgCallback = callback.length === 0 &&
    !otelState.TRACING_ENABLED;
  let mapped = async function (req, span) {
    // Get the response from the user-provided callback. If that fails, use onError. If that fails, return a fallback
    // 500 error.
    let innerRequest;
    let response;
    let inner;
    try {
      if (zeroArgCallback && op_http_is_raw_request(req)) {
        response = await callback();
      } else {
        innerRequest = new InnerRequest(req, context);
        const request = fromInnerRequest(innerRequest, "immutable");
        innerRequest.request = request;

        if (span) {
          updateSpanFromRequest(span, request);
        }

        response = await callback(
          request,
          new ServeHandlerInfo(innerRequest),
        );
      }

      // Throwing Error if the handler return value is not a Response class
      if (!ObjectPrototypeIsPrototypeOf(ResponsePrototype, response)) {
        throw new TypeError(
          "Return value from serve handler must be a response or a promise resolving to a response",
        );
      }

      // The Response prototype check above passes for Response-like objects
      // (e.g. a subclass that skipped super(), or a Response from a different
      // realm/polyfill). Those don't carry the internal slot we read from
      // below, so reject them with a clear error instead of crashing later.
      inner = getInnerResponse(response);
      if (inner === undefined) {
        throw new TypeError(
          "Return value from serve handler must be a Response constructed via the Response constructor in this realm",
        );
      }

      if (inner.type === "error") {
        throw new TypeError(
          "Return value from serve handler must not be an error response (like Response.error())",
        );
      }

      if (responseBodyUsed(response)) {
        throw new TypeError(
          "The body of the Response returned from the serve handler has already been consumed",
        );
      }
    } catch (error) {
      try {
        response = await onError(error);
        if (!ObjectPrototypeIsPrototypeOf(ResponsePrototype, response)) {
          throw new TypeError(
            "Return value from onError handler must be a response or a promise resolving to a response",
          );
        }
        inner = toInnerResponse(response);
        if (inner === undefined) {
          throw new TypeError(
            "Return value from onError handler must be a Response constructed via the Response constructor in this realm",
          );
        }
      } catch (error) {
        if (otelState.METRICS_ENABLED) {
          op_http_metric_handle_otel_error(req);
        }
        internals.log(
          "error",
          "Exception in onError while handling exception",
          error,
        );
        response = internalServerError();
        inner = toInnerResponse(response);
      }
    }

    if (span) {
      updateSpanFromServerResponse(span, response);
      // Copy span attributes (like http.route) to OtelInfo for HTTP metrics.
      // Must be done here, before the request external is invalidated.
      const otelSpan = otelState.getOtelSpan?.(span);
      if (otelSpan) {
        op_http_copy_span_to_otel_info(req, otelSpan);
      }
    }

    if (innerRequest?.[_upgraded]) {
      if (response.status !== 101) {
        internals.log(
          "error",
          "Upgrade response was not returned from callback",
        );
        context.close();
        return;
      }
      if (response === UPGRADE_RESPONSE_SENTINEL) {
        return;
      }
    }

    // Did everything shut down while we were waiting?
    if (context.closed) {
      // We're shutting down, so this status shouldn't make it back to the client but "Service Unavailable" seems appropriate
      innerRequest?.close();
      op_http_set_promise_complete(req, 503);
      return;
    }

    const nativeResponse = response[serveNativeResponseKey];
    if (trySetServeFastStaticResponse(req, response, innerRequest)) {
      return;
    }
    if (
      nativeResponse !== null && nativeResponse !== undefined &&
      op_http_set_response_native(req, nativeResponse)
    ) {
      dropServeNativeResponse(response);
      response[serveFastConsumedKey] = true;
      innerRequest?.close();
      return;
    }

    inner = toInnerResponse(response);
    const status = inner.status;
    const headers = wireHeaderList(inner);
    const respBody = inner.body;
    fastSyncResponseOrStream(
      req,
      respBody,
      status,
      innerRequest,
      headers,
      (error) => reportResponseStreamError(onError, error),
    );
  };

  if (otelState.TRACING_ENABLED) {
    const origMapped = mapped;
    mapped = function (req, _span) {
      const snapshot = currentSnapshot();
      restoreSnapshot(context.asyncContext);

      const reqHeaders = op_http_get_request_headers(req);
      const headers: [key: string, value: string][] = [];
      for (let i = 0; i < reqHeaders.length; i += 2) {
        ArrayPrototypePush(headers, [reqHeaders[i], reqHeaders[i + 1]]);
      }
      let activeContext = ContextManager.active();
      for (const propagator of new SafeArrayIterator(otelState.PROPAGATORS)) {
        activeContext = propagator.extract(activeContext, headers, {
          get(carrier: [key: string, value: string][], key: string) {
            return ArrayPrototypeFind(
              carrier,
              (carrierEntry) => carrierEntry[0] === key,
            )?.[1];
          },
          keys(carrier: [key: string, value: string][]) {
            return ArrayPrototypeMap(
              carrier,
              (carrierEntry) => carrierEntry[0],
            );
          },
        });
      }

      const span = builtinTracer().startSpan(
        "deno.serve",
        { kind: 1 },
        activeContext,
      );
      enterSpan(span, activeContext);
      try {
        return SafePromisePrototypeFinally(
          origMapped(req, span),
          () => span.end(),
        );
      } finally {
        restoreSnapshot(snapshot);
      }
    };
  } else {
    const origMapped = mapped;
    mapped = function (req, span) {
      const snapshot = currentSnapshot();
      restoreSnapshot(context.asyncContext);
      try {
        return origMapped(req, span);
      } finally {
        restoreSnapshot(snapshot);
      }
    };
  }

  return mapped;
}

function mapToNativeResponseCallback(context, callback, onError) {
  const zeroArgCallback = callback.length === 0 &&
    !otelState.TRACING_ENABLED &&
    !otelState.METRICS_ENABLED;

  function finishOrReturnNative(
    req,
    span,
    innerRequest,
    response,
    fromPromise = false,
  ) {
    if (!ObjectPrototypeIsPrototypeOf(ResponsePrototype, response)) {
      throw new TypeError(
        "Return value from serve handler must be a response or a promise resolving to a response",
      );
    }
    let inner = getInnerResponse(response);
    if (inner === undefined) {
      throw new TypeError(
        "Return value from serve handler must be a Response constructed via the Response constructor in this realm",
      );
    }
    if (inner.type === "error") {
      throw new TypeError(
        "Return value from serve handler must not be an error response (like Response.error())",
      );
    }
    if (responseBodyUsed(response)) {
      throw new TypeError(
        "The body of the Response returned from the serve handler has already been consumed",
      );
    }

    if (
      innerRequest?.request !== undefined &&
      requestHeadersExposed(innerRequest.request)
    ) {
      cacheRequestHeaders(innerRequest.request);
    }

    if (span) {
      updateSpanFromServerResponse(span, response);
      const otelSpan = otelState.getOtelSpan?.(span);
      if (otelSpan) {
        op_http_copy_span_to_otel_info(req, otelSpan);
      }
    }

    if (context.closed) {
      innerRequest?.close();
      op_http_set_promise_complete(req, 503);
      return undefined;
    }

    if (innerRequest?.[_upgraded]) {
      if (response.status !== 101) {
        internals.log(
          "error",
          "Upgrade response was not returned from callback",
        );
        context.close();
        return undefined;
      }
      if (response === UPGRADE_RESPONSE_SENTINEL) {
        return undefined;
      }
    }

    if (
      trySetServeFastStaticResponse(
        req,
        response,
        innerRequest,
        fromPromise ? closeInnerRequestImmediately : closeInnerRequestForNative,
      )
    ) {
      return undefined;
    }

    const nativeResponse = response[serveNativeResponseKey];
    if (
      nativeResponse !== null && nativeResponse !== undefined &&
      op_http_set_response_native(req, nativeResponse)
    ) {
      dropServeNativeResponse(response);
      response[serveFastConsumedKey] = true;
      if (fromPromise) {
        closeInnerRequestImmediately(innerRequest);
      } else {
        closeInnerRequestForNative(innerRequest);
      }
      return undefined;
    }

    inner = toInnerResponse(response);
    fastSyncResponseOrStream(
      req,
      inner.body,
      inner.status,
      innerRequest,
      wireHeaderList(inner),
      (error) => reportResponseStreamError(onError, error),
    );
    return undefined;
  }

  function handleError(req, span, innerRequest, error) {
    let response;
    try {
      response = onError(error);
    } catch (error) {
      if (otelState.METRICS_ENABLED) {
        op_http_metric_handle_otel_error(req);
      }
      internals.log(
        "error",
        "Exception in onError while handling exception",
        error,
      );
      response = internalServerError();
    }
    try {
      return finishOrReturnMaybePromise(req, span, innerRequest, response);
    } catch (error) {
      if (otelState.METRICS_ENABLED) {
        op_http_metric_handle_otel_error(req);
      }
      internals.log(
        "error",
        "Exception in onError while handling exception",
        error,
      );
      return finishOrReturnNative(
        req,
        span,
        innerRequest,
        internalServerError(),
      );
    }
  }

  function finishOrReturnMaybePromise(req, span, innerRequest, response) {
    if (
      response !== null &&
      (typeof response === "object" || typeof response === "function") &&
      typeof response.then === "function"
    ) {
      return PromisePrototypeThen(
        PromiseResolve(response),
        (response) =>
          finishOrReturnNative(req, span, innerRequest, response, true),
        (error) => handleError(req, span, innerRequest, error),
      );
    }
    if (
      innerRequest?.request !== undefined &&
      requestHeadersExposed(innerRequest.request)
    ) {
      return PromisePrototypeThen(
        PromiseResolve(response),
        (response) =>
          finishOrReturnNative(req, span, innerRequest, response, true),
        (error) => handleError(req, span, innerRequest, error),
      );
    }
    return finishOrReturnNative(req, span, innerRequest, response);
  }

  return function nativeMapped(req, span) {
    let innerRequest;
    let response;
    try {
      if (zeroArgCallback && op_http_is_raw_request(req)) {
        response = callback();
      } else {
        innerRequest = new InnerRequest(req, context);
        const request = fromInnerRequest(innerRequest, "immutable");
        innerRequest.request = request;
        if (span) {
          updateSpanFromRequest(span, request);
        }
        response = callback(request, new ServeHandlerInfo(innerRequest));
      }
    } catch (error) {
      return handleError(req, span, innerRequest, error);
    }
    try {
      return finishOrReturnMaybePromise(req, span, innerRequest, response);
    } catch (error) {
      return handleError(req, span, innerRequest, error);
    }
  };
}

type RawHandler = (
  request: Request,
  info: ServeHandlerInfo,
) => Response | Promise<Response>;

type RawServeOptions = {
  port?: number;
  hostname?: string;
  signal?: AbortSignal;
  reusePort?: boolean;
  key?: string;
  cert?: string;
  onError?: (error: unknown) => Response | Promise<Response>;
  onListen?: (params: { hostname: string; port: number }) => void;
  handler?: RawHandler;
  automaticCompression?: boolean;
};

const kLoadBalanced = Symbol("kLoadBalanced");

function formatHostName(hostname: string): string {
  // If the hostname is "0.0.0.0", we display "localhost" in console
  // because browsers in Windows don't resolve "0.0.0.0".
  // See the discussion in https://github.com/denoland/deno_std/issues/1165
  if (
    Deno.build.os === "windows" &&
    (hostname == "0.0.0.0" || hostname == "::")
  ) {
    return "localhost";
  }

  // Add brackets around ipv6 hostname
  return StringPrototypeIncludes(hostname, ":") ? `[${hostname}]` : hostname;
}

// Flag to track if DENO_SERVE_ADDRESS override has been consumed
let serveAddressOverrideConsumed = false;

function serve(arg1, arg2) {
  let options: RawServeOptions | undefined;
  let handler: RawHandler | undefined;

  if (typeof arg1 === "function") {
    handler = arg1;
  } else if (typeof arg2 === "function") {
    handler = arg2;
    options = arg1;
  } else {
    options = arg1;
  }
  if (handler === undefined) {
    if (options === undefined) {
      throw new TypeError(
        "Cannot serve HTTP requests: either a `handler` or `options` must be specified",
      );
    }
    handler = options.handler;
  }
  if (typeof handler !== "function") {
    throw new TypeError(
      `Cannot serve HTTP requests: handler must be a function, received ${typeof handler}`,
    );
  }
  if (options === undefined) {
    options = { __proto__: null };
  }

  if (serveAddressOverrideConsumed) {
    return serveInner(options, handler);
  }

  const {
    0: overrideKind,
    1: overrideHost,
    2: overridePort,
    3: duplicateListener,
  } = op_http_serve_address_override();
  if (overrideKind) {
    serveAddressOverrideConsumed = true;

    let envOptions = duplicateListener
      ? {
        __proto__: null,
        signal: options.signal,
        onError: options.onError,
        automaticCompression: options.automaticCompression,
      }
      : options;

    switch (overrideKind) {
      case 1: {
        // TCP
        envOptions = {
          ...envOptions,
          hostname: overrideHost,
          port: overridePort,
        };
        delete envOptions.path;
        delete envOptions.cid;
        break;
      }
      case 2: {
        // Unix
        envOptions = {
          ...envOptions,
          path: overrideHost,
        };
        delete envOptions.hostname;
        delete envOptions.cid;
        delete envOptions.port;
        break;
      }
      case 3: {
        // Vsock
        envOptions = {
          ...envOptions,
          cid: Number(overrideHost),
          port: overridePort,
        };
        delete envOptions.hostname;
        delete envOptions.path;
        break;
      }
      case 4: {
        // Tunnel
        envOptions = {
          ...envOptions,
          tunnel: true,
        };
        delete envOptions.hostname;
        delete envOptions.cid;
        delete envOptions.port;
        delete envOptions.path;
      }
    }

    if (duplicateListener) {
      envOptions.onListen = () => {
        // override default console.log behavior
      };
      const envListener = serveInner(envOptions, handler);
      const userListener = serveInner(options, handler);

      return {
        addr: userListener.addr,
        finished: SafePromiseAll([envListener.finished, userListener.finished]),
        shutdown() {
          return SafePromiseAll([
            envListener.shutdown(),
            userListener.shutdown(),
          ]);
        },
        ref() {
          envListener.ref();
          userListener.ref();
        },
        unref() {
          envListener.unref();
          userListener.unref();
        },
        [SymbolAsyncDispose]() {
          return this.shutdown();
        },
      };
    }

    options = envOptions;
  }

  return serveInner(options, handler);
}

function serveInner(options, handler) {
  const wantsHttps = hasTlsKeyPairOptions(options);
  const wantsUnix = ObjectHasOwn(options, "path");
  const wantsVsock = ObjectHasOwn(options, "cid");
  const wantsTunnel = options.tunnel === true;
  const automaticCompression = options.automaticCompression ??
    op_http_serve_default_compression();
  const signal = options.signal;
  const onError = options.onError ??
    function (error) {
      internals.log("error", error);
      return internalServerError();
    };

  if (wantsUnix) {
    const listener = listen({
      transport: "unix",
      path: options.path,
      [listenOptionApiName]: "Deno.serve",
    });
    const path = listener.addr.path;
    return serveHttpOnListener(
      listener,
      signal,
      handler,
      onError,
      () => {
        if (options.onListen) {
          options.onListen(listener.addr);
        } else {
          internals.log("info", `Listening on ${path}`);
        }
      },
      automaticCompression,
    );
  }

  if (wantsVsock) {
    const listener = listen({
      transport: "vsock",
      cid: options.cid,
      port: options.port,
      [listenOptionApiName]: "Deno.serve",
    });
    const { cid, port } = listener.addr;
    return serveHttpOnListener(
      listener,
      signal,
      handler,
      onError,
      () => {
        if (options.onListen) {
          options.onListen(listener.addr);
        } else {
          internals.log("info", `Listening on vsock:${cid}:${port}`);
        }
      },
      automaticCompression,
    );
  }

  if (wantsTunnel) {
    const listener = listen({
      transport: "tunnel",
      [listenOptionApiName]: "Deno.serve",
    });
    return serveHttpOnListener(
      listener,
      signal,
      handler,
      onError,
      () => {
        if (options.onListen) {
          options.onListen(listener.addr);
        } else {
          const additional = listener.addr.port === 443
            ? ""
            : `:${listener.addr.port}`;
          internals.log(
            "info",
            `Listening on https://${
              formatHostName(listener.addr.hostname)
            }${additional}`,
          );
        }
      },
      automaticCompression,
    );
  }

  const listenOpts = {
    hostname: options.hostname ?? "0.0.0.0",
    port: options.port ?? 8000,
    reusePort: options.reusePort ?? false,
    loadBalanced: options[kLoadBalanced] ?? false,
    tcpBacklog: options.tcpBacklog,
  };

  if (options.certFile || options.keyFile) {
    throw new TypeError(
      "Unsupported 'certFile' / 'keyFile' options provided: use 'cert' / 'key' instead.",
    );
  }
  if (options.alpnProtocols) {
    throw new TypeError(
      "Unsupported 'alpnProtocols' option provided. 'h2' and 'http/1.1' are automatically supported.",
    );
  }

  let listener;
  if (wantsHttps) {
    if (!options.cert || !options.key) {
      throw new TypeError(
        "Both 'cert' and 'key' must be provided to enable HTTPS",
      );
    }
    listenOpts.cert = options.cert;
    listenOpts.key = options.key;
    listenOpts.alpnProtocols = ["h2", "http/1.1"];
    listener = listenTls(listenOpts);
    listenOpts.port = listener.addr.port;
  } else {
    listener = listen(listenOpts);
    listenOpts.port = listener.addr.port;
  }

  const addr = listener.addr;

  const onListen = (scheme) => {
    if (options.onListen) {
      options.onListen(addr);
    } else {
      const host = formatHostName(addr.hostname);

      const url = `${scheme}${host}:${addr.port}/`;
      const helper = host !== "localhost" &&
          (addr.hostname === "0.0.0.0" || addr.hostname === "::")
        ? ` (${scheme}localhost:${addr.port}/)`
        : "";

      internals.log("info", `Listening on ${url}${helper}`);
    }
  };

  return serveHttpOnListener(
    listener,
    signal,
    handler,
    onError,
    onListen,
    automaticCompression,
  );
}

/**
 * Serve HTTP/1.1 and/or HTTP/2 on an arbitrary listener.
 */
function serveHttpOnListener(
  listener,
  signal,
  handler,
  onError,
  onListen,
  automaticCompression = op_http_serve_default_compression(),
) {
  let serverContext = undefined;
  let callback = undefined;
  let nativeCallback = undefined;
  const promiseErrorHandler = (error) => {
    internals.log(
      "error",
      "Terminating Deno.serve loop due to unexpected error",
      error,
    );
    serverContext?.close();
  };
  const dispatch = (req) => {
    PromisePrototypeCatch(callback(req, undefined), promiseErrorHandler);
  };
  const nativeFastPath = !otelState.TRACING_ENABLED &&
    !otelState.METRICS_ENABLED;
  const nativeDispatch = (req) => {
    if (!nativeFastPath) {
      PromisePrototypeCatch(callback(req, undefined), promiseErrorHandler);
      return undefined;
    }
    return nativeCallback(req, undefined);
  };
  const rawNoRequest = handler.length === 0 && nativeFastPath;
  serverContext = new CallbackContext(
    signal,
    op_http_serve(
      listener[internalRidSymbol],
      automaticCompression,
      dispatch,
      rawNoRequest,
      nativeDispatch,
      serveNativeResponseKey,
      serveFastStatusKey,
      serveFastBodyKey,
      serveFastHeaderKindKey,
      serveFastContentTypeKey,
      serveFastConsumedKey,
    ),
    listener,
  );
  callback = mapToCallback(serverContext, handler, onError);
  nativeCallback = mapToNativeResponseCallback(serverContext, handler, onError);

  onListen(serverContext.scheme);

  return serveHttpOn(serverContext, listener.addr);
}

/**
 * Serve HTTP/1.1 and/or HTTP/2 on an arbitrary connection.
 */
function serveHttpOnConnection(connection, signal, handler, onError, onListen) {
  let serverContext = undefined;
  let callback = undefined;
  let nativeCallback = undefined;
  const promiseErrorHandler = (error) => {
    internals.log(
      "error",
      "Terminating Deno.serve loop due to unexpected error",
      error,
    );
    serverContext?.close();
  };
  const dispatch = (req) => {
    PromisePrototypeCatch(callback(req, undefined), promiseErrorHandler);
  };
  const nativeFastPath = !otelState.TRACING_ENABLED &&
    !otelState.METRICS_ENABLED;
  const nativeDispatch = (req) => {
    if (!nativeFastPath) {
      PromisePrototypeCatch(callback(req, undefined), promiseErrorHandler);
      return undefined;
    }
    return nativeCallback(req, undefined);
  };
  const rawNoRequest = handler.length === 0 && nativeFastPath;
  const automaticCompression = op_http_serve_default_compression();
  serverContext = new CallbackContext(
    signal,
    op_http_serve_on(
      connection[internalRidSymbol],
      automaticCompression,
      dispatch,
      rawNoRequest,
      nativeDispatch,
      serveNativeResponseKey,
      serveFastStatusKey,
      serveFastBodyKey,
      serveFastHeaderKindKey,
      serveFastContentTypeKey,
      serveFastConsumedKey,
    ),
    null,
  );
  callback = mapToCallback(serverContext, handler, onError);
  nativeCallback = mapToNativeResponseCallback(serverContext, handler, onError);

  onListen(serverContext.scheme);

  return serveHttpOn(serverContext, connection.localAddr);
}

function serveHttpOn(context, addr) {
  let ref = true;
  let currentPromise = null;

  // Run the server
  const finished = (async () => {
    const rid = context.serverRid;
    try {
      currentPromise = op_http_wait(rid);
      if (!ref) {
        core.unrefOpPromise(currentPromise);
      }
      await currentPromise;
      currentPromise = null;
    } catch (error) {
      if (
        !ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) &&
        !ObjectPrototypeIsPrototypeOf(InterruptedPrototype, error)
      ) {
        throw new Deno.errors.Http(error);
      }
    }

    try {
      if (!context.closing && !context.closed) {
        context.closing = await op_http_close(rid, false);
        context.close();
      }

      await context.closing;
    } catch (error) {
      if (ObjectPrototypeIsPrototypeOf(InterruptedPrototype, error)) {
        return;
      }
      if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
        return;
      }

      throw error;
    } finally {
      context.close();
      context.closed = true;
    }
  })();

  op_http_notify_serving();

  return {
    addr,
    finished,
    async shutdown() {
      try {
        if (!context.closing && !context.closed) {
          // Shut this HTTP server down gracefully
          context.closing = op_http_close(context.serverRid, true);
        }

        await context.closing;
      } catch (error) {
        // The server was interrupted
        if (ObjectPrototypeIsPrototypeOf(InterruptedPrototype, error)) {
          return;
        }
        if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
          return;
        }

        throw error;
      } finally {
        context.closed = true;
      }
    },
    ref() {
      ref = true;
      if (currentPromise) {
        core.refOpPromise(currentPromise);
      }
    },
    unref() {
      ref = false;
      if (currentPromise) {
        core.unrefOpPromise(currentPromise);
      }
    },
    [SymbolAsyncDispose]() {
      return this.shutdown();
    },
  };
}

internals.addTrailers = addTrailers;
internals.upgradeHttpRaw = upgradeHttpRaw;
internals.serveHttpOnListener = serveHttpOnListener;
internals.serveHttpOnConnection = serveHttpOnConnection;
internals.resetLegacyAbortWarning = () => {
  legacyAbortWarned = false;
};

function registerDeclarativeServer(exports) {
  if (!ObjectHasOwn(exports, "fetch")) return;

  if (typeof exports.fetch !== "function") {
    throw new TypeError("Invalid type for fetch: must be a function");
  }

  if (
    exports.onListen !== undefined && typeof exports.onListen !== "function"
  ) {
    throw new TypeError("Invalid type for onListen: must be a function");
  }

  return ({
    servePort,
    serveHost,
    workerCountWhenMain,
  }) => {
    const server = Deno.serve({
      port: servePort,
      hostname: serveHost,
      [kLoadBalanced]: workerCountWhenMain == null
        ? true
        : workerCountWhenMain > 0,
      onListen: (localAddr) => {
        if (workerCountWhenMain != null) {
          if (exports.onListen) {
            exports.onListen(localAddr);
            return;
          }

          let target;
          switch (localAddr.transport) {
            case "tcp":
              target = `http://${
                formatHostName(localAddr.hostname)
              }:${localAddr.port}/`;
              break;
            case "unix":
              target = localAddr.path;
              break;
            case "vsock":
              target = `vsock:${localAddr.cid}:${localAddr.port}`;
              break;
          }

          const nThreads = workerCountWhenMain > 0
            ? ` with ${workerCountWhenMain + 1} threads`
            : "";

          internals.log(
            "info",
            `%cdeno serve%c: Listening on %c${target}%c${nThreads}`,
            "color: green",
            "color: inherit",
            "color: yellow",
            "color: inherit",
          );
        }
      },
      handler: (req, connInfo) => {
        return exports.fetch(req, connInfo);
      },
    });

    // Wire SIGTERM/SIGINT to a graceful server shutdown so that `deno serve`
    // drains in-flight requests and exits cleanly (exit code 0) instead of
    // being terminated by the OS default signal handler (exit code 143/130),
    // e.g. when a container is redeployed.
    const shutdownHandler = () => {
      // Stop listening for the signal so a second SIGTERM/SIGINT falls through
      // to the default handler and forcibly terminates a server that is slow
      // to drain.
      Deno.removeSignalListener("SIGTERM", shutdownHandler);
      Deno.removeSignalListener("SIGINT", shutdownHandler);
      // `shutdown()` already swallows the errors from an interrupted server,
      // but guard against any rejection becoming unhandled.
      PromisePrototypeCatch(server.shutdown(), () => {});
    };
    try {
      Deno.addSignalListener("SIGTERM", shutdownHandler);
      Deno.addSignalListener("SIGINT", shutdownHandler);
    } catch {
      // Adding signal listeners can fail in restricted environments; fall back
      // to the default behavior in that case.
    }
  };
}

return {
  addTrailers,
  registerDeclarativeServer,
  serve,
  serveHttpOnConnection,
  serveHttpOnListener,
  upgradeHttpRaw,
};
})();
