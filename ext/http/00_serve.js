// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file camelcase
const core = globalThis.Deno.core;
const primordials = globalThis.__bootstrap.primordials;
const internals = globalThis.__bootstrap.internals;

const { BadResourcePrototype } = core;
import { InnerBody } from "ext:deno_fetch/22_body.js";
import { Event } from "ext:deno_web/02_event.js";
import {
  fromInnerResponse,
  newInnerResponse,
  toInnerResponse,
} from "ext:deno_fetch/23_response.js";
import { fromInnerRequest, toInnerRequest } from "ext:deno_fetch/23_request.js";
import { AbortController } from "ext:deno_web/03_abort_signal.js";
import {
  _eventLoop,
  _idleTimeoutDuration,
  _idleTimeoutTimeout,
  _protocol,
  _readyState,
  _rid,
  _role,
  _server,
  _serverHandleIdleTimeout,
  SERVER,
  WebSocket,
} from "ext:deno_websocket/01_websocket.js";
import {
  Deferred,
  getReadableStreamResourceBacking,
  readableStreamClose,
  readableStreamForRid,
  ReadableStreamPrototype,
} from "ext:deno_web/06_streams.js";
import { listen, TcpConn } from "ext:deno_net/01_net.js";
import { listenTls } from "ext:deno_net/02_tls.js";
const {
  ArrayPrototypePush,
  Error,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeCatch,
  SafeSet,
  SafeSetIterator,
  SetPrototypeAdd,
  SetPrototypeDelete,
  Symbol,
  SymbolFor,
  TypeError,
  Uint8Array,
  Uint8ArrayPrototype,
} = primordials;

const {
  op_http_get_request_headers,
  op_http_get_request_method_and_url,
  op_http_read_request_body,
  op_http_serve,
  op_http_serve_on,
  op_http_set_promise_complete,
  op_http_set_response_body_bytes,
  op_http_set_response_body_resource,
  op_http_set_response_body_stream,
  op_http_set_response_body_text,
  op_http_set_response_header,
  op_http_set_response_headers,
  op_http_set_response_trailers,
  op_http_upgrade_raw,
  op_http_upgrade_websocket_next,
  op_http_try_wait,
  op_http_wait,
} = core.ensureFastOps();
const _upgraded = Symbol("_upgraded");

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

function upgradeHttpRaw(req, conn) {
  const inner = toInnerRequest(req);
  if (inner._wantsUpgrade) {
    return inner._wantsUpgrade("upgradeHttpRaw", conn);
  }
  throw new TypeError("upgradeHttpRaw may only be used with Deno.serve");
}

function addTrailers(resp, headerList) {
  const inner = toInnerResponse(resp);
  op_http_set_response_trailers(inner.slabId, headerList);
}

class InnerRequest {
  #slabId;
  #context;
  #methodAndUri;
  #streamRid;
  #body;
  #upgraded;

  constructor(slabId, context) {
    this.#slabId = slabId;
    this.#context = context;
    this.#upgraded = false;
  }

  close() {
    if (this.#streamRid !== undefined) {
      core.close(this.#streamRid);
      this.#streamRid = undefined;
    }
    this.#slabId = undefined;
  }

  get [_upgraded]() {
    return this.#upgraded;
  }

  _wantsUpgrade(upgradeType, ...originalArgs) {
    if (this.#upgraded) {
      throw new Deno.errors.Http("already upgraded");
    }
    if (this.#slabId === undefined) {
      throw new Deno.errors.Http("already closed");
    }

    // upgradeHttp is async
    // TODO(mmastrac)
    if (upgradeType == "upgradeHttp") {
      throw "upgradeHttp is unavailable in Deno.serve at this time";
    }

    // upgradeHttpRaw is sync
    if (upgradeType == "upgradeHttpRaw") {
      const slabId = this.#slabId;
      const underlyingConn = originalArgs[0];

      this.url();
      this.headerList;
      this.close();

      this.#upgraded = () => {};

      const upgradeRid = op_http_upgrade_raw(slabId);

      const conn = new TcpConn(
        upgradeRid,
        underlyingConn?.remoteAddr,
        underlyingConn?.localAddr,
      );

      return { response: UPGRADE_RESPONSE_SENTINEL, conn };
    }

    // upgradeWebSocket is sync
    if (upgradeType == "upgradeWebSocket") {
      const response = originalArgs[0];
      const ws = originalArgs[1];

      const slabId = this.#slabId;

      this.url();
      this.headerList;
      this.close();

      const goAhead = new Deferred();
      this.#upgraded = () => {
        goAhead.resolve();
      };

      // Start the upgrade in the background.
      (async () => {
        try {
          // Returns the upgraded websocket connection
          const wsRid = await op_http_upgrade_websocket_next(
            slabId,
            response.headerList,
          );

          // We have to wait for the go-ahead signal
          await goAhead;

          ws[_rid] = wsRid;
          ws[_readyState] = WebSocket.OPEN;
          ws[_role] = SERVER;
          const event = new Event("open");
          ws.dispatchEvent(event);

          ws[_eventLoop]();
          if (ws[_idleTimeoutDuration]) {
            ws.addEventListener(
              "close",
              () => clearTimeout(ws[_idleTimeoutTimeout]),
            );
          }
          ws[_serverHandleIdleTimeout]();
        } catch (error) {
          const event = new ErrorEvent("error", { error });
          ws.dispatchEvent(event);
        }
      })();
      return { response: UPGRADE_RESPONSE_SENTINEL, socket: ws };
    }
  }

  url() {
    if (this.#methodAndUri === undefined) {
      if (this.#slabId === undefined) {
        throw new TypeError("request closed");
      }
      // TODO(mmastrac): This is quite slow as we're serializing a large number of values. We may want to consider
      // splitting this up into multiple ops.
      this.#methodAndUri = op_http_get_request_method_and_url(this.#slabId);
    }

    const path = this.#methodAndUri[2];

    // * is valid for OPTIONS
    if (path === "*") {
      return "*";
    }

    // If the path is empty, return the authority (valid for CONNECT)
    if (path == "") {
      return this.#methodAndUri[1];
    }

    // CONNECT requires an authority
    if (this.#methodAndUri[0] == "CONNECT") {
      return this.#methodAndUri[1];
    }

    const hostname = this.#methodAndUri[1];
    if (hostname) {
      // Construct a URL from the scheme, the hostname, and the path
      return this.#context.scheme + hostname + path;
    }

    // Construct a URL from the scheme, the fallback hostname, and the path
    return this.#context.scheme + this.#context.fallbackHost + path;
  }

  get remoteAddr() {
    if (this.#methodAndUri === undefined) {
      if (this.#slabId === undefined) {
        throw new TypeError("request closed");
      }
      this.#methodAndUri = op_http_get_request_method_and_url(this.#slabId);
    }
    return {
      transport: "tcp",
      hostname: this.#methodAndUri[3],
      port: this.#methodAndUri[4],
    };
  }

  get method() {
    if (this.#methodAndUri === undefined) {
      if (this.#slabId === undefined) {
        throw new TypeError("request closed");
      }
      this.#methodAndUri = op_http_get_request_method_and_url(this.#slabId);
    }
    return this.#methodAndUri[0];
  }

  get body() {
    if (this.#slabId === undefined) {
      throw new TypeError("request closed");
    }
    if (this.#body !== undefined) {
      return this.#body;
    }
    // If the method is GET or HEAD, we do not want to include a body here, even if the Rust
    // side of the code is willing to provide it to us.
    if (this.method == "GET" || this.method == "HEAD") {
      this.#body = null;
      return null;
    }
    this.#streamRid = op_http_read_request_body(this.#slabId);
    this.#body = new InnerBody(readableStreamForRid(this.#streamRid, false));
    return this.#body;
  }

  get headerList() {
    if (this.#slabId === undefined) {
      throw new TypeError("request closed");
    }
    const headers = [];
    const reqHeaders = op_http_get_request_headers(this.#slabId);
    for (let i = 0; i < reqHeaders.length; i += 2) {
      ArrayPrototypePush(headers, [reqHeaders[i], reqHeaders[i + 1]]);
    }
    return headers;
  }

  get slabId() {
    return this.#slabId;
  }
}

class CallbackContext {
  abortController;
  responseBodies;
  scheme;
  fallbackHost;
  serverRid;
  closed;

  constructor(signal, args) {
    signal?.addEventListener(
      "abort",
      () => this.close(),
      { once: true },
    );
    this.abortController = new AbortController();
    this.responseBodies = new SafeSet();
    this.serverRid = args[0];
    this.scheme = args[1];
    this.fallbackHost = args[2];
    this.closed = false;
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

function fastSyncResponseOrStream(req, respBody) {
  if (respBody === null || respBody === undefined) {
    // Don't set the body
    return null;
  }

  const stream = respBody.streamOrStatic;
  const body = stream.body;

  if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, body)) {
    op_http_set_response_body_bytes(req, body);
    return null;
  }

  if (typeof body === "string") {
    op_http_set_response_body_text(req, body);
    return null;
  }

  // At this point in the response it needs to be a stream
  if (!ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, stream)) {
    throw TypeError("invalid response");
  }
  const resourceBacking = getReadableStreamResourceBacking(stream);
  if (resourceBacking) {
    op_http_set_response_body_resource(
      req,
      resourceBacking.rid,
      resourceBacking.autoClose,
    );
    return null;
  }

  return stream;
}

async function asyncResponse(responseBodies, req, status, stream) {
  const reader = stream.getReader();
  let responseRid;
  let closed = false;
  let timeout;

  try {
    // IMPORTANT: We get a performance boost from this optimization, but V8 is very
    // sensitive to the order and structure. Benchmark any changes to this code.

    // Optimize for streams that are done in zero or one packets. We will not
    // have to allocate a resource in this case.
    const { value: value1, done: done1 } = await reader.read();
    if (done1) {
      closed = true;
      // Exit 1: no response body at all, extreme fast path
      // Reader will be closed by finally block
      return;
    }

    // The second value cannot block indefinitely, as someone may be waiting on a response
    // of the first packet that may influence this packet. We set this timeout arbitrarily to 250ms
    // and we race it.
    let timeoutPromise;
    timeout = setTimeout(() => {
      responseRid = op_http_set_response_body_stream(req);
      SetPrototypeAdd(responseBodies, responseRid);
      op_http_set_promise_complete(req, status);
      // TODO(mmastrac): if this promise fails before we get to the await below, it crashes
      // the process with an error:
      //
      // 'Uncaught (in promise) BadResource: failed to write'.
      //
      // To avoid this, we're going to swallow errors here and allow the code later in the
      // file to re-throw them in a way that doesn't appear to be an uncaught promise rejection.
      timeoutPromise = core.writeAll(responseRid, value1).catch(() => null);
    }, 250);
    const { value: value2, done: done2 } = await reader.read();

    if (timeoutPromise) {
      await timeoutPromise;
      if (done2) {
        closed = true;
        // Exit 2(a): read 2 is EOS, and timeout resolved.
        // Reader will be closed by finally block
        // Response stream will be closed by finally block.
        return;
      }

      // Timeout resolved, value1 written but read2 is not EOS. Carry value2 forward.
    } else {
      clearTimeout(timeout);
      timeout = undefined;

      if (done2) {
        // Exit 2(b): read 2 is EOS, and timeout did not resolve as we read fast enough.
        // Reader will be closed by finally block
        // No response stream
        closed = true;
        op_http_set_response_body_bytes(req, value1);
        return;
      }

      responseRid = op_http_set_response_body_stream(req);
      SetPrototypeAdd(responseBodies, responseRid);
      op_http_set_promise_complete(req, status);
      // Write our first packet
      await core.writeAll(responseRid, value1);
    }

    await core.writeAll(responseRid, value2);
    while (true) {
      const { value, done } = await reader.read();
      if (done) {
        closed = true;
        break;
      }
      await core.writeAll(responseRid, value);
    }
  } catch (error) {
    closed = true;
    try {
      await reader.cancel(error);
    } catch {
      // Pass
    }
  } finally {
    if (!closed) {
      readableStreamClose(reader);
    }
    if (timeout !== undefined) {
      clearTimeout(timeout);
    }
    if (responseRid) {
      core.tryClose(responseRid);
      SetPrototypeDelete(responseBodies, responseRid);
    } else {
      op_http_set_promise_complete(req, status);
    }
  }
}

/**
 * Maps the incoming request slab ID to a fully-fledged Request object, passes it to the user-provided
 * callback, then extracts the response that was returned from that callback. The response is then pulled
 * apart and handled on the Rust side.
 *
 * This function returns a promise that will only reject in the case of abnormal exit.
 */
function mapToCallback(context, callback, onError) {
  const responseBodies = context.responseBodies;
  const signal = context.abortController.signal;
  const hasCallback = callback.length > 0;
  const hasOneCallback = callback.length === 1;

  return async function (req) {
    // Get the response from the user-provided callback. If that fails, use onError. If that fails, return a fallback
    // 500 error.
    let innerRequest;
    let response;
    try {
      if (hasCallback) {
        innerRequest = new InnerRequest(req, context);
        const request = fromInnerRequest(innerRequest, signal, "immutable");
        if (hasOneCallback) {
          response = await callback(request);
        } else {
          response = await callback(request, {
            get remoteAddr() {
              return innerRequest.remoteAddr;
            },
          });
        }
      } else {
        response = await callback();
      }
    } catch (error) {
      try {
        response = await onError(error);
      } catch (error) {
        console.error("Exception in onError while handling exception", error);
        response = internalServerError();
      }
    }

    const inner = toInnerResponse(response);
    if (innerRequest?.[_upgraded]) {
      // We're done here as the connection has been upgraded during the callback and no longer requires servicing.
      if (response !== UPGRADE_RESPONSE_SENTINEL) {
        console.error("Upgrade response was not returned from callback");
        context.close();
      }
      innerRequest?.[_upgraded]();
      return;
    }

    // Did everything shut down while we were waiting?
    if (context.closed) {
      // We're shutting down, so this status shouldn't make it back to the client but "Service Unavailable" seems appropriate
      op_http_set_promise_complete(req, 503);
      innerRequest?.close();
      return;
    }

    const status = inner.status;
    const headers = inner.headerList;
    if (headers && headers.length > 0) {
      if (headers.length == 1) {
        op_http_set_response_header(req, headers[0][0], headers[0][1]);
      } else {
        op_http_set_response_headers(req, headers);
      }
    }

    // Attempt to respond quickly to this request, otherwise extract the stream
    const stream = fastSyncResponseOrStream(req, inner.body);
    if (stream !== null) {
      // Handle the stream asynchronously
      await asyncResponse(responseBodies, req, status, stream);
    } else {
      op_http_set_promise_complete(req, status);
    }

    innerRequest?.close();
  };
}

function serve(arg1, arg2) {
  let options = undefined;
  let handler = undefined;
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
        "No handler was provided, so an options bag is mandatory.",
      );
    }
    handler = options.handler;
  }
  if (typeof handler !== "function") {
    throw new TypeError("A handler function must be provided.");
  }
  if (options === undefined) {
    options = {};
  }

  const wantsHttps = options.cert || options.key;
  const signal = options.signal;
  const onError = options.onError ?? function (error) {
    console.error(error);
    return internalServerError();
  };
  const listenOpts = {
    hostname: options.hostname ?? "0.0.0.0",
    port: options.port ?? (wantsHttps ? 9000 : 8000),
    reusePort: options.reusePort ?? false,
  };

  let listener;
  if (wantsHttps) {
    if (!options.cert || !options.key) {
      throw new TypeError(
        "Both cert and key must be provided to enable HTTPS.",
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

  const onListen = (scheme) => {
    // If the hostname is "0.0.0.0", we display "localhost" in console
    // because browsers in Windows don't resolve "0.0.0.0".
    // See the discussion in https://github.com/denoland/deno_std/issues/1165
    const hostname = listenOpts.hostname == "0.0.0.0"
      ? "localhost"
      : listenOpts.hostname;
    const port = listenOpts.port;

    if (options.onListen) {
      options.onListen({ hostname, port });
    } else {
      console.log(`Listening on ${scheme}${hostname}:${port}/`);
    }
  };

  return serveHttpOnListener(listener, signal, handler, onError, onListen);
}

/**
 * Serve HTTP/1.1 and/or HTTP/2 on an arbitrary listener.
 */
function serveHttpOnListener(listener, signal, handler, onError, onListen) {
  const context = new CallbackContext(signal, op_http_serve(listener.rid));
  const callback = mapToCallback(context, handler, onError);

  onListen(context.scheme);

  return serveHttpOn(context, callback);
}

/**
 * Serve HTTP/1.1 and/or HTTP/2 on an arbitrary connection.
 */
function serveHttpOnConnection(connection, signal, handler, onError, onListen) {
  const context = new CallbackContext(signal, op_http_serve_on(connection.rid));
  const callback = mapToCallback(context, handler, onError);

  onListen(context.scheme);

  return serveHttpOn(context, callback);
}

function serveHttpOn(context, callback) {
  let ref = true;
  let currentPromise = null;
  const promiseIdSymbol = SymbolFor("Deno.core.internalPromiseId");

  const promiseErrorHandler = (error) => {
    // Abnormal exit
    console.error(
      "Terminating Deno.serve loop due to unexpected error",
      error,
    );
    context.close();
  };

  // Run the server
  const finished = (async () => {
    const rid = context.serverRid;
    while (true) {
      let req;
      try {
        // Attempt to pull as many requests out of the queue as possible before awaiting. This API is
        // a synchronous, non-blocking API that returns u32::MAX if anything goes wrong.
        while ((req = op_http_try_wait(rid)) !== 0xffffffff) {
          PromisePrototypeCatch(callback(req), promiseErrorHandler);
        }
        currentPromise = op_http_wait(rid);
        if (!ref) {
          core.unrefOp(currentPromise[promiseIdSymbol]);
        }
        req = await currentPromise;
        currentPromise = null;
      } catch (error) {
        if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
          break;
        }
        throw new Deno.errors.Http(error);
      }
      if (req === 0xffffffff) {
        break;
      }
      PromisePrototypeCatch(callback(req), promiseErrorHandler);
    }

    for (const streamRid of new SafeSetIterator(context.responseBodies)) {
      core.tryClose(streamRid);
    }
  })();

  return {
    finished,
    then() {
      throw new Error(
        "Deno.serve no longer returns a promise. await server.finished instead of server.",
      );
    },
    ref() {
      ref = true;
      if (currentPromise) {
        core.refOp(currentPromise[promiseIdSymbol]);
      }
    },
    unref() {
      ref = false;
      if (currentPromise) {
        core.unrefOp(currentPromise[promiseIdSymbol]);
      }
    },
  };
}

internals.addTrailers = addTrailers;
internals.upgradeHttpRaw = upgradeHttpRaw;
internals.serveHttpOnListener = serveHttpOnListener;
internals.serveHttpOnConnection = serveHttpOnConnection;

export {
  addTrailers,
  serve,
  serveHttpOnConnection,
  serveHttpOnListener,
  upgradeHttpRaw,
};
